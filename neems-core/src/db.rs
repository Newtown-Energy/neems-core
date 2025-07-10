use diesel::connection::SimpleConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rocket::{Rocket, Build, fairing::AdHoc};
use rocket::figment::{
    util::map,
    value::{Map, Value},
};
use rocket_sync_db_pools::{database, diesel};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);

fn set_sqlite_test_pragmas(conn: &mut diesel::SqliteConnection) {
    conn.batch_execute(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA synchronous = OFF;
        PRAGMA journal_mode = OFF;
        "#
    ).expect("Failed to set SQLite PRAGMAs");
}
pub fn run_migrations_and_pragmas_fairing() -> AdHoc {
    AdHoc::on_ignite("Diesel Migrations", |rocket| async {
        let conn = DbConn::get_one(&rocket).await
            .expect("database connection for migration");
        conn.run(|c| {
            set_sqlite_test_pragmas(c);
            c.run_pending_migrations(MIGRATIONS)
                .expect("diesel migrations");
        }).await;
        rocket
    })
}

pub fn run_migrations_fairing() -> AdHoc {
    AdHoc::on_ignite("Diesel Migrations", |rocket| async {
        // Get a database connection from Rocket's pool
        let conn = DbConn::get_one(&rocket).await
            .expect("database connection for migration");
        // Run migrations on that connection
        conn.run(|c| {
            c.run_pending_migrations(MIGRATIONS)
                .expect("diesel migrations");
        }).await;
        rocket
    })
}

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
	.attach(run_migrations_and_pragmas_fairing());

    crate::mount_all_routes(rocket)
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
    conn.batch_execute("PRAGMA foreign_keys = ON")
	.expect("Could not enable foreign keys");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Migrations failed");
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
///
/// # Example
/// ```
/// use neems_core::db::setup_test_db;
/// use neems_core::db::setup_test_dbconn;
/// let mut conn = setup_test_db();
/// let fake_db = setup_test_dbconn(&mut conn);
/// // Now you can use fake_db.run(|c| ...).await in async tests.
/// ```
pub struct FakeDbConn<'a>(pub &'a mut diesel::SqliteConnection);

impl<'a> FakeDbConn<'a> {
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
/// This function uses the same in-memory database as
/// `setup_test_db()` because it wraps the same connection:
///
/// # Example
/// ```
/// use neems_core::db::setup_test_db;
/// use neems_core::db::setup_test_dbconn;
/// let mut conn = setup_test_db();
/// let fake_db = setup_test_dbconn(&mut conn);
/// ```
///
/// # Arguments
/// * `conn` - A mutable reference to a `diesel::SqliteConnection` (from `setup_test_db()`).
///
/// # Returns
/// A `FakeDbConn` wrapping the provided connection.
pub fn setup_test_dbconn<'a>(conn: &'a mut diesel::SqliteConnection) -> FakeDbConn<'a> {
    FakeDbConn(conn)
}
