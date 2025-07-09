// neems-core/src/api/institution.rs

use diesel::prelude::*;
use diesel::QueryableByName;
use diesel::sql_types::BigInt;
use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::Route;

use crate::db::DbConn;
use crate::models::{Institution, NewInstitution, InstitutionName};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

pub fn insert_institution(
    conn: &mut SqliteConnection, 
    inst_name: String,
) -> Result<Institution, diesel::result::Error> {
    use crate::schema::institutions::dsl::*;
    let now = chrono::Utc::now().naive_utc();

    let new_inst = NewInstitution {
        name: inst_name,
        created_at: Some(now),
        updated_at: Some(now),
    };

    diesel::insert_into(institutions)
        .values(&new_inst)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    institutions
        .filter(id.eq(last_id as i32))
        .first::<Institution>(conn)
}

#[post("/institutions", data = "<new_institution>")]
pub async fn create_institution(
    db: DbConn,
    new_institution: Json<InstitutionName>
) -> Result<Json<Institution>, Status> {
    db.run(move |conn| {
        insert_institution(conn, new_institution.name.clone())
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}


#[get("/institutions")]
pub async fn list_institutions(
    db: DbConn
) -> Result<Json<Vec<Institution>>, Status> {
    db.run(|conn| {
        use crate::schema::institutions::dsl::*;
        institutions
            .order(id.asc())
            .load::<Institution>(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

// Helper to return all routes for mounting
pub fn routes() -> Vec<Route> {
    routes![create_institution, list_institutions]
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::sqlite::SqliteConnection;
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

    pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

    /// Returns a new in-memory SQLite connection for testing.
pub fn setup_test_db() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:")
        .expect("Failed to create in-memory SQLite database");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Migrations failed");
    conn
}

    #[test]
    fn test_insert_institution() {
	let mut conn = setup_test_db();
	let result = insert_institution(&mut conn, "Test Institution".to_string());
	assert!(result.is_ok());
	let inst = result.unwrap();
	assert_eq!(inst.name, "Test Institution");

	let now = chrono::Utc::now().naive_utc();
	let diff_created = (inst.created_at - now).num_seconds().abs();
	let diff_updated = (inst.updated_at - now).num_seconds().abs();

	assert!(
	    diff_created <= 1,
	    "created_at should be within 1 second of now (diff: {})",
	    diff_created
	);
	assert!(
	    diff_updated <= 1,
	    "updated_at should be within 1 second of now (diff: {})",
	    diff_updated
	);
    }
}
