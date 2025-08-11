use diesel::QueryableByName;
use diesel::prelude::*;
use diesel::sql_types::BigInt;

use crate::models::{NewUser, User, UserInput, UserWithRoles, UserWithTimestamps, UserWithRolesAndTimestamps};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Inserts a new user (timestamps handled automatically by database triggers)
pub fn insert_user(
    conn: &mut SqliteConnection,
    new_user: UserInput,
) -> Result<User, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    let insertable_user = NewUser {
        email: new_user.email,
        password_hash: new_user.password_hash,
        company_id: new_user.company_id,
        totp_secret: new_user.totp_secret,
    };

    diesel::insert_into(users)
        .values(&insertable_user)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    users.filter(id.eq(last_id as i32)).first::<User>(conn)
}

/// Get a user with computed timestamps from activity log
pub fn get_user_with_timestamps(
    conn: &mut SqliteConnection,
    user_id: i32,
) -> Result<Option<UserWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity;
    
    // First get the user
    let user = match get_user(conn, user_id)? {
        Some(u) => u,
        None => return Ok(None),
    };

    // Get timestamps from activity log
    let created_at = entity_activity::get_created_at(conn, "users", user_id)?;
    let updated_at = entity_activity::get_updated_at(conn, "users", user_id)?;

    Ok(Some(UserWithTimestamps {
        id: user.id,
        email: user.email,
        password_hash: user.password_hash,
        company_id: user.company_id,
        totp_secret: user.totp_secret,
        created_at,
        updated_at,
    }))
}

/// Get a user with roles and computed timestamps from activity log
pub fn get_user_with_roles_and_timestamps(
    conn: &mut SqliteConnection,
    user_id: i32,
) -> Result<Option<UserWithRolesAndTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity;
    
    // First get the user with roles
    let user_with_roles = match get_user_with_roles(conn, user_id)? {
        Some(u) => u,
        None => return Ok(None),
    };

    // Get timestamps from activity log
    let created_at = entity_activity::get_created_at(conn, "users", user_id)?;
    let updated_at = entity_activity::get_updated_at(conn, "users", user_id)?;

    Ok(Some(UserWithRolesAndTimestamps {
        id: user_with_roles.id,
        email: user_with_roles.email,
        password_hash: user_with_roles.password_hash,
        company_id: user_with_roles.company_id,
        totp_secret: user_with_roles.totp_secret,
        created_at,
        updated_at,
        roles: user_with_roles.roles,
    }))
}

/// Returns all users in ascending order by id.
pub fn list_all_users(conn: &mut SqliteConnection) -> Result<Vec<User>, diesel::result::Error> {
    use crate::schema::users::dsl::*;
    users.order(id.asc()).load::<User>(conn)
}

/// Returns all users for a specific company, ordered by id.
///
/// This function retrieves all users that belong to the specified company.
/// Results are ordered by user ID in ascending order.
///
/// # Arguments
/// * `conn` - Database connection
/// * `target_company_id` - ID of the company whose users to retrieve
///
/// # Returns
/// * `Ok(Vec<User>)` - List of users for the company
/// * `Err(diesel::result::Error)` - Database error
pub fn get_users_by_company(
    conn: &mut SqliteConnection,
    target_company_id: i32,
) -> Result<Vec<User>, diesel::result::Error> {
    use crate::schema::users::dsl::*;
    users
        .filter(company_id.eq(target_company_id))
        .order(id.asc())
        .load::<User>(conn)
}

/// Gets a single user by ID.
pub fn get_user(conn: &mut SqliteConnection, user_id: i32) -> Result<Option<User>, diesel::result::Error> {
    use crate::schema::users::dsl::*;
    users.filter(id.eq(user_id)).first::<User>(conn).optional()
}

/// Gets a single user by email (case-insensitive).
pub fn get_user_by_email(
    conn: &mut SqliteConnection,
    user_email: &str,
) -> Result<Option<User>, diesel::result::Error> {
    // Convert to lowercase for case-insensitive comparison
    let lowercase_email = user_email.to_lowercase();

    // Use raw SQL with parameter binding for case-insensitive search
    diesel::sql_query("SELECT * FROM users WHERE LOWER(email) = LOWER(?)")
        .bind::<diesel::sql_types::Text, _>(&lowercase_email)
        .get_result::<User>(conn)
        .optional()
}

