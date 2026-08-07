//! API endpoints for operator-requested emergency stops.
//!
//! E-stop state is **read from the site**, never written by a client. What a
//! client can do is *request* a trip, and what this system then owes is to get
//! that signal to the RTAC — no more. A request is therefore done once the
//! collector has written it; how the RTAC acts on an E-stop is the RTAC's
//! business, reported independently and continuously as alarm 104.
//!
//! Those two things are kept apart on purpose. `observed_active` says whether
//! the site is tripped; the request says whether the operator's ask got out.
//! Either can be true without the other.
//!
//! Engage-only: there is no endpoint to clear an E-stop. A latched E-stop is
//! cleared on site, after which alarm 104 drops and the observed state follows
//! on its own.

use chrono::Utc;
use neems_data::rtac::{alarm_definitions::ESTOP_ALARM_NUM, state::AlarmFlags};
use rocket::{Route, State, http::Status, response::status, serde::json::Json};

use super::application_rule::ErrorResponse;
use crate::{
    api::alarm::{DemoForcedAlarms, parse_alarm_registers},
    models::{EstopRequestDto, EstopRequestStatus, EstopStatusResponse},
    orm::{
        DbConn,
        estop::{
            get_estop_request, get_latest_estop_request, get_unresolved_estop_request,
            mark_estop_dispatched, request_estop, resolve_estop_request,
        },
        neems_data::db::SiteDbConn,
        site::get_site_by_id,
    },
    session_guards::AuthenticatedUser,
};

/// How long a pending request waits to reach the RTAC before it is declared
/// failed.
///
/// The collector polls at 1 Hz and its worker retries the write on every 10 Hz
/// tick, so a minute is a long time to have got nowhere. This covers the cases
/// where the signal is not going to arrive at all: the collector is not running
/// (`RTAC_ENABLED` unset), it has no credentials, or the RTAC is unreachable.
/// An operator has to be told that, rather than watching a spinner.
const DISPATCH_TIMEOUT_SECONDS: i64 = 60;

/// Whether the user may see or act on this site.
///
/// Mirrors the schedule endpoints' rule: Newtown staff see every site, everyone
/// else sees their own company's.
fn can_access_site(
    user: &AuthenticatedUser,
    site_id: i32,
    conn: &mut diesel::SqliteConnection,
) -> bool {
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }
    if let Ok(Some(site)) = get_site_by_id(conn, site_id) {
        return site.company_id == user.user.company_id;
    }
    false
}

