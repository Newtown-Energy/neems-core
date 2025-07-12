#[macro_use]
extern crate rocket;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use dotenvy::dotenv;
use rocket::{Rocket, Build};
use rocket::fairing::AdHoc;
use rocket::fs::FileServer;
use rocket::serde::json::{Json, json, Value};
use rocket::request::Request;

use crate::institution::{get_institution_by_name, insert_institution};
use crate::models::InstitutionNoTime;

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

pub fn admin_init_fairing() -> AdHoc {
    AdHoc::try_on_ignite("Admin User Initialization", |rocket| async {
        // Load .env variables
        dotenv().ok();

        // Get a database connection from the pool
        let conn = match DbConn::get_one(&rocket).await {
            Some(conn) => conn,
            None => {
                eprintln!("[admin-init] ERROR: Could not get DB connection.");
                return Err(rocket);
            }
        };

        // Run institution logic in the connection
        let result = conn.run(|c| {
            // Candidate institution names
            let candidate_names = [
                "Newtown Energy",
                "Newtown Energy, Inc",
                "Newtown Energy, Inc.",
            ];

            // Try each candidate name
            for cand in candidate_names {
                let inst_no_time = InstitutionNoTime { name: cand.to_string() };
                match get_institution_by_name(c, &inst_no_time) {
                    Ok(Some(found)) => {
                        println!("[admin-init] Matched institution: '{}'", cand);
                        return Ok(found);
                    }
                    Ok(None) => continue,
                    Err(e) => {
                        eprintln!("[admin-init] ERROR querying institution '{}': {:?}", cand, e);
                        return Err(e);
                    }
                }
            }

            // None found, create "Newtown Energy"
            println!("[admin-init] No matching institution found. Creating 'Newtown Energy'.");
            match insert_institution(c, "Newtown Energy".to_string()) {
                Ok(inst) => Ok(inst),
                Err(e) => {
                    eprintln!("[admin-init] ERROR creating institution: {:?}", e);
                    Err(e)
                }
            }
        }).await;

        match result {
            Ok(_institution) => {
                // You can pass _institution to the next step (role/user) if you wish
                Ok(rocket)
            }
            Err(e) => {
                eprintln!("[admin-init] FATAL: Admin institution step failed: {:?}", e);
                Err(rocket)
            }
        }
    })
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

#[launch]
pub fn rocket() -> Rocket<Build> {

    let rocket = rocket::build()
	.attach(DbConn::fairing())
	.attach(admin_init_fairing())
        .register("/", catchers![not_found]);

    mount_api_routes(rocket).mount("/", FileServer::from("static").rank(10))
}
