//! Unified Control Logic Task
//!
//! This module implements the control logic that:
//! - Reads the effective schedule for the current date/time
//! - Determines which command should be active based on
//!   execution_offset_seconds
//! - Monitors real-time state for conditions requiring reactive adjustments
//! - Sends commands via watch channel to the Modbus worker

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use tokio::sync::{RwLock, watch};
use tracing::{debug, error, info, warn};

use super::{
    protocol::{CommandType, OperatingMode},
    state::{PendingCommand, RtacState},
};

/// Configuration for the control logic task
#[derive(Debug, Clone)]
pub struct ControlConfig {
    /// How often to evaluate the schedule and state (default: 1 second)
    pub evaluation_interval: Duration,
    /// Default ramp duration in seconds
    pub default_ramp_duration_seconds: i32,
    /// SOC threshold for low battery warning (percentage)
    pub low_soc_threshold: f32,
    /// SOC threshold for high battery warning (percentage)
    pub high_soc_threshold: f32,
    /// Whether to enable reactive control based on SOC thresholds
    pub enable_reactive_control: bool,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            evaluation_interval: Duration::from_secs(1),
            default_ramp_duration_seconds: 30,
            low_soc_threshold: 10.0,
            high_soc_threshold: 95.0,
            enable_reactive_control: true,
        }
    }
}

/// A scheduled command from the database
#[derive(Debug, Clone)]
pub struct ScheduledCommand {
    /// Unique identifier for tracking
    pub id: i64,
    /// Command type to execute
    pub command_type: CommandType,
    /// When this command becomes active (UTC)
    pub starts_at: DateTime<Utc>,
    /// When this command ends (UTC), if applicable
    pub ends_at: Option<DateTime<Utc>>,
    /// Duration in seconds (alternative to ends_at)
    pub duration_seconds: Option<i32>,
    /// Target SOC percentage
    pub target_soc_percent: Option<i32>,
    /// Ramp duration override
    pub ramp_duration_seconds: Option<i32>,
}

impl ScheduledCommand {
    /// Check if this command is currently active
    pub fn is_active_at(&self, time: DateTime<Utc>) -> bool {
        if time < self.starts_at {
            return false;
        }

        if let Some(ends_at) = self.ends_at {
            time < ends_at
        } else if let Some(duration) = self.duration_seconds {
            let end_time = self.starts_at + chrono::Duration::seconds(duration as i64);
            time < end_time
        } else {
            // No end time specified, command is active indefinitely
            true
        }
    }

    /// Convert to a PendingCommand
    pub fn to_pending_command(&self, default_ramp_duration: i32) -> PendingCommand {
        PendingCommand {
            command_type: self.command_type,
            duration_seconds: self.duration_seconds,
            target_soc_percent: self.target_soc_percent,
            ramp_duration_seconds: self.ramp_duration_seconds.unwrap_or(default_ramp_duration),
            created_at: Utc::now(),
            source_id: Some(self.id),
        }
    }
}

/// Trait for schedule providers
///
/// This trait allows different implementations for fetching schedules,
/// making the control logic testable with mock schedules.
pub trait ScheduleProvider: Send + Sync {
    /// Get the currently active scheduled command, if any
    fn get_active_command(&self, at_time: DateTime<Utc>) -> Option<ScheduledCommand>;

