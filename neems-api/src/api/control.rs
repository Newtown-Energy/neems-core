//! API endpoints for operator requests against a site input.
//!
//! A click on the single-line diagram — open a breaker, close a switch — is a
//! *request to send a signal*. This module records that request and reports its
//! lifecycle; it never reports equipment position. Where a breaker actually is
//! comes from its readback point through the alarm endpoints, independently and
//! continuously, and the two must be read as separate axes: a `sent` request
//! whose breaker has not moved is information about the site, not a failure of
//! the request.
//!
//! Nothing can be written to the RTAC yet. Every control in
//! [`SITE_CONTROLS`](neems_data::rtac::site_controls::SITE_CONTROLS) has no
//! write register, because the client's `Outputs` sheet is empty, so a request
//! is recorded and immediately failed with a reason an operator can read. That
//! is the honest answer, and it exercises the same path a real dispatch failure
//! takes — see `docs/site-inputs.md`.

use chrono::Utc;
use neems_data::rtac::site_controls::{SITE_CONTROLS, SiteControlAction, site_control_by_id};
use rocket::{Route, http::Status, response::status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{application_rule::ErrorResponse, estop::can_access_site};
use crate::{
    models::ControlRequestDto,
    orm::{
        DbConn,
        control_request::{
            fail_control_request, get_control_request, get_latest_control_requests,
            get_pending_control_requests, mark_control_request_sent, request_control,
        },
    },
    session_guards::AuthenticatedUser,
};

/// How long a pending request waits to reach the RTAC before it is declared
/// failed.
///
/// Matches the E-stop's timeout for the same reason: the collector polls at
/// 1 Hz and retries on every 10 Hz tick, so a minute without progress means the
/// signal is not going to arrive at all. This is the backstop for a collector
/// that is not running; a collector that *is* running reports its own failures
/// immediately, with a better reason.
const DISPATCH_TIMEOUT_SECONDS: i64 = 60;

/// Why a request cannot be sent while the client's `Outputs` sheet is empty.
///
/// Phrased for an operator reading it on the diagram, not for a developer
/// reading a log: it says the click was recorded, that nothing went to the
/// site, and that the reason is a missing configuration rather than a fault.
const NO_WRITE_REGISTER: &str =
    "This control has no RTAC point configured yet, so nothing was sent to the site.";

/// One control, as served to clients.
///
/// The frontend renders one interactable element per entry and refuses to offer
/// an action this does not list, so the diagram and the backend cannot drift
/// into offering a click that will always be refused.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SiteControlDto {
    /// Stable id, matching the React SLD component id.
    pub id: String,
    pub label: String,
    /// Actions the control accepts: `open`, `close`, `trip`.
    pub actions: Vec<String>,
    /// The digital point reporting the resulting state. The only thing entitled
    /// to drive the position drawn on the diagram.
    pub readback_alarm_num: Option<u16>,
    /// Whether a request against this control could reach the site at all.
    /// False for every control until the `Outputs` sheet lands — the UI uses it
    /// to warn *before* the click rather than only explain after it.
    pub writable: bool,
    /// The most recent request for this control, whatever its status, or `null`
    /// if it has never been asked for anything.
    pub latest_request: Option<ControlRequestDto>,
}

/// Body of a control request: which action is being asked for.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ControlRequestBody {
    pub action: String,
}

/// Body of a collector's failure report: why the signal did not get out.
///
/// The text is shown to an operator verbatim, so the collector is responsible
/// for writing something a person can act on.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ControlFailureBody {
    pub reason: String,
}

fn forbidden() -> status::Custom<Json<ErrorResponse>> {
    status::Custom(
        Status::Forbidden,
        Json(ErrorResponse {
            error: "Forbidden: insufficient permissions".to_string(),
        }),
    )
}

fn not_found(what: &str) -> status::Custom<Json<ErrorResponse>> {
    status::Custom(Status::NotFound, Json(ErrorResponse { error: what.to_string() }))
}

fn bad_request(what: String) -> status::Custom<Json<ErrorResponse>> {
    status::Custom(Status::BadRequest, Json(ErrorResponse { error: what }))
}