fn forbidden() -> status::Custom<Json<ErrorResponse>> {
    status::Custom(
        Status::Forbidden,
        Json(ErrorResponse {
            error: "Forbidden: insufficient permissions".to_string(),
        }),
    )
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

/// What the RTAC last reported about the E-stop line.
struct ObservedEstop {
    active: bool,
    observed_at: Option<chrono::NaiveDateTime>,
    age_seconds: Option<i64>,
}

/// Read alarm 104 from the most recent reading that carries alarm registers.
///
/// Like `/1/Alarms/Active`, this reads the single site database rather than
/// selecting per site — the deployment is single-site today — and overlays the
/// demo forced-alarm set. Both endpoints must agree about alarm 104; two
/// answers to "is the site tripped" is exactly the sort of split-brain this
/// whole feature exists to remove.
///
/// When no reading carries alarm data, `active` is false because nothing is
/// known. Callers must not read that as "the site is confirmed running": the
/// accompanying `observed_at`/`age_seconds` are `None` precisely so a stale or
/// absent feed is distinguishable from a healthy one.
async fn read_observed_estop(
    site_db: &SiteDbConn,
    forced: &DemoForcedAlarms,
) -> Result<ObservedEstop, diesel::result::Error> {
    let forced_estop = forced.snapshot().contains(&ESTOP_ALARM_NUM);

    let mut observed = site_db
        .run(|conn| -> Result<ObservedEstop, diesel::result::Error> {
            use diesel::prelude::*;
            use neems_data::schema::readings::dsl::*;

            let recent: Vec<neems_data::models::Reading> =
                readings.order(timestamp.desc()).limit(10).load(conn)?;

            for reading in &recent {
                if let Some(registers) = parse_alarm_registers(&reading.data) {
                    let flags = AlarmFlags::from_registers(&registers);
                    let now = Utc::now().naive_utc();
                    return Ok(ObservedEstop {
                        active: flags.is_estop_active(),
                        observed_at: Some(reading.timestamp),
                        age_seconds: Some((now - reading.timestamp).num_seconds()),
                    });
                }
            }

            Ok(ObservedEstop {
                active: false,
                observed_at: None,
                age_seconds: None,
            })
        })
        .await?;

    if forced_estop {
        observed.active = true;
        // Surface a fresh timestamp so a forced trip does not also read as
        // stale data in the UI.
        if observed.observed_at.is_none() {
            observed.observed_at = Some(Utc::now().naive_utc());
            observed.age_seconds = Some(0);
        }
    }

    Ok(observed)
}

/// Fail a request whose signal never reached the RTAC in time.
///
/// This is the only way a request resolves other than being written, and it is
/// deliberately not a judgement about the plant: a request is not failed
/// because the RTAC declined to trip, only because nobody managed to *tell* it
/// to. Runs on read, so the collector's polling drives it with no operator
/// client needing to be watching.
fn fail_if_undelivered(
    conn: &mut diesel::SqliteConnection,
    request: crate::models::EstopRequest,
) -> Result<crate::models::EstopRequest, diesel::result::Error> {
    if request.status() != EstopRequestStatus::Pending {
        return Ok(request);
    }

    let waited = (Utc::now().naive_utc() - request.requested_at).num_seconds();
    if waited <= DISPATCH_TIMEOUT_SECONDS {
        return Ok(request);
    }

    Ok(resolve_estop_request(
        conn,
        request.id,
        EstopRequestStatus::Failed,
        Some(format!(
            "the E-stop signal did not reach the RTAC within {DISPATCH_TIMEOUT_SECONDS}s"
        )),
    )?
    .unwrap_or(request))
}

/// Request an emergency stop for a site.
///
/// - **URL:** `/api/1/Sites/<site_id>/EmergencyStop`
/// - **Method:** `POST`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// Records the request and returns the site's E-stop status. The response's
/// `observed_active` reflects the RTAC, not the request — a fresh request
/// returns `observed_active: false` until the RTAC reports a trip, which it may
/// never do.
///
/// Requesting while a signal is still waiting to go out returns that request
/// rather than creating a second one.
#[post("/1/Sites/<site_id>/EmergencyStop")]
pub async fn request_site_estop(
    db: DbConn,
    site_db: SiteDbConn,
    forced: &State<DemoForcedAlarms>,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<EstopStatusResponse>, status::Custom<Json<ErrorResponse>>> {
    let observed = read_observed_estop(&site_db, forced)
        .await
        .map_err(|e| internal_error("Error reading observed E-stop state", e))?;

    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        let requested = request_estop(conn, site_id, Some(auth_user.user.id))
            .map_err(|e| internal_error("Error recording E-stop request", e))?;

        // A coalesced-onto request may already have been waiting too long, in
        // which case say so rather than handing back a stale "pending".
        let resolved = fail_if_undelivered(conn, requested)
            .map_err(|e| internal_error("Error resolving E-stop request", e))?;

        Ok(Json(EstopStatusResponse {
            site_id,
            observed_active: observed.active,
            observed_at: observed.observed_at,
            observed_age_seconds: observed.age_seconds,
            request: Some(EstopRequestDto::from(resolved)),
        }))
    })
    .await
}

