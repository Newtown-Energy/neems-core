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
use rocket::serde::json::{Json, json, Value};
#[cfg(feature = "test-staging")]
use rocket::Route;
#[cfg(feature = "test-staging")]
use rocket::http::Status;

#[cfg(feature = "test-staging")]
use crate::session_guards::{AuthenticatedUser, AdminUser, StaffUser, NewtownAdminUser, NewtownStaffUser};

/// Admin-only endpoint that requires the "admin" role.
///
/// This endpoint demonstrates role-specific authorization using the
/// AdminUser guard. Only users with the "admin" role can access this endpoint.
#[cfg(feature = "test-staging")]
#[get("/1/test/admin-only")]
pub fn admin_only(admin_user: AdminUser) -> Json<Value> {
    Json(json!({
        "message": format!("Admin access granted to {}", admin_user.user.email),
        "endpoint": "admin-only",
        "required_role": "admin"
    }))
}

/// Staff-only endpoint that requires the "staff" role.
///
/// This endpoint demonstrates role-specific authorization using the
/// StaffUser guard. Only users with the "staff" role can access this endpoint.
#[cfg(feature = "test-staging")]
#[get("/1/test/staff-only")]
pub fn staff_only(staff_user: StaffUser) -> Json<Value> {
    Json(json!({
        "message": format!("Staff access granted to {}", staff_user.user.email),
        "endpoint": "staff-only",
        "required_role": "staff"
    }))
}

/// Newtown admin-only endpoint that requires the "newtown-admin" role.
///
/// This endpoint demonstrates a more specific role requirement using the
/// NewtownAdminUser guard. Only users with the "newtown-admin" role can access this endpoint.
#[cfg(feature = "test-staging")]
#[get("/1/test/newtown-admin-only")]
pub fn newtown_admin_only(newtown_admin_user: NewtownAdminUser) -> Json<Value> {
    Json(json!({
        "message": format!("Newtown admin access granted to {}", newtown_admin_user.user.email),
        "endpoint": "newtown-admin-only",
        "required_role": "newtown-admin"
    }))
}

/// Newtown staff-only endpoint that requires the "newtown-staff" role.
///
/// This endpoint demonstrates a more specific role requirement using the
/// NewtownStaffUser guard. Only users with the "newtown-staff" role can access this endpoint.
#[cfg(feature = "test-staging")]
#[get("/1/test/newtown-staff-only")]
pub fn newtown_staff_only(newtown_staff_user: NewtownStaffUser) -> Json<Value> {
    Json(json!({
        "message": format!("Newtown staff access granted to {}", newtown_staff_user.user.email),
        "endpoint": "newtown-staff-only",
        "required_role": "newtown-staff"
    }))
}

/// Endpoint requiring multiple roles (admin AND staff).
///
/// This endpoint demonstrates manual role checking for complex authorization
/// requirements. The user must have both "admin" and "staff" roles to access
/// this endpoint.
#[cfg(feature = "test-staging")]
#[get("/1/test/admin-and-staff")]
pub fn admin_and_staff(auth_user: AuthenticatedUser) -> Result<Json<Value>, Status> {
    if auth_user.has_all_roles(&["admin", "staff"]) {
        Ok(Json(json!({
            "message": format!("Multi-role access granted to {}", auth_user.user.email),
            "endpoint": "admin-and-staff",
            "required_roles": ["admin", "staff"],
            "user_roles": auth_user.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
        })))
    } else {
        Err(Status::Forbidden)
    }
}

/// Endpoint that forbids admin access (no admin role allowed).
///
/// This endpoint demonstrates negative role checking - users with the "admin"
/// role are explicitly forbidden from accessing this endpoint. This could be
/// useful for endpoints that should only be accessible to non-admin users.
#[cfg(feature = "test-staging")]
#[get("/1/test/no-admin-allowed")]
pub fn no_admin_allowed(auth_user: AuthenticatedUser) -> Result<Json<Value>, Status> {
    if auth_user.has_no_roles(&["admin"]) {
        Ok(Json(json!({
            "message": format!("Non-admin access granted to {}", auth_user.user.email),
            "endpoint": "no-admin-allowed",
            "forbidden_roles": ["admin"],
            "user_roles": auth_user.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
        })))
    } else {
        Err(Status::Forbidden)
    }
}

/// Endpoint requiring any of several roles (admin OR staff OR newtown-admin).
///
/// This endpoint demonstrates flexible role checking where users need at least
/// one of several possible roles to access the endpoint.
#[cfg(feature = "test-staging")]
#[get("/1/test/any-admin-or-staff")]
pub fn any_admin_or_staff(auth_user: AuthenticatedUser) -> Result<Json<Value>, Status> {
    if auth_user.has_any_role(&["admin", "staff", "newtown-admin"]) {
        Ok(Json(json!({
            "message": format!("Flexible role access granted to {}", auth_user.user.email),
            "endpoint": "any-admin-or-staff",
            "accepted_roles": ["admin", "staff", "newtown-admin"],
            "user_roles": auth_user.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
        })))
    } else {
        Err(Status::Forbidden)
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
pub fn routes() -> Vec<Route> {
    vec![]
}