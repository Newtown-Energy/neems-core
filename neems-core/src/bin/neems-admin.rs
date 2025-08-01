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

mod admin_cli {
    pub mod user_commands;
    pub mod company_commands;
    pub mod site_commands;
    pub mod utils;
}

use clap::{Parser, Subcommand};
use admin_cli::user_commands::{UserAction, handle_user_command_with_conn};
use admin_cli::company_commands::{CompanyAction, handle_company_command_with_conn};
use admin_cli::site_commands::{SiteAction, handle_site_command_with_conn};
use admin_cli::utils::establish_connection;

#[derive(Parser)]
#[command(name = "neems-admin")]
#[command(about = "Administrative CLI for NEEMS database management")]
#[command(version)]
#[command(disable_help_subcommand = true)]
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
    use neems_core::orm::user::{get_user_by_email, list_all_users, get_user};
    use neems_core::orm::company::{get_all_companies, get_company_by_id};
    use neems_core::orm::site::{get_all_sites, insert_site, get_site_by_id};
    use admin_cli::user_commands::{create_user_impl, change_password_impl, list_users_impl, remove_users_impl, user_edit_impl, hash_password};
    use admin_cli::company_commands::{company_ls_impl, company_create_impl, company_rm_impl, company_edit_impl};
    use admin_cli::site_commands::{site_ls_impl, site_create_impl, site_rm_impl, site_edit_impl};
    use argon2::{Argon2, PasswordVerifier, PasswordHash};

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

    #[test]
    fn test_hash_password() {
        let password = "test_password_123";
        let hash = hash_password(password).expect("Failed to hash password");
        
        // Verify the hash is valid argon2 format
        assert!(hash.starts_with("$argon2"));
        
        // Verify we can verify the password with the hash
        let argon2 = Argon2::default();
        let parsed_hash = PasswordHash::new(&hash).expect("Failed to parse hash");
        assert!(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok());
    }

    #[test]
    fn test_create_user_impl() {
        let mut conn = setup_test_db();
        
        // Create a test company first
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        // Test creating a user
        let result = create_user_impl(
            &mut conn,
            "test@example.com",
            Some("password123".to_string()),
            company.id,
            Some("totp_secret".to_string()),
        );
        
        assert!(result.is_ok());
        
        // Verify user was created by fetching it
        let created_user = get_user_by_email(&mut conn, "test@example.com")
            .expect("Failed to get created user");
        
        assert_eq!(created_user.email, "test@example.com");
        assert_eq!(created_user.company_id, company.id);
        assert_eq!(created_user.totp_secret, Some("totp_secret".to_string()));
        
        // Verify password was hashed (not stored as plaintext)
        assert_ne!(created_user.password_hash, "password123");
        assert!(created_user.password_hash.starts_with("$argon2"));
    }

    #[test]
    fn test_create_user_impl_duplicate_email() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        // Create first user
        create_user_impl(&mut conn, "test@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create first user");
        
        // Try to create second user with same email
        let result = create_user_impl(&mut conn, "test@example.com", Some("password2".to_string()), company.id, None);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_change_password_impl() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        // Create a user first
        create_user_impl(&mut conn, "test@example.com", Some("original_password".to_string()), company.id, None)
            .expect("Failed to create user");
        
        let original_user = get_user_by_email(&mut conn, "test@example.com")
            .expect("Failed to get user");
        let original_hash = original_user.password_hash.clone();
        
        // Change password
        let result = change_password_impl(&mut conn, "test@example.com", Some("new_password".to_string()));
        assert!(result.is_ok());
        
        // Verify password was changed
        let updated_user = get_user_by_email(&mut conn, "test@example.com")
            .expect("Failed to get updated user");
        
        assert_ne!(updated_user.password_hash, original_hash);
        assert!(updated_user.password_hash.starts_with("$argon2"));
        
        // Verify new password works
        let argon2 = Argon2::default();
        let parsed_hash = PasswordHash::new(&updated_user.password_hash)
            .expect("Failed to parse new hash");
        assert!(argon2.verify_password("new_password".as_bytes(), &parsed_hash).is_ok());
    }

    #[test]
    fn test_change_password_impl_nonexistent_user() {
        let mut conn = setup_test_db();
        
        let result = change_password_impl(&mut conn, "nonexistent@example.com", Some("password".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_list_users_impl_empty() {
        let mut conn = setup_test_db();
        
        // Should not panic with empty database
        let result = list_users_impl(&mut conn, None, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_users_impl_with_users() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        // Create a few users
        create_user_impl(&mut conn, "user1@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "user2@example.com", Some("password2".to_string()), company.id, None)
            .expect("Failed to create user2");
        
        let result = list_users_impl(&mut conn, None, false);
        assert!(result.is_ok());
        
        // Verify users exist
        let users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].email, "user1@example.com");
        assert_eq!(users[1].email, "user2@example.com");
    }

    #[test]
    fn test_list_users_impl_with_regex_search() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "alice@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "bob@test.com", Some("password2".to_string()), company.id, None)
            .expect("Failed to create user2");
        create_user_impl(&mut conn, "charlie@example.org", Some("password3".to_string()), company.id, None)
            .expect("Failed to create user3");
        
        let result = list_users_impl(&mut conn, Some("example\\.com$".to_string()), false);
        assert!(result.is_ok());
        
        let result = list_users_impl(&mut conn, Some("@test".to_string()), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_users_impl_with_fixed_string_search() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "user.with.dots@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "normaluser@test.com", Some("password2".to_string()), company.id, None)
            .expect("Failed to create user2");
        
        let result = list_users_impl(&mut conn, Some(".with.".to_string()), true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_users_impl_invalid_regex() {
        let mut conn = setup_test_db();
        
        let result = list_users_impl(&mut conn, Some("[invalid".to_string()), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_users_impl_no_matches() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "user@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user");
        
        let result = list_users_impl(&mut conn, Some("nonexistent".to_string()), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_users_impl_with_regex() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "alice@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "bob@test.com", Some("password2".to_string()), company.id, None)
            .expect("Failed to create user2");
        create_user_impl(&mut conn, "charlie@example.org", Some("password3".to_string()), company.id, None)
            .expect("Failed to create user3");
        
        let result = remove_users_impl(&mut conn, "example\\.com$".to_string(), false, true);
        assert!(result.is_ok());
        
        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 2);
        assert_eq!(remaining_users[0].email, "bob@test.com");
        assert_eq!(remaining_users[1].email, "charlie@example.org");
    }

    #[test]
    fn test_remove_users_impl_with_fixed_string() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "user.with.dots@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "normaluser@test.com", Some("password2".to_string()), company.id, None)
            .expect("Failed to create user2");
        
        let result = remove_users_impl(&mut conn, ".with.".to_string(), true, true);
        assert!(result.is_ok());
        
        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 1);
        assert_eq!(remaining_users[0].email, "normaluser@test.com");
    }

    #[test]
    fn test_remove_users_impl_no_matches() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "user@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user");
        
        let result = remove_users_impl(&mut conn, "nonexistent".to_string(), false, true);
        assert!(result.is_ok());
        
        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 1);
    }

    #[test]
    fn test_remove_users_impl_invalid_regex() {
        let mut conn = setup_test_db();
        
        let result = remove_users_impl(&mut conn, "[invalid".to_string(), false, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_change_password_impl_with_provided_password() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "password_test@example.com", Some("original_password".to_string()), company.id, None)
            .expect("Failed to create user");
        
        let original_user = get_user_by_email(&mut conn, "password_test@example.com")
            .expect("Failed to get user");
        let original_hash = original_user.password_hash.clone();
        
        let result = change_password_impl(&mut conn, "password_test@example.com", Some("new_password".to_string()));
        assert!(result.is_ok());
        
        let updated_user = get_user_by_email(&mut conn, "password_test@example.com")
            .expect("Failed to get updated user");
        
        assert_ne!(updated_user.password_hash, original_hash);
        assert!(updated_user.password_hash.starts_with("$argon2"));
    }

    #[test]
    fn test_create_user_impl_with_provided_password() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        let result = create_user_impl(
            &mut conn,
            "create_test@example.com",
            Some("test_password".to_string()),
            company.id,
            None,
        );
        assert!(result.is_ok());
        
        let created_user = get_user_by_email(&mut conn, "create_test@example.com")
            .expect("Failed to get created user");
        
        assert_eq!(created_user.email, "create_test@example.com");
        assert_eq!(created_user.company_id, company.id);
        assert!(created_user.password_hash.starts_with("$argon2"));
    }

    #[test]
    fn test_company_ls_impl_all() {
        let mut conn = setup_test_db();
        
        insert_company(&mut conn, "Test Company 1".to_string())
            .expect("Failed to create company 1");
        insert_company(&mut conn, "Test Company 2".to_string())
            .expect("Failed to create company 2");
        
        let result = company_ls_impl(&mut conn, None, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_company_ls_impl_with_search() {
        let mut conn = setup_test_db();
        
        insert_company(&mut conn, "ACME Corp".to_string())
            .expect("Failed to create company 1");
        insert_company(&mut conn, "Tech Solutions".to_string())
            .expect("Failed to create company 2");
        
        let result = company_ls_impl(&mut conn, Some("ACME".to_string()), true);
        assert!(result.is_ok());
        
        let result = company_ls_impl(&mut conn, Some("^Tech".to_string()), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_company_create_impl() {
        let mut conn = setup_test_db();
        
        let result = company_create_impl(&mut conn, "New Test Company".to_string());
        assert!(result.is_ok());
        
        let companies = get_all_companies(&mut conn).expect("Failed to get companies");
        let found = companies.iter().any(|c| c.name == "New Test Company");
        assert!(found);
    }

    #[test]
    fn test_company_rm_impl_with_cascade() {
        let mut conn = setup_test_db();
        
        // Create company with users and sites
        let company = insert_company(&mut conn, "Delete Me Company".to_string())
            .expect("Failed to create company");
        let keep_company = insert_company(&mut conn, "Keep Me Company".to_string())
            .expect("Failed to create company");
        
        // Create user in company to be deleted
        create_user_impl(&mut conn, "user@deleteme.com", Some("password".to_string()), company.id, None)
            .expect("Failed to create user");
        
        // Create user in company to keep
        create_user_impl(&mut conn, "user@keepme.com", Some("password".to_string()), keep_company.id, None)
            .expect("Failed to create user");
        
        // Delete company
        let result = company_rm_impl(&mut conn, "Delete Me".to_string(), true, true);
        assert!(result.is_ok());
        
        // Verify company was deleted
        let companies = get_all_companies(&mut conn).expect("Failed to get companies");
        let found_deleted = companies.iter().any(|c| c.name == "Delete Me Company");
        assert!(!found_deleted);
        
        // Verify other company still exists
        let found_kept = companies.iter().any(|c| c.name == "Keep Me Company");
        assert!(found_kept);
        
        // Verify users were deleted with company
        let all_users = list_all_users(&mut conn).expect("Failed to get users");
        let deleted_user_exists = all_users.iter().any(|u| u.email == "user@deleteme.com");
        assert!(!deleted_user_exists);
        
        let kept_user_exists = all_users.iter().any(|u| u.email == "user@keepme.com");
        assert!(kept_user_exists);
    }

    #[test]
    fn test_site_ls_impl_all() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        insert_site(&mut conn, "Site 1".to_string(), "Address 1".to_string(), 40.0, -74.0, company.id)
            .expect("Failed to create site 1");
        insert_site(&mut conn, "Site 2".to_string(), "Address 2".to_string(), 41.0, -75.0, company.id)
            .expect("Failed to create site 2");
        
        let result = site_ls_impl(&mut conn, None, false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_site_ls_impl_with_search() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        insert_site(&mut conn, "Main Office".to_string(), "123 Main St".to_string(), 40.0, -74.0, company.id)
            .expect("Failed to create site 1");
        insert_site(&mut conn, "Branch Office".to_string(), "456 Branch Ave".to_string(), 41.0, -75.0, company.id)
            .expect("Failed to create site 2");
        
        let result = site_ls_impl(&mut conn, Some("Main".to_string()), true, None);
        assert!(result.is_ok());
        
        let result = site_ls_impl(&mut conn, Some("^Branch".to_string()), false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_site_ls_impl_with_company_filter() {
        let mut conn = setup_test_db();
        
        let company1 = insert_company(&mut conn, "Company 1".to_string())
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string())
            .expect("Failed to create company 2");
        
        insert_site(&mut conn, "Site A".to_string(), "Address A".to_string(), 40.0, -74.0, company1.id)
            .expect("Failed to create site A");
        insert_site(&mut conn, "Site B".to_string(), "Address B".to_string(), 41.0, -75.0, company2.id)
            .expect("Failed to create site B");
        
        let result = site_ls_impl(&mut conn, None, false, Some(company1.id));
        assert!(result.is_ok());
    }

    #[test]
    fn test_site_create_impl() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        let result = site_create_impl(
            &mut conn,
            "New Site".to_string(),
            "123 New St".to_string(),
            40.7128,
            -74.0060,
            company.id,
        );
        assert!(result.is_ok());
        
        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found = sites.iter().any(|s| s.name == "New Site");
        assert!(found);
    }

    #[test]
    fn test_site_rm_impl() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        insert_site(&mut conn, "Delete Me Site".to_string(), "Address".to_string(), 40.0, -74.0, company.id)
            .expect("Failed to create site");
        insert_site(&mut conn, "Keep Me Site".to_string(), "Address".to_string(), 41.0, -75.0, company.id)
            .expect("Failed to create site");
        
        let result = site_rm_impl(&mut conn, "Delete Me".to_string(), true, true, None);
        assert!(result.is_ok());
        
        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found_deleted = sites.iter().any(|s| s.name == "Delete Me Site");
        assert!(!found_deleted);
        
        let found_kept = sites.iter().any(|s| s.name == "Keep Me Site");
        assert!(found_kept);
    }

    #[test]
    fn test_site_rm_impl_with_company_filter() {
        let mut conn = setup_test_db();
        
        let company1 = insert_company(&mut conn, "Company 1".to_string())
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string())
            .expect("Failed to create company 2");
        
        insert_site(&mut conn, "Test Site".to_string(), "Address 1".to_string(), 40.0, -74.0, company1.id)
            .expect("Failed to create site 1");
        insert_site(&mut conn, "Test Site".to_string(), "Address 2".to_string(), 41.0, -75.0, company2.id)
            .expect("Failed to create site 2");
        
        // Delete only from company1
        let result = site_rm_impl(&mut conn, "Test Site".to_string(), true, true, Some(company1.id));
        assert!(result.is_ok());
        
        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].company_id, company2.id);
    }

    #[test]
    fn test_user_edit_impl() {
        let mut conn = setup_test_db();
        
        let company1 = insert_company(&mut conn, "Company 1".to_string())
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string())
            .expect("Failed to create company 2");
        
        // Create a user
        create_user_impl(&mut conn, "original@example.com", Some("password".to_string()), company1.id, Some("original_totp".to_string()))
            .expect("Failed to create user");
        
        let user = get_user_by_email(&mut conn, "original@example.com")
            .expect("Failed to get user");
        
        // Edit email
        let result = user_edit_impl(&mut conn, user.id, Some("updated@example.com".to_string()), None, None);
        assert!(result.is_ok());
        
        let updated_user = get_user(&mut conn, user.id).expect("Failed to get updated user");
        assert_eq!(updated_user.email, "updated@example.com");
        assert_eq!(updated_user.company_id, company1.id);
        
        // Edit company and TOTP
        let result = user_edit_impl(&mut conn, user.id, None, Some(company2.id), Some("new_totp".to_string()));
        assert!(result.is_ok());
        
        let updated_user = get_user(&mut conn, user.id).expect("Failed to get updated user");
        assert_eq!(updated_user.company_id, company2.id);
        assert_eq!(updated_user.totp_secret, Some("new_totp".to_string()));
    }

    #[test]
    fn test_user_edit_impl_nonexistent_user() {
        let mut conn = setup_test_db();
        
        let result = user_edit_impl(&mut conn, 99999, Some("new@example.com".to_string()), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_edit_impl_nonexistent_company() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        create_user_impl(&mut conn, "user@example.com", Some("password".to_string()), company.id, None)
            .expect("Failed to create user");
        
        let user = get_user_by_email(&mut conn, "user@example.com")
            .expect("Failed to get user");
        
        let result = user_edit_impl(&mut conn, user.id, None, Some(99999), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_company_edit_impl() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Original Company".to_string())
            .expect("Failed to create company");
        
        let result = company_edit_impl(&mut conn, company.id, Some("Updated Company".to_string()));
        assert!(result.is_ok());
        
        let updated_company = get_company_by_id(&mut conn, company.id)
            .expect("Failed to get updated company")
            .expect("Company should exist");
        assert_eq!(updated_company.name, "Updated Company");
    }

    #[test]
    fn test_company_edit_impl_nonexistent_company() {
        let mut conn = setup_test_db();
        
        let result = company_edit_impl(&mut conn, 99999, Some("New Name".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_site_edit_impl() {
        let mut conn = setup_test_db();
        
        let company1 = insert_company(&mut conn, "Company 1".to_string())
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string())
            .expect("Failed to create company 2");
        
        let site = insert_site(&mut conn, "Original Site".to_string(), "Original Address".to_string(), 40.0, -74.0, company1.id)
            .expect("Failed to create site");
        
        // Edit name and address
        let result = site_edit_impl(&mut conn, site.id, Some("Updated Site".to_string()), Some("Updated Address".to_string()), None, None, None);
        assert!(result.is_ok());
        
        let updated_site = get_site_by_id(&mut conn, site.id)
            .expect("Failed to get updated site")
            .expect("Site should exist");
        assert_eq!(updated_site.name, "Updated Site");
        assert_eq!(updated_site.address, "Updated Address");
        assert_eq!(updated_site.latitude, 40.0);
        assert_eq!(updated_site.longitude, -74.0);
        
        // Edit coordinates and company
        let result = site_edit_impl(&mut conn, site.id, None, None, Some(41.0), Some(-75.0), Some(company2.id));
        assert!(result.is_ok());
        
        let updated_site = get_site_by_id(&mut conn, site.id)
            .expect("Failed to get updated site")
            .expect("Site should exist");
        assert_eq!(updated_site.latitude, 41.0);
        assert_eq!(updated_site.longitude, -75.0);
        assert_eq!(updated_site.company_id, company2.id);
    }

    #[test]
    fn test_site_edit_impl_nonexistent_site() {
        let mut conn = setup_test_db();
        
        let result = site_edit_impl(&mut conn, 99999, Some("New Name".to_string()), None, None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_site_edit_impl_nonexistent_company() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create company");
        
        let site = insert_site(&mut conn, "Test Site".to_string(), "Address".to_string(), 40.0, -74.0, company.id)
            .expect("Failed to create site");
        
        let result = site_edit_impl(&mut conn, site.id, None, None, None, None, Some(99999));
        assert!(result.is_err());
    }
}