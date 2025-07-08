#[macro_use]
extern crate rocket;

pub mod api;

use rocket::Build;
use rocket::Config;
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::request::Request;
use rocket::Rocket;
use std::path::PathBuf;
use rocket::figment::{Figment, providers::{Env, Format, Toml}};
use figment_file_provider_adapter::FileAdapter;

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
