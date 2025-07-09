#[macro_use]
extern crate rocket;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use rocket::Build;
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::request::Request;
use rocket::Rocket;

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
    let static_dir = "static";

    rocket::build()
	.attach(DbConn::fairing())
        .register("/", catchers![not_found])
	// Mount /api routes first (takes precedence over static files)
        .mount("/api", routes![
            api::health_status,
            api::encode_fixphrase,
	    auth::logout::logout,
        ])
	.mount("/api", api::routes())
	.mount("/api", auth::login::routes())
	.mount("/api/1", institution::routes())
	.mount("/api/1", role::routes())
	.mount("/api/1", user::routes())
        // Mount static file server at root (serves everything else)
        .mount("/", FileServer::from(static_dir).rank(10))
}
