//! Test and staging-only secure endpoints for authentication and authorization testing.
//!
//! This module provides dummy secure endpoints that demonstrate different
//! types of authentication and authorization requirements. These endpoints
//! are only available when the `test-staging` feature is enabled.
//!
//! # Usage
//!
//! Enable the feature flag to include these endpoints:
//! ```bash
//! cargo run --features test-staging
//! ```

#[cfg(feature = "test-staging")]
use rocket::Route;
#[cfg(feature = "test-staging")]
use rocket::http::Status;
#[cfg(feature = "test-staging")]
use rocket::response::{self};
#[cfg(feature = "test-staging")]
use rocket::serde::json::{Json, Value, json};
#[cfg(feature = "test-staging")]
use serde::Serialize;
#[cfg(feature = "test-staging")]
use ts_rs::TS;

#[cfg(feature = "test-staging")]
use crate::session_guards::{
    AdminUser, AuthenticatedUser, NewtownAdminUser, NewtownStaffUser, StaffUser,
};

/// Error response structure for secure test API failures.
#[cfg(feature = "test-staging")]
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Admin-Only Test Endpoint.
///
/// - **URL:** `/api/1/test/admin-only`
/// - **Method:** `GET`
/// - **Purpose:** Test endpoint demonstrating admin role authorization
/// - **Authentication:** Required (admin role)
///
/// This endpoint demonstrates role-specific authorization using the
/// AdminUser guard. Only users with the "admin" role can access this endpoint.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "message": "Admin access granted to user@example.com",
///   "endpoint": "admin-only",
///   "required_role": "admin"
/// }
/// ```
///
/// **Failure (HTTP 401/403):** Authorization failure
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/test/admin-only")]
pub fn admin_only(admin_user: AdminUser) -> Json<Value> {
    Json(json!({
        "message": format!("Admin access granted to {}", admin_user.user.email),
        "endpoint": "admin-only",
        "required_role": "admin"
    }))
}

/// Staff-Only Test Endpoint.
///
/// - **URL:** `/api/1/test/staff-only`
/// - **Method:** `GET`
/// - **Purpose:** Test endpoint demonstrating staff role authorization
/// - **Authentication:** Required (staff role)
///
/// This endpoint demonstrates role-specific authorization using the
/// StaffUser guard. Only users with the "staff" role can access this endpoint.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "message": "Staff access granted to user@example.com",
///   "endpoint": "staff-only",
///   "required_role": "staff"
/// }
/// ```
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/test/staff-only")]
pub fn staff_only(staff_user: StaffUser) -> Json<Value> {
    Json(json!({
        "message": format!("Staff access granted to {}", staff_user.user.email),
        "endpoint": "staff-only",
        "required_role": "staff"
    }))
}

/// Newtown Admin-Only Test Endpoint.
///
/// - **URL:** `/api/1/test/newtown-admin-only`
/// - **Method:** `GET`
/// - **Purpose:** Test endpoint demonstrating newtown-admin role authorization
/// - **Authentication:** Required (newtown-admin role)
///
/// This endpoint demonstrates a more specific role requirement using the
/// NewtownAdminUser guard. Only users with the "newtown-admin" role can access this endpoint.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "message": "Newtown admin access granted to user@example.com",
///   "endpoint": "newtown-admin-only",
///   "required_role": "newtown-admin"
/// }
/// ```
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/test/newtown-admin-only")]
pub fn newtown_admin_only(newtown_admin_user: NewtownAdminUser) -> Json<Value> {
    Json(json!({
        "message": format!("Newtown admin access granted to {}", newtown_admin_user.user.email),
        "endpoint": "newtown-admin-only",
        "required_role": "newtown-admin"
    }))
}

/// Newtown Staff-Only Test Endpoint.
///
/// - **URL:** `/api/1/test/newtown-staff-only`
/// - **Method:** `GET`
/// - **Purpose:** Test endpoint demonstrating newtown-staff role authorization
/// - **Authentication:** Required (newtown-staff role)
///
/// This endpoint demonstrates a more specific role requirement using the
/// NewtownStaffUser guard. Only users with the "newtown-staff" role can access this endpoint.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "message": "Newtown staff access granted to user@example.com",
///   "endpoint": "newtown-staff-only",
///   "required_role": "newtown-staff"
/// }
/// ```
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/test/newtown-staff-only")]
pub fn newtown_staff_only(newtown_staff_user: NewtownStaffUser) -> Json<Value> {
    Json(json!({
        "message": format!("Newtown staff access granted to {}", newtown_staff_user.user.email),
        "endpoint": "newtown-staff-only",
        "required_role": "newtown-staff"
    }))
}

