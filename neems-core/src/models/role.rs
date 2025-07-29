use diesel::{Queryable, Identifiable, Insertable};
use serde::{Serialize, Deserialize};
use crate::schema::roles;

#[derive(Queryable, Identifiable, Debug, Serialize, Deserialize)]
pub struct Role {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Insertable, Debug, Deserialize, Serialize)]
#[diesel(table_name = roles)]
pub struct NewRole {
    pub name: String,
    pub description: Option<String>,
}

