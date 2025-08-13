#![cfg(feature = "test-staging")]

use diesel::connection::SimpleConnection;
use diesel::sqlite::SqliteConnection;
use rocket::figment::{
    util::map,
    value::{Map, Value},
};
use rocket::{Build, Rocket, fairing::AdHoc};
use rocket_sync_db_pools::diesel;
use std::path::PathBuf;

use super::db::{DbConn, run_pending_migrations, set_foreign_keys};
use crate::admin_init_fairing::admin_init_fairing;

/// Creates a golden database template with all test data pre-populated.
/// This is created once and then copied for each test that needs it.
/// The golden database is identified by timestamp and located automatically.




/// Gets the golden database path by finding the most recent timestamp-based golden database
fn get_golden_db_path() -> PathBuf {
    use std::fs;
    
    let target_dir = PathBuf::from("../target");
    
    if let Ok(entries) = fs::read_dir(&target_dir) {
        let mut golden_dbs: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("golden_test_") && name.ends_with(".db"))
                    .unwrap_or(false)
            })
            .collect();
            
        // Sort by filename (which includes timestamp) to get the most recent
        golden_dbs.sort();
        
        if let Some(latest) = golden_dbs.last() {
            return latest.clone();
        }
    }
    
    // Fallback: return a path that won't exist to trigger the error in fast_test_rocket
    PathBuf::from("../target/golden_test_not_found.db")
}

use diesel::prelude::*;
use crate::models::{CompanyInput, NewRole, NewUserRole, Role, User, UserInput};
use crate::orm::company::{get_company_by_name, insert_company};
use crate::orm::login::hash_password;
use crate::orm::role::{get_role_by_name, insert_role};
use crate::orm::user::insert_user;
use crate::schema::{user_roles, users};

/// Configures SQLite with performance-optimized settings for testing.
///
/// Sets the following PRAGMAs:
/// - `synchronous = OFF`: Disables synchronous writes for faster performance
/// - `journal_mode = OFF`: Disables rollback journal
///
/// These settings make SQLite faster but less durable - only use for testing.
///
/// # Arguments
/// * `conn` - A mutable reference to a SQLite database connection
///
/// # Panics
/// Panics if the PRAGMA commands fail to execute
fn set_sqlite_test_pragmas(conn: &mut diesel::SqliteConnection) {
    conn.batch_execute(
        r#"
        PRAGMA synchronous = OFF;
        PRAGMA journal_mode = OFF;
        "#,
    )
    .expect("Failed to set SQLite PRAGMAs");
}

/// Creates a Rocket fairing that sets SQLite testing pragmas.
///
/// This fairing configures SQLite for faster but less durable operation,
/// suitable only for testing environments.
fn set_sqlite_test_pragmas_fairing() -> AdHoc {
    AdHoc::on_ignite("Set SQLite Test Pragmas", |rocket| async {
        let conn = DbConn::get_one(&rocket)
            .await
            .expect("database connection for migration");
        conn.run(|c| {
            set_sqlite_test_pragmas(c);
        })
        .await;
        rocket
    })
}

/// Creates a Rocket fairing that initializes standard test data.
///
/// This fairing creates a consistent set of companies, users, and sites that all tests can rely on.
/// It only runs when the `test-staging` feature is enabled to ensure it never runs in production.
fn test_data_init_fairing() -> AdHoc {
    AdHoc::on_ignite("Test Data Initialization", |rocket| async {
        let conn = DbConn::get_one(&rocket)
            .await
            .expect("database connection for test data initialization");
        
        conn.run(|c| {
            if let Err(e) = create_test_data(c) {
                eprintln!("[test-data-init] ERROR: Failed to create test data: {:?}", e);
            } else {
                eprintln!("[test-data-init] Test data initialization completed");
            }
        }).await;
        
        rocket
    })
}

