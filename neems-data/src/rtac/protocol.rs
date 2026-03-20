//! Modbus protocol definitions for RTAC communication
//!
//! This module defines the register mappings, command types, operating modes,
//! and parsing functions for the RTAC Modbus protocol.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Operating modes for the battery energy storage system
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatingMode {
    /// System is idle, not charging or discharging
    #[default]
    Standby,
    /// System is charging the battery
    Charging,
    /// System is discharging the battery
    Discharging,
    /// System is in trickle charge mode (low-power maintenance charging)
    TrickleCharge,
    /// System is in fault state
    Fault,
    /// System is in emergency stop state
    EmergencyStop,
    /// System is initializing
    Initializing,
    /// Unknown mode (unrecognized register value)
    Unknown(u16),
}

impl OperatingMode {
    /// Parse operating mode from a register value
    pub fn from_register(value: u16) -> Self {
        match value {
            0 => Self::Standby,
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::TrickleCharge,
            4 => Self::Fault,
            5 => Self::EmergencyStop,
            6 => Self::Initializing,
            _ => Self::Unknown(value),
        }
    }

    /// Convert operating mode to a register value
    pub fn to_register(&self) -> u16 {
        match self {
            Self::Standby => 0,
            Self::Charging => 1,
            Self::Discharging => 2,
            Self::TrickleCharge => 3,
            Self::Fault => 4,
            Self::EmergencyStop => 5,
            Self::Initializing => 6,
            Self::Unknown(v) => *v,
        }
    }

    /// Check if the mode indicates the system is actively operating
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Charging | Self::Discharging | Self::TrickleCharge)
    }

    /// Check if the mode indicates a fault condition
    pub fn is_fault(&self) -> bool {
        matches!(self, Self::Fault | Self::EmergencyStop)
    }
}

impl fmt::Display for OperatingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standby => write!(f, "standby"),
            Self::Charging => write!(f, "charging"),
            Self::Discharging => write!(f, "discharging"),
            Self::TrickleCharge => write!(f, "trickle_charge"),
            Self::Fault => write!(f, "fault"),
            Self::EmergencyStop => write!(f, "emergency_stop"),
            Self::Initializing => write!(f, "initializing"),
            Self::Unknown(v) => write!(f, "unknown_{}", v),
        }
    }
}

/// Command types that can be sent to the RTAC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    /// Enter standby mode
    Standby,
    /// Start charging
    Charge,
    /// Start discharging
    Discharge,
    /// Start trickle charging
    TrickleCharge,
    /// Emergency stop
    EmergencyStop,
    /// Clear faults and reset
    ClearFaults,
}

impl CommandType {
    /// Convert command type to the register value for the command register
    pub fn to_register(&self) -> u16 {
        match self {
            Self::Standby => 0,
            Self::Charge => 1,
            Self::Discharge => 2,
            Self::TrickleCharge => 3,
            Self::EmergencyStop => 4,
            Self::ClearFaults => 5,
        }
    }
}

impl fmt::Display for CommandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standby => write!(f, "standby"),
            Self::Charge => write!(f, "charge"),
            Self::Discharge => write!(f, "discharge"),
            Self::TrickleCharge => write!(f, "trickle_charge"),
            Self::EmergencyStop => write!(f, "emergency_stop"),
            Self::ClearFaults => write!(f, "clear_faults"),
        }
    }
}

/// Modbus register map for RTAC communication
///
/// All addresses are in the holding register address space (function code 3 for
/// read, 6/16 for write). Register addresses are 0-based.
#[derive(Debug, Clone, Copy)]
pub struct RegisterMap;

impl RegisterMap {
    // === Read Registers (Holding Registers, Function Code 3) ===

    /// Operating mode register (read-only status)
    pub const STATUS_MODE: u16 = 0;

    /// State of charge percentage (0-10000 = 0.00-100.00%)
    pub const STATUS_SOC: u16 = 1;

    /// Active power in watts (signed, negative = charging)
    /// Stored as two consecutive 16-bit registers (high word, low word)
    pub const STATUS_POWER_HIGH: u16 = 2;
    pub const STATUS_POWER_LOW: u16 = 3;

    /// Voltage in decivolts (e.g., 4800 = 480.0V)
    pub const STATUS_VOLTAGE: u16 = 4;

    /// Current in deciamps (e.g., 1000 = 100.0A, signed)
    pub const STATUS_CURRENT: u16 = 5;

