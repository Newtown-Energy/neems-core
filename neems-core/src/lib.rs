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

/// Add default admin user and inst if needed.
///
/// Set the default admin email/pass based on envars NEEMS_DEFAULT_EMAIL and NEEMS_DEFAULT_PASSWORD
pub fn admin_init_fairing() -> AdHoc {
    AdHoc::try_on_ignite("Admin User Initialization", |rocket| async {
        // Load .env variables
        dotenv().ok();

        // Get a database connection from the pool.
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
            Ok(institution) => {
                // Now create the admin user
                let admin_email = std::env::var("NEEMS_DEFAULT_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
                
                let user_result = conn.run(move |c| {
                    use crate::user::*;
                    use crate::models::{User, UserNoTime};
                    use crate::schema::users::dsl::*;
                    use diesel::prelude::*;
                    
                    // Check if admin user already exists
                    let existing_user = users.filter(email.eq(&admin_email))
                        .first::<User>(c)
                        .optional()
                        .expect("user query should not fail");
                    
                    if existing_user.is_some() {
                        println!("[admin-init] Admin user '{}' already exists", admin_email);
                        return Ok(());
                    }
                    
                    // Get admin password from env or default
                    let admin_password = std::env::var("NEEMS_DEFAULT_PASSWORD").unwrap_or_else(|_| "admin".to_string());
                    
                    // Create admin user
                    let admin_user = UserNoTime {
                        email: admin_email.clone(),
                        password_hash: admin_password, // Will be hashed by insert_user
                        institution_id: institution.id.expect("must have institution id"),
                        totp_secret: "".to_string(), // Empty TOTP secret
                    };
                    
                    match insert_user(c, admin_user) {
                        Ok(user) => {
                            println!("[admin-init] Created admin user: '{}'", admin_email);
                            
                            // Now assign the newtown-admin role
                            use crate::role::*;
                            use crate::models::{Role, NewRole, NewUserRole};
                            use crate::schema::roles::dsl::*;
                            use crate::schema::user_roles;
                            
                            let role_name = "newtown-admin";
                            
                            // First, check if the role exists
                            let existing_role = roles.filter(name.eq(role_name))
                                .first::<Role>(c)
                                .optional()
                                .expect("role query should not fail");
                            
                            let role = match existing_role {
                                Some(r) => r,
                                None => {
                                    // Role doesn't exist, create it
                                    println!("[admin-init] Creating role: '{}'", role_name);
                                    let new_role = NewRole {
                                        name: role_name.to_string(),
                                        description: Some("Administrator for Newtown".to_string()),
                                    };
                                    match insert_role(c, new_role) {
                                        Ok(r) => r,
                                        Err(e) => {
                                            eprintln!("[admin-init] ERROR creating role: {:?}", e);
                                            return Err(e);
                                        }
                                    }
                                }
                            };
                            
                            // Create the user-role association
                            let new_user_role = NewUserRole {
                                user_id: user.id.expect("user must have id"),
                                role_id: role.id.expect("role must have id"),
                            };
                            
                            match diesel::insert_into(user_roles::table)
                                .values(&new_user_role)
                                .execute(c) {
                                Ok(_) => {
                                    println!("[admin-init] Assigned role '{}' to user '{}'", role_name, admin_email);
                                    Ok(())
                                }
                                Err(e) => {
                                    eprintln!("[admin-init] ERROR assigning role: {:?}", e);
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[admin-init] ERROR creating admin user: {:?}", e);
                            Err(e)
                        }
                    }
                }).await;
                
                match user_result {
                    Ok(()) => Ok(rocket),
                    Err(e) => {
                        eprintln!("[admin-init] FATAL: Admin user creation failed: {:?}", e);
                        Err(rocket)
                    }
                }
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
