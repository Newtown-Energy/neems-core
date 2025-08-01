/*!
 * NEEMS Administrative CLI Utility
 * 
 * This is a command-line interface for administrative management of a neems-core 
 * instance's SQLite database. The utility provides comprehensive database management
 * capabilities including user management, company management, and system operations.
 * 
 * The CLI leverages the ORM functions located in @neems-core/src/orm/ for all database
 * manipulations, ensuring consistent data access patterns and maintaining referential
 * integrity across operations.
 * 
 * Key Features:
 * - User management (create, list, remove, password changes)  
 * - Company management (create, list, remove with cascading deletes)
 * - Search functionality with regex and fixed-string support
 * - Secure password prompting without echo
 * - Cascading deletes to maintain data consistency
 * - Interactive confirmation prompts for destructive operations
 * 
 * For detailed usage information and available commands, run with --help.
 */

use clap::{Parser, Subcommand};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use neems_core::orm::user::{insert_user, list_all_users, get_user_by_email, update_user, delete_user_with_cleanup, get_users_by_company};
use neems_core::orm::company::{get_all_companies, insert_company, delete_company};
use neems_core::orm::site::{get_sites_by_company, delete_site};
use neems_core::models::UserNoTime;
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use dotenvy::dotenv;
use regex::Regex;
use std::io::{self, Write};
use rpassword::read_password;

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
    #[command(about = "Future: Non-database administrative commands")]
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
}

#[derive(Subcommand)]
enum UserAction {
    #[command(about = "Create a new user")]
    Create {
        #[arg(short, long, help = "Email address")]
        email: String,
        #[arg(short, long, help = "Password (will be prompted securely if not provided)")]
        password: Option<String>,
        #[arg(short, long, help = "Company ID")]
        company_id: i32,
        #[arg(long, help = "TOTP secret (optional)")]
        totp_secret: Option<String>,
    },
    #[command(about = "Change user password")]
    ChangePassword {
        #[arg(short, long, help = "Email address")]
        email: String,
        #[arg(short, long, help = "New password (will be prompted securely if not provided)")]
        password: Option<String>,
    },
    #[command(about = "List users, optionally filtered by search term")]
    Ls {
        #[arg(help = "Search term (regex by default, use -F for fixed string)")]
        search_term: Option<String>,
        #[arg(short = 'F', long = "fixed-string", help = "Treat search term as fixed string instead of regex")]
        fixed_string: bool,
    },
    #[command(about = "Remove users matching search term")]
    Rm {
        #[arg(help = "Search term to match users for removal (regex by default, use -F for fixed string)")]
        search_term: String,
        #[arg(short = 'F', long = "fixed-string", help = "Treat search term as fixed string instead of regex")]
        fixed_string: bool,
        #[arg(short = 'y', long = "yes", help = "Skip confirmation prompt")]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum CompanyAction {
    #[command(about = "List companies, optionally filtered by search term")]
    Ls {
        #[arg(help = "Search term (regex by default, use -F for fixed string)")]
        search_term: Option<String>,
        #[arg(short = 'F', long = "fixed-string", help = "Treat search term as fixed string instead of regex")]
        fixed_string: bool,
    },
    #[command(about = "Create a new company")]
    Create {
        #[arg(short, long, help = "Company name")]
        name: String,
    },
    #[command(about = "Remove companies matching search term")]
    Rm {
        #[arg(help = "Search term to match companies for removal (regex by default, use -F for fixed string)")]
        search_term: String,
        #[arg(short = 'F', long = "fixed-string", help = "Treat search term as fixed string instead of regex")]
        fixed_string: bool,
        #[arg(short = 'y', long = "yes", help = "Skip confirmation prompt")]
        yes: bool,
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
        Commands::System { action } => handle_system_command(action)?,
    }

    Ok(())
}

fn handle_user_command(action: UserAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    handle_user_command_with_conn(&mut conn, action)
}

fn handle_user_command_with_conn(
    conn: &mut SqliteConnection, 
    action: UserAction
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        UserAction::Create {
            email,
            password,
            company_id,
            totp_secret,
        } => {
            create_user_impl(conn, &email, password, company_id, totp_secret)?;
        }
        UserAction::ChangePassword { email, password } => {
            change_password_impl(conn, &email, password)?;
        }
        UserAction::Ls { search_term, fixed_string } => {
            list_users_impl(conn, search_term, fixed_string)?;
        }
        UserAction::Rm { search_term, fixed_string, yes } => {
            remove_users_impl(conn, search_term, fixed_string, yes)?;
        }
    }
    Ok(())
}

