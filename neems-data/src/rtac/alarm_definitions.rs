//! Alarm Definitions
//!
//! This module defines all digital alarm points for the site, derived
//! from the alarm register map spreadsheet. Each alarm has a unique number,
//! zone, name, severity level, and a specific bit position within the Modbus
//! alarm registers.
//!
//! ## Register Layout
//!
//! Alarm registers are read as a contiguous block of 22 holding registers
//! starting at address 8 (immediately after the 8 status registers). Each
//! register holds 16 alarm bits.
//!
//! | Register | Address | Zone                | Alarm Numbers |
//! |----------|---------|---------------------|---------------|
//! | 0        | 8       | Site (Newtown)      | 1–7           |
//! | 1–2      | 9–10    | Breaker Relay       | 101–126       |
//! | 3        | 11      | Meter               | 201–203       |
//! | 4        | 12      | Transformer 1       | 301–310       |
//! | 5        | 13      | Transformer 2       | 311–320       |
//! | 6        | 14      | RTAC                | 321–330       |
//! | 7–8      | 15–16   | FACP                | 401–425       |
//! | 9        | 17      | Tesla Site Ctrl     | 501–510       |
//! | 10–11    | 18–19   | Megapack 1A         | 601–630       |
//! | 12–13    | 20–21   | Megapack 1B         | 631–660       |
//! | 14–15    | 22–23   | Megapack 1C         | 661–690       |
//! | 16–17    | 24–25   | Megapack 2A         | 691–720       |
//! | 18–19    | 26–27   | Megapack 2B         | 721–750       |
//! | 20–21    | 28–29   | Megapack 2C         | 751–780       |
//!
//! ## Alarm Levels
//!
//! Levels follow the Newtown alarm matrix:
//! - **1**: Emergency — active fire alarm, call 911, emergency shutdown
//! - **2**: High — activate COF, contact SMEs, immediate response
//! - **3**: Medium — inform management, contact SMEs
//! - **4**: Low — operator troubleshooting, escalate during business hours
//! - **5**: Informational / unclassified

use std::fmt;

use serde::{Deserialize, Serialize};

/// Number of 16-bit Modbus registers used for alarm flags
pub const ALARM_REGISTER_COUNT: usize = 22;

/// Alarm number for the emergency stop (Estop) alarm
pub const ESTOP_ALARM_NUM: u16 = 104;

/// Alarm number for the 86-M1 relay lockout
pub const RELAY_86_M1_SET_ALARM_NUM: u16 = 103;

/// Alarm number for the fire alarm
pub const FIRE_ALARM_NUM: u16 = 401;

/// Alarm number for transformer 1 temperature alarm
pub const T1_TEMP_ALARM_NUM: u16 = 301;

/// Alarm number for transformer 1 temperature trip
pub const T1_TEMP_TRIP_ALARM_NUM: u16 = 302;

/// Alarm number for transformer 1 temperature fault
pub const T1_TEMP_FAULT_ALARM_NUM: u16 = 303;

/// Alarm number for transformer 2 temperature trip
pub const T2_TEMP_TRIP_ALARM_NUM: u16 = 312;

/// Alarm number for ConEd curtailment trip
pub const CONED_CURTAILMENT_TRIP_ALARM_NUM: u16 = 321;

/// Alarm number for utility loss of power
pub const UTILITY_LOP_ALARM_NUM: u16 = 322;

/// Alarm zones in the Newtown system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlarmZone {
    /// Site-level alarms (Newtown)
    Site,
    /// Breaker Relay SEL-451
    BreakerRelay,
    /// Meter 1 SEL-735
    Meter,
    /// Transformer 1
    Transformer1,
    /// Transformer 2
    Transformer2,
    /// RTAC
    Rtac,
    /// Fire Alarm Control Panel
    Facp,
    /// Tesla Site Controller
    TeslaSiteController,
    /// Megapack 1A
    Mp1a,
    /// Megapack 1B
    Mp1b,
    /// Megapack 1C
    Mp1c,
    /// Megapack 2A
    Mp2a,
    /// Megapack 2B
    Mp2b,
    /// Megapack 2C
    Mp2c,
}

impl fmt::Display for AlarmZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Site => write!(f, "site"),
            Self::BreakerRelay => write!(f, "breaker_relay"),
            Self::Meter => write!(f, "meter"),
            Self::Transformer1 => write!(f, "transformer_1"),
            Self::Transformer2 => write!(f, "transformer_2"),
            Self::Rtac => write!(f, "rtac"),
            Self::Facp => write!(f, "facp"),
            Self::TeslaSiteController => write!(f, "tesla_site_controller"),
            Self::Mp1a => write!(f, "mp1a"),
            Self::Mp1b => write!(f, "mp1b"),
            Self::Mp1c => write!(f, "mp1c"),
            Self::Mp2a => write!(f, "mp2a"),
            Self::Mp2b => write!(f, "mp2b"),
            Self::Mp2c => write!(f, "mp2c"),
        }
    }
}

