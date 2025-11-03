use diesel::{
    Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable,
    deserialize::{self, FromSql},
    serialize::{self, Output, ToSql},
    sql_types::Text,
    sqlite::Sqlite,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::schedule_commands;

/// The type of battery charging command
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TS,
    diesel::expression::AsExpression,
    diesel::deserialize::FromSqlRow,
)]
#[diesel(sql_type = Text)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    Charge,
    Discharge,
    TrickleCharge,
}

impl ToSql<Text, Sqlite> for CommandType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        let s = match self {
            CommandType::Charge => "charge",
            CommandType::Discharge => "discharge",
            CommandType::TrickleCharge => "trickle_charge",
        };
        out.set_value(s);
        Ok(serialize::IsNull::No)
    }
}

impl FromSql<Text, Sqlite> for CommandType {
    fn from_sql(
        bytes: <Sqlite as diesel::backend::Backend>::RawValue<'_>,
    ) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Sqlite>>::from_sql(bytes)?;
        match s.as_str() {
            "charge" => Ok(CommandType::Charge),
            "discharge" => Ok(CommandType::Discharge),
            "trickle_charge" => Ok(CommandType::TrickleCharge),
            _ => Err(format!("Invalid CommandType value: {}", s).into()),
        }
    }
}

/// A battery charging command
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
#[diesel(table_name = schedule_commands)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct ScheduleCommand {
    pub id: i32,
    pub site_id: i32,
    /// The type of charging command
    #[serde(rename = "type")]
    pub type_: CommandType,
    /// JSON-encoded parameters for the command
    pub parameters: Option<String>,
    pub is_active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schedule_commands)]
pub struct NewScheduleCommand {
    pub site_id: i32,
    #[diesel(column_name = type_)]
    pub type_: CommandType,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ScheduleCommandInput {
    pub site_id: i32,
    #[serde(rename = "type")]
    pub type_: CommandType,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ScheduleCommandWithTimestamps {
    pub id: i32,
    pub site_id: i32,
    #[serde(rename = "type")]
    pub type_: CommandType,
    pub parameters: Option<String>,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
