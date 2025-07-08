use crate::schema::users;
use diesel::{Identifiable, Queryable, Insertable, Selectable};
use serde::{Serialize, Deserialize};

#[derive(Queryable, Identifiable, Debug, Serialize)]
pub struct User {
    pub id: i32,
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
