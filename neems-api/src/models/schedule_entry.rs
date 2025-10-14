use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::schedule_entries;

/// A specific scheduled event that references a ScheduleCommand
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
#[diesel(belongs_to(crate::models::schedule::Schedule))]
#[diesel(belongs_to(crate::models::schedule_command::ScheduleCommand))]
#[diesel(table_name = schedule_entries)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct ScheduleEntry {
    pub id: i32,
    /// The schedule this entry belongs to
    pub schedule_id: i32,
    /// Seconds offset from schedule start time when this entry should execute
    pub execution_offset_seconds: i32,
    /// Reference to the schedule command to execute
    pub schedule_command_id: i32,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schedule_entries)]
pub struct NewScheduleEntry {
    pub schedule_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ScheduleEntryInput {
    pub schedule_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ScheduleEntryWithTimestamps {
    pub id: i32,
    pub schedule_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
