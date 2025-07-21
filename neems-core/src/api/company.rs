//! API endpoints for managing companies.
//!
//! This module provides HTTP endpoints for creating and listing companies
//! in the system. Companies represent organizations or entities that can
//! be associated with users and roles.

use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::response::status;
use rocket::Route;

use crate::session_guards::AuthenticatedUser;
use crate::orm::DbConn;
use crate::models::{Company, CompanyName};
use crate::company::insert_company;
use crate::orm::company::get_all_companies;

/// Create Company endpoint.
///
/// - **URL:** `/api/1/companies`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new company in the system
/// - **Authentication:** Required
///
/// This endpoint accepts a JSON payload containing the company name and
/// creates a new company record in the database.
///
/// # Request Format
///
/// ```json
/// {
///   "name": "Example University"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 201 Created):**
/// ```json
/// {
///   "id": 1,
///   "name": "Example University",
///   "created_at": "2023-01-01T00:00:00Z",
///   "updated_at": "2023-01-01T00:00:00Z"
/// }
/// ```
///
/// # Arguments
/// * `db` - Database connection pool
/// * `new_company` - JSON payload containing the company name
///
/// # Returns
/// * `Ok(status::Created<Json<Company>>)` - Successfully created company
/// * `Err(Status)` - Error during creation (typically InternalServerError)
#[post("/1/companies", data = "<new_company>")]
pub async fn create_company(
    db: DbConn,
    new_company: Json<CompanyName>,
    _auth_user: AuthenticatedUser
) -> Result<status::Created<Json<Company>>, Status> {
    db.run(move |conn| {
        insert_company(conn, new_company.name.clone())
            .map(|comp| status::Created::new("/").body(Json(comp)))
            .map_err(|e| {
                eprintln!("Error creating company: {:?}", e);
                Status::InternalServerError
            })
    }).await
}

/// List Companies endpoint.
///
/// - **URL:** `/api/1/companies`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all companies in the system (ordered by ID)
/// - **Authentication:** Required
///
/// This endpoint retrieves all companies from the database and returns them
/// as a JSON array, ordered by ID in ascending order.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "Example University",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   },
///   {
///     "id": 2,
///     "name": "Another Company",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   }
/// ]
/// ```
///
/// # Arguments
/// * `db` - Database connection pool
///
/// # Returns
/// * `Ok(Json<Vec<Company>>)` - List of all companies
/// * `Err(Status)` - Error during retrieval (typically InternalServerError)
#[get("/1/companies")]
pub async fn list_companies(
    db: DbConn,
    _auth_user: AuthenticatedUser
) -> Result<Json<Vec<Company>>, Status> {
    db.run(|conn| {
        get_all_companies(conn)
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
/// A vector containing all route handlers for company endpoints
pub fn routes() -> Vec<Route> {
    routes![create_company, list_companies]
}