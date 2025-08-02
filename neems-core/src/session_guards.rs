//! Session-based authentication and authorization guards for Rocket routes.
//!
//! This module provides request guards that automatically validate user sessions
//! and enforce role-based access control. It ensures that only authenticated
//! users with appropriate roles can access protected routes.
//!
//! # Basic Authentication
//!
//! ```rust
//! use rocket::get;
//! use neems_core::session_guards::AuthenticatedUser;
//!
//! #[get("/profile")]
//! fn get_profile(user: AuthenticatedUser) -> String {
//!     let role_names: Vec<&str> = user.roles.iter().map(|r| r.name.as_str()).collect();
//!     format!("Welcome, {}! Roles: {:?}", user.user.email, role_names)
//! }
//! ```
//!
//! # Role-Based Authorization
//!
//! ## Using Role-Specific Guards
//!
//! ```rust
//! use rocket::get;
//! use neems_core::session_guards::{AdminUser, NewtownAdminUser, StaffUser};
//!
//! #[get("/admin")]
//! fn admin_only(user: AdminUser) -> String {
//!     format!("Admin access granted to {}", user.user.email)
//! }
//!
//! #[get("/newtown-admin")]
//! fn newtown_admin_only(user: NewtownAdminUser) -> String {
//!     format!("Newtown admin access granted to {}", user.user.email)
//! }
//! ```
//!
//! ## Using Role Helper Methods
//!
//! ```rust
//! use rocket::get;
//! use neems_core::session_guards::AuthenticatedUser;
//!
//! #[get("/flexible")]
//! fn flexible_roles(user: AuthenticatedUser) -> String {
//!     if user.has_any_role(&["admin", "newtown-admin"]) {
//!         format!("Admin access for {}", user.user.email)
//!     } else if user.has_role("staff") {
//!         format!("Staff access for {}", user.user.email)
//!     } else {
//!         format!("Regular user access for {}", user.user.email)
//!     }
//! }
//! ```
//!
//! ## Manual Role Checking
//!
//! ```rust
//! use rocket::{get, http::Status};
//! use neems_core::session_guards::AuthenticatedUser;
//!
//! #[get("/conditional")]
//! fn conditional_access(user: AuthenticatedUser) -> Result<String, Status> {
//!     if user.has_all_roles(&["admin", "staff"]) {
//!         Ok(format!("Special access for {}", user.user.email))
//!     } else {
//!         Err(Status::Forbidden)
//!     }
//! }
//! ```

use chrono::Utc;
use diesel::prelude::*;
use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::{self, FromRequest, Request};

use crate::DbConn;
use crate::models::{Role, Session, User};
use crate::orm::user_role::get_user_roles;
use crate::schema::{sessions, users};

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
/// 6. Loads all roles assigned to the user
/// 7. Ensures the user has at least one role (database constraint)
///
/// # Returns
///
/// - `Outcome::Success(AuthenticatedUser)` if authentication succeeds
/// - `Outcome::Error(Status::Unauthorized)` if authentication fails or user has no roles
/// - `Outcome::Error(Status::InternalServerError)` if database connection fails
///
/// # Usage
///
/// Simply add `AuthenticatedUser` as a parameter to any route handler that
/// requires authentication:
///
/// ```rust
/// use rocket::get;
/// use neems_core::session_guards::AuthenticatedUser;
/// #[get("/protected")]
/// fn protected_route(user: AuthenticatedUser) -> String {
///     format!("Hello, {}! You have {} roles.", user.user.email, user.roles.len())
/// }
/// ```
///
/// # Role-Based Access Control
///
/// The `AuthenticatedUser` struct provides several helper methods for role checking:
///
/// - `has_role(&self, role_name: &str) -> bool` - Check if user has a specific role
/// - `has_any_role(&self, role_names: &[&str]) -> bool` - Check if user has any of the specified roles
/// - `has_all_roles(&self, role_names: &[&str]) -> bool` - Check if user has all of the specified roles
/// - `has_no_roles(&self, role_names: &[&str]) -> bool` - Check if user has none of the specified roles
#[derive(Debug)]
pub struct AuthenticatedUser {
    /// The authenticated user from the database
    pub user: User,
    /// All roles assigned to the user
    pub roles: Vec<Role>,
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
        let session_result = db
            .run(move |conn| {
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
            })
            .await;

        let session = match session_result {
            Ok(Some(sess)) => sess,
            Ok(None) => return Outcome::Error((Status::Unauthorized, ())),
            Err(e) => {
                error!("Database error finding session: {:?}", e);
                return Outcome::Error((Status::Unauthorized, ()));
            }
        };

        // Query the users table for the user associated with the session
        let user_result = db
            .run(move |conn| {
                users::table
                    .filter(users::id.eq(session.user_id))
                    .first::<User>(conn)
                    .optional()
            })
            .await;

        let user = match user_result {
            Ok(Some(u)) => u,
            Ok(None) => return Outcome::Error((Status::Unauthorized, ())),
            Err(e) => {
                error!("Database error finding user: {:?}", e);
                return Outcome::Error((Status::Unauthorized, ()));
            }
        };

