#[macro_use]
extern crate rocket;

pub mod api;  // Make api module public

use rocket::Build;
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::request::Request;
use rocket::Rocket;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Go up one level from crate dir to get workspace root
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

    let static_dir = workspace_root().join("static");
    // let template_dir = workspace_root().join("templates"); 

    rocket::build()
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
