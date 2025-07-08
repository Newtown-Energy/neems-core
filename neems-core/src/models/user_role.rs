use crate::schema::user_roles;
use diesel::{Associations, Identifiable, Insertable, Queryable};
use serde::{Deserialize, Serialize};
use crate::models::User;
use crate::models::Role;

#[derive(Queryable, Associations, Debug, Serialize)]
#[diesel(belongs_to(User, foreign_key = user_id))]
#[diesel(belongs_to(Role, foreign_key = role_id))]
#[diesel(table_name = crate::schema::user_roles)]
#[diesel(primary_key(user_id, role_id))]
pub struct UserRole {
    pub user_id: i32,
    pub role_id: i32,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = user_roles)]
pub struct NewUserRole {
    pub user_id: i32,
    pub role_id: i32,
}


