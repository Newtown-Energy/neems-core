use rocket::{Build, Rocket};
use rocket::figment::{
    util::map,
    value::{Map, Value},
};
use rocket_sync_db_pools::{database, diesel};

use crate::institution;
use crate::role;
use crate::user;

#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);

pub fn test_rocket() -> Rocket<Build> {
    // Configure the in-memory SQLite database
    let db_config: Map<_, Value> = map! {
        "url" => ":memory:".into(),
        "pool_size" => 10.into(),
        "timeout" => 5.into(),
    };

    // Merge DB config into Rocket's figment
    let figment = rocket::Config::figment()
        .merge(("databases", map!["sqlite_db" => db_config]));

    // Build the Rocket instance with the DB fairing attached
    rocket::custom(figment)
        .attach(DbConn::fairing())
	.mount("/api/1", institution::routes())
	.mount("/api/1", role::routes())
	.mount("/api/1", user::routes())
}


