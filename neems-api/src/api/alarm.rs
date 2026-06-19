//! API endpoints for alarm data.
//!
//! This module provides HTTP endpoints for accessing alarm information
//! derived from RTAC readings stored in the site database.

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use chrono::{DateTime, NaiveDateTime, Utc};
use neems_data::{
    get_all_alarm_state,
    models::AlarmStateRow,
    rtac::{
        alarm_definitions::{ALARM_DEFINITIONS, ALARM_REGISTER_COUNT, AlarmDefinition, AlarmZone},
        alarm_sld_meta::sld_meta_for,
        state::AlarmFlags,
    },
};
use rocket::{FromForm, Route, State, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::AlarmAcknowledgement,
    orm::{
        DbConn,
        alarm_acknowledgement::{create_acknowledgement, latest_ack_by_alarm},
        neems_data::db::SiteDbConn,
    },
    session_guards::AuthenticatedUser,
};

/// Roles allowed to control the demo forced-alarm set — mirrors the
/// frontend Demo Controls drawer's gate.
const DEMO_CONTROL_ROLES: &[&str] = &["admin", "newtown-admin", "newtown-staff"];

/// In-memory set of alarm numbers the demo drawer has forced on. Unioned
/// into [`get_active_alarms`] responses so the SLD, alarms page, and
/// anything else polling `/Alarms/Active` see them as if they were real.
///
/// Temporary scaffolding for the demo — meant to be deleted once the
/// real RTAC feed is hooked up. Lives in memory only; resets on server
/// restart, which is the desired demo behavior.
#[derive(Default)]
pub struct DemoForcedAlarms {
    inner: Mutex<HashSet<u16>>,
}

impl DemoForcedAlarms {
    fn snapshot(&self) -> HashSet<u16> {
        self.inner.lock().map(|g| g.clone()).unwrap_or_default()
    }

    fn replace(&self, nums: HashSet<u16>) {
        if let Ok(mut g) = self.inner.lock() {
            *g = nums;
        }
    }
}

/// Alarm severity level for API responses
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AlarmSeverityDto {
    Emergency,
    Critical,
    Warning,
    Info,
}

impl AlarmSeverityDto {
    fn from_level(level: u8) -> Self {
        match level {
            1 => Self::Emergency,
            2 => Self::Critical,
            3 => Self::Warning,
            _ => Self::Info,
        }
    }
}

/// Alarm zone for API responses
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AlarmZoneDto {
    Site,
    BreakerRelay,
    Meter,
    Transformer1,
    Transformer2,
    Rtac,
    Facp,
    TeslaSiteController,
    Mp1a,
    Mp1b,
    Mp1c,
    Mp2a,
    Mp2b,
    Mp2c,
}

impl From<AlarmZone> for AlarmZoneDto {
    fn from(zone: AlarmZone) -> Self {
        match zone {
            AlarmZone::Site => Self::Site,
            AlarmZone::BreakerRelay => Self::BreakerRelay,
            AlarmZone::Meter => Self::Meter,
            AlarmZone::Transformer1 => Self::Transformer1,
            AlarmZone::Transformer2 => Self::Transformer2,
            AlarmZone::Rtac => Self::Rtac,
            AlarmZone::Facp => Self::Facp,
            AlarmZone::TeslaSiteController => Self::TeslaSiteController,
            AlarmZone::Mp1a => Self::Mp1a,
            AlarmZone::Mp1b => Self::Mp1b,
            AlarmZone::Mp1c => Self::Mp1c,
            AlarmZone::Mp2a => Self::Mp2a,
            AlarmZone::Mp2b => Self::Mp2b,
            AlarmZone::Mp2c => Self::Mp2c,
        }
    }
}

/// Operator-facing message for an alarm, sourced from the alarm spreadsheet
/// ("Mouseover" column). `None` when the spreadsheet left it blank.
fn message_for(alarm_num: u16) -> Option<String> {
    sld_meta_for(alarm_num).and_then(|m| m.message_opt()).map(|s| s.to_string())
}

