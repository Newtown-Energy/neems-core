//! Persistence for alarm acknowledgements (main app database).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::{
    models::{AlarmAcknowledgement, NewAlarmAcknowledgement},
    schema::alarm_acknowledgements,
};

/// Record an acknowledgement of `alarm_num` by `user_id`. Append-only: always
/// inserts a new row. Returns the persisted row (including its server-assigned
/// `acknowledged_at`).
pub fn create_acknowledgement(
    conn: &mut SqliteConnection,
    alarm_num: i32,
    user_id: i32,
    note: Option<String>,
) -> QueryResult<AlarmAcknowledgement> {
    let new = NewAlarmAcknowledgement { alarm_num, user_id, acknowledged_at: None, note };
    diesel::insert_into(alarm_acknowledgements::table).values(&new).execute(conn)?;
    alarm_acknowledgements::table
        .order(alarm_acknowledgements::id.desc())
        .first::<AlarmAcknowledgement>(conn)
}

/// The most recent acknowledgement per `alarm_num`, keyed by alarm number.
///
/// Loads acknowledgements oldest-first and folds, so the last write wins per
/// alarm. The set of distinct alarms ever acknowledged is small, so this is
/// cheap relative to the alarm-state scan it complements.
pub fn latest_ack_by_alarm(
    conn: &mut SqliteConnection,
) -> QueryResult<HashMap<i32, AlarmAcknowledgement>> {
    let all: Vec<AlarmAcknowledgement> = alarm_acknowledgements::table
        .order(alarm_acknowledgements::acknowledged_at.asc())
        .load(conn)?;

    let mut latest: HashMap<i32, AlarmAcknowledgement> = HashMap::new();
    for ack in all {
        latest.insert(ack.alarm_num, ack);
    }
    Ok(latest)
}

/// All acknowledgements with `acknowledged_at` in `[from, to]`, oldest-first.
/// Used to interleave ack events into the alarm history endpoint.
pub fn acks_in_range(
    conn: &mut SqliteConnection,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> QueryResult<Vec<AlarmAcknowledgement>> {
    alarm_acknowledgements::table
        .filter(alarm_acknowledgements::acknowledged_at.ge(from.naive_utc()))
        .filter(alarm_acknowledgements::acknowledged_at.le(to.naive_utc()))
        .order(alarm_acknowledgements::acknowledged_at.asc())
        .load::<AlarmAcknowledgement>(conn)
}