/// Get a site's E-stop status.
///
/// - **URL:** `/api/1/Sites/<site_id>/EmergencyStop`
/// - **Method:** `GET`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// `observed_active` comes from alarm 104 and is the authority on whether the
/// site is tripped. `request` describes the latest operator request and says
/// only whether their signal reached the RTAC. Read them together: a
/// `dispatched` request with `observed_active: false` means the RTAC was asked
/// and has not tripped — worth an operator's attention, but not a failure of
/// this system to do its job.
#[get("/1/Sites/<site_id>/EmergencyStop")]
pub async fn get_site_estop(
    db: DbConn,
    site_db: SiteDbConn,
    forced: &State<DemoForcedAlarms>,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<EstopStatusResponse>, status::Custom<Json<ErrorResponse>>> {
    let observed = read_observed_estop(&site_db, forced)
        .await
        .map_err(|e| internal_error("Error reading observed E-stop state", e))?;

    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        let latest = get_latest_estop_request(conn, site_id)
            .map_err(|e| internal_error("Error loading E-stop request", e))?;

        let request = match latest {
            Some(row) => Some(EstopRequestDto::from(
                fail_if_undelivered(conn, row)
                    .map_err(|e| internal_error("Error resolving E-stop request", e))?,
            )),
            None => None,
        };

        Ok(Json(EstopStatusResponse {
            site_id,
            observed_active: observed.active,
            observed_at: observed.observed_at,
            observed_age_seconds: observed.age_seconds,
            request,
        }))
    })
    .await
}

/// Report that the E-stop command has been written to the RTAC.
///
/// - **URL:** `/api/1/Sites/<site_id>/EmergencyStop/<request_id>/Dispatch`
/// - **Method:** `POST`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// Called by the neems-data collector once its Modbus write of
/// `CommandType::EmergencyStop` has actually succeeded — not when the command
/// was queued. This resolves the request: the signal is out, which is what was
/// asked for.
///
/// Idempotent: reporting dispatch for an already-resolved request leaves it
/// unchanged, so a duplicate report cannot restate when the trip went out.
#[post("/1/Sites/<site_id>/EmergencyStop/<request_id>/Dispatch")]
pub async fn dispatch_site_estop(
    db: DbConn,
    site_id: i32,
    request_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<EstopRequestDto>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        // Scope the lookup to the site so a request id from elsewhere cannot be
        // advanced through this site's endpoint.
        if get_estop_request(conn, site_id, request_id)
            .map_err(|e| internal_error("Error loading E-stop request", e))?
            .is_none()
        {
            return Err(status::Custom(
                Status::NotFound,
                Json(ErrorResponse {
                    error: "E-stop request not found".to_string(),
                }),
            ));
        }

        let updated = mark_estop_dispatched(conn, request_id)
            .map_err(|e| internal_error("Error marking E-stop request dispatched", e))?
            .ok_or_else(|| {
                status::Custom(
                    Status::NotFound,
                    Json(ErrorResponse {
                        error: "E-stop request not found".to_string(),
                    }),
                )
            })?;

        Ok(Json(EstopRequestDto::from(updated)))
    })
    .await
}

/// Get the request the collector should act on, if any.
///
/// - **URL:** `/api/1/Sites/<site_id>/EmergencyStop/Pending`
/// - **Method:** `GET`
/// - **Authentication:** Required; the user must be able to access the site.
///
/// Returns the site's `pending` request — one whose signal has not yet reached
/// the RTAC — or `null`.
///
/// The timeout is applied here too, so a request nothing ever picked up fails
/// on the collector's own polling rather than depending on an operator's UI
/// being open. Nothing in that decision needs the RTAC feed, so this stays a
/// single read of the API database, off the site database entirely.
#[get("/1/Sites/<site_id>/EmergencyStop/Pending")]
pub async fn get_pending_site_estop(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Option<EstopRequestDto>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        if !can_access_site(&auth_user, site_id, conn) {
            return Err(forbidden());
        }

        let pending = get_unresolved_estop_request(conn, site_id)
            .map_err(|e| internal_error("Error loading pending E-stop request", e))?;

        let resolved = match pending {
            Some(row) => Some(
                fail_if_undelivered(conn, row)
                    .map_err(|e| internal_error("Error resolving E-stop request", e))?,
            ),
            None => None,
        };

        // A request that just timed out is no longer outstanding work.
        Ok(Json(resolved.filter(|r| r.status().is_unresolved()).map(EstopRequestDto::from)))
    })
    .await
}