/// Raw "Related SLD Object" tokens for an alarm. Mapping tokens to concrete UI
/// elements is the frontend's job; the backend stays UI-agnostic.
fn sld_targets_for(alarm_num: u16) -> Vec<String> {
    sld_meta_for(alarm_num)
        .map(|m| m.sld_targets.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

/// A single alarm definition (static metadata)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlarmDefinitionDto {
    pub alarm_num: u16,
    pub zone: AlarmZoneDto,
    pub name: String,
    pub level: u8,
    pub severity: AlarmSeverityDto,
    /// Operator-facing message (spreadsheet "Mouseover"); null when blank.
    pub message: Option<String>,
    /// Target SLD object tokens (spreadsheet "Related SLD Object").
    pub sld_targets: Vec<String>,
}

impl From<&AlarmDefinition> for AlarmDefinitionDto {
    fn from(def: &AlarmDefinition) -> Self {
        Self {
            alarm_num: def.alarm_num,
            zone: def.zone.into(),
            name: def.name.to_string(),
            level: def.level,
            severity: AlarmSeverityDto::from_level(def.level),
            message: message_for(def.alarm_num),
            sld_targets: sld_targets_for(def.alarm_num),
        }
    }
}

/// Effective status of a visible alarm, combining raw data state with
/// acknowledgement. Cleared alarms (acknowledged after returning to normal,
/// with no activity since) are omitted from the active list entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AlarmStatusDto {
    /// Data is currently active and has not been acknowledged since it last
    /// went active.
    Active,
    /// Data is currently active and has been acknowledged — the operator has
    /// seen it, but the condition is still physically present.
    AcknowledgedActive,
    /// Data is no longer active, but the alarm was active at some point since
    /// the last acknowledgement (the "blip" / returned-to-normal-unacked). It
    /// still requires acknowledgement before it clears.
    ReturnedUnacknowledged,
}

/// A currently visible alarm: either active now, or latched (returned to
/// normal but not yet acknowledged).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActiveAlarmDto {
    pub alarm_num: u16,
    pub zone: AlarmZoneDto,
    pub name: String,
    pub severity: AlarmSeverityDto,
    /// Operator-facing message (spreadsheet "Mouseover"); null when blank.
    pub message: Option<String>,
    /// Target SLD object tokens (spreadsheet "Related SLD Object").
    pub sld_targets: Vec<String>,
    /// Effective status (active / acknowledged-active / returned-unacked).
    pub status: AlarmStatusDto,
    /// Raw current data state, independent of acknowledgement. `false` for a
    /// returned-to-normal alarm that is still latched awaiting acknowledgement.
    pub data_active: bool,
    /// ISO 8601 timestamp of the most recent acknowledgement, if any.
    pub acknowledged_at: Option<String>,
    /// User id of the most recent acknowledger, if any.
    pub acknowledged_by_user_id: Option<i32>,
    /// Email of the most recent acknowledger, if any.
    pub acknowledged_by_email: Option<String>,
}

