use chrono::NaiveDateTime;
use diesel::{Identifiable, Queryable, Insertable};
use serde::{Serialize, Deserialize};

#[derive(Deserialize, Queryable, Identifiable, Debug, Serialize)]
#[diesel(table_name = crate::schema::institutions)]
pub struct Institution {
    pub id: i32,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = crate::schema::institutions)]
pub struct NewInstitution {
    pub name: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct InstitutionName {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InstitutionNoTime {
    pub name: String,
}