/// Static definition of a single alarm point
#[derive(Debug, Clone)]
pub struct AlarmDefinition {
    /// Unique alarm number from the Newtown alarm matrix
    pub alarm_num: u16,
    /// Zone this alarm belongs to
    pub zone: AlarmZone,
    /// Snake_case alarm name
    pub name: &'static str,
    /// Alarm level (1=emergency, 2=high, 3=medium, 4=low, 5=info)
    pub level: u8,
    /// Index into the alarm register array (0-based)
    pub register_index: usize,
    /// Bit position within the register (0–15)
    pub bit: u8,
}

impl AlarmDefinition {
    /// Returns a qualified name: "zone/name"
    pub fn qualified_name(&self) -> String {
        format!("{}/{}", self.zone, self.name)
    }
}

/// Compact alarm definition constructor
macro_rules! alarm {
    ($num:expr, $zone:ident, $name:expr, $level:expr, $reg:expr, $bit:expr) => {
        AlarmDefinition {
            alarm_num: $num,
            zone: AlarmZone::$zone,
            name: $name,
            level: $level,
            register_index: $reg,
            bit: $bit,
        }
    };
}

/// Per-megapack alarm type names (30 types per megapack, matching DNP3 points 1–30)
#[cfg(test)]
const MP_ALARM_NAMES: [&str; 30] = [
    "megapack_loss_of_comms",       // pt 1
    "megapack_iso_failure",         // pt 2
    "megapack_inverter_fault",      // pt 3
    "enable_circuit_open",          // pt 4
    "enable_switch_off",            // pt 5
    "door_switch_open",             // pt 6
    "ac_breaker_closed",            // pt 7
    "bus_controller_prolonged_fault", // pt 8
    "remote_shutdown",              // pt 9
    "coolant_low",                  // pt 10
    "extreme_temp_warning",         // pt 11
    "extreme_temp_fault",           // pt 12
    "grid_uncompliant",             // pt 13
    "low_state_of_energy",          // pt 14
    "breaker_irrational",           // pt 15
    "breaker_ready_to_close",       // pt 16
    "megapack_disabled",            // pt 17
    "power_elec_over_temp",         // pt 18
    "sparker",                      // pt 19
    "cell_modem_en",                // pt 20
    "mp_comms_warning",             // pt 21
    "di_spare_1",                   // pt 22
    "di_spare_2",                   // pt 23
    "di_spare_3",                   // pt 24
    "di_spare_4",                   // pt 25
    "di_spare_5",                   // pt 26
    "di_spare_6",                   // pt 27
    "di_spare_7",                   // pt 28
    "di_spare_8",                   // pt 29
    "di_spare_9",                   // pt 30
];

/// Per-megapack alarm severity levels (matching the alarm type order above).
/// Most MP alarms are unclassified in the spreadsheet; levels here are
/// conservative defaults based on alarm semantics.
#[cfg(test)]
const MP_ALARM_LEVELS: [u8; 30] = [
    3, // megapack_loss_of_comms
    2, // megapack_iso_failure
    2, // megapack_inverter_fault
    3, // enable_circuit_open
    3, // enable_switch_off
    4, // door_switch_open
    4, // ac_breaker_closed
    2, // bus_controller_prolonged_fault
    3, // remote_shutdown
    3, // coolant_low
    3, // extreme_temp_warning
    2, // extreme_temp_fault
    3, // grid_uncompliant
    4, // low_state_of_energy
    3, // breaker_irrational
    5, // breaker_ready_to_close
    3, // megapack_disabled
    2, // power_elec_over_temp
    2, // sparker
    5, // cell_modem_en
    4, // mp_comms_warning
    5, // di_spare_1
    5, // di_spare_2
    5, // di_spare_3
    5, // di_spare_4
    5, // di_spare_5
    5, // di_spare_6
    5, // di_spare_7
    5, // di_spare_8
    5, // di_spare_9
];

// ---- All alarm definitions ----