impl ActiveAlarmDto {
    /// Build a visible-alarm DTO from its definition plus the computed status
    /// and the most recent acknowledgement (if any).
    fn build(
        def: &AlarmDefinition,
        status: AlarmStatusDto,
        data_active: bool,
        ack: Option<&AlarmAcknowledgement>,
        emails: &HashMap<i32, String>,
    ) -> Self {
        Self {
            alarm_num: def.alarm_num,
            zone: def.zone.into(),
            name: def.name.to_string(),
            severity: AlarmSeverityDto::from_level(def.level),
            message: message_for(def.alarm_num),
            sld_targets: sld_targets_for(def.alarm_num),
            status,
            data_active,
            acknowledged_at: ack
                .map(|a| a.acknowledged_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            acknowledged_by_user_id: ack.map(|a| a.user_id),
            acknowledged_by_email: ack.and_then(|a| emails.get(&a.user_id).cloned()),
        }
    }
}

/// Effective visible status of a single alarm.
///
/// Inputs are the raw current data state, the last rising/falling edge
/// timestamps, and the timestamp of the most recent acknowledgement (all UTC).
/// Returns `None` when the alarm is cleared (not visible).
///
/// Rules (see issue #76):
/// - Active now: `AcknowledgedActive` if an ack landed at/after the rising edge
///   that started the current activation, else `Active`. With no recorded
///   rising edge (seeded/forced data) any ack counts as acknowledged.
/// - Inactive now: visible as `ReturnedUnacknowledged` only if it went active
///   since the last ack — i.e. the last falling edge is after the most recent
///   ack (or it was never acked). Otherwise it has cleared.
fn effective_status(
    data_active: bool,
    last_rising_at: Option<NaiveDateTime>,
    last_falling_at: Option<NaiveDateTime>,
    last_ack_at: Option<NaiveDateTime>,
) -> Option<AlarmStatusDto> {
    if data_active {
        let acked = match (last_ack_at, last_rising_at) {
            (Some(ack), Some(rise)) => ack >= rise,
            (Some(_), None) => true,
            _ => false,
        };
        Some(if acked { AlarmStatusDto::AcknowledgedActive } else { AlarmStatusDto::Active })
    } else {
        match last_falling_at {
            Some(fall) => {
                let visible = match last_ack_at {
                    Some(ack) => fall > ack,
                    None => true,
                };
                visible.then_some(AlarmStatusDto::ReturnedUnacknowledged)
            }
            None => None,
        }
    }
}

/// Response for active alarms endpoint
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActiveAlarmsResponse {
    pub alarms: Vec<ActiveAlarmDto>,
    pub has_critical: bool,
    pub has_emergency: bool,
    /// ISO 8601 timestamp of the reading used to determine alarm state
    pub timestamp: Option<String>,
    /// How many seconds old the reading data is (null if no data)
    pub data_age_seconds: Option<i64>,
}

/// Response for alarm definitions endpoint
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlarmDefinitionsResponse {
    pub definitions: Vec<AlarmDefinitionDto>,
    pub total_count: usize,
}

/// Attempt to parse alarm_registers from a reading's JSON data field.
///
/// Returns the alarm registers array if the data contains a valid
/// `alarm_registers` field with exactly ALARM_REGISTER_COUNT elements.
pub fn parse_alarm_registers(data_json: &str) -> Option<[u16; ALARM_REGISTER_COUNT]> {
    let parsed: serde_json::Value = serde_json::from_str(data_json).ok()?;
    let arr = parsed.get("alarm_registers")?.as_array()?;
    if arr.len() != ALARM_REGISTER_COUNT {
        return None;
    }
    let mut registers = [0u16; ALARM_REGISTER_COUNT];
    for (i, val) in arr.iter().enumerate() {
        registers[i] = val.as_u64()? as u16;
    }
    Some(registers)
}

/// Get currently visible alarms.
///
/// - **URL:** `/api/1/Alarms/Active`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Combines the latest RTAC reading (current raw data state) with the
/// materialised `alarm_state` table and acknowledgement history to return
/// every alarm that is still visible to operators:
///
/// - currently active (`Active` or `AcknowledgedActive`), or
/// - returned to normal but active at some point since the last acknowledgement
///   (`ReturnedUnacknowledged`, the "blip").
///
/// An alarm clears (and drops out of this list) only once it has been
/// acknowledged *after* its data returned to normal, with no activity since.
#[get("/1/Alarms/Active")]
pub async fn get_active_alarms(
    _user: AuthenticatedUser,
    db: DbConn,
    site_db: SiteDbConn,
    forced: &State<DemoForcedAlarms>,
) -> Result<Json<ActiveAlarmsResponse>, Status> {
    let forced_nums = forced.snapshot();

    // Site DB: the latest reading's active alarm set (+ its timestamp) and the
    // materialised per-alarm data-state rows.
    let (mut reading_active, reading_ts, alarm_state) = site_db
        .run(|conn| {
            use diesel::prelude::*;
            use neems_data::schema::readings::dsl::*;

            let recent: Vec<neems_data::models::Reading> = readings
                .order(timestamp.desc())
                .limit(10)
                .load(conn)
                .map_err(|_| Status::InternalServerError)?;

            let mut active_set: HashSet<u16> = HashSet::new();
            let mut ts: Option<NaiveDateTime> = None;
            for reading in &recent {
                if let Some(regs) = parse_alarm_registers(&reading.data) {
                    let flags = AlarmFlags::from_registers(&regs);
                    active_set = flags.active_alarms().iter().map(|d| d.alarm_num).collect();
                    ts = Some(reading.timestamp);
                    break;
                }
            }

            let state = get_all_alarm_state(conn).map_err(|_| Status::InternalServerError)?;
            Ok::<_, Status>((active_set, ts, state))
        })
        .await?;

    // Main DB: most recent acknowledgement per alarm, plus the acknowledgers'
    // emails so the UI can show who acked.
    let (latest_ack, emails) = db
        .run(|conn| {
            use diesel::prelude::*;

            use crate::schema::users;

            let latest = latest_ack_by_alarm(conn).map_err(|_| Status::InternalServerError)?;
            let ids: Vec<i32> = latest.values().map(|a| a.user_id).collect();
            let email_pairs: Vec<(i32, String)> = users::table
                .filter(users::id.eq_any(&ids))
                .select((users::id, users::email))
                .load(conn)
                .map_err(|_| Status::InternalServerError)?;
            let emails: HashMap<i32, String> = email_pairs.into_iter().collect();
            Ok::<_, Status>((latest, emails))
        })
        .await?;

    // Overlay demo-forced alarms onto the "currently active" set.
    let valid: HashSet<u16> = ALARM_DEFINITIONS.iter().map(|d| d.alarm_num).collect();
    for n in &forced_nums {
        if valid.contains(n) {
            reading_active.insert(*n);
        }
    }

    let state_by_num: HashMap<i32, &AlarmStateRow> =
        alarm_state.iter().map(|s| (s.alarm_num, s)).collect();

    // Consider every alarm that is active now or has any recorded data state,
    // and keep the ones [`effective_status`] deems still visible. Iterating
    // ALARM_DEFINITIONS gives a stable (definition) order.
    let mut consider: HashSet<u16> = reading_active.clone();
    for s in &alarm_state {
        if let Ok(num) = u16::try_from(s.alarm_num) {
            consider.insert(num);
        }
    }

    let mut alarms: Vec<ActiveAlarmDto> = Vec::new();
    for def in ALARM_DEFINITIONS.iter() {
        if !consider.contains(&def.alarm_num) {
            continue;
        }
        let num_i32 = def.alarm_num as i32;
        let ack = latest_ack.get(&num_i32);
        let state = state_by_num.get(&num_i32).copied();
        let data_active = reading_active.contains(&def.alarm_num);

        let status = effective_status(
            data_active,
            state.and_then(|s| s.last_rising_at),
            state.and_then(|s| s.last_falling_at),
            ack.map(|a| a.acknowledged_at),
        );

        if let Some(status) = status {
            alarms.push(ActiveAlarmDto::build(def, status, data_active, ack, &emails));
        }
    }

    let has_emergency = alarms.iter().any(|a| matches!(a.severity, AlarmSeverityDto::Emergency));
    let has_critical = alarms.iter().any(|a| matches!(a.severity, AlarmSeverityDto::Critical));

    // Timestamp/age: prefer the real reading. With no readings but visible
    // alarms (demo/forced), synthesise a fresh timestamp so the SLD's
    // stale-data banner doesn't fire spuriously.
    let (timestamp, data_age_seconds) = match reading_ts {
        Some(t) => {
            let age = (Utc::now().naive_utc() - t).num_seconds();
            (Some(t.format("%Y-%m-%dT%H:%M:%SZ").to_string()), Some(age))
        }
        None if !alarms.is_empty() => {
            let now = Utc::now().naive_utc();
            (Some(now.format("%Y-%m-%dT%H:%M:%SZ").to_string()), Some(0))
        }
        None => (None, None),
    };

    Ok(Json(ActiveAlarmsResponse {
        alarms,
        has_critical,
        has_emergency,
        timestamp,
        data_age_seconds,
    }))
}

/// Body for `POST /1/Alarms/Acknowledge`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AcknowledgeAlarmRequest {
    pub alarm_num: u16,
    /// Optional free-form note recorded with the acknowledgement.
    pub note: Option<String>,
}

