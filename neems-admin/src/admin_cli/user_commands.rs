use std::io::{self, Write};

use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use clap::Subcommand;
use diesel::sqlite::SqliteConnection;
use neems_api::{
    models::UserInput,
    orm::{
        company::get_company_by_id,
        entity_activity::get_created_at,
        role::get_role_by_name,
        user::{
            delete_user_with_cleanup, get_user, get_user_by_email, insert_user, list_all_users,
            update_user,
        },
        user_role::{
            assign_user_role_by_name, get_user_roles, remove_all_user_roles,
            remove_user_role_by_name,
        },
    },
};
use regex::Regex;
use rpassword::read_password;

use crate::admin_cli::utils::resolve_company_id;

#[derive(Subcommand)]
pub enum UserAction {
    #[command(about = "Add a new user")]
    Add {
        #[arg(short, long, help = "Email address")]
        email: String,
        #[arg(
            short,
            long,
            help = "Password (will be prompted securely if not provided)"
        )]
        password: Option<String>,
        #[arg(short, long, help = "Company ID or name")]
        company_id: String,
        #[arg(long, help = "TOTP secret (optional)")]
        totp_secret: Option<String>,
    },
    #[command(about = "Change user password")]
    ChangePassword {
        #[arg(short, long, help = "Email address")]
        email: String,
        #[arg(
            short,
            long,
            help = "New password (will be prompted securely if not provided)"
        )]
        password: Option<String>,
    },
    #[command(about = "List users, optionally filtered by search term")]
    Ls {
        #[arg(help = "Search term (regex by default, use -F for fixed string)")]
        search_term: Option<String>,
        #[arg(
            short = 'F',
            long = "fixed-string",
            help = "Treat search term as fixed string instead of regex"
        )]
        fixed_string: bool,
    },
    #[command(about = "Remove users matching search term")]
    Rm {
        #[arg(
            help = "Search term to match users for removal (regex by default, use -F for fixed string)"
        )]
        search_term: String,
        #[arg(
            short = 'F',
            long = "fixed-string",
            help = "Treat search term as fixed string instead of regex"
        )]
        fixed_string: bool,
        #[arg(short = 'y', long = "yes", help = "Skip confirmation prompt")]
        yes: bool,
    },
    #[command(about = "Edit user fields")]
    Edit {
        #[arg(short, long, help = "User ID to edit")]
        id: i32,
        #[arg(long, help = "New email address")]
        email: Option<String>,
        #[arg(long, help = "New company ID or name")]
        company_id: Option<String>,
        #[arg(long, help = "New TOTP secret")]
        totp_secret: Option<String>,
    },
    #[command(about = "Add a role to a user")]
    AddRole {
        #[arg(short, long, help = "User email address")]
        email: String,
        #[arg(short, long, help = "Role name to add")]
        role: String,
    },
    #[command(about = "Remove a role from a user")]
    RmRole {
        #[arg(short, long, help = "User email address")]
        email: String,
        #[arg(short, long, help = "Role name to remove")]
        role: String,
    },
    #[command(about = "Set all roles for a user (replaces existing roles)")]
    SetRoles {
        #[arg(short, long, help = "User email address")]
        email: String,
        #[arg(short, long, help = "Comma-separated list of role names")]
        roles: String,
    },
}

