use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::alarm_state;

/// Persistent per-alarm data state, maintained by the RTAC collector
/// independently of acknowledgement.
///
/// One row per `alarm_num`. The collector upserts it on every rising/falling
/// edge it observes (see `DatabaseAlarmStateHandler`). The API reads it to
/// compute latched alarm visibility without rescanning the `readings` history.
#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = alarm_state)]
#[diesel(primary_key(alarm_num))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AlarmStateRow {
    /// Newtown alarm number (unique key).
    pub alarm_num: i32,
    /// Current raw bit state — true while the alarm condition is present.
    pub data_active: bool,
    /// Most recent false->true transition (UTC); NULL until first seen.
    pub last_rising_at: Option<NaiveDateTime>,
    /// Most recent true->false transition (UTC); NULL until first seen.
    pub last_falling_at: Option<NaiveDateTime>,
    /// When this row was last updated (UTC).
    pub updated_at: NaiveDateTime,
}
