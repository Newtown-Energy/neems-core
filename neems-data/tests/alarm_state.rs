//! Edge semantics of the `alarm_state` table.
//!
//! `last_rising_at` is the timestamp an acknowledgement is compared against to
//! decide whether an alarm has been acknowledged (see `effective_state` in
//! neems-api). That only works if it marks the start of the current
//! *continuous* activation, so these tests pin down what does and does not
//! count as an edge.

use chrono::{Duration, NaiveDate, NaiveDateTime};
use diesel::{prelude::*, sqlite::SqliteConnection};
use diesel_migrations::MigrationHarness;
use neems_data::{MIGRATIONS, get_all_alarm_state, upsert_alarm_transition};

const ALARM: i32 = 321;

fn setup_test_db() -> SqliteConnection {
    let mut connection =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory db");
    connection.run_pending_migrations(MIGRATIONS).expect("Failed to run migrations");
    connection
}

/// Test timestamp `base + secs` seconds.
fn t(secs: i64) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 8, 28).unwrap().and_hms_opt(0, 0, 0).unwrap()
        + Duration::seconds(secs)
}

/// The stored edges for [`ALARM`], as `(data_active, rising, falling)`.
fn edges(conn: &mut SqliteConnection) -> (bool, Option<NaiveDateTime>, Option<NaiveDateTime>) {
    let rows = get_all_alarm_state(conn).expect("read alarm state");
    let row = rows.iter().find(|r| r.alarm_num == ALARM).expect("alarm has a state row");
    (row.data_active, row.last_rising_at, row.last_falling_at)
}

#[test]
fn a_genuine_edge_stamps_its_own_column_and_leaves_the_other() {
    let mut conn = setup_test_db();

    upsert_alarm_transition(&mut conn, ALARM, true, t(10)).unwrap();
    assert_eq!(edges(&mut conn), (true, Some(t(10)), None));

    // Falling edge records itself without disturbing the rising edge, so the
    // last activation remains reconstructable.
    upsert_alarm_transition(&mut conn, ALARM, false, t(30)).unwrap();
    assert_eq!(edges(&mut conn), (false, Some(t(10)), Some(t(30))));
}

/// A repeated "still active" report is not a new activation.
///
/// The RTAC feed is polled, not edge-triggered at the source, so the same state
/// can be asserted many times over. Re-stamping `last_rising_at` on each would
/// make a five-hour activation look like a fresh one every poll, and every
/// acknowledgement would go stale the moment after it was recorded.
#[test]
fn a_repeated_active_report_does_not_restamp_the_rising_edge() {
    let mut conn = setup_test_db();

    upsert_alarm_transition(&mut conn, ALARM, true, t(10)).unwrap();
    upsert_alarm_transition(&mut conn, ALARM, true, t(20)).unwrap();
    upsert_alarm_transition(&mut conn, ALARM, true, t(18_000)).unwrap();

    assert_eq!(edges(&mut conn), (true, Some(t(10)), None));
}

/// The mirror case: a repeated "still clear" report is not a new return to
/// normal.
#[test]
fn a_repeated_clear_report_does_not_restamp_the_falling_edge() {
    let mut conn = setup_test_db();

    upsert_alarm_transition(&mut conn, ALARM, true, t(10)).unwrap();
    upsert_alarm_transition(&mut conn, ALARM, false, t(30)).unwrap();
    upsert_alarm_transition(&mut conn, ALARM, false, t(40)).unwrap();

    assert_eq!(edges(&mut conn), (false, Some(t(10)), Some(t(30))));
}

/// A clear followed by a re-activation is a second instance, and gets a second
/// rising edge — this is what makes the new activation demand its own
/// acknowledgement.
#[test]
fn reactivation_after_a_clear_stamps_a_new_rising_edge() {
    let mut conn = setup_test_db();

    upsert_alarm_transition(&mut conn, ALARM, true, t(10)).unwrap();
    upsert_alarm_transition(&mut conn, ALARM, false, t(30)).unwrap();
    upsert_alarm_transition(&mut conn, ALARM, true, t(40)).unwrap();

    assert_eq!(edges(&mut conn), (true, Some(t(40)), Some(t(30))));
}

/// The collector restarting must not split a continuous activation in two.
///
/// `RtacWorker::last_alarm_flags` starts out all-clear, so on the first poll
/// after a restart every alarm that is still genuinely active is reported as a
/// rising edge. The stored row already says active, so the redundant edge is
/// dropped and an operator's acknowledgement survives the restart.
#[test]
fn a_collector_restart_does_not_split_a_continuous_activation() {
    let mut conn = setup_test_db();

    upsert_alarm_transition(&mut conn, ALARM, true, t(10)).unwrap();
    // ... collector restarts, re-reports every active alarm as a rising edge
    upsert_alarm_transition(&mut conn, ALARM, true, t(600)).unwrap();

    assert_eq!(edges(&mut conn), (true, Some(t(10)), None));
}