/// Response for `POST /1/Alarms/Acknowledge`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AcknowledgeAlarmResponse {
    pub alarm_num: u16,
    /// ISO 8601 timestamp the acknowledgement was recorded.
    pub acknowledged_at: String,
    pub acknowledged_by_user_id: i32,
    pub acknowledged_by_email: Option<String>,
    pub note: Option<String>,
}

/// Acknowledge an alarm on behalf of the authenticated user.
///
/// - **URL:** `/api/1/Alarms/Acknowledge`
/// - **Method:** `POST`
/// - **Body:** `{ "alarm_num": u16, "note": "optional" }`
/// - **Authentication:** Required
///
/// Append-only: records a new acknowledgement row tied to the current user.
/// Acknowledging an alarm that is still active does not clear it — the alarm
/// stays visible (as `AcknowledgedActive`) and must be acknowledged again once
/// it has returned to normal. Re-poll `/Alarms/Active` for the updated status.
#[post("/1/Alarms/Acknowledge", data = "<body>")]
pub async fn acknowledge_alarm(
    user: AuthenticatedUser,
    db: DbConn,
    body: Json<AcknowledgeAlarmRequest>,
) -> Result<Json<AcknowledgeAlarmResponse>, Status> {
    let alarm_num = body.alarm_num;
    if !ALARM_DEFINITIONS.iter().any(|d| d.alarm_num == alarm_num) {
        return Err(Status::BadRequest);
    }
    let user_id = user.user.id;
    let email = user.user.email.clone();
    let note = body.note.clone();

    let ack = db
        .run(move |conn| create_acknowledgement(conn, alarm_num as i32, user_id, note))
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(AcknowledgeAlarmResponse {
        alarm_num,
        acknowledged_at: ack.acknowledged_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        acknowledged_by_user_id: ack.user_id,
        acknowledged_by_email: Some(email),
        note: ack.note,
    }))
}

