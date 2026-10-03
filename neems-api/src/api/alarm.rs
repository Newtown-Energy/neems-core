//! API endpoints for alarm data.
//!
//! This module provides HTTP endpoints for accessing alarm information
//! derived from RTAC readings stored in the site database.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDateTime, Utc};
use neems_data::{
    get_all_alarm_state,
    models::AlarmStateRow,
    rtac::{
        alarm_definitions::{ALARM_REGISTER_COUNT, AlarmDefinition, AlarmZone, alarm_definitions},
        alarm_sld_meta::sld_meta_for,
        state::AlarmFlags,
    },
};
use rocket::{FromForm, Route, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::AlarmAcknowledgement,
    orm::{
        DbConn,
        alarm_acknowledgement::{acks_in_range, create_acknowledgement, latest_ack_by_alarm},
        neems_data::db::SiteDbConn,
    },
    session_guards::AuthenticatedUser,
};

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

/// The two independent axes of an alarm's state, as reported to clients.
///
/// Data state and acknowledgement are orthogonal: acknowledging records that an
/// operator has seen the alarm and does nothing to the condition itself, and
/// the condition going away does not acknowledge anything. The one coupling
/// between them is that a rising edge sets the alarm unacknowledged.
///
/// Three of the four combinations are visible; the fourth (cleared *and*
/// acknowledged) is exactly what it means for an alarm to be finished, and such
/// alarms are omitted from the active list entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmState {
    /// The condition is physically present right now.
    pub data_active: bool,
    /// An operator has acknowledged the alarm since it last went active.
    pub acknowledged: bool,
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
    /// Current data state: whether the condition is physically present. `false`
    /// for a returned-to-normal alarm that is still latched awaiting
    /// acknowledgement.
    pub data_active: bool,
    /// Whether an operator has acknowledged the alarm since it last went
    /// active. Orthogonal to [`Self::data_active`]: an acknowledged alarm may
    /// still be firing, and a cleared one may still be waiting for an
    /// acknowledgement.
    pub acknowledged: bool,
    /// ISO 8601 timestamp of the acknowledgement in force, if any.
    pub acknowledged_at: Option<String>,
    /// User id of the acknowledger, if any.
    pub acknowledged_by_user_id: Option<i32>,
    /// Email of the acknowledger, if any.
    pub acknowledged_by_email: Option<String>,
}

