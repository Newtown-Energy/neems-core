use diesel::{Identifiable, Insertable, Queryable, QueryableByName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{models::Role, schema::users};

#[derive(Deserialize, Queryable, Identifiable, QueryableByName, Debug, Serialize, TS)]
#[diesel(table_name = users)]
#[ts(export)]
pub struct User {
    pub id: i32,
    pub email: String, // Will be unique
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
}

// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UserInput {
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
}

// User with roles but no timestamps (for internal use)
#[derive(Deserialize, Debug, Serialize, TS)]
#[ts(export)]
pub struct UserWithRoles {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
    pub roles: Vec<Role>,
}

// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct UserWithTimestamps {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}

// User with roles AND timestamps (for complete API responses)
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct UserWithRolesAndTimestamps {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
    pub roles: Vec<Role>,
}
