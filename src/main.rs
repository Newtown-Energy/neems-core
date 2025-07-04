#[macro_use]
extern crate rocket;

mod api;

use rocket::fs::FileServer;

#[launch]
fn rocket() -> _ {
    rocket::build()
        // Mount /api routes first (takes precedence over static files)
        .mount("/api", routes![
            api::status,
            api::get_clients,
            api::create_client,
        ])
        // Mount static file server at root (serves everything else)
        .mount("/", FileServer::from("static").rank(10))
}
