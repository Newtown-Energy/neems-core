//! The second site design: a PV-coupled site with two inverter blocks, each
//! feeding a DC bus of DC/DC converters and battery units.
//!
//! Only its single line diagram exists so far (drawn by the frontend's
//! `site-2` design). Its alarm matrix and analog points have not been
//! specified, so every table here but the controls is **borrowed from
//! [`newtown`](super::newtown)**: that keeps the collector, the simulator and
//! seeded history running under this design, at the cost of reporting
//! Newtown's alarms against this site's equipment. Replace the borrowed tables
//! with this site's own when its workbook lands; the E-stop number and the
//! seeded cycles go with them.

use super::{SiteDesign, newtown};

pub mod site_controls;

/// The second site design.
pub const DESIGN: SiteDesign = SiteDesign {
    id: "site-2",
    site_controls: site_controls::SITE_CONTROLS,
    ..newtown::DESIGN
};

#[cfg(test)]
mod tests {
    use super::super::by_id;

    /// The id is what a deployment sets and what the frontend keys its diagram
    /// on, and the control ids are the frontend's component ids. Nothing else
    /// would notice either drifting: the invariants in the parent module only
    /// check the designs that are registered, under whatever ids they have.
    #[test]
    fn is_registered_under_the_id_and_controls_the_frontend_draws() {
        let design = by_id("site-2").expect("site-2 is registered");
        let ids: Vec<_> = design.site_controls.iter().map(|c| c.id).collect();
        assert_eq!(ids, ["switch-mvsw-1", "switch-mvsw-2"]);
    }
}
