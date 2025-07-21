use chrono::NaiveDateTime;
use diesel::{Identifiable, Queryable, Insertable};
use serde::{Serialize, Deserialize};

#[derive(Deserialize, Queryable, Identifiable, Debug, Serialize)]
#[diesel(table_name = crate::schema::companies)]
pub struct Company {
    pub id: i32,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = crate::schema::companies)]
pub struct NewCompany {
    pub name: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyName {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CompanyNoTime {
    pub name: String,
}