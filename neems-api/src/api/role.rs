//! API endpoints for managing roles.
//!
//! This module provides HTTP endpoints for creating and listing roles
//! in the system. Roles define permissions and access levels that can
//! be assigned to users within companies.

use rocket::{
    Route,
    http::Status,
    response::{self},
    serde::json::Json,
};
use serde::Serialize;
use ts_rs::TS;

use crate::{
    logged_json::LoggedJson,
    models::{NewRole, Role},
    orm::{
        DbConn,
        role::{delete_role, get_all_roles, get_role, insert_role, update_role},
    },
    session_guards::AuthenticatedUser,
};

/// Error response structure for role API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Create Role endpoint.
///
/// - **URL:** `/api/1/roles`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new role in the system
/// - **Authentication:** Required
/// - **Authorization:** Only newtown-admin can create new roles
///
/// This endpoint accepts a JSON payload containing the role information and
/// creates a new role record in the database.
///
/// # Request Format
///
/// ```json
/// {
///   "name": "Administrator",
///   "description": "Full system access"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "id": 1,
///   "name": "Administrator",
///   "description": "Full system access"
/// }
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to create roles
///
/// # Arguments
/// * `db` - Database connection pool
/// * `new_role` - JSON payload containing the new role data
/// * `auth_user` - Authenticated user for authorization
///
/// # Returns
/// * `Ok(Json<Role>)` - Successfully created role
/// * `Err(Status)` - Error during creation or authorization failure
#[post("/1/Roles", data = "<new_role>")]
pub async fn create_role(
    db: DbConn,
    new_role: LoggedJson<NewRole>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Role>, response::status::Custom<Json<ErrorResponse>>> {
    // Only newtown-admin can create roles
    if !auth_user.has_role("newtown-admin") {
        let err = Json(ErrorResponse {
            error: "Forbidden: only newtown-admin can create roles".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }
    db.run(move |conn| {
        insert_role(conn, new_role.into_inner()).map(Json).map_err(|e| {
            eprintln!("Error creating role: {:?}", e);
            let err = Json(ErrorResponse {
                error: "Internal server error while creating role".to_string(),
            });
            response::status::Custom(Status::InternalServerError, err)
        })
    })
    .await
}

/// List Roles endpoint.
///
/// - **URL:** `/api/1/roles`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all roles in the system
/// - **Authentication:** Required
/// - **Authorization:** All authenticated users can list roles
///
/// This endpoint retrieves all roles from the database and returns them
/// as a JSON array.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "Administrator",
///     "description": "Full system access"
///   },
///   {
///     "id": 2,
///     "name": "User",
///     "description": "Basic user access"
///   }
/// ]
/// ```
///
/// # Arguments
/// * `db` - Database connection pool
/// * `auth_user` - Authenticated user for authorization
///
/// # Returns
/// * `Ok(Json<Vec<Role>>)` - List of all roles
/// * `Err(Status)` - Error during retrieval (typically InternalServerError)
#[get("/1/Roles")]
pub async fn list_roles(
    db: DbConn,
    _auth_user: AuthenticatedUser,
) -> Result<Json<Vec<Role>>, response::status::Custom<Json<ErrorResponse>>> {
    // All authenticated users can list roles (needed for role assignment UIs)
    db.run(|conn| {
        get_all_roles(conn).map(Json).map_err(|e| {
            eprintln!("Error listing roles: {:?}", e);
            let err = Json(ErrorResponse {
                error: "Internal server error while listing roles".to_string(),
            });
            response::status::Custom(Status::InternalServerError, err)
        })
    })
    .await
}

/// Update Role Request structure for partial updates.
///
/// This structure represents the JSON payload for updating a role.
/// All fields are optional to support partial updates.
#[derive(serde::Deserialize, Debug, TS)]
#[ts(export)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_description")]
    #[ts(skip)]
    pub description: Option<Option<String>>,
}

/// Custom deserializer for description field to handle null values properly
fn deserialize_description<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    // If the field is present, deserialize it and wrap in Some
    // If field value is null, we'll get None from the inner Option
    // If field value is a string, we'll get Some(string) from the inner Option
    let inner: Option<String> = Option::deserialize(deserializer)?;
    Ok(Some(inner))
}

/// Get Role endpoint.
///
/// - **URL:** `/api/1/roles/<role_id>`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves a specific role by ID
/// - **Authentication:** Required
/// - **Authorization:** All authenticated users can get individual roles
///
/// This endpoint retrieves a single role from the database by its ID.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "id": 1,
///   "name": "Administrator",
///   "description": "Full system access"
/// }
/// ```
///
/// **Failure (HTTP 404 Not Found):**
/// Role with the specified ID does not exist
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error during retrieval
///
/// # Arguments
/// * `db` - Database connection pool
/// * `role_id` - The ID of the role to retrieve
/// * `auth_user` - Authenticated user for authorization
///
/// # Returns
/// * `Ok(Json<Role>)` - The requested role
/// * `Err(Status)` - Error during retrieval or role not found
#[get("/1/Roles/<role_id>")]
pub async fn get_role_endpoint(
    db: DbConn,
    role_id: i32,
    _auth_user: AuthenticatedUser,
) -> Result<Json<Role>, response::status::Custom<Json<ErrorResponse>>> {
    // All authenticated users can get individual roles (needed for role assignment
    // UIs)
    db.run(move |conn| {
        get_role(conn, role_id).map(Json).map_err(|e| match e {
            diesel::result::Error::NotFound => {
                let err = Json(ErrorResponse {
                    error: format!("Role with ID {} not found", role_id),
                });
                response::status::Custom(Status::NotFound, err)
            }
            _ => {
                eprintln!("Error getting role: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error while getting role".to_string(),
                });
                response::status::Custom(Status::InternalServerError, err)
            }
        })
    })
    .await
}

