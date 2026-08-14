//! Demo-only API endpoints.
//!
//! These exist to make a hardware-free demo look real: rather than the
//! frontend writing a pile of fake data, it asks the backend to generate
//! plausible multi-day SoC and alarm history server-side (reusing the same
//! generators as the `neems-data seed-*-history` CLI commands).
//!
//! Gated to the same roles as the Demo Controls drawer. Meant to be deleted
//! once the real RTAC feed is the source of truth.

use neems_data::{
    SeedOutcome, get_all_alarm_state, record_alarm_snapshot,
    rtac::alarm_definitions::ALARM_DEFINITIONS, seed_alarm_history, seed_soc_history,
    upsert_alarm_transition,
};
use rocket::{Route, State, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{orm::neems_data::db::SiteDbConn, session_guards::AuthenticatedUser};

/// Roles allowed to drive demo controls — mirrors the frontend drawer's gate
/// and the forced-alarm endpoints in [`crate::api::alarm`].
const DEMO_CONTROL_ROLES: &[&str] = &["admin", "newtown-admin", "newtown-staff"];

/// Whether demo-only endpoints are live, from `NEEMS_DEMO_MODE` at startup.
///
/// Off unless the variable is set to a truthy value, so a production deploy
/// that simply doesn't set it cannot be driven by the demo write endpoints.
/// Managed by Rocket (see `mount_api_routes`) and read by the demo guards.
pub struct DemoMode(pub bool);

impl DemoMode {
    /// Resolve from the Rocket config key `demo_mode` if present, else from
    /// the `NEEMS_DEMO_MODE` environment variable.
    ///
    /// The config key exists so tests can build a Rocket with demo mode on or
    /// off explicitly; deployments set the environment variable, matching the
    /// other `NEEMS_*` settings.
    pub fn resolve(figment: &rocket::figment::Figment) -> Self {
        match figment.extract_inner::<bool>("demo_mode") {
            Ok(enabled) => Self(enabled),
            Err(_) => Self::from_env(),
        }
    }

    /// Read `NEEMS_DEMO_MODE`. Accepts `1`, `true`, `yes`, `on`
    /// (case-insensitive); anything else — including unset — is off.
    pub fn from_env() -> Self {
        let enabled = std::env::var("NEEMS_DEMO_MODE")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        Self(enabled)
    }

    pub fn enabled(&self) -> bool {
        self.0
    }
}

/// Reject unless demo mode is on.
///
/// Answers 404 rather than 403 so a non-demo deployment doesn't advertise that
/// these routes exist at all. The role check is applied separately and still
/// runs on demo deployments.
pub fn forbid_unless_demo_mode(demo: &DemoMode) -> Result<(), Status> {
    if demo.enabled() {
        Ok(())
    } else {
        Err(Status::NotFound)
    }
}

/// Site the demo alarm endpoints write readings for.
///
/// The alarm read path (`/Alarms/Active`, E-stop, history) already treats the
/// site database as single-site and never filters by site, so the writes match
/// that assumption rather than inventing a per-site parameter the readers would
/// ignore. Revisit when the deployment becomes multi-site.
pub(crate) const DEMO_SITE_ID: i32 = 1;

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

/// Body for `POST /1/Demo/AlarmState`.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct SetAlarmStateRequest {
    pub alarm_num: u16,
    /// `true` raises the alarm, `false` returns it to normal.
    pub active: bool,
}

/// One alarm's demo-visible data state.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DemoAlarmStateDto {
    pub alarm_num: u16,
    /// Whether the alarm condition is currently present.
    pub active: bool,
    /// ISO 8601 timestamp of the most recent false->true edge, if any.
    pub last_rising_at: Option<String>,
    /// ISO 8601 timestamp of the most recent true->false edge, if any.
    pub last_falling_at: Option<String>,
}

/// Response for the `/1/Demo/AlarmState` endpoints.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DemoAlarmStateResponse {
    /// Every alarm that has ever transitioned, newest state included.
    pub alarms: Vec<DemoAlarmStateDto>,
    /// Just the alarm numbers currently active — convenient for the drawer.
    pub active_alarm_nums: Vec<u16>,
}

