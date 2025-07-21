//! API endpoints for user login and authentication.
//!
//! This module provides HTTP endpoints for user authentication, session management,
//! and secure API access. It handles user login requests, generates session tokens,
//! and provides authenticated endpoints.

use rocket::{post, get, Route, http::CookieJar, serde::json::Json};
use rocket::response;
use rocket::serde::{Serialize, Deserialize};

use crate::session_guards::AuthenticatedUser;
use crate::DbConn;
use crate::orm::login::process_login;
use crate::orm::user_role::get_user_roles;
use crate::orm::company::get_company_by_id;

/// Error response structure for authentication failures.
#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

/// Login success response structure containing user information.
#[derive(Serialize)]
pub struct LoginSuccessResponse {
    pub user_id: i32,
    pub email: String,
    pub company_name: String,
    pub roles: Vec<String>,
}

/// Creates a standardized user response structure for login and hello endpoints.
///
/// This function ensures both login and hello endpoints return exactly the same
/// data structure for a given user, including user_id, email, company_name, and roles.
///
/// # Arguments
/// * `db` - Database connection for fetching user roles and company information
/// * `user` - The user object to build the response for
///
/// # Returns
/// * `Ok(LoginSuccessResponse)` - Standardized user response structure
/// * `Err(response::status::Custom<Json<ErrorResponse>>)` - Database error
async fn build_user_response(
    db: &DbConn,
    user: crate::models::User,
) -> Result<LoginSuccessResponse, response::status::Custom<Json<ErrorResponse>>> {
    // Get user roles
    let user_id = user.id;
    let roles = match db.run(move |conn| {
        get_user_roles(conn, user_id)
    }).await {
        Ok(user_roles) => user_roles.into_iter().map(|role| role.name).collect(),
        Err(_) => vec![], // Return empty roles on error rather than failing
    };

    // Get company name
    let company_id = user.company_id;
    let company_name = match db.run(move |conn| {
        get_company_by_id(conn, company_id)
    }).await {
        Ok(Some(company)) => company.name,
        Ok(None) => "Unknown Company".to_string(),
        Err(_) => "Unknown Company".to_string(),
    };

    Ok(LoginSuccessResponse {
        user_id: user.id,
        email: user.email,
        company_name,
        roles,
    })
}

/// Login request structure containing user credentials.
#[derive(Clone, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Login endpoint that authenticates users and creates sessions.
///
/// - **URL:** `/api/1/login`
/// - **Method:** `POST`
/// - **Purpose:** Authenticates a user by email and password, and sets a secure session cookie
/// - **Authentication:** None required
///
/// This endpoint accepts user credentials via JSON, validates them against
/// the database, and if successful, creates a session token and sets a
/// secure HTTP-only cookie.
///
/// # Request Format
///
/// ```json
/// {
///   "email": "user@example.com",
///   "password": "userpassword"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// - No response body
/// - Sets session cookie named `session` (HTTP-only, secure, SameSite=Lax)
///
/// **Failure (HTTP 401 Unauthorized):**
/// ```json
/// { "error": "Invalid credentials" }
/// ```
///
/// # Arguments
/// * `db` - Database connection for user validation and session storage
/// * `cookies` - Cookie jar for setting the session cookie
/// * `login` - JSON payload containing email and password
///
/// # Returns
/// * `Ok(Status::Ok)` - Authentication successful, session cookie set
/// * `Err(Custom<Json<ErrorResponse>>)` - Authentication failed with error details
///
/// # Security
/// - Session cookies are HTTP-only, secure, and use SameSite=Lax
/// - Passwords are verified using Argon2 hashing
/// - Invalid credentials return generic error messages to prevent enumeration
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/login', {
///   method: 'POST',
///   headers: { 'Content-Type': 'application/json' },
///   body: JSON.stringify({
///     email: 'testuser@example.com',
///     password: 'testpassword'
///   }),
///   credentials: 'include'
/// });
/// ```
#[post("/1/login", data = "<login>")]
pub async fn login(
    db: DbConn,
    cookies: &CookieJar<'_>,
    login: Json<LoginRequest>,
) -> Result<Json<LoginSuccessResponse>, response::status::Custom<Json<ErrorResponse>>> {
    match process_login(&db, cookies, &login).await {
        Ok((_status, user)) => {
            match build_user_response(&db, user).await {
                Ok(response) => Ok(Json(response)),
                Err(err_response) => Err(err_response),
            }
        }
        Err(status) => {
            let err_json = Json(ErrorResponse { error: "Invalid credentials".to_string() });
            Err(response::status::Custom(status, err_json))
        }
    }
}

/// Hello (Authentication Check) endpoint.
///
/// - **URL:** `/api/1/hello`
/// - **Method:** `GET`
/// - **Purpose:** Returns a greeting for authenticated users; useful for checking authentication status
/// - **Authentication:** Required
///
/// This endpoint demonstrates authenticated API access by returning a
/// personalized greeting for authenticated users. The `AuthenticatedUser`
/// guard automatically validates the session cookie and returns a 401
/// Unauthorized status if authentication fails.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```text
/// Hello, user@example.com!
/// ```
///
/// **Failure (HTTP 401 Unauthorized):**
/// Session invalid or expired
///
/// # Arguments
/// * `auth_user` - Authenticated user (automatically validated by guard)
///
/// # Returns
/// * `String` - Personalized greeting with user's email
///
/// # Authentication
/// Uses the `AuthenticatedUser` request guard which automatically:
/// - Validates the session cookie
/// - Checks session expiration and revocation status
/// - Returns 401 Unauthorized if authentication fails
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/hello', {
///   method: 'GET',
///   credentials: 'include'
/// });
/// ```
#[get("/1/hello")]
pub async fn secure_hello(auth_user: AuthenticatedUser, db: DbConn) -> Result<Json<LoginSuccessResponse>, response::status::Custom<Json<ErrorResponse>>> {
    build_user_response(&db, auth_user.user).await.map(Json)
}

/// Returns all login-related API routes.
///
/// This function collects all login and authentication endpoints for
/// registration with the Rocket web framework.
///
/// # Returns
/// Vector of Route objects for login endpoints
pub fn routes() -> Vec<Route> {
    routes![login, secure_hello]
}