/// Body for `PUT /1/Alarms/Forced`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ForcedAlarmsRequest {
    pub alarm_nums: Vec<u16>,
}

/// Response payload for the demo forced-alarm endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ForcedAlarmsResponse {
    pub alarm_nums: Vec<u16>,
}

fn forbid_unless_demo_role(user: &AuthenticatedUser) -> Result<(), Status> {
    if user.has_any_role(DEMO_CONTROL_ROLES) {
        Ok(())
    } else {
        Err(Status::Forbidden)
    }
}

/// Read the current demo forced-alarm set.
///
/// - **URL:** `/api/1/Alarms/Forced`
/// - **Method:** `GET`
/// - **Authentication:** Required; one of `admin`, `newtown-admin`,
///   `newtown-staff`.
#[get("/1/Alarms/Forced")]
pub fn get_forced_alarms(
    user: AuthenticatedUser,
    forced: &State<DemoForcedAlarms>,
) -> Result<Json<ForcedAlarmsResponse>, Status> {
    forbid_unless_demo_role(&user)?;
    let mut nums: Vec<u16> = forced.snapshot().into_iter().collect();
    nums.sort_unstable();
    Ok(Json(ForcedAlarmsResponse { alarm_nums: nums }))
}

/// Replace the demo forced-alarm set.
///
/// - **URL:** `/api/1/Alarms/Forced`
/// - **Method:** `PUT`
/// - **Body:** `{ "alarm_nums": [u16, ...] }`
/// - **Authentication:** Required; one of `admin`, `newtown-admin`,
///   `newtown-staff`.
///
/// The supplied list replaces the in-memory set (it is not additive). Pass
/// an empty list to clear all forced alarms. Unknown alarm numbers are
/// silently filtered against [`ALARM_DEFINITIONS`].
#[put("/1/Alarms/Forced", data = "<body>")]
pub fn put_forced_alarms(
    user: AuthenticatedUser,
    forced: &State<DemoForcedAlarms>,
    body: Json<ForcedAlarmsRequest>,
) -> Result<Json<ForcedAlarmsResponse>, Status> {
    forbid_unless_demo_role(&user)?;
    let valid: HashSet<u16> = ALARM_DEFINITIONS.iter().map(|d| d.alarm_num).collect();
    let next: HashSet<u16> =
        body.alarm_nums.iter().copied().filter(|n| valid.contains(n)).collect();
    forced.replace(next.clone());
    let mut nums: Vec<u16> = next.into_iter().collect();
    nums.sort_unstable();
    Ok(Json(ForcedAlarmsResponse { alarm_nums: nums }))
}

