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

pub fn run_site_migrations(conn: &mut diesel::SqliteConnection) {
    conn.run_pending_migrations(SITE_MIGRATIONS)
        .expect("Failed to run site database migrations");
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
