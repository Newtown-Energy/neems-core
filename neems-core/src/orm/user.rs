use diesel::prelude::*;
use diesel::sql_types::BigInt;
use diesel::QueryableByName;

use crate::models::{User, UserNoTime, NewUser};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Inserts a new user and returns the inserted User
pub fn insert_user(
    conn: &mut SqliteConnection,
    new_user: UserNoTime,
) -> Result<User, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    let now = chrono::Utc::now().naive_utc();
    let insertable_user = NewUser {
        email: new_user.email,
        password_hash: new_user.password_hash,
        created_at: now,
        updated_at: now,
        institution_id: new_user.institution_id,
        totp_secret: new_user.totp_secret,
    };

    diesel::insert_into(users)
        .values(&insertable_user)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    users
        .filter(id.eq(last_id as i32))
        .first::<User>(conn)
}

/// Returns all users in ascending order by id.
pub fn list_all_users(
    conn: &mut SqliteConnection,
) -> Result<Vec<User>, diesel::result::Error> {
    use crate::schema::users::dsl::*;
    users.order(id.asc()).load::<User>(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;
    use crate::orm::institution::insert_institution;

    #[test]
    fn test_insert_user() {
        let mut conn = setup_test_db();

	let institution = insert_institution(&mut conn, "Test Institution".to_string())
	    .expect("Failed to insert institution");

        let new_user = UserNoTime {
            email: "test@example.com".to_string(),
            password_hash: "hashedpassword".to_string(),
            institution_id: institution.id,    // Use a valid institution id for your test db
            totp_secret: "secret".to_string(),
        };

        let result = insert_user(&mut conn, new_user);
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.password_hash, "hashedpassword");
        assert_eq!(user.institution_id, 2); // one more than our existing institution, Newtown
        assert_eq!(user.totp_secret, "secret");
        assert!(user.id > 0);

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
        let user1 = UserNoTime {
            email: "user1@example.com".to_string(),
            password_hash: "pw1".to_string(),
            institution_id: institution.id,
            totp_secret: "secret1".to_string(),
        };
        let user2 = UserNoTime {
            email: "user2@example.com".to_string(),
            password_hash: "pw2".to_string(),
            institution_id: institution.id,
            totp_secret: "secret2".to_string(),
        };

        let _ = insert_user(&mut conn, user1).unwrap();
        let _ = insert_user(&mut conn, user2).unwrap();

        let users = list_all_users(&mut conn).unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].email, "user1@example.com");
        assert_eq!(users[1].email, "user2@example.com");
        assert!(users[0].id < users[1].id);
    }
}