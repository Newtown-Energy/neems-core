//! Modbus Worker Task
//!
//! This module implements the main worker task that handles:
//! - 10Hz read operations for data collection and alarm monitoring
//! - 2Hz write operations for schedule command execution
//! - Time-slotted single worker pattern (reads every tick, writes every 5th
//!   tick)

use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use chrono::Utc;
use tokio::{
    sync::{RwLock, mpsc, watch},
    time::{Instant, Interval, MissedTickBehavior, interval},
};
use tracing::{debug, error, info, trace, warn};

/// Consecutive refused analog reads before polling backs off.
///
/// Two full cycles, so a single pack answering intermittently does not stop
/// the sweep — only the whole block being rejected does. That is the expected
/// state until the client confirms the analog register addresses, and without
/// a limit it would cost a doomed round trip on every tick, forever.
const ANALOG_REFUSAL_LIMIT: u32 = 2 * MEGAPACK_ZONES.len() as u32;

/// How long to stop polling analogs after hitting [`ANALOG_REFUSAL_LIMIT`].
///
/// Long enough that a wrong address costs almost nothing, short enough that
/// correcting one is picked up without a restart.
const ANALOG_REFUSAL_BACKOFF: Duration = Duration::from_secs(60);

/// Round-robin bookkeeping for the per-Megapack analog poll.
///
/// Split out from the worker because this is the part with the interesting
/// rules — which pack is next, when to stop trying, what to discard — and
/// leaving it inline would make it reachable only through a live Modbus
/// connection.
#[derive(Debug)]
struct AnalogPollState {
    /// Latest block per pack, indexed by position in [`MEGAPACK_ZONES`].
    packs: [Option<MegapackAnalogs>; MEGAPACK_ZONES.len()],
    next: usize,
    /// Consecutive refusals across all packs; any successful read resets it.
    refusals: u32,
    suspended_until: Option<Instant>,
}

impl AnalogPollState {
    fn new() -> Self {
        Self {
            packs: [const { None }; MEGAPACK_ZONES.len()],
            next: 0,
            refusals: 0,
            suspended_until: None,
        }
    }

    /// The pack to poll now, advancing the round-robin, or `None` while backed
    /// off.
    fn take_next(&mut self, now: Instant) -> Option<usize> {
        if let Some(until) = self.suspended_until {
            if now < until {
                return None;
            }
            info!("Resuming Megapack analog polling after backoff");
            self.suspended_until = None;
            self.refusals = 0;
        }
        let pack_index = self.next;
        self.next = (pack_index + 1) % MEGAPACK_ZONES.len();
        Some(pack_index)
    }

    fn record_read(&mut self, pack_index: usize, analogs: MegapackAnalogs) {
        self.refusals = 0;
        self.packs[pack_index] = Some(analogs);
    }

    /// Note a refusal, suspending the poll once every pack has refused twice
    /// over. Returns whether this call is what triggered the backoff, so the
    /// caller logs it once rather than on every subsequent refusal.
    fn record_refused(&mut self, pack_index: usize, now: Instant) -> bool {
        self.packs[pack_index] = None;
        self.refusals += 1;
        if self.refusals >= ANALOG_REFUSAL_LIMIT && self.suspended_until.is_none() {
            self.suspended_until = Some(now + ANALOG_REFUSAL_BACKOFF);
            return true;
        }
        false
    }

    fn record_unusable(&mut self, pack_index: usize) {
        self.packs[pack_index] = None;
    }

    /// Drop everything: the link died, so every held value is of unknown age.
    fn record_link_lost(&mut self) {
        self.packs = [const { None }; MEGAPACK_ZONES.len()];
        self.suspended_until = None;
        self.refusals = 0;
    }

    /// The blocks currently held, in [`MEGAPACK_ZONES`] order.
    fn collected(&self) -> Vec<MegapackAnalogs> {
        self.packs.iter().flatten().cloned().collect()
    }
}