/// Get all alarm definitions.
///
/// - **URL:** `/api/1/Alarms/Definitions`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Returns the complete list of alarm definitions with their metadata
/// (alarm number, zone, name, level, severity).
#[get("/1/Alarms/Definitions")]
pub async fn get_alarm_definitions(_user: AuthenticatedUser) -> Json<AlarmDefinitionsResponse> {
    let definitions: Vec<AlarmDefinitionDto> =
        ALARM_DEFINITIONS.iter().map(AlarmDefinitionDto::from).collect();
    let total_count = definitions.len();

    Json(AlarmDefinitionsResponse { definitions, total_count })
}

// --- Alarm history ---

/// A single alarm-state transition emitted by the history endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlarmHistoryEntry {
    /// ISO 8601 timestamp (UTC) of the reading in which the transition was
    /// observed.
    pub timestamp: String,
    pub alarm_num: u16,
    pub zone: AlarmZoneDto,
    pub name: String,
    pub severity: AlarmSeverityDto,
    /// `true` if the alarm became active at this reading, `false` if it
    /// cleared.
    pub active: bool,
}

/// Response for the alarm-history endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlarmHistoryResponse {
    pub entries: Vec<AlarmHistoryEntry>,
    /// Echo of the requested range start (ISO 8601).
    pub from: String,
    /// Echo of the requested range end (ISO 8601).
    pub to: String,
}

/// Query parameters for `GET /1/Alarms/History`.
#[derive(Debug, Clone, FromForm, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlarmHistoryQuery {
    /// ISO 8601 timestamp — start of the range (inclusive).
    pub from: Option<String>,
    /// ISO 8601 timestamp — end of the range (inclusive).
    pub to: Option<String>,
    /// Comma-separated list of alarm_num values to filter on. Omitted = all
    /// alarms.
    pub alarm_nums: Option<String>,
}

fn parse_iso8601(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))
}

fn parse_alarm_nums_filter(raw: &str) -> HashSet<u16> {
    raw.split(',').filter_map(|t| t.trim().parse::<u16>().ok()).collect()
}

/// Get alarm status transitions over a date range.
///
/// - **URL:** `/api/1/Alarms/History?from=<ISO8601>&to=<ISO8601>&
///   alarm_nums=<u16,u16,...>`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Walks readings in `[from, to]`, decodes each reading's alarm register
/// bitfield, and emits a transition entry each time a given alarm's active bit
/// flips relative to the prior reading in range. Does not seed a baseline from
/// before `from`, so a transition that occurred right before the range start
/// won't appear — extend the range to capture it, or cross-reference with
/// `/Alarms/Active` for current state.
#[get("/1/Alarms/History?<query..>")]
pub async fn get_alarm_history(
    query: AlarmHistoryQuery,
    _user: AuthenticatedUser,
    site_db: SiteDbConn,
) -> Result<Json<AlarmHistoryResponse>, Status> {
    let from_str = query.from.clone().ok_or(Status::BadRequest)?;
    let to_str = query.to.clone().ok_or(Status::BadRequest)?;
    let from_dt = parse_iso8601(&from_str).ok_or(Status::BadRequest)?;
    let to_dt = parse_iso8601(&to_str).ok_or(Status::BadRequest)?;
    if from_dt > to_dt {
        return Err(Status::BadRequest);
    }
    let alarm_filter: Option<HashSet<u16>> =
        query.alarm_nums.as_deref().map(parse_alarm_nums_filter);

    let from_naive = from_dt.naive_utc();
    let to_naive = to_dt.naive_utc();

    let readings: Vec<neems_data::models::Reading> = site_db
        .run(move |conn| {
            use diesel::prelude::*;
            use neems_data::schema::readings::dsl::*;

            readings
                .filter(timestamp.ge(from_naive))
                .filter(timestamp.le(to_naive))
                .order(timestamp.asc())
                .load(conn)
                .map_err(|e| {
                    eprintln!("Error loading readings for alarm history: {:?}", e);
                    Status::InternalServerError
                })
        })
        .await?;

    let mut entries: Vec<AlarmHistoryEntry> = Vec::new();
    let mut prev_flags: Option<AlarmFlags> = None;

    for reading in &readings {
        let Some(regs) = parse_alarm_registers(&reading.data) else {
            continue;
        };
        let flags = AlarmFlags::from_registers(&regs);

        if let Some(prev) = &prev_flags {
            for def in ALARM_DEFINITIONS.iter() {
                if let Some(filter) = &alarm_filter {
                    if !filter.contains(&def.alarm_num) {
                        continue;
                    }
                }
                let was_active = prev.is_alarm_active(def);
                let is_active = flags.is_alarm_active(def);
                if was_active != is_active {
                    entries.push(AlarmHistoryEntry {
                        timestamp: reading.timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                        alarm_num: def.alarm_num,
                        zone: def.zone.into(),
                        name: def.name.to_string(),
                        severity: AlarmSeverityDto::from_level(def.level),
                        active: is_active,
                    });
                }
            }
        }
        prev_flags = Some(flags);
    }

    Ok(Json(AlarmHistoryResponse { entries, from: from_str, to: to_str }))
}