/// Update Role endpoint.
///
/// - **URL:** `/api/1/roles/<role_id>`
/// - **Method:** `PUT`
/// - **Purpose:** Updates an existing role's information
/// - **Authentication:** Required
/// - **Authorization:** Only newtown-admin can update roles
///
/// This endpoint accepts a JSON payload with optional fields to update
/// a role's information. Only provided fields will be updated.
///
/// # Request Format
///
/// ```json
/// {
///   "name": "New Role Name",
///   "description": "Updated description"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "id": 1,
///   "name": "New Role Name",
///   "description": "Updated description"
/// }
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to update roles
///
/// **Failure (HTTP 404 Not Found):**
/// Role with the specified ID does not exist
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error during update
///
/// # Arguments
/// * `db` - Database connection pool
/// * `role_id` - The ID of the role to update
/// * `request` - JSON payload containing the fields to update
/// * `auth_user` - Authenticated user for authorization
///
/// # Returns
/// * `Ok(Json<Role>)` - Updated role object
/// * `Err(Status)` - Error during update or role not found
#[put("/1/Roles/<role_id>", data = "<request>")]
pub async fn update_role_endpoint(
    db: DbConn,
    role_id: i32,
    request: Json<UpdateRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Role>, response::status::Custom<Json<ErrorResponse>>> {
    // Only newtown-admin can update roles
    if !auth_user.has_role("newtown-admin") {
        let err = Json(ErrorResponse {
            error: "Forbidden: only newtown-admin can update roles".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }
    db.run(move |conn| {
        update_role(conn, role_id, request.name.clone(), request.description.clone())
            .map(Json)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => {
                    let err = Json(ErrorResponse {
                        error: format!("Role with ID {} not found", role_id),
                    });
                    response::status::Custom(Status::NotFound, err)
                }
                _ => {
                    eprintln!("Error updating role: {:?}", e);
                    let err = Json(ErrorResponse {
                        error: "Internal server error while updating role".to_string(),
                    });
                    response::status::Custom(Status::InternalServerError, err)
                }
            })
    })
    .await
}

/// Delete Role endpoint.
///
/// - **URL:** `/api/1/roles/<role_id>`
/// - **Method:** `DELETE`
/// - **Purpose:** Deletes a role from the system
/// - **Authentication:** Required
/// - **Authorization:** Only newtown-admin can delete roles
///
/// This endpoint permanently removes a role from the database.
/// **Warning**: This will affect any user_roles records that reference
/// this role due to foreign key constraints.
///
/// # Response
///
/// **Success (HTTP 204 No Content):**
/// Role successfully deleted
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to delete roles
///
/// **Failure (HTTP 404 Not Found):**
/// Role with the specified ID does not exist
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error during deletion
///
/// # Arguments
/// * `db` - Database connection pool
/// * `role_id` - The ID of the role to delete
/// * `auth_user` - Authenticated user for authorization
///
/// # Returns
/// * `Ok(Status::NoContent)` - Role successfully deleted
/// * `Err(Status)` - Error during deletion or role not found
#[delete("/1/Roles/<role_id>")]
pub async fn delete_role_endpoint(
    db: DbConn,
    role_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, response::status::Custom<Json<ErrorResponse>>> {
    // Only newtown-admin can delete roles
    if !auth_user.has_role("newtown-admin") {
        let err = Json(ErrorResponse {
            error: "Forbidden: only newtown-admin can delete roles".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }
    db.run(move |conn| match delete_role(conn, role_id) {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                Ok(Status::NoContent)
            } else {
                let err = Json(ErrorResponse {
                    error: format!("Role with ID {} not found", role_id),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
        }
        Err(e) => {
            eprintln!("Error deleting role: {:?}", e);
            let err = Json(ErrorResponse {
                error: "Internal server error while deleting role".to_string(),
            });
            Err(response::status::Custom(Status::InternalServerError, err))
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
/// A vector containing all route handlers for role endpoints
pub fn routes() -> Vec<Route> {
    routes![
        create_role,
        list_roles,
        get_role_endpoint,
        update_role_endpoint,
        delete_role_endpoint
    ]
}
