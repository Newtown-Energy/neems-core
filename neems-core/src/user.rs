use diesel::prelude::*;
use diesel::sql_types::BigInt;
use diesel::QueryableByName;
use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::Route;

use crate::db::DbConn;
use crate::models::{User, NewUser};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

#[post("/users", data = "<new_user>")]
pub async fn create_user(
    db: DbConn,
    new_user: Json<NewUser>
) -> Result<Json<User>, Status> {
    db.run(move |conn| {
        use crate::schema::users::dsl::*;
        let mut new_user = new_user.into_inner();
        let now = chrono::Utc::now().naive_utc();
        new_user.created_at = now;
        new_user.updated_at = now;

        diesel::insert_into(users)
            .values(&new_user)
            .execute(conn)
            .map_err(|_| Status::InternalServerError)?;

        let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
            .get_result::<LastInsertRowId>(conn)
            .map_err(|_| Status::InternalServerError)?
            .last_insert_rowid;

        users
            .filter(id.eq(last_id as i32))
            .first::<User>(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

#[get("/users")]
pub async fn list_users(
    db: DbConn
) -> Result<Json<Vec<User>>, Status> {
    db.run(|conn| {
        use crate::schema::users::dsl::*;
        users
            .order(id.asc())
            .load::<User>(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

pub fn routes() -> Vec<Route> {
    routes![create_user, list_users]
}