/// Creates standard test data for all tests to use.
fn create_test_data(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    // Create test companies
    let test_company1 = find_or_create_company(conn, "Test Company 1")?;
    let test_company2 = find_or_create_company(conn, "Test Company 2")?;
    let _removable_company = find_or_create_company(conn, "Removable LLC")?;
    
    // Get the Newtown Energy company (created by admin_init_fairing)
    let newtown_company = get_company_by_name(conn, &CompanyInput { name: "Newtown Energy".to_string() })?
        .expect("Newtown Energy company should exist from admin_init_fairing");

    // Create standard roles if they don't exist
    ensure_role_exists(conn, "admin", "Administrator role")?;
    ensure_role_exists(conn, "staff", "Staff role")?;
    ensure_role_exists(conn, "newtown-admin", "Newtown Administrator role")?;
    ensure_role_exists(conn, "newtown-staff", "Newtown Staff role")?;

    // Create standard test users
    create_test_user(conn, "user@testcompany.com", test_company1.id, "admin")?;
    create_test_user(conn, "user@company1.com", test_company1.id, "admin")?;
    create_test_user(conn, "user@company2.com", test_company2.id, "admin")?;
    create_test_user(conn, "user@empty.com", test_company1.id, "admin")?;
    create_test_user(conn, "admin@company1.com", test_company1.id, "admin")?;
    create_test_user(conn, "admin@company2.com", test_company2.id, "admin")?;
    create_test_user(conn, "staff@testcompany.com", test_company1.id, "staff")?;
    create_test_user(conn, "newtownadmin@newtown.com", newtown_company.id, "newtown-admin")?;
    create_test_user(conn, "newtownstaff@newtown.com", newtown_company.id, "newtown-staff")?;
    
    // Additional test users for login.rs tests
    create_test_user(conn, "testuser@example.com", test_company1.id, "staff")?;
    
    // Additional test users for secure_test.rs tests
    create_test_user_with_password(conn, "test_superadmin@example.com", newtown_company.id, "admin", "adminpass")?;
    create_test_user_with_password(conn, "staff@example.com", test_company1.id, "staff", "staffpass")?;
    create_test_user_with_password_and_roles(conn, "admin_staff@example.com", test_company1.id, &["admin", "staff"], "adminstaff")?;
    create_test_user_with_password(conn, "newtown_superadmin@example.com", newtown_company.id, "newtown-admin", "newtownpass")?;
    create_test_user_with_password(conn, "newtown_staff@example.com", newtown_company.id, "newtown-staff", "newtownstaffpass")?;
    create_test_user_with_password(conn, "regular@example.com", test_company1.id, "staff", "regularpass")?;

    Ok(())
}

/// Creates a test user with a custom password with the specified email, company, role, and password.
fn create_test_user_with_password(conn: &mut SqliteConnection, email: &str, company_id: i32, role_name: &str, password: &str) -> Result<(), diesel::result::Error> {
    // Check if user already exists
    let existing_user = users::table
        .filter(users::email.eq(email))
        .first::<User>(conn)
        .optional()?;
        
    if existing_user.is_some() {
        println!("[test-data-init] User '{}' already exists", email);
        return Ok(());
    }

    // Create user with custom password hash
    let password_hash = hash_password(password);
    let user_input = UserInput {
        email: email.to_string(),
        password_hash,
        company_id,
        totp_secret: None,
    };

    let user = insert_user(conn, user_input)?;
    println!("[test-data-init] Created user with custom password: '{}'", email);

    // Assign role to user
    let role = get_role_by_name(conn, role_name)?
        .expect(&format!("Role '{}' should exist", role_name));
    
    let new_user_role = NewUserRole {
        user_id: user.id,
        role_id: role.id,
    };

    diesel::insert_into(user_roles::table)
        .values(&new_user_role)
        .execute(conn)?;
    
    println!("[test-data-init] Assigned role '{}' to user '{}'", role_name, email);
    
    Ok(())
}

