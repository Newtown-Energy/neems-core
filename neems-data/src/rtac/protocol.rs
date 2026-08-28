//! Modbus protocol definitions for RTAC communication
//!
//! This module defines the register mappings, command types, operating modes,
//! and parsing functions for the RTAC Modbus protocol.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::alarm_definitions::{ALARM_REGISTER_COUNT, AlarmZone};

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

    /// Parse a command type from a command register value
    ///
    /// Returns `None` for unrecognized values (so callers can decide how to
    /// treat an invalid command rather than silently coercing it).
    pub fn from_register(value: u16) -> Option<Self> {
        match value {
            0 => Some(Self::Standby),
            1 => Some(Self::Charge),
            2 => Some(Self::Discharge),
            3 => Some(Self::TrickleCharge),
            4 => Some(Self::EmergencyStop),
            5 => Some(Self::ClearFaults),
            _ => None,
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

    /// First alarm register (start of 22-register alarm block)
    pub const STATUS_ALARMS_START: u16 = 8;

    /// Number of registers to read for status (8 base + 22 alarm registers)
    pub const STATUS_READ_COUNT: u16 = 8 + ALARM_REGISTER_COUNT as u16;

    /// How the client's point numbers relate to Modbus wire addresses.
    ///
    /// The spreadsheet's "alarm number" *is* the point's Modbus address. The
    /// two categories do not collide despite sharing numbers — analog 601 is
    /// MP-1A `real_power_target`, digital 601 is MP-1A
    /// `megapack_loss_of_comms` — because Modbus gives bits and registers
    /// separate address spaces (see
    /// [`super::analog_points::ANALOG_POINTS`] and
    /// [`super::alarm_definitions::AlarmDefinition::discrete_address`]).
    ///
    /// This constant exists because vendor point lists are as often 1-based as
    /// 0-based and the spreadsheet does not say which it is. We take the
    /// numbers literally — base 0 — because that is what the client told us.
    /// If readings come back one point out (a `frequency` where
    /// `state_of_energy` was asked for), this is the single line to change;
    /// nothing else encodes the assumption.
    pub const POINT_NUMBER_BASE: u16 = 0;

    /// Wire address of the point the spreadsheet numbers `point_number`.
    pub const fn point_address(point_number: u16) -> u16 {
        point_number - Self::POINT_NUMBER_BASE
    }

    /// Address of `offset` within `pack_index`'s analog block.
    ///
    /// `pack_index` is the position in [`MEGAPACK_ZONES`]; `offset` is the
    /// point's position in the spreadsheet's canonical 30-point order.
    pub const fn mp_analog_address(pack_index: usize, offset: u16) -> u16 {
        Self::point_address(MEGAPACK_ANALOG_BASE_POINT[pack_index] + offset)
    }

    // === Write Registers (Holding Registers, Function Code 6/16) ===

    // These five are ours, not the client's: nothing in the spreadsheet
    // allocates command registers. They used to sit at 100-104, which
    // collided head-on with the transformer winding temperatures at analog
    // points 101 and 102 once point numbers became addresses. Moved to 1000
    // to clear the whole client-defined range (101-102 and 601-780) with room
    // to spare, so the map stays correct whether the analogs turn out to be
    // holding registers or input registers.

    /// Command register - write command type here
    pub const CMD_COMMAND: u16 = 1000;

    /// Target SOC percentage for charge commands (0-10000 = 0.00-100.00%)
    pub const CMD_TARGET_SOC: u16 = 1001;

    /// Command duration in seconds (0 = indefinite)
    pub const CMD_DURATION_HIGH: u16 = 1002;
    pub const CMD_DURATION_LOW: u16 = 1003;

    /// Ramp duration in seconds
    pub const CMD_RAMP_DURATION: u16 = 1004;

    /// Number of registers to write for a command
    pub const CMD_WRITE_COUNT: u16 = 5;

    /// Starting address for command writes
    pub const CMD_START_ADDRESS: u16 = 1000;
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

/// Convert voltage (volts) to register value (decivolts)
///
/// Inverse of [`parse_voltage`]. Clamped to the unsigned 16-bit range.
pub fn voltage_to_register(volts: f32) -> u16 {
    (volts * 10.0).clamp(0.0, u16::MAX as f32) as u16
}

/// Convert current (amps, signed) to register value (deciamps)
///
/// Inverse of [`parse_current`]. Negative currents are encoded as a signed
/// 16-bit value reinterpreted as `u16`.
pub fn current_to_register(amps: f32) -> u16 {
    ((amps * 10.0).clamp(i16::MIN as f32, i16::MAX as f32) as i16) as u16
}

/// Convert temperature (Celsius, signed) to register value (decidegrees)
///
/// Inverse of [`parse_temperature`].
pub fn temperature_to_register(celsius: f32) -> u16 {
    ((celsius * 10.0).clamp(i16::MIN as f32, i16::MAX as f32) as i16) as u16
}

/// Convert grid frequency (Hz) to register value (centihertz)
///
/// Inverse of [`parse_grid_frequency`].
pub fn grid_frequency_to_register(hz: f32) -> u16 {
    (hz * 100.0).clamp(0.0, u16::MAX as f32) as u16
}

/// Convert power (kW, signed) to two registers (watts, big-endian high/low)
///
/// Inverse of [`parse_power_kw`].
pub fn power_kw_to_registers(kw: f32) -> (u16, u16) {
    let watts = (kw * 1000.0) as i32;
    i32_to_registers(watts)
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
    /// Raw alarm register values (22 registers, one per alarm zone group)
    pub alarm_registers: [u16; ALARM_REGISTER_COUNT],
}

impl ParsedStatus {
    /// Parse status from a slice of register values
    ///
    /// Expects registers in order: mode, soc, power_high, power_low, voltage,
    /// current, temperature, grid_frequency, followed by 22 alarm registers
    pub fn from_registers(registers: &[u16]) -> Option<Self> {
        if registers.len() < RegisterMap::STATUS_READ_COUNT as usize {
            return None;
        }

        let mut alarm_registers = [0u16; ALARM_REGISTER_COUNT];
        alarm_registers.copy_from_slice(&registers[8..8 + ALARM_REGISTER_COUNT]);

        Some(Self {
            mode: OperatingMode::from_register(registers[0]),
            soc_percent: parse_soc(registers[1]),
            power_kw: parse_power_kw(registers[2], registers[3]),
            voltage_v: parse_voltage(registers[4]),
            current_a: parse_current(registers[5]),
            temperature_c: parse_temperature(registers[6]),
            grid_frequency_hz: parse_grid_frequency(registers[7]),
            alarm_registers,
        })
    }
}

/// Megapack zones in the order their analog blocks are laid out, matching the
/// spreadsheet's MP-1A..MP-2C row order. A pack's index here is the
/// `pack_index` argument to [`RegisterMap::mp_analog_address`].
pub const MEGAPACK_ZONES: [AlarmZone; 6] = [
    AlarmZone::Mp1a,
    AlarmZone::Mp1b,
    AlarmZone::Mp1c,
    AlarmZone::Mp2a,
    AlarmZone::Mp2b,
    AlarmZone::Mp2c,
];

/// Analog measurements the spreadsheet defines per Megapack — 30 rows per pack
/// on the `Analogs` sheet (601-630 for MP-1A through 751-780 for MP-2C).
pub const MP_ANALOG_POINT_COUNT: usize = 30;

/// The `Analogs` sheet's point number for each Megapack's first measurement,
/// in [`MEGAPACK_ZONES`] order.
///
/// Listed rather than computed as `601 + 30 * pack_index`, even though that is
/// what the six blocks currently work out to. These are addresses on a device
/// we do not control; if the client renumbers or leaves a gap, that should be
/// a one-line data change here rather than arithmetic that quietly keeps
/// producing plausible-looking wrong addresses.
pub const MEGAPACK_ANALOG_BASE_POINT: [u16; MEGAPACK_ZONES.len()] = [601, 631, 661, 691, 721, 751];

/// Analog points the spreadsheet places outside a Megapack block.
///
/// Just the two transformer winding temperatures; every other analog row
/// belongs to a pack. Both carry a 60C threshold and the spreadsheet's fire
/// flag.
pub mod site_analog_point {
    /// Transformer 1 winding temperature (`49T1`).
    pub const T1_WINDING_TEMPERATURE: u16 = 101;
    /// Transformer 2 winding temperature (`49T2`).
    pub const T2_WINDING_TEMPERATURE: u16 = 102;
}

/// Offsets within a pack's analog block, from the spreadsheet's canonical
/// point order (`megapack_analog_template` in
/// `docs/alarms/newtown-alarms.json`).
///
/// Only the points the SLD renders are named. The other 27 offsets are
/// addressed but left raw, because the spreadsheet gives no point its units or
/// scaling and inventing an encoding we would have to unpick later is worse
/// than carrying the register value through untouched.
///
/// **The named three are no better documented.** Their scaling below is
/// assumed, not specified: it copies the encoding the site-level status
/// registers already use, because that is the only precedent we have. The
/// simulator encodes with the inverse of those same helpers, so a round trip
/// through it proves the two halves of *our* assumption agree — not that
/// either matches the hardware. Confirm the scaling with the client alongside
/// the register addresses; a right address with a wrong scale reports 8.25%
/// where the pack means 82.5%, silently and plausibly.
pub mod mp_analog_offset {
    /// Charge level as a percentage — the vendor's term for state of charge.
    /// The spreadsheet marks this point as driving the SLD's "MP gas gauge".
    pub const STATE_OF_ENERGY: u16 = 4;
    /// AC terminal voltage.
    pub const AC_VOLTAGE: u16 = 10;
    /// Hottest measured battery temperature in the pack.
    pub const MAX_BATTERY_TEMPERATURE: u16 = 18;
}

/// Parsed analog measurements for a single Megapack.
///
/// The three named fields are the ones the SLD renders. `raw_registers` keeps
/// the pack's whole block so the remaining points can be given meanings later
/// without another round of Modbus work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MegapackAnalogs {
    pub zone: AlarmZone,
    /// Charge level, 0.0-100.0%.
    pub state_of_energy_percent: f32,
    pub ac_voltage_v: f32,
    pub max_battery_temperature_c: f32,
    pub raw_registers: [u16; MP_ANALOG_POINT_COUNT],
}

impl MegapackAnalogs {
    /// Parse one pack's block. `registers` must be the pack's
    /// [`MP_ANALOG_POINT_COUNT`] registers, starting at its block base.
    ///
    /// Returns `None` on a short slice rather than padding with zeros — a
    /// zero here would read as "0% charge", which is a claim we have no
    /// grounds to make.
    ///
    /// Also returns `None` when the charge level lands outside 0-100%.
    /// [`parse_soc`] does not clamp, so on the assumed percent-times-100
    /// encoding any register above 10000 yields an impossible percentage, up
    /// to 655.35. That cannot happen against the simulator, whose
    /// [`soc_to_register`] clamps on the way in — which is exactly why it must
    /// be caught here instead: the first hardware to disagree with our scaling
    /// would otherwise put a plausible-looking wrong number on an operator's
    /// gauge. Dropping the whole pack rather than just the charge figure is
    /// deliberate; if the scale is wrong for one point in the block, the other
    /// two are not worth more trust.
    pub fn from_registers(zone: AlarmZone, registers: &[u16]) -> Option<Self> {
        if registers.len() < MP_ANALOG_POINT_COUNT {
            return None;
        }
        let at = |offset: u16| registers[offset as usize];

        let state_of_energy_percent = parse_soc(at(mp_analog_offset::STATE_OF_ENERGY));
        if !(0.0..=100.0).contains(&state_of_energy_percent) {
            return None;
        }

        let mut raw_registers = [0u16; MP_ANALOG_POINT_COUNT];
        raw_registers.copy_from_slice(&registers[..MP_ANALOG_POINT_COUNT]);

        Some(Self {
            zone,
            state_of_energy_percent,
            ac_voltage_v: parse_voltage(at(mp_analog_offset::AC_VOLTAGE)),
            max_battery_temperature_c: parse_temperature(at(
                mp_analog_offset::MAX_BATTERY_TEMPERATURE,
            )),
            raw_registers,
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
    fn megapack_analog_blocks_are_disjoint_and_clear_of_other_blocks() {
        let mut seen = std::collections::HashSet::new();
        for pack_index in 0..MEGAPACK_ZONES.len() {
            for offset in 0..MP_ANALOG_POINT_COUNT as u16 {
                let addr = RegisterMap::mp_analog_address(pack_index, offset);
                // Must not collide with the status/alarm block or the command
                // block, or a charge reading would alias a live register.
                assert!(addr >= RegisterMap::STATUS_READ_COUNT);
                assert!(
                    !(RegisterMap::CMD_START_ADDRESS
                        ..RegisterMap::CMD_START_ADDRESS + RegisterMap::CMD_WRITE_COUNT)
                        .contains(&addr),
                    "address {} collides with the command block",
                    addr
                );
                assert!(seen.insert(addr), "address {} allocated twice", addr);
            }
        }
    }

    #[test]
    fn the_site_analogs_are_clear_of_the_command_block() {
        // The reason the command block moved off 100-104: these two are
        // client-assigned and cannot be relocated, ours could.
        for point in [
            site_analog_point::T1_WINDING_TEMPERATURE,
            site_analog_point::T2_WINDING_TEMPERATURE,
        ] {
            let addr = RegisterMap::point_address(point);
            assert!(
                !(RegisterMap::CMD_START_ADDRESS
                    ..RegisterMap::CMD_START_ADDRESS + RegisterMap::CMD_WRITE_COUNT)
                    .contains(&addr),
                "site analog {} collides with the command block",
                point
            );
        }
    }

    #[test]
    fn megapack_analog_address_is_the_spreadsheet_point_number() {
        // The whole point of the change: an address is the number the client
        // wrote on the row, not a slot in a layout we invented.
        assert_eq!(RegisterMap::mp_analog_address(0, 0), 601); // MP-1A real_power_target
        assert_eq!(RegisterMap::mp_analog_address(0, 29), 630); // MP-1A AI_spare_7
        assert_eq!(RegisterMap::mp_analog_address(1, 0), 631); // MP-1B, no gap
        assert_eq!(RegisterMap::mp_analog_address(5, 29), 780); // MP-2C, last row

        // The three the SLD renders, spelled out so a shifted offset table
        // fails here rather than on an operator's gauge.
        assert_eq!(RegisterMap::mp_analog_address(0, mp_analog_offset::STATE_OF_ENERGY), 605);
        assert_eq!(RegisterMap::mp_analog_address(0, mp_analog_offset::AC_VOLTAGE), 611);
        assert_eq!(
            RegisterMap::mp_analog_address(0, mp_analog_offset::MAX_BATTERY_TEMPERATURE),
            619
        );
    }

    #[test]
    fn every_megapack_block_is_thirty_points_long() {
        // A renumbering that overlapped two packs would otherwise be caught
        // only by the disjointness test above, which cannot say which pack
        // moved.
        for (pack_index, base) in MEGAPACK_ANALOG_BASE_POINT.iter().enumerate() {
            assert_eq!(RegisterMap::mp_analog_address(pack_index, 0), *base);
            if let Some(next) = MEGAPACK_ANALOG_BASE_POINT.get(pack_index + 1) {
                assert_eq!(
                    next - base,
                    MP_ANALOG_POINT_COUNT as u16,
                    "pack {} block is not {} points long",
                    pack_index,
                    MP_ANALOG_POINT_COUNT
                );
            }
        }
    }

    #[test]
    fn the_generated_registry_agrees_with_the_address_map() {
        use super::super::analog_points::ANALOG_POINTS;

        assert_eq!(ANALOG_POINTS.len(), 182, "spec defines 182 analog points");

        for point in ANALOG_POINTS {
            match point.offset {
                // A pack's point must be reachable at its own number through
                // the address map. This is the join between the generated
                // table and the arithmetic the client actually uses; without
                // it the two could disagree and each would look right alone.
                Some(offset) => {
                    let pack_index = MEGAPACK_ZONES
                        .iter()
                        .position(|z| *z == point.zone)
                        .unwrap_or_else(|| panic!("{} is not a Megapack zone", point.zone));
                    assert_eq!(
                        RegisterMap::mp_analog_address(pack_index, offset),
                        RegisterMap::point_address(point.point_number),
                        "{} {} does not sit where the address map puts it",
                        point.zone,
                        point.name
                    );
                    assert!((offset as usize) < MP_ANALOG_POINT_COUNT);
                }
                // The only points outside a pack block are the two transformer
                // winding temperatures.
                None => assert!(
                    point.point_number == site_analog_point::T1_WINDING_TEMPERATURE
                        || point.point_number == site_analog_point::T2_WINDING_TEMPERATURE,
                    "unexpected block-less analog point {}",
                    point.point_number
                ),
            }
        }
    }

    #[test]
    fn the_named_offsets_point_at_the_measurements_they_claim() {
        use super::super::analog_points::analog_point;

        // mp_analog_offset is hand-written while the registry is generated, so
        // a spreadsheet reordering would silently repoint these three at
        // whatever moved into the slot. Checking the names catches that.
        for (offset, expected) in [
            (mp_analog_offset::STATE_OF_ENERGY, "state_of_energy"),
            (mp_analog_offset::AC_VOLTAGE, "ac_voltage"),
            (mp_analog_offset::MAX_BATTERY_TEMPERATURE, "max_battery_temperature"),
        ] {
            let number = RegisterMap::mp_analog_address(0, offset);
            let point = analog_point(number).expect("MP-1A point exists");
            assert_eq!(point.name, expected, "offset {} is no longer {}", offset, expected);
            assert_eq!(point.zone, AlarmZone::Mp1a);
        }
    }

    #[test]
    fn megapack_analogs_parse_the_rendered_points() {
        let mut regs = [0u16; MP_ANALOG_POINT_COUNT];
        regs[mp_analog_offset::STATE_OF_ENERGY as usize] = soc_to_register(82.5);
        regs[mp_analog_offset::AC_VOLTAGE as usize] = voltage_to_register(479.6);
        regs[mp_analog_offset::MAX_BATTERY_TEMPERATURE as usize] = temperature_to_register(27.4);

        let parsed = MegapackAnalogs::from_registers(AlarmZone::Mp1a, &regs).unwrap();
        assert_eq!(parsed.zone, AlarmZone::Mp1a);
        assert_eq!(parsed.state_of_energy_percent, 82.5);
        assert_eq!(parsed.ac_voltage_v, 479.6);
        assert_eq!(parsed.max_battery_temperature_c, 27.4);
        assert_eq!(parsed.raw_registers, regs);
    }

    #[test]
    fn megapack_analogs_reject_an_impossible_charge_level() {
        // The simulator can never produce this — soc_to_register clamps — so
        // only real hardware disagreeing with our assumed scaling would. That
        // is precisely the case that must not reach a gauge.
        let mut regs = [0u16; MP_ANALOG_POINT_COUNT];
        regs[mp_analog_offset::STATE_OF_ENERGY as usize] = 10_001;
        assert!(MegapackAnalogs::from_registers(AlarmZone::Mp1a, &regs).is_none());

        regs[mp_analog_offset::STATE_OF_ENERGY as usize] = u16::MAX;
        assert!(MegapackAnalogs::from_registers(AlarmZone::Mp1a, &regs).is_none());

        // The boundary itself is a legitimate reading: a full pack.
        regs[mp_analog_offset::STATE_OF_ENERGY as usize] = 10_000;
        let parsed = MegapackAnalogs::from_registers(AlarmZone::Mp1a, &regs).unwrap();
        assert_eq!(parsed.state_of_energy_percent, 100.0);
    }

    #[test]
    fn megapack_analogs_reject_a_short_block() {
        // Padding a short read with zeros would surface as "0% charge".
        let regs = [0u16; MP_ANALOG_POINT_COUNT - 1];
        assert!(MegapackAnalogs::from_registers(AlarmZone::Mp1a, &regs).is_none());
    }

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
    fn test_command_type_register_roundtrip() {
        for cmd in [
            CommandType::Standby,
            CommandType::Charge,
            CommandType::Discharge,
            CommandType::TrickleCharge,
            CommandType::EmergencyStop,
            CommandType::ClearFaults,
        ] {
            assert_eq!(CommandType::from_register(cmd.to_register()), Some(cmd));
        }
        assert_eq!(CommandType::from_register(99), None);
    }

    #[test]
    fn test_scaling_inverse_roundtrips() {
        // Voltage: decivolts
        assert_eq!(voltage_to_register(480.0), 4800);
        assert_eq!(parse_voltage(voltage_to_register(480.0)), 480.0);

        // Grid frequency: centihertz
        assert_eq!(grid_frequency_to_register(60.0), 6000);
        assert_eq!(parse_grid_frequency(grid_frequency_to_register(60.0)), 60.0);

        // Temperature: signed decidegrees
        assert_eq!(parse_temperature(temperature_to_register(25.0)), 25.0);
        assert_eq!(parse_temperature(temperature_to_register(-10.0)), -10.0);

        // Current: signed deciamps
        assert_eq!(parse_current(current_to_register(100.0)), 100.0);
        assert_eq!(parse_current(current_to_register(-50.0)), -50.0);

        // Power: signed watts across two registers
        let (h, l) = power_kw_to_registers(10.0);
        assert_eq!(parse_power_kw(h, l), 10.0);
        let (h, l) = power_kw_to_registers(-25.5);
        assert_eq!(parse_power_kw(h, l), -25.5);
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
        let mut registers = vec![
            1,     // Mode: Charging
            5000,  // SOC: 50%
            0,     // Power high
            10000, // Power low: 10000W = 10kW
            4800,  // Voltage: 480V
            1000,  // Current: 100A
            250,   // Temperature: 25C
            6000,  // Grid frequency: 60Hz
        ];
        // Append 22 alarm registers (all zeros = no alarms)
        registers.extend(std::iter::repeat_n(0u16, ALARM_REGISTER_COUNT));

        let status = ParsedStatus::from_registers(&registers).unwrap();
        assert_eq!(status.mode, OperatingMode::Charging);
        assert_eq!(status.soc_percent, 50.0);
        assert_eq!(status.power_kw, 10.0);
        assert_eq!(status.voltage_v, 480.0);
        assert_eq!(status.current_a, 100.0);
        assert_eq!(status.temperature_c, 25.0);
        assert_eq!(status.grid_frequency_hz, 60.0);
        assert_eq!(status.alarm_registers, [0u16; ALARM_REGISTER_COUNT]);
    }

    #[test]
    fn test_parsed_status_with_alarms() {
        let mut registers = vec![0u16; 8]; // base status registers
        let mut alarm_regs = [0u16; ALARM_REGISTER_COUNT];
        // Set bit 3 in register 1 (estop, alarm 104)
        alarm_regs[1] = 0x0008;
        registers.extend_from_slice(&alarm_regs);

        let status = ParsedStatus::from_registers(&registers).unwrap();
        assert_eq!(status.alarm_registers[1], 0x0008);
    }
}
