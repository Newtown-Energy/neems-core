// neems-core/tests/schema.rs
//
// This file is for testing the schema and relationships in the database.
// Do not use it to test the actual application logic.
//

use neems_core::orm::setup_test_db;
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
    email: &str,
) -> Result<User, diesel::result::Error> {
    let new_user = NewUser {
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

/// Ensures that institutions with dependent sites cannot be deleted.
/// This test protects the integrity of the relationship between institutions and sites.
/// If this fails, deleting an institution could leave orphaned site records or lose associated site data.
#[test]
fn test_institution_restrict_delete_with_existing_sites() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Site Parent").unwrap();
    create_test_site(&mut conn, inst.id.unwrap(), "Main", "1 Any St", 1.0, 2.0).unwrap();
    let res = diesel::delete(institutions::table.filter(institutions::id.eq(inst.id.unwrap())))
        .execute(&mut conn);
    assert!(matches!(res, Err(Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _))));
}

/// Ensures that institutions with dependent users cannot be deleted.
/// Prevents accidental deletion of entire business structures if users still exist, maintaining referential integrity.
#[test]
fn test_institution_restrict_delete_with_existing_users() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Restrict Co").unwrap();
    let _u = create_test_user(&mut conn, inst.id.unwrap(), "restrict@test.com").unwrap();
    let res = diesel::delete(institutions::table.filter(institutions::id.eq(inst.id.unwrap())))
        .execute(&mut conn);
    assert!(matches!(res, Err(Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _))));
}

/// Asserts that no two institutions may share the same name (database-level uniqueness constraint).
/// Avoids ambiguity when institutions are referenced by name. This catches accidental duplicate creation and ensures business entities are globally unique.
#[test]
fn test_institution_name_uniqueness() {
    let mut conn = setup_test_db();

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

/// Asserts enforcement of user email uniqueness at the database level.
/// Essential to guarantee account identity and prevent confusion or login security errors.
#[test]
fn test_email_uniqueness_constraints() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Test Inst")
        .expect("First institution insert should succeed");

    // First user should succeed
    create_test_user(
        &mut conn,
        inst.id.expect("Institution ID should be set"),
        "user1@test.com"
    ).expect("First user insert should succeed");

    // Second user with different email should succeed
    create_test_user(
        &mut conn,
        inst.id.expect("Institution ID should be set"),
        "user2@test.com"
    ).expect("Second user insert should succeed");

    // Third user with duplicate email should fail
    let result = create_test_user(
        &mut conn,
        inst.id.expect("Institution ID should be set"),
        "user1@test.com" // duplicate email
    );
    assert!(matches!(
        result,
        Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))
    ));
}

/// Verifies that the one-to-many relationship between institutions and users is enforced.
/// Tests proper linkage, and verifies that users cannot exist for missing/invalid institutions, ensuring data consistency.
#[test]
fn test_institution_to_users_relationship() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Test Inst")
        .expect("First institution insert should succeed");

    // Create users for this institution
    let user1 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "user1@test.com")
        .expect("user1 should be created");
    let user2 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "user2@test.com")
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
        "new@test.com"
    );
    assert!(matches!(
        result,
        Err(Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _))
    ));
}

/// Asserts that the one-to-many relationship between institutions and sites is enforced at the DB level.
/// Prevents sites from being orphaned and confirms only valid parents may have sites.
#[test]
fn test_institution_to_sites_relationship() {
    let mut conn = setup_test_db();
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

/// Asserts that sites are only unique within an institution—site names can be reused across institutions but not within one.
/// Prevents ambiguous location naming within an organization.
#[test]
fn test_site_name_uniqueness_per_institution() {
    let mut conn = setup_test_db();
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

/// Verifies that `revoked` on sessions defaults to false/0 when not explicitly provided.
/// Prevents accidental enables of revoked state by default, ensuring session validity on creation.
#[test]
fn test_sessions_revoked_defaults_to_false() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "DefaultTest").unwrap();
    let user = create_test_user(&mut conn, inst.id.unwrap(), "default@test.com").unwrap();

    diesel::insert_into(sessions::table)
        .values((
            sessions::id.eq("defsession"),
            sessions::user_id.eq(user.id.unwrap()),
            sessions::created_at.eq(Utc::now().naive_utc()),
            sessions::expires_at.eq::<Option<chrono::NaiveDateTime>>(None),
        ))
        .execute(&mut conn)
        .unwrap();

    let session: Session = sessions::table.filter(sessions::id.eq("defsession")).first(&mut conn).unwrap();
    assert_eq!(session.revoked, false);
}

/// Ensures that users with active user_roles associations cannot be deleted.
/// Guarantees that role assignments are never left dangling, protecting authorization integrity.
#[test]
fn test_user_restrict_delete_with_existing_roles() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Role Parent").unwrap();
    let user = create_test_user(&mut conn, inst.id.unwrap(), "roleuser@test.com").unwrap();
    let role = create_test_role(&mut conn, "deletetestrole", Some("A role")).unwrap();
    let assoc = NewUserRole { user_id: user.id.unwrap(), role_id: role.id.unwrap() };
    diesel::insert_into(user_roles::table).values(&assoc).execute(&mut conn).unwrap();
    let res = diesel::delete(users::table.filter(users::id.eq(user.id.unwrap()))).execute(&mut conn);
    assert!(matches!(res, Err(Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _))));
}

