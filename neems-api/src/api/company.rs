//! API endpoints for managing companies.
//!
//! This module provides HTTP endpoints for creating and listing companies
//! in the system. Companies represent organizations or entities that can
//! be associated with users and roles.

use rocket::Route;
use rocket::http::Status;
use rocket::response::{self, status};
use rocket::serde::json::Json;
use serde::Serialize;
use ts_rs::TS;

use crate::company::{get_company_by_name_case_insensitive, insert_company};
use crate::models::Site;
use crate::models::{Company, CompanyInput, UserWithRoles};
use crate::orm::DbConn;
use crate::orm::company::{delete_company, get_all_companies};
use crate::orm::site::get_sites_by_company;
use crate::orm::user::get_users_by_company_with_roles;
use crate::session_guards::AuthenticatedUser;

/// Error response structure for company API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

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
/// * `Err(response::status::Custom<Json<ErrorResponse>>)` - Error during creation with JSON error details
#[post("/1/Companies", data = "<new_company>")]
pub async fn create_company(
    db: DbConn,
    new_company: Json<CompanyInput>,
    _auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<Company>>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // First check if company with this name already exists (case-insensitive)
        match get_company_by_name_case_insensitive(conn, &new_company.name) {
            Ok(Some(_existing_company)) => {
                // Company with this name already exists
                let err = Json(ErrorResponse {
                    error: format!("Company with name '{}' already exists", new_company.name),
                });
                return Err(response::status::Custom(Status::Conflict, err));
            }
            Ok(None) => {
                // Company doesn't exist, we can proceed
            }
            Err(e) => {
                eprintln!("Error checking for existing company: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while checking for existing company".to_string(),
                });
                return Err(response::status::Custom(Status::InternalServerError, err));
            }
        }

        // Proceed with company creation
        insert_company(conn, new_company.name.clone())
            .map(|comp| status::Created::new("/").body(Json(comp)))
            .map_err(|e| {
                eprintln!("Error creating company: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while creating company".to_string(),
                });
                response::status::Custom(Status::InternalServerError, err)
            })
    })
    .await
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
#[get("/1/Companies?<query..>")]
pub async fn list_companies(
    db: DbConn,
    _auth_user: AuthenticatedUser,
    query: crate::odata_query::ODataQuery,
) -> Result<Json<serde_json::Value>, Status> {
    // Validate query options
    query.validate().map_err(|_| Status::BadRequest)?;
    
    let companies = db.run(|conn| {
        get_all_companies(conn).map_err(|_| Status::InternalServerError)
    })
    .await?;

    // Apply filtering if specified
    let mut filtered_companies = companies;
    if let Some(filter_expr) = query.parse_filter() {
        // Basic filtering implementation
        filtered_companies = filtered_companies
            .into_iter()
            .filter(|company| {
                match &filter_expr.property.as_str() {
                    &"name" => match &filter_expr.value {
                        crate::odata_query::FilterValue::String(s) => match filter_expr.operator {
                            crate::odata_query::FilterOperator::Eq => company.name == *s,
                            crate::odata_query::FilterOperator::Ne => company.name != *s,
                            crate::odata_query::FilterOperator::Contains => company.name.contains(s),
                            _ => true,
                        },
                        _ => true,
                    },
                    _ => true, // Unknown property, don't filter
                }
            })
            .collect();
    }

    // Apply ordering
    if let Some(order_props) = query.parse_orderby() {
        for (property, direction) in order_props {
            match property.as_str() {
                "name" => {
                    filtered_companies.sort_by(|a, b| {
                        let cmp = a.name.cmp(&b.name);
                        match direction {
                            crate::odata_query::OrderDirection::Asc => cmp,
                            crate::odata_query::OrderDirection::Desc => cmp.reverse(),
                        }
                    });
                }
                "id" => {
                    filtered_companies.sort_by(|a, b| {
                        let cmp = a.id.cmp(&b.id);
                        match direction {
                            crate::odata_query::OrderDirection::Asc => cmp,
                            crate::odata_query::OrderDirection::Desc => cmp.reverse(),
                        }
                    });
                }
                _ => {} // Unknown property, don't sort
            }
        }
    }

    // Get count before applying top/skip
    let total_count = filtered_companies.len() as i64;

    // Apply skip and top
    if let Some(skip) = query.skip {
        filtered_companies = filtered_companies.into_iter().skip(skip as usize).collect();
    }
    if let Some(top) = query.top {
        filtered_companies = filtered_companies.into_iter().take(top as usize).collect();
    }

    // Handle $expand first, then $select
    let expand_props = query.parse_expand();
    let mut expanded_companies: Vec<serde_json::Value> = Vec::new();
    
    for company in &filtered_companies {
        let mut company_json = serde_json::to_value(company).map_err(|_| Status::InternalServerError)?;
        
        // Handle expansions
        if let Some(expansions) = &expand_props {
            // Handle $expand=users
            if expansions.iter().any(|e| e.eq_ignore_ascii_case("users")) {
                let company_id = company.id;
                let users = db.run(move |conn| {
                    use crate::orm::user::get_users_by_company_with_roles;
                    get_users_by_company_with_roles(conn, company_id)
                }).await.map_err(|_| Status::InternalServerError)?;
                
                company_json.as_object_mut()
                    .unwrap()
                    .insert("Users".to_string(), serde_json::to_value(users).map_err(|_| Status::InternalServerError)?);
            }
            
            // Handle $expand=sites
            if expansions.iter().any(|e| e.eq_ignore_ascii_case("sites")) {
                let company_id = company.id;
                let sites = db.run(move |conn| {
                    use crate::orm::site::get_sites_by_company;
                    get_sites_by_company(conn, company_id)
                }).await.map_err(|_| Status::InternalServerError)?;
                
                company_json.as_object_mut()
                    .unwrap()
                    .insert("Sites".to_string(), serde_json::to_value(sites).map_err(|_| Status::InternalServerError)?);
            }
        }
        
        expanded_companies.push(company_json);
    }

    // Apply $select to each expanded company if specified
    let select_props = query.parse_select();
    let selected_companies: Result<Vec<serde_json::Value>, _> = expanded_companies
        .iter()
        .map(|company| crate::odata_query::apply_select(company, select_props.as_deref()))
        .collect();

    let selected_companies = selected_companies.map_err(|_| Status::InternalServerError)?;

    // Build OData response
    let context = crate::odata_query::build_context_url("http://localhost/api/1", "Companies", select_props.as_deref());
    let mut response = crate::odata_query::ODataCollectionResponse::new(context, selected_companies);

    // Add count if requested
    if query.count.unwrap_or(false) {
        response = response.with_count(total_count);
    }

    Ok(Json(serde_json::to_value(response).map_err(|_| Status::InternalServerError)?))
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
#[get("/1/Companies/<company_id>/Sites")]
pub async fn list_company_sites(
    db: DbConn,
    company_id: i32,
    auth_user: AuthenticatedUser,
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
    })
    .await
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
#[get("/1/Companies/<company_id>/Users")]
pub async fn list_company_users(
    db: DbConn,
    company_id: i32,
    auth_user: AuthenticatedUser,
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
    })
    .await
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
#[delete("/1/Companies/<company_id>")]
pub async fn delete_company_endpoint(
    db: DbConn,
    company_id: i32,
    _auth_user: AuthenticatedUser,
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
    })
    .await
}

/// Get Company Users Navigation endpoint.
///
/// - **URL:** `/api/1/Companies/<company_id>/Users`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves users associated with a company (OData navigation property)
/// - **Authentication:** Required
///
/// This is an OData navigation endpoint that returns the User entities
/// associated with the specified company. This is the same as list_company_users
/// but follows OData navigation conventions.
// Note: This endpoint is already implemented as list_company_users above

/// Get Company Sites Navigation endpoint.
///
/// - **URL:** `/api/1/Companies/<company_id>/Sites`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves sites associated with a company (OData navigation property)
/// - **Authentication:** Required
///
/// This is an OData navigation endpoint that returns the Site entities
/// associated with the specified company. This is the same as list_company_sites
/// but follows OData navigation conventions.
// Note: This endpoint is already implemented as list_company_sites above

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for company endpoints
pub fn routes() -> Vec<Route> {
    routes![
        create_company,
        list_companies,
        list_company_sites,
        list_company_users,
        delete_company_endpoint
    ]
}
