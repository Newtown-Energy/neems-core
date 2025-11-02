use diesel::connection::SimpleConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rocket::fairing::AdHoc;
use rocket_sync_db_pools::{database, diesel};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);

/// Enables foreign key support for SQLite connections.
///
/// This executes the `PRAGMA foreign_keys = ON` command on the provided
/// connection. Foreign keys are disabled by default in SQLite for backwards
/// compatibility.
///
/// # Arguments
/// * `conn` - A mutable reference to a SQLite database connection
///
/// # Panics
/// Panics if the PRAGMA command fails to execute
pub fn set_foreign_keys(conn: &mut diesel::SqliteConnection) {
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

/// Creates a Rocket fairing that enables foreign key support for SQLite
/// connections.
///
/// This fairing will execute when the Rocket application ignites, ensuring
/// foreign keys are enabled for all database connections in the pool.
pub fn set_foreign_keys_fairing() -> AdHoc {
    AdHoc::on_ignite("Set Foreign Keys", |rocket| async {
        let conn = DbConn::get_one(&rocket).await.expect("database connection for migration");
        conn.run(|c| {
            set_foreign_keys(c);
        })
        .await;
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
pub fn run_pending_migrations(conn: &mut diesel::SqliteConnection) {
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
        let conn = DbConn::get_one(&rocket).await.expect("database connection for migration");
        conn.run(|c| {
            run_pending_migrations(c);
        })
        .await;
        rocket
    })
}
