use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::deleted_users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeletedUser {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
    pub deleted_at: NaiveDateTime,
    pub deleted_by: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Insertable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::deleted_users)]
pub struct NewDeletedUser {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
    pub deleted_by: Option<i32>,
    // deleted_at uses database default (CURRENT_TIMESTAMP)
}
