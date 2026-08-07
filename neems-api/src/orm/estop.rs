//! Database operations for operator-requested emergency stops.
//!
//! See [`crate::models::estop`] for the lifecycle these functions move a
//! request through.

use diesel::{prelude::*, sql_types::BigInt};

use crate::models::{EstopRequest, EstopRequestStatus, NewEstopRequest};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Record an E-stop request for a site.
///
/// If a request is already `pending`, that request is returned untouched
/// instead of a new one being created: a second click before the signal has
/// gone out is the same ask. Once it *has* gone out the request is finished, so
/// a later ask starts a new one and produces a new signal.
///
/// The look-then-insert runs in an immediate transaction so it takes SQLite's
/// write lock up front: with a deferred one, two concurrent requests could both
/// read no pending row before either inserted. The partial unique index from
/// the migration backs that up at the database level.
pub fn request_estop(
    conn: &mut SqliteConnection,
    site_id_val: i32,
    requested_by_val: Option<i32>,
) -> Result<EstopRequest, diesel::result::Error> {
    use crate::schema::estop_requests::dsl::*;

    conn.immediate_transaction(|conn| {
        if let Some(existing) = get_unresolved_estop_request(conn, site_id_val)? {
            return Ok(existing);
        }

        let new_request = NewEstopRequest {
            site_id: site_id_val,
            status: EstopRequestStatus::Pending.to_string(),
            requested_by: requested_by_val,
            requested_at: chrono::Utc::now().naive_utc(),
        };

        match diesel::insert_into(estop_requests).values(&new_request).execute(conn) {
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
                return get_unresolved_estop_request(conn, site_id_val)?.ok_or(e);
            }
            Err(e) => return Err(e),
        }

        let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
            .get_result::<LastInsertRowId>(conn)?
            .last_insert_rowid as i32;

        estop_requests.find(last_id).select(EstopRequest::as_select()).first(conn)
    })
}

/// Get the site's request that has not yet reached the RTAC, if any. This is
/// what the collector polls for.
pub fn get_unresolved_estop_request(
    conn: &mut SqliteConnection,
    site_id_val: i32,
) -> Result<Option<EstopRequest>, diesel::result::Error> {
    use crate::schema::estop_requests::dsl::*;

    estop_requests
        .filter(site_id.eq(site_id_val))
        .filter(status.eq(EstopRequestStatus::Pending.as_str()))
        .order(requested_at.desc())
        .select(EstopRequest::as_select())
        .first(conn)
        .optional()
}

/// Get the site's most recent request regardless of status.
pub fn get_latest_estop_request(
    conn: &mut SqliteConnection,
    site_id_val: i32,
) -> Result<Option<EstopRequest>, diesel::result::Error> {
    use crate::schema::estop_requests::dsl::*;

    estop_requests
        .filter(site_id.eq(site_id_val))
        .order((requested_at.desc(), id.desc()))
        .select(EstopRequest::as_select())
        .first(conn)
        .optional()
}

/// Get a single request by id, scoped to a site so one site's id cannot be used
/// to move another's request along.
pub fn get_estop_request(
    conn: &mut SqliteConnection,
    site_id_val: i32,
    request_id: i32,
) -> Result<Option<EstopRequest>, diesel::result::Error> {
    use crate::schema::estop_requests::dsl::*;

    estop_requests
        .find(request_id)
        .filter(site_id.eq(site_id_val))
        .select(EstopRequest::as_select())
        .first(conn)
        .optional()
}

/// Mark a request as having been written to the RTAC.
///
/// This resolves the request: getting the signal out is all that was asked of
/// this system, so there is nothing further to wait for. `resolved_at` is set
/// alongside `dispatched_at` to say so.
///
/// Only a `pending` request moves; re-reporting a dispatch is a no-op that
/// returns the row unchanged, so a collector retry cannot rewrite the
/// timestamps of a trip that already went out.
pub fn mark_estop_dispatched(
    conn: &mut SqliteConnection,
    request_id: i32,
) -> Result<Option<EstopRequest>, diesel::result::Error> {
    use crate::schema::estop_requests::dsl::*;

    conn.transaction(|conn| {
        let now = chrono::Utc::now().naive_utc();
        diesel::update(
            estop_requests
                .find(request_id)
                .filter(status.eq(EstopRequestStatus::Pending.as_str())),
        )
        .set((
            status.eq(EstopRequestStatus::Dispatched.as_str()),
            dispatched_at.eq(Some(now)),
            resolved_at.eq(Some(now)),
        ))
        .execute(conn)?;

        estop_requests
            .find(request_id)
            .select(EstopRequest::as_select())
            .first(conn)
            .optional()
    })
}

