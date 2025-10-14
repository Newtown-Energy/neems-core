use diesel::prelude::*;

use crate::models::{Command, CommandWithTimestamps, NewCommand};

/// Gets all commands for a specific site ID.
pub fn get_commands_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<Command>, diesel::result::Error> {
    use crate::schema::commands::dsl::*;
    commands
        .filter(site_id.eq(site_id_param))
        .order(id.asc())
        .select(Command::as_select())
        .load(conn)
}

/// Gets all active commands for a specific site ID.
pub fn get_active_commands_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<Command>, diesel::result::Error> {
    use crate::schema::commands::dsl::*;
    commands
        .filter(site_id.eq(site_id_param))
        .filter(is_active.eq(true))
        .order(id.asc())
        .select(Command::as_select())
        .load(conn)
}

/// Creates a new command in the database
pub fn insert_command(
    conn: &mut SqliteConnection,
    new_command: NewCommand,
    acting_user_id: Option<i32>,
) -> Result<Command, diesel::result::Error> {
    use crate::schema::commands::dsl::*;

    diesel::insert_into(commands).values(&new_command).execute(conn)?;

    // Return the inserted command
    let command = commands.order(id.desc()).select(Command::as_select()).first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "commands", command.id, "create", user_id);
    }

    Ok(command)
}

/// Gets a command by its ID.
pub fn get_command_by_id(
    conn: &mut SqliteConnection,
    command_id: i32,
) -> Result<Option<Command>, diesel::result::Error> {
    use crate::schema::commands::dsl::*;
    commands.filter(id.eq(command_id)).select(Command::as_select()).first(conn).optional()
}

/// Gets a command by site ID and name (case-insensitive).
pub fn get_command_by_site_and_name(
    conn: &mut SqliteConnection,
    command_site_id: i32,
    command_name: &str,
) -> Result<Option<Command>, diesel::result::Error> {
    diesel::sql_query("SELECT id, site_id, name, description, equipment_type, equipment_id, action, parameters, is_active FROM commands WHERE site_id = ? AND LOWER(name) = LOWER(?)")
        .bind::<diesel::sql_types::Integer, _>(command_site_id)
        .bind::<diesel::sql_types::Text, _>(command_name)
        .get_result::<Command>(conn)
        .optional()
}

/// Updates a command in the database
pub fn update_command(
    conn: &mut SqliteConnection,
    command_id: i32,
    new_name: Option<String>,
    new_description: Option<Option<String>>,
    new_equipment_type: Option<String>,
    new_equipment_id: Option<String>,
    new_action: Option<String>,
    new_parameters: Option<Option<String>>,
    new_is_active: Option<bool>,
    acting_user_id: Option<i32>,
) -> Result<Command, diesel::result::Error> {
    use crate::schema::commands::dsl::*;

    // First, get the current command to preserve existing values
    let current_command = commands.filter(id.eq(command_id)).select(Command::as_select()).first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(commands.filter(id.eq(command_id)))
        .set((
            name.eq(new_name.unwrap_or(current_command.name)),
            description.eq(new_description.unwrap_or(current_command.description)),
            equipment_type.eq(new_equipment_type.unwrap_or(current_command.equipment_type)),
            equipment_id.eq(new_equipment_id.unwrap_or(current_command.equipment_id)),
            action.eq(new_action.unwrap_or(current_command.action)),
            parameters.eq(new_parameters.unwrap_or(current_command.parameters)),
            is_active.eq(new_is_active.unwrap_or(current_command.is_active)),
        ))
        .execute(conn)?;

    let updated_command = commands.filter(id.eq(command_id)).select(Command::as_select()).first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "commands", command_id, "update", user_id);
    }

    Ok(updated_command)
}

/// Deletes a command from the database
pub fn delete_command(
    conn: &mut SqliteConnection,
    command_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::commands::dsl::*;

    // Update the activity log before deletion (since trigger runs after)
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        // The trigger will create the delete entry, we'll update it after
        let _ = update_latest_activity_user(conn, "commands", command_id, "delete", user_id);
    }

    diesel::delete(commands.filter(id.eq(command_id))).execute(conn)
}

/// Gets a command with timestamps from entity activity
pub fn get_command_with_timestamps(
    conn: &mut SqliteConnection,
    command_id: i32,
) -> Result<Option<CommandWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity::{get_created_at, get_updated_at};

    let command = match get_command_by_id(conn, command_id)? {
        Some(c) => c,
        None => return Ok(None),
    };

    let created_at = get_created_at(conn, "commands", command_id)?;
    let updated_at = get_updated_at(conn, "commands", command_id)?;

    Ok(Some(CommandWithTimestamps {
        id: command.id,
        site_id: command.site_id,
        name: command.name,
        description: command.description,
        equipment_type: command.equipment_type,
        equipment_id: command.equipment_id,
        action: command.action,
        parameters: command.parameters,
        is_active: command.is_active,
        created_at,
        updated_at,
    }))
}