/// Creates a test user with a custom password and multiple roles.
fn create_test_user_with_password_and_roles(conn: &mut SqliteConnection, email: &str, company_id: i32, role_names: &[&str], password: &str) -> Result<(), diesel::result::Error> {
    // Check if user already exists
    let existing_user = users::table
        .filter(users::email.eq(email))
        .first::<User>(conn)
        .optional()?;
        
    if existing_user.is_some() {
        println!("[test-data-init] User '{}' already exists", email);
        return Ok(());
    }

    // Create user with custom password hash
    let password_hash = hash_password(password);
    let user_input = UserInput {
        email: email.to_string(),
        password_hash,
        company_id,
        totp_secret: None,
    };

    let user = insert_user(conn, user_input)?;
    println!("[test-data-init] Created user with custom password: '{}'", email);

    // Assign multiple roles to user
    for role_name in role_names {
        let role = get_role_by_name(conn, role_name)?
            .expect(&format!("Role '{}' should exist", role_name));
        
        let new_user_role = NewUserRole {
            user_id: user.id,
            role_id: role.id,
        };

        diesel::insert_into(user_roles::table)
            .values(&new_user_role)
            .execute(conn)?;
        
        println!("[test-data-init] Assigned role '{}' to user '{}'", role_name, email);
    }
    
    Ok(())
}

/// Finds or creates a company with the given name.
fn find_or_create_company(conn: &mut SqliteConnection, name: &str) -> Result<crate::models::Company, diesel::result::Error> {
    let company_input = CompanyInput { name: name.to_string() };
    
    match get_company_by_name(conn, &company_input)? {
        Some(company) => {
            eprintln!("[test-data-init] Found existing company: '{}'", name);
            Ok(company)
        },
        None => {
            eprintln!("[test-data-init] Creating company: '{}'", name);
            insert_company(conn, name.to_string())
        }
    }
}

/// Ensures a role exists, creating it if necessary.
fn ensure_role_exists(conn: &mut SqliteConnection, role_name: &str, description: &str) -> Result<Role, diesel::result::Error> {
    match get_role_by_name(conn, role_name)? {
        Some(role) => Ok(role),
        None => {
            eprintln!("[test-data-init] Creating role: '{}'", role_name);
            let new_role = NewRole {
                name: role_name.to_string(),
                description: Some(description.to_string()),
            };
            insert_role(conn, new_role)
        }
    }
}

/// Creates a test user with the specified email, company, and role.
fn create_test_user(conn: &mut SqliteConnection, email: &str, company_id: i32, role_name: &str) -> Result<(), diesel::result::Error> {
    // Check if user already exists
    let existing_user = users::table
        .filter(users::email.eq(email))
        .first::<User>(conn)
        .optional()?;
        
    if existing_user.is_some() {
        println!("[test-data-init] User '{}' already exists", email);
        return Ok(());
    }

    // Create user with consistent password hash
    let password_hash = hash_password("admin");
    let user_input = UserInput {
        email: email.to_string(),
        password_hash,
        company_id,
        totp_secret: None,
    };

    let user = insert_user(conn, user_input)?;
    println!("[test-data-init] Created user: '{}'", email);

    // Assign role to user
    let role = get_role_by_name(conn, role_name)?
        .expect(&format!("Role '{}' should exist", role_name));
    
    let new_user_role = NewUserRole {
        user_id: user.id,
        role_id: role.id,
    };

    diesel::insert_into(user_roles::table)
        .values(&new_user_role)
        .execute(conn)?;
    
    println!("[test-data-init] Assigned role '{}' to user '{}'", role_name, email);
    
    Ok(())
}

