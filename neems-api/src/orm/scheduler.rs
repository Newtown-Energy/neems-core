use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;

use crate::{
    models::{SchedulerExecutionInput, SchedulerScript, SiteState},
    orm::{
        scheduler_execution::insert_scheduler_execution,
        scheduler_override::get_current_override_for_site,
        scheduler_script::get_latest_active_script_for_site,
    },
    scheduler_executor::{ExecutionResult, ScriptExecutor, SiteData},
};

/// Main scheduler service that determines site state.
pub struct SchedulerService {
    executor: ScriptExecutor,
}

impl SchedulerService {
    pub fn new() -> Result<Self, String> {
        Ok(Self { executor: ScriptExecutor::new()? })
    }

    /// Gets the current state for a site at a specific datetime.
    /// Resolution logic:
    /// 1. Check for active overrides first (within start_time and end_time)
    /// 2. If no override, execute active script for the site
    /// 3. If no script, return default state (idle)
    pub fn get_site_state(
        &self,
        conn: &mut SqliteConnection,
        site_id: i32,
        datetime: NaiveDateTime,
    ) -> Result<SiteStateResult, String> {
        // Step 1: Check for active overrides
        match get_current_override_for_site(conn, site_id, datetime) {
            Ok(Some(override_record)) => {
                let state = SiteState::from_str(&override_record.state)
                    .map_err(|e| format!("Invalid override state: {}", e))?;
                return Ok(SiteStateResult {
                    state,
                    source: StateSource::Override(override_record.id),
                    execution_time_ms: 0,
                    error: None,
                });
            }
            Ok(None) => {
                // No active override, continue to script execution
            }
            Err(e) => {
                return Err(format!("Database error checking overrides: {}", e));
            }
        }

        // Step 2: Check for active script
        match get_latest_active_script_for_site(conn, site_id) {
            Ok(Some(script)) => {
                // We need site data for script execution
                let site_data = self.get_site_data(conn, site_id)?;
                let execution_result = self.executor.execute_script(&script, datetime, &site_data);

                Ok(SiteStateResult {
                    state: execution_result.state,
                    source: StateSource::Script(script.id),
                    execution_time_ms: execution_result.execution_time_ms,
                    error: execution_result.error,
                })
            }
            Ok(None) => {
                // No active script, return default state
                Ok(SiteStateResult {
                    state: SiteState::Idle,
                    source: StateSource::Default,
                    execution_time_ms: 0,
                    error: None,
                })
            }
            Err(e) => Err(format!("Database error checking scripts: {}", e)),
        }
    }

    /// Executes the scheduler for a site and logs the result.
    pub fn execute_scheduler_for_site(
        &self,
        conn: &mut SqliteConnection,
        site_id: i32,
        datetime: NaiveDateTime,
    ) -> Result<SiteStateResult, String> {
        let result = self.get_site_state(conn, site_id, datetime)?;

        // Log the execution
        let execution_input = SchedulerExecutionInput {
            site_id,
            script_id: match result.source {
                StateSource::Script(id) => Some(id),
                _ => None,
            },
            override_id: match result.source {
                StateSource::Override(id) => Some(id),
                _ => None,
            },
            execution_time: Some(datetime),
            state_result: result.state.as_str().to_string(),
            execution_duration_ms: Some(result.execution_time_ms as i32),
            error_message: result.error.clone(),
        };

        if let Err(e) = insert_scheduler_execution(conn, execution_input) {
            eprintln!("Failed to log scheduler execution: {}", e);
            // Don't fail the whole operation if logging fails
        }

        Ok(result)
    }

    /// Validates a script by attempting to compile and run it with sample data.
    pub fn validate_script(
        &self,
        conn: &mut SqliteConnection,
        script: &SchedulerScript,
        site_id: i32,
    ) -> Result<ValidationResult, String> {
        // First validate syntax
        if let Err(e) = self.executor.validate_script(script) {
            return Ok(ValidationResult {
                is_valid: false,
                error: Some(e),
                test_execution: None,
            });
        }

        // Run a test execution with current datetime and site data
        let site_data = self.get_site_data(conn, site_id)?;
        let current_time = Utc::now().naive_utc();
        let test_result = self.executor.execute_script(script, current_time, &site_data);

        Ok(ValidationResult {
            is_valid: test_result.error.is_none(),
            error: test_result.error.clone(),
            test_execution: Some(test_result),
        })
    }

    /// Gets site data needed for script execution.
    fn get_site_data(&self, conn: &mut SqliteConnection, site_id: i32) -> Result<SiteData, String> {
        use crate::schema::sites::dsl::*;

        let site = sites
            .find(site_id)
            .select((id, name, company_id, latitude, longitude))
            .first::<(i32, String, i32, f64, f64)>(conn)
            .map_err(|e| format!("Failed to get site data: {}", e))?;

        Ok(SiteData {
            site_id: site.0,
            name: site.1,
            company_id: site.2,
            latitude: Some(site.3),
            longitude: Some(site.4),
        })
    }
}

impl Default for SchedulerService {
    fn default() -> Self {
        Self::new().expect("Failed to create default SchedulerService")
    }
}

/// Result of determining site state.
#[derive(Debug, Clone)]
pub struct SiteStateResult {
    pub state: SiteState,
    pub source: StateSource,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

/// Source of the state determination.
#[derive(Debug, Clone)]
pub enum StateSource {
    /// State came from an active override
    Override(i32),
    /// State came from executing a script
    Script(i32),
    /// Default state (no script or override)
    Default,
}

/// Result of script validation.
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub error: Option<String>,
    pub test_execution: Option<ExecutionResult>,
}

/// Convenience function to get site state without creating a service instance.
pub fn get_site_state_at_datetime(
    conn: &mut SqliteConnection,
    site_id: i32,
    datetime: NaiveDateTime,
) -> Result<SiteStateResult, String> {
    let service = SchedulerService::new()?;
    service.get_site_state(conn, site_id, datetime)
}

/// Convenience function to execute scheduler for a site without creating a
/// service instance.
pub fn execute_scheduler_for_site(
    conn: &mut SqliteConnection,
    site_id: i32,
    datetime: Option<NaiveDateTime>,
) -> Result<SiteStateResult, String> {
    let service = SchedulerService::new()?;
    let execution_time = datetime.unwrap_or_else(|| Utc::now().naive_utc());
    service.execute_scheduler_for_site(conn, site_id, execution_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "test-staging")]
    use crate::orm::testing::setup_test_db;

    #[test]
    fn test_scheduler_service_creation() {
        let service = SchedulerService::new();
        assert!(service.is_ok());
    }

    #[cfg(feature = "test-staging")]
    #[test]
    fn test_default_state() {
        let mut conn = setup_test_db();
        let service = SchedulerService::new().unwrap();

        // Test with a non-existent site should return default idle state
        let datetime = Utc::now().naive_utc();
        let result = service.get_site_state(&mut conn, 999, datetime);

        // Should return default idle state when no overrides or scripts exist
        assert!(result.is_ok());
        let state_result = result.unwrap();
        assert_eq!(state_result.state, SiteState::Idle);
        assert!(matches!(state_result.source, StateSource::Default));
        assert_eq!(state_result.execution_time_ms, 0);
        assert!(state_result.error.is_none());
    }
}
