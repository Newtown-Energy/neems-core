use diesel::prelude::*;
use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::response::status;
use rocket::Route;

use crate::orm::DbConn;
use crate::models::{Institution, InstitutionName};
use crate::institution::insert_institution;

#[post("/1/institutions", data = "<new_institution>")]
pub async fn create_institution(
    db: DbConn,
    new_institution: Json<InstitutionName>
) -> Result<status::Created<Json<Institution>>, Status> {
    db.run(move |conn| {
        insert_institution(conn, new_institution.name.clone())
            .map(|inst| status::Created::new("/").body(Json(inst)))
            .map_err(|e| {
                eprintln!("Error creating institution: {:?}", e);
                Status::InternalServerError
            })
    }).await
}

#[get("/1/institutions")]
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

// Return all routes for mounting
pub fn routes() -> Vec<Route> {
    routes![create_institution, list_institutions]
}