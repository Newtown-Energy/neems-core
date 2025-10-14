use crate::schema::commands;
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A single, atomic action directed at equipment
#[derive(
    Queryable, Selectable, Identifiable, Associations, QueryableByName, Debug, Serialize, Deserialize, TS,
)]
#[diesel(belongs_to(crate::models::site::Site))]
#[diesel(table_name = commands)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct Command {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    /// The type of equipment this command targets (e.g., "inverter", "battery", "charger")
    pub equipment_type: String,
    /// The specific equipment identifier (e.g., "inverter-a", "battery-1")
    pub equipment_id: String,
    /// The action to perform (e.g., "turn_on", "turn_off", "set_charge_rate")
    pub action: String,
    /// JSON-encoded parameters for the command
    pub parameters: Option<String>,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = commands)]
pub struct NewCommand {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub equipment_type: String,
    pub equipment_id: String,
    pub action: String,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CommandInput {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub equipment_type: String,
    pub equipment_id: String,
    pub action: String,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CommandWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub equipment_type: String,
    pub equipment_id: String,
    pub action: String,
    pub parameters: Option<String>,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
