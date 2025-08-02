use chrono::NaiveDateTime;
use diesel::{Identifiable, Insertable, Queryable, QueryableByName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Queryable, Identifiable, QueryableByName, Debug, Serialize, TS)]
#[diesel(table_name = crate::schema::companies)]
#[ts(export)]
pub struct Company {
    pub id: i32,
    pub name: String,
    #[ts(type = "string")]
    pub created_at: NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = crate::schema::companies)]
pub struct NewCompany {
    pub name: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CompanyName {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CompanyNoTime {
    pub name: String,
}
