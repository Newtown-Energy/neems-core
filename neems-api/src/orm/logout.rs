//! Database operations for user logout and session revocation.
//!
//! This module provides database layer functions for session termination,
//! including session revocation and cleanup operations.

use diesel::prelude::*;

use crate::{DbConn, schema::sessions::dsl::*};

/// Revokes a session by marking it as revoked in the database.
///
/// This function updates the session record in the database to mark it as
/// revoked, effectively terminating the user's session. The session token
/// becomes invalid after this operation.
///
/// # Arguments
/// * `db` - Database connection for updating the session
/// * `session_id` - Session token to revoke
///
/// # Returns
/// * `Ok(usize)` - Number of rows affected (should be 1 if successful)
/// * `Err(diesel::result::Error)` - Database operation failed
///
/// # Behavior
/// - Updates the `revoked` field to `true` for the matching session
/// - Does not delete the session record (maintains audit trail)
/// - Returns the number of affected rows for verification
///
/// # Security
/// - Ensures session cannot be reused after revocation
/// - Maintains session history for security auditing
/// - Gracefully handles non-existent session IDs
pub async fn revoke_session(db: &DbConn, session_id: &str) -> Result<usize, diesel::result::Error> {
    let session_id = session_id.to_string();
    db.run(move |conn| {
        diesel::update(sessions.filter(id.eq(&session_id)))
            .set(revoked.eq(true))
            .execute(conn)
    })
    .await
}
