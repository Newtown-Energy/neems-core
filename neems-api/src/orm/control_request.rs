//! Database operations for operator requests against a site input.
//!
//! See [`crate::models::control_request`] for the lifecycle these functions
//! move a request through.

use diesel::{prelude::*, sql_types::BigInt};

use crate::models::{ControlRequest, ControlRequestStatus, NewControlRequest};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Record a request against one control.
///
/// If that control already has a `pending` request, it is returned untouched
/// instead of a new one being created: a second click before the signal has
/// gone out is the same ask. Once it *has* gone out the request is finished, so
/// a later click starts a new one and produces a new signal.
///
/// Coalescing ignores the action, and deliberately so — an operator who clicks
/// open and then close before either has gone out has told us two contradictory
/// things, and sending the first is the only answer that cannot surprise them.
/// The alternative, letting the second overwrite the first, means a breaker
/// moves in the direction of whichever click the network happened to deliver
/// last.
///
/// The look-then-insert runs in an immediate transaction so it takes SQLite's
/// write lock up front: with a deferred one, two concurrent clicks could both
/// read no pending row before either inserted. The partial unique index from
/// the migration backs that up at the database level.
pub fn request_control(
    conn: &mut SqliteConnection,
    site_id_val: i32,
    control_id_val: &str,
    action_val: &str,
    requested_by_val: Option<i32>,
) -> Result<ControlRequest, diesel::result::Error> {
    use crate::schema::control_requests::dsl::*;

    conn.immediate_transaction(|conn| {
        if let Some(existing) = get_pending_control_request(conn, site_id_val, control_id_val)? {
            return Ok(existing);
        }

        let new_request = NewControlRequest {
            site_id: site_id_val,
            control_id: control_id_val.to_string(),
            action: action_val.to_string(),
            status: ControlRequestStatus::Pending.to_string(),
            requested_by: requested_by_val,
            requested_at: chrono::Utc::now().naive_utc(),
        };

        match diesel::insert_into(control_requests).values(&new_request).execute(conn) {
            Ok(_) => {}
            Err(
                e @ diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            ) => {
                // The unique index refused a second pending request. Coalesce
                // onto whatever is already in flight, which is what the read
                // above would have done had it seen the row.
                return get_pending_control_request(conn, site_id_val, control_id_val)?.ok_or(e);
            }
            Err(e) => return Err(e),
        }

        let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
            .get_result::<LastInsertRowId>(conn)?
            .last_insert_rowid as i32;

        control_requests.find(last_id).select(ControlRequest::as_select()).first(conn)
    })
}

/// Get the control's request that has not yet reached the RTAC, if any.
pub fn get_pending_control_request(
    conn: &mut SqliteConnection,
    site_id_val: i32,
    control_id_val: &str,
) -> Result<Option<ControlRequest>, diesel::result::Error> {
    use crate::schema::control_requests::dsl::*;

    control_requests
        .filter(site_id.eq(site_id_val))
        .filter(control_id.eq(control_id_val))
        .filter(status.eq(ControlRequestStatus::Pending.as_str()))
        .order(requested_at.desc())
        .select(ControlRequest::as_select())
        .first(conn)
        .optional()
}

/// Every request across the site that has not yet reached the RTAC. This is
/// what the collector polls for — one query per tick rather than one per
/// control.
pub fn get_pending_control_requests(
    conn: &mut SqliteConnection,
    site_id_val: i32,
) -> Result<Vec<ControlRequest>, diesel::result::Error> {
    use crate::schema::control_requests::dsl::*;

    control_requests
        .filter(site_id.eq(site_id_val))
        .filter(status.eq(ControlRequestStatus::Pending.as_str()))
        .order((requested_at.asc(), id.asc()))
        .select(ControlRequest::as_select())
        .load(conn)
}

/// The most recent request for each of a site's controls, regardless of status.
///
/// This is what the diagram renders its per-element overlay from, so it must
/// return the *latest* row per control rather than the latest overall — a
/// failed breaker request must stay visible while another control is asked for
/// something else.
pub fn get_latest_control_requests(
    conn: &mut SqliteConnection,
    site_id_val: i32,
) -> Result<Vec<ControlRequest>, diesel::result::Error> {
    use std::collections::HashMap;

    use crate::schema::control_requests::dsl::*;

    // Ordered oldest first so the later insert overwrites the earlier one in
    // the map. The row count per site is small (one per click) and only the
    // newest per control survives, so this stays a single indexed scan rather
    // than a correlated subquery diesel would need spelled out in raw SQL.
    let rows: Vec<ControlRequest> = control_requests
        .filter(site_id.eq(site_id_val))
        .order((requested_at.asc(), id.asc()))
        .select(ControlRequest::as_select())
        .load(conn)?;

    let mut latest: HashMap<String, ControlRequest> = HashMap::new();
    for row in rows {
        latest.insert(row.control_id.clone(), row);
    }

    let mut out: Vec<ControlRequest> = latest.into_values().collect();
    out.sort_by(|a, b| a.control_id.cmp(&b.control_id));
    Ok(out)
}