    /// Temperature in decidegrees Celsius (e.g., 250 = 25.0°C)
    pub const STATUS_TEMPERATURE: u16 = 6;

    /// Grid frequency in centihertz (e.g., 6000 = 60.00 Hz)
    pub const STATUS_GRID_FREQUENCY: u16 = 7;

    /// Alarm flags (bitfield)
    pub const STATUS_ALARMS: u16 = 8;

    /// Number of registers to read for status (all status registers)
    pub const STATUS_READ_COUNT: u16 = 9;

    // === Write Registers (Holding Registers, Function Code 6/16) ===

    /// Command register - write command type here
    pub const CMD_COMMAND: u16 = 100;

    /// Target SOC percentage for charge commands (0-10000 = 0.00-100.00%)
    pub const CMD_TARGET_SOC: u16 = 101;

    /// Command duration in seconds (0 = indefinite)
    pub const CMD_DURATION_HIGH: u16 = 102;
    pub const CMD_DURATION_LOW: u16 = 103;

    /// Ramp duration in seconds
    pub const CMD_RAMP_DURATION: u16 = 104;

    /// Number of registers to write for a command
    pub const CMD_WRITE_COUNT: u16 = 5;

    /// Starting address for command writes
    pub const CMD_START_ADDRESS: u16 = 100;
}

/// Parse a signed 32-bit integer from two consecutive 16-bit registers
/// (big-endian)
pub fn parse_i32_from_registers(high: u16, low: u16) -> i32 {
    ((high as i32) << 16) | (low as i32)
}

/// Parse an unsigned 32-bit integer from two consecutive 16-bit registers
/// (big-endian)
pub fn parse_u32_from_registers(high: u16, low: u16) -> u32 {
    ((high as u32) << 16) | (low as u32)
}

/// Split a signed 32-bit integer into two 16-bit registers (big-endian)
pub fn i32_to_registers(value: i32) -> (u16, u16) {
    let high = ((value >> 16) & 0xffff) as u16;
    let low = (value & 0xffff) as u16;
    (high, low)
}

/// Split an unsigned 32-bit integer into two 16-bit registers (big-endian)
pub fn u32_to_registers(value: u32) -> (u16, u16) {
    let high = ((value >> 16) & 0xffff) as u16;
    let low = (value & 0xffff) as u16;
    (high, low)
}

/// Parse SOC from register value (0-10000) to percentage (0.0-100.0)
pub fn parse_soc(register_value: u16) -> f32 {
    (register_value as f32) / 100.0
}

/// Convert SOC percentage (0.0-100.0) to register value (0-10000)
pub fn soc_to_register(soc_percent: f32) -> u16 {
    (soc_percent * 100.0).clamp(0.0, 10000.0) as u16
}

/// Parse voltage from register value (decivolts) to volts
pub fn parse_voltage(register_value: u16) -> f32 {
    (register_value as f32) / 10.0
}

/// Parse current from register value (deciamps, signed) to amps
pub fn parse_current(register_value: u16) -> f32 {
    (register_value as i16 as f32) / 10.0
}

/// Parse temperature from register value (decidegrees) to Celsius
pub fn parse_temperature(register_value: u16) -> f32 {
    (register_value as i16 as f32) / 10.0
}

/// Parse grid frequency from register value (centihertz) to Hz
pub fn parse_grid_frequency(register_value: u16) -> f32 {
    (register_value as f32) / 100.0
}

/// Parse power from register values (watts) to kW
pub fn parse_power_kw(high: u16, low: u16) -> f32 {
    let watts = parse_i32_from_registers(high, low);
    (watts as f32) / 1000.0
}

/// Parsed status data from a read operation
#[derive(Debug, Clone)]
pub struct ParsedStatus {
    pub mode: OperatingMode,
    pub soc_percent: f32,
    pub power_kw: f32,
    pub voltage_v: f32,
    pub current_a: f32,
    pub temperature_c: f32,
    pub grid_frequency_hz: f32,
    pub alarm_flags: u16,
}

