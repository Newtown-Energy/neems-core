//! API version 1 - Status endpoints
//!
//! This module provides health check and status endpoints for monitoring
//! the application's operational state and availability.

use rocket::{Route, serde::json::Json};
use serde::Serialize;
use ts_rs::TS;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct HealthStatus {
    status: &'static str,
    version: &'static str,
    built: &'static str,
    git_commit: Option<&'static str>,
}

/// Health Status endpoint.
///
/// - **URL:** `/api/1/status`
/// - **Method:** `GET`
/// - **Purpose:** Returns the health status of the application
/// - **Authentication:** None required
///
/// This endpoint provides a simple health check that indicates whether
/// the application is running and responsive. It always returns a "running"
/// status if the application is operational.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "status": "running",
///   "version": "0.1.4",
///   "built": "Fri, 15 Aug 2025 18:13:43 +0000",
///   "git_commit": "cd51275141a2e7d49737aa7dd4e8ff7c9a804d67"
/// }
/// ```
///
/// # Returns
/// A JSON response containing the application's health status
#[rocket::get("/1/status")]
pub fn health_status() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "running",
        version: env!("CARGO_PKG_VERSION"),
        built: built_info::BUILT_TIME_UTC,
        git_commit: built_info::GIT_COMMIT_HASH,
    })
}

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for status endpoints
pub fn routes() -> Vec<Route> {
    routes![health_status]
}