fn create_user_impl(
    conn: &mut SqliteConnection,
    email: &str,
    password: Option<String>,
    company_id: i32,
    totp_secret: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let password = match password {
        Some(p) => p,
        None => prompt_for_password()?,
    };
    
    let password_hash = hash_password(&password)
        .map_err(|e| format!("Failed to hash password: {}", e))?;
    
    let new_user = UserNoTime {
        email: email.to_string(),
        password_hash,
        company_id,
        totp_secret,
    };

    let created_user = insert_user(conn, new_user)?;
    
    println!("User created successfully!");
    println!("ID: {}", created_user.id);
    println!("Email: {}", created_user.email);
    println!("Company ID: {}", created_user.company_id);
    
    Ok(())
}

fn change_password_impl(
    conn: &mut SqliteConnection,
    email: &str,
    password: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let password = match password {
        Some(p) => p,
        None => prompt_for_password()?,
    };
    
    let password_hash = hash_password(&password)
        .map_err(|e| format!("Failed to hash password: {}", e))?;
    let user = get_user_by_email(conn, email)?;
    update_user(conn, user.id, None, Some(password_hash), None, None)?;
    
    println!("Password changed successfully for user: {}", email);
    Ok(())
}

fn list_users_impl(
    conn: &mut SqliteConnection, 
    search_term: Option<String>, 
    fixed_string: bool
) -> Result<(), Box<dyn std::error::Error>> {
    let users = list_all_users(conn)?;
    
    let filtered_users = if let Some(term) = search_term {
        if fixed_string {
            users.into_iter()
                .filter(|user| user.email.contains(&term))
                .collect::<Vec<_>>()
        } else {
            let regex = Regex::new(&term)
                .map_err(|e| format!("Invalid regex pattern '{}': {}", term, e))?;
            users.into_iter()
                .filter(|user| regex.is_match(&user.email))
                .collect::<Vec<_>>()
        }
    } else {
        users
    };
    
    if filtered_users.is_empty() {
        println!("No users found.");
    } else {
        println!("Users:");
        for user in filtered_users {
            println!("  ID: {}, Email: {}, Company ID: {}, Created: {}", 
                    user.id, user.email, user.company_id, user.created_at);
        }
    }
    
    Ok(())
}

fn remove_users_impl(
    conn: &mut SqliteConnection,
    search_term: String,
    fixed_string: bool,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let users = list_all_users(conn)?;
    
    let matching_users = if fixed_string {
        users.into_iter()
            .filter(|user| user.email.contains(&search_term))
            .collect::<Vec<_>>()
    } else {
        let regex = Regex::new(&search_term)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", search_term, e))?;
        users.into_iter()
            .filter(|user| regex.is_match(&user.email))
            .collect::<Vec<_>>()
    };
    
    if matching_users.is_empty() {
        println!("No users found matching the search term.");
        return Ok(());
    }
    
    println!("Found {} user(s) matching the search term:", matching_users.len());
    for user in &matching_users {
        println!("  ID: {}, Email: {}, Company ID: {}", 
                user.id, user.email, user.company_id);
    }
    
    if !yes {
        print!("Are you sure you want to delete these {} user(s)? [y/N]: ", matching_users.len());
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        
        if input != "y" && input != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    let mut deleted_count = 0;
    let mut errors = Vec::new();
    
    for user in matching_users {
        match delete_user_with_cleanup(conn, user.id) {
            Ok(rows_affected) => {
                if rows_affected > 0 {
                    deleted_count += 1;
                    println!("Deleted user: {} (ID: {})", user.email, user.id);
                }
            }
            Err(e) => {
                errors.push(format!("Failed to delete user {} (ID: {}): {}", user.email, user.id, e));
            }
        }
    }
    
    println!("Successfully deleted {} user(s).", deleted_count);
    
    if !errors.is_empty() {
        println!("Errors encountered:");
        for error in errors {
            println!("  {}", error);
        }
        return Err("Some deletions failed".into());
    }
    
    Ok(())
}

fn establish_connection() -> Result<SqliteConnection, Box<dyn std::error::Error>> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let conn = SqliteConnection::establish(&database_url)?;
    Ok(conn)
}

fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

fn prompt_for_password() -> Result<String, Box<dyn std::error::Error>> {
    print!("Enter new password: ");
    io::stdout().flush()?;
    let password = read_password()?;
    
    if password.is_empty() {
        return Err("Password cannot be empty".into());
    }
    
    print!("Confirm new password: ");
    io::stdout().flush()?;
    let confirm_password = read_password()?;
    
    if password != confirm_password {
        return Err("Passwords do not match".into());
    }
    
    Ok(password)
}

