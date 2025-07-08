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
    fn create_test_institution(conn: &mut SqliteConnection, name: &str) -> Result<Institution, diesel::result::Error> {
	let new_institution = NewInstitution {
	    name: name.to_string(),
	    created_at: Utc::now().naive_utc(),
	    updated_at: Utc::now().naive_utc(),
	};

	diesel::insert_into(institutions::table)
	    .values(&new_institution)
	    .execute(conn)?;

	let institution = institutions::table
	    .order(institutions::id.desc())
	    .first(conn)?;

	Ok(institution)
    }

    fn create_test_user(
	conn: &mut SqliteConnection,
	inst_id: i32,
	username: &str,
	email: &str,
    ) -> Result<User, diesel::result::Error> {
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
	    .execute(conn)?;

	let user = users::table
	    .order(users::id.desc())
	    .first(conn)?;

	Ok(user)
    }

    #[test]
    fn test_institution_name_uniqueness() {
	let mut conn = establish_test_connection();

	// First insert should succeed
	create_test_institution(&mut conn, "Unique Institution")
	    .expect("First institution insert should succeed");

	// Second insert with same name should fail
	let result = create_test_institution(&mut conn, "Unique Institution");
	assert!(matches!(
	    result,
	    Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))
	));
    }


    #[test]
    fn test_user_uniqueness_constraints() {
	let mut conn = establish_test_connection();
	let inst = create_test_institution(&mut conn, "Test Inst")
	    .expect("First institution insert should succeed");


	// First user should succeed
	create_test_user(
	    &mut conn,
	    inst.id.expect("Institution ID should be set"),
	    "user1",
	    "user1@test.com"
	).expect("First user insert should succeed");

	// Second user with duplicate username should fail
	let result = create_test_user(
	    &mut conn,
	    inst.id.expect("Institution ID should be set"),
	    "user1", // duplicate username
	    "user2@test.com"
	);
	assert!(matches!(
	    result,
	    Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))
	));

	// Third user with duplicate email should fail
	let result = create_test_user(
	    &mut conn,
	    inst.id.expect("Institution ID should be set"),
	    "user2",
	    "user1@test.com" // duplicate email
	);
	assert!(matches!(
	    result,
	    Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))
	));
    }

}