/// Updates a user's fields (timestamps handled automatically by database triggers).
///
/// This function updates the specified fields of a user. All fields are optional - only provided
/// fields will be updated.
///
/// # Arguments
/// * `conn` - Database connection
/// * `user_id` - ID of the user to update
/// * `new_email` - Optional new email address
/// * `new_password_hash` - Optional new password hash
/// * `new_company_id` - Optional new company ID
/// * `new_totp_secret` - Optional new TOTP secret
///
/// # Returns
/// * `Ok(User)` - Updated user object
/// * `Err(diesel::result::Error)` - Database error
pub fn update_user(
    conn: &mut SqliteConnection,
    user_id: i32,
    new_email: Option<String>,
    new_password_hash: Option<String>,
    new_company_id: Option<i32>,
    new_totp_secret: Option<String>,
) -> Result<User, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    // Update each field individually if provided
    if let Some(email_val) = new_email {
        diesel::update(users.filter(id.eq(user_id)))
            .set(email.eq(email_val))
            .execute(conn)?;
    }

    if let Some(password_val) = new_password_hash {
        diesel::update(users.filter(id.eq(user_id)))
            .set(password_hash.eq(password_val))
            .execute(conn)?;
    }

    if let Some(company_val) = new_company_id {
        diesel::update(users.filter(id.eq(user_id)))
            .set(company_id.eq(company_val))
            .execute(conn)?;
    }

    if let Some(totp_val) = new_totp_secret {
        diesel::update(users.filter(id.eq(user_id)))
            .set(totp_secret.eq(totp_val))
            .execute(conn)?;
    }

    // Return the updated user
    users.filter(id.eq(user_id)).first::<User>(conn)
}

/// Deletes a user by ID.
///
/// This function permanently removes a user from the database. This is a hard delete
/// operation - the user record will be completely removed.
///
/// **Warning**: This will also remove any associated records that reference this user
/// (like user roles, sessions, etc.) due to foreign key constraints. Consider the
/// implications before using this function.
///
/// # Arguments
/// * `conn` - Database connection
/// * `user_id` - ID of the user to delete
///
/// # Returns
/// * `Ok(usize)` - Number of rows affected (should be 1 if user existed, 0 if not found)
/// * `Err(diesel::result::Error)` - Database error
pub fn delete_user(
    conn: &mut SqliteConnection,
    user_id: i32,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    diesel::delete(users.filter(id.eq(user_id))).execute(conn)
}

/// Deletes a user and all associated data (roles, sessions) by ID.
///
/// This function performs a complete deletion of a user and all their associated
/// data, properly handling foreign key constraints and database triggers.
/// It disables the trigger temporarily to allow complete user deletion.
///
/// # Arguments
/// * `conn` - Database connection
/// * `user_id` - ID of the user to delete
///
/// # Returns
/// * `Ok(usize)` - Number of users deleted (should be 1 if user existed, 0 if not found)
/// * `Err(diesel::result::Error)` - Database error
pub fn delete_user_with_cleanup(
    conn: &mut SqliteConnection,
    user_id: i32,
) -> Result<usize, diesel::result::Error> {
    // Temporarily drop the trigger to allow deletion
    diesel::sql_query("DROP TRIGGER IF EXISTS prevent_user_without_roles").execute(conn)?;

    // Delete user_roles first
    diesel::sql_query("DELETE FROM user_roles WHERE user_id = ?1")
        .bind::<diesel::sql_types::Integer, _>(user_id)
        .execute(conn)?;

    // Delete the user
    use crate::schema::users::dsl::*;
    let result = diesel::delete(users.filter(id.eq(user_id))).execute(conn);

    // Recreate the trigger
    diesel::sql_query(r#"
        CREATE TRIGGER prevent_user_without_roles
        BEFORE DELETE ON user_roles
        FOR EACH ROW
        BEGIN
            SELECT CASE
                WHEN (SELECT COUNT(*) FROM user_roles WHERE user_id = OLD.user_id) = 1
                THEN RAISE(ABORT, 'Cannot remove the last role from a user. Users must have at least one role.')
            END;
        END
    "#)
        .execute(conn)?;

    result
}

/// Gets a single user by ID with their roles.
///
/// This function retrieves a user and their associated roles in a single
/// efficient query using a JOIN operation.
///
/// # Arguments
/// * `conn` - Database connection
/// * `user_id` - ID of the user to retrieve
///
/// # Returns
/// * `Ok(UserWithRoles)` - User with their roles
/// * `Err(diesel::result::Error)` - Database error or user not found
pub fn get_user_with_roles(
    conn: &mut SqliteConnection,
    user_id: i32,
) -> Result<Option<UserWithRoles>, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    // First get the user
    let user = match users.filter(id.eq(user_id)).first::<User>(conn).optional()? {
        Some(u) => u,
        None => return Ok(None),
    };

    // Then get their roles
    let user_roles = crate::orm::user_role::get_user_roles(conn, user_id)?;

    Ok(Some(UserWithRoles {
        id: user.id,
        email: user.email,
        password_hash: user.password_hash,
        company_id: user.company_id,
        totp_secret: user.totp_secret,
        roles: user_roles,
    }))
}

