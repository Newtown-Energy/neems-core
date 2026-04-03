//! API endpoints for alarm data.
//!
//! This module provides HTTP endpoints for accessing alarm information
//! derived from RTAC readings stored in the site database.

use chrono::Utc;
use neems_data::rtac::{
    alarm_definitions::{ALARM_DEFINITIONS, ALARM_REGISTER_COUNT, AlarmDefinition, AlarmZone},
    state::AlarmFlags,
};
use rocket::{Route, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{orm::neems_data::db::SiteDbConn, session_guards::AuthenticatedUser};

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

/// A single alarm definition (static metadata)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlarmDefinitionDto {
    pub alarm_num: u16,
    pub zone: AlarmZoneDto,
    pub name: String,
    pub level: u8,
    pub severity: AlarmSeverityDto,
}

impl From<&AlarmDefinition> for AlarmDefinitionDto {
    fn from(def: &AlarmDefinition) -> Self {
        Self {
            alarm_num: def.alarm_num,
            zone: def.zone.into(),
            name: def.name.to_string(),
            level: def.level,
            severity: AlarmSeverityDto::from_level(def.level),
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
}

impl From<&AlarmDefinition> for ActiveAlarmDto {
    fn from(def: &AlarmDefinition) -> Self {
        Self {
            alarm_num: def.alarm_num,
            zone: def.zone.into(),
            name: def.name.to_string(),
            severity: AlarmSeverityDto::from_level(def.level),
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
fn parse_alarm_registers(data_json: &str) -> Option<[u16; ALARM_REGISTER_COUNT]> {
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
) -> Result<Json<ActiveAlarmsResponse>, Status> {
    site_db
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

                    return Ok(Json(ActiveAlarmsResponse {
                        alarms,
                        has_critical,
                        has_emergency,
                        timestamp: Some(reading_timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                        data_age_seconds: Some(age_seconds),
                    }));
                }
            }

            // No readings with alarm data found — return empty response
            Ok(Json(ActiveAlarmsResponse {
                alarms: vec![],
                has_critical: false,
                has_emergency: false,
                timestamp: None,
                data_age_seconds: None,
            }))
        })
        .await
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

/// Returns all routes defined in this module.
pub fn routes() -> Vec<Route> {
    routes![get_active_alarms, get_alarm_definitions]
}
