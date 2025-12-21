use diesel::{prelude::*, sql_types::BigInt};

use crate::models::{
    CommandType, CreateCommandRequest, CreateLibraryItemRequest, NewScheduleCommand,
    NewScheduleTemplate, NewScheduleTemplateEntry, ScheduleCommandDto, ScheduleLibraryItem,
    ScheduleTemplate, ScheduleTemplateEntry, UpdateLibraryItemRequest,
};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Creates a new library item with commands in a transaction
pub fn create_library_item(
    conn: &mut SqliteConnection,
    site_id: i32,
    request: CreateLibraryItemRequest,
    acting_user_id: Option<i32>,
) -> Result<ScheduleLibraryItem, diesel::result::Error> {
    use crate::schema::{schedule_commands, schedule_template_entries, schedule_templates};

    conn.transaction(|conn| {
        // 1. Validate execution offsets
        validate_execution_offsets(&request.commands)?;

        // 2. Validate unique name
        validate_library_item_name(conn, site_id, &request.name, None)?;

        // 3. Insert template
        let new_template = NewScheduleTemplate {
            site_id,
            name: request.name.clone(),
            description: request.description.clone(),
            is_active: true,
            is_default: false, // Normal schedules are not default
        };

        diesel::insert_into(schedule_templates::table)
            .values(&new_template)
            .execute(conn)?;

        let template_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
            .get_result::<LastInsertRowId>(conn)?
            .last_insert_rowid as i32;

        // Update activity log with user info
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ = update_latest_activity_user(
                conn,
                "schedule_templates",
                template_id,
                "create",
                user_id,
            );
        }

        // 4. For each command, create command + entry
        let mut created_commands = Vec::new();
        for cmd_req in request.commands.iter() {
            // Insert command
            let new_cmd = NewScheduleCommand {
                site_id,
                type_: cmd_req.command_type.as_str().to_string(),
                parameters: None,
                is_active: true,
            };

            diesel::insert_into(schedule_commands::table).values(&new_cmd).execute(conn)?;

            let cmd_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
                .get_result::<LastInsertRowId>(conn)?
                .last_insert_rowid as i32;

            // Insert entry
            let new_entry = NewScheduleTemplateEntry {
                template_id,
                execution_offset_seconds: cmd_req.execution_offset_seconds,
                schedule_command_id: cmd_id,
                is_active: true,
            };

            diesel::insert_into(schedule_template_entries::table)
                .values(&new_entry)
                .execute(conn)?;

            created_commands.push(ScheduleCommandDto {
                id: cmd_id,
                execution_offset_seconds: cmd_req.execution_offset_seconds,
                command_type: cmd_req.command_type.clone(),
            });
        }

        // 5. Get the created template
        let template =
            schedule_templates::table.find(template_id).first::<ScheduleTemplate>(conn)?;

        Ok(ScheduleLibraryItem {
            id: template.id,
            site_id: template.site_id,
            name: template.name,
            description: template.description,
            commands: created_commands,
            created_at: template.created_at,
        })
    })
}

/// Gets a library item by ID with all its commands
pub fn get_library_item(
    conn: &mut SqliteConnection,
    item_id: i32,
) -> Result<ScheduleLibraryItem, diesel::result::Error> {
    use crate::schema::{schedule_commands, schedule_template_entries, schedule_templates};

    // Get template
    let template = schedule_templates::table.find(item_id).first::<ScheduleTemplate>(conn)?;

    // Get entries with commands (JOIN)
    let entries_with_commands: Vec<(ScheduleTemplateEntry, String)> =
        schedule_template_entries::table
            .inner_join(schedule_commands::table)
            .filter(schedule_template_entries::template_id.eq(item_id))
            .filter(schedule_template_entries::is_active.eq(true))
            .order_by(schedule_template_entries::execution_offset_seconds.asc())
            .select((ScheduleTemplateEntry::as_select(), schedule_commands::type_))
            .load(conn)?;

    // Map to ScheduleCommandDto
    let commands: Result<Vec<ScheduleCommandDto>, String> = entries_with_commands
        .into_iter()
        .map(|(entry, type_str)| {
            Ok(ScheduleCommandDto {
                id: entry.id,
                execution_offset_seconds: entry.execution_offset_seconds,
                command_type: CommandType::from_str(&type_str)?,
            })
        })
        .collect();

    let commands = commands.map_err(|e| {
        diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e,
        )))
    })?;

    Ok(ScheduleLibraryItem {
        id: template.id,
        site_id: template.site_id,
        name: template.name,
        description: template.description,
        commands,
        created_at: template.created_at,
    })
}

