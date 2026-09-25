//! The Newtown site design.
//!
//! Its alarm matrix, SLD metadata and analog points come from the client's
//! workbook via `docs/alarms/newtown-alarms.json`; see `docs/alarms/README.md`
//! for how the generated tables are rebuilt.

use super::{SeededAlarmCycle, SiteDesign};

pub mod alarm_definitions;
pub mod alarm_sld_meta;
pub mod analog_points;
pub mod site_controls;

/// The Newtown design.
pub const DESIGN: SiteDesign = SiteDesign {
    id: "newtown",
    alarm_definitions: alarm_definitions::ALARM_DEFINITIONS,
    alarm_sld_meta: alarm_sld_meta::ALARM_SLD_META,
    analog_points: analog_points::ANALOG_POINTS,
    megapack_analog_names: &analog_points::MEGAPACK_ANALOG_NAMES,
    site_controls: site_controls::SITE_CONTROLS,
    estop_alarm_num: alarm_definitions::ESTOP_ALARM_NUM,
    seeded_alarm_cycles: SEEDED_ALARM_CYCLES,
};

/// The chosen alarms span several zones and severities so the FDNY timeline
/// has variety, and the long periods keep transitions sparse (a handful per
/// alarm per week) rather than flapping every sample.
const SEEDED_ALARM_CYCLES: &[SeededAlarmCycle] = &[
    // loss_fiber (L3) — ~daily, 90 min
    cycle(1, 1440, 90, 0),
    // meter_loss_of_comms (L5) — every 2 days, 3 h
    cycle(203, 2880, 180, 600),
    // t1_temp_alarm (L4) — twice daily, 1 h
    cycle(301, 720, 60, 200),
    // estop (L2, critical) — every 3 days, 4 h
    cycle(alarm_definitions::ESTOP_ALARM_NUM, 4320, 240, 1000),
    // intruder_detected (L5) — every 4 days, 30 min
    cycle(7, 5760, 30, 2500),
];

const fn cycle(
    alarm_num: u16,
    period_minutes: i64,
    active_minutes: i64,
    phase_minutes: i64,
) -> SeededAlarmCycle {
    SeededAlarmCycle {
        alarm_num,
        period_minutes,
        active_minutes,
        phase_minutes,
    }
}
