//! API endpoints for managing institutions.
//!
//! This module provides HTTP endpoints for creating and listing institutions
//! in the system. Institutions represent organizations or entities that can
//! be associated with users and roles.

use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::response::status;
use rocket::Route;

use crate::auth::session_guard::AuthenticatedUser;
use crate::orm::DbConn;
use crate::models::{Institution, InstitutionName};
use crate::institution::insert_institution;
use crate::orm::institution::get_all_institutions;

/// Creates a new institution in the system.
///
/// This endpoint accepts a JSON payload containing the institution name and
/// creates a new institution record in the database.
///
/// # Arguments
/// * `db` - Database connection pool
/// * `new_institution` - JSON payload containing the institution name
///
/// # Returns
/// * `Ok(status::Created<Json<Institution>>)` - Successfully created institution
/// * `Err(Status)` - Error during creation (typically InternalServerError)
#[post("/1/institutions", data = "<new_institution>")]
pub async fn create_institution(
    db: DbConn,
    new_institution: Json<InstitutionName>,
    _auth_user: AuthenticatedUser
) -> Result<status::Created<Json<Institution>>, Status> {
    db.run(move |conn| {
        insert_institution(conn, new_institution.name.clone())
            .map(|inst| status::Created::new("/").body(Json(inst)))
            .map_err(|e| {
                eprintln!("Error creating institution: {:?}", e);
                Status::InternalServerError
            })
    }).await
}

/// Lists all institutions in the system.
///
/// This endpoint retrieves all institutions from the database and returns them
/// as a JSON array, ordered by ID in ascending order.
///
/// # Arguments
/// * `db` - Database connection pool
///
/// # Returns
/// * `Ok(Json<Vec<Institution>>)` - List of all institutions
/// * `Err(Status)` - Error during retrieval (typically InternalServerError)
#[get("/1/institutions")]
pub async fn list_institutions(
    db: DbConn,
    _auth_user: AuthenticatedUser
) -> Result<Json<Vec<Institution>>, Status> {
    db.run(|conn| {
        get_all_institutions(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for institution endpoints
pub fn routes() -> Vec<Route> {
    routes![create_institution, list_institutions]
}
