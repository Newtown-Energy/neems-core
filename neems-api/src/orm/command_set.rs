use diesel::prelude::*;

use crate::models::{
    Command, CommandSet, CommandSetCommand, CommandSetWithTimestamps, NewCommandSet,
    NewCommandSetCommand,
};

/// Gets all command sets for a specific site ID.
pub fn get_command_sets_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<CommandSet>, diesel::result::Error> {
    use crate::schema::command_sets::dsl::*;
    command_sets
        .filter(site_id.eq(site_id_param))
        .order(id.asc())
        .select(CommandSet::as_select())
        .load(conn)
}

/// Gets all active command sets for a specific site ID.
pub fn get_active_command_sets_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<CommandSet>, diesel::result::Error> {
    use crate::schema::command_sets::dsl::*;
    command_sets
        .filter(site_id.eq(site_id_param))
        .filter(is_active.eq(true))
        .order(id.asc())
        .select(CommandSet::as_select())
        .load(conn)
}

/// Creates a new command set in the database
pub fn insert_command_set(
    conn: &mut SqliteConnection,
    new_command_set: NewCommandSet,
    acting_user_id: Option<i32>,
) -> Result<CommandSet, diesel::result::Error> {
    use crate::schema::command_sets::dsl::*;

    diesel::insert_into(command_sets)
        .values(&new_command_set)
        .execute(conn)?;

    // Return the inserted command set
    let command_set = command_sets
        .order(id.desc())
        .select(CommandSet::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "command_sets", command_set.id, "create", user_id);
    }

    Ok(command_set)
}

/// Gets a command set by its ID.
pub fn get_command_set_by_id(
    conn: &mut SqliteConnection,
    command_set_id: i32,
) -> Result<Option<CommandSet>, diesel::result::Error> {
    use crate::schema::command_sets::dsl::*;
    command_sets
        .filter(id.eq(command_set_id))
        .select(CommandSet::as_select())
        .first(conn)
        .optional()
}

/// Gets a command set by site ID and name (case-insensitive).
pub fn get_command_set_by_site_and_name(
    conn: &mut SqliteConnection,
    command_set_site_id: i32,
    command_set_name: &str,
) -> Result<Option<CommandSet>, diesel::result::Error> {
    diesel::sql_query(
        "SELECT id, site_id, name, description, is_active FROM command_sets WHERE site_id = ? AND LOWER(name) = LOWER(?)",
    )
    .bind::<diesel::sql_types::Integer, _>(command_set_site_id)
    .bind::<diesel::sql_types::Text, _>(command_set_name)
    .get_result::<CommandSet>(conn)
    .optional()
}

/// Updates a command set in the database
pub fn update_command_set(
    conn: &mut SqliteConnection,
    command_set_id: i32,
    new_name: Option<String>,
    new_description: Option<Option<String>>,
    new_is_active: Option<bool>,
    acting_user_id: Option<i32>,
) -> Result<CommandSet, diesel::result::Error> {
    use crate::schema::command_sets::dsl::*;

    // First, get the current command set to preserve existing values
    let current = command_sets
        .filter(id.eq(command_set_id))
        .select(CommandSet::as_select())
        .first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(command_sets.filter(id.eq(command_set_id)))
        .set((
            name.eq(new_name.unwrap_or(current.name)),
            description.eq(new_description.unwrap_or(current.description)),
            is_active.eq(new_is_active.unwrap_or(current.is_active)),
        ))
        .execute(conn)?;

    let updated = command_sets
        .filter(id.eq(command_set_id))
        .select(CommandSet::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "command_sets", command_set_id, "update", user_id);
    }

    Ok(updated)
}

/// Deletes a command set from the database
pub fn delete_command_set(
    conn: &mut SqliteConnection,
    command_set_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::command_sets::dsl::*;

    // Update the activity log before deletion
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "command_sets", command_set_id, "delete", user_id);
    }

    diesel::delete(command_sets.filter(id.eq(command_set_id))).execute(conn)
}

/// Gets all commands in a command set, ordered by execution_order
pub fn get_commands_in_set(
    conn: &mut SqliteConnection,
    command_set_id: i32,
) -> Result<Vec<(CommandSetCommand, Command)>, diesel::result::Error> {
    use crate::schema::{command_set_commands, commands};

    command_set_commands::table
        .inner_join(commands::table)
        .filter(command_set_commands::command_set_id.eq(command_set_id))
        .order(command_set_commands::execution_order.asc())
        .select((
            CommandSetCommand::as_select(),
            Command::as_select(),
        ))
        .load(conn)
}

/// Adds a command to a command set
pub fn add_command_to_set(
    conn: &mut SqliteConnection,
    new_command_set_command: NewCommandSetCommand,
) -> Result<CommandSetCommand, diesel::result::Error> {
    use crate::schema::command_set_commands::dsl::*;

    diesel::insert_into(command_set_commands)
        .values(&new_command_set_command)
        .execute(conn)?;

    command_set_commands
        .filter(command_set_id.eq(new_command_set_command.command_set_id))
        .filter(command_id.eq(new_command_set_command.command_id))
        .select(CommandSetCommand::as_select())
        .first(conn)
}

/// Removes a command from a command set
pub fn remove_command_from_set(
    conn: &mut SqliteConnection,
    set_id: i32,
    cmd_id: i32,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::command_set_commands::dsl::*;

    diesel::delete(
        command_set_commands
            .filter(command_set_id.eq(set_id))
            .filter(command_id.eq(cmd_id)),
    )
    .execute(conn)
}

/// Updates the execution order of a command in a command set
pub fn update_command_in_set(
    conn: &mut SqliteConnection,
    set_id: i32,
    cmd_id: i32,
    new_execution_order: Option<i32>,
    new_delay_ms: Option<Option<i32>>,
    new_condition: Option<Option<String>>,
) -> Result<CommandSetCommand, diesel::result::Error> {
    use crate::schema::command_set_commands::dsl::*;

    // Get current values
    let current = command_set_commands
        .filter(command_set_id.eq(set_id))
        .filter(command_id.eq(cmd_id))
        .select(CommandSetCommand::as_select())
        .first(conn)?;

    diesel::update(
        command_set_commands
            .filter(command_set_id.eq(set_id))
            .filter(command_id.eq(cmd_id)),
    )
    .set((
        execution_order.eq(new_execution_order.unwrap_or(current.execution_order)),
        delay_ms.eq(new_delay_ms.unwrap_or(current.delay_ms)),
        condition.eq(new_condition.unwrap_or(current.condition)),
    ))
    .execute(conn)?;

    command_set_commands
        .filter(command_set_id.eq(set_id))
        .filter(command_id.eq(cmd_id))
        .select(CommandSetCommand::as_select())
        .first(conn)
}

/// Gets a command set with timestamps from entity activity
pub fn get_command_set_with_timestamps(
    conn: &mut SqliteConnection,
    command_set_id: i32,
) -> Result<Option<CommandSetWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity::{get_created_at, get_updated_at};

    let command_set = match get_command_set_by_id(conn, command_set_id)? {
        Some(cs) => cs,
        None => return Ok(None),
    };

    let created_at = get_created_at(conn, "command_sets", command_set_id)?;
    let updated_at = get_updated_at(conn, "command_sets", command_set_id)?;

    Ok(Some(CommandSetWithTimestamps {
        id: command_set.id,
        site_id: command_set.site_id,
        name: command_set.name,
        description: command_set.description,
        is_active: command_set.is_active,
        created_at,
        updated_at,
    }))
}
