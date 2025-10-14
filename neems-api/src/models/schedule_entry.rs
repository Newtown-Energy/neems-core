use crate::schema::schedule_entries;
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A specific scheduled event that references a Command or CommandSet, bound to a time window
#[derive(
    Queryable, Selectable, Identifiable, Associations, QueryableByName, Debug, Serialize, Deserialize, TS,
)]
#[diesel(belongs_to(crate::models::schedule::Schedule))]
#[diesel(belongs_to(crate::models::command::Command))]
#[diesel(belongs_to(crate::models::command_set::CommandSet))]
#[diesel(belongs_to(crate::models::schedule_template::ScheduleTemplate, foreign_key = template_id))]
#[diesel(table_name = schedule_entries)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct ScheduleEntry {
    pub id: i32,
    /// The schedule this entry belongs to (null for template entries)
    pub schedule_id: Option<i32>,
    /// The template this entry belongs to (null for schedule entries)
    pub template_id: Option<i32>,
    /// The time this entry should execute (time only, no date component)
    #[ts(type = "string")]
    pub execution_time: chrono::NaiveTime,
    /// Optional end time for time window
    #[ts(type = "string")]
    pub end_time: Option<chrono::NaiveTime>,
    /// Reference to a single command (mutually exclusive with command_set_id)
    pub command_id: Option<i32>,
    /// Reference to a command set (mutually exclusive with command_id)
    pub command_set_id: Option<i32>,
    /// Optional condition that must be met for this entry to execute (JSON-encoded)
    pub condition: Option<String>,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schedule_entries)]
pub struct NewScheduleEntry {
    pub schedule_id: Option<i32>,
    pub template_id: Option<i32>,
    pub execution_time: chrono::NaiveTime,
    pub end_time: Option<chrono::NaiveTime>,
    pub command_id: Option<i32>,
    pub command_set_id: Option<i32>,
    pub condition: Option<String>,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ScheduleEntryInput {
    pub schedule_id: Option<i32>,
    pub template_id: Option<i32>,
    #[ts(type = "string")]
    pub execution_time: chrono::NaiveTime,
    #[ts(type = "string")]
    pub end_time: Option<chrono::NaiveTime>,
    pub command_id: Option<i32>,
    pub command_set_id: Option<i32>,
    pub condition: Option<String>,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ScheduleEntryWithTimestamps {
    pub id: i32,
    pub schedule_id: Option<i32>,
    pub template_id: Option<i32>,
    #[ts(type = "string")]
    pub execution_time: chrono::NaiveTime,
    #[ts(type = "string")]
    pub end_time: Option<chrono::NaiveTime>,
    pub command_id: Option<i32>,
    pub command_set_id: Option<i32>,
    pub condition: Option<String>,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
