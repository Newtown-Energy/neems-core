use diesel::prelude::*;
use chrono::NaiveDateTime;
use crate::models::{NewSchedulerOverride, SchedulerOverride, SchedulerOverrideInput, UpdateSchedulerOverrideRequest};

/// Gets all scheduler overrides for a specific site ID.
pub fn get_scheduler_overrides_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;
    scheduler_overrides
        .filter(site_id.eq(site_id_param))
        .order(id.asc())
        .select(SchedulerOverride::as_select())
        .load(conn)
}

/// Gets all active scheduler overrides for a specific site ID.
pub fn get_active_scheduler_overrides_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Vec<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;
    scheduler_overrides
        .filter(site_id.eq(site_id_param).and(is_active.eq(true)))
        .order(start_time.asc())
        .select(SchedulerOverride::as_select())
        .load(conn)
}

/// Gets a scheduler override by ID.
pub fn get_scheduler_override_by_id(
    conn: &mut SqliteConnection,
    override_id: i32,
) -> Result<Option<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;
    scheduler_overrides
        .find(override_id)
        .select(SchedulerOverride::as_select())
        .first(conn)
        .optional()
}

/// Gets all scheduler overrides.
pub fn get_all_scheduler_overrides(
    conn: &mut SqliteConnection,
) -> Result<Vec<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;
    scheduler_overrides
        .order(id.asc())
        .select(SchedulerOverride::as_select())
        .load(conn)
}

/// Creates a new scheduler override in the database.
pub fn insert_scheduler_override(
    conn: &mut SqliteConnection,
    input: SchedulerOverrideInput,
    created_by_user_id: i32,
    acting_user_id: Option<i32>,
) -> Result<SchedulerOverride, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;

    let mut new_override = NewSchedulerOverride::from(input);
    new_override.created_by = created_by_user_id;

    diesel::insert_into(scheduler_overrides)
        .values(&new_override)
        .execute(conn)?;

    // Return the inserted override
    let override_record = scheduler_overrides
        .order(id.desc())
        .select(SchedulerOverride::as_select())
        .first(conn)?;
    
    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "scheduler_overrides", override_record.id, "create", user_id);
    }
    
    Ok(override_record)
}

/// Updates a scheduler override.
pub fn update_scheduler_override(
    conn: &mut SqliteConnection,
    override_id: i32,
    update_request: UpdateSchedulerOverrideRequest,
    acting_user_id: Option<i32>,
) -> Result<SchedulerOverride, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;

    // Build the update query dynamically based on what fields are provided
    let mut updates = vec![];
    
    if let Some(new_state) = update_request.state {
        updates.push(("state", new_state));
    }
    if let Some(new_start) = update_request.start_time {
        // Handle start_time update
        diesel::update(scheduler_overrides.find(override_id))
            .set(start_time.eq(new_start))
            .execute(conn)?;
    }
    if let Some(new_end) = update_request.end_time {
        // Handle end_time update
        diesel::update(scheduler_overrides.find(override_id))
            .set(end_time.eq(new_end))
            .execute(conn)?;
    }
    if let Some(new_reason) = update_request.reason {
        updates.push(("reason", new_reason));
    }
    if let Some(new_active) = update_request.is_active {
        diesel::update(scheduler_overrides.find(override_id))
            .set(is_active.eq(new_active))
            .execute(conn)?;
    }

    // Apply string field updates
    for (field, value) in updates {
        match field {
            "state" => {
                diesel::update(scheduler_overrides.find(override_id))
                    .set(state.eq(value))
                    .execute(conn)?;
            }
            "reason" => {
                diesel::update(scheduler_overrides.find(override_id))
                    .set(reason.eq(Some(value)))
                    .execute(conn)?;
            }
            _ => {}
        }
    }

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "scheduler_overrides", override_id, "update", user_id);
    }

    // Return the updated override
    scheduler_overrides
        .find(override_id)
        .select(SchedulerOverride::as_select())
        .first(conn)
}

/// Deletes a scheduler override.
pub fn delete_scheduler_override(
    conn: &mut SqliteConnection,
    override_id: i32,
    acting_user_id: Option<i32>,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;

    // Update the trigger-created activity entry with user information before deletion
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "scheduler_overrides", override_id, "delete", user_id);
    }

    let affected_rows = diesel::delete(scheduler_overrides.find(override_id)).execute(conn)?;
    Ok(affected_rows > 0)
}

/// Gets active overrides for a site at a specific datetime.
pub fn get_active_overrides_at_datetime(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    datetime: NaiveDateTime,
) -> Result<Vec<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;
    scheduler_overrides
        .filter(
            site_id.eq(site_id_param)
                .and(is_active.eq(true))
                .and(start_time.le(datetime))
                .and(end_time.gt(datetime))
        )
        .order(start_time.asc())
        .select(SchedulerOverride::as_select())
        .load(conn)
}

/// Gets the most recent active override for a site at a specific datetime.
/// If multiple overrides are active, returns the one that started most recently.
pub fn get_current_override_for_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    datetime: NaiveDateTime,
) -> Result<Option<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;
    scheduler_overrides
        .filter(
            site_id.eq(site_id_param)
                .and(is_active.eq(true))
                .and(start_time.le(datetime))
                .and(end_time.gt(datetime))
        )
        .order(start_time.desc())
        .select(SchedulerOverride::as_select())
        .first(conn)
        .optional()
}

/// Gets upcoming overrides for a site (starting after the specified datetime).
pub fn get_upcoming_overrides_for_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    from_datetime: NaiveDateTime,
    limit: Option<i64>,
) -> Result<Vec<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;
    
    let mut query = scheduler_overrides
        .filter(
            site_id.eq(site_id_param)
                .and(is_active.eq(true))
                .and(start_time.gt(from_datetime))
        )
        .order(start_time.asc())
        .into_boxed();

    if let Some(limit_val) = limit {
        query = query.limit(limit_val);
    }

    query
        .select(SchedulerOverride::as_select())
        .load(conn)
}

/// Expires overrides that have ended (sets is_active = false for overrides where end_time < now).
pub fn expire_ended_overrides(
    conn: &mut SqliteConnection,
    current_datetime: NaiveDateTime,
    _acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;

    let affected_rows = diesel::update(
        scheduler_overrides
            .filter(is_active.eq(true).and(end_time.lt(current_datetime)))
    )
    .set(is_active.eq(false))
    .execute(conn)?;

    // Note: This is a bulk operation, so we can't easily track individual override updates
    // in the activity log. In a production system, you might want to handle this differently.

    Ok(affected_rows)
}

/// Validates that an override doesn't overlap with existing active overrides for the same site.
pub fn check_override_conflicts(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    start_datetime: NaiveDateTime,
    end_datetime: NaiveDateTime,
    exclude_override_id: Option<i32>,
) -> Result<Vec<SchedulerOverride>, diesel::result::Error> {
    use crate::schema::scheduler_overrides::dsl::*;

    let mut query = scheduler_overrides
        .filter(
            site_id.eq(site_id_param)
                .and(is_active.eq(true))
                .and(
                    // Check for overlaps: new start is before existing end AND new end is after existing start
                    start_time.lt(end_datetime).and(end_time.gt(start_datetime))
                )
        )
        .into_boxed();

    if let Some(exclude_id) = exclude_override_id {
        query = query.filter(id.ne(exclude_id));
    }

    query
        .select(SchedulerOverride::as_select())
        .load(conn)
}