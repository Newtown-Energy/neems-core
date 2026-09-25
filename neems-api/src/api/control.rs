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
//! What it does report is whether a request has *registered* — whether the
//! readback reached the position asked for at any moment since the request was
//! made. That is a statement about the request, read off the readback's edges,
//! and deliberately not about where the equipment is now. See [`registered`].
//!
//! Nothing can be written to the RTAC yet. Every control in
//! [`site_controls`](neems_data::rtac::site_controls::site_controls) has no
//! write register, because the client's `Outputs` sheet is empty, so a request
//! is recorded and immediately failed with a reason an operator can read. That
//! is the honest answer, and it exercises the same path a real dispatch failure
//! takes — see `docs/site-inputs.md`.
//!
//! A demo deployment answers differently, because the question is different:
//! there is no RTAC to fail to reach and no collector to wait for, so the
//! request is carried out here and the control's readback point is moved to
//! match. See [`demo_dispatch`].

use std::collections::HashMap;

use chrono::{NaiveDateTime, Utc};
use neems_data::{
    get_all_alarm_state,
    models::AlarmStateRow,
    rtac::{
        site_controls::{SiteControl, SiteControlAction, site_control_by_id, site_controls},
        state::AlarmFlags,
    },
};
use rocket::{Route, State, http::Status, response::status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    alarm::parse_alarm_registers,
    application_rule::ErrorResponse,
    demo::{DemoMode, apply_control_readback},
    emergency_shutdown::can_access_site,
};
use crate::{
    models::{ControlRequest, ControlRequestDto, ControlRequestStatus},
    orm::{
        DbConn,
        control_request::{
            fail_control_request, get_control_request, get_latest_control_requests,
            get_pending_control_requests, mark_control_request_sent, request_control,
        },
        neems_data::db::SiteDbConn,
    },
    session_guards::AuthenticatedUser,
};

/// How long a pending request waits to reach the RTAC before it is declared
/// failed.
///
/// Matches the emergency shutdown timeout for the same reason: the collector
/// polls at 1 Hz and retries on every 10 Hz tick, so a minute without progress
/// means the signal is not going to arrive at all. This is the backstop for a
/// collector that is not running; a collector that *is* running reports its own
/// failures immediately, with a better reason.
const DISPATCH_TIMEOUT_SECONDS: i64 = 60;

/// Why a request cannot be sent while the client's `Outputs` sheet is empty.
///
/// Phrased for an operator reading it on the diagram, not for a developer
/// reading a log: it says the click was recorded, that nothing went to the
/// site, and that the reason is a missing configuration rather than a fault.
const NO_WRITE_REGISTER: &str =
    "This control has no RTAC point configured yet, so nothing was sent to the site.";

/// Why a request failed on a demo deployment, where the only thing that can go
/// wrong is this system writing to its own database.
///
/// Worth distinguishing from the real failure reasons rather than reusing one:
/// a demo that says "unreachable RTAC" would send someone looking for hardware
/// that was never in the room.
const DEMO_READBACK_FAILED: &str =
    "The demo could not record the new position, so the diagram still shows the old one.";

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

/// What the site is reporting about its readback points.
///
/// Read the way `/Alarms/Active` reads them — the newest reading that carries
/// alarm registers, unioned with `alarm_state` — so a request cannot register
/// against a position the diagram is not drawing.
struct ObservedReadbacks {
    /// The newest reading's alarm bits, or `None` if no reading carries any.
    flags: Option<AlarmFlags>,
    /// Each point's materialised data state and most recent edges.
    state: HashMap<i32, AlarmStateRow>,
}

impl ObservedReadbacks {
    /// Whether a point is set now, or `None` when nothing has ever reported it:
    /// no reading, and no transition on record.
    fn is_set(&self, alarm_num: u16) -> Option<bool> {
        let row = self.state.get(&i32::from(alarm_num));
        if self.flags.is_none() && row.is_none() {
            return None;
        }
        let in_reading = self.flags.is_some_and(|f| f.is_alarm_num_active(alarm_num));
        Some(in_reading || row.is_some_and(|r| r.data_active))
    }
}

