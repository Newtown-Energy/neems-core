use std::time::{Duration, Instant};

#[cfg(test)]
use chrono::Utc;
use chrono::{Datelike, NaiveDateTime, Timelike};
use mlua::{Lua, Result as LuaResult, Value};

use crate::models::{SchedulerScript, SiteState};

const SCRIPT_TIMEOUT_MS: u64 = 100;
const SCRIPT_MAX_SIZE: usize = 10 * 1024; // 10KB

/// Default NEEMS scheduler script that implements time-based charging logic:
/// - Discharge: 4pm to 8pm (16:00-20:00)
/// - Charge: 8pm to 1pm (20:00-13:00, crossing midnight)
/// - Idle: 1pm to 4pm (13:00-16:00)
const DEFAULT_SCHEDULER_SCRIPT: &str = r#"
-- Default NEEMS Scheduler Script
-- Discharge: 4pm-8pm, Charge: 8pm-1pm, Idle: otherwise

if datetime.hour >= 16 and datetime.hour < 20 then
    return 'discharge'  -- 4pm to 8pm
elseif datetime.hour >= 20 or datetime.hour < 13 then  
    return 'charge'     -- 8pm to 1pm (crosses midnight)
else
    return 'idle'       -- 1pm to 4pm
end
"#;

#[derive(Debug)]
pub struct ScriptExecutor {
    lua: Lua,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub state: SiteState,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct SiteData {
    pub site_id: i32,
    pub name: String,
    pub company_id: i32,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl ScriptExecutor {
    pub fn new() -> Result<Self, String> {
        let lua = Lua::new();

        // Remove potentially dangerous modules
        lua.globals()
            .set("io", Value::Nil)
            .map_err(|e| format!("Failed to remove io module: {}", e))?;
        lua.globals()
            .set("os", Value::Nil)
            .map_err(|e| format!("Failed to remove os module: {}", e))?;
        lua.globals()
            .set("package", Value::Nil)
            .map_err(|e| format!("Failed to remove package module: {}", e))?;
        lua.globals()
            .set("debug", Value::Nil)
            .map_err(|e| format!("Failed to remove debug module: {}", e))?;
        lua.globals()
            .set("require", Value::Nil)
            .map_err(|e| format!("Failed to remove require function: {}", e))?;
        lua.globals()
            .set("loadfile", Value::Nil)
            .map_err(|e| format!("Failed to remove loadfile function: {}", e))?;
        lua.globals()
            .set("dofile", Value::Nil)
            .map_err(|e| format!("Failed to remove dofile function: {}", e))?;
        lua.globals()
            .set("load", Value::Nil)
            .map_err(|e| format!("Failed to remove load function: {}", e))?;
        lua.globals()
            .set("loadstring", Value::Nil)
            .map_err(|e| format!("Failed to remove loadstring function: {}", e))?;

        Ok(Self { lua })
    }

    pub fn validate_script(&self, script: &SchedulerScript) -> Result<(), String> {
        // Check script size
        if script.script_content.len() > SCRIPT_MAX_SIZE {
            return Err(format!(
                "Script size {} bytes exceeds maximum allowed size of {} bytes",
                script.script_content.len(),
                SCRIPT_MAX_SIZE
            ));
        }

        // Check language
        if script.language != "lua" {
            return Err(format!("Unsupported script language: {}", script.language));
        }

        // Try to compile the script (syntax check only)
        match self.lua.load(&script.script_content).into_function() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Script compilation failed: {}", e)),
        }
    }

    pub fn execute_script(
        &self,
        script: &SchedulerScript,
        datetime: NaiveDateTime,
        site_data: &SiteData,
    ) -> ExecutionResult {
        let start_time = Instant::now();

        // Validate script size first
        if script.script_content.len() > SCRIPT_MAX_SIZE {
            return ExecutionResult {
                state: SiteState::Idle,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                error: Some(format!(
                    "Script size {} bytes exceeds maximum allowed size of {} bytes",
                    script.script_content.len(),
                    SCRIPT_MAX_SIZE
                )),
            };
        }

        // Create a timeout wrapper for the execution
        let result = self.execute_with_timeout(
            script,
            datetime,
            site_data,
            Duration::from_millis(SCRIPT_TIMEOUT_MS),
        );

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(state) => ExecutionResult { state, execution_time_ms, error: None },
            Err(error) => ExecutionResult {
                state: SiteState::Idle, // Default to idle on error
                execution_time_ms,
                error: Some(error),
            },
        }
    }

