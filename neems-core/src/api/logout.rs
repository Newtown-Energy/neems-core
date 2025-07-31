//! API endpoint for user logout and session termination.
//!
//! This module provides HTTP endpoints for user logout functionality,
//! including session revocation and cookie cleanup.

use rocket::{post, http::{Cookie, CookieJar}, Route};
use rocket::serde::json::{Json, json, Value};
use crate::DbConn;
use crate::orm::logout::revoke_session;

/// Logout endpoint that terminates user sessions.
///
/// - **URL:** `/api/1/logout`
/// - **Method:** `POST`
/// - **Purpose:** Terminates the current session and removes the session cookie
/// - **Authentication:** None required (works with or without valid session)
///
/// This endpoint handles user logout by revoking the current session in the database
/// and removing the session cookie from the client. It safely handles cases where
/// no session cookie is present.
///
/// # Response
///
/// **Always returns HTTP 200 OK** - Success regardless of session state
///
/// # Arguments
/// * `db` - Database connection for session revocation
/// * `cookies` - Cookie jar containing the session cookie to remove
///
/// # Returns
/// * `Json<Value>` - Always returns JSON success message, regardless of session state
///
/// # Behavior
/// - Extracts the session token from the "session" cookie
/// - Marks the session as revoked in the database
/// - Removes the session cookie from the client
/// - Returns success even if no session cookie exists
///
/// # Security
/// - Safely handles missing or invalid session cookies
/// - Ensures session is properly revoked in the database
/// - Removes client-side session cookie to prevent reuse
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/logout', {
///   method: 'POST',
///   credentials: 'include'
/// });
/// ```
#[post("/1/logout")]
pub async fn logout(
    db: DbConn,
    cookies: &CookieJar<'_>,
) -> Json<Value> {
    // Get the cookie value first without holding a reference
    let cookie_value = cookies.get("session").map(|c| c.value().to_string());
    
    if let Some(session_id) = cookie_value {
        // Mark session as revoked in DB
        let _ = revoke_session(&db, &session_id).await;
        
        // Remove cookie
        cookies.remove(Cookie::from("session"));
    }
    
    Json(json!({
        "message": "Logout successful",
        "status": "ok"
    }))
}

/// Returns all logout-related API routes.
///
/// This function collects all logout endpoints for registration with the
/// Rocket web framework.
///
/// # Returns
/// Vector of Route objects for logout endpoints
pub fn routes() -> Vec<Route> {
    routes![logout]
}