    /// Reload schedules from the source (e.g., database)
    fn reload(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// In-memory schedule provider for testing
pub struct InMemoryScheduleProvider {
    commands: Vec<ScheduledCommand>,
}

impl InMemoryScheduleProvider {
    pub fn new(commands: Vec<ScheduledCommand>) -> Self {
        Self { commands }
    }

    pub fn add_command(&mut self, command: ScheduledCommand) {
        self.commands.push(command);
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }
}

impl ScheduleProvider for InMemoryScheduleProvider {
    fn get_active_command(&self, at_time: DateTime<Utc>) -> Option<ScheduledCommand> {
        // Find the most recent active command
        self.commands
            .iter()
            .filter(|cmd| cmd.is_active_at(at_time))
            .max_by_key(|cmd| cmd.starts_at)
            .cloned()
    }

    fn reload(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // No-op for in-memory provider
        Ok(())
    }
}

/// The unified control logic task
pub struct ControlLogicTask<S: ScheduleProvider> {
    config: ControlConfig,
    schedule_provider: S,
    state: Arc<RwLock<RtacState>>,
    command_tx: watch::Sender<Option<PendingCommand>>,
    last_command_id: Option<i64>,
    last_reactive_command: Option<CommandType>,
}

impl<S: ScheduleProvider> ControlLogicTask<S> {
    /// Create a new control logic task
    pub fn new(
        config: ControlConfig,
        schedule_provider: S,
        state: Arc<RwLock<RtacState>>,
        command_tx: watch::Sender<Option<PendingCommand>>,
    ) -> Self {
        Self {
            config,
            schedule_provider,
            state,
            command_tx,
            last_command_id: None,
            last_reactive_command: None,
        }
    }

    /// Run the control logic loop
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting control logic task");

        let mut interval = tokio::time::interval(self.config.evaluation_interval);

        loop {
            interval.tick().await;

            if let Err(e) = self.evaluate().await {
                error!(error = %e, "Control logic evaluation failed");
            }
        }
    }

    /// Evaluate current state and schedules, send commands as needed
    async fn evaluate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let current_state = self.state.read().await.clone();

        // Check if system is available for commands
        if !current_state.is_available_for_commands() {
            if current_state.alarms.is_estop_active() {
                warn!("System in emergency stop, skipping command evaluation");
            } else if current_state.alarms.has_critical_alarm() {
                let active: Vec<_> = current_state.alarms.active_alarms().iter().map(|a| a.qualified_name()).collect();
                warn!(alarms = ?active, "Critical alarm active, skipping command evaluation");
            }

            // Ensure no commands are issued while the system is unavailable
            if self.last_command_id.is_some() || self.last_reactive_command.is_some() {
                debug!("System not available for commands, clearing pending command");
                // Clear the current command in the watch channel
                self.command_tx.send(None)?;
                self.last_command_id = None;
                self.last_reactive_command = None;
            }

            return Ok(());
        }

        // First, check for reactive control conditions
        if self.config.enable_reactive_control {
            if let Some(reactive_cmd) = self.check_reactive_conditions(&current_state) {
                self.send_reactive_command(reactive_cmd).await;
                return Ok(());
            }
        }

        // Clear reactive command if conditions no longer apply
        if self.last_reactive_command.is_some() {
            self.last_reactive_command = None;
            debug!("Reactive conditions cleared, resuming schedule");
        }

        // Check scheduled commands
        if let Some(scheduled) = self.schedule_provider.get_active_command(now) {
            // Only send if this is a different command than last time
            if self.last_command_id != Some(scheduled.id) {
                let pending =
                    scheduled.to_pending_command(self.config.default_ramp_duration_seconds);
                info!(
                    command_id = scheduled.id,
                    command_type = %pending.command_type,
                    target_soc = ?pending.target_soc_percent,
                    duration = ?pending.duration_seconds,
                    "Activating scheduled command"
                );

                self.command_tx.send(Some(pending))?;
                self.last_command_id = Some(scheduled.id);
            }
        } else {
            // No active scheduled command
            if self.last_command_id.is_some() {
                debug!("No active scheduled command, clearing");
                // Optionally send standby command when schedule ends
                let standby = PendingCommand::standby(self.config.default_ramp_duration_seconds);
                self.command_tx.send(Some(standby))?;
                self.last_command_id = None;
            }
        }

        Ok(())
    }