impl ActiveAlarmDto {
    /// Build a visible-alarm DTO from its definition, its two-axis state, and
    /// the most recent acknowledgement (if any).
    ///
    /// The acknowledger fields describe the acknowledgement *currently in
    /// force*, so they are populated only when the alarm reads as acknowledged.
    /// An alarm that was acknowledged and then went active again is
    /// unacknowledged, and naming the operator who acknowledged the previous
    /// activation would misattribute this one.
    fn build(
        def: &AlarmDefinition,
        state: AlarmState,
        ack: Option<&AlarmAcknowledgement>,
        emails: &HashMap<i32, String>,
    ) -> Self {
        let in_force = ack.filter(|_| state.acknowledged);
        Self {
            alarm_num: def.alarm_num,
            zone: def.zone.into(),
            name: def.name.to_string(),
            severity: AlarmSeverityDto::from_level(def.level),
            message: message_for(def.alarm_num),
            sld_targets: sld_targets_for(def.alarm_num),
            data_active: state.data_active,
            acknowledged: state.acknowledged,
            acknowledged_at: in_force
                .map(|a| a.acknowledged_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            acknowledged_by_user_id: in_force.map(|a| a.user_id),
            acknowledged_by_email: in_force.and_then(|a| emails.get(&a.user_id).cloned()),
        }
    }
}

/// The two-axis state of a single alarm, or `None` when it is not visible.
///
/// Inputs are the current data state, the last rising edge, and the most recent
/// acknowledgement (all UTC).
///
/// Acknowledgement latches to the *rising* edge, because the unit an operator
/// acknowledges is an activation instance rather than a reading (see issue
/// #106). An alarm held active for hours has one rising edge, so one
/// acknowledgement settles the whole span; a clear followed by a re-activation
/// stamps a new rising edge, which is what makes the second instance demand its
/// own acknowledgement. Returning to normal is not an acknowledgement event and
/// does not enter into it — `last_falling_at` plays no part here.
///
/// - Acknowledged: an acknowledgement landed at or after the rising edge that
///   started the current activation. With no recorded rising edge (seeded or
///   forced data) any acknowledgement counts.
/// - Visible: the alarm is active now, or it has gone active at some point and
///   is not acknowledged. Cleared *and* acknowledged means finished, so it
///   drops off the list.
fn effective_state(
    data_active: bool,
    last_rising_at: Option<NaiveDateTime>,
    last_ack_at: Option<NaiveDateTime>,
) -> Option<AlarmState> {
    let acknowledged = match (last_ack_at, last_rising_at) {
        (Some(ack), Some(rise)) => ack >= rise,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let visible = data_active || (last_rising_at.is_some() && !acknowledged);
    visible.then_some(AlarmState { data_active, acknowledged })
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
) -> Result<Json<ActiveAlarmsResponse>, Status> {
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

    // Union the materialised data-state into the "currently active" set.
    //
    // For the real feed this is redundant — the collector derives `alarm_state`
    // from the same readings — but it is what makes a demo-driven alarm (see
    // `/1/Demo/AlarmState`) behave like a real one when no RTAC feed is
    // present. Where the two disagree, because the collector lags a reading,
    // the union biases toward showing the alarm, which is the safe direction
    // for an alarm system.
    for s in &alarm_state {
        if s.data_active
            && let Ok(num) = u16::try_from(s.alarm_num)
        {
            reading_active.insert(num);
        }
    }

    let state_by_num: HashMap<i32, &AlarmStateRow> =
        alarm_state.iter().map(|s| (s.alarm_num, s)).collect();

    // Consider every alarm that is active now or has any recorded data state,
    // and keep the ones [`effective_state`] deems still visible. Iterating
    // The design's alarm definitions give a stable (definition) order.
    let mut consider: HashSet<u16> = reading_active.clone();
    for s in &alarm_state {
        if let Ok(num) = u16::try_from(s.alarm_num) {
            consider.insert(num);
        }
    }

    let mut alarms: Vec<ActiveAlarmDto> = Vec::new();
    for def in alarm_definitions().iter() {
        if !consider.contains(&def.alarm_num) {
            continue;
        }
        let num_i32 = def.alarm_num as i32;
        let ack = latest_ack.get(&num_i32);
        let row = state_by_num.get(&num_i32).copied();
        let data_active = reading_active.contains(&def.alarm_num);

        let state = effective_state(
            data_active,
            row.and_then(|s| s.last_rising_at),
            ack.map(|a| a.acknowledged_at),
        );

        if let Some(state) = state {
            alarms.push(ActiveAlarmDto::build(def, state, ack, &emails));
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
    if !alarm_definitions().iter().any(|d| d.alarm_num == alarm_num) {
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
        alarm_definitions().iter().map(AlarmDefinitionDto::from).collect();
    let total_count = definitions.len();

    Json(AlarmDefinitionsResponse { definitions, total_count })
}

// --- Alarm history ---

/// What happened to an alarm at a given point in the history timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AlarmHistoryEventDto {
    /// The alarm's data bit went from inactive to active.
    Activated,
    /// The alarm's data bit went from active to inactive.
    Cleared,
    /// A user acknowledged the alarm. Carries the acknowledger and note; does
    /// not imply any change in data state.
    Acknowledged,
}

/// A single event on an alarm's history timeline: a data-state transition
/// observed in a reading, or an operator acknowledgement.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlarmHistoryEntry {
    /// ISO 8601 timestamp (UTC): the reading the transition was observed in,
    /// or the moment the acknowledgement was recorded.
    pub timestamp: String,
    pub alarm_num: u16,
    pub zone: AlarmZoneDto,
    pub name: String,
    pub severity: AlarmSeverityDto,
    /// What happened. Switch on this; `active` is the older two-state view.
    pub event: AlarmHistoryEventDto,
    /// `true` only for [`AlarmHistoryEventDto::Activated`]. Retained so
    /// existing consumers that render Activated/Cleared keep working;
    /// acknowledgement entries report `false`.
    pub active: bool,
    /// User id of the acknowledger — `Acknowledged` entries only.
    pub acknowledged_by_user_id: Option<i32>,
    /// Email of the acknowledger — `Acknowledged` entries only.
    pub acknowledged_by_email: Option<String>,
    /// Free-form note recorded with the acknowledgement, if any.
    pub note: Option<String>,
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
/// flips relative to the prior reading. The most recent reading before `from`
/// seeds the diff baseline, so a flip carried by the very first in-range
/// reading is reported rather than swallowed.
///
/// Acknowledgements recorded in the same range are interleaved into the same
/// timeline as `Acknowledged` entries, so an operator can see when an alarm was
/// acknowledged relative to when it activated and cleared. Entries are sorted
/// by timestamp; transitions precede acknowledgements at the same instant.
#[get("/1/Alarms/History?<query..>")]
pub async fn get_alarm_history(
    query: AlarmHistoryQuery,
    _user: AuthenticatedUser,
    db: DbConn,
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

    let (readings, baseline): (
        Vec<neems_data::models::Reading>,
        Option<neems_data::models::Reading>,
    ) = site_db
        .run(move |conn| {
            use diesel::prelude::*;
            use neems_data::schema::readings::dsl::*;

            let in_range: Vec<neems_data::models::Reading> = readings
                .filter(timestamp.ge(from_naive))
                .filter(timestamp.le(to_naive))
                .order(timestamp.asc())
                .load(conn)
                .map_err(|e| {
                    eprintln!("Error loading readings for alarm history: {:?}", e);
                    Status::InternalServerError
                })?;

            // The most recent reading *before* the range, used only as the
            // diff baseline. Without it the first in-range reading has nothing
            // to compare against and silently emits no transition — so the
            // opening event of any window is invisible. That is barely
            // noticeable against a 1 Hz RTAC feed, but demo-driven readings are
            // sparse, and the first one is usually the whole point.
            let prior = readings
                .filter(timestamp.lt(from_naive))
                .order(timestamp.desc())
                .first(conn)
                .optional()
                .map_err(|e| {
                    eprintln!("Error loading baseline reading for alarm history: {:?}", e);
                    Status::InternalServerError
                })?;

            Ok::<_, Status>((in_range, prior))
        })
        .await?;

    /// An entry plus the raw sort keys used to merge the two sources. The
    /// wire type carries only a formatted timestamp string, which is a lossy
    /// key to sort on.
    struct TimedEntry {
        at: NaiveDateTime,
        /// Tie-break within the same instant: transitions (`false`) first.
        is_ack: bool,
        entry: AlarmHistoryEntry,
    }

    let mut entries: Vec<TimedEntry> = Vec::new();
    let mut prev_flags: Option<AlarmFlags> = baseline
        .as_ref()
        .and_then(|r| parse_alarm_registers(&r.data))
        .map(|regs| AlarmFlags::from_registers(&regs));

    for reading in &readings {
        let Some(regs) = parse_alarm_registers(&reading.data) else {
            continue;
        };
        let flags = AlarmFlags::from_registers(&regs);

        if let Some(prev) = &prev_flags {
            for def in alarm_definitions().iter() {
                if let Some(filter) = &alarm_filter {
                    if !filter.contains(&def.alarm_num) {
                        continue;
                    }
                }
                let was_active = prev.is_alarm_active(def);
                let is_active = flags.is_alarm_active(def);
                if was_active != is_active {
                    entries.push(TimedEntry {
                        at: reading.timestamp,
                        is_ack: false,
                        entry: AlarmHistoryEntry {
                            timestamp: reading.timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                            alarm_num: def.alarm_num,
                            zone: def.zone.into(),
                            name: def.name.to_string(),
                            severity: AlarmSeverityDto::from_level(def.level),
                            event: if is_active {
                                AlarmHistoryEventDto::Activated
                            } else {
                                AlarmHistoryEventDto::Cleared
                            },
                            active: is_active,
                            acknowledged_by_user_id: None,
                            acknowledged_by_email: None,
                            note: None,
                        },
                    });
                }
            }
        }
        prev_flags = Some(flags);
    }

    // Acknowledgements live in the main DB, so they're fetched separately and
    // merged into the same timeline.
    let (acks, emails) = db
        .run(move |conn| {
            use diesel::prelude::*;

            use crate::schema::users;

            let acks = acks_in_range(conn, from_dt, to_dt).map_err(|e| {
                eprintln!("Error loading acknowledgements for alarm history: {:?}", e);
                Status::InternalServerError
            })?;
            let ids: Vec<i32> = acks.iter().map(|a| a.user_id).collect();
            let email_pairs: Vec<(i32, String)> = users::table
                .filter(users::id.eq_any(&ids))
                .select((users::id, users::email))
                .load(conn)
                .map_err(|_| Status::InternalServerError)?;
            Ok::<_, Status>((acks, email_pairs.into_iter().collect::<HashMap<i32, String>>()))
        })
        .await?;

    let defs_by_num: HashMap<u16, &AlarmDefinition> =
        alarm_definitions().iter().map(|d| (d.alarm_num, d)).collect();

    for ack in &acks {
        let Ok(num) = u16::try_from(ack.alarm_num) else {
            continue;
        };
        if let Some(filter) = &alarm_filter {
            if !filter.contains(&num) {
                continue;
            }
        }
        // An ack for an alarm_num with no definition (e.g. one retired from the
        // spec) has nothing to render, so it is skipped rather than guessed at.
        let Some(def) = defs_by_num.get(&num) else {
            continue;
        };
        entries.push(TimedEntry {
            at: ack.acknowledged_at,
            is_ack: true,
            entry: AlarmHistoryEntry {
                timestamp: ack.acknowledged_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                alarm_num: num,
                zone: def.zone.into(),
                name: def.name.to_string(),
                severity: AlarmSeverityDto::from_level(def.level),
                event: AlarmHistoryEventDto::Acknowledged,
                active: false,
                acknowledged_by_user_id: Some(ack.user_id),
                acknowledged_by_email: emails.get(&ack.user_id).cloned(),
                note: ack.note.clone(),
            },
        });
    }

    // Stable merge: chronological, with transitions ahead of acknowledgements
    // recorded at the same instant (you can't ack an alarm before it fires).
    entries.sort_by(|a, b| a.at.cmp(&b.at).then(a.is_ack.cmp(&b.is_ack)));
    let entries: Vec<AlarmHistoryEntry> = entries.into_iter().map(|e| e.entry).collect();

    Ok(Json(AlarmHistoryResponse { entries, from: from_str, to: to_str }))
}

/// Returns all routes defined in this module.
pub fn routes() -> Vec<Route> {
    routes![get_active_alarms, acknowledge_alarm, get_alarm_definitions, get_alarm_history]
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate, NaiveDateTime};
    use neems_data::rtac::alarm_definitions::AlarmZone;

    use super::{AlarmState, AlarmZoneDto, effective_state};

    /// A zone must spell itself the same way everywhere the frontend meets it.
    ///
    /// `AlarmZone::code()` is what the RTAC collector writes as the key of
    /// each per-Megapack analog block, and `AlarmZoneDto` is what every alarm
    /// response carries. The frontend joins the two — the gauge and the alarm
    /// badge on one diagram component — by that string. Nothing but this
    /// assertion stops a rename or a `#[serde(rename_all)]` on the DTO from
    /// splitting them silently: the join would just stop matching, and a
    /// component would show alarms with no readings, or readings with no
    /// alarms.
    #[test]
    fn alarm_zone_dto_matches_zone_code() {
        for zone in AlarmZone::ALL {
            let dto = AlarmZoneDto::from(zone);
            let serialized = serde_json::to_string(&dto).expect("zone DTO serializes");
            assert_eq!(serialized, format!("\"{}\"", zone.code()), "{zone:?}");
        }
    }

    /// Test timestamp `base + secs` seconds.
    fn t(secs: i64) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, 19).unwrap().and_hms_opt(0, 0, 0).unwrap()
            + Duration::seconds(secs)
    }

    /// A visible alarm in the given state.
    fn visible(data_active: bool, acknowledged: bool) -> Option<AlarmState> {
        Some(AlarmState { data_active, acknowledged })
    }

    #[test]
    fn active_and_never_acked_is_unacknowledged() {
        assert_eq!(effective_state(true, Some(t(10)), None), visible(true, false));
    }

    #[test]
    fn acked_after_rise_while_active_stays_active_and_acknowledged() {
        // rose at 10, acked at 20, still active: acking does not clear
        assert_eq!(effective_state(true, Some(t(10)), Some(t(20))), visible(true, true));
    }

    #[test]
    fn stale_ack_before_current_rise_does_not_acknowledge() {
        // a new activation rose at 30; the ack at 10 belongs to an earlier one
        assert_eq!(effective_state(true, Some(t(30)), Some(t(10))), visible(true, false));
    }

    #[test]
    fn blip_never_acked_stays_visible() {
        // rose 10, fell 15, now inactive, never acked: the operator missed it
        // overnight and must still be told it happened
        assert_eq!(effective_state(false, Some(t(10)), None), visible(false, false));
    }

    /// One acknowledgement settles the activation it belongs to, for good.
    ///
    /// This is the fix for issue #106, and inverts the "require 2nd ack" rule
    /// from #76: acking a firing alarm used to leave it demanding a second
    /// acknowledgement once it returned to normal.
    #[test]
    fn ack_while_active_then_return_is_finished() {
        // rose 10, acked 20 (while active), fell 30 -> done, not visible
        assert_eq!(effective_state(false, Some(t(10)), Some(t(20))), None);
    }

    /// A clear splits the timeline: what follows is a second instance, and an
    /// acknowledgement of the first does not carry over to it.
    #[test]
    fn reactivation_after_ack_requires_its_own_ack() {
        // rose 10, acked 20, fell 30, rose 40, fell 50 -> needs another ack
        assert_eq!(effective_state(false, Some(t(40)), Some(t(20))), visible(false, false));
    }

    /// Conversely, a continuously-active alarm is one instance however long it
    /// runs. This function reads only the rising edge, so elapsed time cannot
    /// re-arm the acknowledgement; what keeps that edge from moving under a
    /// stream of active readings is `upsert_alarm_transition`, which drops
    /// writes that assert the state the row already holds.
    #[test]
    fn continuous_activation_stays_acknowledged() {
        // rose at 10, acked at 20, still reading active five hours on
        assert_eq!(effective_state(true, Some(t(10)), Some(t(20))), visible(true, true));
        assert_eq!(effective_state(true, Some(t(10)), Some(t(18_000))), visible(true, true));
    }

    #[test]
    fn ack_after_return_to_normal_clears() {
        // rose 10, fell 30, acked 40 (after it returned) -> cleared
        assert_eq!(effective_state(false, Some(t(10)), Some(t(40))), None);
    }

    #[test]
    fn never_active_is_cleared() {
        assert_eq!(effective_state(false, None, None), None);
    }

    #[test]
    fn active_without_recorded_edges_falls_back_to_ack_presence() {
        // forced/seeded data has no edges recorded
        assert_eq!(effective_state(true, None, None), visible(true, false));
        assert_eq!(effective_state(true, None, Some(t(5))), visible(true, true));
    }
}
