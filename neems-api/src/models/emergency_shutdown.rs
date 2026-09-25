//! Models for operator emergency shutdown requests.
//!
//! The E-stop is a physical button at the site, and its *state* is not modeled
//! here — it is read from the RTAC (the site design's E-stop alarm; 104 for
//! Newtown) and surfaced through
//! [`EmergencyShutdownStatusResponse::observed_active`]. These types model the
//! operator's *request* for an emergency shutdown and its lifecycle, so a
//! request can be audited and so the collector has something durable to act
//! on.
//!
//! The lifecycle tracks what this system owes an operator, which is to get the
//! signal to the RTAC — nothing more. What the RTAC then does with it is the
//! RTAC's business, and nothing here infers it.

use std::{fmt, str::FromStr};

use diesel::{Associations, Identifiable, Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::emergency_shutdown_requests;

/// Lifecycle of an emergency shutdown request.
///
/// `Pending -> Dispatched | Failed`. `Dispatched` is success and is terminal:
/// the signal reached the RTAC, which is the entirety of what this system can
/// promise. What the site then does is its own business, and nothing here
/// infers it — in particular, the site's E-stop state
/// ([`EmergencyShutdownStatusResponse::observed_active`]) is not the request's
/// outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyShutdownRequestStatus {
    /// Recorded from an operator; not yet written to the RTAC.
    Pending,
    /// The collector wrote `CommandType::EmergencyShutdown` to the RTAC and the
    /// write succeeded.
    Dispatched,
    /// Nothing managed to write it to the RTAC within the timeout — the
    /// collector is not running, or could not reach the RTAC at all.
    Failed,
}

impl EmergencyShutdownRequestStatus {
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

impl fmt::Display for EmergencyShutdownRequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for EmergencyShutdownRequestStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "dispatched" => Ok(Self::Dispatched),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown emergency shutdown request status: {other}")),
        }
    }
}

/// Database row for an emergency shutdown request.
#[derive(
    Queryable, Selectable, Identifiable, Associations, Debug, Clone, Serialize, Deserialize,
)]
#[diesel(belongs_to(super::site::Site))]
#[diesel(table_name = emergency_shutdown_requests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct EmergencyShutdownRequest {
    pub id: i32,
    pub site_id: i32,
    /// Serialized [`EmergencyShutdownRequestStatus`]. Stored as text so the set
    /// can grow without a migration.
    pub status: String,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
    pub dispatched_at: Option<chrono::NaiveDateTime>,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub failure_reason: Option<String>,
}

impl EmergencyShutdownRequest {
    /// Parse the stored status. Unrecognized values are treated as `Failed`
    /// rather than panicking — an unreadable request must never read as an
    /// in-flight or confirmed trip.
    pub fn status(&self) -> EmergencyShutdownRequestStatus {
        self.status.parse().unwrap_or(EmergencyShutdownRequestStatus::Failed)
    }
}

/// Insertable row for a new emergency shutdown request.
#[derive(Insertable, Debug)]
#[diesel(table_name = emergency_shutdown_requests)]
pub struct NewEmergencyShutdownRequest {
    pub site_id: i32,
    pub status: String,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
}

/// An emergency shutdown request as served to clients.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmergencyShutdownRequestDto {
    pub id: i32,
    pub site_id: i32,
    pub status: EmergencyShutdownRequestStatus,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
    pub dispatched_at: Option<chrono::NaiveDateTime>,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub failure_reason: Option<String>,
}

impl From<EmergencyShutdownRequest> for EmergencyShutdownRequestDto {
    fn from(row: EmergencyShutdownRequest) -> Self {
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

/// Emergency shutdown status for a site: the E-stop state the RTAC reports,
/// plus any request in flight.
///
/// The two halves answer different questions and neither is evidence about the
/// other. `observed_active` is the only field a UI should use to decide whether
/// the site's E-stop is tripped. `request` says only whether the operator's
/// signal got out; a delivered request need never raise the E-stop alarm.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmergencyShutdownStatusResponse {
    pub site_id: i32,
    /// The design's E-stop alarm as of the most recent RTAC reading. The
    /// authority on whether the site's E-stop is tripped.
    pub observed_active: bool,
    /// Timestamp of the reading `observed_active` was taken from, if any.
    pub observed_at: Option<chrono::NaiveDateTime>,
    /// Age of that reading in seconds. `None` when no reading carried alarm
    /// data — in which case `observed_active` is false because nothing is
    /// known, not because the site is known to be running.
    pub observed_age_seconds: Option<i64>,
    /// The most recent request for this site.
    pub request: Option<EmergencyShutdownRequestDto>,
}