/// Creates and configures a Rocket instance for testing with an in-memory SQLite database.
///
/// The returned Rocket instance will have:
/// - An in-memory SQLite database configured
/// - Database connection pool attached
/// - Foreign keys enabled
/// - Testing pragmas set
/// - All migrations run
/// - Admin initialization completed
/// - API routes mounted
pub fn test_rocket() -> Rocket<Build> {
    use uuid::Uuid;

    // Generate a unique database name for this test instance
    let unique_db_name = format!("file:test_db_{}?mode=memory&cache=shared", Uuid::new_v4());

    // Configure the in-memory SQLite database
    let db_config: Map<_, Value> = map! {
        "url" => unique_db_name.into(),  // Unique shared in-memory DB per test
        "pool_size" => 5.into(),
        "timeout" => 5.into(),
    };

    // Create database config map
    let mut databases = map!["sqlite_db" => db_config.clone()];
    
    // Add site_db configuration when test-staging feature is enabled
    {
        let site_unique_db_name = format!("file:test_site_db_{}?mode=memory&cache=shared", Uuid::new_v4());
        let site_db_config: Map<_, Value> = map! {
            "url" => site_unique_db_name.into(),
            "pool_size" => 5.into(),
            "timeout" => 5.into(),
        };
        databases.insert("site_db", site_db_config.into());
    }

    // Merge DB config into Rocket's figment
    let figment = rocket::Config::figment().merge(("databases", databases));

    // Build the Rocket instance with the DB fairing attached
    let mut rocket = rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(super::db::set_foreign_keys_fairing())
        .attach(set_sqlite_test_pragmas_fairing())
        .attach(super::db::run_migrations_fairing())
        .attach(admin_init_fairing());

    // Attach test data initialization fairing when test-staging feature is enabled
    {
        rocket = rocket.attach(test_data_init_fairing());
    }
    
    // Attach SiteDbConn fairing when test-staging feature is enabled
    {
        rocket = rocket.attach(super::neems_data::db::SiteDbConn::fairing())
                      .attach(super::neems_data::db::set_foreign_keys_fairing());
    }
    
    crate::mount_api_routes(rocket)
}

/// Creates a fast Rocket instance for testing by copying a pre-populated golden database.
/// This is much faster than test_rocket() because it skips all the initialization fairings.
/// 
/// Use this for tests that don't need to modify the core test data structure.
pub fn fast_test_rocket() -> Rocket<Build> {
    use uuid::Uuid;
    
    // Get the golden database template
    let golden_db_path = get_golden_db_path();
    
    // Check if golden database exists
    if !golden_db_path.exists() {
        panic!(
            "Golden database not found at {:?}. Run './bin/create-golden-db.sh' first.",
            golden_db_path
        );
    }
    
    // Create a unique copy for this test in the workspace target directory
    let test_db_path = PathBuf::from(format!("../target/test_db_{}.db", Uuid::new_v4()));
    
    // Copy the golden database - this creates a brand new file with no existing connections
    std::fs::copy(&golden_db_path, &test_db_path)
        .expect("Failed to copy golden database");
    
    // Verify the copied database exists
    if !test_db_path.exists() {
        panic!("Copied test database does not exist at: {:?}", test_db_path);
    }
    
    println!("[fast-test] Copied golden DB to: {:?}", test_db_path);
    
    // Use the absolute path directly without file: prefix (like test_rocket uses bare paths)
    let absolute_path = std::fs::canonicalize(&test_db_path)
        .expect("Failed to get absolute path for test database");
    let db_url = absolute_path.to_string_lossy().to_string();
    println!("[fast-test] Database URL will be: {}", db_url);
    let db_config: Map<_, Value> = map! {
        "url" => db_url.clone().into(),
        "pool_size" => 5.into(),     // Match test_rocket exactly
        "timeout" => 5.into(),       // Match test_rocket timeout
    };
    
    // Create site database config (using in-memory for fast tests)
    let site_unique_db_name = format!("file:test_site_db_{}?mode=memory&cache=shared", Uuid::new_v4());
    let site_db_config: Map<_, Value> = map! {
        "url" => site_unique_db_name.into(),
        "pool_size" => 5.into(),
        "timeout" => 5.into(),
    };
    
    // Create database config map with both main and site databases
    let databases = map![
        "sqlite_db" => db_config.clone(),
        "site_db" => site_db_config
    ];
    
    // Merge DB config into Rocket's figment
    let figment = rocket::Config::figment().merge(("databases", databases));
    
    // Build the Rocket instance with minimal fairings (no initialization fairings needed!)
    // The golden database is already fully set up, so we only need basic database connections
    let rocket = rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(super::neems_data::db::SiteDbConn::fairing());
        
    crate::mount_api_routes(rocket)
}

