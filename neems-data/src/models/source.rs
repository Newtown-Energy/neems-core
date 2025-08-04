use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::sources;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = sources)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Source {
    pub id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub interval_seconds: i32,
    pub last_run: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = sources)]
pub struct NewSource {
    pub name: String,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub interval_seconds: Option<i32>,
}

#[derive(Debug, Clone, AsChangeset, Serialize, Deserialize)]
#[diesel(table_name = sources)]
pub struct UpdateSource {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub active: Option<bool>,
    pub interval_seconds: Option<i32>,
    pub last_run: Option<Option<NaiveDateTime>>,
}
