//! Which site design this deployment runs.
//!
//! The design is chosen by `NEEMS_SITE_DESIGN` when the process starts (see
//! [`neems_data::rtac::design`]). Clients read it here rather than being
//! configured separately, so a deployment has one setting to get right instead
//! of two that can disagree.

use neems_data::rtac::design;
use rocket::{Route, serde::json::Json};
use serde::Serialize;
use ts_rs::TS;

/// The site design this deployment runs.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct SiteDesignDto {
    /// Stable design identifier, e.g. `"newtown"`. The client keys its diagram
    /// and alarm presentation on this.
    pub id: String,
    /// The alarm that reports the site's physical E-stop. The client's design
    /// carries its own copy, to decide the diagram's lockout without a round
    /// trip; this lets it check that copy against the backend's instead of
    /// trusting two constants to stay in step.
    pub estop_alarm_num: u16,
}

/// Site design endpoint.
///
/// - **URL:** `/api/1/SiteDesign`
/// - **Method:** `GET`
/// - **Purpose:** Returns the id of the site design this deployment runs
/// - **Authentication:** None required. The id is deployment configuration, not
///   site data, and a client may need it before anyone has logged in.
#[rocket::get("/1/SiteDesign")]
pub fn get_site_design() -> Json<SiteDesignDto> {
    let design = design::active();
    Json(SiteDesignDto {
        id: design.id.to_string(),
        estop_alarm_num: design.estop_alarm_num,
    })
}

pub fn routes() -> Vec<Route> {
    routes![get_site_design]
}
