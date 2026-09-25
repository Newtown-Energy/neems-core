//! The Newtown site design.
//!
//! Its alarm matrix, SLD metadata and analog points come from the client's
//! workbook via `docs/alarms/newtown-alarms.json`; see `docs/alarms/README.md`
//! for how the generated tables are rebuilt.

use super::SiteDesign;

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
};
