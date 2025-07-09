/*

This file is for testing the schema and relationships in the database.
Do not use it to test the actual application logic.

*/

#[cfg(test)]

mod tests {
    use neems_core::establish_test_connection;
    use neems_core::models::*;
    use neems_core::schema::*;
    use diesel::prelude::*;
    use chrono::Utc;
    use diesel::result::{Error, DatabaseErrorKind};

    // Helper to create test institution
    fn create_test_institution(conn: &mut SqliteConnection, name: &str) -> Result<Institution, diesel::result::Error> {
	let now = Some(Utc::now().naive_utc());
	let new_institution = NewInstitution {
	    name: name.to_string(),
	    created_at: now,
	    updated_at: now,
	};

	diesel::insert_into(institutions::table)
	    .values(&new_institution)
	    .execute(conn)?;

	let institution = institutions::table
	    .order(institutions::id.desc())
	    .first(conn)?;

	Ok(institution)
    }

    fn create_test_role(
	conn: &mut SqliteConnection,
	name: &str,
	description: Option<&str>,
    ) -> Result<Role, diesel::result::Error> {
	let new_role = NewRole {
	    name: name.to_string(),
	    description: description.map(|s| s.to_string()),
	};

	diesel::insert_into(roles::table)
	    .values(&new_role)
	    .execute(conn)?;

	roles::table
	    .order(roles::id.desc())
	    .first(conn)
    }

    fn create_test_site(
	conn: &mut SqliteConnection,
	inst_id: i32,
	name: &str,
	address: &str,
	latitude: f64,
	longitude: f64,
    ) -> Result<Site, diesel::result::Error> {
	let new_site = NewSite {
	    name: name.to_string(),
	    address: address.to_string(),
	    latitude,
	    longitude,
	    institution_id: inst_id,
	    created_at: Utc::now().naive_utc(),
	    updated_at: Utc::now().naive_utc(),
	};

	diesel::insert_into(sites::table)
	    .values(&new_site)
	    .execute(conn)?;

	sites::table
	    .order(sites::id.desc())
	    .first(conn)
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

    #[test]
    fn test_institution_to_users_relationship() {
        let mut conn = establish_test_connection();
	let inst = create_test_institution(&mut conn, "Test Inst")
	    .expect("First institution insert should succeed");

        // Create users for this institution
	let user1 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "user1", "user1@test.com")
	    .expect("user1 should be created");
	let user2 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "user2", "user2@test.com")
	    .expect("user2 should be created");

        // Verify relationship
        let users = users::table
            .filter(users::institution_id.eq(inst.id.expect("Must have inst id")))
            .load::<User>(&mut conn)
            .expect("Failed to load users");
        
        assert_eq!(users.len(), 2);
        assert!(users.iter().any(|u| u.id == user1.id));
        assert!(users.iter().any(|u| u.id == user2.id));

