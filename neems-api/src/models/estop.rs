//! Models for operator-requested emergency stops.
//!
//! E-stop *state* is not modeled here — it is read from the RTAC (alarm 104)
//! and surfaced through [`EstopStatusResponse::observed_active`]. These types
//! model the *request* to trip and its lifecycle, so a trip can be audited and
//! so the collector has something durable to act on.
//!
//! The lifecycle tracks what this system owes an operator, which is to get the
//! signal to the RTAC — nothing more. What the RTAC then does with it is the
//! RTAC's business, reported separately and continuously as alarm 104.

use std::{fmt, str::FromStr};

use diesel::{Associations, Identifiable, Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::estop_requests;

/// Lifecycle of an E-stop request.
///
/// `Pending -> Dispatched | Failed`. `Dispatched` is success and is terminal:
/// the signal reached the RTAC, which is the entirety of what this system can
/// promise. Whether the plant actually tripped is a separate question, answered
/// by [`EstopStatusResponse::observed_active`] for as long as anyone cares to
/// look — it is deliberately not folded into the request's own outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum EstopRequestStatus {
    /// Recorded from an operator; not yet written to the RTAC.
    Pending,
    /// The collector wrote `CommandType::EmergencyStop` to the RTAC and the
    /// write succeeded.
    Dispatched,
    /// Nothing managed to write it to the RTAC within the timeout — the
    /// collector is not running, or could not reach the RTAC at all.
    Failed,
}

impl EstopRequestStatus {
    /// Whether the request still has work outstanding.
    ///
    /// Only `Pending` does: it is what the collector polls for and what a
    /// repeated request coalesces onto.
    pub fn is_unresolved(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatched => "dispatched",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for EstopRequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for EstopRequestStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "dispatched" => Ok(Self::Dispatched),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown estop request status: {other}")),
        }
    }
}

/// Database row for an E-stop request.
#[derive(
    Queryable, Selectable, Identifiable, Associations, Debug, Clone, Serialize, Deserialize,
)]
#[diesel(belongs_to(super::site::Site))]
#[diesel(table_name = estop_requests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct EstopRequest {
    pub id: i32,
    pub site_id: i32,
    /// Serialized [`EstopRequestStatus`]. Stored as text so the set can grow
    /// without a migration.
    pub status: String,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
    pub dispatched_at: Option<chrono::NaiveDateTime>,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub failure_reason: Option<String>,
}

impl EstopRequest {
    /// Parse the stored status. Unrecognized values are treated as `Failed`
    /// rather than panicking — an unreadable request must never read as an
    /// in-flight or confirmed trip.
    pub fn status(&self) -> EstopRequestStatus {
        self.status.parse().unwrap_or(EstopRequestStatus::Failed)
    }
}

/// Insertable row for a new E-stop request.
#[derive(Insertable, Debug)]
#[diesel(table_name = estop_requests)]
pub struct NewEstopRequest {
    pub site_id: i32,
    pub status: String,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
}

/// An E-stop request as served to clients.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EstopRequestDto {
    pub id: i32,
    pub site_id: i32,
    pub status: EstopRequestStatus,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
    pub dispatched_at: Option<chrono::NaiveDateTime>,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub failure_reason: Option<String>,
}

impl From<EstopRequest> for EstopRequestDto {
    fn from(row: EstopRequest) -> Self {
        Self {
            status: row.status(),
            id: row.id,
            site_id: row.site_id,
            requested_by: row.requested_by,
            requested_at: row.requested_at,
            dispatched_at: row.dispatched_at,
            resolved_at: row.resolved_at,
            failure_reason: row.failure_reason,
        }
    }
}

/// E-stop status for a site: what the RTAC reports, plus any request in flight.
///
/// The two halves answer different questions and are meant to be read together.
/// `observed_active` is the only field a UI should use to decide whether the
/// site is tripped. `request` says only whether the operator's signal got out —
/// a `Dispatched` request alongside `observed_active: false` means the RTAC was
/// asked and has not (yet) tripped, which is information about the RTAC, not a
/// failure of the request.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EstopStatusResponse {
    pub site_id: i32,
    /// Alarm 104 as of the most recent RTAC reading. The authority on whether
    /// the site is tripped.
    pub observed_active: bool,
    /// Timestamp of the reading `observed_active` was taken from, if any.
    pub observed_at: Option<chrono::NaiveDateTime>,
    /// Age of that reading in seconds. `None` when no reading carried alarm
    /// data — in which case `observed_active` is false because nothing is
    /// known, not because the site is known to be running.
    pub observed_age_seconds: Option<i64>,
    /// The most recent request for this site, resolved as far as the RTAC feed
    /// allows.
    pub request: Option<EstopRequestDto>,
}