/// Creates a synchronous in-memory SQLite database connection for unit tests.
///
/// This function returns a `diesel::SqliteConnection` connected to an in-memory SQLite database,
/// runs all embedded Diesel migrations, and enables foreign key support. This is ideal for
/// direct Diesel queries in synchronous test code.
///
/// Each call to this function returns a new, independent in-memory database.
pub fn setup_test_db() -> SqliteConnection {
    use diesel::Connection;

    let mut conn = SqliteConnection::establish(":memory:")
        .expect("Failed to create in-memory SQLite database");
    set_foreign_keys(&mut conn);
    run_pending_migrations(&mut conn);
    conn
}

/// A minimal async-compatible wrapper for a synchronous SQLite connection for unit testing.
///
/// This helper struct and function allow you to use your test database with code that expects
/// a Rocket-style async `.run()` interface (such as functions that take a `DbConn`).
///
/// Unlike `setup_test_db()`, which returns a synchronous Diesel connection for direct use,
/// `setup_test_dbconn()` returns a `FakeDbConn` that can be used with async code expecting
/// a `.run()` method.
///
/// Both use the same in-memory database if you only call `setup_test_db()` once and wrap the result.
/// Each call to `setup_test_dbconn()` creates a new, independent in-memory database.
pub struct FakeDbConn<'a>(pub &'a mut diesel::SqliteConnection);

impl<'a> FakeDbConn<'a> {
    /// Executes a closure with a mutable reference to the underlying SQLite connection.
    ///
    /// This method mimics the async `.run()` interface used by Rocket's database connections,
    /// but operates synchronously for testing purposes.
    ///
    /// # Arguments
    /// * `f` - A closure that takes a mutable reference to the SQLite connection
    ///
    /// # Safety
    /// This uses unsafe code to convert an immutable reference to mutable, which is safe
    /// in this controlled test environment where we know we have exclusive access.
    pub async fn run<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut diesel::SqliteConnection) -> R + Send + 'static,
        R: Send + 'static,
    {
        // Safety: We need to get a mutable reference from an immutable reference
        // This is safe because we're in a test environment and we control the lifetime
        unsafe {
            let conn_ptr =
                self.0 as *const diesel::SqliteConnection as *mut diesel::SqliteConnection;
            f(&mut *conn_ptr)
        }
    }
}

/// Creates a `FakeDbConn` for async-style testing with the given SQLite connection.
///
/// This is useful for testing code that expects a Rocket-style `.run()` interface,
/// but you want to use your in-memory test database.
///
/// # Arguments
/// * `conn` - A mutable reference to a `diesel::SqliteConnection` (typically from `setup_test_db()`)
///
/// # Returns
/// A `FakeDbConn` wrapping the provided connection
pub fn setup_test_dbconn<'a>(conn: &'a mut diesel::SqliteConnection) -> FakeDbConn<'a> {
    FakeDbConn(conn)
}

/// Creates a minimal Rocket instance for testing APIs that don't require a database.
///
/// This is useful for testing endpoints that don't need database access, avoiding
/// potential database conflicts and improving test performance.
///
/// The returned Rocket instance will have:
/// - Only fixphrase API routes mounted (if fixphrase feature is enabled)
/// - No database connection
/// - No database-related fairings
#[cfg(feature = "fixphrase")]
pub fn test_rocket_no_db() -> Rocket<Build> {
    rocket::build().mount("/api", crate::api::fixphrase::routes())
}
