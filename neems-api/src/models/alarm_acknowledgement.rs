use chrono::NaiveDateTime;
use diesel::{Identifiable, Insertable, Queryable, QueryableByName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::alarm_acknowledgements;

/// A single acknowledgement of an alarm by a user. The table is append-only:
/// each acknowledgement is its own row, so the history is a full audit of who
/// acknowledged which alarm and when. The most recent row for an `alarm_num`
/// is what drives latched-status computation.
#[derive(Queryable, Identifiable, QueryableByName, Debug, Clone, Serialize, Deserialize, TS)]
#[diesel(table_name = alarm_acknowledgements)]
#[ts(export)]
pub struct AlarmAcknowledgement {
    pub id: i32,
    pub alarm_num: i32,
    pub user_id: i32,
    #[ts(type = "string")]
    pub acknowledged_at: NaiveDateTime,
    pub note: Option<String>,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = alarm_acknowledgements)]
pub struct NewAlarmAcknowledgement {
    pub alarm_num: i32,
    pub user_id: i32,
    /// Optional; falls back to the database default (`CURRENT_TIMESTAMP`).
    pub acknowledged_at: Option<NaiveDateTime>,
    pub note: Option<String>,
}
