use rocket_sync_db_pools::{database, diesel};

#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);