/// Reason for worker shutdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// Shutdown was requested via the shutdown channel
    Requested,
    /// Storage channel was closed
    StorageChannelClosed,
    /// Alarm channel was closed
    AlarmChannelClosed,
}

use super::{
    alarm_definitions::ALARM_DEFINITIONS,
    alarms::Alarm,
    modbus_client::{MegapackAnalogRead, ModbusClient, ModbusClientConfig},
    protocol::{MEGAPACK_ZONES, MegapackAnalogs, ParsedStatus},
    state::{AlarmFlags, ConnectionStatus, PendingCommand, RtacReading, RtacState},
};

/// Configuration for the RTAC worker
#[derive(Debug, Clone)]
pub struct RtacConfig {
    /// RTAC IP address and port
    pub rtac_address: SocketAddr,
    /// Modbus slave ID
    pub slave_id: u8,
    /// Read interval (default: 100ms = 10Hz)
    pub read_interval: Duration,
    /// Write every N ticks (default: 5 = 2Hz when read is 10Hz)
    pub write_every_n_ticks: u32,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Operation timeout for individual read/write operations
    pub operation_timeout: Duration,
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Delay between reconnection attempts
    pub reconnect_delay: Duration,
    /// Sample rate for storage (store every N reads, default: 10 = 1Hz storage)
    pub storage_sample_rate: u32,
    /// Log read values every N reads (default: 10 = 1Hz logging)
    pub log_sample_rate: u32,
}

impl Default for RtacConfig {
    fn default() -> Self {
        Self {
            rtac_address: "127.0.0.1:502".parse().unwrap(),
            slave_id: 1,
            read_interval: Duration::from_millis(100), // 10Hz
            write_every_n_ticks: 5,                    // 2Hz writes
            connect_timeout: Duration::from_secs(5),
            operation_timeout: Duration::from_millis(500),
            max_reconnect_attempts: 10,
            reconnect_delay: Duration::from_secs(1),
            storage_sample_rate: 10, // 1Hz storage
            log_sample_rate: 10,     // 1Hz logging
        }
    }
}

impl RtacConfig {
    /// Create config with a specific RTAC address
    pub fn with_address(mut self, address: SocketAddr) -> Self {
        self.rtac_address = address;
        self
    }

    /// Create config with a specific slave ID
    pub fn with_slave_id(mut self, slave_id: u8) -> Self {
        self.slave_id = slave_id;
        self
    }

    /// Build a config from the environment, falling back to [`Default`].
    ///
    /// Honored variables:
    /// - `RTAC_ADDRESS`: the RTAC Modbus endpoint as `host:port` (a hostname,
    ///   e.g. the `neems-rtac-sim` service in docker, or an `ip:port`). The
    ///   hostname is resolved to a socket address.
    /// - `RTAC_SLAVE_ID`: the Modbus slave/unit id (u8).
    ///
    /// Invalid or unresolvable values are logged and the default is kept, so a
    /// misconfigured environment never prevents startup.
    pub fn from_env() -> Self {
        Self::from_env_values(
            std::env::var("RTAC_ADDRESS").ok().as_deref(),
            std::env::var("RTAC_SLAVE_ID").ok().as_deref(),
        )
    }

    /// Pure helper backing [`from_env`](Self::from_env): build a config from
    /// the already-extracted values, falling back to [`Default`] for
    /// anything missing or invalid. Kept free of environment access so it
    /// can be tested without mutating the process-wide environment.
    pub fn from_env_values(addr: Option<&str>, slave_id: Option<&str>) -> Self {
        let mut config = Self::default();

        if let Some(addr) = addr {
            match addr.to_socket_addrs() {
                Ok(mut addrs) => match addrs.next() {
                    Some(resolved) => config.rtac_address = resolved,
                    None => {
                        warn!(address = %addr, "RTAC_ADDRESS resolved to no addresses, using default")
                    }
                },
                Err(e) => {
                    warn!(address = %addr, error = %e, "Failed to resolve RTAC_ADDRESS, using default")
                }
            }
        }

        if let Some(slave_id) = slave_id {
            match slave_id.parse::<u8>() {
                Ok(parsed) => config.slave_id = parsed,
                Err(e) => {
                    warn!(slave_id = %slave_id, error = %e, "Invalid RTAC_SLAVE_ID, using default")
                }
            }
        }

        config
    }
}