	// Test foreign key constraint
	let result = create_test_user(
	    &mut conn,
	    99999, // Invalid FK
	    "newuser",
	    "new@test.com"
	);
	assert!(matches!(
	    result,
	    Err(Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _))
  	));
    }

    #[test]
    fn test_institution_to_sites_relationship() {
        let mut conn = establish_test_connection();
	let inst = create_test_institution(&mut conn, "Test Inst")
	    .expect("First institution insert should succeed");

        // Create sites for this institution
	create_test_site(
	    &mut conn,
	    inst.id.expect("Must have inst id"),
	    "Site A",
	    "123 Main St",
	    40.7128,
	    -74.0060,
	).expect("Failed to create site");

        // Verify relationship
        let sites = sites::table
            .filter(sites::institution_id.eq(inst.id.expect("Must have inst id")))
            .load::<Site>(&mut conn)
            .expect("Failed to load sites");
        
        assert_eq!(sites.len(), 1);
    }

    #[test]
    fn test_site_name_uniqueness_per_institution() {
	let mut conn = establish_test_connection();
	let inst1 = create_test_institution(&mut conn, "Inst 1")
	    .expect("First institution insert should succeed");
	let inst2 = create_test_institution(&mut conn, "Inst 2")
	    .expect("Second institution insert should succeed");

	// Create site for first institution
	create_test_site(
	    &mut conn,
	    inst1.id.expect("Must have inst1 id"),
	    "Main Site",
	    "123 Main St",
	    40.7128,
	    -74.0060,
	)
	.expect("Failed to create site");

	// Same name in different institution should work
	create_test_site(
	    &mut conn,
	    inst2.id.expect("Must have inst2 id"),
	    "Main Site",
	    "456 Other St",
	    34.0522,
	    -118.2437,
	)
	.expect("Failed to create site with same name in different institution");

	// Same name in same institution should fail
	let result = create_test_site(
	    &mut conn,
	    inst1.id.expect("Must have inst1 id"),
	    "Main Site",
	    "789 Third St",
	    41.8781,
	    -87.6298,
	);

	assert!(matches!(result, Err(Error::DatabaseError(_, _))));
    }

    #[test]
    fn test_user_roles_many_to_many() {
	use diesel::prelude::*;

	let mut conn = establish_test_connection();
	let inst = create_test_institution(&mut conn, "Roles Institution")
	    .expect("Roles institution insert should succeed");
	let user1 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "roleuser1", "roleuser1@test.com")
	    .expect("user1 should be created");
	let user2 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "roleuser2", "roleuser2@test.com")
	    .expect("user2 should be created");

	// Create roles
	let role1 = create_test_role(&mut conn, "Admin", Some("Administrator"))
	    .expect("Failed to create role1");
	let role2 = create_test_role(&mut conn, "Editor", Some("Content Editor"))
	    .expect("Failed to create role2");

	// Create associations
	diesel::insert_into(user_roles::table)
	    .values(&NewUserRole {
		user_id: user1.id.expect("Must have user1 id"),
		role_id: role1.id.expect("Must have role1 id"),
	    })
	    .execute(&mut conn)
	    .expect("Failed to create user role");

	diesel::insert_into(user_roles::table)
	    .values(&NewUserRole {
		user_id: user1.id.expect("Must have user1 id"),
		role_id: role2.id.expect("Must have role2 id"),
	    })
	    .execute(&mut conn)
	    .expect("Failed to create user role");

	diesel::insert_into(user_roles::table)
	    .values(&NewUserRole {
		user_id: user2.id.expect("Must have user2 id"),
		role_id: role1.id.expect("Must have role1 id"),
	    })
	    .execute(&mut conn)
	    .expect("Failed to create user role");

	// --- Verify many-to-many relationships ---

	// 1. All roles for user1
	let user1_user_roles = user_roles::table
	    .filter(user_roles::user_id.eq(user1.id.expect("Must have user1 id")))
	    .load::<UserRole>(&mut conn)
	    .expect("Failed to load user_roles for user1");
	let user1_role_ids: Vec<i32> = user1_user_roles.iter().map(|ur| ur.role_id).collect();
	let user1_roles = roles::table
	    .filter(roles::id.eq_any(user1_role_ids))
	    .load::<Role>(&mut conn)
	    .expect("Failed to load roles");
	assert_eq!(user1_roles.len(), 2);

	// 2. All users for role1
	let role1_user_roles = user_roles::table
	    .filter(user_roles::role_id.eq(role1.id.expect("Must have role1 id")))
	    .load::<UserRole>(&mut conn)
	    .expect("Failed to load user_roles for role1");
	let role1_user_ids: Vec<i32> = role1_user_roles.iter().map(|ur| ur.user_id).collect();
	let role1_users = users::table
	    .filter(users::id.eq_any(role1_user_ids))
	    .load::<User>(&mut conn)
	    .expect("Failed to load users");
	assert_eq!(role1_users.len(), 2);
    }

}