fn observe_readbacks(
    conn: &mut diesel::SqliteConnection,
) -> Result<ObservedReadbacks, Box<dyn std::error::Error + Send + Sync>> {
    use diesel::prelude::*;
    use neems_data::schema::readings::dsl::{readings, timestamp};

    let recent: Vec<neems_data::models::Reading> =
        readings.order(timestamp.desc()).limit(10).load(conn)?;
    let flags = recent
        .iter()
        .find_map(|reading| parse_alarm_registers(&reading.data))
        .map(|registers| AlarmFlags::from_registers(&registers));

    let state = get_all_alarm_state(conn)?.into_iter().map(|row| (row.alarm_num, row)).collect();

    Ok(ObservedReadbacks { flags, state })
}

/// Read the readback points from the site database.
///
/// A failed read is an error, not "nothing reported": serving every request as
/// unregistered on no evidence would leave a client waiting on requests that
/// have long since landed.
///
/// Not scoped to a site, because the site database is not: it holds one site's
/// alarms, and `/Alarms/Active`, the emergency shutdown status and the demo's
/// readback writes all read and write it the same way. A second site would
/// share these readbacks with the first. Site-scoping alarms is #98, and this
/// endpoint is part of it: both queries here — the newest reading and
/// `alarm_state` — would need filtering by site, with `site_id` passed in to do
/// it.
async fn read_readbacks(
    site_db: &SiteDbConn,
) -> Result<ObservedReadbacks, status::Custom<Json<ErrorResponse>>> {
    site_db
        .run(observe_readbacks)
        .await
        .map_err(|e| internal_error("Error reading control readbacks", e))
}

/// Whether the site has reported the position `request` asked for since it was
/// made.
///
/// Only a `Sent` request can have registered: a pending one has not reached the
/// site, and a failed one never will. Neither can a control with no readback or
/// a row whose action does not parse — there is nothing to confirm them
/// against, and leaving a client waiting is better than clearing on no
/// evidence.
fn registered(request: &ControlRequest, observed: &ObservedReadbacks) -> bool {
    if request.status() != ControlRequestStatus::Sent {
        return false;
    }
    let Some(readback) = site_control_by_id(&request.control_id).and_then(|c| c.readback) else {
        return false;
    };
    let Ok(action) = request.action.parse::<SiteControlAction>() else {
        return false;
    };

    let row = observed.state.get(&i32::from(readback.alarm_num));
    reached_since(
        request.requested_at,
        readback.bit_for(action.resulting_position()),
        observed.is_set(readback.alarm_num),
        row.and_then(|r| r.last_rising_at),
        row.and_then(|r| r.last_falling_at),
    )
}

/// Whether a point has been in the `target` state at any moment since
/// `requested_at`, given its current state and its most recent edges.
///
/// True if it is in the target now, or entered it at or after the request, or
/// left it at or after the request *having entered it before leaving* — it was
/// already there when asked, and has moved since.
///
/// Leaving needs an entry behind it because `upsert_alarm_transition` stamps
/// the first state it sees for a point with no row, so a lone edge can be where
/// the record starts rather than a movement away from anywhere.
fn reached_since(
    requested_at: NaiveDateTime,
    target: bool,
    is_set: Option<bool>,
    last_rising_at: Option<NaiveDateTime>,
    last_falling_at: Option<NaiveDateTime>,
) -> bool {
    if is_set == Some(target) {
        return true;
    }
    let (entered, left) = if target {
        (last_rising_at, last_falling_at)
    } else {
        (last_falling_at, last_rising_at)
    };

    let entered_since = entered.is_some_and(|t| t >= requested_at);
    let left_since_being_there = match (entered, left) {
        (Some(entered), Some(left)) => entered <= left && left >= requested_at,
        _ => false,
    };
    entered_since || left_since_being_there
}

