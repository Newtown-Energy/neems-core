use diesel::connection::SimpleConnection;
use rocket::fairing::AdHoc;
use rocket_sync_db_pools::{database, diesel};

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