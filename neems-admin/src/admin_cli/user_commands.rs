use clap::Subcommand;
use diesel::sqlite::SqliteConnection;
use neems_core::orm::user::{insert_user, list_all_users, get_user_by_email, update_user, delete_user_with_cleanup, get_user};
use neems_core::orm::company::get_company_by_id;
use neems_core::orm::user_role::{assign_user_role_by_name, remove_user_role_by_name, get_user_roles, remove_all_user_roles};
use neems_core::orm::role::get_role_by_name;
use neems_core::models::UserNoTime;
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use regex::Regex;
use std::io::{self, Write};
use rpassword::read_password;

#[derive(Subcommand)]
pub enum UserAction {
    #[command(about = "Add a new user")]
    Add {
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
    #[command(about = "Edit user fields")]
    Edit {
        #[arg(short, long, help = "User ID to edit")]
        id: i32,
        #[arg(long, help = "New email address")]
        email: Option<String>,
        #[arg(long, help = "New company ID")]
        company_id: Option<i32>,
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
    action: UserAction
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        UserAction::Add {
            email,
            password,
            company_id,
            totp_secret,
        } => {
            add_user_impl(conn, &email, password, company_id, totp_secret)?;
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
        UserAction::Edit { id, email, company_id, totp_secret } => {
            user_edit_impl(conn, id, email, company_id, totp_secret)?;
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

pub fn change_password_impl(
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

pub fn list_users_impl(
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

pub fn remove_users_impl(
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

pub fn user_edit_impl(
    conn: &mut SqliteConnection,
    user_id: i32,
    new_email: Option<String>,
    new_company_id: Option<i32>,
    new_totp_secret: Option<String>,
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
    
    let updated_user = update_user(conn, user_id, new_email, None, new_company_id, new_totp_secret)?;
    
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
    let user = get_user_by_email(conn, email)
        .map_err(|_| format!("User with email '{}' not found", email))?;

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
    let user = get_user_by_email(conn, email)
        .map_err(|_| format!("User with email '{}' not found", email))?;

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
        return Err(format!("Cannot remove role '{}' from user '{}': users must have at least one role", role_name, email).into());
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
    let user = get_user_by_email(conn, email)
        .map_err(|_| format!("User with email '{}' not found", email))?;

    // Parse roles from comma-separated string
    let role_names: Vec<&str> = roles_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    
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