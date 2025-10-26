use chrono::NaiveDateTime;
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::scheduler_executions;

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
#[diesel(belongs_to(crate::models::scheduler_script::SchedulerScript, foreign_key = script_id))]
#[diesel(belongs_to(crate::models::scheduler_override::SchedulerOverride, foreign_key = override_id))]
#[diesel(table_name = scheduler_executions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct SchedulerExecution {
    pub id: i32,
    pub site_id: i32,
    pub script_id: Option<i32>,
    pub override_id: Option<i32>,
    #[ts(type = "string")]
    pub execution_time: NaiveDateTime,
    pub state_result: String,
    pub execution_duration_ms: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = scheduler_executions)]
pub struct NewSchedulerExecution {
    pub site_id: i32,
    pub script_id: Option<i32>,
    pub override_id: Option<i32>,
    pub execution_time: NaiveDateTime,
    pub state_result: String,
    pub execution_duration_ms: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct SchedulerExecutionInput {
    pub site_id: i32,
    pub script_id: Option<i32>,
    pub override_id: Option<i32>,
    #[ts(type = "string")]
    pub execution_time: Option<NaiveDateTime>, // Optional, defaults to now
    pub state_result: String,
    pub execution_duration_ms: Option<i32>,
    pub error_message: Option<String>,
}

impl From<SchedulerExecutionInput> for NewSchedulerExecution {
    fn from(input: SchedulerExecutionInput) -> Self {
        Self {
            site_id: input.site_id,
            script_id: input.script_id,
            override_id: input.override_id,
            execution_time: input.execution_time.unwrap_or_else(|| chrono::Utc::now().naive_utc()),
            state_result: input.state_result,
            execution_duration_ms: input.execution_duration_ms,
            error_message: input.error_message,
        }
    }
}
