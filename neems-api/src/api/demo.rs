//! Demo-only API endpoints.
//!
//! These exist to make a hardware-free demo look real: rather than the
//! frontend writing a pile of fake data, it asks the backend to generate
//! plausible multi-day SoC and alarm history server-side (reusing the same
//! generators as the `neems-data seed-*-history` CLI commands).
//!
//! Gated to the same roles as the Demo Controls drawer. Meant to be deleted
//! once the real RTAC feed is the source of truth.

use neems_data::{SeedOutcome, seed_alarm_history, seed_soc_history};
use rocket::{Route, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{orm::neems_data::db::SiteDbConn, session_guards::AuthenticatedUser};

/// Roles allowed to drive demo controls — mirrors the frontend drawer's gate
/// and the forced-alarm endpoints in [`crate::api::alarm`].
const DEMO_CONTROL_ROLES: &[&str] = &["admin", "newtown-admin", "newtown-staff"];

/// Default days of history to backfill when the request omits it.
const DEFAULT_DAYS: u32 = 14;
/// Cap so a stray request can't try to generate an unbounded amount of data.
const MAX_DAYS: u32 = 90;
/// Sample cadence; matches the RTAC collector / seeder default of 6 minutes.
const INTERVAL_MINUTES: u32 = 6;

/// Body for `POST /1/Demo/InjectHistory`.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct InjectHistoryRequest {
    pub site_id: i32,
    /// Days of history to backfill. Defaults to 14, clamped to 1..=90.
    #[serde(default)]
    pub days: Option<u32>,
}

/// Per-source result of a seed run.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SeedSummary {
    pub source_name: String,
    /// New readings written this run.
    pub written: u32,
    /// Slots skipped because a reading already existed there.
    pub already_present: u32,
    /// Total slots spanned by the window.
    pub total_slots: u32,
}

impl From<SeedOutcome> for SeedSummary {
    fn from(o: SeedOutcome) -> Self {
        SeedSummary {
            source_name: o.source_name,
            written: o.written as u32,
            already_present: o.already_present as u32,
            total_slots: o.total_slots as u32,
        }
    }
}

/// Response for `POST /1/Demo/InjectHistory`.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct InjectHistoryResponse {
    pub site_id: i32,
    pub days: u32,
    pub soc: SeedSummary,
    pub alarms: SeedSummary,
}

fn forbid_unless_demo_role(user: &AuthenticatedUser) -> Result<(), Status> {
    if user.has_any_role(DEMO_CONTROL_ROLES) {
        Ok(())
    } else {
        Err(Status::Forbidden)
    }
}

/// Inject simulated SoC + alarm history for a site.
///
/// - **URL:** `/api/1/Demo/InjectHistory`
/// - **Method:** `POST`
/// - **Body:** `{ "site_id": i32, "days"?: u32 }`
/// - **Authentication:** Required; one of `admin`, `newtown-admin`,
///   `newtown-staff`.
///
/// Backfills the last `days` (default 14) of plausible SoC and alarm readings
/// for the site at a 6-minute cadence. Idempotent: re-running only fills slots
/// that aren't already present, so it's safe to call repeatedly (e.g. to keep
/// the trailing window fresh as days pass).
#[post("/1/Demo/InjectHistory", data = "<body>")]
pub async fn inject_history(
    user: AuthenticatedUser,
    site_db: SiteDbConn,
    body: Json<InjectHistoryRequest>,
) -> Result<Json<InjectHistoryResponse>, Status> {
    forbid_unless_demo_role(&user)?;

    let site_id = body.site_id;
    let days = body.days.unwrap_or(DEFAULT_DAYS).clamp(1, MAX_DAYS);

    let response = site_db
        .run(move |conn| {
            let soc = seed_soc_history(conn, site_id, days, INTERVAL_MINUTES).map_err(|e| {
                eprintln!("Demo inject: SoC seeding failed for site {site_id}: {e}");
                Status::InternalServerError
            })?;
            let alarms =
                seed_alarm_history(conn, site_id, days, INTERVAL_MINUTES).map_err(|e| {
                    eprintln!("Demo inject: alarm seeding failed for site {site_id}: {e}");
                    Status::InternalServerError
                })?;
            Ok::<InjectHistoryResponse, Status>(InjectHistoryResponse {
                site_id,
                days,
                soc: soc.into(),
                alarms: alarms.into(),
            })
        })
        .await?;

    Ok(Json(response))
}

pub fn routes() -> Vec<Route> {
    routes![inject_history]
}
