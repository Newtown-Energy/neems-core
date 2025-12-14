use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::{schedule_commands, schedule_template_entries, schedule_templates};

/// Command type for battery operations
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    Charge,
    Discharge,
    TrickleCharge,
}

/// Database model for schedule commands
#[derive(
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    QueryableByName,
    Debug,
    Clone,
    Serialize,
    Deserialize,
)]
#[diesel(belongs_to(super::site::Site))]
#[diesel(table_name = schedule_commands)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ScheduleCommand {
    pub id: i32,
    pub site_id: i32,
    #[serde(rename = "type")]
    pub type_: String,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// Insertable struct for creating new schedule commands
#[derive(Insertable, Debug)]
#[diesel(table_name = schedule_commands)]
pub struct NewScheduleCommand {
    pub site_id: i32,
    pub type_: String,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// Database model for schedule templates (library items)
#[derive(
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    QueryableByName,
    Debug,
    Clone,
    Serialize,
    Deserialize,
)]
#[diesel(belongs_to(super::site::Site))]
#[diesel(table_name = schedule_templates)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ScheduleTemplate {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: chrono::NaiveDateTime,
    pub is_default: bool,
}

/// Insertable struct for creating new schedule templates
#[derive(Insertable, Debug)]
#[diesel(table_name = schedule_templates)]
pub struct NewScheduleTemplate {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub is_default: bool,
}

/// Database model for schedule template entries
#[derive(
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    QueryableByName,
    Debug,
    Clone,
    Serialize,
    Deserialize,
)]
#[diesel(belongs_to(ScheduleTemplate, foreign_key = template_id))]
#[diesel(belongs_to(ScheduleCommand, foreign_key = schedule_command_id))]
#[diesel(table_name = schedule_template_entries)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ScheduleTemplateEntry {
    pub id: i32,
    pub template_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
}

/// Insertable struct for creating new schedule template entries
#[derive(Insertable, Debug)]
#[diesel(table_name = schedule_template_entries)]
pub struct NewScheduleTemplateEntry {
    pub template_id: i32,
    pub execution_offset_seconds: i32,
    pub schedule_command_id: i32,
    pub is_active: bool,
}

// ============================================================================
// API Models (exported to TypeScript)
// ============================================================================

/// A single command within a schedule (API model)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScheduleCommandDto {
    pub id: i32,
    pub execution_offset_seconds: i32,
    pub command_type: CommandType,
}

/// A schedule library item (template with embedded commands)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScheduleLibraryItem {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub commands: Vec<ScheduleCommandDto>,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
}

/// Request to create a new library item
#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateLibraryItemRequest {
    pub name: String,
    pub description: Option<String>,
    pub commands: Vec<CreateCommandRequest>,
}

/// Command data for creating/updating
#[derive(Debug, Deserialize, Serialize, TS, Clone)]
#[ts(export)]
pub struct CreateCommandRequest {
    pub execution_offset_seconds: i32,
    pub command_type: CommandType,
}

/// Request to update a library item
#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UpdateLibraryItemRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub commands: Option<Vec<CreateCommandRequest>>,
}

/// Request to clone a library item
#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CloneLibraryItemRequest {
    pub name: String,
    pub description: Option<String>,
}

// Helper function to convert CommandType to string for database
impl CommandType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommandType::Charge => "charge",
            CommandType::Discharge => "discharge",
            CommandType::TrickleCharge => "trickle_charge",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "charge" => Ok(CommandType::Charge),
            "discharge" => Ok(CommandType::Discharge),
            "trickle_charge" => Ok(CommandType::TrickleCharge),
            _ => Err(format!("Unknown command type: {}", s)),
        }
    }
}