fn internal_error(context: &str, e: impl std::fmt::Debug) -> status::Custom<Json<ErrorResponse>> {
    eprintln!("{context}: {e:?}");
    status::Custom(
        Status::InternalServerError,
        Json(ErrorResponse {
            error: "Internal server error".to_string(),
        }),
    )
}

/// Fail a request whose signal has had long enough to get out and hasn't.
///
/// Applied on read rather than by a background sweep, so a request nothing ever
/// picked up resolves whether or not anyone is watching, and without a timer to
/// keep alive.
fn fail_if_undelivered(
    conn: &mut diesel::SqliteConnection,
    request: crate::models::ControlRequest,
) -> Result<crate::models::ControlRequest, diesel::result::Error> {
    if !request.status().is_unresolved() {
        return Ok(request);
    }

    let age = Utc::now().naive_utc() - request.requested_at;
    if age.num_seconds() < DISPATCH_TIMEOUT_SECONDS {
        return Ok(request);
    }

    let id = request.id;
    Ok(fail_control_request(
        conn,
        id,
        "The signal did not reach the site controller in time.".to_string(),
    )?
    .unwrap_or(request))
}

/// List a site's controls and the latest request against each.
///
/// - **URL:** `/api/1/Sites/<site_id>/Controls`
/// - **Method:** `GET`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// The control list is compiled in rather than stored: it describes the site's
/// equipment and the code that knows how to write it, not data an operator can
/// edit. Serving it keeps the diagram from carrying its own copy of which
/// elements are interactable.
#[get("/1/Sites/<site_id>/Controls")]
pub async fn list_site_controls(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<SiteControlDto>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        let latest = get_latest_control_requests(conn, site_id)
            .map_err(|e| internal_error("Error loading control requests", e))?;

        let mut resolved = Vec::with_capacity(latest.len());
        for row in latest {
            resolved.push(
                fail_if_undelivered(conn, row)
                    .map_err(|e| internal_error("Error resolving control request", e))?,
            );
        }

        Ok(Json(
            SITE_CONTROLS
                .iter()
                .map(|input| SiteControlDto {
                    id: input.id.to_string(),
                    label: input.label.to_string(),
                    actions: input.actions.iter().map(|a| a.to_string()).collect(),
                    readback_alarm_num: input.readback_alarm_num,
                    writable: input.is_writable(),
                    latest_request: resolved
                        .iter()
                        .find(|r| r.control_id == input.id)
                        .cloned()
                        .map(ControlRequestDto::from),
                })
                .collect(),
        ))
    })
    .await
}

/// Request an action on one control.
///
/// - **URL:** `/api/1/Sites/<site_id>/Controls/<control_id>/Requests`
/// - **Method:** `POST`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// Returns the request, which the caller should render rather than assuming
/// anything about the equipment: it may come back `pending` (recorded, on its
/// way), or `failed` (recorded, and it is not going anywhere — with the reason
/// to show). It never comes back describing a breaker position.
///
/// Clicking again before the signal goes out returns the request already in
/// flight rather than creating a second one.
#[post("/1/Sites/<site_id>/Controls/<control_id>/Requests", data = "<body>")]
pub async fn request_site_control(
    db: DbConn,
    site_id: i32,
    control_id: String,
    body: Json<ControlRequestBody>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ControlRequestDto>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        let Some(input) = site_control_by_id(&control_id) else {
            return Err(not_found("Unknown control"));
        };

        let action: SiteControlAction = body
            .action
            .parse()
            .map_err(|_| bad_request(format!("Unknown action: {}", body.action)))?;

        if !input.accepts(action) {
            return Err(bad_request(format!(
                "{} does not accept the action {action}",
                input.label
            )));
        }

        let requested =
            request_control(conn, site_id, input.id, action.as_str(), Some(auth_user.user.id))
                .map_err(|e| internal_error("Error recording control request", e))?;

        // Nothing can carry this to the site while the control has no write
        // register, and an operator should be told that now rather than after a
        // minute of waiting. Recorded first regardless: the ask happened, and
        // the audit trail is the point of the row.
        let resolved = if !input.is_writable() && requested.status().is_unresolved() {
            fail_control_request(conn, requested.id, NO_WRITE_REGISTER.to_string())
                .map_err(|e| internal_error("Error failing control request", e))?
                .unwrap_or(requested)
        } else {
            fail_if_undelivered(conn, requested)
                .map_err(|e| internal_error("Error resolving control request", e))?
        };

        Ok(Json(ControlRequestDto::from(resolved)))
    })
    .await
}

