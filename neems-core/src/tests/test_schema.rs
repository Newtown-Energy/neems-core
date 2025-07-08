// src/tests/test_schema.rs
#[cfg(test)]

mod tests {
    use crate::establish_test_connection;
    use crate::models::*;
    use crate::schema::*;
    use diesel::prelude::*;
    use chrono::Utc;
    use diesel::result::{Error, DatabaseErrorKind};

    // Helper to create test institution
    fn create_test_institution(conn: &mut SqliteConnection, name: &str) -> Institution {
	let new_institution = NewInstitution {
	    name: name.to_string(),
	    created_at: Utc::now().naive_utc(),
	    updated_at: Utc::now().naive_utc(),
	};

	diesel::insert_into(institutions::table)
	    .values(&new_institution)
	    .execute(conn)
	    .expect("Failed to insert institution");

	institutions::table
	    .order(institutions::id.desc())
	    .first(conn)
	    .expect("Failed to load created institution")
    }

    fn create_test_user(
	conn: &mut SqliteConnection,
	inst_id: i32,
	username: &str,
	email: &str,
    ) -> User {
	let new_user = NewUser {
	    username: username.to_string(),
	    email: email.to_string(),
	    password_hash: "testhash".to_string(),
	    institution_id: inst_id,
	    created_at: Utc::now().naive_utc(),
	    updated_at: Utc::now().naive_utc(),
	    totp_secret: "testsecret".to_string(),
	};

	diesel::insert_into(users::table)
	    .values(&new_user)
	    .execute(conn)
	    .expect("Failed to insert user");

	users::table
	    .order(users::id.desc())
	    .first(conn)
	    .expect("Failed to load created user")
    }

    #[test]
    fn test_institution_name_uniqueness() {
        let mut conn = establish_test_connection();

        // First insert should succeed
        create_test_institution(&mut conn, "Unique Institution");

        // Second insert with same name should fail
        let result = diesel::insert_into(institutions::table)
            .values(&NewInstitution {
                name: "Unique Institution".to_string(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            })
            .execute(&mut conn);

        assert!(matches!(result, Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))));

    }

    #[test]
    fn test_user_uniqueness_constraints() {
	let mut conn = establish_test_connection();
	let inst = create_test_institution(&mut conn, "Test Inst");

	// First user should succeed
	create_test_user(
	    &mut conn,
	    inst.id.expect("Institution ID should be set"),
	    "user1",
	    "user1@test.com"
	);

        // Test username uniqueness
        let result = diesel::insert_into(users::table)
	    .values(&NewUser {
		username: "user1".to_string(),  // Duplicate username
		email: "user2@test.com".to_string(),
		password_hash: "testhash".to_string(),
		institution_id: inst.id.expect("Institution ID should be set"),
		created_at: Utc::now().naive_utc(),
		updated_at: Utc::now().naive_utc(),
		totp_secret: "testsecret".to_string(),
	    })
            .execute(&mut conn);
        assert!(matches!(result, Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))));

        // Test email uniqueness
        let result = diesel::insert_into(users::table)
	    .values(&NewUser {
		username: "user2".to_string(),
		email: "user1@test.com".to_string(),
		password_hash: "testhash".to_string(),
		institution_id: inst.id.expect("Institution ID should be set"),
		created_at: Utc::now().naive_utc(),
		updated_at: Utc::now().naive_utc(),
		totp_secret: "testsecret".to_string(),
	    })
            .execute(&mut conn);
        assert!(matches!(result, Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))));
    }
}
