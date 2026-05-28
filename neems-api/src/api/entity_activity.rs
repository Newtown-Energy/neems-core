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
    orm::{DbConn, entity_activity::get_activity_history, user::get_user},
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

/// A single row in the per-site recent-schedule-activity feed (S1c-4).
/// Adds a human-readable label so the frontend can render
/// "Weeknight Discharge — Edited commands by alice@example.com"
/// without a per-row round-trip to look up the library item.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct RecentScheduleActivityEntry {
    pub id: i32,
    pub table_name: String,
    pub entity_id: i32,
    pub operation_type: String,
    pub timestamp: String,
    pub user_id: Option<i32>,
    pub user_email: Option<String>,
    pub change_reason: Option<String>,
    /// Library item this activity belongs to. For
    /// `schedule_templates` rows this is the item itself; for
    /// `application_rules` rows it's the rule's parent template.
    pub library_item_id: i32,
    pub library_item_name: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct RecentScheduleActivityResponse {
    pub site_id: i32,
    pub entries: Vec<RecentScheduleActivityEntry>,
}

/// Merged recent-activity feed for a site's schedule library +
/// application rules, newest first.
///
/// - **URL:** `/api/1/Sites/<site_id>/RecentScheduleActivity?<limit>`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Default limit is 50; max is 500 to keep response shapes bounded.
#[get("/1/Sites/<site_id>/RecentScheduleActivity?<limit>")]
pub async fn get_site_recent_schedule_activity(
    db: DbConn,
    site_id: i32,
    limit: Option<i64>,
    _auth_user: AuthenticatedUser,
) -> Result<Json<RecentScheduleActivityResponse>, status::Custom<Json<ErrorResponse>>> {
    let limit = limit.unwrap_or(50).clamp(1, 500);

    db.run(move |conn| {
        use diesel::prelude::*;

        use crate::{
            models::EntityActivity,
            schema::{application_rules, entity_activity, schedule_templates},
        };

        // Load library items for this site so we can both filter
        // activity rows and surface item names in the response.
        let items: Vec<(i32, String)> = schedule_templates::table
            .filter(schedule_templates::site_id.eq(site_id))
            .select((schedule_templates::id, schedule_templates::name))
            .load::<(i32, String)>(conn)
            .map_err(|e| {
                eprintln!("Error loading library items for activity feed: {:?}", e);
                let err = Json(ErrorResponse { error: e.to_string() });
                status::Custom(Status::InternalServerError, err)
            })?;

        if items.is_empty() {
            return Ok(Json(RecentScheduleActivityResponse { site_id, entries: vec![] }));
        }

        let item_ids: Vec<i32> = items.iter().map(|(id, _)| *id).collect();
        let name_by_item_id: std::collections::HashMap<i32, String> = items.into_iter().collect();

        // Map application_rule ids → their parent library item id so
        // we can join back to the item name in the response.
        let rule_to_item: std::collections::HashMap<i32, i32> = application_rules::table
            .filter(application_rules::template_id.eq_any(&item_ids))
            .select((application_rules::id, application_rules::template_id))
            .load::<(i32, i32)>(conn)
            .map_err(|e| {
                eprintln!("Error loading application rules for activity feed: {:?}", e);
                let err = Json(ErrorResponse { error: e.to_string() });
                status::Custom(Status::InternalServerError, err)
            })?
            .into_iter()
            .collect();

        let rule_ids: Vec<i32> = rule_to_item.keys().copied().collect();

        // Pull the union of template + rule activity in one query,
        // newest first, capped by the requested limit.
        let activity: Vec<EntityActivity> = entity_activity::table
            .filter(
                (entity_activity::table_name
                    .eq("schedule_templates")
                    .and(entity_activity::entity_id.eq_any(&item_ids)))
                .or(entity_activity::table_name
                    .eq("application_rules")
                    .and(entity_activity::entity_id.eq_any(&rule_ids))),
            )
            .order(entity_activity::timestamp.desc())
            .limit(limit)
            .load::<EntityActivity>(conn)
            .map_err(|e| {
                eprintln!("Error loading recent schedule activity: {:?}", e);
                let err = Json(ErrorResponse { error: e.to_string() });
                status::Custom(Status::InternalServerError, err)
            })?;

        let mut entries: Vec<RecentScheduleActivityEntry> = Vec::with_capacity(activity.len());
        for row in activity {
            // Resolve back to a library item — the row's entity is
            // either the item itself or one of its rules.
            let library_item_id = if row.table_name == "schedule_templates" {
                row.entity_id
            } else {
                match rule_to_item.get(&row.entity_id) {
                    Some(item_id) => *item_id,
                    None => continue, // Stale row whose rule is gone; skip rather than 500.
                }
            };
            let library_item_name = name_by_item_id
                .get(&library_item_id)
                .cloned()
                .unwrap_or_else(|| format!("Item #{}", library_item_id));

            let user_email =
                row.user_id.and_then(|uid| get_user(conn, uid).ok().flatten().map(|u| u.email));

            entries.push(RecentScheduleActivityEntry {
                id: row.id,
                table_name: row.table_name,
                entity_id: row.entity_id,
                operation_type: row.operation_type,
                timestamp: row.timestamp.and_utc().to_rfc3339(),
                user_id: row.user_id,
                user_email,
                change_reason: row.change_reason,
                library_item_id,
                library_item_name,
            });
        }

        Ok(Json(RecentScheduleActivityResponse { site_id, entries }))
    })
    .await
}

pub fn routes() -> Vec<Route> {
    routes![get_entity_activity, get_site_recent_schedule_activity]
}
