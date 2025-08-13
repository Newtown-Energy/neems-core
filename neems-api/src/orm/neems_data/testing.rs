//! Testing utilities for neems-data database connections.
//!
//! This module provides helper functions for setting up test rocket instances
//! that can access the neems-data database (SiteDbConn) for testing purposes.

use rocket::figment::{
    util::map,
    value::{Map, Value},
};
use rocket::{Build, Rocket};
use uuid::Uuid;

use crate::orm::{DbConn, SiteDbConn, set_foreign_keys_fairing, run_migrations_fairing};
use crate::orm::neems_data::set_foreign_keys_fairing as site_set_foreign_keys_fairing;
use crate::admin_init_fairing::admin_init_fairing;

/// Creates a test rocket instance configured to use test-data.db for the site database.
/// 
/// This function sets up a Rocket instance with:
/// - Main API database: In-memory SQLite for isolation
/// - Site database: Uses existing test-data.db file for real data testing
/// - All necessary fairings for database connections and migrations
/// 
/// The test-data.db file location is determined by:
/// 1. Using SITE_DATABASE_URL environment variable to find the directory
/// 2. Replacing the filename with "test-data.db"
/// 3. Falling back to current directory if SITE_DATABASE_URL is not set
/// 
/// # Panics
/// - If test-data.db file doesn't exist at the expected location
/// - If unable to connect to the test database
/// - If the sources table is not accessible in the test database
/// 
/// # Returns
/// A configured Rocket instance ready for testing with real site data
pub fn test_rocket_with_site_db() -> Rocket<Build> {
    // Configure the main API database (temporary file for isolation with unsafe fast pragmas)
    let temp_db_path = std::env::temp_dir().join(format!("test_db_{}.sqlite", Uuid::new_v4()));
    let unique_db_name = format!("sqlite://{}?synchronous=OFF&journal_mode=OFF&locking_mode=EXCLUSIVE&temp_store=MEMORY&cache_size=-64000", temp_db_path.display());
    let db_config: Map<_, Value> = map! {
        "url" => unique_db_name.into(),
        "pool_size" => 5.into(),
        "timeout" => 5.into(),
    };

    // Create database config map with main database
    let mut databases = map!["sqlite_db" => db_config];
    
    // Configure site_db to use the existing test-data.db file
    // Use SITE_DATABASE_URL to find the correct directory, then replace filename with test-data.db
    let test_db_path = if let Ok(site_db_url) = std::env::var("SITE_DATABASE_URL") {
        // Parse the URL to get the path, then replace filename
        let path = site_db_url.strip_prefix("sqlite://").unwrap_or(&site_db_url);
        let path = std::path::Path::new(path);
        if let Some(parent) = path.parent() {
            parent.join("test-data.db")
        } else {
            std::path::PathBuf::from("test-data.db")
        }
    } else {
        // Fallback to current directory
        std::env::current_dir()
            .expect("current directory")
            .join("test-data.db")
    };
    
    // Debug: Print the path we're trying to use
    println!("Looking for test-data.db at: {}", test_db_path.display());
    println!("File exists: {}", test_db_path.exists());
    
    // Panic in tests if the database file doesn't exist
    if !test_db_path.exists() {
        panic!("Test database file not found at: {}. Please ensure test-data.db exists.", test_db_path.display());
    }
    
    // Test database connection and schema in tests with retry logic
    {
        use diesel::prelude::*;
        use diesel::sqlite::SqliteConnection;
        use std::thread::sleep;
        use std::time::Duration;
        
        let mut retries = 0;
        let max_retries = 3;
        
        loop {
            match SqliteConnection::establish(&format!("sqlite://{}", test_db_path.display())) {
                Ok(mut conn) => {
                    // Verify the sources table exists by trying to query it
                    match diesel::sql_query("SELECT COUNT(*) FROM sources").execute(&mut conn) {
                        Ok(_) => {
                            println!("✓ Sources table found and accessible in test database");
                            break;
                        }
                        Err(e) => {
                            let error_msg = format!("{}", e);
                            if error_msg.contains("database is locked") && retries < max_retries {
                                retries += 1;
                                println!("Database locked during verification, retrying ({}/{})", retries, max_retries);
                                sleep(Duration::from_millis(100 * retries as u64));
                            } else {
                                panic!("Sources table not accessible in test database at {}: {}", test_db_path.display(), e);
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    if error_msg.contains("database is locked") && retries < max_retries {
                        retries += 1;
                        println!("Database locked during connection, retrying ({}/{})", retries, max_retries);
                        sleep(Duration::from_millis(100 * retries as u64));
                    } else {
                        panic!("Failed to connect to test database at: {} - {}", test_db_path.display(), e);
                    }
                }
            }
        }
    }
    
    let test_db_url = format!("sqlite://{}", test_db_path.display());
    
    let site_db_config: Map<_, Value> = map! {
        "url" => test_db_url.into(),
        "pool_size" => 5.into(),
        "timeout" => 5.into(),
    };
    databases.insert("site_db", site_db_config.into());

    // Merge DB config into Rocket's figment
    let figment = rocket::Config::figment().merge(("databases", databases));

    // Build the Rocket instance with both DB fairings attached
    let rocket = rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(SiteDbConn::fairing())
        .attach(set_foreign_keys_fairing())
        .attach(site_set_foreign_keys_fairing())
        .attach(run_migrations_fairing())
        .attach(admin_init_fairing());
    
    crate::mount_api_routes(rocket)
}