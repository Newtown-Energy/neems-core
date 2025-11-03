use diesel::prelude::*;

use crate::models::{
    NewScheduleTemplateEntry, ScheduleTemplateEntry, ScheduleTemplateEntryWithTimestamps,
};

/// Gets all template entries for a specific template ID.
pub fn get_template_entries_by_template(
    conn: &mut SqliteConnection,
    template_id_param: i32,
) -> Result<Vec<ScheduleTemplateEntry>, diesel::result::Error> {
    use crate::schema::schedule_template_entries::dsl::*;
    schedule_template_entries
        .filter(template_id.eq(template_id_param))
        .order(execution_offset_seconds.asc())
        .select(ScheduleTemplateEntry::as_select())
        .load(conn)
}

/// Gets all active template entries for a specific template ID.
pub fn get_active_template_entries_by_template(
    conn: &mut SqliteConnection,
    template_id_param: i32,
) -> Result<Vec<ScheduleTemplateEntry>, diesel::result::Error> {
    use crate::schema::schedule_template_entries::dsl::*;
    schedule_template_entries
        .filter(template_id.eq(template_id_param))
        .filter(is_active.eq(true))
        .order(execution_offset_seconds.asc())
        .select(ScheduleTemplateEntry::as_select())
        .load(conn)
}

/// Creates a new template entry in the database
pub fn insert_template_entry(
    conn: &mut SqliteConnection,
    new_entry: NewScheduleTemplateEntry,
    acting_user_id: Option<i32>,
) -> Result<ScheduleTemplateEntry, diesel::result::Error> {
    use crate::schema::schedule_template_entries::dsl::*;

    diesel::insert_into(schedule_template_entries)
        .values(&new_entry)
        .execute(conn)?;

    // Return the inserted entry
    let entry = schedule_template_entries
        .order(id.desc())
        .select(ScheduleTemplateEntry::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(
            conn,
            "schedule_template_entries",
            entry.id,
            "create",
            user_id,
        );
    }

    Ok(entry)
}

/// Gets a template entry by its ID.
pub fn get_template_entry_by_id(
    conn: &mut SqliteConnection,
    entry_id: i32,
) -> Result<Option<ScheduleTemplateEntry>, diesel::result::Error> {
    use crate::schema::schedule_template_entries::dsl::*;
    schedule_template_entries
        .filter(id.eq(entry_id))
        .select(ScheduleTemplateEntry::as_select())
        .first(conn)
        .optional()
}

/// Updates a template entry in the database
pub fn update_template_entry(
    conn: &mut SqliteConnection,
    entry_id: i32,
    new_execution_offset_seconds: Option<i32>,
    new_schedule_command_id: Option<i32>,
    new_is_active: Option<bool>,
    acting_user_id: Option<i32>,
) -> Result<ScheduleTemplateEntry, diesel::result::Error> {
    use crate::schema::schedule_template_entries::dsl::*;

    // First, get the current entry to preserve existing values
    let current = schedule_template_entries
        .filter(id.eq(entry_id))
        .select(ScheduleTemplateEntry::as_select())
        .first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(schedule_template_entries.filter(id.eq(entry_id)))
        .set((
            execution_offset_seconds
                .eq(new_execution_offset_seconds.unwrap_or(current.execution_offset_seconds)),
            schedule_command_id.eq(new_schedule_command_id.unwrap_or(current.schedule_command_id)),
            is_active.eq(new_is_active.unwrap_or(current.is_active)),
        ))
        .execute(conn)?;

    let updated = schedule_template_entries
        .filter(id.eq(entry_id))
        .select(ScheduleTemplateEntry::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(
            conn,
            "schedule_template_entries",
            entry_id,
            "update",
            user_id,
        );
    }

    Ok(updated)
}

/// Deletes a template entry from the database
pub fn delete_template_entry(
    conn: &mut SqliteConnection,
    entry_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::schedule_template_entries::dsl::*;

    // Update the activity log before deletion
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(
            conn,
            "schedule_template_entries",
            entry_id,
            "delete",
            user_id,
        );
    }

    diesel::delete(schedule_template_entries.filter(id.eq(entry_id))).execute(conn)
}

/// Gets a template entry with timestamps from entity activity
pub fn get_template_entry_with_timestamps(
    conn: &mut SqliteConnection,
    entry_id: i32,
) -> Result<Option<ScheduleTemplateEntryWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity::{get_created_at, get_updated_at};

    let entry = match get_template_entry_by_id(conn, entry_id)? {
        Some(e) => e,
        None => return Ok(None),
    };

    let created_at = get_created_at(conn, "schedule_template_entries", entry_id)?;
    let updated_at = get_updated_at(conn, "schedule_template_entries", entry_id)?;

    Ok(Some(ScheduleTemplateEntryWithTimestamps {
        id: entry.id,
        template_id: entry.template_id,
        execution_offset_seconds: entry.execution_offset_seconds,
        schedule_command_id: entry.schedule_command_id,
        is_active: entry.is_active,
        created_at,
        updated_at,
    }))
}
