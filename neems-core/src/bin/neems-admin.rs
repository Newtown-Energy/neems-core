/*! 
 * NEEMS Administrative CLI Utility
 * 
 * This is a command-line interface for administrative management of a neems-core 
 * instance's SQLite database. The utility provides comprehensive database management
 * capabilities including user, company, and site management, as well as system operations.
 * 
 * The CLI leverages the ORM functions located in @neems-core/src/orm/ for all database
 * manipulations, ensuring consistent data access patterns and maintaining referential
 * integrity across operations.
 * 
 * Key Features:
 * - User management (create, list, edit, remove, password changes)  
 * - Company management (create, list, edit, remove with cascading deletes)
 * - Site management (create, list, edit, remove)
 * - Search functionality with regex and fixed-string support
 * - Secure password prompting without echo
 * - Cascading deletes to maintain data consistency
 * - Interactive confirmation prompts for destructive operations
 * 
 * For detailed usage information and available commands, run with --help.
 */

mod user_commands;
mod company_commands;
mod site_commands;
mod utils;

use clap::{Parser, Subcommand};
use user_commands::{UserAction, handle_user_command_with_conn};
use company_commands::{CompanyAction, handle_company_command_with_conn};
use site_commands::{SiteAction, handle_site_command_with_conn};
use utils::establish_connection;

#[derive(Parser)]
#[command(name = "neems-admin")]
#[command(about = "Administrative CLI for NEEMS database management")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::User { action } => handle_user_command(action)?,
        Commands::Company { action } => handle_company_command(action)?,
        Commands::Site { action } => handle_site_command(action)?,
        Commands::System { action } => handle_system_command(action)?,
    }

    Ok(())
}

fn handle_user_command(action: UserAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    handle_user_command_with_conn(&mut conn, action)
}

fn handle_company_command(action: CompanyAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    handle_company_command_with_conn(&mut conn, action)
}

fn handle_site_command(action: SiteAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    handle_site_command_with_conn(&mut conn, action)
}

fn handle_system_command(action: SystemAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SystemAction::Status => {
            println!("System Status: OK");
            println!("Database: Connected");
            // TODO: Add more system status checks
        }
        SystemAction::Maintenance => {
            println!("Running maintenance tasks...");
            // TODO: Implement maintenance tasks
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use neems_core::orm::testing::setup_test_db;
    use neems_core::orm::company::insert_company;
    use neems_core::orm::user::{get_user_by_email, list_all_users};
    use neems_core::orm::company::get_all_companies;
    use neems_core::orm::site::{get_all_sites, insert_site};

    #[test]
    fn test_handle_user_command_with_conn_create() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        let action = UserAction::Create {
            email: "cli_test@example.com".to_string(),
            password: Some("cli_password".to_string()),
            company_id: company.id,
            totp_secret: Some("cli_totp".to_string()),
        };
        
        let result = handle_user_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
        
        // Verify user was created
        let user = get_user_by_email(&mut conn, "cli_test@example.com")
            .expect("Failed to get CLI created user");
        assert_eq!(user.email, "cli_test@example.com");
        assert_eq!(user.company_id, company.id);
    }

    #[test]
    fn test_handle_user_command_with_conn_change_password() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        // Create user first
        let create_action = UserAction::Create {
            email: "change_test@example.com".to_string(),
            password: Some("original".to_string()),
            company_id: company.id,
            totp_secret: None,
        };
        handle_user_command_with_conn(&mut conn, create_action)
            .expect("Failed to create user");
        
        let original_user = get_user_by_email(&mut conn, "change_test@example.com")
            .expect("Failed to get user");
        let original_hash = original_user.password_hash.clone();
        
        // Change password
        let change_action = UserAction::ChangePassword {
            email: "change_test@example.com".to_string(),
            password: Some("new_password".to_string()),
        };
        
        let result = handle_user_command_with_conn(&mut conn, change_action);
        assert!(result.is_ok());
        
        // Verify password changed
        let updated_user = get_user_by_email(&mut conn, "change_test@example.com")
            .expect("Failed to get updated user");
        assert_ne!(updated_user.password_hash, original_hash);
    }

    #[test]
    fn test_handle_user_command_with_conn_list() {
        let mut conn = setup_test_db();
        
        let action = UserAction::Ls {
            search_term: None,
            fixed_string: false,
        };
        let result = handle_user_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_user_command_with_conn_rm() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        let create_action = UserAction::Create {
            email: "delete_me@example.com".to_string(),
            password: Some("password1".to_string()),
            company_id: company.id,
            totp_secret: None,
        };
        handle_user_command_with_conn(&mut conn, create_action).expect("Failed to create user");
        
        let create_action2 = UserAction::Create {
            email: "keep_me@test.com".to_string(),
            password: Some("password2".to_string()),
            company_id: company.id,
            totp_secret: None,
        };
        handle_user_command_with_conn(&mut conn, create_action2).expect("Failed to create user");
        
        let action = UserAction::Rm {
            search_term: "@example.com".to_string(),
            fixed_string: true,
            yes: true,
        };
        let result = handle_user_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
        
        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 1);
        assert_eq!(remaining_users[0].email, "keep_me@test.com");
    }

    #[test]
    fn test_handle_company_command_with_conn_ls() {
        let mut conn = setup_test_db();
        
        let action = CompanyAction::Ls {
            search_term: None,
            fixed_string: false,
        };
        let result = handle_company_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_company_command_with_conn_create() {
        let mut conn = setup_test_db();
        
        let action = CompanyAction::Create {
            name: "CLI Test Company".to_string(),
        };
        let result = handle_company_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
        
        let companies = get_all_companies(&mut conn).expect("Failed to get companies");
        let found = companies.iter().any(|c| c.name == "CLI Test Company");
        assert!(found);
    }

    #[test]
    fn test_handle_company_command_with_conn_rm() {
        let mut conn = setup_test_db();
        
        insert_company(&mut conn, "Remove This Company".to_string())
            .expect("Failed to create company");
        
        let action = CompanyAction::Rm {
            search_term: "Remove This".to_string(),
            fixed_string: true,
            yes: true,
        };
        let result = handle_company_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
        
        let companies = get_all_companies(&mut conn).expect("Failed to get companies");
        let found = companies.iter().any(|c| c.name == "Remove This Company");
        assert!(!found);
    }

    #[test]
    fn test_handle_site_command_with_conn_ls() {
        let mut conn = setup_test_db();
        
        let action = SiteAction::Ls {
            search_term: None,
            fixed_string: false,
            company_id: None,
        };
        let result = handle_site_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_site_command_with_conn_create() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        let action = SiteAction::Create {
            name: "CLI Test Site".to_string(),
            address: "CLI Test Address".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            company_id: company.id,
        };
        let result = handle_site_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
        
        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found = sites.iter().any(|s| s.name == "CLI Test Site");
        assert!(found);
    }

    #[test]
    fn test_handle_site_command_with_conn_rm() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        insert_site(&mut conn, "Remove This Site".to_string(), "Address".to_string(), 40.0, -74.0, company.id)
            .expect("Failed to create site");
        
        let action = SiteAction::Rm {
            search_term: "Remove This".to_string(),
            fixed_string: true,
            yes: true,
            company_id: None,
        };
        let result = handle_site_command_with_conn(&mut conn, action);
        assert!(result.is_ok());
        
        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found = sites.iter().any(|s| s.name == "Remove This Site");
        assert!(!found);
    }
}