/// Gets all library items for a site
pub fn get_library_items_for_site(
    conn: &mut SqliteConnection,
    site_id: i32,
) -> Result<Vec<ScheduleLibraryItem>, diesel::result::Error> {
    use crate::schema::schedule_templates;

    let templates: Vec<ScheduleTemplate> = schedule_templates::table
        .filter(schedule_templates::site_id.eq(site_id))
        .filter(schedule_templates::is_active.eq(true))
        .order_by(schedule_templates::name.asc())
        .load(conn)?;

    let mut items = Vec::new();
    for template in templates {
        items.push(get_library_item(conn, template.id)?);
    }

    Ok(items)
}

/// Updates a library item (replaces commands atomically)
/// Note: is_default flag cannot be changed (enforced by database triggers)
pub fn update_library_item(
    conn: &mut SqliteConnection,
    item_id: i32,
    request: UpdateLibraryItemRequest,
    acting_user_id: Option<i32>,
) -> Result<ScheduleLibraryItem, diesel::result::Error> {
    use crate::schema::{schedule_commands, schedule_template_entries, schedule_templates};

    conn.transaction(|conn| {
        // Get current template
        let current = schedule_templates::table.find(item_id).first::<ScheduleTemplate>(conn)?;

        // Validate name if changing
        if let Some(ref new_name) = request.name {
            validate_library_item_name(conn, current.site_id, new_name, Some(item_id))?;
        }

        // Update template fields
        if let Some(name_val) = request.name {
            diesel::update(schedule_templates::table.filter(schedule_templates::id.eq(item_id)))
                .set(schedule_templates::name.eq(name_val))
                .execute(conn)?;
        }

        if let Some(description_val) = request.description {
            diesel::update(schedule_templates::table.filter(schedule_templates::id.eq(item_id)))
                .set(schedule_templates::description.eq(description_val))
                .execute(conn)?;
        }

        // Update activity log
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ =
                update_latest_activity_user(conn, "schedule_templates", item_id, "update", user_id);
        }

        // Replace commands if provided
        if let Some(commands) = request.commands {
            validate_execution_offsets(&commands)?;

            // Get existing entries
            let existing_entries: Vec<ScheduleTemplateEntry> = schedule_template_entries::table
                .filter(schedule_template_entries::template_id.eq(item_id))
                .load(conn)?;

            // Delete existing entries and their commands
            for entry in existing_entries {
                diesel::delete(
                    schedule_template_entries::table
                        .filter(schedule_template_entries::id.eq(entry.id)),
                )
                .execute(conn)?;

                diesel::delete(
                    schedule_commands::table
                        .filter(schedule_commands::id.eq(entry.schedule_command_id)),
                )
                .execute(conn)?;
            }

            // Create new commands and entries
            for cmd_req in commands.iter() {
                let new_cmd = NewScheduleCommand {
                    site_id: current.site_id,
                    type_: cmd_req.command_type.as_str().to_string(),
                    parameters: None,
                    is_active: true,
                };

                diesel::insert_into(schedule_commands::table).values(&new_cmd).execute(conn)?;

                let cmd_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
                    .get_result::<LastInsertRowId>(conn)?
                    .last_insert_rowid as i32;

                let new_entry = NewScheduleTemplateEntry {
                    template_id: item_id,
                    execution_offset_seconds: cmd_req.execution_offset_seconds,
                    schedule_command_id: cmd_id,
                    is_active: true,
                };

                diesel::insert_into(schedule_template_entries::table)
                    .values(&new_entry)
                    .execute(conn)?;
            }
        }

        // Return updated item
        get_library_item(conn, item_id)
    })
}

/// Deletes a library item (cascades to entries and rules)
/// Returns an error if the item is the default schedule
pub fn delete_library_item(
    conn: &mut SqliteConnection,
    item_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::schedule_templates;

    // Check if this is the default schedule
    let template = schedule_templates::table.find(item_id).first::<ScheduleTemplate>(conn)?;

    if template.is_default {
        return Err(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::ForeignKeyViolation,
            Box::new("Cannot delete the default schedule".to_string()),
        ));
    }

    let result =
        diesel::delete(schedule_templates::table.filter(schedule_templates::id.eq(item_id)))
            .execute(conn)?;

    if result > 0 {
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ =
                update_latest_activity_user(conn, "schedule_templates", item_id, "delete", user_id);
        }
    }

    Ok(result)
}

