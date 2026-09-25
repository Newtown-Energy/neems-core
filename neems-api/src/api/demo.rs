//! Demo-only API endpoints.
//!
//! These exist to make a hardware-free demo look real: rather than the
//! frontend writing a pile of fake data, it asks the backend to generate
//! plausible multi-day SoC and alarm history server-side (reusing the same
//! generators as the `neems-data seed-*-history` CLI commands).
//!
//! Gated to the same roles as the Demo Controls drawer. Meant to be deleted
//! once the real RTAC feed is the source of truth.

use std::collections::HashSet;

use diesel::prelude::*;
use neems_data::{
    SeedOutcome, get_all_alarm_state, record_alarm_snapshot,
    rtac::{
        alarm_definitions::{ALARM_DEFINITIONS, ESTOP_ALARM_NUM},
        site_controls::{SiteControl, SiteControlAction},
        state::AlarmFlags,
    },
    seed_alarm_history, seed_soc_history, upsert_alarm_transition,
};
use rocket::{Route, State, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    api::alarm::parse_alarm_registers, orm::neems_data::db::SiteDbConn,
    session_guards::AuthenticatedUser,
};

/// Anything that went wrong writing demo state to the site database.
type SiteWriteResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Roles allowed to drive demo controls — mirrors the frontend drawer's gate.
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
/// The alarm read path (`/Alarms/Active`, emergency shutdown status, history)
/// already treats the site database as single-site and never filters by site,
/// so the writes match that assumption rather than inventing a per-site
/// parameter the readers would ignore. Revisit when the deployment becomes
/// multi-site.
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
            write_site_alarm(conn, alarm_num, active).map_err(|e| {
                eprintln!("Demo alarm state write failed for {alarm_num}: {e}");
                Status::InternalServerError
            })?;
            get_all_alarm_state(conn).map_err(|_| Status::InternalServerError)
        })
        .await?;

    Ok(Json(build_state_response(&rows)))
}

/// Carry out a control request the way the site would, for a deployment that
/// has no site.
///
/// Demo mode's premise is that there is no RTAC and no collector: `neems-data`
/// is not running (see `.do/app.yaml`), and no control has a write register
/// anyway while the client's `Outputs` sheet is empty. Left alone, every click
/// on the diagram fails with "this control has no RTAC point configured yet" —
/// the truth on a real deployment, and useless on a demo whose whole subject is
/// the controls.
///
/// So stand in for the collector and move the control's readback point to the
/// position the action implies, through the same `alarm_state` + snapshot path
/// the RTAC collector writes and [`set_alarm_state`] already uses. The caller
/// reports the request `sent` once this returns, which is what a collector does
/// after a successful write.
///
/// Note what this writes: the **readback**, not the request. The two axes stay
/// separate here as everywhere else — a demo breaker moves because the site
/// says it moved, not because somebody clicked. That is also what makes the
/// demo worth taking screenshots of: the diagram is reading the same points it
/// would read against real hardware.
///
/// **Not site-scoped, because nothing here is.** `alarm_state` is keyed on
/// `alarm_num` alone and the alarm read path never filters by site, so a
/// request naming site 2 moves the same readback a request naming site 1 does.
/// Carrying the site id through this write would look like a fix and not be
/// one: the readers would go on ignoring it. Site-scoping alarms is #98, and it
/// is the only thing that would change this. Demo mode does not produce a
/// second site regardless — `demo_seed_fairing` creates one only when the
/// deployment has none.
pub(crate) fn apply_control_readback(
    conn: &mut diesel::SqliteConnection,
    control: &SiteControl,
    action: SiteControlAction,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(readback) = control.readback else {
        // Nothing reports this control's position, so there is nothing to move
        // and that is not a failure: the signal is what was asked for, and a
        // control with no feedback point was never going to confirm it either.
        return Ok(());
    };

    write_site_alarm(conn, readback.alarm_num, readback.bit_for(action.resulting_position()))
}

/// Carry out an emergency shutdown request for the demo: raise alarm 104.
///
/// Demo-only. A real site may act on the request without raising 104, and
/// nothing outside the demo path assumes it does.
///
/// The emergency shutdown's counterpart to [`apply_control_readback`], for the
/// same reason — a demo has no collector, so a trip request otherwise waits for
/// one and fails after a minute, telling the audience the site was never asked.
/// Raising 104 is what a real RTAC does when it trips, so
/// `/EmergencyShutdown`'s `observed_active` and the diagram's lockout follow
/// from the alarm feed exactly as they would against hardware.
///
/// Engage-only, like the real thing. There is still no way to clear a trip
/// through a request; on a demo, the "panel on site" is `POST
/// /1/Demo/AlarmState` lowering 104.
pub(crate) fn apply_estop_trip(conn: &mut diesel::SqliteConnection) -> SiteWriteResult {
    write_site_alarm(conn, ESTOP_ALARM_NUM, true)
}