/// Channels used by the worker for communication with other tasks
pub struct WorkerChannels {
    /// Shared state updated after each read
    pub state: Arc<RwLock<RtacState>>,
    /// Receiver for pending commands from control logic
    pub command_rx: watch::Receiver<Option<PendingCommand>>,
    /// Sender for readings to storage task
    pub storage_tx: mpsc::Sender<RtacReading>,
    /// Sender for alarms to alarm handler task
    pub alarm_tx: mpsc::UnboundedSender<Alarm>,
    /// Receiver for shutdown signal (any value triggers shutdown)
    pub shutdown_rx: watch::Receiver<bool>,
    /// Receiver for operator E-stop requests to signal, carrying the request id
    ///
    /// Deliberately separate from `command_rx`: that is a `watch`, which keeps
    /// only the latest value, so an E-stop placed there could be overwritten by
    /// the next schedule command before the worker ever wrote it. Sending the
    /// signal is the one thing this system owes an operator who asks for a
    /// trip.
    pub estop_rx: mpsc::UnboundedReceiver<i64>,
    /// Sender reporting the request ids whose E-stop reached the RTAC
    pub estop_sent_tx: mpsc::UnboundedSender<i64>,
}

/// Statistics for the worker
#[derive(Debug, Default, Clone)]
pub struct WorkerStats {
    /// Total number of read operations
    pub total_reads: u64,
    /// Number of successful reads
    pub successful_reads: u64,
    /// Number of failed reads
    pub failed_reads: u64,
    /// Total number of write operations
    pub total_writes: u64,
    /// Number of successful writes
    pub successful_writes: u64,
    /// Number of failed writes
    pub failed_writes: u64,
    /// Number of reconnection attempts
    pub reconnect_attempts: u64,
}

/// The main Modbus worker task
pub struct ModbusWorker {
    config: RtacConfig,
    client: ModbusClient,
    channels: WorkerChannels,
    stats: WorkerStats,
    tick_count: u64,
    sequence: u64,
    last_alarm_flags: AlarmFlags,
    /// Request id of an E-stop that has been asked for but not yet written.
    ///
    /// Held until the write succeeds so a failed or disconnected write is
    /// retried on the next tick rather than lost.
    pending_estop: Option<i64>,
    /// Per-Megapack analog polling, held here rather than in the shared
    /// [`RtacState`] because nothing outside storage consumes it, and parking
    /// it in state would put values refreshed once per cycle under a timestamp
    /// rewritten every tick.
    analogs: AnalogPollState,
}

impl ModbusWorker {
    /// Create a new Modbus worker
    pub fn new(config: RtacConfig, channels: WorkerChannels) -> Self {
        let client_config = ModbusClientConfig {
            address: config.rtac_address,
            slave_id: config.slave_id,
            connect_timeout: config.connect_timeout,
            operation_timeout: config.operation_timeout,
            max_reconnect_attempts: config.max_reconnect_attempts,
            reconnect_delay: config.reconnect_delay,
        };

        Self {
            config,
            client: ModbusClient::new(client_config),
            channels,
            stats: WorkerStats::default(),
            tick_count: 0,
            sequence: 0,
            last_alarm_flags: AlarmFlags::default(),
            pending_estop: None,
            analogs: AnalogPollState::new(),
        }
    }

