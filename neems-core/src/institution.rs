// neems-core/src/api/institution.rs

use chrono::Utc;
use diesel::prelude::*;
use diesel::QueryableByName;
use diesel::sql_types::BigInt;
use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::Route;

use crate::db::DbConn;
use crate::models::{Institution, NewInstitution};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[sql_type = "BigInt"]
    last_insert_rowid: i64,
}

#[post("/institutions", data = "<new_institution>")]
pub async fn create_institution(
    db: DbConn,
    new_institution: Json<NewInstitution>
) -> Result<Json<Institution>, Status> {
    db.run(move |conn| {
        use crate::schema::institutions::dsl::*;
        let mut new_inst = new_institution.into_inner();
        let now = Some(chrono::Utc::now().naive_utc());
        new_inst.created_at = now;
        new_inst.updated_at = now;

        // Insert the new institution
        diesel::insert_into(institutions)
            .values(&new_inst)
            .execute(conn)
            .map_err(|_| Status::InternalServerError)?;

        // Get the last inserted id
	let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
	    .get_result::<LastInsertRowId>(conn)
	    .map_err(|_| Status::InternalServerError)?
	    .last_insert_rowid;

        // Fetch the inserted row by id
        institutions
            .filter(id.eq(last_id as i32))
            .first::<Institution>(conn)
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
