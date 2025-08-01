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
pub mod company; 
pub mod logged_json;
pub mod models; 
pub mod orm;
pub use orm::DbConn;
pub mod session_guards;
pub mod schema;

#[cfg(test)]
pub mod generate_types;  

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[catch(401)]
fn unauthorized(req: &Request) -> Json<Value> {
    Json(json!({
        "error": "Unauthorized",
        "path": req.uri().path().to_string(),
        "status": 401
    }))
}

#[catch(403)]
fn forbidden(req: &Request) -> Json<Value> {
    Json(json!({
        "error": "Forbidden",
        "path": req.uri().path().to_string(),
        "status": 403
    }))
}

#[catch(404)]
fn not_found(req: &Request) -> Json<Value> {
    Json(json!({
        "error": "Not Found",
        "path": req.uri().path().to_string(),
        "status": 404
    }))
}

#[catch(422)]
fn unprocessable_entity(req: &Request) -> Json<Value> {
    Json(json!({
        "error": "Unprocessable Entity",
        "path": req.uri().path().to_string(),
        "status": 422
    }))
}

#[catch(500)]
fn internal_server_error(req: &Request) -> Json<Value> {
    Json(json!({
        "error": "Internal Server Error",
        "path": req.uri().path().to_string(),
        "status": 500
    }))
}

#[catch(default)]
fn default_catcher(status: rocket::http::Status, req: &Request) -> Json<Value> {
    Json(json!({
        "error": status.reason().unwrap_or("Unknown Error"),
        "path": req.uri().path().to_string(),
        "status": status.code
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
        .register("/", catchers![unauthorized, forbidden, not_found, unprocessable_entity, internal_server_error, default_catcher]);

    log_rocket_info(&rocket);

    let static_dir = std::env::var("NEEMS_STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    mount_api_routes(rocket).mount("/", FileServer::from(static_dir).rank(10))
}
