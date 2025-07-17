//! Session-based authentication guard for Rocket routes.
//! 
//! This module provides a request guard that automatically validates user sessions
//! by checking session cookies against the database. It ensures that only authenticated
//! users can access protected routes.
//!
//! # Example
//! 
//! ```rust
//! use rocket::get;
//! use neems_core::auth::session_guard::AuthenticatedUser;
//! 
//! #[get("/profile")]
//! fn get_profile(user: AuthenticatedUser) -> String {
//!     format!("Welcome, {}!", user.user.email)
//! }
//! ```

use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::outcome::Outcome;
use diesel::prelude::*;
use chrono::Utc;

use crate::models::{User, Session};
use crate::schema::{sessions, users};
use crate::DbConn;

/// A request guard for routes that require an authenticated user.
/// 
/// This guard automatically validates session cookies and ensures the user
/// is authenticated before allowing access to protected routes. It performs
/// the following checks:
/// 
/// 1. Extracts the session cookie from the request
/// 2. Validates the session exists in the database
/// 3. Checks that the session is not revoked
/// 4. Verifies the session has not expired
/// 5. Retrieves the associated user from the database
/// 
/// # Returns
/// 
/// - `Outcome::Success(AuthenticatedUser)` if authentication succeeds
/// - `Outcome::Error(Status::Unauthorized)` if authentication fails
/// - `Outcome::Error(Status::InternalServerError)` if database connection fails
/// 
/// # Usage
/// 
/// Simply add `AuthenticatedUser` as a parameter to any route handler that
/// requires authentication:
/// 
/// ```rust
/// use rocket::get;
/// use neems_core::auth::session_guard::AuthenticatedUser;
/// #[get("/protected")]
/// fn protected_route(user: AuthenticatedUser) -> String {
///     format!("Hello, {}!", user.user.email)
/// }
/// ```
#[derive(Debug)]
pub struct AuthenticatedUser {
    /// The authenticated user from the database
    pub user: User,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = ();

    /// Extracts and validates an authenticated user from the request.
    /// 
    /// This method implements the core authentication logic by:
    /// 1. Extracting the database connection from the request
    /// 2. Reading the "session" cookie from the request
    /// 3. Querying the sessions table to find a valid, non-revoked, non-expired session
    /// 4. Retrieving the associated user from the users table
    /// 
    /// # Arguments
    /// 
    /// * `request` - The incoming HTTP request containing cookies and database connection
    /// 
    /// # Returns
    /// 
    /// * `Outcome::Success(AuthenticatedUser)` - Valid session with authenticated user
    /// * `Outcome::Error(Status::Unauthorized)` - Invalid/missing session or user
    /// * `Outcome::Error(Status::InternalServerError)` - Database connection failure
    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let cookies = request.cookies();
        let db = match request.guard::<DbConn>().await {
            Outcome::Success(db) => db,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        // Get session cookie
        let session_cookie = match cookies.get("session") {
            Some(cookie) => cookie,
            None => return Outcome::Error((Status::Unauthorized, ())),
        };

        let session_id = session_cookie.value().to_string();

        // Query the sessions table for a valid session
        let session_result = db.run(move |conn| {
            sessions::table
                .filter(sessions::id.eq(&session_id))
                .filter(sessions::revoked.eq(false))
                .filter(
                    sessions::expires_at
                        .is_null()
                        .or(sessions::expires_at.gt(Utc::now().naive_utc())),
                )
                .first::<Session>(conn)
                .optional()
        }).await;

        let session = match session_result {
            Ok(Some(sess)) => sess,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        // Query the users table for the user associated with the session
        let user_result = db.run(move |conn| {
            users::table
                .filter(users::id.eq(session.user_id))
                .first::<User>(conn)
                .optional()
        }).await;

        let user = match user_result {
            Ok(Some(u)) => u,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        Outcome::Success(AuthenticatedUser { user })
    }
}
