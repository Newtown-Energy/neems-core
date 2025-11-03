use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::schedule_templates;

/// A daily "plan" that defines which CommandSets (or Commands) should happen at
/// what times
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
#[diesel(table_name = schedule_templates)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct ScheduleTemplate {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    /// Whether this template is marked as a default for the site
    pub is_default: bool,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schedule_templates)]
pub struct NewScheduleTemplate {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ScheduleTemplateInput {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ScheduleTemplateWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