        // Query all roles for the user
        let user_id = user.id;
        let roles_result = db.run(move |conn| get_user_roles(conn, user_id)).await;

        let roles = match roles_result {
            Ok(r) => {
                if r.is_empty() {
                    return Outcome::Error((Status::Unauthorized, ()));
                }
                r
            }
            Err(e) => {
                error!("Database error finding user roles: {:?}", e);
                return Outcome::Error((Status::Unauthorized, ()));
            }
        };

        Outcome::Success(AuthenticatedUser { user, roles })
    }
}

impl AuthenticatedUser {
    /// Helper method to check if the user has any of the specified roles
    pub fn has_any_role(&self, role_names: &[&str]) -> bool {
        let user_role_names: Vec<&str> = self.roles.iter().map(|r| r.name.as_str()).collect();
        role_names
            .iter()
            .any(|required| user_role_names.contains(required))
    }

    /// Helper method to check if the user has all of the specified roles
    pub fn has_all_roles(&self, role_names: &[&str]) -> bool {
        let user_role_names: Vec<&str> = self.roles.iter().map(|r| r.name.as_str()).collect();
        role_names
            .iter()
            .all(|required| user_role_names.contains(required))
    }

    /// Helper method to check if the user has none of the specified roles
    pub fn has_no_roles(&self, role_names: &[&str]) -> bool {
        let user_role_names: Vec<&str> = self.roles.iter().map(|r| r.name.as_str()).collect();
        !role_names
            .iter()
            .any(|forbidden| user_role_names.contains(forbidden))
    }

    /// Helper method to check if the user has a specific role
    pub fn has_role(&self, role_name: &str) -> bool {
        self.roles.iter().any(|r| r.name == role_name)
    }
}

/// Macro to create role-specific request guards
macro_rules! create_role_guard {
    ($name:ident, $role:expr) => {
        #[derive(Debug)]
        pub struct $name {
            pub user: User,
            pub roles: Vec<Role>,
        }

        #[rocket::async_trait]
        impl<'r> FromRequest<'r> for $name {
            type Error = ();

            async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
                let auth_user = match AuthenticatedUser::from_request(request).await {
                    Outcome::Success(user) => user,
                    Outcome::Error(e) => return Outcome::Error(e),
                    Outcome::Forward(f) => return Outcome::Forward(f),
                };

                if auth_user.has_role($role) {
                    Outcome::Success($name {
                        user: auth_user.user,
                        roles: auth_user.roles,
                    })
                } else {
                    Outcome::Error((Status::Forbidden, ()))
                }
            }
        }
    };
}

// Create guards for common roles

// A request guard that requires the user to have the "admin" role.
//
// This guard automatically validates both authentication and authorization,
// ensuring the user is logged in and has the "admin" role.
//
// # Returns
//
// - `Outcome::Success(AdminUser)` if user is authenticated and has "admin" role
// - `Outcome::Error(Status::Forbidden)` if user is authenticated but lacks "admin" role
// - `Outcome::Error(Status::Unauthorized)` if user is not authenticated
//
// # Usage
//
// ```rust
// use rocket::get;
// use neems_core::session_guards::AdminUser;
//
// #[get("/admin-panel")]
// fn admin_panel(user: AdminUser) -> String {
//     format!("Admin panel access for {}", user.user.email)
// }
// ```
create_role_guard!(AdminUser, "admin");

// A request guard that requires the user to have the "newtown-admin" role.
create_role_guard!(NewtownAdminUser, "newtown-admin");

// A request guard that requires the user to have the "newtown-staff" role.
create_role_guard!(NewtownStaffUser, "newtown-staff");

// A request guard that requires the user to have the "staff" role.
create_role_guard!(StaffUser, "staff");

/// A more flexible role guard that can be configured at runtime
#[derive(Debug)]
pub struct RoleGuard {
    pub user: User,
    pub roles: Vec<Role>,
    pub required_roles: Vec<String>,
}

impl RoleGuard {
    pub fn new(required_roles: Vec<String>) -> Self {
        Self {
            user: User {
                id: 0,
                email: String::new(),
                password_hash: String::new(),
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
                company_id: 0,
                totp_secret: None,
            },
            roles: Vec::new(),
            required_roles,
        }
    }

    pub async fn check_request<'r>(
        request: &'r Request<'_>,
        required_roles: &[String],
    ) -> request::Outcome<AuthenticatedUser, ()> {
        let auth_user = match AuthenticatedUser::from_request(request).await {
            Outcome::Success(user) => user,
            Outcome::Error(e) => return Outcome::Error(e),
            Outcome::Forward(f) => return Outcome::Forward(f),
        };

        if required_roles.is_empty()
            || auth_user.has_any_role(
                &required_roles
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>(),
            )
        {
            Outcome::Success(auth_user)
        } else {
            Outcome::Error((Status::Forbidden, ()))
        }
    }
}
