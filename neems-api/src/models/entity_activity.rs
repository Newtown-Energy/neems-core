use crate::schema::entity_activity;
use chrono::NaiveDateTime;
use diesel::{Identifiable, Insertable, Queryable, QueryableByName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Queryable, Identifiable, QueryableByName, Debug, Serialize, Deserialize, TS)]
#[diesel(table_name = entity_activity)]
#[ts(export)]
pub struct EntityActivity {
    pub id: i32,
    pub table_name: String,
    pub entity_id: i32,
    pub operation_type: String, // 'create', 'update', 'delete'
    #[ts(type = "string")]
    pub timestamp: NaiveDateTime,
    pub user_id: Option<i32>,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = entity_activity)]
pub struct NewEntityActivity {
    pub table_name: String,
    pub entity_id: i32,
    pub operation_type: String,
    pub timestamp: Option<NaiveDateTime>, // Optional to use database default
    pub user_id: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ActivityLogEntry {
    pub operation_type: String,
    #[ts(type = "string")]
    pub timestamp: NaiveDateTime,
    pub user_id: Option<i32>,
}