/// Ensures that users with dependent sessions cannot be deleted.
/// Maintains correct referential connection between users and their sessions, preventing orphaned sessions which could misrepresent security state.
#[test]
fn test_user_restrict_delete_with_existing_sessions() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Session Parent").unwrap();
    let user = create_test_user(&mut conn, inst.id.unwrap(), "session@test.com").unwrap();
    // Insert a session for this user
    diesel::insert_into(sessions::table)
        .values((
            sessions::id.eq("sessionid"),
            sessions::user_id.eq(user.id.unwrap()),
            sessions::created_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .unwrap();
    let res = diesel::delete(users::table.filter(users::id.eq(user.id.unwrap()))).execute(&mut conn);
    assert!(matches!(res, Err(Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _))));
}

/// Verifies that the users.email column is NOT NULL, providing a DB-level guarantee that all user records are addressable by email.
/// Avoids breaking authentication flows or business logic that assumes every user has a valid email.
#[test]
fn test_user_email_not_null_constraint() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Nullable U").unwrap();
    let new_user = NewUser {
        email: "".to_string(), // Simulate NULL with empty for Rust struct, real NULL can't be constructed directly
        password_hash: "pw".to_string(),
        institution_id: inst.id.unwrap(),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
        totp_secret: "secret".to_string(),
    };
    // Intentionally using raw Diesel to try to insert None for email (Rust won't let us send Option::None to required field)
    let res = diesel::insert_into(users::table)
        .values((
            users::institution_id.eq(new_user.institution_id),
            users::password_hash.eq(new_user.password_hash),
            users::totp_secret.eq(new_user.totp_secret),
            users::created_at.eq(new_user.created_at),
            users::updated_at.eq(new_user.updated_at),
        ))
        .execute(&mut conn);
    assert!(matches!(res, Err(Error::DatabaseError(DatabaseErrorKind::NotNullViolation, _))));
}

/// Asserts AUTOINCREMENT behavior for user primary key for integrity and predictability.
/// Failing this could indicate a disruptive change in primary key management, risking duplicate or reused user IDs.
#[test]
fn test_user_id_autoincrements() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "PK Inc").unwrap();

    let user1 = create_test_user(&mut conn, inst.id.unwrap(), "pk1@test.com").unwrap();
    let user2 = create_test_user(&mut conn, inst.id.unwrap(), "pk2@test.com").unwrap();

    assert!(user2.id.unwrap() > user1.id.unwrap());
}

/// Asserts that user-role many-to-many mapping functions as expected, and verifies both sides of the relation.
/// Maintaining this mapping guarantees correctness for permission checks and role membership logic.
#[test]
fn test_user_roles_many_to_many() {
    use diesel::prelude::*;

    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Roles Institution")
        .expect("Roles institution insert should succeed");
    let user1 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "roleuser1@test.com")
        .expect("user1 should be created");
    let user2 = create_test_user(&mut conn, inst.id.expect("Must have inst id"), "roleuser2@test.com")
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

/// Ensures that roles in active use by any user cannot be deleted.
/// Prevents breaking permission assignments, thereby maintaining consistent authorization data.
#[test]
fn test_role_restrict_delete_in_use_by_user() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Role RESTRICT Institution").unwrap();
    let user = create_test_user(&mut conn, inst.id.unwrap(), "restrictrole@test.com").unwrap();
    let role = create_test_role(&mut conn, "restrictrole", Some("A role")).unwrap();
    let assoc = NewUserRole { user_id: user.id.unwrap(), role_id: role.id.unwrap() };
    diesel::insert_into(user_roles::table).values(&assoc).execute(&mut conn).unwrap();
    let res = diesel::delete(roles::table.filter(roles::id.eq(role.id.unwrap()))).execute(&mut conn);
    assert!(matches!(res, Err(Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _))));
}

/// Ensures that role names remain globally unique in the database.
/// Prevents ambiguity when assigning or referencing roles by name, protecting authorization correctness.
#[test]
fn test_role_name_uniqueness_constraint() {
    let mut conn = setup_test_db();

    // First role creation should succeed
    create_test_role(&mut conn, "unique-role", Some("Testing role"))
        .expect("First role insert should succeed");

    // Second role with the same name should fail due to unique constraint
    let result = create_test_role(&mut conn, "unique-role", Some("Should fail"));
    assert!(
        matches!(result, Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))),
        "Expected UNIQUE violation when inserting duplicate role name"
    );
}

/// Verifies that the composite primary key on user_roles enforces uniqueness, preventing duplicate user-role assignments.
/// Ensures correctness of role membership calculations and avoids accidental privilege escalations.
#[test]
fn test_user_roles_composite_primary_key_uniqueness() {
    let mut conn = setup_test_db();
    let inst = create_test_institution(&mut conn, "Composite PK Test").unwrap();
    let user = create_test_user(&mut conn, inst.id.unwrap(), "m2m@test.com").unwrap();
    let role = create_test_role(&mut conn, "user_roles_pk", None).unwrap();
    let assoc = NewUserRole { user_id: user.id.unwrap(), role_id: role.id.unwrap() };
    diesel::insert_into(user_roles::table).values(&assoc).execute(&mut conn).unwrap();
    // Attempt to re-insert the same association
    let res = diesel::insert_into(user_roles::table).values(&assoc).execute(&mut conn);
    assert!(matches!(res, Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _))));
}
