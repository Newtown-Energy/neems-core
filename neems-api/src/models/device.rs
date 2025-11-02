use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::devices;

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
#[diesel(belongs_to(crate::models::company::Company))]
#[diesel(belongs_to(crate::models::site::Site))]
#[diesel(table_name = devices)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct Device {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    #[ts(type = "string")]
    pub type_: String,
    pub model: String,
    pub serial: Option<String>,
    pub ip_address: Option<String>,
    #[ts(type = "string | null")]
    pub install_date: Option<chrono::NaiveDateTime>,
    pub company_id: i32,
    pub site_id: i32,
}

#[derive(Insertable)]
#[diesel(table_name = devices)]
pub struct NewDevice {
    pub name: String,
    pub description: Option<String>,
    pub type_: String,
    pub model: String,
    pub serial: Option<String>,
    pub ip_address: Option<String>,
    pub install_date: Option<chrono::NaiveDateTime>,
    pub company_id: i32,
    pub site_id: i32,
}

// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct DeviceInput {
    pub name: Option<String>, // Optional, will default to type if not provided
    pub description: Option<String>,
    #[ts(type = "string")]
    pub type_: String,
    pub model: String,
    pub serial: Option<String>,
    pub ip_address: Option<String>,
    #[ts(type = "string | null")]
    pub install_date: Option<chrono::NaiveDateTime>,
    pub company_id: i32,
    pub site_id: i32,
}

// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct DeviceWithTimestamps {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    #[ts(type = "string")]
    pub type_: String,
    pub model: String,
    pub serial: Option<String>,
    pub ip_address: Option<String>,
    #[ts(type = "string | null")]
    pub install_date: Option<chrono::NaiveDateTime>,
    pub company_id: i32,
    pub site_id: i32,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
