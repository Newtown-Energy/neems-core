use crate::schema::users;
use diesel::{Identifiable, Queryable, Insertable};
use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;

#[derive(Deserialize, Queryable, Identifiable, Debug, Serialize)]
pub struct User {
    pub id: i32,
    pub email: String,     // Will be unique
    pub password_hash: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub company_id: i32,
    pub totp_secret: Option<String>,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub company_id: i32,
    pub totp_secret: Option<String>,
}

#[derive(Deserialize)]
pub struct UserNoTime {
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
}
