//! API endpoints for reading the entity-activity audit log.
//!
//! The demo's "Resulting Schedule" pane shows who created or last
//! edited each rule and library item for the selected day. The audit
//! log already stores those rows (see `entity_activity` table and the
//! `update_latest_activity_user` helper); this module just exposes it
//! over HTTP with the acting user's email resolved.

use rocket::{Route, http::Status, response::status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    orm::{
        DbConn,
        entity_activity::get_activity_history,
        user::get_user,
    },
    session_guards::AuthenticatedUser,
};

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// A single audit-log row with the acting user's email resolved so the
/// frontend doesn't have to round-trip per row to render "edited by
/// alice@example.com at 4:32 pm".
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntityActivityWithUser {
    pub id: i32,
    pub table_name: String,
    pub entity_id: i32,
    pub operation_type: String,
    /// ISO-8601 string in UTC.
    pub timestamp: String,
    pub user_id: Option<i32>,
    pub user_email: Option<String>,
    /// Free-form reason provided at the API layer (S1b). Backfilled
    /// after the trigger writes the activity row; NULL for
    /// non-update operations or callers that didn't provide one.
    pub change_reason: Option<String>,
}

/// Query parameters for [get_entity_activity].
#[derive(Debug, Deserialize, rocket::FromForm)]
pub struct EntityActivityQuery {
    pub table_name: String,
    pub entity_id: i32,
}

/// Return the audit log for a single entity, oldest first.
///
/// - **URL:** `/api/1/EntityActivity?table_name=<table>&entity_id=<id>`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Any authenticated user can read; the audit surface is intentionally
/// broad because the demo's "Resulting Schedule" pane needs it visible
/// across roles. Tighten if this becomes a real-world endpoint.
#[get("/1/EntityActivity?<query..>")]
pub async fn get_entity_activity(
    db: DbConn,
    query: EntityActivityQuery,
    _auth_user: AuthenticatedUser,
) -> Result<Json<Vec<EntityActivityWithUser>>, status::Custom<Json<ErrorResponse>>> {
    let table_name = query.table_name.clone();
    let entity_id = query.entity_id;

    db.run(move |conn| {
        let rows = match get_activity_history(conn, &table_name, entity_id) {
            Ok(r) => r,
            Err(e) => {
                let err = Json(ErrorResponse { error: e.to_string() });
                return Err(status::Custom(Status::InternalServerError, err));
            }
        };

        let mut out: Vec<EntityActivityWithUser> = Vec::with_capacity(rows.len());
        for row in rows {
            // Look up the user email if we have one. Failures here are
            // not fatal — the row just renders without an email.
            let email = match row.user_id {
                Some(uid) => match get_user(conn, uid) {
                    Ok(Some(u)) => Some(u.email),
                    _ => None,
                },
                None => None,
            };
            out.push(EntityActivityWithUser {
                id: row.id,
                table_name: row.table_name,
                entity_id: row.entity_id,
                operation_type: row.operation_type,
                timestamp: row.timestamp.and_utc().to_rfc3339(),
                user_id: row.user_id,
                user_email: email,
                change_reason: row.change_reason,
            });
        }
        Ok(Json(out))
    })
    .await
}

pub fn routes() -> Vec<Route> {
    routes![get_entity_activity]
}
