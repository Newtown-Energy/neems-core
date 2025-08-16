use chrono::{NaiveDateTime, Timelike, Utc};
use clap::{Args, Subcommand};
use neems_api::scheduler_executor::{ScriptExecutor, SiteData};

#[derive(Debug, Subcommand)]
pub enum SchedulerCommand {
    /// Execute a scheduler script with a given timestamp
    Execute(ExecuteArgs),
    /// Test the default scheduler script with a timestamp
    Test(TestArgs),
    /// Show the default scheduler script
    ShowDefault,
}

#[derive(Debug, Args)]
pub struct ExecuteArgs {
    /// Timestamp to use for script execution (ISO 8601 format: YYYY-MM-DDTHH:MM:SS)
    #[arg(short, long)]
    pub timestamp: String,
    
    /// Site ID for the execution context
    #[arg(short, long)]
    pub site_id: i32,
    
    /// Site name for the execution context
    #[arg(short = 'n', long, default_value = "Test Site")]
    pub site_name: String,
    
    /// Company ID for the execution context
    #[arg(short, long, default_value_t = 1)]
    pub company_id: i32,
    
    /// Custom Lua script content (if not provided, uses default script)
    #[arg(long)]
    pub script: Option<String>,
}

#[derive(Debug, Args)]
pub struct TestArgs {
    /// Timestamp to use for script execution (ISO 8601 format: YYYY-MM-DDTHH:MM:SS)
    #[arg(short, long)]
    pub timestamp: String,
    
    /// Site ID for the execution context
    #[arg(short, long, default_value_t = 1)]
    pub site_id: i32,
    
    /// Site name for the execution context
    #[arg(short = 'n', long, default_value = "Test Site")]
    pub site_name: String,
    
    /// Company ID for the execution context
    #[arg(short, long, default_value_t = 1)]
    pub company_id: i32,
}

pub fn handle_scheduler_command(command: SchedulerCommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        SchedulerCommand::Execute(args) => execute_script(args),
        SchedulerCommand::Test(args) => test_default_script(args),
        SchedulerCommand::ShowDefault => show_default_script(),
    }
}

fn execute_script(args: ExecuteArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Parse the timestamp
    let datetime = parse_timestamp(&args.timestamp)?;
    
    // Create site data
    let site_data = SiteData {
        site_id: args.site_id,
        name: args.site_name,
        company_id: args.company_id,
        latitude: None,
        longitude: None,
    };
    
    // Use provided script or default
    let script_content = args.script.as_deref().unwrap_or_else(|| ScriptExecutor::get_default_script());
    
    // Execute the script
    let result = ScriptExecutor::execute_simple_script(datetime, &site_data, script_content);
    
    // Display results
    println!("Script Execution Results:");
    println!("  State: {}", result.state.as_str());
    println!("  Execution Time: {}ms", result.execution_time_ms);
    
    if let Some(error) = &result.error {
        println!("  Error: {}", error);
        return Err(error.clone().into());
    } else {
        println!("  Status: Success");
    }
    
    Ok(())
}

fn test_default_script(args: TestArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Parse the timestamp
    let datetime = parse_timestamp(&args.timestamp)?;
    
    // Create site data
    let site_data = SiteData {
        site_id: args.site_id,
        name: args.site_name,
        company_id: args.company_id,
        latitude: None,
        longitude: None,
    };
    
    // Use default script
    let script_content = ScriptExecutor::get_default_script();
    
    println!("Testing Default Scheduler Script");
    println!("Script Content:");
    println!("{}", script_content);
    println!();
    
    // Execute the script
    let result = ScriptExecutor::execute_simple_script(datetime, &site_data, script_content);
    
    // Display results
    println!("Execution Results:");
    println!("  Timestamp: {}", datetime.format("%Y-%m-%d %H:%M:%S"));
    println!("  Hour: {}", datetime.hour());
    println!("  State: {}", result.state.as_str());
    println!("  Execution Time: {}ms", result.execution_time_ms);
    
    if let Some(error) = &result.error {
        println!("  Error: {}", error);
        return Err(error.clone().into());
    } else {
        println!("  Status: Success");
    }
    
    Ok(())
}

fn show_default_script() -> Result<(), Box<dyn std::error::Error>> {
    println!("Default NEEMS Scheduler Script:");
    println!("{}", ScriptExecutor::get_default_script());
    Ok(())
}

fn parse_timestamp(timestamp_str: &str) -> Result<NaiveDateTime, Box<dyn std::error::Error>> {
    // Try parsing with different formats
    let formats = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];
    
    for format in &formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(timestamp_str, format) {
            return Ok(dt);
        }
    }
    
    // If none of the formats work, try parsing as "now"
    if timestamp_str.to_lowercase() == "now" {
        return Ok(Utc::now().naive_utc());
    }
    
    Err(format!("Unable to parse timestamp '{}'. Use format: YYYY-MM-DDTHH:MM:SS or 'now'", timestamp_str).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        // Test ISO format
        let result = parse_timestamp("2023-12-25T14:30:00");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2023);
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 25);
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
        
        // Test space-separated format
        let result = parse_timestamp("2023-12-25 14:30:00");
        assert!(result.is_ok());
        
        // Test "now"
        let result = parse_timestamp("now");
        assert!(result.is_ok());
        
        // Test invalid format
        let result = parse_timestamp("invalid");
        assert!(result.is_err());
    }
}