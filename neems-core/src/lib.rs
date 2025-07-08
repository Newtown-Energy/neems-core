#[macro_use]
extern crate rocket;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use rocket::Build;
use rocket::Config;
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::request::Request;
use rocket::Rocket;
use std::path::PathBuf;
use rocket::figment::{Figment, providers::{Env, Format, Toml}};
use figment_file_provider_adapter::FileAdapter;

pub mod api;
pub mod auth;
pub mod db;
pub use db::DbConn;
pub mod institution; 
pub mod models; 
pub mod role;
pub mod schema;  
pub mod user;

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
        .merge(FileAdapter::wrap(Toml::file("rocket.toml")))
        .merge(FileAdapter::wrap(Toml::file("../rocket.toml")))
        // Merge with environment variables, supporting _FILE suffixes
        .merge(FileAdapter::wrap(Env::prefixed("ROCKET_")));

    let static_dir = workspace_root().join("static");

    rocket::custom(figment)
	.attach(DbConn::fairing())
        .register("/", catchers![not_found])
	// Mount /api routes first (takes precedence over static files)
        .mount("/api", routes![
            api::health_status,
            api::get_clients,
            api::create_client,
            api::encode_fixphrase,
	    auth::login::login,
	    auth::login::secure_hello,
	    auth::logout::logout,
        ])
        // Mount static file server at root (serves everything else)
        .mount("/", FileServer::from(static_dir).rank(10))
}

pub fn test_rocket() -> Rocket<Build> {
    use rocket::figment::{Figment, providers::Serialized};
    use rocket::Config;
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Deserialize, Serialize)]
    struct Database {
        url: String,
    }
    
    #[derive(Debug, Deserialize, Serialize)]
    struct Databases {
        sqlite_db: Database,
    }
    
    #[derive(Debug, Deserialize, Serialize)]
    struct TestConfig {
        databases: Databases,
    }
    
    let test_config = TestConfig {
        databases: Databases {
            sqlite_db: Database {
                url: ":memory:".to_string(),
            },
        },
    };
    
    let figment = Figment::from(Config::default())
        .merge(Serialized::defaults(test_config));
    
    let static_dir = workspace_root().join("static");
    
    rocket::custom(figment)
        .attach(DbConn::fairing())
        .register("/", catchers![not_found])
        .mount("/api", routes![
            api::health_status,
            api::get_clients,
            api::create_client,
            api::encode_fixphrase,
            auth::login::login,
            auth::login::secure_hello,
            auth::logout::logout,
        ])
	.mount("/api/1", institution::routes())
	.mount("/api/1", role::routes())
	.mount("/api/1", user::routes())
        .mount("/", FileServer::from(static_dir).rank(10))
}

#[cfg(test)]
pub fn establish_test_connection() -> diesel::SqliteConnection {
    use diesel::Connection;
    use diesel::connection::SimpleConnection;
    use diesel::sqlite::SqliteConnection;
    use diesel_migrations::MigrationHarness;

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
