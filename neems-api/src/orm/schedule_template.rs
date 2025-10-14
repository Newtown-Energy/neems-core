use diesel::prelude::*;

use crate::models::{NewScheduleTemplate, ScheduleTemplate, ScheduleTemplateWithTimestamps};

/// Gets all schedule templates for a specific site ID.
pub fn get_schedule_templates_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<ScheduleTemplate>, diesel::result::Error> {
    use crate::schema::schedule_templates::dsl::*;
    schedule_templates
        .filter(site_id.eq(site_id_param))
        .order(id.asc())
        .select(ScheduleTemplate::as_select())
        .load(conn)
}

/// Gets all active schedule templates for a specific site ID.
pub fn get_active_schedule_templates_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<ScheduleTemplate>, diesel::result::Error> {
    use crate::schema::schedule_templates::dsl::*;
    schedule_templates
        .filter(site_id.eq(site_id_param))
        .filter(is_active.eq(true))
        .order(id.asc())
        .select(ScheduleTemplate::as_select())
        .load(conn)
}

/// Gets the default schedule template for a site (if any)
pub fn get_default_schedule_template_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Option<ScheduleTemplate>, diesel::result::Error> {
    use crate::schema::schedule_templates::dsl::*;
    schedule_templates
        .filter(site_id.eq(site_id_param))
        .filter(is_default.eq(true))
        .filter(is_active.eq(true))
        .select(ScheduleTemplate::as_select())
        .first(conn)
        .optional()
}

/// Creates a new schedule template in the database
pub fn insert_schedule_template(
    conn: &mut SqliteConnection,
    new_template: NewScheduleTemplate,
    acting_user_id: Option<i32>,
) -> Result<ScheduleTemplate, diesel::result::Error> {
    use crate::schema::schedule_templates::dsl::*;

    // If this template is being set as default, unset any existing defaults for
    // this site
    if new_template.is_default {
        diesel::update(schedule_templates.filter(site_id.eq(new_template.site_id)))
            .set(is_default.eq(false))
            .execute(conn)?;
    }

    diesel::insert_into(schedule_templates).values(&new_template).execute(conn)?;

    // Return the inserted template
    let template = schedule_templates
        .order(id.desc())
        .select(ScheduleTemplate::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ =
            update_latest_activity_user(conn, "schedule_templates", template.id, "create", user_id);
    }

    Ok(template)
}

/// Gets a schedule template by its ID.
pub fn get_schedule_template_by_id(
    conn: &mut SqliteConnection,
    template_id: i32,
) -> Result<Option<ScheduleTemplate>, diesel::result::Error> {
    use crate::schema::schedule_templates::dsl::*;
    schedule_templates
        .filter(id.eq(template_id))
        .select(ScheduleTemplate::as_select())
        .first(conn)
        .optional()
}

/// Gets a schedule template by site ID and name (case-insensitive).
pub fn get_schedule_template_by_site_and_name(
    conn: &mut SqliteConnection,
    template_site_id: i32,
    template_name: &str,
) -> Result<Option<ScheduleTemplate>, diesel::result::Error> {
    diesel::sql_query("SELECT id, site_id, name, description, is_default, is_active FROM schedule_templates WHERE site_id = ? AND LOWER(name) = LOWER(?)")
        .bind::<diesel::sql_types::Integer, _>(template_site_id)
        .bind::<diesel::sql_types::Text, _>(template_name)
        .get_result::<ScheduleTemplate>(conn)
        .optional()
}

/// Updates a schedule template in the database
pub fn update_schedule_template(
    conn: &mut SqliteConnection,
    template_id: i32,
    new_name: Option<String>,
    new_description: Option<Option<String>>,
    new_is_default: Option<bool>,
    new_is_active: Option<bool>,
    acting_user_id: Option<i32>,
) -> Result<ScheduleTemplate, diesel::result::Error> {
    use crate::schema::schedule_templates::dsl::*;

    // First, get the current template to preserve existing values
    let current = schedule_templates
        .filter(id.eq(template_id))
        .select(ScheduleTemplate::as_select())
        .first(conn)?;

    // If setting as default, unset any existing defaults for this site
    if new_is_default == Some(true) {
        diesel::update(
            schedule_templates
                .filter(site_id.eq(current.site_id))
                .filter(id.ne(template_id)),
        )
        .set(is_default.eq(false))
        .execute(conn)?;
    }

    // Update with new values or keep existing ones
    diesel::update(schedule_templates.filter(id.eq(template_id)))
        .set((
            name.eq(new_name.unwrap_or(current.name)),
            description.eq(new_description.unwrap_or(current.description)),
            is_default.eq(new_is_default.unwrap_or(current.is_default)),
            is_active.eq(new_is_active.unwrap_or(current.is_active)),
        ))
        .execute(conn)?;

    let updated = schedule_templates
        .filter(id.eq(template_id))
        .select(ScheduleTemplate::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ =
            update_latest_activity_user(conn, "schedule_templates", template_id, "update", user_id);
    }

    Ok(updated)
}

/// Deletes a schedule template from the database
pub fn delete_schedule_template(
    conn: &mut SqliteConnection,
    template_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::schedule_templates::dsl::*;

    // Update the activity log before deletion
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ =
            update_latest_activity_user(conn, "schedule_templates", template_id, "delete", user_id);
    }

    diesel::delete(schedule_templates.filter(id.eq(template_id))).execute(conn)
}

/// Gets a schedule template with timestamps from entity activity
pub fn get_schedule_template_with_timestamps(
    conn: &mut SqliteConnection,
    template_id: i32,
) -> Result<Option<ScheduleTemplateWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity::{get_created_at, get_updated_at};

    let template = match get_schedule_template_by_id(conn, template_id)? {
        Some(t) => t,
        None => return Ok(None),
    };

    let created_at = get_created_at(conn, "schedule_templates", template_id)?;
    let updated_at = get_updated_at(conn, "schedule_templates", template_id)?;

    Ok(Some(ScheduleTemplateWithTimestamps {
        id: template.id,
        site_id: template.site_id,
        name: template.name,
        description: template.description,
        is_default: template.is_default,
        is_active: template.is_active,
        created_at,
        updated_at,
    }))
}
