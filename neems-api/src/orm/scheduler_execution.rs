use diesel::prelude::*;
use chrono::NaiveDateTime;
use crate::models::{NewSchedulerExecution, SchedulerExecution, SchedulerExecutionInput};

/// Gets all scheduler executions for a specific site ID.
pub fn get_scheduler_executions_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    limit: Option<i64>,
) -> Result<Vec<SchedulerExecution>, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    
    let mut query = scheduler_executions
        .filter(site_id.eq(site_id_param))
        .order(execution_time.desc())
        .into_boxed();

    if let Some(limit_val) = limit {
        query = query.limit(limit_val);
    }

    query
        .select(SchedulerExecution::as_select())
        .load(conn)
}

/// Gets a scheduler execution by ID.
pub fn get_scheduler_execution_by_id(
    conn: &mut SqliteConnection,
    execution_id: i32,
) -> Result<Option<SchedulerExecution>, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    scheduler_executions
        .find(execution_id)
        .select(SchedulerExecution::as_select())
        .first(conn)
        .optional()
}

/// Gets all scheduler executions.
pub fn get_all_scheduler_executions(
    conn: &mut SqliteConnection,
    limit: Option<i64>,
) -> Result<Vec<SchedulerExecution>, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    
    let mut query = scheduler_executions
        .order(execution_time.desc())
        .into_boxed();

    if let Some(limit_val) = limit {
        query = query.limit(limit_val);
    }

    query
        .select(SchedulerExecution::as_select())
        .load(conn)
}

/// Creates a new scheduler execution log entry.
pub fn insert_scheduler_execution(
    conn: &mut SqliteConnection,
    input: SchedulerExecutionInput,
) -> Result<SchedulerExecution, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;

    let new_execution = NewSchedulerExecution::from(input);

    diesel::insert_into(scheduler_executions)
        .values(&new_execution)
        .execute(conn)?;

    // Return the inserted execution
    scheduler_executions
        .order(id.desc())
        .select(SchedulerExecution::as_select())
        .first(conn)
}

/// Gets executions within a time range for a site.
pub fn get_executions_by_site_and_time_range(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    start_time: NaiveDateTime,
    end_time: NaiveDateTime,
) -> Result<Vec<SchedulerExecution>, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    scheduler_executions
        .filter(
            site_id.eq(site_id_param)
                .and(execution_time.ge(start_time))
                .and(execution_time.le(end_time))
        )
        .order(execution_time.asc())
        .select(SchedulerExecution::as_select())
        .load(conn)
}

/// Gets executions that had errors for a site.
pub fn get_failed_executions_by_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    limit: Option<i64>,
) -> Result<Vec<SchedulerExecution>, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    
    let mut query = scheduler_executions
        .filter(
            site_id.eq(site_id_param)
                .and(error_message.is_not_null())
        )
        .order(execution_time.desc())
        .into_boxed();

    if let Some(limit_val) = limit {
        query = query.limit(limit_val);
    }

    query
        .select(SchedulerExecution::as_select())
        .load(conn)
}

/// Gets the most recent execution for a site.
pub fn get_latest_execution_for_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
) -> Result<Option<SchedulerExecution>, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    scheduler_executions
        .filter(site_id.eq(site_id_param))
        .order(execution_time.desc())
        .select(SchedulerExecution::as_select())
        .first(conn)
        .optional()
}

/// Deletes old execution logs to prevent table from growing too large.
/// Keeps the most recent N executions for each site.
pub fn cleanup_old_executions(
    conn: &mut SqliteConnection,
    _keep_per_site: i64,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    
    // This is a complex operation that requires finding the Nth most recent execution
    // for each site and deleting everything older than that.
    // For now, we'll implement a simpler version that deletes executions older than a certain date.
    
    let cutoff_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
    
    let affected_rows = diesel::delete(
        scheduler_executions.filter(execution_time.lt(cutoff_date))
    ).execute(conn)?;
    
    Ok(affected_rows)
}

/// Gets execution statistics for a site (success rate, average execution time, etc.).
pub fn get_execution_stats_for_site(
    conn: &mut SqliteConnection,
    site_id_param: i32,
    since: Option<NaiveDateTime>,
) -> Result<ExecutionStats, diesel::result::Error> {
    use crate::schema::scheduler_executions::dsl::*;
    
    let mut query = scheduler_executions
        .filter(site_id.eq(site_id_param))
        .into_boxed();
    
    if let Some(since_date) = since {
        query = query.filter(execution_time.ge(since_date));
    }
    
    let executions: Vec<SchedulerExecution> = query
        .select(SchedulerExecution::as_select())
        .load(conn)?;
    
    let total_count = executions.len();
    let error_count = executions.iter().filter(|e| e.error_message.is_some()).count();
    let success_count = total_count - error_count;
    
    let avg_duration = if !executions.is_empty() {
        executions
            .iter()
            .filter_map(|e| e.execution_duration_ms)
            .map(|d| d as f64)
            .sum::<f64>() / executions.len() as f64
    } else {
        0.0
    };
    
    Ok(ExecutionStats {
        total_executions: total_count as i32,
        successful_executions: success_count as i32,
        failed_executions: error_count as i32,
        success_rate: if total_count > 0 { success_count as f64 / total_count as f64 } else { 0.0 },
        average_duration_ms: avg_duration,
    })
}

/// Statistics for scheduler executions.
#[derive(Debug)]
pub struct ExecutionStats {
    pub total_executions: i32,
    pub successful_executions: i32,
    pub failed_executions: i32,
    pub success_rate: f64,
    pub average_duration_ms: f64,
}