fn build_state_response(rows: &[neems_data::models::AlarmStateRow]) -> DemoAlarmStateResponse {
    let fmt = |t: chrono::NaiveDateTime| t.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut alarms: Vec<DemoAlarmStateDto> = rows
        .iter()
        .filter_map(|r| {
            let alarm_num = u16::try_from(r.alarm_num).ok()?;
            Some(DemoAlarmStateDto {
                alarm_num,
                active: r.data_active,
                last_rising_at: r.last_rising_at.map(fmt),
                last_falling_at: r.last_falling_at.map(fmt),
            })
        })
        .collect();
    alarms.sort_by_key(|a| a.alarm_num);
    let active_alarm_nums = alarms.iter().filter(|a| a.active).map(|a| a.alarm_num).collect();
    DemoAlarmStateResponse { alarms, active_alarm_nums }
}

/// Read the current demo alarm data-state.
///
/// - **URL:** `/api/1/Demo/AlarmState`
/// - **Method:** `GET`
/// - **Authentication:** Required; one of `admin`, `newtown-admin`,
///   `newtown-staff`. Demo mode must be enabled.
#[get("/1/Demo/AlarmState")]
pub async fn get_alarm_state(
    user: AuthenticatedUser,
    demo: &State<DemoMode>,
    site_db: SiteDbConn,
) -> Result<Json<DemoAlarmStateResponse>, Status> {
    forbid_unless_demo_mode(demo)?;
    forbid_unless_demo_role(&user)?;

    let rows = site_db
        .run(move |conn| get_all_alarm_state(conn).map_err(|_| Status::InternalServerError))
        .await?;

    Ok(Json(build_state_response(&rows)))
}

/// Activate or deactivate a single alarm.
///
/// - **URL:** `/api/1/Demo/AlarmState`
/// - **Method:** `POST`
/// - **Body:** `{ "alarm_num": u16, "active": bool }`
/// - **Authentication:** Required; one of `admin`, `newtown-admin`,
///   `newtown-staff`. Demo mode must be enabled.
///
/// Writes a real data-state transition through the same
/// [`upsert_alarm_transition`] path the RTAC collector uses, so a demo alarm
/// latches exactly like a real one: raising it stamps `last_rising_at`,
/// returning it to normal stamps `last_falling_at` and leaves the alarm
/// visible as `ReturnedUnacknowledged` until it is acknowledged again.
///
/// Also appends a reading carrying the full alarm bitfield, so the change shows
/// up as an `Activated`/`Cleared` entry in `GET /Alarms/History` (and the FDNY
/// timeline built on it) the same way a real RTAC reading would.
///
/// Idempotent in effect but not in timestamps — posting the same `active`
/// value twice re-stamps that edge.
#[post("/1/Demo/AlarmState", data = "<body>")]
pub async fn set_alarm_state(
    user: AuthenticatedUser,
    demo: &State<DemoMode>,
    site_db: SiteDbConn,
    body: Json<SetAlarmStateRequest>,
) -> Result<Json<DemoAlarmStateResponse>, Status> {
    forbid_unless_demo_mode(demo)?;
    forbid_unless_demo_role(&user)?;

    let alarm_num = body.alarm_num;
    if !ALARM_DEFINITIONS.iter().any(|d| d.alarm_num == alarm_num) {
        return Err(Status::BadRequest);
    }
    let active = body.active;

    let rows = site_db
        .run(move |conn| {
            let now = chrono::Utc::now().naive_utc();
            upsert_alarm_transition(conn, alarm_num as i32, active, now)
                .map_err(|_| Status::InternalServerError)?;
            let rows = get_all_alarm_state(conn).map_err(|_| Status::InternalServerError)?;
            record_snapshot(conn, &rows, now)?;
            Ok::<_, Status>(rows)
        })
        .await?;

    Ok(Json(build_state_response(&rows)))
}

/// Append a reading for the full current alarm set, so the change lands in
/// `/Alarms/History` as a real transition.
///
/// Snapshots every active alarm rather than just the one that changed: history
/// is derived by diffing consecutive bitfields, so a partial bitfield would
/// read as every other alarm clearing at once.
fn record_snapshot(
    conn: &mut diesel::SqliteConnection,
    rows: &[neems_data::models::AlarmStateRow],
    at: chrono::NaiveDateTime,
) -> Result<(), Status> {
    let active_nums: std::collections::HashSet<u16> = rows
        .iter()
        .filter(|r| r.data_active)
        .filter_map(|r| u16::try_from(r.alarm_num).ok())
        .collect();
    record_alarm_snapshot(conn, DEMO_SITE_ID, &active_nums, at).map_err(|e| {
        eprintln!("Demo alarm snapshot write failed: {e}");
        Status::InternalServerError
    })
}

pub fn routes() -> Vec<Route> {
    routes![inject_history, get_alarm_state, set_alarm_state]
}
