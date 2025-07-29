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
        company_id: new_user.company_id,
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
pub fn get_user(
    conn: &mut SqliteConnection,
    user_id: i32,
) -> Result<User, diesel::result::Error> {
    use crate::schema::users::dsl::*;
    users.filter(id.eq(user_id)).first::<User>(conn)
}

/// Updates a user's fields.
/// 
/// This function updates the specified fields of a user and automatically
/// sets the `updated_at` timestamp. All fields are optional - only provided
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
    
    let now = chrono::Utc::now().naive_utc();
    
    // Update each field individually if provided
    if let Some(email_val) = new_email {
        diesel::update(users.filter(id.eq(user_id)))
            .set((email.eq(email_val), updated_at.eq(now)))
            .execute(conn)?;
    }
    
    if let Some(password_val) = new_password_hash {
        diesel::update(users.filter(id.eq(user_id)))
            .set((password_hash.eq(password_val), updated_at.eq(now)))
            .execute(conn)?;
    }
    
    if let Some(company_val) = new_company_id {
        diesel::update(users.filter(id.eq(user_id)))
            .set((company_id.eq(company_val), updated_at.eq(now)))
            .execute(conn)?;
    }
    
    if let Some(totp_val) = new_totp_secret {
        diesel::update(users.filter(id.eq(user_id)))
            .set((totp_secret.eq(totp_val), updated_at.eq(now)))
            .execute(conn)?;
    }
    
    // Always update the timestamp even if no other fields changed
    diesel::update(users.filter(id.eq(user_id)))
        .set(updated_at.eq(now))
        .execute(conn)?;
    
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
    
    diesel::delete(users.filter(id.eq(user_id)))
        .execute(conn)
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
    diesel::sql_query("DROP TRIGGER IF EXISTS prevent_user_without_roles")
        .execute(conn)?;
    
    // Delete user_roles first
    diesel::sql_query("DELETE FROM user_roles WHERE user_id = ?1")
        .bind::<diesel::sql_types::Integer, _>(user_id)
        .execute(conn)?;
    
    // Delete the user
    use crate::schema::users::dsl::*;
    let result = diesel::delete(users.filter(id.eq(user_id)))
        .execute(conn);
    
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;
    use crate::orm::company::insert_company;

    #[test]
    fn test_insert_user() {
        let mut conn = setup_test_db();

	let company = insert_company(&mut conn, "Test Company".to_string())
	    .expect("Failed to insert company");

        let new_user = UserNoTime {
            email: "test@example.com".to_string(),
            password_hash: "hashedpassword".to_string(),
            company_id: company.id,    // Use a valid company id for your test db
            totp_secret: Some("secret".to_string()),
        };

        let result = insert_user(&mut conn, new_user);
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.password_hash, "hashedpassword");
        assert_eq!(user.company_id, 2); // one more than our existing company, Newtown
        assert_eq!(user.totp_secret, Some("secret".to_string()));
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

	let company = insert_company(&mut conn, "Test Company".to_string())
	    .expect("Failed to insert company");

        // Insert two users
        let user1 = UserNoTime {
            email: "user1@example.com".to_string(),
            password_hash: "pw1".to_string(),
            company_id: company.id,
            totp_secret: Some("secret1".to_string()),
        };
        let user2 = UserNoTime {
            email: "user2@example.com".to_string(),
            password_hash: "pw2".to_string(),
            company_id: company.id,
            totp_secret: Some("secret2".to_string()),
        };

        let _ = insert_user(&mut conn, user1).unwrap();
        let _ = insert_user(&mut conn, user2).unwrap();

        let users = list_all_users(&mut conn).unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].email, "user1@example.com");
        assert_eq!(users[1].email, "user2@example.com");
        assert!(users[0].id < users[1].id);
    }

    #[test]
    fn test_get_user() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let new_user = UserNoTime {
            email: "gettest@example.com".to_string(),
            password_hash: "gethash".to_string(),
            company_id: company.id,
            totp_secret: Some("getsecret".to_string()),
        };

        let inserted_user = insert_user(&mut conn, new_user).unwrap();
        let retrieved_user = get_user(&mut conn, inserted_user.id).unwrap();

        assert_eq!(retrieved_user.id, inserted_user.id);
        assert_eq!(retrieved_user.email, "gettest@example.com");
        assert_eq!(retrieved_user.password_hash, "gethash");
        assert_eq!(retrieved_user.company_id, company.id);
        assert_eq!(retrieved_user.totp_secret, Some("getsecret".to_string()));
    }

    #[test]
    fn test_update_user() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let new_user = UserNoTime {
            email: "updatetest@example.com".to_string(),
            password_hash: "originalhash".to_string(),
            company_id: company.id,
            totp_secret: Some("originalsecret".to_string()),
        };

        let inserted_user = insert_user(&mut conn, new_user).unwrap();
        let original_updated_at = inserted_user.updated_at;

        // Wait a moment to ensure updated_at changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Update email only
        let updated_user = update_user(
            &mut conn,
            inserted_user.id,
            Some("newemail@example.com".to_string()),
            None,
            None,
            None,
        ).unwrap();

        assert_eq!(updated_user.id, inserted_user.id);
        assert_eq!(updated_user.email, "newemail@example.com");
        assert_eq!(updated_user.password_hash, "originalhash"); // Unchanged
        assert_eq!(updated_user.company_id, company.id); // Unchanged
        assert_eq!(updated_user.totp_secret, Some("originalsecret".to_string())); // Unchanged
        assert!(updated_user.updated_at > original_updated_at); // Should be updated

        // Update multiple fields
        let updated_user2 = update_user(
            &mut conn,
            inserted_user.id,
            None,
            Some("newhash".to_string()),
            None,
            Some("newsecret".to_string()),
        ).unwrap();

        assert_eq!(updated_user2.email, "newemail@example.com"); // From previous update
        assert_eq!(updated_user2.password_hash, "newhash"); // Updated
        assert_eq!(updated_user2.totp_secret, Some("newsecret".to_string())); // Updated
    }

    #[test]
    fn test_delete_user() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let new_user = UserNoTime {
            email: "deletetest@example.com".to_string(),
            password_hash: "deletehash".to_string(),
            company_id: company.id,
            totp_secret: Some("deletesecret".to_string()),
        };

        let inserted_user = insert_user(&mut conn, new_user).unwrap();

        // Verify user exists
        let retrieved_user = get_user(&mut conn, inserted_user.id);
        assert!(retrieved_user.is_ok());

        // Delete user
        let rows_affected = delete_user(&mut conn, inserted_user.id).unwrap();
        assert_eq!(rows_affected, 1);

        // Verify user no longer exists
        let retrieved_user_after = get_user(&mut conn, inserted_user.id);
        assert!(retrieved_user_after.is_err());
    }

    #[test]
    fn test_delete_nonexistent_user() {
        let mut conn = setup_test_db();

        // Try to delete a user that doesn't exist
        let rows_affected = delete_user(&mut conn, 99999).unwrap();
        assert_eq!(rows_affected, 0);
    }

    #[test]
    fn test_get_users_by_company() {
        let mut conn = setup_test_db();

        // Create two companies
        let company1 = insert_company(&mut conn, "Company 1".to_string())
            .expect("Failed to insert company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string())
            .expect("Failed to insert company 2");

        // Create users for company 1
        let user1_company1 = UserNoTime {
            email: "user1@company1.com".to_string(),
            password_hash: "hash1".to_string(),
            company_id: company1.id,
            totp_secret: Some("secret1".to_string()),
        };
        let user2_company1 = UserNoTime {
            email: "user2@company1.com".to_string(),
            password_hash: "hash2".to_string(),
            company_id: company1.id,
            totp_secret: Some("secret2".to_string()),
        };

        // Create user for company 2
        let user1_company2 = UserNoTime {
            email: "user1@company2.com".to_string(),
            password_hash: "hash3".to_string(),
            company_id: company2.id,
            totp_secret: Some("secret3".to_string()),
        };

        // Insert users
        let _ = insert_user(&mut conn, user1_company1).unwrap();
        let _ = insert_user(&mut conn, user2_company1).unwrap();
        let _ = insert_user(&mut conn, user1_company2).unwrap();

        // Test getting users for company 1
        let company1_users = get_users_by_company(&mut conn, company1.id).unwrap();
        assert_eq!(company1_users.len(), 2);
        assert_eq!(company1_users[0].email, "user1@company1.com");
        assert_eq!(company1_users[1].email, "user2@company1.com");
        assert!(company1_users[0].id < company1_users[1].id); // Should be ordered by ID

        // Test getting users for company 2
        let company2_users = get_users_by_company(&mut conn, company2.id).unwrap();
        assert_eq!(company2_users.len(), 1);
        assert_eq!(company2_users[0].email, "user1@company2.com");

        // Test getting users for non-existent company
        let no_users = get_users_by_company(&mut conn, 99999).unwrap();
        assert_eq!(no_users.len(), 0);
    }
}