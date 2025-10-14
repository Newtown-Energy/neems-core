use diesel::prelude::*;

use crate::models::{NewSchedule, Schedule, ScheduleWithTimestamps};

/// Gets all schedules for a specific site ID.
pub fn get_schedules_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<Schedule>, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;
    schedules
        .filter(site_id.eq(site_id_param))
        .order(schedule_date.desc())
        .select(Schedule::as_select())
        .load(conn)
}

/// Gets all active schedules for a specific site ID.
pub fn get_active_schedules_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<Schedule>, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;
    schedules
        .filter(site_id.eq(site_id_param))
        .filter(is_active.eq(true))
        .order(schedule_date.desc())
        .select(Schedule::as_select())
        .load(conn)
}

/// Gets a schedule for a specific site and date
pub fn get_schedule_by_site_and_date(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    date: chrono::NaiveDate,
) -> Result<Option<Schedule>, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;
    schedules
        .filter(site_id.eq(site_id_param))
        .filter(schedule_date.eq(date))
        .select(Schedule::as_select())
        .first(conn)
        .optional()
}

/// Gets schedules within a date range for a site
pub fn get_schedules_by_site_and_date_range(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
) -> Result<Vec<Schedule>, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;
    schedules
        .filter(site_id.eq(site_id_param))
        .filter(schedule_date.ge(start_date))
        .filter(schedule_date.le(end_date))
        .order(schedule_date.asc())
        .select(Schedule::as_select())
        .load(conn)
}

/// Creates a new schedule in the database
pub fn insert_schedule(
    conn: &mut SqliteConnection,
    new_schedule: NewSchedule,
    acting_user_id: Option<i32>,
) -> Result<Schedule, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;

    diesel::insert_into(schedules)
        .values(&new_schedule)
        .execute(conn)?;

    // Return the inserted schedule
    let schedule = schedules
        .order(id.desc())
        .select(Schedule::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "schedules", schedule.id, "create", user_id);
    }

    Ok(schedule)
}

/// Gets a schedule by its ID.
pub fn get_schedule_by_id(
    conn: &mut SqliteConnection,
    schedule_id: i32,
) -> Result<Option<Schedule>, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;
    schedules
        .filter(id.eq(schedule_id))
        .select(Schedule::as_select())
        .first(conn)
        .optional()
}

/// Updates a schedule in the database
pub fn update_schedule(
    conn: &mut SqliteConnection,
    schedule_id: i32,
    new_template_id: Option<Option<i32>>,
    new_is_custom: Option<bool>,
    new_is_active: Option<bool>,
    acting_user_id: Option<i32>,
) -> Result<Schedule, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;

    // First, get the current schedule to preserve existing values
    let current = schedules
        .filter(id.eq(schedule_id))
        .select(Schedule::as_select())
        .first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(schedules.filter(id.eq(schedule_id)))
        .set((
            template_id.eq(new_template_id.unwrap_or(current.template_id)),
            is_custom.eq(new_is_custom.unwrap_or(current.is_custom)),
            is_active.eq(new_is_active.unwrap_or(current.is_active)),
        ))
        .execute(conn)?;

    let updated = schedules
        .filter(id.eq(schedule_id))
        .select(Schedule::as_select())
        .first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "schedules", schedule_id, "update", user_id);
    }

    Ok(updated)
}

/// Marks a schedule as custom (used when editing a template-based schedule)
pub fn mark_schedule_as_custom(
    conn: &mut SqliteConnection,
    schedule_id: i32,
    acting_user_id: Option<i32>,
) -> Result<Schedule, diesel::result::Error> {
    update_schedule(conn, schedule_id, None, Some(true), None, acting_user_id)
}

/// Deletes a schedule from the database
pub fn delete_schedule(
    conn: &mut SqliteConnection,
    schedule_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::schedules::dsl::*;

    // Update the activity log before deletion
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "schedules", schedule_id, "delete", user_id);
    }

    diesel::delete(schedules.filter(id.eq(schedule_id))).execute(conn)
}

/// Gets a schedule with timestamps from entity activity
pub fn get_schedule_with_timestamps(
    conn: &mut SqliteConnection,
    schedule_id: i32,
) -> Result<Option<ScheduleWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity::{get_created_at, get_updated_at};

    let schedule = match get_schedule_by_id(conn, schedule_id)? {
        Some(s) => s,
        None => return Ok(None),
    };

    let created_at = get_created_at(conn, "schedules", schedule_id)?;
    let updated_at = get_updated_at(conn, "schedules", schedule_id)?;

    Ok(Some(ScheduleWithTimestamps {
        id: schedule.id,
        site_id: schedule.site_id,
        template_id: schedule.template_id,
        schedule_date: schedule.schedule_date,
        is_custom: schedule.is_custom,
        is_active: schedule.is_active,
        created_at,
        updated_at,
    }))
}
