//! Modbus Worker Task
//!
//! This module implements the main worker task that handles:
//! - 10Hz read operations for data collection and alarm monitoring
//! - 2Hz write operations for schedule command execution
//! - Time-slotted single worker pattern (reads every tick, writes every 5th
//!   tick)

use std::{net::SocketAddr, sync::Arc, time::Duration};

use chrono::Utc;
use tokio::{
    sync::{RwLock, mpsc, watch},
    time::{Interval, MissedTickBehavior, interval},
};
use tracing::{debug, error, info, trace, warn};

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
    modbus_client::{ModbusClient, ModbusClientConfig},
    protocol::ParsedStatus,
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

                    // Perform write operation (every Nth tick = 2Hz)
                    if self.tick_count.is_multiple_of(self.config.write_every_n_ticks as u64) {
                        self.perform_write().await;
                    }

                    // Log stats periodically (every 10 seconds at 10Hz = every 100 ticks)
                    if self.tick_count.is_multiple_of(100) {
                        self.log_stats();
                    }

                    // If read failed due to connection issue, try to reconnect
                    if !read_success && !self.client.is_connected() {
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
        let reading = RtacReading::from(&*state);

        if let Err(e) = self.channels.storage_tx.send(reading).await {
            warn!(error = %e, "Failed to send reading to storage");
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

/// Create the channels needed for the worker
///
/// Returns the worker channels and a shutdown sender. Send `true` to the
/// shutdown sender to request graceful shutdown.
pub fn create_worker_channels(
    command_rx: watch::Receiver<Option<PendingCommand>>,
    storage_tx: mpsc::Sender<RtacReading>,
    alarm_tx: mpsc::UnboundedSender<Alarm>,
) -> (WorkerChannels, watch::Sender<bool>) {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let channels = WorkerChannels {
        state: Arc::new(RwLock::new(RtacState::default())),
        command_rx,
        storage_tx,
        alarm_tx,
        shutdown_rx,
    };
    (channels, shutdown_tx)
}

#[cfg(test)]
mod tests {
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
    fn test_worker_stats_default() {
        let stats = WorkerStats::default();
        assert_eq!(stats.total_reads, 0);
        assert_eq!(stats.successful_reads, 0);
        assert_eq!(stats.total_writes, 0);
    }
}
