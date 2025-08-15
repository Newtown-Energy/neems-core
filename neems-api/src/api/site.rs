//! API endpoints for site management.
//!
//! This module provides HTTP endpoints for CRUD operations on sites.
//! Sites are associated with companies and have location data.
//!
//! # Authorization Rules
//! - Company admins can perform CRUD operations on sites within their company
//! - newtown-staff and newtown-admin roles can perform CRUD operations on any site
//! - Regular users cannot perform CRUD operations

use rocket::Route;
use rocket::http::Status;
use rocket::response::{self, status};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::logged_json::LoggedJson;
use crate::models::Site;
use crate::orm::DbConn;
use crate::orm::company::get_company_by_id;
use crate::orm::site::{
    delete_site, get_all_sites, get_site_by_company_and_name, get_site_by_id, get_sites_by_company,
    insert_site, update_site,
};
use crate::session_guards::AuthenticatedUser;

/// Error response structure for site API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Request payload for creating a new site
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateSiteRequest {
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,
}

/// Request payload for updating a site (all fields optional)
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
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
#[post("/1/Sites", data = "<new_site>")]
pub async fn create_site(
    db: DbConn,
    new_site: LoggedJson<CreateSiteRequest>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<Site>>, response::status::Custom<Json<ErrorResponse>>> {
    // Check authorization
    if !can_crud_site(&auth_user, new_site.company_id) {
        let err = Json(ErrorResponse {
            error: "Forbidden: insufficient permissions to create site for this company"
                .to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }

    db.run(move |conn| {
        // First validate that the company exists
        match get_company_by_id(conn, new_site.company_id) {
            Ok(Some(_)) => {
                // Company exists, now check if site with this name already exists in the company
                match get_site_by_company_and_name(conn, new_site.company_id, &new_site.name) {
                    Ok(Some(_existing_site)) => {
                        // Site with this name already exists in this company
                        let err = Json(ErrorResponse {
                            error: format!(
                                "Site with name '{}' already exists in this company",
                                new_site.name
                            ),
                        });
                        return Err(response::status::Custom(Status::Conflict, err));
                    }
                    Ok(None) => {
                        // Site doesn't exist, we can proceed
                    }
                    Err(e) => {
                        eprintln!("Error checking for existing site: {:?}", e);
                        let err = Json(ErrorResponse {
                            error: "Database error while checking for existing site".to_string(),
                        });
                        return Err(response::status::Custom(Status::InternalServerError, err));
                    }
                }

                // Proceed with site creation
                insert_site(
                    conn,
                    new_site.name.clone(),
                    new_site.address.clone(),
                    new_site.latitude,
                    new_site.longitude,
                    new_site.company_id,
                    Some(auth_user.user.id),
                )
                .map(|site| status::Created::new("/").body(Json(site)))
                .map_err(|e| {
                    eprintln!("Error creating site: {:?}", e);
                    let err = Json(ErrorResponse {
                        error: "Internal server error while creating site".to_string(),
                    });
                    response::status::Custom(Status::InternalServerError, err)
                })
            }
            Ok(None) => {
                eprintln!(
                    "Error creating site: Company with ID {} does not exist",
                    new_site.company_id
                );
                let err = Json(ErrorResponse {
                    error: format!("Company with ID {} does not exist", new_site.company_id),
                });
                Err(response::status::Custom(Status::BadRequest, err))
            }
            Err(e) => {
                eprintln!("Error validating company for site creation: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error while validating company".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get Site endpoint.
///
/// - **URL:** `/api/1/sites/<site_id>`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves a specific site by ID
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or newtown-admin/newtown-staff (for any company)
#[get("/1/Sites/<site_id>")]
pub async fn get_site(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
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
    })
    .await
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
#[get("/1/Sites")]
pub async fn list_sites(
    db: DbConn,
    auth_user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, Status> {
    db.run(move |conn| {
        let sites = if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
            // Newtown roles can see all sites
            match get_all_sites(conn) {
                Ok(sites) => sites,
                Err(_) => return Err(Status::InternalServerError),
            }
        } else if auth_user.has_role("admin") {
            // Company admin can see sites from their company
            match get_sites_by_company(conn, auth_user.user.company_id) {
                Ok(sites) => sites,
                Err(_) => return Err(Status::InternalServerError),
            }
        } else {
            // Regular users cannot list sites
            return Err(Status::Forbidden);
        };

        let response = serde_json::json!({
            "@odata.context": "http://localhost/api/1/$metadata#Sites",
            "value": sites
        });
        
        Ok(Json(response))
    })
    .await
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
#[put("/1/Sites/<site_id>", data = "<update_data>")]
pub async fn update_site_endpoint(
    db: DbConn,
    site_id: i32,
    update_data: LoggedJson<UpdateSiteRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Site>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // First get the site to check authorization
        match get_site_by_id(conn, site_id) {
            Ok(Some(site)) => {
                // Check authorization against the current site's company
                if !can_crud_site(&auth_user, site.company_id) {
                    let err = Json(ErrorResponse {
                        error: "Forbidden: insufficient permissions to update this site".to_string(),
                    });
                    return Err(response::status::Custom(Status::Forbidden, err));
                }

                // If changing company, validate new company exists and check authorization
                if let Some(new_company_id) = update_data.company_id {
                    // First check if the new company exists
                    match get_company_by_id(conn, new_company_id) {
                        Ok(Some(_)) => {
                            // Company exists, check authorization
                            if !can_crud_site(&auth_user, new_company_id) {
                                let err = Json(ErrorResponse {
                                    error: "Forbidden: insufficient permissions to move site to this company".to_string(),
                                });
                                return Err(response::status::Custom(Status::Forbidden, err));
                            }
                        }
                        Ok(None) => {
                            eprintln!("Error updating site: Company with ID {} does not exist", new_company_id);
                            let err = Json(ErrorResponse {
                                error: format!("Company with ID {} does not exist", new_company_id),
                            });
                            return Err(response::status::Custom(Status::BadRequest, err));
                        }
                        Err(e) => {
                            eprintln!("Error validating company for site update: {:?}", e);
                            let err = Json(ErrorResponse {
                                error: "Internal server error while validating company".to_string(),
                            });
                            return Err(response::status::Custom(Status::InternalServerError, err));
                        }
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
                    Some(auth_user.user.id),
                )
                .map(Json)
                .map_err(|e| {
                    eprintln!("Error updating site: {:?}", e);
                    let err = Json(ErrorResponse {
                        error: "Internal server error while updating site".to_string(),
                    });
                    response::status::Custom(Status::InternalServerError, err)
                })
            }
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: format!("Site with ID {} not found", site_id),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error finding site for update: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error while finding site".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
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
#[delete("/1/Sites/<site_id>")]
pub async fn delete_site_endpoint(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
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
                match delete_site(conn, site_id, Some(auth_user.user.id)) {
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
    })
    .await
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
