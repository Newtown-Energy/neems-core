use diesel::{Queryable, Identifiable, Insertable, Associations};
use serde::Serialize;
use chrono::NaiveDateTime;
use crate::schema::sites;

#[derive(Queryable, Identifiable, Associations, Debug, Serialize)]
#[diesel(belongs_to(crate::models::company::Company))]
pub struct Site {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,  // Foreign key to Company
    pub created_at: NaiveDateTime,
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
