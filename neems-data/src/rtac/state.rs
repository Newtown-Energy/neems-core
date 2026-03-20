//! State structures for RTAC communication
//!
//! This module defines the shared state types used for communication between
//! the Modbus worker, control logic, storage, and alarm handler tasks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::protocol::{CommandType, OperatingMode};

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
/// Each flag represents a specific alarm condition that can be active.
/// Multiple alarms can be active simultaneously.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmFlags {
    /// Emergency stop activated
    pub emergency_stop: bool,
    /// Over-temperature condition
    pub over_temperature: bool,
    /// Under-temperature condition
    pub under_temperature: bool,
    /// Over-voltage condition
    pub over_voltage: bool,
    /// Under-voltage condition
    pub under_voltage: bool,
    /// Over-current condition
    pub over_current: bool,
    /// Communication fault with battery modules
    pub communication_fault: bool,
    /// Battery management system fault
    pub bms_fault: bool,
    /// Inverter fault
    pub inverter_fault: bool,
    /// Grid fault detected
    pub grid_fault: bool,
    /// System isolation fault
    pub isolation_fault: bool,
    /// Fan or cooling system fault
    pub cooling_fault: bool,
}

impl AlarmFlags {
    /// Returns true if any alarm is active
    pub fn has_any_alarm(&self) -> bool {
        self.emergency_stop
            || self.over_temperature
            || self.under_temperature
            || self.over_voltage
            || self.under_voltage
            || self.over_current
            || self.communication_fault
            || self.bms_fault
            || self.inverter_fault
            || self.grid_fault
            || self.isolation_fault
            || self.cooling_fault
    }

    /// Returns true if any critical alarm is active (requires immediate
    /// attention)
    pub fn has_critical_alarm(&self) -> bool {
        self.emergency_stop
            || self.over_temperature
            || self.over_voltage
            || self.over_current
            || self.bms_fault
            || self.inverter_fault
    }

    /// Returns a list of active alarm names
    pub fn active_alarms(&self) -> Vec<&'static str> {
        let mut alarms = Vec::new();
        if self.emergency_stop {
            alarms.push("emergency_stop");
        }
        if self.over_temperature {
            alarms.push("over_temperature");
        }
        if self.under_temperature {
            alarms.push("under_temperature");
        }
        if self.over_voltage {
            alarms.push("over_voltage");
        }
        if self.under_voltage {
            alarms.push("under_voltage");
        }
        if self.over_current {
            alarms.push("over_current");
        }
        if self.communication_fault {
            alarms.push("communication_fault");
        }
        if self.bms_fault {
            alarms.push("bms_fault");
        }
        if self.inverter_fault {
            alarms.push("inverter_fault");
        }
        if self.grid_fault {
            alarms.push("grid_fault");
        }
        if self.isolation_fault {
            alarms.push("isolation_fault");
        }
        if self.cooling_fault {
            alarms.push("cooling_fault");
        }
        alarms
    }

    /// Parse alarm flags from a 16-bit register value
    pub fn from_register(value: u16) -> Self {
        Self {
            emergency_stop: (value & 0x0001) != 0,
            over_temperature: (value & 0x0002) != 0,
            under_temperature: (value & 0x0004) != 0,
            over_voltage: (value & 0x0008) != 0,
            under_voltage: (value & 0x0010) != 0,
            over_current: (value & 0x0020) != 0,
            communication_fault: (value & 0x0040) != 0,
            bms_fault: (value & 0x0080) != 0,
            inverter_fault: (value & 0x0100) != 0,
            grid_fault: (value & 0x0200) != 0,
            isolation_fault: (value & 0x0400) != 0,
            cooling_fault: (value & 0x0800) != 0,
        }
    }

    /// Convert alarm flags to a 16-bit register value
    pub fn to_register(&self) -> u16 {
        let mut value = 0u16;
        if self.emergency_stop {
            value |= 0x0001;
        }
        if self.over_temperature {
            value |= 0x0002;
        }
        if self.under_temperature {
            value |= 0x0004;
        }
        if self.over_voltage {
            value |= 0x0008;
        }
        if self.under_voltage {
            value |= 0x0010;
        }
        if self.over_current {
            value |= 0x0020;
        }
        if self.communication_fault {
            value |= 0x0040;
        }
        if self.bms_fault {
            value |= 0x0080;
        }
        if self.inverter_fault {
            value |= 0x0100;
        }
        if self.grid_fault {
            value |= 0x0200;
        }
        if self.isolation_fault {
            value |= 0x0400;
        }
        if self.cooling_fault {
            value |= 0x0800;
        }
        value
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
        self.is_healthy() && !self.alarms.emergency_stop
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
    /// Active alarm flags as bitmask
    pub alarm_flags: u16,
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
            alarm_flags: state.alarms.to_register(),
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

    #[test]
    fn test_alarm_flags_from_register() {
        let flags = AlarmFlags::from_register(0x0000);
        assert!(!flags.has_any_alarm());

        let flags = AlarmFlags::from_register(0x0001);
        assert!(flags.emergency_stop);
        assert!(flags.has_critical_alarm());

        let flags = AlarmFlags::from_register(0x0fff);
        assert!(flags.emergency_stop);
        assert!(flags.over_temperature);
        assert!(flags.under_temperature);
        assert!(flags.over_voltage);
        assert!(flags.under_voltage);
        assert!(flags.over_current);
        assert!(flags.communication_fault);
        assert!(flags.bms_fault);
        assert!(flags.inverter_fault);
        assert!(flags.grid_fault);
        assert!(flags.isolation_fault);
        assert!(flags.cooling_fault);
    }

    #[test]
    fn test_alarm_flags_roundtrip() {
        let original = AlarmFlags {
            emergency_stop: true,
            over_temperature: false,
            under_temperature: true,
            over_voltage: false,
            under_voltage: true,
            over_current: false,
            communication_fault: true,
            bms_fault: false,
            inverter_fault: true,
            grid_fault: false,
            isolation_fault: true,
            cooling_fault: false,
        };

        let register = original.to_register();
        let restored = AlarmFlags::from_register(register);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_rtac_state_health_checks() {
        let mut state = RtacState {
            connection_status: ConnectionStatus::Connected,
            ..Default::default()
        };
        assert!(state.is_healthy());
        assert!(state.is_available_for_commands());

        state.alarms.emergency_stop = true;
        assert!(!state.is_healthy());
        assert!(!state.is_available_for_commands());

        state.alarms.emergency_stop = false;
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
