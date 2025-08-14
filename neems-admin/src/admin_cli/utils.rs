use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenvy::dotenv;
use neems_api::models::{CompanyInput, UserInput};
use neems_api::orm::company::get_company_by_name;
use neems_api::orm::user::{get_user_by_email, insert_user};
use neems_api::orm::user_role::assign_user_role_by_name;
use argon2::password_hash::{SaltString, rand_core::OsRng};
use argon2::{Argon2, PasswordHasher};

pub fn establish_connection() -> Result<SqliteConnection, Box<dyn std::error::Error>> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = SqliteConnection::establish(&database_url)?;
    Ok(conn)
}

/// Get or create an admin user for the current system user.
/// This admin user will be used to track who made changes in the database.
pub fn get_or_create_admin_user(conn: &mut SqliteConnection) -> Result<i32, Box<dyn std::error::Error>> {
    // Get the current system username
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "admin".to_string());
    
    let email = format!("{}@localhost", username);
    
    // Check if user already exists
    if let Some(existing_user) = get_user_by_email(conn, &email)? {
        return Ok(existing_user.id);
    }
    
    // Get the Newtown Energy company
    let company_input = CompanyInput { name: "Newtown Energy".to_string() };
    let company = get_company_by_name(conn, &company_input)?
        .ok_or("Newtown Energy company not found in database")?;
    
    // Generate a random password hash (won't be used for login)
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(b"unused-password", &salt)
        .map_err(|e| format!("Failed to hash password: {}", e))?
        .to_string();
    
    // Create the user
    let new_user = UserInput {
        email: email.clone(),
        password_hash,
        company_id: company.id,
        totp_secret: None,
    };
    
    let created_user = insert_user(conn, new_user, None)?;
    
    // Assign the newtown-staff role
    assign_user_role_by_name(conn, created_user.id, "newtown-staff")?;
    
    println!("Created admin user: {} (ID: {})", email, created_user.id);
    
    Ok(created_user.id)
}