    /// Run the worker loop
    ///
    /// This is the main entry point that runs the 10Hz loop. The loop will
    /// continue until a shutdown signal is received or a critical channel
    /// is closed.
    ///
    /// Returns the reason for shutdown.
    pub async fn run(
        &mut self,
    ) -> Result<ShutdownReason, Box<dyn std::error::Error + Send + Sync>> {
        info!(address = %self.config.rtac_address, "Starting Modbus worker");

        // Initial connection
        if let Err(e) = self.client.connect().await {
            error!(error = %e, "Initial connection failed, will retry in loop");
        }

        // Create the interval timer with skip behavior for missed ticks
        let mut interval = self.create_interval();

        loop {
            tokio::select! {
                // Check for shutdown signal
                result = self.channels.shutdown_rx.changed() => {
                    match result {
                        Ok(()) => {
                            if *self.channels.shutdown_rx.borrow() {
                                info!("Shutdown signal received, stopping worker");
                                return Ok(ShutdownReason::Requested);
                            }
                        }
                        Err(_) => {
                            // Sender dropped, treat as shutdown
                            info!("Shutdown channel closed, stopping worker");
                            return Ok(ShutdownReason::Requested);
                        }
                    }
                }

                // Regular tick processing
                _ = interval.tick() => {
                    self.tick_count += 1;

                    // Perform read operation (every tick = 10Hz)
                    let read_success = self.perform_read().await;

                    // Signal any operator E-stop (every tick = 10Hz, and ahead
                    // of the scheduled write). An operator asking for a trip
                    // should not wait out a write slot.
                    self.perform_estop_write().await;

                    // One Megapack analog block per tick, deliberately behind
                    // the E-stop signal and never as a batch: six serialized
                    // round trips ahead of it would be seconds of latency on
                    // exactly the request that must not wait.
                    let analog_link_ok = self.poll_next_megapack_analogs().await;

                    // Perform write operation (every Nth tick = 2Hz)
                    if self.tick_count.is_multiple_of(self.config.write_every_n_ticks as u64) {
                        self.perform_write().await;
                    }

                    // Log stats periodically (every 10 seconds at 10Hz = every 100 ticks)
                    if self.tick_count.is_multiple_of(100) {
                        self.log_stats();
                    }

                    // If either read dropped the link, reconnect. The analog
                    // poll is included because it can be what killed the
                    // connection, and skipping it here would leave shared
                    // state advertising Connected until the next tick's status
                    // read failed.
                    let needs_reconnect =
                        !(self.client.is_connected() || read_success && analog_link_ok);
                    if needs_reconnect {
                        self.handle_reconnection().await;
                    }
                }
            }
        }
    }

    /// Create the interval timer
    fn create_interval(&self) -> Interval {
        let mut int = interval(self.config.read_interval);
        int.set_missed_tick_behavior(MissedTickBehavior::Skip);
        int
    }

    /// Perform a read operation
    async fn perform_read(&mut self) -> bool {
        self.stats.total_reads += 1;

        match self.client.read_status().await {
            Ok(status) => {
                self.stats.successful_reads += 1;
                self.sequence += 1;

                // Update shared state
                self.update_state(&status).await;

                // Check for alarm changes
                self.check_alarms(&status).await;

                // Send to storage (sampled)
                if self.tick_count.is_multiple_of(self.config.storage_sample_rate as u64) {
                    self.send_to_storage().await;
                }

                // Debug logging (sampled)
                if self.tick_count.is_multiple_of(self.config.log_sample_rate as u64) {
                    debug!(
                        soc = status.soc_percent,
                        power_kw = status.power_kw,
                        mode = %status.mode,
                        sequence = self.sequence,
                        "RTAC status read"
                    );
                }

                trace!(
                    soc = status.soc_percent,
                    power_kw = status.power_kw,
                    voltage = status.voltage_v,
                    current = status.current_a,
                    temp = status.temperature_c,
                    frequency = status.grid_frequency_hz,
                    alarm_registers = ?status.alarm_registers,
                    "Raw status values"
                );

                true
            }
            Err(e) => {
                self.stats.failed_reads += 1;
                warn!(error = %e, "Read operation failed");

                // Update connection status in shared state
                self.update_connection_status(self.client.connection_status()).await;

                false
            }
        }
    }

