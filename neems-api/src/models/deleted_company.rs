use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::deleted_companies)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeletedCompany {
    pub id: i32,
    pub name: String,
    pub deleted_at: NaiveDateTime,
    pub deleted_by: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Insertable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::deleted_companies)]
pub struct NewDeletedCompany {
    pub id: i32,
    pub name: String,
    pub deleted_by: Option<i32>,
    // deleted_at uses database default (CURRENT_TIMESTAMP)
}