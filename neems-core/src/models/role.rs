use crate::schema::roles;
use diesel::{Identifiable, Insertable, Queryable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Queryable, Identifiable, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Role {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Insertable, Debug, Deserialize, Serialize, TS)]
#[diesel(table_name = roles)]
#[ts(export)]
pub struct NewRole {
    pub name: String,
    pub description: Option<String>,
}
