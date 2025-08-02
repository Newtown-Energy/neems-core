use crate::schema::sites;
use chrono::NaiveDateTime;
use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(
    Queryable, Identifiable, Associations, QueryableByName, Debug, Serialize, Deserialize, TS,
)]
#[diesel(belongs_to(crate::models::company::Company))]
#[diesel(table_name = sites)]
#[ts(export)]
pub struct Site {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32, // Foreign key to Company
    #[ts(type = "string")]
    pub created_at: NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = sites)]
pub struct NewSite {
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}
