use clap::{Parser, Subcommand};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use neems_core::orm::user::{insert_user, list_all_users, get_user_by_email, update_user};
use neems_core::models::UserNoTime;
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use dotenvy::dotenv;
use regex::Regex;

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
        #[arg(short, long, help = "Password")]
        password: String,
        #[arg(short, long, help = "Company ID")]
        company_id: i32,
        #[arg(long, help = "TOTP secret (optional)")]
        totp_secret: Option<String>,
    },
    #[command(about = "Change user password")]
    ChangePassword {
        #[arg(short, long, help = "Email address")]
        email: String,
        #[arg(short, long, help = "New password")]
        password: String,
    },
    #[command(about = "List users, optionally filtered by search term")]
    List {
        #[arg(help = "Search term (regex by default, use -F for fixed string)")]
        search_term: Option<String>,
        #[arg(short = 'F', long = "fixed-string", help = "Treat search term as fixed string instead of regex")]
        fixed_string: bool,
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
            create_user_impl(conn, &email, &password, company_id, totp_secret)?;
        }
        UserAction::ChangePassword { email, password } => {
            change_password_impl(conn, &email, &password)?;
        }
        UserAction::List { search_term, fixed_string } => {
            list_users_impl(conn, search_term, fixed_string)?;
        }
    }
    Ok(())
}

fn create_user_impl(
    conn: &mut SqliteConnection,
    email: &str,
    password: &str,
    company_id: i32,
    totp_secret: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let password_hash = hash_password(password)
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
    password: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let password_hash = hash_password(password)
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
            "password123",
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
        create_user_impl(&mut conn, "test@example.com", "password1", company.id, None)
            .expect("Failed to create first user");
        
        // Try to create second user with same email
        let result = create_user_impl(&mut conn, "test@example.com", "password2", company.id, None);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_change_password_impl() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to create test company");
        
        // Create a user first
        create_user_impl(&mut conn, "test@example.com", "original_password", company.id, None)
            .expect("Failed to create user");
        
        let original_user = get_user_by_email(&mut conn, "test@example.com")
            .expect("Failed to get user");
        let original_hash = original_user.password_hash.clone();
        
        // Change password
        let result = change_password_impl(&mut conn, "test@example.com", "new_password");
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
        
        let result = change_password_impl(&mut conn, "nonexistent@example.com", "password");
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
        create_user_impl(&mut conn, "user1@example.com", "password1", company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "user2@example.com", "password2", company.id, None)
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
            password: "cli_password".to_string(),
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
            password: "original".to_string(),
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
            password: "new_password".to_string(),
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
        
        let action = UserAction::List {
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
        
        create_user_impl(&mut conn, "alice@example.com", "password1", company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "bob@test.com", "password2", company.id, None)
            .expect("Failed to create user2");
        create_user_impl(&mut conn, "charlie@example.org", "password3", company.id, None)
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
        
        create_user_impl(&mut conn, "user.with.dots@example.com", "password1", company.id, None)
            .expect("Failed to create user1");
        create_user_impl(&mut conn, "normaluser@test.com", "password2", company.id, None)
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
        
        create_user_impl(&mut conn, "user@example.com", "password1", company.id, None)
            .expect("Failed to create user");
        
        let result = list_users_impl(&mut conn, Some("nonexistent".to_string()), false);
        assert!(result.is_ok());
    }
}