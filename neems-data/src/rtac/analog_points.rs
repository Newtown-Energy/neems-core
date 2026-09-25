//! Analog point registry — every measurement the client's `Analogs` sheet
//! defines, with the Modbus address each is read from.
//!
//! The tables themselves are per site design, generated into each design's
//! module (for Newtown, [`crate::rtac::design::newtown::analog_points`]); this
//! module holds the shape and the lookups, which read the active design.
//!
//! ## The point number is the address
//!
//! `point_number` is the spreadsheet's "alarm number" for the row, and it *is*
//! the point's Modbus register address (via
//! [`RegisterMap::point_address`][crate::rtac::protocol::RegisterMap::point_address],
//! which carries the one assumption about numbering base). A digital alarm of
//! the same zone shares the number — 601 is both MP-1A `real_power_target`
//! here and MP-1A `megapack_loss_of_comms` in
//! the design's alarm definitions —
//! and they do not collide because Modbus addresses registers and bits
//! separately.
//!
//! ## Read-only floats, of unsettled width
//!
//! The client states every point on the `Analogs` sheet is a float and every
//! one is read-only; nothing here is ever written. That does not settle the
//! wire encoding: a 32-bit float needs two registers, but the sheet numbers
//! points one apart, so either the values are 16-bit and scaled or these
//! numbers are indexes rather than addresses. The spec flags the contradiction
//! in `data_quality_issues`; until it is resolved, `point_number` is an
//! address only under the first reading.
//!
//! ## No units, no scaling
//!
//! The spreadsheet gives neither for any row, so neither appears here. Only
//! the three points the SLD renders are decoded, and their scaling lives in
//! `protocol.rs` where the assumption is documented; every other point is
//! carried through as a raw register value rather than given an encoding we
//! would have to unpick later.

use super::{alarm_definitions::AlarmZone, design};

/// One analog measurement from the client's `Analogs` sheet.
pub struct AnalogPoint {
    /// The spreadsheet's number for this row, which is also its Modbus
    /// register address. Unique among analog points, but shared with the
    /// digital alarm of the same number.
    pub point_number: u16,
    /// Zone the measurement belongs to.
    pub zone: AlarmZone,
    /// Name as written in the spreadsheet.
    pub name: &'static str,
    /// Position within the owning Megapack's 30-point block, or `None` for the
    /// two site-level transformer points that belong to no block.
    pub offset: Option<u16>,
    /// True for the `AI_spare_*` rows the client reserved but left undefined.
    pub spare: bool,
    /// The spreadsheet's "IsFire?" flag.
    pub is_fire: bool,
}

/// Look up an analog point by its number (equivalently, its address).
pub fn analog_point(point_number: u16) -> Option<&'static AnalogPoint> {
    analog_points().iter().find(|p| p.point_number == point_number)
}

/// Look up a Megapack point's name by its offset within a pack block.
pub fn megapack_point_name(offset: u16) -> Option<&'static str> {
    design::active().megapack_analog_names.get(offset as usize).copied()
}

/// Every analog point the active site design defines, in point-number order.
pub fn analog_points() -> &'static [AnalogPoint] {
    design::active().analog_points
}
