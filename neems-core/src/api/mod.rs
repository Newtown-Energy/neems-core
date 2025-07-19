//! API module containing all HTTP endpoint handlers.
//!
//! This module organizes the various API endpoints into separate submodules
//! and provides a unified interface for collecting all routes for registration
//! with the Rocket web framework.

#[cfg(feature = "fixphrase")]
pub mod fixphrase;
pub mod institution;
pub mod login;
pub mod logout;
pub mod role;
pub mod secure_test;
pub mod status;
pub mod user;

use rocket::Route;

/// Collects all routes from all API submodules.
///
/// This function gathers route handlers from all API submodules (fixphrase,
/// institution, login, logout, role, secure_test, status, and user) and returns them as a single vector
/// for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers from all API submodules
pub fn routes() -> Vec<Route> {
    let mut routes = Vec::new();
    #[cfg(feature = "fixphrase")]
    routes.extend(fixphrase::routes());
    routes.extend(institution::routes());
    routes.extend(login::routes());
    routes.extend(logout::routes());
    routes.extend(role::routes());
    routes.extend(secure_test::routes());
    routes.extend(status::routes());
    routes.extend(user::routes());
    routes
}