pub fn handle_user_command_with_conn(
    conn: &mut SqliteConnection,
    action: UserAction,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        UserAction::Add { email, password, company_id, totp_secret } => {
            let resolved_company_id = resolve_company_id(conn, &company_id)?;
            add_user_impl(conn, &email, password, resolved_company_id, totp_secret, admin_user_id)?;
        }
        UserAction::ChangePassword { email, password } => {
            change_password_impl(conn, &email, password, admin_user_id)?;
        }
        UserAction::Ls { search_term, fixed_string } => {
            list_users_impl(conn, search_term, fixed_string)?;
        }
        UserAction::Rm { search_term, fixed_string, yes } => {
            remove_users_impl(conn, search_term, fixed_string, yes, admin_user_id)?;
        }
        UserAction::Edit { id, email, company_id, totp_secret } => {
            let resolved_company_id = if let Some(company_str) = company_id {
                Some(resolve_company_id(conn, &company_str)?)
            } else {
                None
            };
            user_edit_impl(conn, id, email, resolved_company_id, totp_secret, admin_user_id)?;
        }
        UserAction::AddRole { email, role } => {
            user_add_role_impl(conn, &email, &role)?;
        }
        UserAction::RmRole { email, role } => {
            user_rm_role_impl(conn, &email, &role)?;
        }
        UserAction::SetRoles { email, roles } => {
            user_set_roles_impl(conn, &email, &roles)?;
        }
    }
    Ok(())
}

pub fn add_user_impl(
    conn: &mut SqliteConnection,
    email: &str,
    password: Option<String>,
    company_id: i32,
    totp_secret: Option<String>,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user already exists
    if let Some(existing_user) = get_user_by_email(conn, email)? {
        println!("User already exists!");
        println!("ID: {}", existing_user.id);
        println!("Email: {}", existing_user.email);
        println!("Company ID: {}", existing_user.company_id);
        return Ok(());
    }

    let password = match password {
        Some(p) => p,
        None => prompt_for_password()?,
    };

    let password_hash =
        hash_password(&password).map_err(|e| format!("Failed to hash password: {}", e))?;

    let new_user = UserInput {
        email: email.to_string(),
        password_hash,
        company_id,
        totp_secret,
    };

    let created_user = insert_user(conn, new_user, Some(admin_user_id))?;

    println!("User created successfully!");
    println!("ID: {}", created_user.id);
    println!("Email: {}", created_user.email);
    println!("Company ID: {}", created_user.company_id);

    Ok(())
}

pub fn change_password_impl(
    conn: &mut SqliteConnection,
    email: &str,
    password: Option<String>,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let password = match password {
        Some(p) => p,
        None => prompt_for_password()?,
    };

    let password_hash =
        hash_password(&password).map_err(|e| format!("Failed to hash password: {}", e))?;
    let user = get_user_by_email(conn, email)?
        .ok_or_else(|| format!("User with email '{}' not found", email))?;
    update_user(conn, user.id, None, Some(password_hash), None, None, Some(admin_user_id))?;

    println!("Password changed successfully for user: {}", email);
    Ok(())
}

pub fn list_users_impl(
    conn: &mut SqliteConnection,
    search_term: Option<String>,
    fixed_string: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let users = list_all_users(conn)?;

    let filtered_users = if let Some(term) = search_term {
        if fixed_string {
            users.into_iter().filter(|user| user.email.contains(&term)).collect::<Vec<_>>()
        } else {
            let regex = Regex::new(&term)
                .map_err(|e| format!("Invalid regex pattern '{}': {}", term, e))?;
            users.into_iter().filter(|user| regex.is_match(&user.email)).collect::<Vec<_>>()
        }
    } else {
        users
    };

    if filtered_users.is_empty() {
        println!("No users found.");
    } else {
        println!("Users:");
        for user in filtered_users {
            let created_at = get_created_at(conn, "users", user.id)
                .map(|dt| dt.to_string())
                .unwrap_or_else(|_| "Unknown".to_string());

            let roles = get_user_roles(conn, user.id)
                .map(|roles| {
                    if roles.is_empty() {
                        "None".to_string()
                    } else {
                        roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
                    }
                })
                .unwrap_or_else(|_| "Error loading roles".to_string());

            println!(
                "  ID: {}, Email: {}, Company ID: {}, Created: {}, Roles: {}",
                user.id, user.email, user.company_id, created_at, roles
            );
        }
    }

    Ok(())
}

