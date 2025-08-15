use diesel::prelude::*;
use crate::models::{NewSchedulerScript, SchedulerScript, SchedulerScriptInput, UpdateSchedulerScriptRequest};

/// Gets all scheduler scripts for a specific site ID.
pub fn get_scheduler_scripts_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<SchedulerScript>, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;
    scheduler_scripts
        .filter(site_id.eq(site_id_param))
        .order(id.asc())
        .select(SchedulerScript::as_select())
        .load(conn)
}

/// Gets all active scheduler scripts for a specific site ID.
pub fn get_active_scheduler_scripts_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<SchedulerScript>, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;
    scheduler_scripts
        .filter(site_id.eq(site_id_param).and(is_active.eq(true)))
        .order(id.asc())
        .select(SchedulerScript::as_select())
        .load(conn)
}

/// Gets a scheduler script by ID.
pub fn get_scheduler_script_by_id(
    conn: &mut SqliteConnection,
    script_id: i32,
) -> Result<Option<SchedulerScript>, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;
    scheduler_scripts
        .find(script_id)
        .select(SchedulerScript::as_select())
        .first(conn)
        .optional()
}

/// Gets all scheduler scripts.
pub fn get_all_scheduler_scripts(
    conn: &mut SqliteConnection,
) -> Result<Vec<SchedulerScript>, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;
    scheduler_scripts
        .order(id.asc())
        .select(SchedulerScript::as_select())
        .load(conn)
}

/// Creates a new scheduler script in the database.
pub fn insert_scheduler_script(
    conn: &mut SqliteConnection,
    input: SchedulerScriptInput,
    acting_user_id: Option<i32>,
) -> Result<SchedulerScript, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;

    let new_script = NewSchedulerScript::from(input);

    diesel::insert_into(scheduler_scripts)
        .values(&new_script)
        .execute(conn)?;

    // Return the inserted script
    let script = scheduler_scripts
        .order(id.desc())
        .select(SchedulerScript::as_select())
        .first(conn)?;
    
    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "scheduler_scripts", script.id, "create", user_id);
    }
    
    Ok(script)
}

/// Updates a scheduler script.
pub fn update_scheduler_script(
    conn: &mut SqliteConnection,
    script_id: i32,
    update_request: UpdateSchedulerScriptRequest,
    acting_user_id: Option<i32>,
) -> Result<SchedulerScript, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;

    // Execute separate update statements for each field
    if let Some(new_name) = update_request.name {
        diesel::update(scheduler_scripts.find(script_id))
            .set(name.eq(new_name))
            .execute(conn)?;
    }
    if let Some(new_content) = update_request.script_content {
        diesel::update(scheduler_scripts.find(script_id))
            .set(script_content.eq(new_content))
            .execute(conn)?;
    }
    if let Some(new_language) = update_request.language {
        diesel::update(scheduler_scripts.find(script_id))
            .set(language.eq(new_language))
            .execute(conn)?;
    }
    if let Some(new_active) = update_request.is_active {
        diesel::update(scheduler_scripts.find(script_id))
            .set(is_active.eq(new_active))
            .execute(conn)?;
    }
    if let Some(new_version) = update_request.version {
        diesel::update(scheduler_scripts.find(script_id))
            .set(version.eq(new_version))
            .execute(conn)?;
    }

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "scheduler_scripts", script_id, "update", user_id);
    }

    // Return the updated script
    scheduler_scripts
        .find(script_id)
        .select(SchedulerScript::as_select())
        .first(conn)
}

/// Deletes a scheduler script.
pub fn delete_scheduler_script(
    conn: &mut SqliteConnection,
    script_id: i32,
    acting_user_id: Option<i32>,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;

    // Update the trigger-created activity entry with user information before deletion
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "scheduler_scripts", script_id, "delete", user_id);
    }

    let affected_rows = diesel::delete(scheduler_scripts.find(script_id)).execute(conn)?;
    Ok(affected_rows > 0)
}

/// Gets the latest active script for a site (highest version number).
pub fn get_latest_active_script_for_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Option<SchedulerScript>, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;
    scheduler_scripts
        .filter(site_id.eq(site_id_param).and(is_active.eq(true)))
        .order(version.desc())
        .select(SchedulerScript::as_select())
        .first(conn)
        .optional()
}

/// Deactivates all scripts for a site except the specified one.
pub fn deactivate_other_scripts_for_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    except_script_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;

    let affected_rows = diesel::update(
        scheduler_scripts
            .filter(site_id.eq(site_id_param).and(id.ne(except_script_id)))
    )
    .set(is_active.eq(false))
    .execute(conn)?;

    // Update activity entries for all affected scripts
    if let Some(_user_id) = acting_user_id {
        // Note: This is a bulk operation, so we can't easily track individual script updates
        // in the activity log. In a production system, you might want to handle this differently.
    }

    Ok(affected_rows)
}

/// Validates that a script name is unique within a site (excluding a specific script ID for updates).
pub fn is_script_name_unique_in_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    script_name: &str,
    exclude_script_id: Option<i32>,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::scheduler_scripts::dsl::*;

    let mut query = scheduler_scripts
        .filter(site_id.eq(site_id_param).and(name.eq(script_name)))
        .into_boxed();

    if let Some(exclude_id) = exclude_script_id {
        query = query.filter(id.ne(exclude_id));
    }

    let count: i64 = query.count().get_result(conn)?;
    Ok(count == 0)
}