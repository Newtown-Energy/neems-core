use diesel::prelude::*;

use crate::models::{NewScheduleCommand, ScheduleCommand, ScheduleCommandWithTimestamps};

/// Gets all schedule commands for a specific site ID.
pub fn get_schedule_commands_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<ScheduleCommand>, diesel::result::Error> {
    use crate::schema::schedule_commands::dsl::*;
    schedule_commands
        .filter(site_id.eq(site_id_param))
        .order(id.asc())
        .select(ScheduleCommand::as_select())
        .load(conn)
}

/// Gets all active schedule commands for a specific site ID.
pub fn get_active_schedule_commands_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<ScheduleCommand>, diesel::result::Error> {
    use crate::schema::schedule_commands::dsl::*;
    schedule_commands
        .filter(site_id.eq(site_id_param))
        .filter(is_active.eq(true))
        .order(id.asc())
        .select(ScheduleCommand::as_select())
        .load(conn)
}

/// Creates a new schedule command in the database
pub fn insert_schedule_command(
    conn: &mut SqliteConnection,
    new_schedule_command: NewScheduleCommand,
    acting_user_id: Option<i32>,
) -> Result<ScheduleCommand, diesel::result::Error> {
    use crate::schema::schedule_commands::dsl::*;

    diesel::insert_into(schedule_commands)
        .values(&new_schedule_command)
        .execute(conn)?;

    // Return the inserted schedule command
    let schedule_command = schedule_commands
        .order(id.desc())
        .select(ScheduleCommand::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(
            conn,
            "schedule_commands",
            schedule_command.id,
            "create",
            user_id,
        );
    }

    Ok(schedule_command)
}

/// Gets a schedule command by its ID.
pub fn get_schedule_command_by_id(
    conn: &mut SqliteConnection,
    schedule_command_id: i32,
) -> Result<Option<ScheduleCommand>, diesel::result::Error> {
    use crate::schema::schedule_commands::dsl::*;
    schedule_commands
        .filter(id.eq(schedule_command_id))
        .select(ScheduleCommand::as_select())
        .first(conn)
        .optional()
}

/// Updates a schedule command in the database
pub fn update_schedule_command(
    conn: &mut SqliteConnection,
    schedule_command_id: i32,
    new_type: Option<crate::models::CommandType>,
    new_parameters: Option<Option<String>>,
    new_is_active: Option<bool>,
    acting_user_id: Option<i32>,
) -> Result<ScheduleCommand, diesel::result::Error> {
    use crate::schema::schedule_commands::dsl::*;

    // First, get the current schedule command to preserve existing values
    let current_schedule_command = schedule_commands
        .filter(id.eq(schedule_command_id))
        .select(ScheduleCommand::as_select())
        .first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(schedule_commands.filter(id.eq(schedule_command_id)))
        .set((
            type_.eq(new_type.unwrap_or(current_schedule_command.type_)),
            parameters.eq(new_parameters.unwrap_or(current_schedule_command.parameters)),
            is_active.eq(new_is_active.unwrap_or(current_schedule_command.is_active)),
        ))
        .execute(conn)?;

    let updated_schedule_command = schedule_commands
        .filter(id.eq(schedule_command_id))
        .select(ScheduleCommand::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(
            conn,
            "schedule_commands",
            schedule_command_id,
            "update",
            user_id,
        );
    }

    Ok(updated_schedule_command)
}

/// Deletes a schedule command from the database
pub fn delete_schedule_command(
    conn: &mut SqliteConnection,
    schedule_command_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::schedule_commands::dsl::*;

    // Update the activity log before deletion (since trigger runs after)
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        // The trigger will create the delete entry, we'll update it after
        let _ = update_latest_activity_user(
            conn,
            "schedule_commands",
            schedule_command_id,
            "delete",
            user_id,
        );
    }

    diesel::delete(schedule_commands.filter(id.eq(schedule_command_id))).execute(conn)
}

/// Gets a schedule command with timestamps from entity activity
pub fn get_schedule_command_with_timestamps(
    conn: &mut SqliteConnection,
    schedule_command_id: i32,
) -> Result<Option<ScheduleCommandWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity::{get_created_at, get_updated_at};

    let schedule_command = match get_schedule_command_by_id(conn, schedule_command_id)? {
        Some(c) => c,
        None => return Ok(None),
    };

    let created_at = get_created_at(conn, "schedule_commands", schedule_command_id)?;
    let updated_at = get_updated_at(conn, "schedule_commands", schedule_command_id)?;

    Ok(Some(ScheduleCommandWithTimestamps {
        id: schedule_command.id,
        site_id: schedule_command.site_id,
        type_: schedule_command.type_,
        parameters: schedule_command.parameters,
        is_active: schedule_command.is_active,
        created_at,
        updated_at,
    }))
}