pub fn remove_users_impl(
    conn: &mut SqliteConnection,
    search_term: String,
    fixed_string: bool,
    yes: bool,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let users = list_all_users(conn)?;

    let matching_users = if fixed_string {
        users
            .into_iter()
            .filter(|user| user.email.contains(&search_term))
            .collect::<Vec<_>>()
    } else {
        let regex = Regex::new(&search_term)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", search_term, e))?;
        users.into_iter().filter(|user| regex.is_match(&user.email)).collect::<Vec<_>>()
    };

    if matching_users.is_empty() {
        println!("No users found matching the search term.");
        return Ok(());
    }

    println!("Found {} user(s) matching the search term:", matching_users.len());
    for user in &matching_users {
        println!("  ID: {}, Email: {}, Company ID: {}", user.id, user.email, user.company_id);
    }

    if !yes {
        print!(
            "Are you sure you want to delete these {} user(s)? [y/N]: ",
            matching_users.len()
        );
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
        match delete_user_with_cleanup(conn, user.id, Some(admin_user_id)) {
            Ok(rows_affected) => {
                if rows_affected > 0 {
                    deleted_count += 1;
                    println!("Deleted user: {} (ID: {})", user.email, user.id);
                }
            }
            Err(e) => {
                errors
                    .push(format!("Failed to delete user {} (ID: {}): {}", user.email, user.id, e));
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

pub fn user_edit_impl(
    conn: &mut SqliteConnection,
    user_id: i32,
    new_email: Option<String>,
    new_company_id: Option<i32>,
    new_totp_secret: Option<String>,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists
    let _user = get_user(conn, user_id)?;

    // Check if any fields need updating
    if new_email.is_none() && new_company_id.is_none() && new_totp_secret.is_none() {
        println!("No fields specified for update. Use --email, --company-id, or --totp-secret.");
        return Ok(());
    }

    // Validate company exists if specified
    if let Some(comp_id) = new_company_id {
        if get_company_by_id(conn, comp_id)?.is_none() {
            return Err(format!("Company with ID {} does not exist", comp_id).into());
        }
    }

    let updated_user = update_user(
        conn,
        user_id,
        new_email,
        None,
        new_company_id,
        new_totp_secret,
        Some(admin_user_id),
    )?;

    println!("User updated successfully!");
    println!("ID: {}", updated_user.id);
    println!("Email: {}", updated_user.email);
    println!("Company ID: {}", updated_user.company_id);
    if let Some(ref totp) = updated_user.totp_secret {
        println!("TOTP Secret: {}", totp);
    } else {
        println!("TOTP Secret: None");
    }

    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

pub fn prompt_for_password() -> Result<String, Box<dyn std::error::Error>> {
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

pub fn user_add_role_impl(
    conn: &mut SqliteConnection,
    email: &str,
    role_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists
    let user = get_user_by_email(conn, email)?
        .ok_or_else(|| format!("User with email '{}' not found", email))?;

    // Check if role exists
    let _role = get_role_by_name(conn, role_name)?
        .ok_or_else(|| format!("Role '{}' not found", role_name))?;

    // Check if user already has this role
    let current_roles = get_user_roles(conn, user.id)?;
    if current_roles.iter().any(|r| r.name == role_name) {
        println!("User '{}' already has role '{}'", email, role_name);
        return Ok(());
    }

    // Add the role
    assign_user_role_by_name(conn, user.id, role_name)?;
    println!("Successfully added role '{}' to user '{}'", role_name, email);

    Ok(())
}

pub fn user_rm_role_impl(
    conn: &mut SqliteConnection,
    email: &str,
    role_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists
    let user = get_user_by_email(conn, email)?
        .ok_or_else(|| format!("User with email '{}' not found", email))?;

    // Check if role exists
    let _role = get_role_by_name(conn, role_name)?
        .ok_or_else(|| format!("Role '{}' not found", role_name))?;

    // Check if user has this role
    let current_roles = get_user_roles(conn, user.id)?;
    if !current_roles.iter().any(|r| r.name == role_name) {
        println!("User '{}' does not have role '{}'", email, role_name);
        return Ok(());
    }

    // Check if this is the user's last role (enforce minimum 1 role constraint)
    if current_roles.len() <= 1 {
        return Err(format!(
            "Cannot remove role '{}' from user '{}': users must have at least one role",
            role_name, email
        )
        .into());
    }

    // Remove the role
    remove_user_role_by_name(conn, user.id, role_name)?;
    println!("Successfully removed role '{}' from user '{}'", role_name, email);

    Ok(())
}

pub fn user_set_roles_impl(
    conn: &mut SqliteConnection,
    email: &str,
    roles_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists
    let user = get_user_by_email(conn, email)?
        .ok_or_else(|| format!("User with email '{}' not found", email))?;

    // Parse roles from comma-separated string
    let role_names: Vec<&str> =
        roles_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    if role_names.is_empty() {
        return Err("Cannot set empty role list: users must have at least one role".into());
    }

    // Validate all roles exist
    for role_name in &role_names {
        let _role = get_role_by_name(conn, role_name)?
            .ok_or_else(|| format!("Role '{}' not found", role_name))?;
    }

    // Remove duplicates from role list
    let mut unique_roles: Vec<&str> = Vec::new();
    for role_name in role_names {
        if !unique_roles.contains(&role_name) {
            unique_roles.push(role_name);
        }
    }

    // Get current roles
    let _current_roles = get_user_roles(conn, user.id)?;

    // Remove all current roles
    remove_all_user_roles(conn, user.id)?;

    // Add new roles
    for role_name in &unique_roles {
        assign_user_role_by_name(conn, user.id, role_name)?;
    }

    println!("Successfully set roles for user '{}':", email);
    for role_name in unique_roles {
        println!("  - {}", role_name);
    }

    Ok(())
}

#[cfg(all(test, feature = "test-staging"))]
#[allow(unused_imports)]
mod tests {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    use neems_api::{
        models::CompanyInput,
        orm::{
            company::{get_company_by_name, insert_company},
            role::get_all_roles,
            testing::setup_test_db,
            user::{get_user, get_user_by_email, list_all_users},
        },
    };

    use super::*;
    use crate::admin_cli::{company_commands::CompanyAction, role_commands::RoleAction};

    #[test]
    fn test_handle_user_command_with_conn_add() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        let action = UserAction::Add {
            email: "cli_test@example.com".to_string(),
            password: Some("cli_password".to_string()),
            company_id: company.id.to_string(),
            totp_secret: Some("cli_totp".to_string()),
        };

        let result = handle_user_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());

        // Verify user was created
        let user = get_user_by_email(&mut conn, "cli_test@example.com")
            .expect("Failed to get CLI created user")
            .expect("User should exist");
        assert_eq!(user.email, "cli_test@example.com");
        assert_eq!(user.company_id, company.id);
    }

    #[test]
    fn test_handle_user_command_with_conn_change_password() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        // Create user first
        let create_action = UserAction::Add {
            email: "change_test@example.com".to_string(),
            password: Some("original".to_string()),
            company_id: company.id.to_string(),
            totp_secret: None,
        };
        handle_user_command_with_conn(&mut conn, create_action, 1).expect("Failed to create user");

        let original_user = get_user_by_email(&mut conn, "change_test@example.com")
            .expect("Failed to get user")
            .expect("User should exist");
        let original_hash = original_user.password_hash.clone();

        // Change password
        let change_action = UserAction::ChangePassword {
            email: "change_test@example.com".to_string(),
            password: Some("new_password".to_string()),
        };

        let result = handle_user_command_with_conn(&mut conn, change_action, 1);
        assert!(result.is_ok());

        // Verify password changed
        let updated_user = get_user_by_email(&mut conn, "change_test@example.com")
            .expect("Failed to get updated user")
            .expect("User should exist");
        assert_ne!(updated_user.password_hash, original_hash);
    }

    #[test]
    fn test_handle_user_command_with_conn_list() {
        let mut conn = setup_test_db();

        let action = UserAction::Ls { search_term: None, fixed_string: false };
        let result = handle_user_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_user_command_with_conn_rm() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        let create_action = UserAction::Add {
            email: "delete_me@example.com".to_string(),
            password: Some("password1".to_string()),
            company_id: company.id.to_string(),
            totp_secret: None,
        };
        handle_user_command_with_conn(&mut conn, create_action, 1).expect("Failed to create user");

        let create_action2 = UserAction::Add {
            email: "keep_me@test.com".to_string(),
            password: Some("password2".to_string()),
            company_id: company.id.to_string(),
            totp_secret: None,
        };
        handle_user_command_with_conn(&mut conn, create_action2, 1).expect("Failed to create user");

        let action = UserAction::Rm {
            search_term: "@example.com".to_string(),
            fixed_string: true,
            yes: true,
        };
        let result = handle_user_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());

        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 1);
        assert_eq!(remaining_users[0].email, "keep_me@test.com");
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
    fn test_add_user_impl() {
        let mut conn = setup_test_db();

        // Create a test company first
        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        // Test creating a user
        let result = add_user_impl(
            &mut conn,
            "test@example.com",
            Some("password123".to_string()),
            company.id,
            Some("totp_secret".to_string()),
            1,
        );

        assert!(result.is_ok());

        // Verify user was created by fetching it
        let created_user = get_user_by_email(&mut conn, "test@example.com")
            .expect("Failed to get created user")
            .expect("User should exist");

        assert_eq!(created_user.email, "test@example.com");
        assert_eq!(created_user.company_id, company.id);
        assert_eq!(created_user.totp_secret, Some("totp_secret".to_string()));

        // Verify password was hashed (not stored as plaintext)
        assert_ne!(created_user.password_hash, "password123");
        assert!(created_user.password_hash.starts_with("$argon2"));
    }

    #[test]
    fn test_add_user_impl_duplicate_email() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        // Create first user
        add_user_impl(
            &mut conn,
            "test@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create first user");

        // Try to create second user with same email - should now succeed gracefully
        let result = add_user_impl(
            &mut conn,
            "test@example.com",
            Some("password2".to_string()),
            company.id,
            None,
            1,
        );

        assert!(result.is_ok()); // Now handles duplicates gracefully
    }

    #[test]
    fn test_change_password_impl() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        // Create a user first
        add_user_impl(
            &mut conn,
            "test@example.com",
            Some("original_password".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        let original_user = get_user_by_email(&mut conn, "test@example.com")
            .expect("Failed to get user")
            .expect("User should exist");
        let original_hash = original_user.password_hash.clone();

        // Change password
        let result = change_password_impl(
            &mut conn,
            "test@example.com",
            Some("new_password".to_string()),
            1,
        );
        assert!(result.is_ok());

        // Verify password was changed
        let updated_user = get_user_by_email(&mut conn, "test@example.com")
            .expect("Failed to get updated user")
            .expect("User should exist");

        assert_ne!(updated_user.password_hash, original_hash);
        assert!(updated_user.password_hash.starts_with("$argon2"));

        // Verify new password works
        let argon2 = Argon2::default();
        let parsed_hash =
            PasswordHash::new(&updated_user.password_hash).expect("Failed to parse new hash");
        assert!(argon2.verify_password("new_password".as_bytes(), &parsed_hash).is_ok());
    }

    #[test]
    fn test_change_password_impl_nonexistent_user() {
        let mut conn = setup_test_db();

        let result = change_password_impl(
            &mut conn,
            "nonexistent@example.com",
            Some("password".to_string()),
            1,
        );
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

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        // Create a few users
        add_user_impl(
            &mut conn,
            "user1@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user1");
        add_user_impl(
            &mut conn,
            "user2@example.com",
            Some("password2".to_string()),
            company.id,
            None,
            1,
        )
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

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        add_user_impl(
            &mut conn,
            "alice@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user1");
        add_user_impl(
            &mut conn,
            "bob@test.com",
            Some("password2".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user2");
        add_user_impl(
            &mut conn,
            "charlie@example.org",
            Some("password3".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user3");

        let result = list_users_impl(&mut conn, Some("example\\.com$".to_string()), false);
        assert!(result.is_ok());

        let result = list_users_impl(&mut conn, Some("@test".to_string()), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_users_impl_with_fixed_string_search() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        add_user_impl(
            &mut conn,
            "user.with.dots@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user1");
        add_user_impl(
            &mut conn,
            "normaluser@test.com",
            Some("password2".to_string()),
            company.id,
            None,
            1,
        )
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

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        add_user_impl(
            &mut conn,
            "user@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        let result = list_users_impl(&mut conn, Some("nonexistent".to_string()), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_users_impl_with_regex() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        add_user_impl(
            &mut conn,
            "alice@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user1");
        add_user_impl(
            &mut conn,
            "bob@test.com",
            Some("password2".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user2");
        add_user_impl(
            &mut conn,
            "charlie@example.org",
            Some("password3".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user3");

        let result = remove_users_impl(&mut conn, "example\\.com$".to_string(), false, true, 1);
        assert!(result.is_ok());

        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 2);
        assert_eq!(remaining_users[0].email, "bob@test.com");
        assert_eq!(remaining_users[1].email, "charlie@example.org");
    }

    #[test]
    fn test_remove_users_impl_with_fixed_string() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        add_user_impl(
            &mut conn,
            "user.with.dots@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user1");
        add_user_impl(
            &mut conn,
            "normaluser@test.com",
            Some("password2".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user2");

        let result = remove_users_impl(&mut conn, ".with.".to_string(), true, true, 1);
        assert!(result.is_ok());

        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 1);
        assert_eq!(remaining_users[0].email, "normaluser@test.com");
    }

    #[test]
    fn test_remove_users_impl_no_matches() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        add_user_impl(
            &mut conn,
            "user@example.com",
            Some("password1".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        let result = remove_users_impl(&mut conn, "nonexistent".to_string(), false, true, 1);
        assert!(result.is_ok());

        let remaining_users = list_all_users(&mut conn).expect("Failed to list users");
        assert_eq!(remaining_users.len(), 1);
    }

    #[test]
    fn test_remove_users_impl_invalid_regex() {
        let mut conn = setup_test_db();

        let result = remove_users_impl(&mut conn, "[invalid".to_string(), false, true, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_change_password_impl_with_provided_password() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        add_user_impl(
            &mut conn,
            "password_test@example.com",
            Some("original_password".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        let original_user = get_user_by_email(&mut conn, "password_test@example.com")
            .expect("Failed to get user")
            .expect("User should exist");
        let original_hash = original_user.password_hash.clone();

        let result = change_password_impl(
            &mut conn,
            "password_test@example.com",
            Some("new_password".to_string()),
            1,
        );
        assert!(result.is_ok());

        let updated_user = get_user_by_email(&mut conn, "password_test@example.com")
            .expect("Failed to get updated user")
            .expect("User should exist");

        assert_ne!(updated_user.password_hash, original_hash);
        assert!(updated_user.password_hash.starts_with("$argon2"));
    }

    #[test]
    fn test_add_user_impl_with_provided_password() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        let result = add_user_impl(
            &mut conn,
            "create_test@example.com",
            Some("test_password".to_string()),
            company.id,
            None,
            1,
        );
        assert!(result.is_ok());

        let created_user = get_user_by_email(&mut conn, "create_test@example.com")
            .expect("Failed to get created user")
            .expect("User should exist");

        assert_eq!(created_user.email, "create_test@example.com");
        assert_eq!(created_user.company_id, company.id);
        assert!(created_user.password_hash.starts_with("$argon2"));
    }

    #[test]
    fn test_user_edit_impl() {
        let mut conn = setup_test_db();

        let company1 = insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to create company 2");

        // Create a user
        add_user_impl(
            &mut conn,
            "original@example.com",
            Some("password".to_string()),
            company1.id,
            Some("original_totp".to_string()),
            1,
        )
        .expect("Failed to create user");

        let user = get_user_by_email(&mut conn, "original@example.com")
            .expect("Failed to get user")
            .expect("User should exist");

        // Edit email
        let result = user_edit_impl(
            &mut conn,
            user.id,
            Some("updated@example.com".to_string()),
            None,
            None,
            1,
        );
        assert!(result.is_ok());

        let updated_user = get_user(&mut conn, user.id)
            .expect("Failed to get updated user")
            .expect("User should exist");
        assert_eq!(updated_user.email, "updated@example.com");
        assert_eq!(updated_user.company_id, company1.id);

        // Edit company and TOTP
        let result = user_edit_impl(
            &mut conn,
            user.id,
            None,
            Some(company2.id),
            Some("new_totp".to_string()),
            1,
        );
        assert!(result.is_ok());

        let updated_user = get_user(&mut conn, user.id)
            .expect("Failed to get updated user")
            .expect("User should exist");
        assert_eq!(updated_user.company_id, company2.id);
        assert_eq!(updated_user.totp_secret, Some("new_totp".to_string()));
    }

    #[test]
    fn test_user_edit_impl_nonexistent_user() {
        let mut conn = setup_test_db();

        let result =
            user_edit_impl(&mut conn, 99999, Some("new@example.com".to_string()), None, None, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_edit_impl_nonexistent_company() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        add_user_impl(
            &mut conn,
            "user@example.com",
            Some("password".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        let user = get_user_by_email(&mut conn, "user@example.com")
            .expect("Failed to get user")
            .expect("User should exist");

        let result = user_edit_impl(&mut conn, user.id, None, Some(99999), None, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_add_role_impl() {
        let mut conn = setup_test_db();

        let company =
            get_company_by_name(&mut conn, &CompanyInput { name: "Newtown Energy".to_string() })
                .expect("Failed to query company")
                .expect("Newtown Energy company should exist");

        add_user_impl(
            &mut conn,
            "test@example.com",
            Some("password".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        let result = user_add_role_impl(&mut conn, "test@example.com", "newtown-staff");
        assert!(result.is_ok());
    }

    #[test]
    fn test_user_set_roles_impl() {
        let mut conn = setup_test_db();

        let company =
            get_company_by_name(&mut conn, &CompanyInput { name: "Newtown Energy".to_string() })
                .expect("Failed to query company")
                .expect("Newtown Energy company should exist");

        add_user_impl(
            &mut conn,
            "test2@example.com",
            Some("password".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        let result =
            user_set_roles_impl(&mut conn, "test2@example.com", "newtown-admin,newtown-staff");
        assert!(result.is_ok());
    }

    #[test]
    fn test_user_rm_role_impl_last_role_fails() {
        let mut conn = setup_test_db();

        let company =
            get_company_by_name(&mut conn, &CompanyInput { name: "Newtown Energy".to_string() })
                .expect("Failed to query company")
                .expect("Newtown Energy company should exist");

        add_user_impl(
            &mut conn,
            "test3@example.com",
            Some("password".to_string()),
            company.id,
            None,
            1,
        )
        .expect("Failed to create user");

        // First assign a role
        user_add_role_impl(&mut conn, "test3@example.com", "newtown-admin")
            .expect("Failed to add role");

        // Try to remove the only role - should fail
        let result = user_rm_role_impl(&mut conn, "test3@example.com", "newtown-admin");
        assert!(result.is_err());
    }
}
