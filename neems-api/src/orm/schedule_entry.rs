use diesel::prelude::*;

use crate::models::{NewScheduleEntry, ScheduleEntry, ScheduleEntryWithTimestamps};

/// Gets all schedule entries for a specific schedule ID.
pub fn get_schedule_entries_by_schedule(
    conn: &mut SqliteConnection,
    schedule_id_param: i32,
) -> Result<Vec<ScheduleEntry>, diesel::result::Error> {
    use crate::schema::schedule_entries::dsl::*;
    schedule_entries
        .filter(schedule_id.eq(schedule_id_param))
        .order(execution_offset_seconds.asc())
        .select(ScheduleEntry::as_select())
        .load(conn)
}

/// Gets all active schedule entries for a specific schedule ID.
pub fn get_active_schedule_entries_by_schedule(
    conn: &mut SqliteConnection,
    schedule_id_param: i32,
) -> Result<Vec<ScheduleEntry>, diesel::result::Error> {
    use crate::schema::schedule_entries::dsl::*;
    schedule_entries
        .filter(schedule_id.eq(schedule_id_param))
        .filter(is_active.eq(true))
        .order(execution_offset_seconds.asc())
        .select(ScheduleEntry::as_select())
        .load(conn)
}

/// Gets schedule entries within an offset range for a schedule
pub fn get_schedule_entries_by_offset_range(
    conn: &mut SqliteConnection,
    schedule_id_param: i32,
    min_offset_seconds: i32,
    max_offset_seconds: i32,
) -> Result<Vec<ScheduleEntry>, diesel::result::Error> {
    use crate::schema::schedule_entries::dsl::*;
    schedule_entries
        .filter(schedule_id.eq(schedule_id_param))
        .filter(execution_offset_seconds.ge(min_offset_seconds))
        .filter(execution_offset_seconds.le(max_offset_seconds))
        .order(execution_offset_seconds.asc())
        .select(ScheduleEntry::as_select())
        .load(conn)
}

/// Creates a new schedule entry in the database
pub fn insert_schedule_entry(
    conn: &mut SqliteConnection,
    new_entry: NewScheduleEntry,
    acting_user_id: Option<i32>,
) -> Result<ScheduleEntry, diesel::result::Error> {
    use crate::schema::schedule_entries::dsl::*;

    diesel::insert_into(schedule_entries).values(&new_entry).execute(conn)?;

    // Return the inserted entry
    let entry = schedule_entries
        .order(id.desc())
        .select(ScheduleEntry::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "schedule_entries", entry.id, "create", user_id);
    }

    Ok(entry)
}

/// Gets a schedule entry by its ID.
pub fn get_schedule_entry_by_id(
    conn: &mut SqliteConnection,
    entry_id: i32,
) -> Result<Option<ScheduleEntry>, diesel::result::Error> {
    use crate::schema::schedule_entries::dsl::*;
    schedule_entries
        .filter(id.eq(entry_id))
        .select(ScheduleEntry::as_select())
        .first(conn)
        .optional()
}

/// Updates a schedule entry in the database
pub fn update_schedule_entry(
    conn: &mut SqliteConnection,
    entry_id: i32,
    new_execution_offset_seconds: Option<i32>,
    new_schedule_command_id: Option<i32>,
    new_is_active: Option<bool>,
    acting_user_id: Option<i32>,
) -> Result<ScheduleEntry, diesel::result::Error> {
    use crate::schema::schedule_entries::dsl::*;

    // First, get the current entry to preserve existing values
    let current = schedule_entries
        .filter(id.eq(entry_id))
        .select(ScheduleEntry::as_select())
        .first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(schedule_entries.filter(id.eq(entry_id)))
        .set((
            execution_offset_seconds
                .eq(new_execution_offset_seconds.unwrap_or(current.execution_offset_seconds)),
            schedule_command_id.eq(new_schedule_command_id.unwrap_or(current.schedule_command_id)),
            is_active.eq(new_is_active.unwrap_or(current.is_active)),
        ))
        .execute(conn)?;

    let updated = schedule_entries
        .filter(id.eq(entry_id))
        .select(ScheduleEntry::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "schedule_entries", entry_id, "update", user_id);
    }

    Ok(updated)
}

/// Deletes a schedule entry from the database
pub fn delete_schedule_entry(
    conn: &mut SqliteConnection,
    entry_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::schedule_entries::dsl::*;

    // Update the activity log before deletion
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "schedule_entries", entry_id, "delete", user_id);
    }

    diesel::delete(schedule_entries.filter(id.eq(entry_id))).execute(conn)
}

/// Gets a schedule entry with timestamps from entity activity
pub fn get_schedule_entry_with_timestamps(
    conn: &mut SqliteConnection,
    entry_id: i32,
) -> Result<Option<ScheduleEntryWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity::{get_created_at, get_updated_at};

    let entry = match get_schedule_entry_by_id(conn, entry_id)? {
        Some(e) => e,
        None => return Ok(None),
    };

    let created_at = get_created_at(conn, "schedule_entries", entry_id)?;
    let updated_at = get_updated_at(conn, "schedule_entries", entry_id)?;

    Ok(Some(ScheduleEntryWithTimestamps {
        id: entry.id,
        schedule_id: entry.schedule_id,
        execution_offset_seconds: entry.execution_offset_seconds,
        schedule_command_id: entry.schedule_command_id,
        is_active: entry.is_active,
        created_at,
        updated_at,
    }))
}
