//! Alarm Definitions
//!
//! This module defines all digital alarm points for the site, derived
//! from the alarm register map spreadsheet. Each alarm has a unique number,
//! zone, name, severity level, and a specific bit position within the Modbus
//! alarm registers.
//!
//! ## Addressing: two different things, both called an address
//!
//! **The client's address** is the alarm number itself. Each digital point
//! lives at that address in the Modbus *discrete-input* space, reached with
//! [`AlarmDefinition::discrete_address`]. Analog points use the same numbers
//! in the *register* space (601 is a digital MP-1A status bit and an analog
//! MP-1A measurement); the two never collide, because Modbus addresses bits
//! and registers separately.
//!
//! **The packed-register layout below** is ours, not the client's. It predates
//! knowing how the RTAC addresses these points, and is what the simulator
//! serves and what [`AlarmDefinition::register_index`] and
//! [`AlarmDefinition::bit`] describe. Analog points have already moved onto
//! the client's numbering; the digital read path still uses this block, and
//! moving it over is a separate change that touches the E-stop and alarm
//! paths.
//!
//! ## Register Layout (simulator-side packed block)
//!
//! Alarm registers are read as a contiguous block of [`ALARM_REGISTER_COUNT`]
//! holding registers starting at address 8 (immediately after the 8 status
//! registers). Each register holds 16 alarm bits. Where each alarm sits in the
//! block is the site design's choice, recorded in each definition's
//! `register_index`/`bit`; Newtown's layout is documented in
//! [`crate::rtac::design::newtown::alarm_definitions`]. The block size itself
//! is still one constant shared by every design.
//!
//! ## Alarm Levels
//!
//! Every design uses this scale; the control logic and the API rely on it (1
//! is an emergency, 2 or below is critical). The descriptions come from the
//! Newtown alarm matrix:
//! - **1**: Emergency — active fire alarm, call 911, emergency shutdown
//! - **2**: High — activate COF, contact SMEs, immediate response
//! - **3**: Medium — inform management, contact SMEs
//! - **4**: Low — operator troubleshooting, escalate during business hours
//! - **5**: Informational / unclassified

use std::fmt;

use serde::{Deserialize, Serialize};

/// Number of 16-bit Modbus registers used for alarm flags
pub const ALARM_REGISTER_COUNT: usize = 22;

/// Alarm zones.
///
/// Still the closed set of Newtown's zones, shared by every design; a design
/// with different zones needs this to become per-design data.
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

impl AlarmZone {
    /// Every zone, so a caller that must cover all of them — a test pinning
    /// the wire format, a UI enumerating the site — has one list to read
    /// rather than its own copy to keep in step.
    pub const ALL: [Self; 14] = [
        Self::Site,
        Self::BreakerRelay,
        Self::Meter,
        Self::Transformer1,
        Self::Transformer2,
        Self::Rtac,
        Self::Facp,
        Self::TeslaSiteController,
        Self::Mp1a,
        Self::Mp1b,
        Self::Mp1c,
        Self::Mp2a,
        Self::Mp2b,
        Self::Mp2c,
    ];

    /// The zone's stable identifier, as used in persisted reading JSON and in
    /// API responses.
    ///
    /// This is deliberately an explicit match rather than `format!("{:?}")`.
    /// `Debug` carries no stability contract — it exists for programmers, and
    /// a rename or a `#[derive]` change could silently rewrite every key we
    /// have written to disk and every key a client matches on. These strings
    /// are a format, so they are spelled out. `matches_serde_representation`
    /// pins them to the DTO the frontend consumes.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Site => "Site",
            Self::BreakerRelay => "BreakerRelay",
            Self::Meter => "Meter",
            Self::Transformer1 => "Transformer1",
            Self::Transformer2 => "Transformer2",
            Self::Rtac => "Rtac",
            Self::Facp => "Facp",
            Self::TeslaSiteController => "TeslaSiteController",
            Self::Mp1a => "Mp1a",
            Self::Mp1b => "Mp1b",
            Self::Mp1c => "Mp1c",
            Self::Mp2a => "Mp2a",
            Self::Mp2b => "Mp2b",
            Self::Mp2c => "Mp2c",
        }
    }
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
    /// Unique alarm number from the site's alarm matrix
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

    /// The alarm's Modbus discrete-input address on the RTAC.
    ///
    /// The client's alarm number *is* the address. It shares its number with
    /// an analog point of the same zone — digital 601 is MP-1A
    /// `megapack_loss_of_comms`, analog 601 is MP-1A `real_power_target` —
    /// and the two do not collide because Modbus addresses bits and registers
    /// in separate spaces.
    ///
    /// Deliberately a method over `alarm_num` rather than a field: storing the
    /// address alongside the number it is equal to would give the pair room to
    /// disagree, and a definition whose address had drifted from its number
    /// would read a neighbouring point while looking correct in every listing.
    ///
    /// Note this is *not* the address [`register_index`]/[`bit`] describe.
    /// Those two locate the alarm within the packed holding-register block the
    /// simulator serves, which is our own framing and predates knowing the
    /// client's addressing; see the module docs.
    ///
    /// [`register_index`]: AlarmDefinition::register_index
    /// [`bit`]: AlarmDefinition::bit
    pub const fn discrete_address(&self) -> u16 {
        super::protocol::RegisterMap::point_address(self.alarm_num)
    }
}

/// Every alarm the active site design defines, in alarm-number order.
pub fn alarm_definitions() -> &'static [AlarmDefinition] {
    super::design::active().alarm_definitions
}

/// Look up an alarm definition by alarm number
pub fn find_by_alarm_num(alarm_num: u16) -> Option<&'static AlarmDefinition> {
    alarm_definitions().iter().find(|d| d.alarm_num == alarm_num)
}

/// Get all alarm definitions for a specific zone
pub fn alarms_in_zone(zone: AlarmZone) -> impl Iterator<Item = &'static AlarmDefinition> {
    alarm_definitions().iter().filter(move |d| d.zone == zone)
}

/// Get all alarm definitions at or above a severity threshold (lower number =
/// more severe)
pub fn alarms_at_level_or_above(max_level: u8) -> impl Iterator<Item = &'static AlarmDefinition> {
    alarm_definitions().iter().filter(move |d| d.level <= max_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every zone's `code()` must equal how serde serializes it, because the
    /// two are the same format seen from different sides: `code()` writes the
    /// keys into stored readings, and serde carries the zone to anything
    /// reading them back.
    ///
    /// The other half of that promise — that neems-api's `AlarmZoneDto`
    /// serializes the same way, so the frontend sees one zone spelling for
    /// alarms and analogs alike — cannot be checked from here, because
    /// neems-data does not know about the DTO. It is pinned by
    /// `alarm_zone_dto_matches_zone_code` in neems-api.
    #[test]
    fn zone_code_matches_serde_representation() {
        for zone in AlarmZone::ALL {
            let serialized = serde_json::to_string(&zone).expect("zone serializes");
            assert_eq!(serialized, format!("\"{}\"", zone.code()), "{zone:?}");
        }
    }
}