    /// Write an operator's E-stop signal to the RTAC, if one has been asked
    /// for.
    ///
    /// This is the whole of the system's responsibility for an operator E-stop:
    /// get the signal to the RTAC. What the RTAC does with it — whether it
    /// latches, trips, or ignores it — is the RTAC's business, and is reported
    /// separately through alarm 104 in the readings feed.
    ///
    /// The signal is therefore retried until a write actually succeeds, and is
    /// only reported as sent once one has. A disconnected RTAC delays the
    /// signal; it must not silently swallow it.
    async fn perform_estop_write(&mut self) {
        // Take the most recent request; an older un-sent one is superseded
        // rather than queued behind, since both ask for the same thing.
        while let Ok(request_id) = self.channels.estop_rx.try_recv() {
            if let Some(superseded) = self.pending_estop.replace(request_id) {
                if superseded != request_id {
                    debug!(superseded, request_id, "Newer E-stop request supersedes an unsent one");
                }
            }
        }

        let Some(request_id) = self.pending_estop else {
            return;
        };

        let command = PendingCommand::emergency_stop(request_id);
        self.stats.total_writes += 1;

        match self.client.write_command(&command).await {
            Ok(()) => {
                self.stats.successful_writes += 1;
                warn!(request_id, "Operator E-stop signal written to RTAC");
                self.pending_estop = None;
                if self.channels.estop_sent_tx.send(request_id).is_err() {
                    error!(
                        request_id,
                        "E-stop was written but nothing is listening to report it as sent"
                    );
                }
            }
            Err(e) => {
                self.stats.failed_writes += 1;
                error!(
                    error = %e,
                    request_id,
                    "Operator E-stop write failed, retrying on the next tick"
                );
            }
        }
    }

    /// Perform a write operation if there's a pending command
    async fn perform_write(&mut self) {
        // Check if there's a new command
        let command = {
            let command_ref = self.channels.command_rx.borrow();
            command_ref.clone()
        };

        if let Some(ref cmd) = command {
            self.stats.total_writes += 1;

            match self.client.write_command(cmd).await {
                Ok(()) => {
                    self.stats.successful_writes += 1;
                    info!(
                        command_type = %cmd.command_type,
                        target_soc = ?cmd.target_soc_percent,
                        duration = ?cmd.duration_seconds,
                        "Command executed successfully"
                    );
                }
                Err(e) => {
                    self.stats.failed_writes += 1;
                    error!(
                        error = %e,
                        command_type = %cmd.command_type,
                        "Command execution failed"
                    );
                }
            }
        }
    }

    /// Update the shared state with new status data
    async fn update_state(&self, status: &ParsedStatus) {
        let mut state = self.channels.state.write().await;
        state.timestamp = Utc::now();
        state.soc_percent = status.soc_percent;
        state.power_kw = status.power_kw;
        state.mode = status.mode;
        state.voltage_v = status.voltage_v;
        state.current_a = status.current_a;
        state.temperature_c = status.temperature_c;
        state.grid_frequency_hz = status.grid_frequency_hz;
        state.alarms = AlarmFlags::from_registers(&status.alarm_registers);
        state.connection_status = ConnectionStatus::Connected;
        state.sequence = self.sequence;
    }

    /// Update only the connection status in shared state
    async fn update_connection_status(&self, status: ConnectionStatus) {
        let mut state = self.channels.state.write().await;
        state.connection_status = status;
        state.timestamp = Utc::now();
    }

    /// Check for alarm changes and send new alarms to the handler
    async fn check_alarms(&mut self, status: &ParsedStatus) {
        let new_flags = AlarmFlags::from_registers(&status.alarm_registers);

        for def in ALARM_DEFINITIONS {
            let was_active = self.last_alarm_flags.is_alarm_active(def);
            let is_active = new_flags.is_alarm_active(def);

            if !was_active && is_active {
                let alarm = Alarm::new(def);
                if let Err(e) = self.channels.alarm_tx.send(alarm) {
                    error!(
                        alarm = def.name,
                        alarm_num = def.alarm_num,
                        error = %e,
                        "Failed to send alarm notification"
                    );
                }
            } else if was_active && !is_active {
                let alarm = Alarm::cleared(def);
                if let Err(e) = self.channels.alarm_tx.send(alarm) {
                    error!(
                        alarm = def.name,
                        alarm_num = def.alarm_num,
                        error = %e,
                        "Failed to send alarm cleared notification"
                    );
                }
            }
        }

        self.last_alarm_flags = new_flags;
    }

