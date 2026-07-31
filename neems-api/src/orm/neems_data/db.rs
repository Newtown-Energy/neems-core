use diesel::connection::SimpleConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rocket::fairing::AdHoc;
use rocket_sync_db_pools::{database, diesel};

pub const SITE_MIGRATIONS: EmbeddedMigrations = embed_migrations!("../neems-data/migrations");

#[database("site_db")]
pub struct SiteDbConn(diesel::SqliteConnection);

pub fn set_foreign_keys(conn: &mut diesel::SqliteConnection) {
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

pub fn set_foreign_keys_fairing() -> AdHoc {
    AdHoc::on_ignite("Set Site DB Foreign Keys", |rocket| async {
        let conn = SiteDbConn::get_one(&rocket)
            .await
            .expect("site database connection for foreign keys setup");
        conn.run(|c| {
            set_foreign_keys(c);
        })
        .await;
        rocket
    })
}

/// How many times to attempt the site migrations when SQLite reports a lock.
const MIGRATION_ATTEMPTS: u32 = 5;

/// Runs the site database migrations, retrying while SQLite reports a lock.
///
/// `neems-data` runs this same migration set against the same file on its first
/// connection, so on a fresh database both processes can race at startup. The
/// loser gets `database is locked` -- SQLite's default `busy_timeout` is 0, so
/// it fails immediately rather than waiting. Retrying with a linear backoff
/// lets the winner finish; the migrations are bookkept in
/// `__diesel_schema_migrations`, so the loser then finds nothing pending.
///
/// # Panics
/// Panics if the migrations fail for any other reason, or if they are still
/// locked out after [`MIGRATION_ATTEMPTS`] tries. A site DB we cannot migrate
/// serves nothing but 500s, so failing the boot is preferable to starting.
pub fn run_site_migrations(conn: &mut diesel::SqliteConnection) {
    for attempt in 1..=MIGRATION_ATTEMPTS {
        let err = match conn.run_pending_migrations(SITE_MIGRATIONS) {
            Ok(_) => return,
            Err(e) => e,
        };

        if !is_locked_error(&err) {
            panic!("Failed to run site database migrations: {}", err);
        }

        if attempt == MIGRATION_ATTEMPTS {
            panic!(
                "Failed to run site database migrations: still locked after {} attempts: {}",
                MIGRATION_ATTEMPTS, err
            );
        }

        warn!(
            "[site-migrations] Database locked, retrying ({}/{})",
            attempt, MIGRATION_ATTEMPTS
        );
        std::thread::sleep(std::time::Duration::from_millis(100 * u64::from(attempt)));
    }
}

/// Whether a migration error is SQLite's transient "database is locked".
fn is_locked_error<E: std::fmt::Display>(err: &E) -> bool {
    err.to_string().contains("database is locked")
}

pub fn run_site_migrations_fairing() -> AdHoc {
    AdHoc::on_ignite("Run Site DB Migrations", |rocket| async {
        let conn = SiteDbConn::get_one(&rocket)
            .await
            .expect("site database connection for migrations");
        conn.run(|c| {
            run_site_migrations(c);
        })
        .await;
        rocket
    })
}