impl ParsedStatus {
    /// Parse status from a slice of register values
    ///
    /// Expects registers in order: mode, soc, power_high, power_low, voltage,
    /// current, temperature, grid_frequency, alarms
    pub fn from_registers(registers: &[u16]) -> Option<Self> {
        if registers.len() < RegisterMap::STATUS_READ_COUNT as usize {
            return None;
        }

        Some(Self {
            mode: OperatingMode::from_register(registers[0]),
            soc_percent: parse_soc(registers[1]),
            power_kw: parse_power_kw(registers[2], registers[3]),
            voltage_v: parse_voltage(registers[4]),
            current_a: parse_current(registers[5]),
            temperature_c: parse_temperature(registers[6]),
            grid_frequency_hz: parse_grid_frequency(registers[7]),
            alarm_flags: registers[8],
        })
    }
}

/// Build command register values for a write operation
///
/// Values are validated and clamped to safe ranges:
/// - SOC is clamped to 0-100% before scaling to register representation
///   (0-10000)
/// - Ramp duration is clamped to 0..=u16::MAX to avoid wrapping on cast
pub fn build_command_registers(
    command_type: CommandType,
    target_soc_percent: Option<i32>,
    duration_seconds: Option<i32>,
    ramp_duration_seconds: i32,
) -> Vec<u16> {
    let command = command_type.to_register();

    // Clamp SOC to 0-100% before scaling to register representation (0-10000)
    let target_soc = target_soc_percent
        .map(|s| {
            let clamped = s.clamp(0, 100);
            (clamped * 100) as u16
        })
        .unwrap_or(0);

    let (duration_high, duration_low) = i32_to_registers(duration_seconds.unwrap_or(0));

    // Clamp ramp duration to 0..=u16::MAX to avoid wrapping on cast
    let ramp = if ramp_duration_seconds <= 0 {
        0
    } else if ramp_duration_seconds as i64 > u16::MAX as i64 {
        u16::MAX
    } else {
        ramp_duration_seconds as u16
    };

    vec![command, target_soc, duration_high, duration_low, ramp]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operating_mode_roundtrip() {
        for mode in [
            OperatingMode::Standby,
            OperatingMode::Charging,
            OperatingMode::Discharging,
            OperatingMode::TrickleCharge,
            OperatingMode::Fault,
            OperatingMode::EmergencyStop,
            OperatingMode::Initializing,
        ] {
            let register = mode.to_register();
            let restored = OperatingMode::from_register(register);
            assert_eq!(mode, restored);
        }
    }

    #[test]
    fn test_i32_register_conversion() {
        let test_values = [0, 1, -1, 1000000, -1000000, i32::MAX, i32::MIN];
        for value in test_values {
            let (high, low) = i32_to_registers(value);
            let restored = parse_i32_from_registers(high, low);
            assert_eq!(value, restored, "Failed for value {}", value);
        }
    }

    #[test]
    fn test_parse_soc() {
        assert_eq!(parse_soc(0), 0.0);
        assert_eq!(parse_soc(5000), 50.0);
        assert_eq!(parse_soc(10000), 100.0);
    }

    #[test]
    fn test_parse_voltage() {
        assert_eq!(parse_voltage(4800), 480.0);
        assert_eq!(parse_voltage(0), 0.0);
    }

    #[test]
    fn test_parse_temperature() {
        assert_eq!(parse_temperature(250), 25.0);
        // Test negative temperature (signed)
        assert_eq!(parse_temperature(65526), -1.0); // -10 in signed 16-bit / 10
    }

    #[test]
    fn test_build_command_registers() {
        let registers = build_command_registers(CommandType::Charge, Some(80), Some(3600), 30);
        assert_eq!(registers.len(), 5);
        assert_eq!(registers[0], 1); // Charge command
        assert_eq!(registers[1], 8000); // 80% * 100
        assert_eq!(registers[4], 30); // Ramp duration
    }

    #[test]
    fn test_parsed_status() {
        let registers = vec![
            1,     // Mode: Charging
            5000,  // SOC: 50%
            0,     // Power high
            10000, // Power low: 10000W = 10kW
            4800,  // Voltage: 480V
            1000,  // Current: 100A
            250,   // Temperature: 25C
            6000,  // Grid frequency: 60Hz
            0,     // No alarms
        ];

        let status = ParsedStatus::from_registers(&registers).unwrap();
        assert_eq!(status.mode, OperatingMode::Charging);
        assert_eq!(status.soc_percent, 50.0);
        assert_eq!(status.power_kw, 10.0);
        assert_eq!(status.voltage_v, 480.0);
        assert_eq!(status.current_a, 100.0);
        assert_eq!(status.temperature_c, 25.0);
        assert_eq!(status.grid_frequency_hz, 60.0);
    }
}