    /// Send current state to storage
    async fn send_to_storage(&self) {
        let state = self.channels.state.read().await;
        let mut reading = RtacReading::from(&*state);
        reading.megapack_analogs = self.analogs.collected();

        if let Err(e) = self.channels.storage_tx.send(reading).await {
            warn!(error = %e, "Failed to send reading to storage");
        }
    }

    /// Poll one Megapack's analog block, advancing the round-robin.
    ///
    /// Returns `false` only when the link died, so the caller can reconnect.
    /// A refused or unusable pack is a per-pack outcome: it clears that pack
    /// and leaves the rest alone.
    ///
    /// Clearing on failure is what bounds staleness. Because a pack is dropped
    /// the moment it stops answering, every entry still present was read
    /// within the last full cycle, and the reading it is attached to can say
    /// so. Keeping the last good value instead would let a dead pack show a
    /// charge level indefinitely.
    async fn poll_next_megapack_analogs(&mut self) -> bool {
        let now = Instant::now();
        let Some(pack_index) = self.analogs.take_next(now) else {
            return true;
        };

        match self.client.read_megapack_analog_block(pack_index).await {
            Ok(MegapackAnalogRead::Read(analogs)) => {
                self.analogs.record_read(pack_index, analogs);
                true
            }
            Ok(MegapackAnalogRead::Refused) => {
                if self.analogs.record_refused(pack_index, now) {
                    warn!(
                        backoff_secs = ANALOG_REFUSAL_BACKOFF.as_secs(),
                        "Megapack analog reads refused across every pack; backing off. \
                         Expected if the analog register addresses are still provisional."
                    );
                }
                true
            }
            Ok(MegapackAnalogRead::Unusable) => {
                self.analogs.record_unusable(pack_index);
                true
            }
            Err(e) => {
                warn!(error = %e, "Megapack analog poll lost the connection");
                self.analogs.record_link_lost();
                false
            }
        }
    }

    /// Handle reconnection after connection loss
    async fn handle_reconnection(&mut self) {
        self.stats.reconnect_attempts += 1;
        warn!("Connection lost, attempting to reconnect");

        self.update_connection_status(ConnectionStatus::Reconnecting).await;

        match self.client.reconnect().await {
            Ok(()) => {
                info!("Reconnection successful");
                self.update_connection_status(ConnectionStatus::Connected).await;
            }
            Err(e) => {
                error!(error = %e, "Reconnection failed");
                self.update_connection_status(ConnectionStatus::Failed).await;
            }
        }
    }

    /// Log worker statistics
    fn log_stats(&self) {
        let read_success_rate = if self.stats.total_reads > 0 {
            (self.stats.successful_reads as f64 / self.stats.total_reads as f64) * 100.0
        } else {
            0.0
        };

        let write_success_rate = if self.stats.total_writes > 0 {
            (self.stats.successful_writes as f64 / self.stats.total_writes as f64) * 100.0
        } else {
            100.0
        };

        info!(
            total_reads = self.stats.total_reads,
            successful_reads = self.stats.successful_reads,
            read_success_rate = format!("{:.1}%", read_success_rate),
            total_writes = self.stats.total_writes,
            successful_writes = self.stats.successful_writes,
            write_success_rate = format!("{:.1}%", write_success_rate),
            reconnect_attempts = self.stats.reconnect_attempts,
            "Worker statistics"
        );
    }

    /// Get current statistics
    pub fn stats(&self) -> &WorkerStats {
        &self.stats
    }
}