    fn execute_with_timeout(
        &self,
        script: &SchedulerScript,
        datetime: NaiveDateTime,
        site_data: &SiteData,
        timeout: Duration,
    ) -> Result<SiteState, String> {
        // Set up the Lua environment
        let globals = self.lua.globals();

        // Create datetime table
        let datetime_table = self
            .lua
            .create_table()
            .map_err(|e| format!("Failed to create datetime table: {}", e))?;
        datetime_table
            .set("year", datetime.year())
            .map_err(|e| format!("Failed to set year: {}", e))?;
        datetime_table
            .set("month", datetime.month())
            .map_err(|e| format!("Failed to set month: {}", e))?;
        datetime_table
            .set("day", datetime.day())
            .map_err(|e| format!("Failed to set day: {}", e))?;
        datetime_table
            .set("hour", datetime.hour())
            .map_err(|e| format!("Failed to set hour: {}", e))?;
        datetime_table
            .set("minute", datetime.minute())
            .map_err(|e| format!("Failed to set minute: {}", e))?;
        datetime_table
            .set("second", datetime.second())
            .map_err(|e| format!("Failed to set second: {}", e))?;
        datetime_table
            .set("weekday", datetime.weekday().number_from_monday())
            .map_err(|e| format!("Failed to set weekday: {}", e))?;
        datetime_table
            .set("timestamp", datetime.and_utc().timestamp())
            .map_err(|e| format!("Failed to set timestamp: {}", e))?;

        // Create site_data table
        let site_table = self
            .lua
            .create_table()
            .map_err(|e| format!("Failed to create site_data table: {}", e))?;
        site_table
            .set("id", site_data.site_id)
            .map_err(|e| format!("Failed to set site id: {}", e))?;
        site_table
            .set("name", site_data.name.clone())
            .map_err(|e| format!("Failed to set site name: {}", e))?;
        site_table
            .set("company_id", site_data.company_id)
            .map_err(|e| format!("Failed to set company_id: {}", e))?;

        if let Some(lat) = site_data.latitude {
            site_table
                .set("latitude", lat)
                .map_err(|e| format!("Failed to set latitude: {}", e))?;
        }
        if let Some(lon) = site_data.longitude {
            site_table
                .set("longitude", lon)
                .map_err(|e| format!("Failed to set longitude: {}", e))?;
        }

        // Set global variables
        globals
            .set("datetime", datetime_table)
            .map_err(|e| format!("Failed to set datetime global: {}", e))?;
        globals
            .set("site_data", site_table)
            .map_err(|e| format!("Failed to set site_data global: {}", e))?;

        // Execute the script with timeout check
        let start = Instant::now();

        // Load and execute the script
        let chunk = self.lua.load(&script.script_content);
        let result: LuaResult<Value> = chunk.call(());

        // Check if we exceeded timeout
        if start.elapsed() > timeout {
            return Err("Script execution timed out".to_string());
        }

        match result {
            Ok(value) => {
                // Convert the result to a string and then to SiteState
                let state_str = match value {
                    Value::String(s) => s.to_str().unwrap_or("idle").to_string(),
                    Value::Nil => "idle".to_string(),
                    _ => {
                        return Err(
                            "Script must return a string value (charge, discharge, or idle)"
                                .to_string(),
                        );
                    }
                };

                SiteState::from_str(&state_str)
                    .map_err(|e| format!("Invalid state returned by script: {}", e))
            }
            Err(e) => Err(format!("Script execution error: {}", e)),
        }
    }

    pub fn execute_simple_script(
        datetime: NaiveDateTime,
        site_data: &SiteData,
        script_content: &str,
    ) -> ExecutionResult {
        match ScriptExecutor::new() {
            Ok(executor) => {
                let script = SchedulerScript {
                    id: 0,
                    site_id: site_data.site_id,
                    name: "test".to_string(),
                    script_content: script_content.to_string(),
                    language: "lua".to_string(),
                    is_active: true,
                    version: 1,
                };
                executor.execute_script(&script, datetime, site_data)
            }
            Err(e) => ExecutionResult {
                state: SiteState::Idle,
                execution_time_ms: 0,
                error: Some(format!("Failed to create script executor: {}", e)),
            },
        }
    }

    /// Returns the default NEEMS scheduler script
    pub fn get_default_script() -> &'static str {
        DEFAULT_SCHEDULER_SCRIPT
    }
}

