use crate::schema::sessions;
use chrono::NaiveDateTime;
use diesel::{Identifiable, Insertable, Queryable};

#[derive(Queryable, Identifiable, Debug)]
pub struct Session {
    pub id: String, // Opaque session token (UUID or random)
    pub user_id: i32,
    pub created_at: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
    pub revoked: bool,
}

#[derive(Insertable)]
#[diesel(table_name = sessions)]
pub struct NewSession {
    pub id: String,
    pub user_id: i32,
    pub created_at: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
    pub revoked: bool,
}

pub struct SessionNoTime {
    pub id: String,
    pub user_id: i32,
    pub revoked: bool,
}