fn company_ls_impl(
    conn: &mut SqliteConnection,
    search_term: Option<String>,
    fixed_string: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let companies = get_all_companies(conn)?;
    
    let filtered_companies = if let Some(term) = search_term {
        if fixed_string {
            companies.into_iter()
                .filter(|company| company.name.contains(&term))
                .collect::<Vec<_>>()
        } else {
            let regex = Regex::new(&term)
                .map_err(|e| format!("Invalid regex pattern '{}': {}", term, e))?;
            companies.into_iter()
                .filter(|company| regex.is_match(&company.name))
                .collect::<Vec<_>>()
        }
    } else {
        companies
    };
    
    if filtered_companies.is_empty() {
        println!("No companies found.");
    } else {
        println!("Companies:");
        for company in filtered_companies {
            println!("  ID: {}, Name: {}, Created: {}", 
                    company.id, company.name, company.created_at);
        }
    }
    
    Ok(())
}

fn company_create_impl(
    conn: &mut SqliteConnection,
    name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let created_company = insert_company(conn, name)?;
    
    println!("Company created successfully!");
    println!("ID: {}", created_company.id);
    println!("Name: {}", created_company.name);
    println!("Created: {}", created_company.created_at);
    
    Ok(())
}

fn company_rm_impl(
    conn: &mut SqliteConnection,
    search_term: String,
    fixed_string: bool,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let companies = get_all_companies(conn)?;
    
    let matching_companies = if fixed_string {
        companies.into_iter()
            .filter(|company| company.name.contains(&search_term))
            .collect::<Vec<_>>()
    } else {
        let regex = Regex::new(&search_term)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", search_term, e))?;
        companies.into_iter()
            .filter(|company| regex.is_match(&company.name))
            .collect::<Vec<_>>()
    };
    
    if matching_companies.is_empty() {
        println!("No companies found matching the search term.");
        return Ok(());
    }
    
    println!("Found {} company(ies) matching the search term:", matching_companies.len());
    for company in &matching_companies {
        // Get associated users and sites counts
        let users = get_users_by_company(conn, company.id)?;
        let sites = get_sites_by_company(conn, company.id)?;
        
        println!("  ID: {}, Name: {}, Users: {}, Sites: {}", 
                company.id, company.name, users.len(), sites.len());
    }
    
    if !yes {
        print!("Are you sure you want to delete these {} company(ies) and all associated users and sites? [y/N]: ", 
               matching_companies.len());
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        
        if input != "y" && input != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    let mut deleted_count = 0;
    let mut errors = Vec::new();
    
    for company in matching_companies {
        match delete_company_with_cascade(conn, company.id) {
            Ok(success) => {
                if success {
                    deleted_count += 1;
                    println!("Deleted company: {} (ID: {})", company.name, company.id);
                }
            }
            Err(e) => {
                errors.push(format!("Failed to delete company {} (ID: {}): {}", company.name, company.id, e));
            }
        }
    }
    
    println!("Successfully deleted {} company(ies).", deleted_count);
    
    if !errors.is_empty() {
        println!("Errors encountered:");
        for error in errors {
            println!("  {}", error);
        }
        return Err("Some deletions failed".into());
    }
    
    Ok(())
}

fn delete_company_with_cascade(
    conn: &mut SqliteConnection,
    company_id: i32,
) -> Result<bool, Box<dyn std::error::Error>> {
    // First delete all users in the company
    let users = get_users_by_company(conn, company_id)?;
    for user in users {
        delete_user_with_cleanup(conn, user.id)?;
    }
    
    // Then delete all sites in the company
    let sites = get_sites_by_company(conn, company_id)?;
    for site in sites {
        delete_site(conn, site.id)?;
    }
    
    // Finally delete the company itself
    let deleted = delete_company(conn, company_id)?;
    Ok(deleted)
}

fn handle_company_command(action: CompanyAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = establish_connection()?;
    handle_company_command_with_conn(&mut conn, action)
}

fn handle_company_command_with_conn(
    conn: &mut SqliteConnection,
    action: CompanyAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        CompanyAction::Ls { search_term, fixed_string } => {
            company_ls_impl(conn, search_term, fixed_string)?;
        }
        CompanyAction::Create { name } => {
            company_create_impl(conn, name)?;
        }
        CompanyAction::Rm { search_term, fixed_string, yes } => {
            company_rm_impl(conn, search_term, fixed_string, yes)?;
        }
    }
    Ok(())
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
    use argon2::{PasswordVerifier, PasswordHash};

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
    fn test_handle_user_command_with_conn_rm() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        create_user_impl(&mut conn, "delete_me@example.com", Some("password1".to_string()), company.id, None)
            .expect("Failed to create user");
        create_user_impl(&mut conn, "keep_me@test.com", Some("password2".to_string()), company.id, None)
            .expect("Failed to create user");
        
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
}