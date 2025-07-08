use chrono::NaiveDateTime;
use diesel::{Queryable, Identifiable, Insertable, Associations};
use serde::{Serialize, Deserialize};
use crate::schema::roles;
use crate::models::user::User;

#[derive(Queryable, Identifiable, Debug, Serialize, Deserialize)]
pub struct Role {
    pub id: Option<i32>,  // Nullable in schema
    pub name: String,
    pub description: Option<String>,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = roles)]
pub struct NewRole {
    pub name: String,
    pub description: Option<String>,
}

