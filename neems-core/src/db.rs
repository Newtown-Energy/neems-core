use diesel::connection::SimpleConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rocket::{Rocket, Build, fairing::AdHoc};
use rocket::figment::{
    util::map,
    value::{Map, Value},
};
use rocket_sync_db_pools::{database, diesel};

use crate::api;
use crate::auth;
use crate::institution;
use crate::role;
use crate::user;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);

pub fn run_migrations_fairing() -> AdHoc {
    AdHoc::on_ignite("Diesel Migrations", |rocket| async {
        // Get a database connection from Rocket's pool
        let conn = DbConn::get_one(&rocket).await
            .expect("database connection for migration");
        // Run migrations on that connection
        conn.run(|c| {
            c.run_pending_migrations(MIGRATIONS)
                .expect("diesel migrations");
        }).await;
        rocket
    })
}

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
	.attach(run_migrations_fairing())
	.mount("/api", api::routes())
	.mount("/api", auth::login::routes())
	.mount("/api/1", institution::routes())
	.mount("/api/1", role::routes())
	.mount("/api/1", user::routes())
}


pub fn setup_test_db() -> SqliteConnection {
    use diesel::Connection;

    let mut conn = SqliteConnection::establish(":memory:")
        .expect("Failed to create in-memory SQLite database");
    conn.batch_execute("PRAGMA foreign_keys = ON")
	.expect("Could not enable foreign keys");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Migrations failed");
    conn
}