/// Clones a library item with a new name
pub fn clone_library_item(
    conn: &mut SqliteConnection,
    item_id: i32,
    new_name: String,
    new_description: Option<String>,
    acting_user_id: Option<i32>,
) -> Result<ScheduleLibraryItem, diesel::result::Error> {
    // Get original item
    let original = get_library_item(conn, item_id)?;

    // Create new item with same commands
    let create_request = CreateLibraryItemRequest {
        name: new_name,
        description: new_description,
        commands: original
            .commands
            .into_iter()
            .map(|cmd| CreateCommandRequest {
                execution_offset_seconds: cmd.execution_offset_seconds,
                command_type: cmd.command_type,
            })
            .collect(),
    };

    create_library_item(conn, original.site_id, create_request, acting_user_id)
}

// ============================================================================
// Default schedule helpers
// ============================================================================

/// Ensures a "Default" schedule exists for a site
/// Creates it if it doesn't exist and returns the schedule
pub fn ensure_default_schedule_exists(
    conn: &mut SqliteConnection,
    site_id: i32,
    acting_user_id: Option<i32>,
) -> Result<ScheduleLibraryItem, diesel::result::Error> {
    use crate::schema::schedule_templates;

    // Check if there's already a default schedule for this site
    let existing_default = schedule_templates::table
        .filter(schedule_templates::site_id.eq(site_id))
        .filter(schedule_templates::is_default.eq(true))
        .filter(schedule_templates::is_active.eq(true))
        .first::<ScheduleTemplate>(conn)
        .optional()?;

    if let Some(default_template) = existing_default {
        // Default schedule already exists, return it
        return get_library_item(conn, default_template.id);
    }

    // No default schedule exists, create one named "Default"
    conn.transaction(|conn| {
        // Insert template with is_default = true
        let new_template = NewScheduleTemplate {
            site_id,
            name: "Default".to_string(),
            description: Some("Default schedule".to_string()),
            is_active: true,
            is_default: true, // Mark as default
        };

        diesel::insert_into(schedule_templates::table)
            .values(&new_template)
            .execute(conn)?;

        let template_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
            .get_result::<LastInsertRowId>(conn)?
            .last_insert_rowid as i32;

        // Update activity log with user info
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ = update_latest_activity_user(
                conn,
                "schedule_templates",
                template_id,
                "create",
                user_id,
            );
        }

        // Create a default application rule for the default schedule
        use crate::{
            models::application_rule::{CreateApplicationRuleRequest, RuleType},
            orm::application_rule::create_application_rule,
        };

        let default_rule_request = CreateApplicationRuleRequest {
            rule_type: RuleType::Default,
            days_of_week: None,
            specific_dates: None,
            override_reason: None,
        };

        // Create the default rule - ignore errors since rule creation is best-effort
        let _ = create_application_rule(conn, template_id, default_rule_request, acting_user_id);

        // Get and return the created template
        get_library_item(conn, template_id)
    })
}

// ============================================================================
// Validation helpers
// ============================================================================

/// Validates that a library item name is unique within a site
fn validate_library_item_name(
    conn: &mut SqliteConnection,
    site_id: i32,
    name: &str,
    exclude_id: Option<i32>,
) -> Result<(), diesel::result::Error> {
    use crate::schema::schedule_templates;

    let mut query = schedule_templates::table
        .filter(schedule_templates::site_id.eq(site_id))
        .filter(schedule_templates::name.eq(name))
        .filter(schedule_templates::is_active.eq(true))
        .into_boxed();

    if let Some(id) = exclude_id {
        query = query.filter(schedule_templates::id.ne(id));
    }

    let count: i64 = query.count().get_result(conn)?;

    if count > 0 {
        return Err(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            Box::new("A schedule with this name already exists".to_string()),
        ));
    }

    Ok(())
}

/// Validates execution offsets are within range and unique
fn validate_execution_offsets(
    commands: &[CreateCommandRequest],
) -> Result<(), diesel::result::Error> {
    // Check range
    for cmd in commands {
        if cmd.execution_offset_seconds < 0 || cmd.execution_offset_seconds >= 86400 {
            return Err(diesel::result::Error::DeserializationError(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Execution time must be within 24 hours (0-86399 seconds)",
                ),
            )));
        }
    }

    // Check duplicates
    let mut offsets: Vec<i32> = commands.iter().map(|c| c.execution_offset_seconds).collect();
    offsets.sort_unstable();
    for window in offsets.windows(2) {
        if window[0] == window[1] {
            return Err(diesel::result::Error::DeserializationError(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Duplicate execution times are not allowed",
                ),
            )));
        }
    }

    Ok(())
}
