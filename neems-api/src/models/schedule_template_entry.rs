use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::schedule_template_entries;

/// A template entry defining a scheduled event for a template
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
#[diesel(belongs_to(crate::models::schedule_template::ScheduleTemplate, foreign_key = template_id))]
#[diesel(belongs_to(crate::models::schedule_command::ScheduleCommand))]
#[diesel(table_name = schedule_template_entries)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct ScheduleTemplateEntry {
    pub id: i32,
    /// The template this entry belongs to
    pub template_id: i32,
    /// Seconds offset from schedule start time when this entry should execute
    pub execution_offset_seconds: i32,
    /// Reference to the schedule command to execute
    pub schedule_command_id: i32,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schedule_template_entries)]
pub struct NewScheduleTemplateEntry {
    pub template_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ScheduleTemplateEntryInput {
    pub template_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ScheduleTemplateEntryWithTimestamps {
    pub id: i32,
    pub template_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
