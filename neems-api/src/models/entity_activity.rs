use chrono::NaiveDateTime;
use diesel::{Identifiable, Insertable, Queryable, QueryableByName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{models::schedule_library::CommandType, schema::entity_activity};

#[derive(Queryable, Identifiable, QueryableByName, Debug, Serialize, Deserialize, TS)]
#[diesel(table_name = entity_activity)]
#[ts(export)]
pub struct EntityActivity {
    pub id: i32,
    pub table_name: String,
    pub entity_id: i32,
    pub operation_type: String, // 'create', 'update', 'delete'
    #[ts(type = "string")]
    pub timestamp: NaiveDateTime,
    pub user_id: Option<i32>,
    /// Free-form reason supplied at the API layer for `update`
    /// operations. Backfilled by the orm after the trigger row lands,
    /// so create rows produced purely by triggers stay NULL.
    pub change_reason: Option<String>,
    /// JSON-encoded [`ChangeDetails`] describing *what* changed.
    /// Backfilled alongside `change_reason`; NULL on rows written
    /// before this existed and on writes no API handler mediated.
    pub change_details: Option<String>,
}

#[derive(Insertable, Debug, Deserialize)]
#[diesel(table_name = entity_activity)]
pub struct NewEntityActivity {
    pub table_name: String,
    pub entity_id: i32,
    pub operation_type: String,
    pub timestamp: Option<NaiveDateTime>, // Optional to use database default
    pub user_id: Option<i32>,
    pub change_reason: Option<String>,
    pub change_details: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ActivityLogEntry {
    pub operation_type: String,
    #[ts(type = "string")]
    pub timestamp: NaiveDateTime,
    pub user_id: Option<i32>,
    pub change_reason: Option<String>,
}

// ============================================================================
// Change details (neems-core#136)
// ============================================================================
//
// `operation_type` says a row was created, updated or deleted. It never
// says *what* an operator changed, so a history pane can only render
// "Edited commands by alice@example.com" — true, and useless a week
// later. `ChangeDetails` is the missing payload: a structured before/
// after that the frontend turns into "Removed the 21:00 charge" or
// "Shortened the 16:00 discharge from 4 h to 2 h".
//
// It is stored JSON-encoded in `entity_activity.change_details` and
// computed where the before and after are both in hand — the orm
// update/create/delete helpers.

/// A single command as it stood at one side of a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CommandSnapshot {
    pub execution_offset_seconds: i32,
    pub command_type: CommandType,
    pub duration_seconds: Option<i32>,
    pub target_soc_percent: Option<i32>,
}

/// What happened to one command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum CommandChangeKind {
    Added,
    Removed,
    Modified,
}

/// One command's worth of change.
///
/// Commands are matched between the two sides by
/// `execution_offset_seconds`, which is unique within a schedule
/// (see `validate_execution_offsets`). A command whose time moved
/// therefore reads as a removal plus an addition rather than a move —
/// still accurate, and it avoids guessing at which command an operator
/// "meant" to drag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CommandChange {
    pub kind: CommandChangeKind,
    /// The command after the change — or, for `removed`, as it stood
    /// before it was dropped.
    pub command: CommandSnapshot,
    /// The command before the change. Only set for `modified`.
    pub previous: Option<CommandSnapshot>,
}

/// One scalar field's worth of change on the entity row itself.
///
/// `from`/`to` are rendered strings rather than typed values: the
/// fields this covers (a name, a description, a rule's dates) are
/// already text or trivially stringified, and a typed union per field
/// would buy the UI nothing it doesn't get from `field`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FieldChange {
    /// Column name, e.g. `name`, `description`, `rule_type`.
    pub field: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// Structured description of an operator's change, attached to one
/// `entity_activity` row.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChangeDetails {
    pub fields: Vec<FieldChange>,
    pub commands: Vec<CommandChange>,
}

impl ChangeDetails {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.commands.is_empty()
    }
}
