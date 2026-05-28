//! NEEMS Administrative CLI Utility
//!
//! This is a command-line interface for administrative management of a
//! neems-api instance's SQLite database. The utility provides comprehensive
//! database management capabilities including user, company, and site
//! management, as well as system operations.
//!
//! The CLI leverages the ORM functions located in @neems-api/src/orm/ for all
//! database manipulations, ensuring consistent data access patterns and
//! maintaining referential integrity across operations.
//!
//! Key Features:
//! - User management (create, list, edit, remove, password changes)
//! - Company management (create, list, edit, remove with cascading deletes)
//! - Site management (create, list, edit, remove)
//! - Device management (create, list, edit, remove with unique constraints)
//! - Search functionality with regex and fixed-string support
//! - Secure password prompting without echo
//! - Cascading deletes to maintain data consistency
//! - Interactive confirmation prompts for destructive operations
//!
//! For detailed usage information and available commands, run with --help.

mod admin_cli {
    pub mod company_commands;
    pub mod device_commands;
    pub mod role_commands;
    pub mod site_commands;
    pub mod user_commands;
    pub mod utils;
}

use admin_cli::{
    company_commands::{CompanyAction, handle_company_command_with_conn},
    device_commands::{DeviceAction, handle_device_command_with_conn},
    role_commands::{RoleAction, handle_role_command_with_conn},
    site_commands::{SiteAction, handle_site_command_with_conn},
    user_commands::{UserAction, handle_user_command_with_conn},
    utils::{establish_connection, get_or_create_admin_user},
};
use clap::{Parser, Subcommand};
use serde::Deserialize;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser)]
#[command(name = "neems-admin")]
#[command(about = "Administrative CLI for NEEMS database management")]
#[command(version)]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Show extended version information
    #[arg(long, action = clap::ArgAction::SetTrue)]
    version_info: bool,
}

#[derive(Subcommand)]
enum Commands {
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    Company {
        #[command(subcommand)]
        action: CompanyAction,
    },
    Site {
        #[command(subcommand)]
        action: SiteAction,
    },
    Device {
        #[command(subcommand)]
        action: DeviceAction,
    },
    Role {
        #[command(subcommand)]
        action: RoleAction,
    },
    #[command(about = "Future: Non-database administrative commands")]
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
}

#[derive(Subcommand)]
enum SystemAction {
    #[command(about = "Display system status")]
    Status,
    #[command(about = "Run maintenance tasks")]
    Maintenance,
}

#[derive(Deserialize)]
struct ApiStatus {
    status: String,
    version: String,
    built: String,
    git_commit: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle --version-info flag
    if cli.version_info {
        println!("neems-admin {}", built_info::PKG_VERSION);
        println!("Built: {}", built_info::BUILT_TIME_UTC);
        if let Some(commit) = built_info::GIT_COMMIT_HASH {
            println!("Git commit: {}", commit);
        }
        return Ok(());
    }

    match cli.command {
        Some(Commands::User { action }) => handle_user_command(action)?,
        Some(Commands::Company { action }) => handle_company_command(action)?,
        Some(Commands::Site { action }) => handle_site_command(action)?,
        Some(Commands::Device { action }) => handle_device_command(action)?,
        Some(Commands::Role { action }) => handle_role_command(action)?,
        Some(Commands::System { action }) => handle_system_command(action).await?,
        None => {
            eprintln!("No command provided. Use --help for usage information.");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn handle_user_command(action: UserAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    let admin_user_id = get_or_create_admin_user(&mut conn)?;
    handle_user_command_with_conn(&mut conn, action, admin_user_id)
}

fn handle_company_command(action: CompanyAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    let admin_user_id = get_or_create_admin_user(&mut conn)?;
    handle_company_command_with_conn(&mut conn, action, admin_user_id)
}

fn handle_site_command(action: SiteAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    let admin_user_id = get_or_create_admin_user(&mut conn)?;
    handle_site_command_with_conn(&mut conn, action, admin_user_id)
}

fn handle_device_command(action: DeviceAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    let admin_user_id = get_or_create_admin_user(&mut conn)?;
    handle_device_command_with_conn(&mut conn, action, admin_user_id)
}

fn handle_role_command(action: RoleAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    let admin_user_id = get_or_create_admin_user(&mut conn)?;
    handle_role_command_with_conn(&mut conn, action, admin_user_id)
}

async fn handle_system_command(action: SystemAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SystemAction::Status => {
            println!("System Status: OK");
            println!("Database: Connected");

            // Try to get API server status
            match get_api_status().await {
                Ok(api_status) => {
                    println!("API Server Status: {}", api_status.status);
                    println!("API Version: {}", api_status.version);
                    println!("API Built: {}", api_status.built);
                    if let Some(commit) = api_status.git_commit {
                        println!("API Git commit: {}", commit);
                    }
                }
                Err(e) => {
                    println!("API Server: Not reachable ({})", e);
                }
            }
        }
        SystemAction::Maintenance => {
            println!("Running maintenance tasks...");
            // TODO: Implement maintenance tasks
        }
    }

    Ok(())
}

async fn get_api_status() -> Result<ApiStatus, Box<dyn std::error::Error>> {
    // Default to localhost:8000, could be made configurable
    let url = "http://localhost:8000/api/1/status";
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    let api_status: ApiStatus = response.json().await?;
    Ok(api_status)
}
