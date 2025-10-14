use crate::schema::{command_sets, command_set_commands};
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A reusable sequence or workflow of commands (may include logic, delays, and conditions)
#[derive(
    Queryable, Selectable, Identifiable, Associations, QueryableByName, Debug, Serialize, Deserialize, TS,
)]
#[diesel(belongs_to(crate::models::site::Site))]
#[diesel(table_name = command_sets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct CommandSet {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = command_sets)]
pub struct NewCommandSet {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
}

/// Junction table linking CommandSets to Commands with execution order
#[derive(
    Queryable, Selectable, Identifiable, Associations, QueryableByName, Debug, Serialize, Deserialize, TS,
)]
#[diesel(belongs_to(crate::models::command_set::CommandSet))]
#[diesel(belongs_to(crate::models::command::Command))]
#[diesel(table_name = command_set_commands)]
#[diesel(primary_key(command_set_id, command_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct CommandSetCommand {
    pub command_set_id: i32,
    pub command_id: i32,
    /// Order in which commands should be executed
    pub execution_order: i32,
    /// Optional delay in milliseconds before executing this command
    pub delay_ms: Option<i32>,
    /// Optional condition that must be met for this command to execute (JSON-encoded)
    pub condition: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = command_set_commands)]
pub struct NewCommandSetCommand {
    pub command_set_id: i32,
    pub command_id: i32,
    pub execution_order: i32,
    pub delay_ms: Option<i32>,
    pub condition: Option<String>,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CommandSetInput {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
}

/// For API inputs when adding commands to a command set
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CommandSetCommandInput {
    pub command_id: i32,
    pub execution_order: i32,
    pub delay_ms: Option<i32>,
    pub condition: Option<String>,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CommandSetWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
