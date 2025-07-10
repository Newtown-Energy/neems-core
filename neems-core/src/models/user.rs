use crate::schema::users;
use diesel::{Identifiable, Queryable, Insertable};
use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;

#[derive(Deserialize, Queryable, Identifiable, Debug, Serialize)]
pub struct User {
    pub id: Option<i32>,  // Nullable in schema
    pub username: String,  // Will be unique
    pub email: String,     // Will be unique
    pub password_hash: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub institution_id: i32,
    pub totp_secret: String,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub institution_id: i32,
    pub totp_secret: String,
}

#[derive(Deserialize)]
pub struct UserNoTime {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub institution_id: i32,
    pub totp_secret: String,
}