/// Set one alarm's data state as the site would report it: the transition, and
/// the reading that carries it, in one transaction.
///
/// Every live demo alarm change goes through here — alarms driven from the
/// drawer (`POST /1/Demo/AlarmState`), control readbacks, E-stop trips — so
/// they share one definition of what a site-side change is. Seeded history
/// (`POST /1/Demo/InjectHistory`) is the one demo write that does not: it
/// backfills a window of readings in bulk rather than changing one alarm now.
///
/// One transaction, because half of this applied is worse than none of it: the
/// alarm would have moved while the caller, seeing the error, reports the
/// request failed — leaving a diagram that shows the change beside a badge
/// saying the signal never got out.
fn write_site_alarm(
    conn: &mut diesel::SqliteConnection,
    alarm_num: u16,
    active: bool,
) -> SiteWriteResult {
    let now = chrono::Utc::now().naive_utc();
    conn.immediate_transaction(|conn| {
        upsert_alarm_transition(conn, alarm_num as i32, active, now)?;
        record_snapshot(conn, now)
    })
}

/// Append a reading for the full current alarm set, so the change lands in
/// `/Alarms/History` as a real transition.
///
/// Snapshots every active alarm rather than just the one that changed: history
/// is derived by diffing consecutive bitfields, so a partial bitfield would
/// read as every other alarm clearing at once.
fn record_snapshot(
    conn: &mut diesel::SqliteConnection,
    at: chrono::NaiveDateTime,
) -> SiteWriteResult {
    // Onto the demo site's timeline like every other demo write — see
    // [`DEMO_SITE_ID`] for why the demo keeps one.
    let active = current_active_alarms(conn)?;
    record_alarm_snapshot(conn, DEMO_SITE_ID, &active, at)
}

/// Every alarm the site is currently reporting.
///
/// The newest reading's bitfield, with `alarm_state` laid over it, and both
/// halves are load-bearing because the demo has two writers that do not know
/// about each other:
///
/// - `POST /1/Demo/InjectHistory` seeds *readings* and never touches
///   `alarm_state`, so the alarms it raises exist only in a bitfield.
/// - `POST /1/Demo/AlarmState` and [`apply_control_readback`] write
///   `alarm_state`, and reach a bitfield only through a snapshot like this one.
///
/// Build a snapshot from either source alone and it silently drops the other's
/// alarms. That is not a cosmetic loss: `/Alarms/History` is derived by diffing
/// consecutive bitfields, so a snapshot missing the seeded alarms reads as the
/// whole site returning to normal at the instant somebody clicked a breaker.
///
/// `alarm_state` wins where the two disagree, because it is the record of a
/// deliberate demo action — an alarm explicitly cleared has to clear, even
/// though the seeded reading underneath it still says otherwise.
fn current_active_alarms(
    conn: &mut diesel::SqliteConnection,
) -> Result<HashSet<u16>, Box<dyn std::error::Error + Send + Sync>> {
    let mut active = latest_reading_alarms(conn)?;

    for row in get_all_alarm_state(conn)? {
        let Ok(num) = u16::try_from(row.alarm_num) else {
            continue;
        };
        if row.data_active {
            active.insert(num);
        } else {
            active.remove(&num);
        }
    }

    Ok(active)
}

/// The alarm set carried by the most recent reading that has one.
///
/// Looks past the newest row because not every reading carries alarm registers
/// — the SoC seeder writes to the same table — which is the same reason
/// `/Alarms/Active` scans rather than taking the first.
fn latest_reading_alarms(
    conn: &mut diesel::SqliteConnection,
) -> Result<HashSet<u16>, Box<dyn std::error::Error + Send + Sync>> {
    use neems_data::schema::readings::dsl::*;

    let recent: Vec<neems_data::models::Reading> =
        readings.order(timestamp.desc()).limit(10).load(conn)?;

    for reading in &recent {
        if let Some(registers) = parse_alarm_registers(&reading.data) {
            return Ok(AlarmFlags::from_registers(&registers)
                .active_alarms()
                .iter()
                .map(|d| d.alarm_num)
                .collect());
        }
    }

    Ok(HashSet::new())
}

