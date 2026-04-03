//! State structures for RTAC communication
//!
//! This module defines the shared state types used for communication between
//! the Modbus worker, control logic, storage, and alarm handler tasks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    alarm_definitions::{
        ALARM_DEFINITIONS, ALARM_REGISTER_COUNT, AlarmDefinition, AlarmZone, ESTOP_ALARM_NUM,
    },
    protocol::{CommandType, OperatingMode},
};

/// Connection status for the Modbus TCP connection
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Not yet connected
    #[default]
    Disconnected,
    /// Actively connected and communicating
    Connected,
    /// Connection lost, attempting to reconnect
    Reconnecting,
    /// Connection failed after max retries
    Failed,
}

/// Alarm flags from the RTAC
///
/// Stores the raw 22-register alarm bitfield read from the RTAC.
/// Each register holds up to 16 alarm bits. Use alarm definitions to
/// interpret individual bits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmFlags {
    /// Raw alarm register values
    pub registers: [u16; ALARM_REGISTER_COUNT],
}

impl AlarmFlags {
    /// Parse alarm flags from a slice of register values
    pub fn from_registers(registers: &[u16; ALARM_REGISTER_COUNT]) -> Self {
        Self { registers: *registers }
    }

    /// Convert alarm flags to the register array
    pub fn to_registers(&self) -> [u16; ALARM_REGISTER_COUNT] {
        self.registers
    }

    /// Check whether a specific alarm definition is active
    pub fn is_alarm_active(&self, def: &AlarmDefinition) -> bool {
        (self.registers[def.register_index] >> def.bit) & 1 != 0
    }

    /// Check whether a specific alarm number is active
    pub fn is_alarm_num_active(&self, alarm_num: u16) -> bool {
        ALARM_DEFINITIONS
            .iter()
            .find(|d| d.alarm_num == alarm_num)
            .is_some_and(|d| self.is_alarm_active(d))
    }

    /// Set an alarm by alarm number (for testing)
    pub fn set_alarm_num(&mut self, alarm_num: u16, active: bool) {
        if let Some(def) = ALARM_DEFINITIONS.iter().find(|d| d.alarm_num == alarm_num) {
            if active {
                self.registers[def.register_index] |= 1 << def.bit;
            } else {
                self.registers[def.register_index] &= !(1 << def.bit);
            }
        }
    }

    /// Returns true if the emergency stop alarm (104) is active
    pub fn is_estop_active(&self) -> bool {
        self.is_alarm_num_active(ESTOP_ALARM_NUM)
    }

    /// Returns true if any alarm is active
    pub fn has_any_alarm(&self) -> bool {
        self.registers.iter().any(|&r| r != 0)
    }

    /// Returns true if any alarm at level 1 or 2 is active (emergency or high)
    pub fn has_critical_alarm(&self) -> bool {
        ALARM_DEFINITIONS
            .iter()
            .filter(|d| d.level <= 2)
            .any(|d| self.is_alarm_active(d))
    }

    /// Returns true if any level-1 alarm (emergency / fire) is active
    pub fn has_emergency_alarm(&self) -> bool {
        ALARM_DEFINITIONS
            .iter()
            .filter(|d| d.level == 1)
            .any(|d| self.is_alarm_active(d))
    }

    /// Returns a list of active alarm definitions
    pub fn active_alarms(&self) -> Vec<&'static AlarmDefinition> {
        ALARM_DEFINITIONS.iter().filter(|d| self.is_alarm_active(d)).collect()
    }

    /// Returns active alarm definitions filtered by zone
    pub fn active_alarms_in_zone(&self, zone: AlarmZone) -> Vec<&'static AlarmDefinition> {
        ALARM_DEFINITIONS
            .iter()
            .filter(|d| d.zone == zone && self.is_alarm_active(d))
            .collect()
    }

    /// OR two alarm flag sets together (used for decimation)
    pub fn bitwise_or(&self, other: &AlarmFlags) -> AlarmFlags {
        let mut result = [0u16; ALARM_REGISTER_COUNT];
        for (i, reg) in result.iter_mut().enumerate() {
            *reg = self.registers[i] | other.registers[i];
        }
        AlarmFlags { registers: result }
    }
}

/// Current state read from the RTAC
///
/// This structure is updated at 10Hz by the Modbus worker and shared
/// via `Arc<RwLock<RtacState>>` with other tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtacState {
    /// Timestamp when this state was read
    pub timestamp: DateTime<Utc>,
    /// Current state of charge (0-100%)
    pub soc_percent: f32,
    /// Current power output in kW (positive = discharging, negative = charging)
    pub power_kw: f32,
    /// Current operating mode
    pub mode: OperatingMode,
    /// Active alarm flags
    pub alarms: AlarmFlags,
    /// Connection status
    pub connection_status: ConnectionStatus,
    /// Monotonically increasing sequence number for each read
    pub sequence: u64,
    /// Voltage in volts
    pub voltage_v: f32,
    /// Current in amps
    pub current_a: f32,
    /// Temperature in celsius
    pub temperature_c: f32,
    /// Grid frequency in Hz
    pub grid_frequency_hz: f32,
}

