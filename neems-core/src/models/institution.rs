use diesel::{Identifiable, Queryable, Insertable};
use chrono::NaiveDateTime;

#[derive(Queryable, Identifiable, Debug)]
pub struct Institution {
    pub id: i32,
    pub name: String,  // Will be unique
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = institutions)]
pub struct NewInstitution {
    pub name: String,
}

