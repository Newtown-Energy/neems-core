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
use crate::models::{Company, CompanyName, UserWithRoles};
use crate::company::insert_company;
use crate::orm::company::{get_all_companies, delete_company};
use crate::orm::site::get_sites_by_company;
use crate::orm::user::get_users_by_company_with_roles;
use crate::models::Site;

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

/// List Company Sites endpoint.
///
/// - **URL:** `/api/1/company/<company_id>/sites`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all sites for a specific company
/// - **Authentication:** Required
/// - **Authorization:** Users can see sites if they work for the company OR have newtown-admin/newtown-staff roles
///
/// This endpoint retrieves all sites belonging to a specific company from the database.
/// Access is restricted based on business rules:
/// - Users can see sites for their own company (same company_id)
/// - Users with 'newtown-admin' or 'newtown-staff' roles can see any company's sites
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "Main Office",
///     "address": "123 Main St, City, State",
///     "latitude": 40.7128,
///     "longitude": -74.0060,
///     "company_id": 1,
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   }
/// ]
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to view sites for this company
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error during retrieval
///
/// # Arguments
/// * `db` - Database connection pool
/// * `company_id` - The ID of the company whose sites to retrieve
/// * `auth_user` - Authenticated user for authorization
///
/// # Returns
/// * `Ok(Json<Vec<Site>>)` - List of sites for the company
/// * `Err(Status)` - Error during retrieval or authorization failure
#[get("/1/company/<company_id>/sites")]
pub async fn list_company_sites(
    db: DbConn,
    company_id: i32,
    auth_user: AuthenticatedUser
) -> Result<Json<Vec<Site>>, Status> {
    // Check authorization: user must be in the same company OR have newtown admin/staff roles
    let has_access = auth_user.user.company_id == company_id 
        || auth_user.has_any_role(&["newtown-admin", "newtown-staff"]);
    
    if !has_access {
        return Err(Status::Forbidden);
    }
    
    db.run(move |conn| {
        get_sites_by_company(conn, company_id)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

/// List Company Users endpoint.
///
/// - **URL:** `/api/1/company/<company_id>/users`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all users for a specific company
/// - **Authentication:** Required
/// - **Authorization:** Users can see users if they work for the company OR have newtown-admin/newtown-staff roles
///
/// This endpoint retrieves all users belonging to a specific company from the database.
/// Access is restricted based on business rules:
/// - Users can see users for their own company (same company_id)
/// - Users with 'newtown-admin' or 'newtown-staff' roles can see any company's users
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 123,
///     "email": "user@example.com",
///     "password_hash": "hashed_password_string",
///     "company_id": 1,
///     "totp_secret": "optional_totp_secret",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   }
/// ]
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to view users for this company
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error during retrieval
///
/// # Arguments
/// * `db` - Database connection pool
/// * `company_id` - The ID of the company whose users to retrieve
/// * `auth_user` - Authenticated user for authorization
///
/// # Returns
/// * `Ok(Json<Vec<User>>)` - List of users for the company
/// * `Err(Status)` - Error during retrieval or authorization failure
#[get("/1/company/<company_id>/users")]
pub async fn list_company_users(
    db: DbConn,
    company_id: i32,
    auth_user: AuthenticatedUser
) -> Result<Json<Vec<UserWithRoles>>, Status> {
    // Check authorization: user must be in the same company OR have newtown admin/staff roles
    let has_access = auth_user.user.company_id == company_id 
        || auth_user.has_any_role(&["newtown-admin", "newtown-staff"]);
    
    if !has_access {
        return Err(Status::Forbidden);
    }
    
    db.run(move |conn| {
        get_users_by_company_with_roles(conn, company_id)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

/// Delete Company endpoint.
///
/// - **URL:** `/api/1/companies/<company_id>`
/// - **Method:** `DELETE`
/// - **Purpose:** Deletes a company by ID
/// - **Authentication:** Required
///
/// This endpoint deletes a company from the database by its ID.
///
/// # Response
///
/// **Success (HTTP 204 No Content):**
/// Company was successfully deleted
///
/// **Failure (HTTP 404 Not Found):**
/// Company with the specified ID was not found
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error during deletion
///
/// # Arguments
/// * `db` - Database connection pool
/// * `company_id` - The ID of the company to delete
///
/// # Returns
/// * `Ok(Status::NoContent)` - Successfully deleted company
/// * `Err(Status)` - Error during deletion (NotFound or InternalServerError)
#[delete("/1/companies/<company_id>")]
pub async fn delete_company_endpoint(
    db: DbConn,
    company_id: i32,
    _auth_user: AuthenticatedUser
) -> Result<Status, Status> {
    db.run(move |conn| {
        delete_company(conn, company_id)
            .map(|found| {
                if found {
                    Status::NoContent
                } else {
                    Status::NotFound
                }
            })
            .map_err(|e| {
                eprintln!("Error deleting company: {:?}", e);
                Status::InternalServerError
            })
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
    routes![create_company, list_companies, list_company_sites, list_company_users, delete_company_endpoint]
}