impl Default for RtacState {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            soc_percent: 0.0,
            power_kw: 0.0,
            mode: OperatingMode::Standby,
            alarms: AlarmFlags::default(),
            connection_status: ConnectionStatus::Disconnected,
            sequence: 0,
            voltage_v: 0.0,
            current_a: 0.0,
            temperature_c: 0.0,
            grid_frequency_hz: 0.0,
        }
    }
}

impl RtacState {
    /// Create a new RtacState with the current timestamp
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the state indicates the system is healthy (connected, no
    /// critical alarms)
    pub fn is_healthy(&self) -> bool {
        self.connection_status == ConnectionStatus::Connected && !self.alarms.has_critical_alarm()
    }

    /// Check if the system is available for commands
    pub fn is_available_for_commands(&self) -> bool {
        self.is_healthy() && !self.alarms.is_estop_active()
    }
}

/// A reading snapshot for storage
///
/// This is a simplified version of RtacState optimized for database storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtacReading {
    /// Timestamp when this reading was taken
    pub timestamp: DateTime<Utc>,
    /// Current state of charge (0-100%)
    pub soc_percent: f32,
    /// Current power output in kW
    pub power_kw: f32,
    /// Current operating mode as string
    pub mode: String,
    /// Voltage in volts
    pub voltage_v: f32,
    /// Current in amps
    pub current_a: f32,
    /// Temperature in celsius
    pub temperature_c: f32,
    /// Grid frequency in Hz
    pub grid_frequency_hz: f32,
    /// Active alarm flags as register array
    pub alarm_registers: [u16; ALARM_REGISTER_COUNT],
    /// Sequence number
    pub sequence: u64,
}

impl From<&RtacState> for RtacReading {
    fn from(state: &RtacState) -> Self {
        Self {
            timestamp: state.timestamp,
            soc_percent: state.soc_percent,
            power_kw: state.power_kw,
            mode: state.mode.to_string(),
            voltage_v: state.voltage_v,
            current_a: state.current_a,
            temperature_c: state.temperature_c,
            grid_frequency_hz: state.grid_frequency_hz,
            alarm_registers: state.alarms.to_registers(),
            sequence: state.sequence,
        }
    }
}

/// A command to be sent to the RTAC
///
/// Commands are sent via a `tokio::sync::watch` channel from the control logic
/// to the Modbus worker. The watch channel provides latest-value semantics,
/// so if multiple commands are sent before the worker processes them, only
/// the most recent command will be executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingCommand {
    /// The type of command to execute
    pub command_type: CommandType,
    /// Duration of the command in seconds (None for indefinite)
    pub duration_seconds: Option<i32>,
    /// Target state of charge percent (for charge commands)
    pub target_soc_percent: Option<i32>,
    /// Ramp duration in seconds for power transitions
    pub ramp_duration_seconds: i32,
    /// Timestamp when this command was created
    pub created_at: DateTime<Utc>,
    /// Optional identifier for tracking (e.g., schedule_command_id)
    pub source_id: Option<i64>,
}

impl PendingCommand {
    /// Create a new charge command
    pub fn charge(
        target_soc_percent: Option<i32>,
        duration_seconds: Option<i32>,
        ramp_duration_seconds: i32,
    ) -> Self {
        Self {
            command_type: CommandType::Charge,
            duration_seconds,
            target_soc_percent,
            ramp_duration_seconds,
            created_at: Utc::now(),
            source_id: None,
        }
    }

    /// Create a new discharge command
    pub fn discharge(duration_seconds: Option<i32>, ramp_duration_seconds: i32) -> Self {
        Self {
            command_type: CommandType::Discharge,
            duration_seconds,
            target_soc_percent: None,
            ramp_duration_seconds,
            created_at: Utc::now(),
            source_id: None,
        }
    }

    /// Create a new trickle charge command
    pub fn trickle_charge(
        target_soc_percent: Option<i32>,
        duration_seconds: Option<i32>,
        ramp_duration_seconds: i32,
    ) -> Self {
        Self {
            command_type: CommandType::TrickleCharge,
            duration_seconds,
            target_soc_percent,
            ramp_duration_seconds,
            created_at: Utc::now(),
            source_id: None,
        }
    }

