//! Models for operator requests against a site input.
//!
//! A click on the diagram — open a breaker, close a switch — is a *request to
//! send a signal*, not a state change. These types model that request and its
//! lifecycle, so the ask can be audited and so the collector has something
//! durable to act on.
//!
//! The resulting position is not modeled here. It is read back from the site
//! through the point named in
//! [`SiteControl::readback_alarm_num`](neems_data::rtac::site_controls::SiteControl),
//! independently and continuously, and is the only authority on where the
//! equipment actually is. The lifecycle below tracks what this system owes an
//! operator, which is to get the signal to the RTAC — nothing more.

use std::{fmt, str::FromStr};

use diesel::{Associations, Identifiable, Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::control_requests;

/// Lifecycle of a control request.
///
/// `Pending -> Sent | Failed`. `Sent` is success and is terminal: the signal
/// reached the RTAC, which is the entirety of what this system can promise.
/// Whether the breaker then moved is a separate question, answered by its
/// readback point for as long as anyone cares to look — deliberately not
/// folded into the request's own outcome.
///
/// The client also described an `acknowledged` state, the RTAC signaling
/// receipt. It is out of scope for now: a successful Modbus write already
/// returns a response, so `Sent` rests on the RTAC answering rather than on us
/// assuming, and distinguishing "the register was written" from "the RTAC's
/// logic acted on it" needs a handshake point that does not exist. Kept an
/// enum on both sides of the wire so adding that variant later is an addition
/// rather than a rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ControlRequestStatus {
    /// Recorded from an operator; not yet written to the RTAC.
    Pending,
    /// Written to the RTAC and the write succeeded.
    Sent,
    /// Nothing wrote it to the RTAC — no write register is defined for the
    /// control, the collector is not running, or the RTAC was unreachable.
    Failed,
}

impl ControlRequestStatus {
    /// Whether the request still has work outstanding.
    ///
    /// Only `Pending` does: it is what the collector polls for and what a
    /// repeated click coalesces onto.
    pub fn is_unresolved(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for ControlRequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ControlRequestStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "sent" => Ok(Self::Sent),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown control request status: {other}")),
        }
    }
}

/// Database row for a control request.
#[derive(
    Queryable, Selectable, Identifiable, Associations, Debug, Clone, Serialize, Deserialize,
)]
#[diesel(belongs_to(super::site::Site))]
#[diesel(table_name = control_requests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ControlRequest {
    pub id: i32,
    pub site_id: i32,
    /// Matches `SiteControl::id`, and so the React SLD component id.
    pub control_id: String,
    /// Serialized `SiteControlAction`.
    pub action: String,
    /// Serialized [`ControlRequestStatus`]. Stored as text so the set can grow
    /// without a migration.
    pub status: String,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
    pub sent_at: Option<chrono::NaiveDateTime>,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub failure_reason: Option<String>,
}

impl ControlRequest {
    /// Parse the stored status. Unrecognized values are treated as `Failed`
    /// rather than panicking — an unreadable request must never read as in
    /// flight or as a signal that got out.
    pub fn status(&self) -> ControlRequestStatus {
        self.status.parse().unwrap_or(ControlRequestStatus::Failed)
    }
}

/// Insertable row for a new control request.
#[derive(Insertable, Debug)]
#[diesel(table_name = control_requests)]
pub struct NewControlRequest {
    pub site_id: i32,
    pub control_id: String,
    pub action: String,
    pub status: String,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
}

/// A control request as served to clients.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ControlRequestDto {
    pub id: i32,
    pub site_id: i32,
    pub control_id: String,
    pub action: String,
    pub status: ControlRequestStatus,
    pub requested_by: Option<i32>,
    pub requested_at: chrono::NaiveDateTime,
    pub sent_at: Option<chrono::NaiveDateTime>,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    /// Why the signal never got out. Set only on `Failed`, and written for an
    /// operator to read: this is the text the diagram shows when a click goes
    /// nowhere.
    pub failure_reason: Option<String>,
}

impl From<ControlRequest> for ControlRequestDto {
    fn from(row: ControlRequest) -> Self {
        Self {
            status: row.status(),
            id: row.id,
            site_id: row.site_id,
            control_id: row.control_id,
            action: row.action,
            requested_by: row.requested_by,
            requested_at: row.requested_at,
            sent_at: row.sent_at,
            resolved_at: row.resolved_at,
            failure_reason: row.failure_reason,
        }
    }
}
