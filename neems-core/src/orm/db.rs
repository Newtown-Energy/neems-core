use diesel::connection::SimpleConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rocket::{Rocket, Build, fairing::AdHoc};
use rocket::figment::{
    util::map,
    value::{Map, Value},
};
use rocket_sync_db_pools::{database, diesel};

use crate::admin_init_fairing::admin_init_fairing;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);

/// Enables foreign key support for SQLite connections.
///
/// This executes the `PRAGMA foreign_keys = ON` command on the provided connection.
/// Foreign keys are disabled by default in SQLite for backwards compatibility.
///
/// # Arguments
/// * `conn` - A mutable reference to a SQLite database connection
///
/// # Panics
/// Panics if the PRAGMA command fails to execute
fn set_foreign_keys(conn: &mut diesel::SqliteConnection) {
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

/// Creates a Rocket fairing that enables foreign key support for SQLite connections.
///
/// This fairing will execute when the Rocket application ignites, ensuring foreign keys
/// are enabled for all database connections in the pool.
pub fn set_foreign_keys_fairing() -> AdHoc {
    AdHoc::on_ignite("Diesel Migrations", |rocket| async {
        let conn = DbConn::get_one(&rocket).await
            .expect("database connection for migration");
        conn.run(|c| {
            set_foreign_keys(c);
        }).await;
        rocket
    })
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
        "#
    ).expect("Failed to set SQLite PRAGMAs");
}

/// Creates a Rocket fairing that sets SQLite testing pragmas.
///
/// This fairing configures SQLite for faster but less durable operation,
/// suitable only for testing environments.
fn set_sqlite_test_pragmas_fairing() -> AdHoc {
    AdHoc::on_ignite("Diesel Migrations", |rocket| async {
        let conn = DbConn::get_one(&rocket).await
            .expect("database connection for migration");
        conn.run(|c| {
            set_sqlite_test_pragmas(c);
        }).await;
        rocket
    })
}

/// Runs all pending database migrations on the provided connection.
///
/// # Arguments
/// * `conn` - A mutable reference to a SQLite database connection
///
/// # Panics
/// Panics if any migration fails to run
fn run_pending_migrations(conn: &mut diesel::SqliteConnection) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run pending migrations");
}

/// Creates a Rocket fairing that runs database migrations on ignition.
///
/// This fairing ensures all pending Diesel migrations are run when the
/// Rocket application starts up.
pub fn run_migrations_fairing() -> AdHoc {
    AdHoc::on_ignite("Diesel Migrations", |rocket| async {
        // Get a database connection from Rocket's pool
        let conn = DbConn::get_one(&rocket).await
            .expect("database connection for migration");
        conn.run(|c| {
            run_pending_migrations(c);
        }).await;
        rocket
    })
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
    // Configure the in-memory SQLite database
    let db_config: Map<_, Value> = map! {
        "url" => ":memory:".into(),
        "pool_size" => 10.into(),
        "timeout" => 5.into(),
    };

    // Merge DB config into Rocket's figment
    let figment = rocket::Config::figment()
        .merge(("databases", map!["sqlite_db" => db_config]));

    // Build the Rocket instance with the DB fairing attached
    let rocket = rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(set_foreign_keys_fairing())
        .attach(set_sqlite_test_pragmas_fairing())
        .attach(run_migrations_fairing())
        .attach(admin_init_fairing());
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
            let conn_ptr = self.0 as *const diesel::SqliteConnection as *mut diesel::SqliteConnection;
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
