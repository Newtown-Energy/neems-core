//! API endpoints for managing roles.
//!
//! This module provides HTTP endpoints for creating and listing roles
//! in the system. Roles define permissions and access levels that can
//! be assigned to users within institutions.

use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::Route;

use crate::session_guards::AuthenticatedUser;
use crate::orm::DbConn;
use crate::orm::role::{insert_role, get_all_roles};
use crate::models::{Role, NewRole};

/// Creates a new role in the system.
///
/// This endpoint accepts a JSON payload containing the role information and
/// creates a new role record in the database.
///
/// # Arguments
/// * `db` - Database connection pool
/// * `new_role` - JSON payload containing the new role data
///
/// # Returns
/// * `Ok(Json<Role>)` - Successfully created role
/// * `Err(Status)` - Error during creation (typically InternalServerError)
#[post("/1/roles", data = "<new_role>")]
pub async fn create_role(
    db: DbConn,
    new_role: Json<NewRole>,
    _auth_user: AuthenticatedUser
) -> Result<Json<Role>, Status> {
    db.run(move |conn| {
        insert_role(conn, new_role.into_inner())
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

/// Lists all roles in the system.
///
/// This endpoint retrieves all roles from the database and returns them
/// as a JSON array.
///
/// # Arguments
/// * `db` - Database connection pool
///
/// # Returns
/// * `Ok(Json<Vec<Role>>)` - List of all roles
/// * `Err(Status)` - Error during retrieval (typically InternalServerError)
#[get("/1/roles")]
pub async fn list_roles(
    db: DbConn,
    _auth_user: AuthenticatedUser
) -> Result<Json<Vec<Role>>, Status> {
    db.run(|conn| {
        get_all_roles(conn)
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
/// A vector containing all route handlers for role endpoints
pub fn routes() -> Vec<Route> {
    routes![create_role, list_roles]
}