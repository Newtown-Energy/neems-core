#[macro_use]
extern crate rocket;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use rocket::{Rocket, Build};
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::figment::value::Map;
use rocket::request::Request;

pub mod admin_init_fairing;
pub mod api;
pub mod auth;
pub mod orm;
pub use orm::DbConn;
pub mod institution; 
pub mod models; 
pub mod schema;  

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

#[catch(404)]
fn not_found(req: &Request) -> Json<Value> {
    Json(json!({
        "error": "Not Found",
        "path": req.uri().path().to_string(),
        "status": 404
    }))
}

pub fn mount_api_routes(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket
        .mount("/api", api::routes())
}

fn log_rocket_info(rocket: &Rocket<Build>) {
    // Get the figment (configuration)
    let figment = rocket.figment();

    // Log address and port from configuration
    if let Ok(address) = figment.extract_inner::<String>("address") {
        info!("Rocket is running at: {}", address);
    }

    if let Ok(port) = figment.extract_inner::<u16>("port") {
        info!("Rocket is listening on port: {}", port);
    }

    // Log the database URL with better error handling
    match figment.extract_inner::<Map<String, Value>>("databases.sqlite_db") {
        Ok(db_config) => {
            if let Some(Value::String(url)) = db_config.get("url") {
                info!("Database URL: {}", url);
            } else {
                warn!("Database URL not found in configuration");
            }
        }
        Err(e) => {
            warn!("Failed to extract database configuration: {}", e);
        }
    }
}

/// Note that this function doesn't get tested by our tests.  Tests
/// set up the test_rocket in-memory db.  That is defined in db.rs.
#[launch]
pub fn rocket() -> Rocket<Build> {

    let rocket = rocket::build()
	.attach(DbConn::fairing())
	.attach(orm::set_foreign_keys_fairing())
	.attach(orm::run_migrations_fairing())
	.attach(admin_init_fairing::admin_init_fairing())
        .register("/", catchers![not_found]);

    log_rocket_info(&rocket);

    let static_dir = std::env::var("NEEMS_STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    mount_api_routes(rocket).mount("/", FileServer::from(static_dir).rank(10))
}
