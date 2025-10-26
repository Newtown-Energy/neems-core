use std::path::PathBuf;

use diesel::{connection::SimpleConnection, sqlite::SqliteConnection};
use rocket::{
    Build, Rocket,
    fairing::AdHoc,
    figment::{
        util::map,
        value::{Map, Value},
    },
};
use rocket_sync_db_pools::diesel;

use super::db::{DbConn, run_pending_migrations, set_foreign_keys};
use crate::admin_init_fairing::admin_init_fairing;

/// Creates a golden database template with all test data pre-populated.
/// This is created once and then copied for each test that needs it.
/// The golden database is identified by timestamp and located automatically.
///
/// Gets the golden database path by finding the most recent timestamp-based
/// golden database
fn get_golden_db_path() -> PathBuf {
    use std::fs;

    // Try multiple possible locations for the target directory
    let possible_target_dirs = vec![
        PathBuf::from("../target"),    // Running from neems-api/
        PathBuf::from("target"),       // Running from workspace root
        PathBuf::from("../../target"), // Running from nested test directory
    ];

    for target_dir in possible_target_dirs {
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
    }

    // Fallback: return a path that won't exist to trigger the error in
    // fast_test_rocket
    PathBuf::from("target/golden_test_not_found.db")
}

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
        let conn = DbConn::get_one(&rocket).await.expect("database connection for migration");
        conn.run(|c| {
            set_sqlite_test_pragmas(c);
        })
        .await;
        rocket
    })
}

/// Creates and configures a Rocket instance for testing with an in-memory
/// SQLite database.
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
        let site_unique_db_name =
            format!("file:test_site_db_{}?mode=memory&cache=shared", Uuid::new_v4());
        let site_db_config: Map<_, Value> = map! {
            "url" => site_unique_db_name.into(),
            "pool_size" => 5.into(),
            "timeout" => 5.into(),
        };
        databases.insert("site_db", site_db_config);
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
    // {
    //     rocket = rocket.attach(test_data_init_fairing());
    // }

    // Attach SiteDbConn fairing when test-staging feature is enabled
    {
        rocket = rocket
            .attach(super::neems_data::db::SiteDbConn::fairing())
            .attach(super::neems_data::db::set_foreign_keys_fairing())
            .attach(super::neems_data::db::run_site_migrations_fairing());
    }

    crate::mount_api_routes(rocket)
}

/// Creates a fast Rocket instance for testing by copying a pre-populated golden
/// database. This is much faster than test_rocket() because it skips all the
/// initialization fairings.
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

    // Create a unique copy for this test in the same directory as the golden
    // database
    let test_db_path = golden_db_path
        .parent()
        .expect("Golden DB should have a parent directory")
        .join(format!("test_db_{}.db", Uuid::new_v4()));

    // Copy the golden database - this creates a brand new file with no existing
    // connections
    std::fs::copy(&golden_db_path, &test_db_path).expect("Failed to copy golden database");

    // Verify the copied database exists
    if !test_db_path.exists() {
        panic!("Copied test database does not exist at: {:?}", test_db_path);
    }

    println!("[fast-test] Copied golden DB to: {:?}", test_db_path);

    // Use the absolute path directly without file: prefix (like test_rocket uses
    // bare paths)
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
    let site_unique_db_name =
        format!("file:test_site_db_{}?mode=memory&cache=shared", Uuid::new_v4());
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

    // Build the Rocket instance with minimal fairings (no initialization fairings
    // needed!) The golden database is already fully set up, so we only need
    // basic database connections. However, the site_db is in-memory, so we need
    // to run migrations on it.
    let rocket = rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(super::neems_data::db::SiteDbConn::fairing())
        .attach(super::neems_data::db::set_foreign_keys_fairing())
        .attach(super::neems_data::db::run_site_migrations_fairing());

    crate::mount_api_routes(rocket)
}

/// Creates a synchronous in-memory SQLite database connection for unit tests.
///
/// This function returns a `diesel::SqliteConnection` connected to an in-memory
/// SQLite database, runs all embedded Diesel migrations, and enables foreign
/// key support. This is ideal for direct Diesel queries in synchronous test
/// code.
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

/// A minimal async-compatible wrapper for a synchronous SQLite connection for
/// unit testing.
///
/// This helper struct and function allow you to use your test database with
/// code that expects a Rocket-style async `.run()` interface (such as functions
/// that take a `DbConn`).
///
/// Unlike `setup_test_db()`, which returns a synchronous Diesel connection for
/// direct use, `setup_test_dbconn()` returns a `FakeDbConn` that can be used
/// with async code expecting a `.run()` method.
///
/// Both use the same in-memory database if you only call `setup_test_db()` once
/// and wrap the result. Each call to `setup_test_dbconn()` creates a new,
/// independent in-memory database.
pub struct FakeDbConn<'a>(pub &'a mut diesel::SqliteConnection);

impl<'a> FakeDbConn<'a> {
    /// Executes a closure with a mutable reference to the underlying SQLite
    /// connection.
    ///
    /// This method mimics the async `.run()` interface used by Rocket's
    /// database connections, but operates synchronously for testing
    /// purposes.
    ///
    /// # Arguments
    /// * `f` - A closure that takes a mutable reference to the SQLite
    ///   connection
    ///
    /// # Safety
    /// This uses unsafe code to convert an immutable reference to mutable,
    /// which is safe in this controlled test environment where we know we
    /// have exclusive access.
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

/// Creates a `FakeDbConn` for async-style testing with the given SQLite
/// connection.
///
/// This is useful for testing code that expects a Rocket-style `.run()`
/// interface, but you want to use your in-memory test database.
///
/// # Arguments
/// * `conn` - A mutable reference to a `diesel::SqliteConnection` (typically
///   from `setup_test_db()`)
///
/// # Returns
/// A `FakeDbConn` wrapping the provided connection
pub fn setup_test_dbconn<'a>(conn: &'a mut diesel::SqliteConnection) -> FakeDbConn<'a> {
    FakeDbConn(conn)
}

/// Creates a minimal Rocket instance for testing APIs that don't require a
/// database.
///
/// This is useful for testing endpoints that don't need database access,
/// avoiding potential database conflicts and improving test performance.
///
/// The returned Rocket instance will have:
/// - Only fixphrase API routes mounted (if fixphrase feature is enabled)
/// - No database connection
/// - No database-related fairings
#[cfg(feature = "fixphrase")]
pub fn test_rocket_no_db() -> Rocket<Build> {
    rocket::build().mount("/api", crate::api::fixphrase::routes())
}
