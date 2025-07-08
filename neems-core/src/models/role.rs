use diesel::{Queryable, Identifiable, Insertable, Associations};
use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use crate::schema::roles;

#[derive(Queryable, Identifiable, Debug, Serialize, Deserialize)]
pub struct Role {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = roles)]
pub struct NewRole {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Associations, Identifiable, Queryable, Debug)]
#[diesel(belongs_to(crate::models::user::User))]
#[diesel(belongs_to(Role))]
#[diesel(table_name = user_roles)]
pub struct UserRole {
    pub user_id: i32,
    pub role_id: i32,
}
