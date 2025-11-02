use chrono::NaiveDateTime;
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::scheduler_overrides;

#[derive(
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    QueryableByName,
    Debug,
    Serialize,
    Deserialize,
    TS,
)]
#[diesel(belongs_to(crate::models::site::Site))]
#[diesel(belongs_to(crate::models::user::User, foreign_key = created_by))]
#[diesel(table_name = scheduler_overrides)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct SchedulerOverride {
    pub id: i32,
    pub site_id: i32,
    pub state: String,
    #[ts(type = "string")]
    pub start_time: NaiveDateTime,
    #[ts(type = "string")]
    pub end_time: NaiveDateTime,
    pub created_by: i32,
    pub reason: Option<String>,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = scheduler_overrides)]
pub struct NewSchedulerOverride {
    pub site_id: i32,
    pub state: String,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub created_by: i32,
    pub reason: Option<String>,
    pub is_active: bool,
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct SchedulerOverrideInput {
    pub site_id: i32,
    pub state: String, // Must be one of: charge, discharge, idle
    #[ts(type = "string")]
    pub start_time: NaiveDateTime,
    #[ts(type = "string")]
    pub end_time: NaiveDateTime,
    pub reason: Option<String>,
    pub is_active: Option<bool>, // Optional, defaults to true
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UpdateSchedulerOverrideRequest {
    pub state: Option<String>,
    #[ts(type = "string")]
    pub start_time: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub end_time: Option<NaiveDateTime>,
    pub reason: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SchedulerOverrideWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    pub state: String,
    #[ts(type = "string")]
    pub start_time: NaiveDateTime,
    #[ts(type = "string")]
    pub end_time: NaiveDateTime,
    pub created_by: i32,
    pub reason: Option<String>,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SiteState {
    #[serde(rename = "charge")]
    Charge,
    #[serde(rename = "discharge")]
    Discharge,
    #[serde(rename = "idle")]
    Idle,
}

impl std::str::FromStr for SiteState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "charge" => Ok(SiteState::Charge),
            "discharge" => Ok(SiteState::Discharge),
            "idle" => Ok(SiteState::Idle),
            _ => Err(format!("Invalid site state: {}", s)),
        }
    }
}

impl SiteState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SiteState::Charge => "charge",
            SiteState::Discharge => "discharge",
            SiteState::Idle => "idle",
        }
    }
}

impl From<SchedulerOverrideInput> for NewSchedulerOverride {
    fn from(input: SchedulerOverrideInput) -> Self {
        Self {
            site_id: input.site_id,
            state: input.state,
            start_time: input.start_time,
            end_time: input.end_time,
            created_by: 0, // This should be set by the caller with the actual user ID
            reason: input.reason,
            is_active: input.is_active.unwrap_or(true),
        }
    }
}