/// Get a single request by id, scoped to a site so one site's id cannot be used
/// to move another's request along.
pub fn get_control_request(
    conn: &mut SqliteConnection,
    site_id_val: i32,
    request_id: i32,
) -> Result<Option<ControlRequest>, diesel::result::Error> {
    use crate::schema::control_requests::dsl::*;

    control_requests
        .find(request_id)
        .filter(site_id.eq(site_id_val))
        .select(ControlRequest::as_select())
        .first(conn)
        .optional()
}

/// Mark a request as having been written to the RTAC.
///
/// This resolves the request: getting the signal out is all that was asked of
/// this system, so there is nothing further to wait for. `resolved_at` is set
/// alongside `sent_at` to say so.
///
/// Only a `pending` request moves; re-reporting is a no-op that returns the row
/// unchanged, so a collector retry cannot rewrite the timestamps of a signal
/// that already went out.
pub fn mark_control_request_sent(
    conn: &mut SqliteConnection,
    request_id: i32,
) -> Result<Option<ControlRequest>, diesel::result::Error> {
    use crate::schema::control_requests::dsl::*;

    conn.transaction(|conn| {
        let now = chrono::Utc::now().naive_utc();
        diesel::update(
            control_requests
                .find(request_id)
                .filter(status.eq(ControlRequestStatus::Pending.as_str())),
        )
        .set((
            status.eq(ControlRequestStatus::Sent.as_str()),
            sent_at.eq(Some(now)),
            resolved_at.eq(Some(now)),
        ))
        .execute(conn)?;

        control_requests
            .find(request_id)
            .select(ControlRequest::as_select())
            .first(conn)
            .optional()
    })
}

