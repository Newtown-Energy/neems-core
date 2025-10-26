use diesel::{Identifiable, Insertable, Queryable, QueryableByName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Queryable, Identifiable, QueryableByName, Debug, Serialize, TS)]
#[diesel(table_name = crate::schema::companies)]
#[ts(export)]
pub struct Company {
    pub id: i32,
    pub name: String,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = crate::schema::companies)]
pub struct NewCompany {
    pub name: String,
}

// For API inputs and validation
#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CompanyInput {
    pub name: String,
}

// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CompanyWithTimestamps {
    pub id: i32,
    pub name: String,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