/// Returns all routes defined in this module.
pub fn routes() -> Vec<Route> {
    routes![
        get_active_alarms,
        acknowledge_alarm,
        get_alarm_definitions,
        get_alarm_history,
        get_forced_alarms,
        put_forced_alarms
    ]
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate, NaiveDateTime};

    use super::{AlarmStatusDto, effective_status};

    /// Test timestamp `base + secs` seconds.
    fn t(secs: i64) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, 19)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            + Duration::seconds(secs)
    }

    #[test]
    fn active_and_never_acked_is_active() {
        assert_eq!(effective_status(true, Some(t(10)), None, None), Some(AlarmStatusDto::Active));
    }

    #[test]
    fn acked_after_rise_while_active_is_acknowledged_active() {
        // rose at 10, acked at 20, still active
        assert_eq!(
            effective_status(true, Some(t(10)), None, Some(t(20))),
            Some(AlarmStatusDto::AcknowledgedActive)
        );
    }

    #[test]
    fn stale_ack_before_current_rise_does_not_acknowledge() {
        // a new activation rose at 30; the ack at 10 predates it
        assert_eq!(
            effective_status(true, Some(t(30)), Some(t(20)), Some(t(10))),
            Some(AlarmStatusDto::Active)
        );
    }

    #[test]
    fn blip_never_acked_stays_visible() {
        // rose 10, fell 15, now inactive, never acked
        assert_eq!(
            effective_status(false, Some(t(10)), Some(t(15)), None),
            Some(AlarmStatusDto::ReturnedUnacknowledged)
        );
    }

    #[test]
    fn ack_while_active_then_return_requires_second_ack() {
        // rose 10, acked 20 (while active), fell 30 -> still needs ack
        assert_eq!(
            effective_status(false, Some(t(10)), Some(t(30)), Some(t(20))),
            Some(AlarmStatusDto::ReturnedUnacknowledged)
        );
    }

    #[test]
    fn ack_after_return_to_normal_clears() {
        // rose 10, fell 30, acked 40 (after it returned) -> cleared
        assert_eq!(effective_status(false, Some(t(10)), Some(t(30)), Some(t(40))), None);
    }

    #[test]
    fn never_active_is_cleared() {
        assert_eq!(effective_status(false, None, None, None), None);
    }

    #[test]
    fn active_without_recorded_edges_falls_back_to_ack_presence() {
        // forced/seeded data has no edges recorded
        assert_eq!(effective_status(true, None, None, None), Some(AlarmStatusDto::Active));
        assert_eq!(
            effective_status(true, None, None, Some(t(5))),
            Some(AlarmStatusDto::AcknowledgedActive)
        );
    }
}
