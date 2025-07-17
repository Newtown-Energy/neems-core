use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::Route;

use crate::orm::DbConn;
use crate::orm::role::{insert_role, get_all_roles};
use crate::models::{Role, NewRole};

#[post("/1/roles", data = "<new_role>")]
pub async fn create_role(
    db: DbConn,
    new_role: Json<NewRole>
) -> Result<Json<Role>, Status> {
    db.run(move |conn| {
        insert_role(conn, new_role.into_inner())
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

#[get("/1/roles")]
pub async fn list_roles(
    db: DbConn
) -> Result<Json<Vec<Role>>, Status> {
    db.run(|conn| {
        get_all_roles(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

// Helper to return all routes for mounting
pub fn routes() -> Vec<Route> {
    routes![create_role, list_roles]
}