    /// Create a standby command
    pub fn standby(ramp_duration_seconds: i32) -> Self {
        Self {
            command_type: CommandType::Standby,
            duration_seconds: None,
            target_soc_percent: None,
            ramp_duration_seconds,
            created_at: Utc::now(),
            source_id: None,
        }
    }

    /// Set the source ID for tracking
    pub fn with_source_id(mut self, source_id: i64) -> Self {
        self.source_id = Some(source_id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtac::alarm_definitions::{ESTOP_ALARM_NUM, FIRE_ALARM_NUM};

    #[test]
    fn test_alarm_flags_default_empty() {
        let flags = AlarmFlags::default();
        assert!(!flags.has_any_alarm());
        assert!(!flags.has_critical_alarm());
        assert!(!flags.is_estop_active());
    }

    #[test]
    fn test_alarm_flags_estop() {
        let mut flags = AlarmFlags::default();
        flags.set_alarm_num(ESTOP_ALARM_NUM, true);
        assert!(flags.is_estop_active());
        assert!(flags.has_any_alarm());
        assert!(flags.has_critical_alarm()); // level 2

        flags.set_alarm_num(ESTOP_ALARM_NUM, false);
        assert!(!flags.is_estop_active());
    }

    #[test]
    fn test_alarm_flags_fire_alarm() {
        let mut flags = AlarmFlags::default();
        flags.set_alarm_num(FIRE_ALARM_NUM, true);
        assert!(flags.has_emergency_alarm());
        assert!(flags.has_critical_alarm());
    }

    #[test]
    fn test_alarm_flags_roundtrip() {
        let mut flags = AlarmFlags::default();
        flags.set_alarm_num(1, true); // loss_fiber
        flags.set_alarm_num(104, true); // estop
        flags.set_alarm_num(401, true); // fire_alarm
        flags.set_alarm_num(601, true); // mp1a loss_of_comms

        let regs = flags.to_registers();
        let restored = AlarmFlags::from_registers(&regs);
        assert_eq!(flags, restored);

        assert!(restored.is_alarm_num_active(1));
        assert!(restored.is_alarm_num_active(104));
        assert!(restored.is_alarm_num_active(401));
        assert!(restored.is_alarm_num_active(601));
        assert!(!restored.is_alarm_num_active(2));
    }

    #[test]
    fn test_alarm_flags_active_alarms() {
        let mut flags = AlarmFlags::default();
        flags.set_alarm_num(1, true);
        flags.set_alarm_num(2, true);
        flags.set_alarm_num(104, true);

        let active = flags.active_alarms();
        assert_eq!(active.len(), 3);

        let names: Vec<_> = active.iter().map(|a| a.name).collect();
        assert!(names.contains(&"loss_fiber"));
        assert!(names.contains(&"loss_cellular"));
        assert!(names.contains(&"estop"));
    }

    #[test]
    fn test_alarm_flags_bitwise_or() {
        let mut a = AlarmFlags::default();
        a.set_alarm_num(1, true);

        let mut b = AlarmFlags::default();
        b.set_alarm_num(104, true);

        let combined = a.bitwise_or(&b);
        assert!(combined.is_alarm_num_active(1));
        assert!(combined.is_alarm_num_active(104));
    }

    #[test]
    fn test_rtac_state_health_checks() {
        let mut state = RtacState {
            connection_status: ConnectionStatus::Connected,
            ..Default::default()
        };
        assert!(state.is_healthy());
        assert!(state.is_available_for_commands());

        // Estop makes unavailable
        state.alarms.set_alarm_num(ESTOP_ALARM_NUM, true);
        assert!(!state.is_healthy());
        assert!(!state.is_available_for_commands());

        // Clear estop, set a level-4 alarm (should still be healthy)
        state.alarms.set_alarm_num(ESTOP_ALARM_NUM, false);
        state.alarms.set_alarm_num(2, true); // loss_cellular, level 4
        assert!(state.is_healthy());

        state.connection_status = ConnectionStatus::Disconnected;
        assert!(!state.is_healthy());
    }

    #[test]
    fn test_pending_command_constructors() {
        let charge = PendingCommand::charge(Some(80), Some(3600), 30);
        assert_eq!(charge.command_type, CommandType::Charge);
        assert_eq!(charge.target_soc_percent, Some(80));
        assert_eq!(charge.duration_seconds, Some(3600));

        let discharge = PendingCommand::discharge(Some(1800), 30);
        assert_eq!(discharge.command_type, CommandType::Discharge);
        assert_eq!(discharge.duration_seconds, Some(1800));

        let standby = PendingCommand::standby(30);
        assert_eq!(standby.command_type, CommandType::Standby);
    }
}