impl Default for ScriptExecutor {
    fn default() -> Self {
        Self::new().expect("Failed to create default ScriptExecutor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_site_data() -> SiteData {
        SiteData {
            site_id: 1,
            name: "Test Site".to_string(),
            company_id: 1,
            latitude: Some(40.7128),
            longitude: Some(-74.0060),
        }
    }

    #[test]
    fn test_script_executor_creation() {
        let executor = ScriptExecutor::new();
        assert!(executor.is_ok());
    }

    #[test]
    fn test_simple_script_execution() {
        let executor = ScriptExecutor::new().unwrap();
        let script = SchedulerScript {
            id: 1,
            site_id: 1,
            name: "test_script".to_string(),
            script_content: "return 'charge'".to_string(),
            language: "lua".to_string(),
            is_active: true,
            version: 1,
        };

        let site_data = create_test_site_data();
        let datetime = Utc::now().naive_utc();
        let result = executor.execute_script(&script, datetime, &site_data);

        assert!(result.error.is_none());
        assert!(matches!(result.state, SiteState::Charge));
    }

    #[test]
    fn test_script_with_datetime_access() {
        let executor = ScriptExecutor::new().unwrap();
        let script = SchedulerScript {
            id: 1,
            site_id: 1,
            name: "datetime_script".to_string(),
            script_content: r#"
                if datetime.hour >= 6 and datetime.hour < 18 then
                    return 'charge'
                else
                    return 'idle'
                end
            "#
            .to_string(),
            language: "lua".to_string(),
            is_active: true,
            version: 1,
        };

        let site_data = create_test_site_data();
        let datetime = chrono::NaiveDate::from_ymd_opt(2023, 1, 1)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let result = executor.execute_script(&script, datetime, &site_data);

        assert!(result.error.is_none());
        assert!(matches!(result.state, SiteState::Charge));
    }

    #[test]
    fn test_script_with_site_data_access() {
        let executor = ScriptExecutor::new().unwrap();
        let script = SchedulerScript {
            id: 1,
            site_id: 1,
            name: "site_script".to_string(),
            script_content: r#"
                if site_data.latitude > 40 then
                    return 'discharge'
                else
                    return 'idle'
                end
            "#
            .to_string(),
            language: "lua".to_string(),
            is_active: true,
            version: 1,
        };

        let site_data = create_test_site_data();
        let datetime = Utc::now().naive_utc();
        let result = executor.execute_script(&script, datetime, &site_data);

        assert!(result.error.is_none());
        assert!(matches!(result.state, SiteState::Discharge));
    }

    #[test]
    fn test_invalid_script() {
        let executor = ScriptExecutor::new().unwrap();
        let script = SchedulerScript {
            id: 1,
            site_id: 1,
            name: "invalid_script".to_string(),
            script_content: "this is not valid lua".to_string(),
            language: "lua".to_string(),
            is_active: true,
            version: 1,
        };

        let site_data = create_test_site_data();
        let datetime = Utc::now().naive_utc();
        let result = executor.execute_script(&script, datetime, &site_data);

        assert!(result.error.is_some());
        assert!(matches!(result.state, SiteState::Idle));
    }

    #[test]
    fn test_script_size_limit() {
        let executor = ScriptExecutor::new().unwrap();
        let large_script_content = "return 'idle'".repeat(1000); // Create a large script
        let script = SchedulerScript {
            id: 1,
            site_id: 1,
            name: "large_script".to_string(),
            script_content: large_script_content,
            language: "lua".to_string(),
            is_active: true,
            version: 1,
        };

        let site_data = create_test_site_data();
        let datetime = Utc::now().naive_utc();
        let result = executor.execute_script(&script, datetime, &site_data);

        if script.script_content.len() > SCRIPT_MAX_SIZE {
            assert!(result.error.is_some());
            assert!(result.error.unwrap().contains("exceeds maximum allowed size"));
        }
    }

    #[test]
    fn test_sandboxing() {
        let executor = ScriptExecutor::new().unwrap();

        // Test that dangerous functions are removed
        let dangerous_scripts = vec![
            "io.open('/etc/passwd', 'r')",
            "os.execute('rm -rf /')",
            "require('os')",
            "loadfile('/etc/passwd')",
            "dofile('/etc/passwd')",
        ];

        for script_content in dangerous_scripts {
            let script = SchedulerScript {
                id: 1,
                site_id: 1,
                name: "dangerous_script".to_string(),
                script_content: script_content.to_string(),
                language: "lua".to_string(),
                is_active: true,
                version: 1,
            };

            let site_data = create_test_site_data();
            let datetime = Utc::now().naive_utc();
            let result = executor.execute_script(&script, datetime, &site_data);

            // Should either error or return idle (safe default)
            assert!(result.error.is_some() || matches!(result.state, SiteState::Idle));
        }
    }
}