/// All alarm definitions for the Newtown system.
///
/// Organized by zone in alarm-number order. Register indices and bit
/// positions correspond to the Modbus alarm register layout documented above.
pub const ALARM_DEFINITIONS: &[AlarmDefinition] = &[
    // =========================================================================
    // Site / Newtown alarms (register 0, alarms 1–7)
    // =========================================================================
    alarm!(1,   Site, "loss_fiber",             3, 0, 0),
    alarm!(2,   Site, "loss_cellular",          4, 0, 1),
    alarm!(3,   Site, "no_ip_connection",       3, 0, 2),
    alarm!(4,   Site, "loss_managed_switch",    5, 0, 3),
    alarm!(5,   Site, "site_cyber_event",       5, 0, 4),
    alarm!(6,   Site, "scada_cabinet_door_open",5, 0, 5),
    alarm!(7,   Site, "intruder_detected",      5, 0, 6),

    // =========================================================================
    // Breaker Relay SEL-451 (registers 1–2, alarms 101–126)
    // Register 1: alarms 101–116 (bits 0–15)
    // Register 2: alarms 117–126 (bits 0–9)
    // =========================================================================
    alarm!(101, BreakerRelay, "bps_89l1_open",   4, 1, 0),
    alarm!(102, BreakerRelay, "bps_89l2_open",   5, 1, 1),
    alarm!(103, BreakerRelay, "relay_86_m1_set", 2, 1, 2),
    alarm!(104, BreakerRelay, "estop",           2, 1, 3),
    alarm!(105, BreakerRelay, "salarm_451",      3, 1, 4),
    alarm!(106, BreakerRelay, "halarm_451",      3, 1, 5),
    alarm!(107, BreakerRelay, "ansi_27_1",       4, 1, 6),
    alarm!(108, BreakerRelay, "ansi_27_2",       5, 1, 7),
    alarm!(109, BreakerRelay, "ansi_59_1",       4, 1, 8),
    alarm!(110, BreakerRelay, "ansi_59_2",       5, 1, 9),
    alarm!(111, BreakerRelay, "ansi_81u_1",      4, 1, 10),
    alarm!(112, BreakerRelay, "ansi_81u_2",      5, 1, 11),
    alarm!(113, BreakerRelay, "ansi_81o_1",      4, 1, 12),
    alarm!(114, BreakerRelay, "ansi_81o_2",      5, 1, 13),
    alarm!(115, BreakerRelay, "ansi_50p1t",      4, 1, 14),
    alarm!(116, BreakerRelay, "ansi_50p2t",      5, 1, 15),
    alarm!(117, BreakerRelay, "ansi_51s1t",      4, 2, 0),
    alarm!(118, BreakerRelay, "ansi_50g1t",      5, 2, 1),
    alarm!(119, BreakerRelay, "ansi_50g2t",      5, 2, 2),
    alarm!(120, BreakerRelay, "ansi_51s2t",      5, 2, 3),
    alarm!(121, BreakerRelay, "ansi_32r",        2, 2, 4),
    alarm!(122, BreakerRelay, "ansi_32f",        5, 2, 5),
    alarm!(123, BreakerRelay, "ansi_psv01t",     5, 2, 6),
    alarm!(124, BreakerRelay, "ansi_psv02t",     5, 2, 7),
    alarm!(125, BreakerRelay, "ansi_psv03t",     5, 2, 8),
    alarm!(126, BreakerRelay, "ansi_psv04t",     5, 2, 9),

    // =========================================================================
    // Meter 1 SEL-735 (register 3, alarms 201–203)
    // =========================================================================
    alarm!(201, Meter, "salarm_735",             2, 3, 0),
    alarm!(202, Meter, "halarm_735",             2, 3, 1),
    alarm!(203, Meter, "meter_loss_of_comms",    5, 3, 2),

    // =========================================================================
    // Transformer 1 (register 4, alarms 301–310)
    // =========================================================================
    alarm!(301, Transformer1, "t1_temp_alarm",   4, 4, 0),
    alarm!(302, Transformer1, "t1_temp_trip",    2, 4, 1),
    alarm!(303, Transformer1, "t1_temp_fault",   3, 4, 2),
    alarm!(304, Transformer1, "t1_reserved_4",   5, 4, 3),
    alarm!(305, Transformer1, "t1_reserved_5",   5, 4, 4),
    alarm!(306, Transformer1, "t1_reserved_6",   5, 4, 5),
    alarm!(307, Transformer1, "t1_reserved_7",   5, 4, 6),
    alarm!(308, Transformer1, "t1_reserved_8",   5, 4, 7),
    alarm!(309, Transformer1, "t1_reserved_9",   5, 4, 8),
    alarm!(310, Transformer1, "t1_reserved_10",  5, 4, 9),

    // =========================================================================
    // Transformer 2 (register 5, alarms 311–320)
    // =========================================================================
    alarm!(311, Transformer2, "t2_temp_alarm",   4, 5, 0),
    alarm!(312, Transformer2, "t2_temp_trip",    2, 5, 1),
    alarm!(313, Transformer2, "t2_temp_fault",   3, 5, 2),
    alarm!(314, Transformer2, "t2_reserved_4",   5, 5, 3),
    alarm!(315, Transformer2, "t2_reserved_5",   5, 5, 4),
    alarm!(316, Transformer2, "t2_reserved_6",   5, 5, 5),
    alarm!(317, Transformer2, "t2_reserved_7",   5, 5, 6),
    alarm!(318, Transformer2, "t2_reserved_8",   5, 5, 7),
    alarm!(319, Transformer2, "t2_reserved_9",   5, 5, 8),
    alarm!(320, Transformer2, "t2_reserved_10",  5, 5, 9),

    // =========================================================================
    // RTAC (register 6, alarms 321–330)
    // =========================================================================
    alarm!(321, Rtac, "coned_curtailment_trip",  4, 6, 0),
    alarm!(322, Rtac, "utility_lop",             4, 6, 1),
    alarm!(323, Rtac, "low_ups_battery",         3, 6, 2),
    alarm!(324, Rtac, "ups_ground_fault_pos",    5, 6, 3),
    alarm!(325, Rtac, "ups_ground_fault_neg",    5, 6, 4),
    alarm!(326, Rtac, "rtac_reserved_6",         5, 6, 5),
    alarm!(327, Rtac, "rtac_reserved_7",         5, 6, 6),
    alarm!(328, Rtac, "rtac_reserved_8",         5, 6, 7),
    alarm!(329, Rtac, "rtac_reserved_9",         5, 6, 8),
    alarm!(330, Rtac, "rtac_general_alarm",      4, 6, 9),

    // =========================================================================
    // FACP — Fire Alarm Control Panel (registers 7–8, alarms 401–425)
    // Register 7: alarms 401–416 (bits 0–15)
    // Register 8: alarms 417–425 (bits 0–8)
    // =========================================================================
    alarm!(401, Facp, "fire_alarm",              1, 7, 0),
    alarm!(402, Facp, "facp_trouble",            2, 7, 1),
    alarm!(403, Facp, "facp_supervisory",        1, 7, 2),
    alarm!(404, Facp, "flir_zone_1",             2, 7, 3),
    alarm!(405, Facp, "flir_zone_2",             1, 7, 4),
    alarm!(406, Facp, "flir_zone_3",             2, 7, 5),
    alarm!(407, Facp, "flir_zone_4",             5, 7, 6),
    alarm!(408, Facp, "flir_zone_5",             1, 7, 7),
    alarm!(409, Facp, "flir_zone_6",             2, 7, 8),
    alarm!(410, Facp, "flir_zone_7",             5, 7, 9),
    alarm!(411, Facp, "flir_zone_8",             1, 7, 10),
    alarm!(412, Facp, "flir_zone_9",             2, 7, 11),
    alarm!(413, Facp, "flir_zone_10",            1, 7, 12),
    alarm!(414, Facp, "suppression_zone_1",      2, 7, 13),
    alarm!(415, Facp, "suppression_zone_2",      5, 7, 14),
    alarm!(416, Facp, "suppression_zone_3",      1, 7, 15),
    alarm!(417, Facp, "suppression_zone_4",      1, 8, 0),
    alarm!(418, Facp, "suppression_zone_5",      2, 8, 1),
    alarm!(419, Facp, "alarm_mp_1a",             5, 8, 2),
    alarm!(420, Facp, "alarm_mp_1b",             1, 8, 3),
    alarm!(421, Facp, "alarm_mp_1c",             5, 8, 4),
    alarm!(422, Facp, "alarm_mp_2a",             5, 8, 5),
    alarm!(423, Facp, "alarm_mp_2b",             5, 8, 6),
    alarm!(424, Facp, "alarm_mp_2c",             5, 8, 7),
    alarm!(425, Facp, "facp_reserved",           2, 8, 8),

    // =========================================================================
    // Tesla Site Controller (register 9, alarms 501–510)
    // =========================================================================
    alarm!(501, TeslaSiteController, "battery_meter_invalid_1",       4, 9, 0),
    alarm!(502, TeslaSiteController, "battery_meter_mia_1",           4, 9, 1),
    alarm!(503, TeslaSiteController, "battery_meter_ct_pt_fail_1",    4, 9, 2),
    alarm!(504, TeslaSiteController, "battery_meter_out_of_bounds_1", 4, 9, 3),
    alarm!(505, TeslaSiteController, "battery_meter_unreasonable_1",  4, 9, 4),
    alarm!(506, TeslaSiteController, "battery_meter_invalid_2",       4, 9, 5),
    alarm!(507, TeslaSiteController, "battery_meter_mia_2",           4, 9, 6),
    alarm!(508, TeslaSiteController, "battery_meter_ct_pt_fail_2",    4, 9, 7),
    alarm!(509, TeslaSiteController, "battery_meter_out_of_bounds_2", 4, 9, 8),
    alarm!(510, TeslaSiteController, "battery_meter_unreasonable_2",  4, 9, 9),

    // =========================================================================
    // Megapack 1A (registers 10–11, alarms 601–630)
    // Register 10: alarms 601–616 (bits 0–15)
    // Register 11: alarms 617–630 (bits 0–13)
    // =========================================================================
    alarm!(601, Mp1a, "megapack_loss_of_comms",        3, 10, 0),
    alarm!(602, Mp1a, "megapack_iso_failure",          2, 10, 1),
    alarm!(603, Mp1a, "megapack_inverter_fault",       2, 10, 2),
    alarm!(604, Mp1a, "enable_circuit_open",           3, 10, 3),
    alarm!(605, Mp1a, "enable_switch_off",             3, 10, 4),
    alarm!(606, Mp1a, "door_switch_open",              4, 10, 5),
    alarm!(607, Mp1a, "ac_breaker_closed",             4, 10, 6),
    alarm!(608, Mp1a, "bus_controller_prolonged_fault", 2, 10, 7),
    alarm!(609, Mp1a, "remote_shutdown",               3, 10, 8),
    alarm!(610, Mp1a, "coolant_low",                   3, 10, 9),
    alarm!(611, Mp1a, "extreme_temp_warning",          3, 10, 10),
    alarm!(612, Mp1a, "extreme_temp_fault",            2, 10, 11),
    alarm!(613, Mp1a, "grid_uncompliant",              3, 10, 12),
    alarm!(614, Mp1a, "low_state_of_energy",           4, 10, 13),
    alarm!(615, Mp1a, "breaker_irrational",            3, 10, 14),
    alarm!(616, Mp1a, "breaker_ready_to_close",        5, 10, 15),
    alarm!(617, Mp1a, "megapack_disabled",             3, 11, 0),
    alarm!(618, Mp1a, "power_elec_over_temp",          2, 11, 1),
    alarm!(619, Mp1a, "sparker",                       2, 11, 2),
    alarm!(620, Mp1a, "cell_modem_en",                 5, 11, 3),
    alarm!(621, Mp1a, "mp_comms_warning",              4, 11, 4),
    alarm!(622, Mp1a, "di_spare_1",                    5, 11, 5),
    alarm!(623, Mp1a, "di_spare_2",                    5, 11, 6),
    alarm!(624, Mp1a, "di_spare_3",                    5, 11, 7),
    alarm!(625, Mp1a, "di_spare_4",                    5, 11, 8),
    alarm!(626, Mp1a, "di_spare_5",                    5, 11, 9),
    alarm!(627, Mp1a, "di_spare_6",                    5, 11, 10),
    alarm!(628, Mp1a, "di_spare_7",                    5, 11, 11),
    alarm!(629, Mp1a, "di_spare_8",                    5, 11, 12),
    alarm!(630, Mp1a, "di_spare_9",                    5, 11, 13),

    // =========================================================================
    // Megapack 1B (registers 12–13, alarms 631–660)
    // =========================================================================
    alarm!(631, Mp1b, "megapack_loss_of_comms",        3, 12, 0),
    alarm!(632, Mp1b, "megapack_iso_failure",          2, 12, 1),
    alarm!(633, Mp1b, "megapack_inverter_fault",       2, 12, 2),
    alarm!(634, Mp1b, "enable_circuit_open",           3, 12, 3),
    alarm!(635, Mp1b, "enable_switch_off",             3, 12, 4),
    alarm!(636, Mp1b, "door_switch_open",              4, 12, 5),
    alarm!(637, Mp1b, "ac_breaker_closed",             4, 12, 6),
    alarm!(638, Mp1b, "bus_controller_prolonged_fault", 2, 12, 7),
    alarm!(639, Mp1b, "remote_shutdown",               3, 12, 8),
    alarm!(640, Mp1b, "coolant_low",                   3, 12, 9),
    alarm!(641, Mp1b, "extreme_temp_warning",          3, 12, 10),
    alarm!(642, Mp1b, "extreme_temp_fault",            2, 12, 11),
    alarm!(643, Mp1b, "grid_uncompliant",              3, 12, 12),
    alarm!(644, Mp1b, "low_state_of_energy",           4, 12, 13),
    alarm!(645, Mp1b, "breaker_irrational",            3, 12, 14),
    alarm!(646, Mp1b, "breaker_ready_to_close",        5, 12, 15),
    alarm!(647, Mp1b, "megapack_disabled",             3, 13, 0),
    alarm!(648, Mp1b, "power_elec_over_temp",          2, 13, 1),
    alarm!(649, Mp1b, "sparker",                       2, 13, 2),
    alarm!(650, Mp1b, "cell_modem_en",                 5, 13, 3),
    alarm!(651, Mp1b, "mp_comms_warning",              4, 13, 4),
    alarm!(652, Mp1b, "di_spare_1",                    5, 13, 5),
    alarm!(653, Mp1b, "di_spare_2",                    5, 13, 6),
    alarm!(654, Mp1b, "di_spare_3",                    5, 13, 7),
    alarm!(655, Mp1b, "di_spare_4",                    5, 13, 8),
    alarm!(656, Mp1b, "di_spare_5",                    5, 13, 9),
    alarm!(657, Mp1b, "di_spare_6",                    5, 13, 10),
    alarm!(658, Mp1b, "di_spare_7",                    5, 13, 11),
    alarm!(659, Mp1b, "di_spare_8",                    5, 13, 12),
    alarm!(660, Mp1b, "di_spare_9",                    5, 13, 13),

    // =========================================================================
    // Megapack 1C (registers 14–15, alarms 661–690)
    // =========================================================================
    alarm!(661, Mp1c, "megapack_loss_of_comms",        3, 14, 0),
    alarm!(662, Mp1c, "megapack_iso_failure",          2, 14, 1),
    alarm!(663, Mp1c, "megapack_inverter_fault",       2, 14, 2),
    alarm!(664, Mp1c, "enable_circuit_open",           3, 14, 3),
    alarm!(665, Mp1c, "enable_switch_off",             3, 14, 4),
    alarm!(666, Mp1c, "door_switch_open",              4, 14, 5),
    alarm!(667, Mp1c, "ac_breaker_closed",             4, 14, 6),
    alarm!(668, Mp1c, "bus_controller_prolonged_fault", 2, 14, 7),
    alarm!(669, Mp1c, "remote_shutdown",               3, 14, 8),
    alarm!(670, Mp1c, "coolant_low",                   3, 14, 9),
    alarm!(671, Mp1c, "extreme_temp_warning",          3, 14, 10),
    alarm!(672, Mp1c, "extreme_temp_fault",            2, 14, 11),
    alarm!(673, Mp1c, "grid_uncompliant",              3, 14, 12),
    alarm!(674, Mp1c, "low_state_of_energy",           4, 14, 13),
    alarm!(675, Mp1c, "breaker_irrational",            3, 14, 14),
    alarm!(676, Mp1c, "breaker_ready_to_close",        5, 14, 15),
    alarm!(677, Mp1c, "megapack_disabled",             3, 15, 0),
    alarm!(678, Mp1c, "power_elec_over_temp",          2, 15, 1),
    alarm!(679, Mp1c, "sparker",                       2, 15, 2),
    alarm!(680, Mp1c, "cell_modem_en",                 5, 15, 3),
    alarm!(681, Mp1c, "mp_comms_warning",              4, 15, 4),
    alarm!(682, Mp1c, "di_spare_1",                    5, 15, 5),
    alarm!(683, Mp1c, "di_spare_2",                    5, 15, 6),
    alarm!(684, Mp1c, "di_spare_3",                    5, 15, 7),
    alarm!(685, Mp1c, "di_spare_4",                    5, 15, 8),
    alarm!(686, Mp1c, "di_spare_5",                    5, 15, 9),
    alarm!(687, Mp1c, "di_spare_6",                    5, 15, 10),
    alarm!(688, Mp1c, "di_spare_7",                    5, 15, 11),
    alarm!(689, Mp1c, "di_spare_8",                    5, 15, 12),
    alarm!(690, Mp1c, "di_spare_9",                    5, 15, 13),

    // =========================================================================
    // Megapack 2A (registers 16–17, alarms 691–720)
    // =========================================================================
    alarm!(691, Mp2a, "megapack_loss_of_comms",        3, 16, 0),
    alarm!(692, Mp2a, "megapack_iso_failure",          2, 16, 1),
    alarm!(693, Mp2a, "megapack_inverter_fault",       2, 16, 2),
    alarm!(694, Mp2a, "enable_circuit_open",           3, 16, 3),
    alarm!(695, Mp2a, "enable_switch_off",             3, 16, 4),
    alarm!(696, Mp2a, "door_switch_open",              4, 16, 5),
    alarm!(697, Mp2a, "ac_breaker_closed",             4, 16, 6),
    alarm!(698, Mp2a, "bus_controller_prolonged_fault", 2, 16, 7),
    alarm!(699, Mp2a, "remote_shutdown",               3, 16, 8),
    alarm!(700, Mp2a, "coolant_low",                   3, 16, 9),
    alarm!(701, Mp2a, "extreme_temp_warning",          3, 16, 10),
    alarm!(702, Mp2a, "extreme_temp_fault",            2, 16, 11),
    alarm!(703, Mp2a, "grid_uncompliant",              3, 16, 12),
    alarm!(704, Mp2a, "low_state_of_energy",           4, 16, 13),
    alarm!(705, Mp2a, "breaker_irrational",            3, 16, 14),
    alarm!(706, Mp2a, "breaker_ready_to_close",        5, 16, 15),
    alarm!(707, Mp2a, "megapack_disabled",             3, 17, 0),
    alarm!(708, Mp2a, "power_elec_over_temp",          2, 17, 1),
    alarm!(709, Mp2a, "sparker",                       2, 17, 2),
    alarm!(710, Mp2a, "cell_modem_en",                 5, 17, 3),
    alarm!(711, Mp2a, "mp_comms_warning",              4, 17, 4),
    alarm!(712, Mp2a, "di_spare_1",                    5, 17, 5),
    alarm!(713, Mp2a, "di_spare_2",                    5, 17, 6),
    alarm!(714, Mp2a, "di_spare_3",                    5, 17, 7),
    alarm!(715, Mp2a, "di_spare_4",                    5, 17, 8),
    alarm!(716, Mp2a, "di_spare_5",                    5, 17, 9),
    alarm!(717, Mp2a, "di_spare_6",                    5, 17, 10),
    alarm!(718, Mp2a, "di_spare_7",                    5, 17, 11),
    alarm!(719, Mp2a, "di_spare_8",                    5, 17, 12),
    alarm!(720, Mp2a, "di_spare_9",                    5, 17, 13),

    // =========================================================================
    // Megapack 2B (registers 18–19, alarms 721–750)
    // =========================================================================
    alarm!(721, Mp2b, "megapack_loss_of_comms",        3, 18, 0),
    alarm!(722, Mp2b, "megapack_iso_failure",          2, 18, 1),
    alarm!(723, Mp2b, "megapack_inverter_fault",       2, 18, 2),
    alarm!(724, Mp2b, "enable_circuit_open",           3, 18, 3),
    alarm!(725, Mp2b, "enable_switch_off",             3, 18, 4),
    alarm!(726, Mp2b, "door_switch_open",              4, 18, 5),
    alarm!(727, Mp2b, "ac_breaker_closed",             4, 18, 6),
    alarm!(728, Mp2b, "bus_controller_prolonged_fault", 2, 18, 7),
    alarm!(729, Mp2b, "remote_shutdown",               3, 18, 8),
    alarm!(730, Mp2b, "coolant_low",                   3, 18, 9),
    alarm!(731, Mp2b, "extreme_temp_warning",          3, 18, 10),
    alarm!(732, Mp2b, "extreme_temp_fault",            2, 18, 11),
    alarm!(733, Mp2b, "grid_uncompliant",              3, 18, 12),
    alarm!(734, Mp2b, "low_state_of_energy",           4, 18, 13),
    alarm!(735, Mp2b, "breaker_irrational",            3, 18, 14),
    alarm!(736, Mp2b, "breaker_ready_to_close",        5, 18, 15),
    alarm!(737, Mp2b, "megapack_disabled",             3, 19, 0),
    alarm!(738, Mp2b, "power_elec_over_temp",          2, 19, 1),
    alarm!(739, Mp2b, "sparker",                       2, 19, 2),
    alarm!(740, Mp2b, "cell_modem_en",                 5, 19, 3),
    alarm!(741, Mp2b, "mp_comms_warning",              4, 19, 4),
    alarm!(742, Mp2b, "di_spare_1",                    5, 19, 5),
    alarm!(743, Mp2b, "di_spare_2",                    5, 19, 6),
    alarm!(744, Mp2b, "di_spare_3",                    5, 19, 7),
    alarm!(745, Mp2b, "di_spare_4",                    5, 19, 8),
    alarm!(746, Mp2b, "di_spare_5",                    5, 19, 9),
    alarm!(747, Mp2b, "di_spare_6",                    5, 19, 10),
    alarm!(748, Mp2b, "di_spare_7",                    5, 19, 11),
    alarm!(749, Mp2b, "di_spare_8",                    5, 19, 12),
    alarm!(750, Mp2b, "di_spare_9",                    5, 19, 13),

    // =========================================================================
    // Megapack 2C (registers 20–21, alarms 751–780)
    // =========================================================================
    alarm!(751, Mp2c, "megapack_loss_of_comms",        3, 20, 0),
    alarm!(752, Mp2c, "megapack_iso_failure",          2, 20, 1),
    alarm!(753, Mp2c, "megapack_inverter_fault",       2, 20, 2),
    alarm!(754, Mp2c, "enable_circuit_open",           3, 20, 3),
    alarm!(755, Mp2c, "enable_switch_off",             3, 20, 4),
    alarm!(756, Mp2c, "door_switch_open",              4, 20, 5),
    alarm!(757, Mp2c, "ac_breaker_closed",             4, 20, 6),
    alarm!(758, Mp2c, "bus_controller_prolonged_fault", 2, 20, 7),
    alarm!(759, Mp2c, "remote_shutdown",               3, 20, 8),
    alarm!(760, Mp2c, "coolant_low",                   3, 20, 9),
    alarm!(761, Mp2c, "extreme_temp_warning",          3, 20, 10),
    alarm!(762, Mp2c, "extreme_temp_fault",            2, 20, 11),
    alarm!(763, Mp2c, "grid_uncompliant",              3, 20, 12),
    alarm!(764, Mp2c, "low_state_of_energy",           4, 20, 13),
    alarm!(765, Mp2c, "breaker_irrational",            3, 20, 14),
    alarm!(766, Mp2c, "breaker_ready_to_close",        5, 20, 15),
    alarm!(767, Mp2c, "megapack_disabled",             3, 21, 0),
    alarm!(768, Mp2c, "power_elec_over_temp",          2, 21, 1),
    alarm!(769, Mp2c, "sparker",                       2, 21, 2),
    alarm!(770, Mp2c, "cell_modem_en",                 5, 21, 3),
    alarm!(771, Mp2c, "mp_comms_warning",              4, 21, 4),
    alarm!(772, Mp2c, "di_spare_1",                    5, 21, 5),
    alarm!(773, Mp2c, "di_spare_2",                    5, 21, 6),
    alarm!(774, Mp2c, "di_spare_3",                    5, 21, 7),
    alarm!(775, Mp2c, "di_spare_4",                    5, 21, 8),
    alarm!(776, Mp2c, "di_spare_5",                    5, 21, 9),
    alarm!(777, Mp2c, "di_spare_6",                    5, 21, 10),
    alarm!(778, Mp2c, "di_spare_7",                    5, 21, 11),
    alarm!(779, Mp2c, "di_spare_8",                    5, 21, 12),
    alarm!(780, Mp2c, "di_spare_9",                    5, 21, 13),
];

