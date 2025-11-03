use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::schedules;

/// A schedule for a specific start time
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
#[diesel(table_name = schedules)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct Schedule {
    pub id: i32,
    pub site_id: i32,
    /// The start time this schedule begins execution
    #[ts(type = "string")]
    pub schedule_start: chrono::NaiveDateTime,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schedules)]
pub struct NewSchedule {
    pub site_id: i32,
    pub schedule_start: chrono::NaiveDateTime,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ScheduleInput {
    pub site_id: i32,
    #[ts(type = "string")]
    pub schedule_start: chrono::NaiveDateTime,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ScheduleWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    #[ts(type = "string")]
    pub schedule_start: chrono::NaiveDateTime,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
