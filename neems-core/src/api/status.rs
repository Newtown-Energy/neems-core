//! API version 1 - Status endpoints
//!
//! This module provides health check and status endpoints for monitoring
//! the application's operational state and availability.

use rocket::Route;
use rocket::serde::json::Json;
use serde::Serialize;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct HealthStatus {
    status: &'static str,
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
///   "status": "running"
/// }
/// ```
///
/// # Returns
/// A JSON response containing the application's health status
#[rocket::get("/1/status")]
pub fn health_status() -> Json<HealthStatus> {
    Json(HealthStatus { status: "running" })
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