/// Resolve a request to a terminal status.
///
/// In practice this is how a request reaches [`EstopRequestStatus::Failed`];
/// success goes through [`mark_estop_dispatched`], which has its own timestamps
/// to set.
pub fn resolve_estop_request(
    conn: &mut SqliteConnection,
    request_id: i32,
    outcome: EstopRequestStatus,
    reason: Option<String>,
) -> Result<Option<EstopRequest>, diesel::result::Error> {
    use crate::schema::estop_requests::dsl::*;

    conn.transaction(|conn| {
        diesel::update(estop_requests.find(request_id))
            .set((
                status.eq(outcome.as_str()),
                resolved_at.eq(Some(chrono::Utc::now().naive_utc())),
                failure_reason.eq(reason),
            ))
            .execute(conn)?;

        estop_requests
            .find(request_id)
            .select(EstopRequest::as_select())
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
        request_status: EstopRequestStatus,
    ) -> Result<usize, diesel::result::Error> {
        use crate::schema::estop_requests::dsl::*;

        diesel::insert_into(estop_requests)
            .values(&NewEstopRequest {
                site_id: site,
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

    /// "At most one pending request per site" is a database rule, not just
    /// something `request_estop` is careful about. Two concurrent requests that
    /// both read no pending row must not both land.
    #[test]
    fn the_database_refuses_a_second_pending_request() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Refuse");

        request_estop(&mut conn, site, None).unwrap();

        assert!(
            is_unique_violation(&raw_insert(&mut conn, site, EstopRequestStatus::Pending)),
            "a second pending request for the site must be refused"
        );
    }

    /// Once the signal has gone out the request is done, so a fresh ask is a
    /// fresh request — and a fresh signal. Re-sending an E-stop the RTAC has
    /// already been given is harmless; refusing to send one an operator asked
    /// for is not.
    #[test]
    fn dispatching_frees_the_site_for_a_new_request() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Free");

        let first = request_estop(&mut conn, site, None).unwrap();
        let dispatched = mark_estop_dispatched(&mut conn, first.id).unwrap().expect("dispatched");
        assert!(dispatched.resolved_at.is_some(), "dispatch resolves the request");
        assert!(get_unresolved_estop_request(&mut conn, site).unwrap().is_none());

        let second = request_estop(&mut conn, site, None).unwrap();
        assert_ne!(second.id, first.id, "a sent request must not swallow the next ask");
        assert_eq!(second.status(), EstopRequestStatus::Pending);
    }

    /// A double-click before the signal goes out is one ask, not two.
    #[test]
    fn a_pending_request_is_coalesced_onto() {
        let mut conn = setup_test_db();
        let site = site_fixture(&mut conn, "Coalesce");

        let first = request_estop(&mut conn, site, None).unwrap();
        let second = request_estop(&mut conn, site, None).unwrap();

        assert_eq!(second.id, first.id);
        assert_eq!(second.status(), EstopRequestStatus::Pending);
    }

    /// The constraint is per site: one site being tripped must not stop another
    /// from being tripped.
    #[test]
    fn a_second_site_may_have_its_own_pending_request() {
        let mut conn = setup_test_db();
        let site_a = site_fixture(&mut conn, "Alpha");
        let site_b = site_fixture(&mut conn, "Bravo");

        let a = request_estop(&mut conn, site_a, None).unwrap();
        let b = request_estop(&mut conn, site_b, None).unwrap();

        assert_ne!(a.id, b.id);
        assert_eq!(get_unresolved_estop_request(&mut conn, site_b).unwrap().unwrap().id, b.id);
    }
}
