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

/// Inserts a new user and returns the inserted User
pub fn insert_user(
    conn: &mut SqliteConnection,
    mut new_user: NewUser,
) -> Result<User, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    let now = chrono::Utc::now().naive_utc();
    new_user.created_at = now;
    new_user.updated_at = now;

    diesel::insert_into(users)
        .values(&new_user)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    users
        .filter(id.eq(last_id as i32))
        .first::<User>(conn)
}

#[post("/users", data = "<new_user>")]
pub async fn create_user(
    db: DbConn,
    new_user: Json<NewUser>
) -> Result<Json<User>, Status> {
    db.run(move |conn| {
        insert_user(conn, new_user.into_inner())
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}


/// Returns all users in ascending order by id.
pub fn list_all_users(
    conn: &mut SqliteConnection,
) -> Result<Vec<User>, diesel::result::Error> {
    use crate::schema::users::dsl::*;
    users.order(id.asc()).load::<User>(conn)
}

#[get("/users")]
pub async fn list_users(
    db: DbConn
) -> Result<Json<Vec<User>>, Status> {
    db.run(|conn| {
        list_all_users(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

pub fn routes() -> Vec<Route> {
    routes![create_user, list_users]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::setup_test_db;
    use crate::institution::insert_institution;

    #[test]
    fn test_insert_user() {
        let mut conn = setup_test_db();

	let institution = insert_institution(&mut conn, "Test Institution".to_string())
	    .expect("Failed to insert institution");

        let now = chrono::Utc::now().naive_utc();
        let new_user = NewUser {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hashedpassword".to_string(),
            created_at: now,      // Will be overwritten in insert_user
            updated_at: now,      // Will be overwritten in insert_user
            institution_id: institution.id.unwrap(),    // Use a valid institution id for your test db
            totp_secret: "secret".to_string(),
        };

        let result = insert_user(&mut conn, new_user);
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.password_hash, "hashedpassword");
        assert_eq!(user.institution_id, 1);
        assert_eq!(user.totp_secret, "secret");
        assert!(user.id.is_some());

        let now = chrono::Utc::now().naive_utc();
        let diff_created = (user.created_at - now).num_seconds().abs();
        let diff_updated = (user.updated_at - now).num_seconds().abs();
        assert!(diff_created <= 1, "created_at should be within 1 second of now (diff: {})", diff_created);
        assert!(diff_updated <= 1, "updated_at should be within 1 second of now (diff: {})", diff_updated);
    }

    #[test]
    fn test_list_all_users() {
        let mut conn = setup_test_db();

	let institution = insert_institution(&mut conn, "Test Institution".to_string())
	    .expect("Failed to insert institution");

        // Insert two users
        let now = chrono::Utc::now().naive_utc();
        let user1 = NewUser {
            username: "user1".to_string(),
            email: "user1@example.com".to_string(),
            password_hash: "pw1".to_string(),
            created_at: now,
            updated_at: now,
            institution_id: institution.id.unwrap(),
            totp_secret: "secret1".to_string(),
        };
        let user2 = NewUser {
            username: "user2".to_string(),
            email: "user2@example.com".to_string(),
            password_hash: "pw2".to_string(),
            created_at: now,
            updated_at: now,
            institution_id: institution.id.unwrap(),
            totp_secret: "secret2".to_string(),
        };

        let _ = insert_user(&mut conn, user1).unwrap();
        let _ = insert_user(&mut conn, user2).unwrap();

        let users = list_all_users(&mut conn).unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].username, "user1");
        assert_eq!(users[1].username, "user2");
        assert!(users[0].id < users[1].id);
    }
}