pub fn routes() -> Vec<Route> {
    routes![request_site_estop, get_site_estop, get_pending_site_estop, dispatch_site_estop]
}

#[cfg(test)]
mod tests {
    use diesel::prelude::*;

    use super::*;
    use crate::orm::{company::insert_company, site::insert_site, testing::setup_test_db};

    fn site_fixture(conn: &mut diesel::SqliteConnection) -> i32 {
        let company = insert_company(conn, "Timeout Co".to_string(), None).unwrap();
        insert_site(
            conn,
            "Timeout Site".to_string(),
            "1 Main St".to_string(),
            0.0,
            0.0,
            company.id,
            60,
            None,
        )
        .unwrap()
        .id
    }

    fn reload(conn: &mut diesel::SqliteConnection, request_id: i32) -> crate::models::EstopRequest {
        use crate::schema::estop_requests::dsl::*;
        estop_requests
            .find(request_id)
            .select(crate::models::EstopRequest::as_select())
            .first(conn)
            .unwrap()
    }

    /// Backdate a request so the timeout can be exercised without waiting.
    fn age_request(conn: &mut diesel::SqliteConnection, request_id: i32, seconds: i64) {
        use crate::schema::estop_requests::dsl::*;

        let then = Utc::now().naive_utc() - chrono::Duration::seconds(seconds);
        diesel::update(estop_requests.find(request_id))
            .set(requested_at.eq(then))
            .execute(conn)
            .unwrap();
    }

    /// A request nobody could deliver has to say so. This is the collector not
    /// running, having no credentials, or being unable to reach the RTAC — the
    /// operator is owed that news rather than an indefinite spinner.
    #[test]
    fn a_request_that_never_reached_the_rtac_fails() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn);
        let request = request_estop(&mut conn, site, None).unwrap();
        age_request(&mut conn, request.id, DISPATCH_TIMEOUT_SECONDS + 1);

        let row = reload(&mut conn, request.id);
        let resolved = fail_if_undelivered(&mut conn, row).unwrap();

        assert_eq!(resolved.status(), EstopRequestStatus::Failed);
        assert!(resolved.failure_reason.is_some());
    }

    #[test]
    fn a_request_still_within_the_timeout_is_left_alone() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn);
        let request = request_estop(&mut conn, site, None).unwrap();
        age_request(&mut conn, request.id, DISPATCH_TIMEOUT_SECONDS - 1);

        let row = reload(&mut conn, request.id);
        let resolved = fail_if_undelivered(&mut conn, row).unwrap();

        assert_eq!(resolved.status(), EstopRequestStatus::Pending);
    }

    /// A signal that went out stays sent. The RTAC declining to trip is not the
    /// request's failure, and no timeout may rewrite it into one.
    #[test]
    fn a_dispatched_request_is_never_failed_by_the_timeout() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn);
        let request = request_estop(&mut conn, site, None).unwrap();
        mark_estop_dispatched(&mut conn, request.id).unwrap().expect("dispatched");
        age_request(&mut conn, request.id, DISPATCH_TIMEOUT_SECONDS * 100);

        let row = reload(&mut conn, request.id);
        let resolved = fail_if_undelivered(&mut conn, row).unwrap();

        assert_eq!(resolved.status(), EstopRequestStatus::Dispatched);
        assert!(resolved.failure_reason.is_none());
    }
}