pub fn routes() -> Vec<Route> {
    routes![inject_history, get_alarm_state, set_alarm_state]
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use diesel_migrations::MigrationHarness;
    use neems_data::rtac::site_controls::site_control_by_id;

    use super::*;

    fn site_conn() -> diesel::SqliteConnection {
        let mut conn =
            diesel::SqliteConnection::establish(":memory:").expect("in-memory site database");
        conn.run_pending_migrations(neems_data::MIGRATIONS).expect("site migrations");
        conn
    }

    /// The regression that matters: a control snapshot must not erase alarms
    /// that live only in a reading.
    ///
    /// `/1/Demo/InjectHistory` writes readings and never touches `alarm_state`,
    /// so a snapshot sourced from `alarm_state` alone drops every seeded alarm.
    /// Because `/Alarms/History` diffs consecutive bitfields, that does not
    /// read as "we forgot some" — it reads as the whole site returning to
    /// normal at the moment somebody clicked a breaker, which is precisely
    /// the timeline the demo exists to show.
    ///
    /// The seeded reading is written here the same way the seeder writes one:
    /// a bitfield, and no `alarm_state` row behind it.
    #[test]
    fn a_control_snapshot_keeps_alarms_that_live_only_in_a_reading() {
        let mut conn = site_conn();
        let earlier = chrono::Utc::now().naive_utc() - Duration::seconds(60);
        let seeded: HashSet<u16> = [1, 301].into_iter().collect();
        record_alarm_snapshot(&mut conn, DEMO_SITE_ID, &seeded, earlier).expect("seed a reading");
        assert!(
            get_all_alarm_state(&mut conn).expect("state").is_empty(),
            "seeding writes no state"
        );

        let feeder = site_control_by_id("feeder-1a").expect("feeder-1a is a control");
        apply_control_readback(&mut conn, feeder, SiteControlAction::Close).expect("dispatch");

        let after = latest_reading_alarms(&mut conn).expect("newest bitfield");
        assert!(after.contains(&607), "the breaker's own readback moved");
        assert!(after.contains(&1), "a seeded alarm must survive someone clicking a breaker");
        assert!(after.contains(&301), "and so must the rest of them");
    }

    /// The other direction: a demo alarm explicitly cleared has to clear, even
    /// though the seeded reading underneath it still carries the bit. This is
    /// why `alarm_state` is laid over the reading rather than merged into it.
    #[test]
    fn an_explicitly_cleared_alarm_wins_over_the_reading_beneath_it() {
        let mut conn = site_conn();
        let earlier = chrono::Utc::now().naive_utc() - Duration::seconds(60);
        let seeded: HashSet<u16> = [301].into_iter().collect();
        record_alarm_snapshot(&mut conn, DEMO_SITE_ID, &seeded, earlier).expect("seed a reading");

        // Raise then clear it, so `alarm_state` carries a false row rather than
        // no row — the state an operator leaves behind after clearing an alarm.
        let now = chrono::Utc::now().naive_utc();
        upsert_alarm_transition(&mut conn, 301, true, now).expect("raise");
        upsert_alarm_transition(&mut conn, 301, false, now).expect("clear");

        let feeder = site_control_by_id("feeder-1a").expect("feeder-1a is a control");
        apply_control_readback(&mut conn, feeder, SiteControlAction::Close).expect("dispatch");

        let after = latest_reading_alarms(&mut conn).expect("newest bitfield");
        assert!(!after.contains(&301), "an alarm cleared on purpose stays cleared");
        assert!(after.contains(&607), "the readback still moved");
    }

    /// A demo emergency shutdown goes through the same site-write path as a
    /// control, so it inherits the same guarantee: raising 104 must not
    /// erase alarms that live only in a seeded reading.
    #[test]
    fn a_demo_trip_raises_104_and_keeps_the_seeded_alarms() {
        let mut conn = site_conn();
        let earlier = chrono::Utc::now().naive_utc() - Duration::seconds(60);
        let seeded: HashSet<u16> = [1, 301].into_iter().collect();
        record_alarm_snapshot(&mut conn, DEMO_SITE_ID, &seeded, earlier).expect("seed a reading");

        apply_estop_trip(&mut conn).expect("trip");

        let after = latest_reading_alarms(&mut conn).expect("newest bitfield");
        assert!(after.contains(&ESTOP_ALARM_NUM), "the trip is in the reading");
        assert!(after.contains(&1) && after.contains(&301), "and nothing seeded was dropped");
        let state = get_all_alarm_state(&mut conn).expect("state");
        assert!(
            state.iter().any(|r| r.alarm_num == ESTOP_ALARM_NUM as i32 && r.data_active),
            "and in alarm_state, which `/EmergencyShutdown` reads"
        );
    }

    /// A control with no readback point has nothing to move, and that is not a
    /// failure: the signal is what was asked for.
    #[test]
    fn a_control_with_no_readback_is_not_an_error() {
        let mut conn = site_conn();
        let no_readback = SiteControl {
            readback: None,
            ..*site_control_by_id("feeder-1a").expect("feeder-1a is a control")
        };
        apply_control_readback(&mut conn, &no_readback, SiteControlAction::Close)
            .expect("dispatch");
    }
}