    /// Check for reactive control conditions based on current state
    fn check_reactive_conditions(&self, state: &RtacState) -> Option<CommandType> {
        // Check for low SOC - stop discharging
        if state.soc_percent <= self.config.low_soc_threshold {
            if state.mode == OperatingMode::Discharging {
                warn!(
                    soc = state.soc_percent,
                    threshold = self.config.low_soc_threshold,
                    "Low SOC detected, stopping discharge"
                );
                return Some(CommandType::Standby);
            }
        }

        // Check for high SOC - stop charging
        if state.soc_percent >= self.config.high_soc_threshold {
            if state.mode == OperatingMode::Charging {
                warn!(
                    soc = state.soc_percent,
                    threshold = self.config.high_soc_threshold,
                    "High SOC detected, stopping charge"
                );
                return Some(CommandType::Standby);
            }
        }

        None
    }

    /// Send a reactive command (overrides scheduled commands)
    async fn send_reactive_command(&mut self, command_type: CommandType) {
        // Only send if different from last reactive command
        if self.last_reactive_command == Some(command_type) {
            return;
        }

        let pending = PendingCommand {
            command_type,
            duration_seconds: None,
            target_soc_percent: None,
            ramp_duration_seconds: self.config.default_ramp_duration_seconds,
            created_at: Utc::now(),
            source_id: None, // Reactive commands don't have a schedule ID
        };

        info!(command_type = %command_type, "Sending reactive command");

        if let Err(e) = self.command_tx.send(Some(pending)) {
            error!(error = %e, "Failed to send reactive command");
        }

        self.last_reactive_command = Some(command_type);
        // Reset last_command_id so that when reactive conditions clear,
        // the scheduled command will be re-evaluated and sent
        self.last_command_id = None;
    }

    /// Force reload of schedules from the provider
    pub fn reload_schedules(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.schedule_provider.reload()
    }

    /// Manually trigger a command (bypasses schedule)
    pub fn trigger_manual_command(
        &mut self,
        command: PendingCommand,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(command_type = %command.command_type, "Manual command triggered");
        self.command_tx.send(Some(command))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn make_test_time(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 6, 15, hour, minute, 0).unwrap()
    }

    #[test]
    fn test_scheduled_command_is_active() {
        let cmd = ScheduledCommand {
            id: 1,
            command_type: CommandType::Charge,
            starts_at: make_test_time(10, 0),
            ends_at: Some(make_test_time(11, 0)),
            duration_seconds: None,
            target_soc_percent: Some(80),
            ramp_duration_seconds: Some(30),
        };

        // Before start
        assert!(!cmd.is_active_at(make_test_time(9, 59)));

        // During
        assert!(cmd.is_active_at(make_test_time(10, 0)));
        assert!(cmd.is_active_at(make_test_time(10, 30)));

        // After end
        assert!(!cmd.is_active_at(make_test_time(11, 0)));
        assert!(!cmd.is_active_at(make_test_time(11, 30)));
    }

    #[test]
    fn test_scheduled_command_with_duration() {
        let cmd = ScheduledCommand {
            id: 1,
            command_type: CommandType::Discharge,
            starts_at: make_test_time(14, 0),
            ends_at: None,
            duration_seconds: Some(1800), // 30 minutes
            target_soc_percent: None,
            ramp_duration_seconds: None,
        };

        assert!(cmd.is_active_at(make_test_time(14, 15)));
        assert!(cmd.is_active_at(make_test_time(14, 29)));
        assert!(!cmd.is_active_at(make_test_time(14, 30)));
    }

    #[test]
    fn test_in_memory_schedule_provider() {
        let mut provider = InMemoryScheduleProvider::new(vec![]);

        provider.add_command(ScheduledCommand {
            id: 1,
            command_type: CommandType::Charge,
            starts_at: make_test_time(8, 0),
            ends_at: Some(make_test_time(12, 0)),
            duration_seconds: None,
            target_soc_percent: Some(90),
            ramp_duration_seconds: None,
        });

        provider.add_command(ScheduledCommand {
            id: 2,
            command_type: CommandType::Discharge,
            starts_at: make_test_time(14, 0),
            ends_at: Some(make_test_time(18, 0)),
            duration_seconds: None,
            target_soc_percent: None,
            ramp_duration_seconds: None,
        });

        // Test morning charge period
        let cmd = provider.get_active_command(make_test_time(10, 0));
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command_type, CommandType::Charge);

