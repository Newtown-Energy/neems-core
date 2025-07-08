#[macro_use]
extern crate rocket;

pub mod api;
pub mod models; 
pub mod schema;  

use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rocket::Build;
use rocket::Config;
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::request::Request;
use rocket::Rocket;
use std::path::PathBuf;
use rocket::figment::{Figment, providers::{Env, Format, Toml}};
use figment_file_provider_adapter::FileAdapter;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent()
        .expect("Failed to get workspace root")
        .to_path_buf()
}

#[catch(404)]
fn not_found(req: &Request) -> Json<Value> {
    Json(json!({
        "error": "Not Found",
        "path": req.uri().path().to_string(),
        "status": 404
    }))
}

#[launch]
pub fn rocket() -> Rocket<Build> {
    // Build our figment configuration
    let figment = Figment::from(Config::default())
        // Merge with Rocket.toml, supporting _FILE suffixes
        .merge(FileAdapter::wrap(Toml::file("Rocket.toml")))
        // Merge with environment variables, supporting _FILE suffixes
        .merge(FileAdapter::wrap(Env::prefixed("ROCKET_")));

    let static_dir = workspace_root().join("static");

    rocket::custom(figment)
        .register("/", catchers![not_found])
	// Mount /api routes first (takes precedence over static files)
        .mount("/api", routes![
            api::health_status,
            api::get_clients,
            api::create_client,
            api::encode_fixphrase
        ])
        // Mount static file server at root (serves everything else)
        .mount("/", FileServer::from(static_dir).rank(10))
}

#[cfg(test)]
pub fn establish_test_connection() -> diesel::SqliteConnection {
    use diesel::Connection;
    use diesel::sqlite::SqliteConnection;
    use diesel::connection::SimpleConnection;

    let mut conn = SqliteConnection::establish(":memory:")
        .expect("Could not create test database");

    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Could not enable foreign keys");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    conn
}

// Pull in other tests
mod tests {
    mod test_schema;
}