/// Resolve a request as failed, with the reason an operator will read.
///
/// Only a `pending` request moves, so a late failure report cannot overwrite a
/// signal that did get out.
pub fn fail_control_request(
    conn: &mut SqliteConnection,
    request_id: i32,
    reason: String,
) -> Result<Option<ControlRequest>, diesel::result::Error> {
    use crate::schema::control_requests::dsl::*;

    conn.transaction(|conn| {
        diesel::update(
            control_requests
                .find(request_id)
                .filter(status.eq(ControlRequestStatus::Pending.as_str())),
        )
        .set((
            status.eq(ControlRequestStatus::Failed.as_str()),
            resolved_at.eq(Some(chrono::Utc::now().naive_utc())),
            failure_reason.eq(Some(reason)),
        ))
        .execute(conn)?;

        control_requests
            .find(request_id)
            .select(ControlRequest::as_select())
            .first(conn)
            .optional()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::{company::insert_company, site::insert_site, testing::setup_test_db};

    fn site_fixture(conn: &mut SqliteConnection, name: &str) -> i32 {
        let company = insert_company(conn, format!("{name} Co"), None).unwrap();
        insert_site(conn, name.to_string(), "1 Main St".to_string(), 0.0, 0.0, company.id, 60, None)
            .unwrap()
            .id
    }

    fn raw_insert(
        conn: &mut SqliteConnection,
        site: i32,
        control: &str,
        request_status: ControlRequestStatus,
    ) -> Result<usize, diesel::result::Error> {
        use crate::schema::control_requests::dsl::*;

        diesel::insert_into(control_requests)
            .values(&NewControlRequest {
                site_id: site,
                control_id: control.to_string(),
                action: "open".to_string(),
                status: request_status.to_string(),
                requested_by: None,
                requested_at: chrono::Utc::now().naive_utc(),
            })
            .execute(conn)
    }

    fn is_unique_violation(result: &Result<usize, diesel::result::Error>) -> bool {
        matches!(
            result,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _
            ))
        )
    }

    /// "At most one pending request per control" is a database rule, not just
    /// something `request_control` is careful about. Two concurrent clicks that
    /// both read no pending row must not both land — for a breaker, that is two
    /// movements for one ask.
    #[test]
    fn the_database_refuses_a_second_pending_request_for_one_control() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Refuse");

        request_control(&mut conn, site, "feeder-1a", "open", None).unwrap();

        assert!(
            is_unique_violation(&raw_insert(
                &mut conn,
                site,
                "feeder-1a",
                ControlRequestStatus::Pending
            )),
            "a second pending request for the control must be refused"
        );
    }

    /// The constraint is per control, not per site: asking for one breaker must
    /// not block asking for another.
    #[test]
    fn another_control_may_have_its_own_pending_request() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Parallel");

        let a = request_control(&mut conn, site, "feeder-1a", "open", None).unwrap();
        let b = request_control(&mut conn, site, "feeder-1b", "open", None).unwrap();

        assert_ne!(a.id, b.id);
        assert_eq!(get_pending_control_requests(&mut conn, site).unwrap().len(), 2);
    }

    /// A double-click before the signal goes out is one ask, not two — and the
    /// action of the first click is the one that survives.
    #[test]
    fn a_pending_request_is_coalesced_onto_without_changing_its_action() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Coalesce");

        let first = request_control(&mut conn, site, "switch-89l-1", "open", None).unwrap();
        let second = request_control(&mut conn, site, "switch-89l-1", "close", None).unwrap();

        assert_eq!(second.id, first.id);
        assert_eq!(
            second.action, "open",
            "a contradicting second click must not redirect the first"
        );
    }

    /// Once the signal has gone out the request is done, so a fresh click is a
    /// fresh request — and a fresh signal.
    #[test]
    fn sending_frees_the_control_for_a_new_request() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Free");

        let first = request_control(&mut conn, site, "feeder-2c", "open", None).unwrap();
        let sent = mark_control_request_sent(&mut conn, first.id).unwrap().expect("sent");
        assert!(sent.resolved_at.is_some(), "sending resolves the request");

        let second = request_control(&mut conn, site, "feeder-2c", "close", None).unwrap();
        assert_ne!(second.id, first.id, "a sent request must not swallow the next ask");
        assert_eq!(second.status(), ControlRequestStatus::Pending);
    }

    /// A failure report that arrives after the signal got out must not rewrite
    /// history: the operator was told it was sent, and it was.
    #[test]
    fn a_late_failure_cannot_overwrite_a_sent_request() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Late");

        let request = request_control(&mut conn, site, "feeder-1c", "close", None).unwrap();
        mark_control_request_sent(&mut conn, request.id).unwrap();

        let after = fail_control_request(&mut conn, request.id, "too late".to_string())
            .unwrap()
            .expect("row still there");

        assert_eq!(after.status(), ControlRequestStatus::Sent);
        assert!(after.failure_reason.is_none());
    }

    /// The migration says the audit row survives the requesting user being
    /// deleted. Without `ON DELETE SET NULL` that is not a nullable column, it
    /// is a delete that gets refused — the record becomes an obstacle to
    /// removing a user rather than a record of what they did.
    #[test]
    fn deleting_the_requester_leaves_the_record_behind() {
        use crate::{
            models::UserInput,
            orm::{login::hash_password, user::insert_user},
        };

        let mut conn = setup_test_db();
        let company = insert_company(&mut conn, "Audit Co".to_string(), None).unwrap();
        let site = insert_site(
            &mut conn,
            "Audit".to_string(),
            "1 Main St".to_string(),
            0.0,
            0.0,
            company.id,
            60,
            None,
        )
        .unwrap()
        .id;
        let user = insert_user(
            &mut conn,
            UserInput {
                email: "operator@example.com".to_string(),
                password_hash: hash_password("hunter2"),
                company_id: company.id,
                totp_secret: None,
            },
            None,
        )
        .unwrap();

        let requested =
            request_control(&mut conn, site, "feeder-1a", "open", Some(user.id)).unwrap();

        diesel::delete(crate::schema::users::dsl::users.find(user.id))
            .execute(&mut conn)
            .expect("deleting the requester must not be refused by the request row");

        let after = get_control_request(&mut conn, site, requested.id).unwrap().expect("row kept");
        assert_eq!(after.control_id, "feeder-1a", "the ask itself survives");
        assert!(after.requested_by.is_none(), "the requester is forgotten, not the request");
    }

    /// The diagram renders one overlay per element, so the read must give the
    /// newest row for *each* control rather than the newest overall.
    #[test]
    fn the_latest_request_is_per_control() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Latest");

        let old = request_control(&mut conn, site, "feeder-1a", "open", None).unwrap();
        fail_control_request(&mut conn, old.id, "no write register".to_string()).unwrap();
        let newer = request_control(&mut conn, site, "feeder-1b", "close", None).unwrap();

        let latest = get_latest_control_requests(&mut conn, site).unwrap();
        assert_eq!(latest.len(), 2, "both controls keep their own latest request");
        assert_eq!(latest[0].id, old.id, "the failed request stays visible");
        assert_eq!(latest[1].id, newer.id);
    }
}