/// The ends of the worker's channels that other tasks hold.
pub struct WorkerHandles {
    /// Send `true` to request graceful shutdown.
    pub shutdown_tx: watch::Sender<bool>,
    /// Ask the worker to signal an operator E-stop, by request id.
    pub estop_tx: mpsc::UnboundedSender<i64>,
    /// Receives request ids once their E-stop has been written to the RTAC.
    pub estop_sent_rx: mpsc::UnboundedReceiver<i64>,
}

/// Create the channels needed for the worker
///
/// Returns the worker channels and the handles other tasks use to drive it.
pub fn create_worker_channels(
    command_rx: watch::Receiver<Option<PendingCommand>>,
    storage_tx: mpsc::Sender<RtacReading>,
    alarm_tx: mpsc::UnboundedSender<Alarm>,
) -> (WorkerChannels, WorkerHandles) {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (estop_tx, estop_rx) = mpsc::unbounded_channel();
    let (estop_sent_tx, estop_sent_rx) = mpsc::unbounded_channel();
    let channels = WorkerChannels {
        state: Arc::new(RwLock::new(RtacState::default())),
        command_rx,
        storage_tx,
        alarm_tx,
        shutdown_rx,
        estop_rx,
        estop_sent_tx,
    };
    (channels, WorkerHandles { shutdown_tx, estop_tx, estop_sent_rx })
}

#[cfg(test)]
mod tests {
    use crate::rtac::{
        alarm_definitions::AlarmZone,
        protocol::{MP_ANALOG_POINT_COUNT, MegapackAnalogs},
    };

    fn analogs_for(zone: AlarmZone, soc: f32) -> MegapackAnalogs {
        MegapackAnalogs {
            zone,
            state_of_energy_percent: soc,
            ac_voltage_v: 480.0,
            max_battery_temperature_c: 25.0,
            raw_registers: [0u16; MP_ANALOG_POINT_COUNT],
        }
    }

