//! API endpoints for site management.
//!
//! This module provides HTTP endpoints for CRUD operations on sites.
//! Sites are associated with companies and have location data.
//! 
//! # Authorization Rules
//! - Company admins can perform CRUD operations on sites within their company
//! - newtown-staff and newtown-admin roles can perform CRUD operations on any site
//! - Regular users cannot perform CRUD operations

use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::response::status;
use rocket::Route;
use serde::{Deserialize, Serialize};

use crate::logged_json::LoggedJson;
use crate::session_guards::AuthenticatedUser;
use crate::orm::DbConn;
use crate::models::Site;
use crate::orm::site::{insert_site, get_site_by_id, update_site, delete_site, get_all_sites, get_sites_by_company};

/// Request payload for creating a new site
#[derive(Deserialize, Serialize)]
pub struct CreateSiteRequest {
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,
}

/// Request payload for updating a site (all fields optional)
#[derive(Deserialize, Serialize)]
pub struct UpdateSiteRequest {
    pub name: Option<String>,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub company_id: Option<i32>,
}

/// Helper function to check if user can perform CRUD operations on a site
fn can_crud_site(user: &AuthenticatedUser, site_company_id: i32) -> bool {
    // newtown-admin and newtown-staff can CRUD any site
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }
    
    // Company admins can CRUD sites in their own company
    if user.has_role("admin") && user.user.company_id == site_company_id {
        return true;
    }
    
    false
}

/// Create Site endpoint.
///
/// - **URL:** `/api/1/sites`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new site
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or newtown-admin/newtown-staff (for any company)
///
/// # Request Format
///
/// ```json
/// {
///   "name": "Main Office",
///   "address": "123 Main St, City, State",
///   "latitude": 40.7128,
///   "longitude": -74.0060,
///   "company_id": 1
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 201 Created):**
/// ```json
/// {
///   "id": 1,
///   "name": "Main Office",
///   "address": "123 Main St, City, State",
///   "latitude": 40.7128,
///   "longitude": -74.0060,
///   "company_id": 1,
///   "created_at": "2023-01-01T00:00:00Z",
///   "updated_at": "2023-01-01T00:00:00Z"
/// }
/// ```
#[post("/1/sites", data = "<new_site>")]
pub async fn create_site(
    db: DbConn,
    new_site: LoggedJson<CreateSiteRequest>,
    auth_user: AuthenticatedUser
) -> Result<status::Created<Json<Site>>, Status> {
    // Check authorization
    if !can_crud_site(&auth_user, new_site.company_id) {
        return Err(Status::Forbidden);
    }
    
    db.run(move |conn| {
        insert_site(
            conn,
            new_site.name.clone(),
            new_site.address.clone(),
            new_site.latitude,
            new_site.longitude,
            new_site.company_id,
        )
        .map(|site| status::Created::new("/").body(Json(site)))
        .map_err(|e| {
            eprintln!("Error creating site: {:?}", e);
            Status::InternalServerError
        })
    }).await
}

/// Get Site endpoint.
///
/// - **URL:** `/api/1/sites/<site_id>`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves a specific site by ID
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or newtown-admin/newtown-staff (for any company)
#[get("/1/sites/<site_id>")]
pub async fn get_site(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser
) -> Result<Json<Site>, Status> {
    db.run(move |conn| {
        // First get the site to check its company
        match get_site_by_id(conn, site_id) {
            Ok(Some(site)) => {
                // Check authorization
                if !can_crud_site(&auth_user, site.company_id) {
                    return Err(Status::Forbidden);
                }
                Ok(Json(site))
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error getting site: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
}

/// List Sites endpoint.
///
/// - **URL:** `/api/1/sites`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all sites the user can access
/// - **Authentication:** Required
/// - **Authorization:** Returns sites based on user's access level
///   - newtown-admin/newtown-staff: all sites
///   - Company admin: sites from their company only
#[get("/1/sites")]
pub async fn list_sites(
    db: DbConn,
    auth_user: AuthenticatedUser
) -> Result<Json<Vec<Site>>, Status> {
    db.run(move |conn| {
        if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
            // Newtown roles can see all sites
            get_all_sites(conn)
                .map(Json)
                .map_err(|_| Status::InternalServerError)
        } else if auth_user.has_role("admin") {
            // Company admin can see sites from their company
            get_sites_by_company(conn, auth_user.user.company_id)
                .map(Json)
                .map_err(|_| Status::InternalServerError)
        } else {
            // Regular users cannot list sites
            Err(Status::Forbidden)
        }
    }).await
}

/// Update Site endpoint.
///
/// - **URL:** `/api/1/sites/<site_id>`
/// - **Method:** `PUT`
/// - **Purpose:** Updates a specific site
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or newtown-admin/newtown-staff (for any company)
///
/// # Request Format
///
/// ```json
/// {
///   "name": "Updated Office Name",
///   "address": "456 New St, City, State",
///   "latitude": 40.7589,
///   "longitude": -73.9851,
///   "company_id": 1
/// }
/// ```
#[put("/1/sites/<site_id>", data = "<update_data>")]
pub async fn update_site_endpoint(
    db: DbConn,
    site_id: i32,
    update_data: LoggedJson<UpdateSiteRequest>,
    auth_user: AuthenticatedUser
) -> Result<Json<Site>, Status> {
    db.run(move |conn| {
        // First get the site to check authorization
        match get_site_by_id(conn, site_id) {
            Ok(Some(site)) => {
                // Check authorization against the current site's company
                if !can_crud_site(&auth_user, site.company_id) {
                    return Err(Status::Forbidden);
                }
                
                // If changing company, check authorization for new company too
                if let Some(new_company_id) = update_data.company_id {
                    if !can_crud_site(&auth_user, new_company_id) {
                        return Err(Status::Forbidden);
                    }
                }
                
                // Perform the update
                update_site(
                    conn,
                    site_id,
                    update_data.name.clone(),
                    update_data.address.clone(),
                    update_data.latitude,
                    update_data.longitude,
                    update_data.company_id,
                )
                .map(Json)
                .map_err(|e| {
                    eprintln!("Error updating site: {:?}", e);
                    Status::InternalServerError
                })
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error finding site for update: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
}

/// Delete Site endpoint.
///
/// - **URL:** `/api/1/sites/<site_id>`
/// - **Method:** `DELETE`
/// - **Purpose:** Deletes a specific site
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or newtown-admin/newtown-staff (for any company)
#[delete("/1/sites/<site_id>")]
pub async fn delete_site_endpoint(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser
) -> Result<Status, Status> {
    db.run(move |conn| {
        // First get the site to check authorization
        match get_site_by_id(conn, site_id) {
            Ok(Some(site)) => {
                // Check authorization
                if !can_crud_site(&auth_user, site.company_id) {
                    return Err(Status::Forbidden);
                }
                
                // Perform the deletion
                match delete_site(conn, site_id) {
                    Ok(rows_affected) => {
                        if rows_affected > 0 {
                            Ok(Status::NoContent)
                        } else {
                            Err(Status::NotFound)
                        }
                    }
                    Err(e) => {
                        eprintln!("Error deleting site: {:?}", e);
                        Err(Status::InternalServerError)
                    }
                }
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error finding site for deletion: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
}

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for site endpoints
pub fn routes() -> Vec<Route> {
    routes![
        create_site,
        get_site,
        list_sites,
        update_site_endpoint,
        delete_site_endpoint
    ]
}