/// Look up an alarm definition by alarm number
pub fn find_by_alarm_num(alarm_num: u16) -> Option<&'static AlarmDefinition> {
    ALARM_DEFINITIONS.iter().find(|d| d.alarm_num == alarm_num)
}

/// Get all alarm definitions for a specific zone
pub fn alarms_in_zone(zone: AlarmZone) -> impl Iterator<Item = &'static AlarmDefinition> {
    ALARM_DEFINITIONS.iter().filter(move |d| d.zone == zone)
}

/// Get all alarm definitions at or above a severity threshold (lower number = more severe)
pub fn alarms_at_level_or_above(max_level: u8) -> impl Iterator<Item = &'static AlarmDefinition> {
    ALARM_DEFINITIONS
        .iter()
        .filter(move |d| d.level <= max_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_definitions_have_valid_register_index() {
        for def in ALARM_DEFINITIONS {
            assert!(
                def.register_index < ALARM_REGISTER_COUNT,
                "Alarm {} ({}) has register_index {} >= ALARM_REGISTER_COUNT {}",
                def.alarm_num,
                def.name,
                def.register_index,
                ALARM_REGISTER_COUNT,
            );
            assert!(
                def.bit < 16,
                "Alarm {} ({}) has bit {} >= 16",
                def.alarm_num,
                def.name,
                def.bit,
            );
        }
    }

    #[test]
    fn test_no_duplicate_alarm_nums() {
        let mut seen = std::collections::HashSet::new();
        for def in ALARM_DEFINITIONS {
            assert!(
                seen.insert(def.alarm_num),
                "Duplicate alarm_num: {}",
                def.alarm_num,
            );
        }
    }

    #[test]
    fn test_no_duplicate_bit_positions() {
        let mut seen = std::collections::HashSet::new();
        for def in ALARM_DEFINITIONS {
            let key = (def.register_index, def.bit);
            assert!(
                seen.insert(key),
                "Duplicate bit position: alarm {} at register {} bit {}",
                def.alarm_num,
                def.register_index,
                def.bit,
            );
        }
    }

    #[test]
    fn test_all_levels_valid() {
        for def in ALARM_DEFINITIONS {
            assert!(
                (1..=5).contains(&def.level),
                "Alarm {} ({}) has invalid level {}",
                def.alarm_num,
                def.name,
                def.level,
            );
        }
    }

    #[test]
    fn test_estop_alarm_exists() {
        let estop = find_by_alarm_num(ESTOP_ALARM_NUM);
        assert!(estop.is_some());
        assert_eq!(estop.unwrap().name, "estop");
        assert_eq!(estop.unwrap().level, 2);
    }

    #[test]
    fn test_fire_alarm_exists() {
        let fire = find_by_alarm_num(FIRE_ALARM_NUM);
        assert!(fire.is_some());
        assert_eq!(fire.unwrap().name, "fire_alarm");
        assert_eq!(fire.unwrap().level, 1);
    }

    #[test]
    fn test_find_by_zone() {
        let site_alarms: Vec<_> = alarms_in_zone(AlarmZone::Site).collect();
        assert_eq!(site_alarms.len(), 7);
        assert_eq!(site_alarms[0].alarm_num, 1);
    }

    #[test]
    fn test_mp_alarm_names_available() {
        assert_eq!(MP_ALARM_NAMES.len(), 30);
        assert_eq!(MP_ALARM_LEVELS.len(), 30);
    }

    #[test]
    fn test_megapack_zones_each_have_30_alarms() {
        for zone in [
            AlarmZone::Mp1a,
            AlarmZone::Mp1b,
            AlarmZone::Mp1c,
            AlarmZone::Mp2a,
            AlarmZone::Mp2b,
            AlarmZone::Mp2c,
        ] {
            let count = alarms_in_zone(zone).count();
            assert_eq!(count, 30, "Zone {:?} has {} alarms, expected 30", zone, count);
        }
    }
}
