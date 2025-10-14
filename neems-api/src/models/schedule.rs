use crate::schema::schedules;
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A concrete instantiation of a template applied to a specific date (with possible overrides)
#[derive(
    Queryable, Selectable, Identifiable, Associations, QueryableByName, Debug, Serialize, Deserialize, TS,
)]
#[diesel(belongs_to(crate::models::site::Site))]
#[diesel(belongs_to(crate::models::schedule_template::ScheduleTemplate, foreign_key = template_id))]
#[diesel(table_name = schedules)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct Schedule {
    pub id: i32,
    pub site_id: i32,
    /// The template this schedule was created from (if any)
    pub template_id: Option<i32>,
    /// The date this schedule applies to (date only, no time component)
    #[ts(type = "string")]
    pub schedule_date: chrono::NaiveDate,
    /// Whether this schedule has been customized from the template
    pub is_custom: bool,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schedules)]
pub struct NewSchedule {
    pub site_id: i32,
    pub template_id: Option<i32>,
    pub schedule_date: chrono::NaiveDate,
    pub is_custom: bool,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ScheduleInput {
    pub site_id: i32,
    pub template_id: Option<i32>,
    #[ts(type = "string")]
    pub schedule_date: chrono::NaiveDate,
    pub is_custom: bool,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ScheduleWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    pub template_id: Option<i32>,
    #[ts(type = "string")]
    pub schedule_date: chrono::NaiveDate,
    pub is_custom: bool,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