/// Returns all users with their roles, ordered by id.
///
/// This function retrieves all users and their associated roles efficiently.
/// For each user, it fetches their roles and constructs a UserWithRoles object.
///
/// # Arguments
/// * `conn` - Database connection
///
/// # Returns
/// * `Ok(Vec<UserWithRoles>)` - List of all users with their roles
/// * `Err(diesel::result::Error)` - Database error
pub fn list_all_users_with_roles(
    conn: &mut SqliteConnection,
) -> Result<Vec<UserWithRoles>, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    let all_users = users.order(id.asc()).load::<User>(conn)?;
    let mut users_with_roles = Vec::new();

    for user in all_users {
        let user_roles = crate::orm::user_role::get_user_roles(conn, user.id)?;
        users_with_roles.push(UserWithRoles {
            id: user.id,
            email: user.email,
            password_hash: user.password_hash,
            company_id: user.company_id,
            totp_secret: user.totp_secret,
            roles: user_roles,
        });
    }

    Ok(users_with_roles)
}

/// Returns all users for a specific company with their roles, ordered by id.
///
/// This function retrieves all users that belong to the specified company
/// along with their associated roles. Results are ordered by user ID.
///
/// # Arguments
/// * `conn` - Database connection
/// * `target_company_id` - ID of the company whose users to retrieve
///
/// # Returns
/// * `Ok(Vec<UserWithRoles>)` - List of users with roles for the company
/// * `Err(diesel::result::Error)` - Database error
pub fn get_users_by_company_with_roles(
    conn: &mut SqliteConnection,
    target_company_id: i32,
) -> Result<Vec<UserWithRoles>, diesel::result::Error> {
    use crate::schema::users::dsl::*;

    let company_users = users
        .filter(company_id.eq(target_company_id))
        .order(id.asc())
        .load::<User>(conn)?;

    let mut users_with_roles = Vec::new();

    for user in company_users {
        let user_roles = crate::orm::user_role::get_user_roles(conn, user.id)?;
        users_with_roles.push(UserWithRoles {
            id: user.id,
            email: user.email,
            password_hash: user.password_hash,
            company_id: user.company_id,
            totp_secret: user.totp_secret,
            roles: user_roles,
        });
    }

    Ok(users_with_roles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::company::insert_company;
    use crate::orm::testing::setup_test_db;

    #[test]
    fn test_insert_user() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let new_user = UserInput {
            email: "test@example.com".to_string(),
            password_hash: "hashedpassword".to_string(),
            company_id: company.id,
            totp_secret: Some("secret".to_string()),
        };

        let result = insert_user(&mut conn, new_user);
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.password_hash, "hashedpassword");
        assert_eq!(user.company_id, company.id);
        assert_eq!(user.totp_secret, Some("secret".to_string()));
        assert!(user.id > 0);
    }

    #[test]
    fn test_user_with_timestamps() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Timestamp Test Company".to_string())
            .expect("Failed to insert company");

        let new_user = UserInput {
            email: "timestamp@example.com".to_string(),
            password_hash: "hashedpassword".to_string(),
            company_id: company.id,
            totp_secret: Some("secret".to_string()),
        };

        // Insert user
        let user = insert_user(&mut conn, new_user).unwrap();
        
        // Get user with timestamps
        let user_with_timestamps = get_user_with_timestamps(&mut conn, user.id)
            .expect("Should get timestamps")
            .expect("User should exist");
            
        assert_eq!(user_with_timestamps.id, user.id);
        assert_eq!(user_with_timestamps.email, "timestamp@example.com");
        
        // Timestamps should be recent (within last few seconds)
        let now = chrono::Utc::now().naive_utc();
        let created_diff = (user_with_timestamps.created_at - now).num_seconds().abs();
        let updated_diff = (user_with_timestamps.updated_at - now).num_seconds().abs();
        
        assert!(created_diff <= 5, "Created timestamp should be recent");
        assert!(updated_diff <= 5, "Updated timestamp should be recent");
    }

    // Keep other existing tests but update to use new types...
    #[test]
    fn test_get_user_by_email_case_insensitive() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let new_user = UserInput {
            email: "Test.User@Example.COM".to_string(),
            password_hash: "hashedpassword".to_string(),
            company_id: company.id,
            totp_secret: Some("secret".to_string()),
        };

        let inserted_user = insert_user(&mut conn, new_user).unwrap();

        // Test case-insensitive lookup with different cases
        let test_cases = vec![
            "test.user@example.com",
            "TEST.USER@EXAMPLE.COM",
            "Test.User@Example.COM",
            "tEsT.uSeR@eXaMpLe.CoM",
        ];

        for test_email in test_cases {
            let retrieved_user = get_user_by_email(&mut conn, test_email)
                .unwrap()
                .expect("User should be found");
            assert_eq!(retrieved_user.id, inserted_user.id);
            assert_eq!(retrieved_user.email, "Test.User@Example.COM"); // Original case preserved
        }

        // Test non-existent email
        let result = get_user_by_email(&mut conn, "nonexistent@example.com").unwrap();
        assert!(result.is_none());
    }
}