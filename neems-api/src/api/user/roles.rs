//! User role assignment endpoints.

use rocket::{http::Status, serde::json::Json};
use ts_rs::TS;

use crate::{
    models::{CompanyInput, Role},
    orm::{
        DbConn,
        company::get_company_by_name,
        user::get_user,
        user_role::{assign_user_role_by_name, get_user_roles, remove_user_role_by_name},
    },
    session_guards::AuthenticatedUser,
};

/// Request structure for adding a role to a user (user_id comes from URL path).
#[derive(serde::Deserialize, TS)]
#[ts(export)]
pub struct AddUserRoleRequest {
    pub role_name: String,
}

/// Request structure for removing a role from a user (user_id comes from URL
/// path).
#[derive(serde::Deserialize, TS)]
#[ts(export)]
pub struct RemoveUserRoleRequest {
    pub role_name: String,
}

/// Get User Roles endpoint.
///
/// - **URL:** `/api/1/users/<user_id>/roles`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all roles assigned to a specific user
/// - **Authentication:** Required (users can view their own roles, or users
///   with admin privileges can view any user's roles)
///
/// This endpoint retrieves all roles assigned to a specific user.
/// Users can view their own roles, or users with sufficient privileges
/// can view any user's roles.
///
/// # Parameters
///
/// - `user_id` - The ID of the user whose roles to retrieve
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "admin",
///     "description": "Administrator role",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   },
///   {
///     "id": 2,
///     "name": "staff",
///     "description": "Staff role",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   }
/// ]
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to view the specified user's roles
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - The ID of the user whose roles to retrieve
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Json<Vec<Role>>)` - List of roles for the specified user
/// * `Err(Status)` - Error status (Forbidden, InternalServerError, etc.)
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/users/123/roles', {
///   method: 'GET',
///   credentials: 'include'
/// });
/// ```
#[get("/1/Users/<user_id>/Roles")]
pub async fn get_user_roles_endpoint(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<Role>>, Status> {
    // Users can view their own roles, admins can view any user's roles
    if auth_user.user.id != user_id
        && !auth_user.has_any_role(&["newtown-admin", "newtown-staff", "admin"])
    {
        return Err(Status::Forbidden);
    }

    db.run(move |conn| {
        get_user_roles(conn, user_id).map(Json).map_err(|e| {
            eprintln!("Error getting user roles: {:?}", e);
            Status::InternalServerError
        })
    })
    .await
}

/// Add User Role endpoint.
///
/// - **URL:** `/api/1/users/<user_id>/roles`
/// - **Method:** `POST`
/// - **Purpose:** Assigns a role to a user with authorization checks
/// - **Authentication:** Required (admin privileges with specific business
///   rules)
///
/// This endpoint allows authorized users to add roles to other users
/// following the business rules:
/// 1. `newtown-staff` and `newtown-admin` roles are reserved for Newtown Energy
///    company
/// 2. `newtown-admin` can set any user's role to anything
/// 3. `newtown-staff` can set any user's role except `newtown-admin`
/// 4. `admin` can set another user's role to `admin` if target user is at same
///    company
/// 5. Users must have at least one role (validated elsewhere)
///
/// # Authorization Rules
///
/// 1. `newtown-staff` and `newtown-admin` roles are reserved for Newtown Energy
///    company
/// 2. `newtown-admin` can set any user's role to anything
/// 3. `newtown-staff` can set any user's role except `newtown-admin`
/// 4. `admin` can set another user's role to `admin` if target user is at same
///    company
/// 5. Users must have at least one role (validated elsewhere)
///
/// # Request Format
///
/// ```json
/// {
///   "role_name": "staff"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// No response body - role successfully assigned
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to assign the specified role
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error or validation failure
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - User ID from URL path parameter
/// * `request` - JSON payload containing role_name to add
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Status::Ok)` - Role successfully assigned
/// * `Err(Status)` - Error status (Forbidden, InternalServerError, etc.)
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/users/123/roles', {
///   method: 'POST',
///   headers: { 'Content-Type': 'application/json' },
///   body: JSON.stringify({
///     role_name: 'staff'
///   }),
///   credentials: 'include'
/// });
/// ```
#[post("/1/Users/<user_id>/Roles", data = "<request>")]
pub async fn add_user_role(
    db: DbConn,
    user_id: i32,
    request: Json<AddUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = user_id;
    let role_name = request.role_name.clone();

    // Get target user's company for validation
    let target_user = db
        .run(move |conn| get_user(conn, target_user_id))
        .await
        .map_err(|e| {
            eprintln!("Error getting target user: {:?}", e);
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    // Authorization check based on business rules
    let can_assign = if auth_user.has_role("newtown-admin") {
        // Rule 2: newtown-admin can set any user's role to anything
        true
    } else if auth_user.has_role("newtown-staff") {
        // Rule 3: newtown-staff can set any user's role except newtown-admin
        role_name != "newtown-admin"
    } else if auth_user.has_role("admin") {
        // Rule 4: admin can set another user's role to any role if same company
        auth_user.user.company_id == target_user.company_id
    } else {
        false
    };

    if !can_assign {
        return Err(Status::Forbidden);
    }

    // Rule 1: newtown-staff and newtown-admin roles are reserved for Newtown Energy
    if role_name == "newtown-staff" || role_name == "newtown-admin" {
        let newtown_company_search = CompanyInput { name: "Newtown Energy".to_string() };
        let newtown_company = db
            .run(move |conn| get_company_by_name(conn, &newtown_company_search))
            .await
            .map_err(|e| {
                eprintln!("Error getting Newtown Energy company: {:?}", e);
                Status::InternalServerError
            })?;

        let newtown_company = match newtown_company {
            Some(inst) => inst,
            None => {
                eprintln!("Newtown Energy company not found");
                return Err(Status::InternalServerError);
            }
        };

        if target_user.company_id != newtown_company.id {
            return Err(Status::Forbidden);
        }
    }

    // Assign the role
    db.run(move |conn| {
        assign_user_role_by_name(conn, target_user_id, &role_name).map_err(|e| {
            eprintln!("Error assigning user role: {:?}", e);
            Status::InternalServerError
        })
    })
    .await?;

    Ok(Status::Ok)
}

/// Remove User Role endpoint.
///
/// - **URL:** `/api/1/users/<user_id>/roles`
/// - **Method:** `DELETE`
/// - **Purpose:** Removes a role from a user with authorization checks
/// - **Authentication:** Required (same authorization rules as adding roles)
///
/// This endpoint allows authorized users to remove roles from other users
/// following the same authorization rules as adding roles. Additionally,
/// it ensures users always retain at least one role.
///
/// # Authorization Rules
///
/// Same authorization rules as adding roles, plus:
/// - Users must retain at least one role after removal
///
/// # Request Format
///
/// ```json
/// {
///   "role_name": "staff"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// No response body - role successfully removed
///
/// **Failure (HTTP 400 Bad Request):**
/// User would have no roles remaining after removal
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to remove the specified role
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error or validation failure
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - User ID from URL path parameter
/// * `request` - JSON payload containing role_name to remove
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Status::Ok)` - Role successfully removed
/// * `Err(Status)` - Error status (Forbidden, BadRequest, InternalServerError,
///   etc.)
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/users/123/roles', {
///   method: 'DELETE',
///   headers: { 'Content-Type': 'application/json' },
///   body: JSON.stringify({
///     role_name: 'staff'
///   }),
///   credentials: 'include'
/// });
/// ```
#[delete("/1/Users/<user_id>/Roles", data = "<request>")]
pub async fn remove_user_role(
    db: DbConn,
    user_id: i32,
    request: Json<RemoveUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = user_id;
    let role_name = request.role_name.clone();

    // Get target user's company for validation
    let target_user = db
        .run(move |conn| get_user(conn, target_user_id))
        .await
        .map_err(|e| {
            eprintln!("Error getting target user: {:?}", e);
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    // Check if user would have any roles left after removal
    let current_roles =
        db.run(move |conn| get_user_roles(conn, target_user_id)).await.map_err(|e| {
            eprintln!("Error getting current user roles: {:?}", e);
            Status::InternalServerError
        })?;

    // Rule 5: Users must have at least one role
    if current_roles.len() <= 1 {
        return Err(Status::BadRequest);
    }

    // Authorization check - same rules as adding roles
    let can_remove = if auth_user.has_role("newtown-admin") {
        true
    } else if auth_user.has_role("newtown-staff") {
        role_name != "newtown-admin"
    } else if auth_user.has_role("admin") {
        auth_user.user.company_id == target_user.company_id
    } else {
        false
    };

    if !can_remove {
        return Err(Status::Forbidden);
    }

    // Remove the role
    db.run(move |conn| {
        remove_user_role_by_name(conn, target_user_id, &role_name).map_err(|e| {
            eprintln!("Error removing user role: {:?}", e);
            Status::InternalServerError
        })
    })
    .await?;

    Ok(Status::Ok)
}