/// Get the requests the collector should act on.
///
/// - **URL:** `/api/1/Sites/<site_id>/Controls/Pending`
/// - **Method:** `GET`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// The timeout is applied here too, so a request nothing ever picked up
/// resolves on the collector's own polling rather than depending on an
/// operator's browser being open.
#[get("/1/Sites/<site_id>/Controls/Pending")]
pub async fn get_pending_site_controls(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<ControlRequestDto>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        let pending = get_pending_control_requests(conn, site_id)
            .map_err(|e| internal_error("Error loading pending control requests", e))?;

        let mut out = Vec::with_capacity(pending.len());
        for row in pending {
            let resolved = fail_if_undelivered(conn, row)
                .map_err(|e| internal_error("Error resolving control request", e))?;
            // A request that just timed out is no longer outstanding work.
            if resolved.status().is_unresolved() {
                out.push(ControlRequestDto::from(resolved));
            }
        }

        Ok(Json(out))
    })
    .await
}

/// Report that a control request has been written to the RTAC.
///
/// - **URL:** `/api/1/Sites/<site_id>/Controls/Requests/<request_id>/Sent`
/// - **Method:** `POST`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// Called by the neems-data collector once its Modbus write has actually
/// succeeded — not when the command was queued. This resolves the request: the
/// signal is out, which is what was asked for.
///
/// Idempotent: reporting on an already-resolved request leaves it unchanged, so
/// a duplicate report cannot restate when a signal went out.
#[post("/1/Sites/<site_id>/Controls/Requests/<request_id>/Sent")]
pub async fn mark_site_control_sent(
    db: DbConn,
    site_id: i32,
    request_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<ControlRequestDto>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        // Scope the lookup to the site so a request id from elsewhere cannot be
        // advanced through this site's endpoint.
        if get_control_request(conn, site_id, request_id)
            .map_err(|e| internal_error("Error loading control request", e))?
            .is_none()
        {
            return Err(not_found("Control request not found"));
        }

        let updated = mark_control_request_sent(conn, request_id)
            .map_err(|e| internal_error("Error marking control request sent", e))?
            .ok_or_else(|| not_found("Control request not found"))?;

        Ok(Json(ControlRequestDto::from(updated)))
    })
    .await
}

/// Report that a control request could not be written to the RTAC.
///
/// - **URL:** `/api/1/Sites/<site_id>/Controls/Requests/<request_id>/Failed`
/// - **Method:** `POST`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// The collector knows why a write failed — unreachable RTAC, refused write —
/// and an operator is better served by that than by a generic timeout a minute
/// later. Only a pending request moves, so a late report cannot contradict a
/// signal that did get out.
#[post(
    "/1/Sites/<site_id>/Controls/Requests/<request_id>/Failed",
    data = "<body>"
)]
pub async fn mark_site_control_failed(
    db: DbConn,
    site_id: i32,
    request_id: i32,
    body: Json<ControlFailureBody>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ControlRequestDto>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        if get_control_request(conn, site_id, request_id)
            .map_err(|e| internal_error("Error loading control request", e))?
            .is_none()
        {
            return Err(not_found("Control request not found"));
        }

        let updated = fail_control_request(conn, request_id, body.reason.clone())
            .map_err(|e| internal_error("Error failing control request", e))?
            .ok_or_else(|| not_found("Control request not found"))?;

        Ok(Json(ControlRequestDto::from(updated)))
    })
    .await
}

pub fn routes() -> Vec<Route> {
    routes![
        list_site_controls,
        request_site_control,
        get_pending_site_controls,
        mark_site_control_sent,
        mark_site_control_failed
    ]
}
