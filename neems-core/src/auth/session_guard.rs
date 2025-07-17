/*

This isn't a proper guard, but rather a utility to check if a user is
authenticated.  Call it from your route handlers to get the
authenticated user.

 */

use rocket::http::CookieJar;
use diesel::prelude::*;
use chrono::Utc;

use crate::models::{User, Session};
use crate::schema::{sessions, users};
use crate::orm::login::DbRunner;

/// A guard for routes that require an authenticated user.
/// Automatically checks session cookies and validates them against the database.
#[derive(Debug)]
pub struct AuthenticatedUser {
    pub user: User,  // Contains the logged-in user's data
}

impl AuthenticatedUser {
    pub async fn from_cookies_and_db<D: DbRunner>(cookies: &CookieJar<'_>, db: &D) -> Option<User> {
        let session_cookie = cookies.get("session")?;
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
            _ => return None,
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
            _ => return None,
        };

        Some(user)
    }
}
