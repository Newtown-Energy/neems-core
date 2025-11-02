use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use diesel::{prelude::*, sqlite::SqliteConnection};
use dotenvy::dotenv;
use neems_api::{
    models::{CompanyInput, UserInput},
    orm::{
        company::{get_company_by_id, get_company_by_name, get_company_by_name_case_insensitive},
        user::{get_user_by_email, insert_user},
        user_role::assign_user_role_by_name,
    },
};

pub fn establish_connection() -> Result<SqliteConnection, Box<dyn std::error::Error>> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = SqliteConnection::establish(&database_url)?;
    Ok(conn)
}

/// Get or create an admin user for the current system user.
/// This admin user will be used to track who made changes in the database.
pub fn get_or_create_admin_user(
    conn: &mut SqliteConnection,
) -> Result<i32, Box<dyn std::error::Error>> {
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
    let password_hash = argon2
        .hash_password(b"unused-password", &salt)
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

/// Resolve a company identifier (either ID as string/number or name) to a
/// company ID. If the input is a valid number, treat it as an ID and verify it
/// exists. If it's not a number, treat it as a name and look it up
/// (case-insensitive).
pub fn resolve_company_id(
    conn: &mut SqliteConnection,
    company_identifier: &str,
) -> Result<i32, Box<dyn std::error::Error>> {
    // Try to parse as a number first
    if let Ok(id) = company_identifier.parse::<i32>() {
        // It's a number, verify the company exists
        match get_company_by_id(conn, id)? {
            Some(_company) => Ok(id),
            None => Err(format!("Company with ID {} does not exist", id).into()),
        }
    } else {
        // It's not a number, treat as name
        match get_company_by_name_case_insensitive(conn, company_identifier)? {
            Some(company) => Ok(company.id),
            None => {
                Err(format!("Company with name '{}' does not exist", company_identifier).into())
            }
        }
    }
}
