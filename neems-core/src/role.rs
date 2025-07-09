use diesel::prelude::*;
use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::Route;

use crate::db::DbConn;
use crate::models::{Role, NewRole};

#[post("/roles", data = "<new_role>")]
pub async fn create_role(
    db: DbConn,
    new_role: Json<NewRole>
) -> Result<Json<Role>, Status> {
    db.run(move |conn| {
        use crate::schema::roles::dsl::*;
        let new_role = new_role.into_inner();

        // Insert the new role
        diesel::insert_into(roles)
            .values(&new_role)
            .execute(conn)
            .map_err(|_| Status::InternalServerError)?;

        // Get the last inserted id (same pattern as institutions)
        // Define a local struct for last_insert_rowid
        #[derive(diesel::QueryableByName)]
        struct LastInsertRowId {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            last_insert_rowid: i64,
        }

        let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
            .get_result::<LastInsertRowId>(conn)
            .map_err(|_| Status::InternalServerError)?
            .last_insert_rowid;

        // Fetch the inserted row by id
        roles
            .filter(id.eq(last_id as i32))
            .first::<Role>(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

#[get("/roles")]
pub async fn list_roles(
    db: DbConn
) -> Result<Json<Vec<Role>>, Status> {
    db.run(|conn| {
        use crate::schema::roles::dsl::*;
        roles
            .order(id.asc())
            .load::<Role>(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

// Helper to return all routes for mounting
pub fn routes() -> Vec<Route> {
    routes![create_role, list_roles]
}
