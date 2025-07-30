use crate::schema::users;
use crate::models::Role;
use diesel::{Identifiable, Queryable, Insertable, QueryableByName};
use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use ts_rs::TS;

#[derive(Deserialize, Queryable, Identifiable, QueryableByName, Debug, Serialize, TS)]
#[diesel(table_name = users)]
#[ts(export)]
pub struct User {
    pub id: i32,
    pub email: String,     // Will be unique
    pub password_hash: String,
    #[ts(type = "string")]
    pub created_at: NaiveDateTime,
    #[ts(type = "string")]
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

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UserNoTime {
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
}

#[derive(Deserialize, Debug, Serialize, TS)]
#[ts(export)]
pub struct UserWithRoles {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    #[ts(type = "string")]
    pub created_at: NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: NaiveDateTime,
    pub company_id: i32,
    pub totp_secret: Option<String>,
    pub roles: Vec<Role>,
}