/// Serve a request with `registered` worked out against what the site reports.
fn serve(request: ControlRequest, observed: &ObservedReadbacks) -> ControlRequestDto {
    let registered = registered(&request, observed);
    ControlRequestDto::new(request, registered)
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
///
/// Each request carries `registered`, worked out against the site's readback
/// points on every read — see [`registered`].
#[get("/1/Sites/<site_id>/Controls")]
pub async fn list_site_controls(
    db: DbConn,
    site_db: SiteDbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<SiteControlDto>>, status::Custom<Json<ErrorResponse>>> {
    let observed = read_readbacks(&site_db).await?;

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
            site_controls()
                .iter()
                .map(|input| SiteControlDto {
                    id: input.id.to_string(),
                    label: input.label.to_string(),
                    actions: input.actions.iter().map(|a| a.to_string()).collect(),
                    readback_alarm_num: input.readback_alarm_num(),
                    writable: input.is_writable(),
                    latest_request: resolved
                        .iter()
                        .find(|r| r.control_id == input.id)
                        .cloned()
                        .map(|request| serve(request, &observed)),
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
    site_db: SiteDbConn,
    demo: &State<DemoMode>,
    site_id: i32,
    control_id: String,
    body: Json<ControlRequestBody>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ControlRequestDto>, status::Custom<Json<ErrorResponse>>> {
    // Validate and record. Recorded whatever happens next: the ask happened,
    // and the audit trail is the point of the row.
    let (input, requested) = db
        .run(move |conn| {
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

            Ok((input, requested))
        })
        .await?;

    // A demo deployment has no collector and no RTAC, so nothing else is coming
    // for this request. Carry it out here instead of failing it.
    if demo.enabled() && requested.status().is_unresolved() {
        let dispatched = demo_dispatch(&db, &site_db, input, requested).await?;
        let observed = read_readbacks(&site_db).await?;
        return Ok(Json(serve(dispatched, &observed)));
    }

    let resolved = db
        .run(move |conn| {
            // Nothing can carry this to the site while the control has no write
            // register, and an operator should be told that now rather than
            // after a minute of waiting.
            if !input.is_writable() && requested.status().is_unresolved() {
                fail_control_request(conn, requested.id, NO_WRITE_REGISTER.to_string())
                    .map_err(|e| internal_error("Error failing control request", e))
                    .map(|updated| updated.unwrap_or(requested))
            } else {
                fail_if_undelivered(conn, requested)
                    .map_err(|e| internal_error("Error resolving control request", e))
            }
        })
        .await?;

    // Only the collector sends a request, so one resolved here is pending or
    // failed and has nothing to have registered.
    Ok(Json(ControlRequestDto::new(resolved, false)))
}

/// Demo mode's stand-in for the collector.
///
/// Takes the same two steps, in the same order, that `neems-data` takes against
/// a real RTAC: write the site, then report the request sent. Writing first is
/// what keeps the failure path honest — if the readback cannot be moved, the
/// request fails with a reason instead of claiming a signal that went nowhere.
///
/// The two connections are separate pools and there is no transaction spanning
/// them, so the readback can land while the status update does not. That is the
/// same window the real collector has between a successful Modbus write and its
/// report back, and it resolves the same way: the readback is the authority on
/// where the equipment is, and a request stuck pending times out on its own.
///
/// Takes the action from the recorded request rather than from the body that
/// prompted this call, and takes no `action` argument at all so the two cannot
/// be confused. A second click while a signal is in flight coalesces onto the
/// request already there and does not redirect it, so the action just parsed is
/// not necessarily the one being carried out — dispatching it would move the
/// breaker whichever way the later click went while the audit row recorded the
/// earlier one.
async fn demo_dispatch(
    db: &DbConn,
    site_db: &SiteDbConn,
    input: &'static SiteControl,
    requested: ControlRequest,
) -> Result<ControlRequest, status::Custom<Json<ErrorResponse>>> {
    // Only this endpoint writes the column, and only from a parsed action, so
    // an unreadable value means the row is corrupt rather than that a caller
    // sent something odd.
    let action: SiteControlAction = requested
        .action
        .parse()
        .map_err(|_| internal_error("Unreadable action on control request", &requested.action))?;

    let applied = site_db.run(move |conn| apply_control_readback(conn, input, action)).await;

    let request_id = requested.id;
    let updated = match applied {
        Ok(()) => {
            db.run(move |conn| {
                mark_control_request_sent(conn, request_id)
                    .map_err(|e| internal_error("Error marking control request sent", e))
            })
            .await?
        }
        Err(e) => {
            eprintln!("Demo control: could not move {}'s readback: {e:?}", input.id);
            db.run(move |conn| {
                fail_control_request(conn, request_id, DEMO_READBACK_FAILED.to_string())
                    .map_err(|e| internal_error("Error failing control request", e))
            })
            .await?
        }
    };

    Ok(updated.unwrap_or(requested))
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
                out.push(ControlRequestDto::new(resolved, false));
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
    site_db: SiteDbConn,
    site_id: i32,
    request_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<ControlRequestDto>, status::Custom<Json<ErrorResponse>>> {
    let observed = read_readbacks(&site_db).await?;

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

        Ok(Json(serve(updated, &observed)))
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
    site_db: SiteDbConn,
    body: Json<ControlFailureBody>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ControlRequestDto>, status::Custom<Json<ErrorResponse>>> {
    // A late report on a request that already went out leaves it sent, and it
    // may well have registered.
    let observed = read_readbacks(&site_db).await?;

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

        Ok(Json(serve(updated, &observed)))
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

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    fn at(requested: NaiveDateTime, minutes: i64) -> Option<NaiveDateTime> {
        Some(requested + Duration::minutes(minutes))
    }

    // `reached_since`, edge by edge. Times are minutes relative to the request,
    // so each case reads as a timeline.

    #[test]
    fn nothing_reported_has_not_registered() {
        let req = Utc::now().naive_utc();
        assert!(!reached_since(req, true, None, None, None));
    }

    #[test]
    fn in_the_target_now_has_registered() {
        let req = Utc::now().naive_utc();
        assert!(reached_since(req, true, Some(true), at(req, 1), None));
    }

    /// Still where it was when asked, with nothing moving since.
    #[test]
    fn not_yet_moved_has_not_registered() {
        let req = Utc::now().naive_utc();
        assert!(!reached_since(req, true, Some(false), at(req, -10), at(req, -5)));
    }

    /// The case the flag exists for: it got there and has been moved back
    /// since, so the current state alone says it never arrived.
    #[test]
    fn moved_there_and_back_since_the_request_has_registered() {
        let req = Utc::now().naive_utc();
        assert!(reached_since(req, true, Some(false), at(req, 1), at(req, 5)));
    }

    /// Already there when asked, and moved away afterwards.
    #[test]
    fn there_before_and_moved_away_after_has_registered() {
        let req = Utc::now().naive_utc();
        assert!(reached_since(req, true, Some(false), at(req, -10), at(req, 5)));
    }

    /// A departure with no arrival on record is where the record starts, not a
    /// movement away from the target.
    #[test]
    fn leaving_with_no_entry_on_record_has_not_registered() {
        let req = Utc::now().naive_utc();
        assert!(!reached_since(req, true, Some(false), None, at(req, 5)));
    }

    /// A target of "clear" is reached by a falling edge, and a rise is not one.
    #[test]
    fn a_clear_target_is_reached_by_falling_not_rising() {
        let req = Utc::now().naive_utc();
        assert!(reached_since(req, false, Some(true), at(req, 5), at(req, 1)));
        assert!(!reached_since(req, false, Some(true), at(req, 1), None));
    }

    fn request(control_id: &str, action: &str, status: ControlRequestStatus) -> ControlRequest {
        ControlRequest {
            id: 1,
            site_id: 1,
            control_id: control_id.to_string(),
            action: action.to_string(),
            status: status.as_str().to_string(),
            requested_by: None,
            requested_at: Utc::now().naive_utc() - Duration::minutes(1),
            sent_at: None,
            resolved_at: None,
            failure_reason: None,
        }
    }

    /// A reading with exactly `set` raised, and no transitions on record.
    fn reading_with(set: &[u16]) -> ObservedReadbacks {
        let mut flags = AlarmFlags::default();
        for &alarm_num in set {
            flags.set_alarm_num(alarm_num, true);
        }
        ObservedReadbacks {
            flags: Some(flags),
            state: HashMap::new(),
        }
    }

    /// The line switches report *open* and the feeders report *closed*, so the
    /// same action registers against opposite bits. A sense read backwards
    /// would clear every badge on the wrong half of the diagram.
    #[test]
    fn registration_follows_the_readback_sense() {
        let open_switch = request("switch-89l-1", "open", ControlRequestStatus::Sent);
        assert!(registered(&open_switch, &reading_with(&[101])));
        assert!(!registered(&open_switch, &reading_with(&[])));

        let open_feeder = request("feeder-1a", "open", ControlRequestStatus::Sent);
        assert!(registered(&open_feeder, &reading_with(&[])));
        assert!(!registered(&open_feeder, &reading_with(&[607])));
    }

    /// Only a request that reached the site can have registered, even with the
    /// equipment already where it asked.
    #[test]
    fn only_a_sent_request_registers() {
        for status in [ControlRequestStatus::Pending, ControlRequestStatus::Failed] {
            let req = request("feeder-1a", "close", status);
            assert!(!registered(&req, &reading_with(&[607])), "a {status} request registered");
        }
    }
}
