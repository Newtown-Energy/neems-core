use crate::schema::scheduler_scripts;
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(
    Queryable, Selectable, Identifiable, Associations, QueryableByName, Debug, Serialize, Deserialize, TS,
)]
#[diesel(belongs_to(crate::models::site::Site))]
#[diesel(table_name = scheduler_scripts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct SchedulerScript {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub script_content: String,
    pub language: String,
    pub is_active: bool,
    pub version: i32,
}

#[derive(Insertable)]
#[diesel(table_name = scheduler_scripts)]
pub struct NewSchedulerScript {
    pub site_id: i32,
    pub name: String,
    pub script_content: String,
    pub language: String,
    pub is_active: bool,
    pub version: i32,
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct SchedulerScriptInput {
    pub site_id: i32,
    pub name: String,
    pub script_content: String,
    pub language: Option<String>, // Optional, defaults to 'lua'
    pub is_active: Option<bool>,  // Optional, defaults to true
    pub version: Option<i32>,     // Optional, defaults to 1
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UpdateSchedulerScriptRequest {
    pub name: Option<String>,
    pub script_content: Option<String>,
    pub language: Option<String>,
    pub is_active: Option<bool>,
    pub version: Option<i32>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SchedulerScriptWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub script_content: String,
    pub language: String,
    pub is_active: bool,
    pub version: i32,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}

impl From<SchedulerScriptInput> for NewSchedulerScript {
    fn from(input: SchedulerScriptInput) -> Self {
        Self {
            site_id: input.site_id,
            name: input.name,
            script_content: input.script_content,
            language: input.language.unwrap_or_else(|| "lua".to_string()),
            is_active: input.is_active.unwrap_or(true),
            version: input.version.unwrap_or(1),
        }
    }
}