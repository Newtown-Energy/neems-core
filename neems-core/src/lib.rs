#[macro_use]
extern crate rocket;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use rocket::{Rocket, Build};
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::request::Request;

pub mod admin_init_fairing;
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

pub fn mount_api_routes(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket
        .mount("/api", routes![auth::logout::logout,])
        .mount("/api", api::routes())
        .mount("/api", auth::login::routes())
        .mount("/api", institution::routes())
        .mount("/api", role::routes())
        .mount("/api", user::routes())
}

/// Note that this function doesn't get tested by our tests.  Tests
/// set up the test_rocket in-memory db.  That is defined in db.rs.
#[launch]
pub fn rocket() -> Rocket<Build> {

    let rocket = rocket::build()
	.attach(DbConn::fairing())
	.attach(db::set_foreign_keys_fairing())
	.attach(db::run_migrations_fairing())
	.attach(admin_init_fairing::admin_init_fairing())
        .register("/", catchers![not_found]);

    mount_api_routes(rocket).mount("/", FileServer::from("static").rank(10))
}