/// Multi-Role Test Endpoint.
///
/// - **URL:** `/api/1/test/admin-and-staff`
/// - **Method:** `GET`
/// - **Purpose:** Test endpoint requiring both admin AND staff roles
/// - **Authentication:** Required (both admin and staff roles)
///
/// This endpoint demonstrates manual role checking for complex authorization
/// requirements. The user must have both "admin" and "staff" roles to access
/// this endpoint.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "message": "Multi-role access granted to user@example.com",
///   "endpoint": "admin-and-staff",
///   "required_roles": ["admin", "staff"],
///   "user_roles": ["admin", "staff"]
/// }
/// ```
///
/// **Failure (HTTP 403 Forbidden):** Missing required roles
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/test/admin-and-staff")]
pub fn admin_and_staff(
    auth_user: AuthenticatedUser,
) -> Result<Json<Value>, response::status::Custom<Json<ErrorResponse>>> {
    if auth_user.has_all_roles(&["admin", "staff"]) {
        Ok(Json(json!({
            "message": format!("Multi-role access granted to {}", auth_user.user.email),
            "endpoint": "admin-and-staff",
            "required_roles": ["admin", "staff"],
            "user_roles": auth_user.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
        })))
    } else {
        let err = Json(ErrorResponse {
            error: "Forbidden: requires both admin and staff roles".to_string(),
        });
        Err(response::status::Custom(Status::Forbidden, err))
    }
}

/// No-Admin Test Endpoint.
///
/// - **URL:** `/api/1/test/no-admin-allowed`
/// - **Method:** `GET`
/// - **Purpose:** Test endpoint that forbids admin role access
/// - **Authentication:** Required (any role except admin)
///
/// This endpoint demonstrates negative role checking - users with the "admin"
/// role are explicitly forbidden from accessing this endpoint. This could be
/// useful for endpoints that should only be accessible to non-admin users.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "message": "Non-admin access granted to user@example.com",
///   "endpoint": "no-admin-allowed",
///   "forbidden_roles": ["admin"],
///   "user_roles": ["staff"]
/// }
/// ```
///
/// **Failure (HTTP 403 Forbidden):** User has admin role
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/test/no-admin-allowed")]
pub fn no_admin_allowed(
    auth_user: AuthenticatedUser,
) -> Result<Json<Value>, response::status::Custom<Json<ErrorResponse>>> {
    if auth_user.has_no_roles(&["admin"]) {
        Ok(Json(json!({
            "message": format!("Non-admin access granted to {}", auth_user.user.email),
            "endpoint": "no-admin-allowed",
            "forbidden_roles": ["admin"],
            "user_roles": auth_user.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
        })))
    } else {
        let err = Json(ErrorResponse {
            error: "Forbidden: admin role is not allowed for this endpoint".to_string(),
        });
        Err(response::status::Custom(Status::Forbidden, err))
    }
}

/// Flexible Role Test Endpoint.
///
/// - **URL:** `/api/1/test/any-admin-or-staff`
/// - **Method:** `GET`
/// - **Purpose:** Test endpoint accepting any of several roles
/// - **Authentication:** Required (admin OR staff OR newtown-admin role)
///
/// This endpoint demonstrates flexible role checking where users need at least
/// one of several possible roles to access the endpoint.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "message": "Flexible role access granted to user@example.com",
///   "endpoint": "any-admin-or-staff",
///   "accepted_roles": ["admin", "staff", "newtown-admin"],
///   "user_roles": ["admin"]
/// }
/// ```
///
/// **Failure (HTTP 403 Forbidden):** Missing any accepted roles
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/test/any-admin-or-staff")]
pub fn any_admin_or_staff(
    auth_user: AuthenticatedUser,
) -> Result<Json<Value>, response::status::Custom<Json<ErrorResponse>>> {
    if auth_user.has_any_role(&["admin", "staff", "newtown-admin"]) {
        Ok(Json(json!({
            "message": format!("Flexible role access granted to {}", auth_user.user.email),
            "endpoint": "any-admin-or-staff",
            "accepted_roles": ["admin", "staff", "newtown-admin"],
            "user_roles": auth_user.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
        })))
    } else {
        let err = Json(ErrorResponse {
            error: "Forbidden: requires admin, staff, or newtown-admin role".to_string(),
        });
        Err(response::status::Custom(Status::Forbidden, err))
    }
}

/// Returns a vector of all test/staging routes defined in this module.
///
/// This function collects all the test route handlers and returns them
/// as a vector for registration with the Rocket framework. Only compiled
/// when the `test-staging` feature is enabled.
#[cfg(feature = "test-staging")]
pub fn routes() -> Vec<Route> {
    routes![
        admin_only,
        staff_only,
        newtown_admin_only,
        newtown_staff_only,
        admin_and_staff,
        no_admin_allowed,
        any_admin_or_staff
    ]
}

/// Returns an empty vector when test-staging feature is disabled.
///
/// This ensures the module can be safely included in builds without
/// the test-staging feature enabled.
#[cfg(not(feature = "test-staging"))]
pub fn routes() -> Vec<rocket::Route> {
    vec![]
}
