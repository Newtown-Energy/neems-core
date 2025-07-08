use diesel::{Queryable, Identifiable, Insertable, Associations};
use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use crate::schema::sites;

#[derive(Queryable, Identifiable, Associations, Debug, Serialize)]
#[diesel(belongs_to(crate::models::institution::Institution))]
pub struct Site {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub institution_id: i32,  // Foreign key to Institution
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = sites)]
pub struct NewSite {
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub institution_id: i32,
}