        // Test afternoon discharge period
        let cmd = provider.get_active_command(make_test_time(15, 0));
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command_type, CommandType::Discharge);

        // Test no active command period
        let cmd = provider.get_active_command(make_test_time(13, 0));
        assert!(cmd.is_none());
    }

    #[test]
    fn test_scheduled_command_to_pending() {
        let cmd = ScheduledCommand {
            id: 42,
            command_type: CommandType::TrickleCharge,
            starts_at: Utc::now(),
            ends_at: None,
            duration_seconds: Some(3600),
            target_soc_percent: Some(95),
            ramp_duration_seconds: Some(60),
        };

        let pending = cmd.to_pending_command(30);

        assert_eq!(pending.command_type, CommandType::TrickleCharge);
        assert_eq!(pending.duration_seconds, Some(3600));
        assert_eq!(pending.target_soc_percent, Some(95));
        assert_eq!(pending.ramp_duration_seconds, 60);
        assert_eq!(pending.source_id, Some(42));
    }

    use super::super::{
        alarm_definitions::{ESTOP_ALARM_NUM, T1_TEMP_TRIP_ALARM_NUM},
        state::ConnectionStatus,
    };

    fn create_test_task(
        config: ControlConfig,
        provider: InMemoryScheduleProvider,
    ) -> (
        ControlLogicTask<InMemoryScheduleProvider>,
        watch::Receiver<Option<PendingCommand>>,
        Arc<RwLock<RtacState>>,
    ) {
        let state = Arc::new(RwLock::new(RtacState {
            connection_status: ConnectionStatus::Connected,
            soc_percent: 50.0,
            mode: OperatingMode::Standby,
            ..Default::default()
        }));
        let (command_tx, command_rx) = watch::channel(None);

        let task = ControlLogicTask::new(config, provider, state.clone(), command_tx);

        (task, command_rx, state)
    }

    #[tokio::test]
    async fn test_reactive_override_resumes_scheduled_command() {
        // Setup: schedule a discharge command, then trigger low SOC reactive override
        let config = ControlConfig {
            low_soc_threshold: 10.0,
            high_soc_threshold: 95.0,
            enable_reactive_control: true,
            ..Default::default()
        };

        let mut provider = InMemoryScheduleProvider::new(vec![]);
        provider.add_command(ScheduledCommand {
            id: 1,
            command_type: CommandType::Discharge,
            starts_at: Utc::now() - chrono::Duration::hours(1),
            ends_at: Some(Utc::now() + chrono::Duration::hours(1)),
            duration_seconds: None,
            target_soc_percent: None,
            ramp_duration_seconds: None,
        });

        let (mut task, command_rx, state) = create_test_task(config, provider);

        // First evaluation: should activate the discharge schedule
        {
            let mut s = state.write().await;
            s.mode = OperatingMode::Discharging;
            s.soc_percent = 50.0;
        }
        task.evaluate().await.unwrap();
        assert_eq!(task.last_command_id, Some(1));

        let cmd = command_rx.borrow().clone();
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command_type, CommandType::Discharge);

        // Second evaluation: low SOC triggers reactive standby
        {
            let mut s = state.write().await;
            s.soc_percent = 5.0; // Below low_soc_threshold
        }
        task.evaluate().await.unwrap();
        assert!(task.last_reactive_command.is_some());
        assert_eq!(task.last_reactive_command, Some(CommandType::Standby));
        // last_command_id should be reset so schedule can resume
        assert_eq!(task.last_command_id, None);

        let cmd = command_rx.borrow().clone();
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command_type, CommandType::Standby);

        // Third evaluation: SOC recovers, reactive condition clears
        // Schedule should resume and re-send the discharge command
        {
            let mut s = state.write().await;
            s.soc_percent = 50.0; // Above threshold
            s.mode = OperatingMode::Standby;
        }
        task.evaluate().await.unwrap();
        assert!(task.last_reactive_command.is_none());
        // Schedule should have been re-activated
        assert_eq!(task.last_command_id, Some(1));

        let cmd = command_rx.borrow().clone();
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command_type, CommandType::Discharge);
    }

    #[tokio::test]
    async fn test_emergency_stop_clears_pending_command() {
        let config = ControlConfig::default();

        let mut provider = InMemoryScheduleProvider::new(vec![]);
        provider.add_command(ScheduledCommand {
            id: 1,
            command_type: CommandType::Charge,
            starts_at: Utc::now() - chrono::Duration::hours(1),
            ends_at: Some(Utc::now() + chrono::Duration::hours(1)),
            duration_seconds: None,
            target_soc_percent: Some(80),
            ramp_duration_seconds: None,
        });

        let (mut task, command_rx, state) = create_test_task(config, provider);

        // First evaluation: should activate the charge schedule
        task.evaluate().await.unwrap();
        assert_eq!(task.last_command_id, Some(1));

        let cmd = command_rx.borrow().clone();
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command_type, CommandType::Charge);

        // Second evaluation: emergency stop is triggered
        {
            let mut s = state.write().await;
            s.alarms.set_alarm_num(ESTOP_ALARM_NUM, true);
        }
        task.evaluate().await.unwrap();

        // Command should be cleared
        assert_eq!(task.last_command_id, None);
        assert_eq!(task.last_reactive_command, None);

        let cmd = command_rx.borrow().clone();
        assert!(cmd.is_none());
    }

    #[tokio::test]
    async fn test_critical_alarm_clears_pending_command() {
        let config = ControlConfig::default();

        let mut provider = InMemoryScheduleProvider::new(vec![]);
        provider.add_command(ScheduledCommand {
            id: 1,
            command_type: CommandType::Discharge,
            starts_at: Utc::now() - chrono::Duration::hours(1),
            ends_at: Some(Utc::now() + chrono::Duration::hours(1)),
            duration_seconds: None,
            target_soc_percent: None,
            ramp_duration_seconds: None,
        });

        let (mut task, command_rx, state) = create_test_task(config, provider);

        // First evaluation: should activate the discharge schedule
        task.evaluate().await.unwrap();
        assert_eq!(task.last_command_id, Some(1));

        // Second evaluation: critical alarm (transformer 1 trip, level 2) is triggered
        {
            let mut s = state.write().await;
            s.alarms.set_alarm_num(T1_TEMP_TRIP_ALARM_NUM, true);
        }
        task.evaluate().await.unwrap();

        // Command should be cleared
        assert_eq!(task.last_command_id, None);
        assert_eq!(task.last_reactive_command, None);

        let cmd = command_rx.borrow().clone();
        assert!(cmd.is_none());
    }

    #[tokio::test]
    async fn test_high_soc_stops_charging() {
        let config = ControlConfig {
            low_soc_threshold: 10.0,
            high_soc_threshold: 95.0,
            enable_reactive_control: true,
            ..Default::default()
        };

        let mut provider = InMemoryScheduleProvider::new(vec![]);
        provider.add_command(ScheduledCommand {
            id: 1,
            command_type: CommandType::Charge,
            starts_at: Utc::now() - chrono::Duration::hours(1),
            ends_at: Some(Utc::now() + chrono::Duration::hours(1)),
            duration_seconds: None,
            target_soc_percent: Some(100),
            ramp_duration_seconds: None,
        });

        let (mut task, command_rx, state) = create_test_task(config, provider);

        // First evaluation: should activate the charge schedule
        {
            let mut s = state.write().await;
            s.mode = OperatingMode::Charging;
            s.soc_percent = 50.0;
        }
        task.evaluate().await.unwrap();
        assert_eq!(task.last_command_id, Some(1));

        // Second evaluation: high SOC triggers reactive standby
        {
            let mut s = state.write().await;
            s.soc_percent = 98.0; // Above high_soc_threshold
        }
        task.evaluate().await.unwrap();

        // Reactive standby should be sent
        assert_eq!(task.last_reactive_command, Some(CommandType::Standby));

        let cmd = command_rx.borrow().clone();
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().command_type, CommandType::Standby);
    }
}