    #[test]
    fn analog_poll_visits_every_pack_in_turn() {
        let mut poll = AnalogPollState::new();
        let now = Instant::now();

        let visited: Vec<usize> =
            (0..MEGAPACK_ZONES.len() * 2).filter_map(|_| poll.take_next(now)).collect();

        assert_eq!(visited, vec![0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn analog_poll_keeps_packs_in_block_order_regardless_of_arrival() {
        // Collected order must follow MEGAPACK_ZONES, not the order packs
        // happened to answer in, so consumers can rely on it.
        let mut poll = AnalogPollState::new();
        poll.record_read(4, analogs_for(AlarmZone::Mp2b, 52.25));
        poll.record_read(0, analogs_for(AlarmZone::Mp1a, 46.25));
        poll.record_read(2, analogs_for(AlarmZone::Mp1c, 49.25));

        let zones: Vec<AlarmZone> = poll.collected().iter().map(|a| a.zone).collect();
        assert_eq!(zones, vec![AlarmZone::Mp1a, AlarmZone::Mp1c, AlarmZone::Mp2b]);
    }

    #[test]
    fn a_pack_that_stops_answering_is_dropped_not_held() {
        // The staleness bound depends on this: anything still collected was
        // read within the last cycle. A retained last-good value would let a
        // dead pack show a charge level forever.
        let mut poll = AnalogPollState::new();
        poll.record_read(0, analogs_for(AlarmZone::Mp1a, 46.25));
        poll.record_read(1, analogs_for(AlarmZone::Mp1b, 47.75));
        assert_eq!(poll.collected().len(), 2);

        poll.record_refused(0, Instant::now());
        assert_eq!(poll.collected().len(), 1);

        poll.record_unusable(1);
        assert!(poll.collected().is_empty());
    }

    #[test]
    fn analog_poll_backs_off_once_every_pack_refuses() {
        let mut poll = AnalogPollState::new();
        let now = Instant::now();

        // One refusal short of the limit, polling continues.
        for _ in 0..ANALOG_REFUSAL_LIMIT - 1 {
            let pack = poll.take_next(now).expect("still polling");
            assert!(!poll.record_refused(pack, now), "backed off too early");
        }
        let pack = poll.take_next(now).expect("still polling");
        assert!(poll.record_refused(pack, now), "should report the backoff once");

        // Suspended: no pack is offered, and no further backoff is announced.
        assert_eq!(poll.take_next(now), None);
        assert_eq!(poll.take_next(now + ANALOG_REFUSAL_BACKOFF / 2), None);

        // ...and resumes once the backoff elapses.
        assert_eq!(poll.take_next(now + ANALOG_REFUSAL_BACKOFF), Some(0));
    }

    #[test]
    fn one_good_read_resets_the_refusal_streak() {
        // A single pack answering intermittently must not accumulate its way
        // into a backoff; only the whole block being rejected should.
        let mut poll = AnalogPollState::new();
        let now = Instant::now();

        for _ in 0..ANALOG_REFUSAL_LIMIT - 1 {
            let pack = poll.take_next(now).expect("still polling");
            poll.record_refused(pack, now);
        }
        poll.record_read(0, analogs_for(AlarmZone::Mp1a, 46.25));

        for _ in 0..ANALOG_REFUSAL_LIMIT - 1 {
            let pack = poll.take_next(now).expect("still polling");
            assert!(!poll.record_refused(pack, now), "streak was not reset");
        }
    }

    #[test]
    fn losing_the_link_discards_everything_and_clears_the_backoff() {
        let mut poll = AnalogPollState::new();
        let now = Instant::now();
        poll.record_read(0, analogs_for(AlarmZone::Mp1a, 46.25));
        for _ in 0..ANALOG_REFUSAL_LIMIT {
            let pack = poll.take_next(now).expect("still polling");
            poll.record_refused(pack, now);
        }
        assert_eq!(poll.take_next(now), None, "expected to be backed off");

        poll.record_link_lost();

        // Nothing survives a dropped link: every held value is of unknown age.
        assert!(poll.collected().is_empty());
        // And the reconnected link gets polled immediately rather than serving
        // out a backoff earned by a connection that no longer exists.
        assert!(poll.take_next(now).is_some());
    }

    use super::*;

    #[test]
    fn test_rtac_config_default() {
        let config = RtacConfig::default();
        assert_eq!(config.read_interval, Duration::from_millis(100));
        assert_eq!(config.write_every_n_ticks, 5);
        assert_eq!(config.storage_sample_rate, 10);
    }

    #[test]
    fn test_rtac_config_builder() {
        let addr: SocketAddr = "192.168.1.100:502".parse().unwrap();
        let config = RtacConfig::default().with_address(addr).with_slave_id(2);

        assert_eq!(config.rtac_address, addr);
        assert_eq!(config.slave_id, 2);
    }

    #[test]
    fn test_rtac_config_from_env_values() {
        // Parsing the address and slave id from supplied values, without
        // touching the process-wide environment.
        let config = RtacConfig::from_env_values(Some("10.0.0.5:1502"), Some("7"));
        assert_eq!(config.rtac_address, "10.0.0.5:1502".parse().unwrap());
        assert_eq!(config.slave_id, 7);

        // An unparseable address falls back to the default rather than failing.
        let config = RtacConfig::from_env_values(Some("definitely not an address"), None);
        assert_eq!(config.rtac_address, RtacConfig::default().rtac_address);

        // An invalid slave id falls back to the default too.
        let config = RtacConfig::from_env_values(None, Some("not a number"));
        assert_eq!(config.slave_id, RtacConfig::default().slave_id);

        // Missing values leave the defaults untouched.
        let config = RtacConfig::from_env_values(None, None);
        assert_eq!(config.rtac_address, RtacConfig::default().rtac_address);
        assert_eq!(config.slave_id, RtacConfig::default().slave_id);
    }

    #[test]
    fn test_worker_stats_default() {
        let stats = WorkerStats::default();
        assert_eq!(stats.total_reads, 0);
        assert_eq!(stats.successful_reads, 0);
        assert_eq!(stats.total_writes, 0);
    }
}
