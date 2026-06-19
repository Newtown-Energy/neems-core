//! API endpoints for alarm data.
//!
//! This module provides HTTP endpoints for accessing alarm information
//! derived from RTAC readings stored in the site database.

use std::{collections::HashSet, sync::Mutex};

use chrono::{DateTime, Utc};
use neems_data::rtac::{
    alarm_definitions::{ALARM_DEFINITIONS, ALARM_REGISTER_COUNT, AlarmDefinition, AlarmZone},
    alarm_sld_meta::sld_meta_for,
    state::AlarmFlags,
};
use rocket::{FromForm, Route, State, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{orm::neems_data::db::SiteDbConn, session_guards::AuthenticatedUser};

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

/// A currently active alarm
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
}

impl From<&AlarmDefinition> for ActiveAlarmDto {
    fn from(def: &AlarmDefinition) -> Self {
        Self {
            alarm_num: def.alarm_num,
            zone: def.zone.into(),
            name: def.name.to_string(),
            severity: AlarmSeverityDto::from_level(def.level),
            message: message_for(def.alarm_num),
            sld_targets: sld_targets_for(def.alarm_num),
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

/// Get currently active alarms.
///
/// - **URL:** `/api/1/Alarms/Active`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Reads the most recent RTAC reading from the site database, decodes
/// the alarm register bitfield, and returns all currently active alarms.
#[get("/1/Alarms/Active")]
pub async fn get_active_alarms(
    _user: AuthenticatedUser,
    site_db: SiteDbConn,
    forced: &State<DemoForcedAlarms>,
) -> Result<Json<ActiveAlarmsResponse>, Status> {
    let forced_nums = forced.snapshot();
    let mut response: ActiveAlarmsResponse = site_db
        .run(|conn| {
            use diesel::prelude::*;
            use neems_data::schema::readings::dsl::*;

            // Get the most recent readings and find one with alarm_registers
            let recent: Vec<neems_data::models::Reading> =
                readings.order(timestamp.desc()).limit(10).load(conn).map_err(|e| {
                    eprintln!("Error loading readings for alarms: {:?}", e);
                    Status::InternalServerError
                })?;

            // Find the first reading that contains alarm_registers
            for reading in &recent {
                if let Some(registers) = parse_alarm_registers(&reading.data) {
                    let flags = AlarmFlags::from_registers(&registers);
                    let active_defs = flags.active_alarms();

                    let alarms: Vec<ActiveAlarmDto> =
                        active_defs.iter().map(|def| ActiveAlarmDto::from(*def)).collect();

                    let has_emergency = flags.has_emergency_alarm();
                    let has_critical = flags.has_critical_alarm();

                    let reading_timestamp = reading.timestamp;
                    let now = Utc::now().naive_utc();
                    let age_seconds = (now - reading_timestamp).num_seconds();

                    return Ok(ActiveAlarmsResponse {
                        alarms,
                        has_critical,
                        has_emergency,
                        timestamp: Some(reading_timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                        data_age_seconds: Some(age_seconds),
                    });
                }
            }

            // No readings with alarm data found — return empty response
            Ok(ActiveAlarmsResponse {
                alarms: vec![],
                has_critical: false,
                has_emergency: false,
                timestamp: None,
                data_age_seconds: None,
            })
        })
        .await?;

    // Overlay demo-forced alarms. We dedupe by alarm_num so a forced
    // alarm that's also currently active in the real feed doesn't
    // appear twice.
    if !forced_nums.is_empty() {
        let already: HashSet<u16> = response.alarms.iter().map(|a| a.alarm_num).collect();
        for def in ALARM_DEFINITIONS.iter() {
            if forced_nums.contains(&def.alarm_num) && !already.contains(&def.alarm_num) {
                response.alarms.push(ActiveAlarmDto::from(def));
            }
        }
        response.has_emergency = response
            .alarms
            .iter()
            .any(|a| matches!(a.severity, AlarmSeverityDto::Emergency));
        response.has_critical =
            response.alarms.iter().any(|a| matches!(a.severity, AlarmSeverityDto::Critical));
        // Surface a synthetic timestamp so the SLD's stale-data banner
        // doesn't fire purely because no readings exist in dev.
        if response.timestamp.is_none() {
            let now = Utc::now().naive_utc();
            response.timestamp = Some(now.format("%Y-%m-%dT%H:%M:%SZ").to_string());
            response.data_age_seconds = Some(0);
        }
    }

    Ok(Json(response))
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
        get_alarm_definitions,
        get_alarm_history,
        get_forced_alarms,
        put_forced_alarms
    ]
}
