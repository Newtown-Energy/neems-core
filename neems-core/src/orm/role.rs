use diesel::prelude::*;
use diesel::sql_types::BigInt;

use crate::models::{Role, NewRole};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

pub fn insert_role(
    conn: &mut SqliteConnection,
    new_role: NewRole,
) -> Result<Role, diesel::result::Error> {
    use crate::schema::roles::dsl::*;

    diesel::insert_into(roles)
        .values(&new_role)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    roles
        .filter(id.eq(last_id as i32))
        .first::<Role>(conn)
}

/// Returns all roles in ascending order by id.
pub fn get_all_roles(
    conn: &mut SqliteConnection,
) -> Result<Vec<Role>, diesel::result::Error> {
    use crate::schema::roles::dsl::*;
    roles.order(id.asc()).load::<Role>(conn)
}

/// Gets a single role by ID.
///
/// This function retrieves a role from the database by its ID.
///
/// # Arguments
/// * `conn` - Database connection
/// * `role_id` - ID of the role to retrieve
///
/// # Returns
/// * `Ok(Role)` - The role if found
/// * `Err(diesel::result::Error)` - Database error (including NotFound if role doesn't exist)
pub fn get_role(
    conn: &mut SqliteConnection,
    role_id: i32,
) -> Result<Role, diesel::result::Error> {
    use crate::schema::roles::dsl::*;
    roles.filter(id.eq(role_id)).first::<Role>(conn)
}

/// Gets a single role by name.
///
/// This function retrieves a role from the database by its name.
///
/// # Arguments
/// * `conn` - Database connection
/// * `role_name` - Name of the role to retrieve
///
/// # Returns
/// * `Ok(Option<Role>)` - The role if found, None if not found
/// * `Err(diesel::result::Error)` - Database error
pub fn get_role_by_name(
    conn: &mut SqliteConnection,
    role_name: &str,
) -> Result<Option<Role>, diesel::result::Error> {
    use crate::schema::roles::dsl::*;
    roles.filter(name.eq(role_name)).first::<Role>(conn).optional()
}

/// Updates a role's fields.
///
/// This function updates the specified fields of a role. All fields are optional -
/// only provided fields will be updated.
///
/// # Arguments
/// * `conn` - Database connection
/// * `role_id` - ID of the role to update
/// * `new_name` - Optional new role name
/// * `new_description` - Optional new description (None to keep current, Some(None) to set to null)
///
/// # Returns
/// * `Ok(Role)` - Updated role object
/// * `Err(diesel::result::Error)` - Database error
pub fn update_role(
    conn: &mut SqliteConnection,
    role_id: i32,
    new_name: Option<String>,
    new_description: Option<Option<String>>,
) -> Result<Role, diesel::result::Error> {
    use crate::schema::roles::dsl::*;

    // Update each field individually if provided
    if let Some(name_val) = new_name {
        diesel::update(roles.filter(id.eq(role_id)))
            .set(name.eq(name_val))
            .execute(conn)?;
    }

    if let Some(description_val) = new_description {
        diesel::update(roles.filter(id.eq(role_id)))
            .set(description.eq(description_val))
            .execute(conn)?;
    }

    // Return the updated role
    roles.filter(id.eq(role_id)).first::<Role>(conn)
}

/// Deletes a role by ID.
///
/// This function permanently removes a role from the database. This is a hard delete
/// operation - the role record will be completely removed.
///
/// **Warning**: This will also affect any user_roles records that reference this role
/// due to foreign key constraints. Make sure to handle user role assignments before
/// deleting roles.
///
/// # Arguments
/// * `conn` - Database connection
/// * `role_id` - ID of the role to delete
///
/// # Returns
/// * `Ok(usize)` - Number of rows affected (should be 1 if role existed, 0 if not found)
/// * `Err(diesel::result::Error)` - Database error
pub fn delete_role(
    conn: &mut SqliteConnection,
    role_id: i32,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::roles::dsl::*;

    diesel::delete(roles.filter(id.eq(role_id)))
        .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;

    #[test]
    fn test_insert_role() {
        let mut conn = setup_test_db();
        let new_role = NewRole {
            name: "Test Role".to_string(),
            description: Some("A role for testing".to_string()),
        };

        let result = insert_role(&mut conn, new_role);
        assert!(result.is_ok());

        let role = result.unwrap();
        assert_eq!(role.name, "Test Role");
        assert_eq!(role.description, Some("A role for testing".to_string()));
        // id should be > 0
        assert!(role.id > 0);
    }

    #[test]
    fn test_get_all_roles() {
	let mut conn = setup_test_db();

	let roles = get_all_roles(&mut conn).unwrap();
	assert_eq!(roles.len(), 4);

	// Check ordering and content
	assert_eq!(roles[0].name, "newtown-admin");
	assert_eq!(roles[1].name, "newtown-staff");
	assert_eq!(roles[0].description, Some("Administrator for Newtown".to_string()));
	assert_eq!(roles[1].description, Some("Staff member for Newtown".to_string()));

	// IDs should be present and ascending
	assert!(roles[0].id > 0);
	assert!(roles[1].id > 0);
	assert!(roles[0].id < roles[1].id);
    }

    #[test]
    fn test_get_role() {
        let mut conn = setup_test_db();

        // Insert a test role
        let new_role = NewRole {
            name: "Get Test Role".to_string(),
            description: Some("A role for get testing".to_string()),
        };

        let inserted_role = insert_role(&mut conn, new_role).unwrap();

        // Test getting the role by ID
        let retrieved_role = get_role(&mut conn, inserted_role.id).unwrap();

        assert_eq!(retrieved_role.id, inserted_role.id);
        assert_eq!(retrieved_role.name, "Get Test Role");
        assert_eq!(retrieved_role.description, Some("A role for get testing".to_string()));
    }

    #[test]
    fn test_get_role_not_found() {
        let mut conn = setup_test_db();

        // Test getting a role that doesn't exist
        let result = get_role(&mut conn, 99999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_role_by_name() {
        let mut conn = setup_test_db();

        // Insert a test role
        let new_role = NewRole {
            name: "Named Test Role".to_string(),
            description: Some("A role for name testing".to_string()),
        };

        let inserted_role = insert_role(&mut conn, new_role).unwrap();

        // Test getting the role by name
        let retrieved_role = get_role_by_name(&mut conn, "Named Test Role").unwrap();
        assert!(retrieved_role.is_some());

        let role = retrieved_role.unwrap();
        assert_eq!(role.id, inserted_role.id);
        assert_eq!(role.name, "Named Test Role");
        assert_eq!(role.description, Some("A role for name testing".to_string()));

        // Test getting a role that doesn't exist by name
        let not_found = get_role_by_name(&mut conn, "Nonexistent Role").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_update_role() {
        let mut conn = setup_test_db();

        // Insert a test role
        let new_role = NewRole {
            name: "Update Test Role".to_string(),
            description: Some("Original description".to_string()),
        };

        let inserted_role = insert_role(&mut conn, new_role).unwrap();

        // Test updating name only
        let updated_role = update_role(
            &mut conn,
            inserted_role.id,
            Some("Updated Name".to_string()),
            None,
        ).unwrap();

        assert_eq!(updated_role.id, inserted_role.id);
        assert_eq!(updated_role.name, "Updated Name");
        assert_eq!(updated_role.description, Some("Original description".to_string())); // Unchanged

        // Test updating description only
        let updated_role2 = update_role(
            &mut conn,
            inserted_role.id,
            None,
            Some(Some("New description".to_string())),
        ).unwrap();

        assert_eq!(updated_role2.name, "Updated Name"); // From previous update
        assert_eq!(updated_role2.description, Some("New description".to_string())); // Updated

        // Test setting description to null
        let updated_role3 = update_role(
            &mut conn,
            inserted_role.id,
            None,
            Some(None),
        ).unwrap();

        assert_eq!(updated_role3.name, "Updated Name"); // Unchanged
        assert_eq!(updated_role3.description, None); // Set to null

        // Test updating both fields
        let updated_role4 = update_role(
            &mut conn,
            inserted_role.id,
            Some("Final Name".to_string()),
            Some(Some("Final description".to_string())),
        ).unwrap();

        assert_eq!(updated_role4.name, "Final Name");
        assert_eq!(updated_role4.description, Some("Final description".to_string()));
    }

    #[test]
    fn test_update_role_not_found() {
        let mut conn = setup_test_db();

        // Try to update a role that doesn't exist
        let result = update_role(
            &mut conn,
            99999,
            Some("New Name".to_string()),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_role() {
        let mut conn = setup_test_db();

        // Insert a test role
        let new_role = NewRole {
            name: "Delete Test Role".to_string(),
            description: Some("A role for delete testing".to_string()),
        };

        let inserted_role = insert_role(&mut conn, new_role).unwrap();

        // Verify role exists
        let retrieved_role = get_role(&mut conn, inserted_role.id);
        assert!(retrieved_role.is_ok());

        // Delete the role
        let rows_affected = delete_role(&mut conn, inserted_role.id).unwrap();
        assert_eq!(rows_affected, 1);

        // Verify role no longer exists
        let retrieved_role_after = get_role(&mut conn, inserted_role.id);
        assert!(retrieved_role_after.is_err());
    }

    #[test]
    fn test_delete_role_not_found() {
        let mut conn = setup_test_db();

        // Try to delete a role that doesn't exist
        let rows_affected = delete_role(&mut conn, 99999).unwrap();
        assert_eq!(rows_affected, 0);
    }

    // Additional edge case tests for role CRUD operations

    #[test]
    fn test_role_name_uniqueness() {
        let mut conn = setup_test_db();

        // Insert first role
        let role1 = NewRole {
            name: "Duplicate Name".to_string(),
            description: Some("First role".to_string()),
        };
        let _inserted1 = insert_role(&mut conn, role1).unwrap();

        // Try to insert role with same name (should fail due to unique constraint)
        let role2 = NewRole {
            name: "Duplicate Name".to_string(),
            description: Some("Second role".to_string()),
        };
        let result = insert_role(&mut conn, role2);
        assert!(result.is_err()); // Should fail due to unique constraint
    }

    #[test]
    fn test_role_with_null_description() {
        let mut conn = setup_test_db();

        // Insert role with null description
        let new_role = NewRole {
            name: "No Description Role".to_string(),
            description: None,
        };

        let inserted_role = insert_role(&mut conn, new_role).unwrap();
        assert_eq!(inserted_role.name, "No Description Role");
        assert_eq!(inserted_role.description, None);

        // Test getting by name
        let retrieved = get_role_by_name(&mut conn, "No Description Role").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, None);
    }

    #[test]
    fn test_update_role_partial_fields() {
        let mut conn = setup_test_db();

        // Insert a test role
        let new_role = NewRole {
            name: "Partial Update Test".to_string(),
            description: Some("Initial description".to_string()),
        };

        let inserted_role = insert_role(&mut conn, new_role).unwrap();

        // Update only name, keep description
        let updated_role = update_role(
            &mut conn,
            inserted_role.id,
            Some("New Name Only".to_string()),
            None, // Don't change description
        ).unwrap();

        assert_eq!(updated_role.name, "New Name Only");
        assert_eq!(updated_role.description, Some("Initial description".to_string()));

        // Update only description, keep name
        let updated_role2 = update_role(
            &mut conn,
            inserted_role.id,
            None, // Don't change name
            Some(Some("New Description Only".to_string())),
        ).unwrap();

        assert_eq!(updated_role2.name, "New Name Only"); // Unchanged from previous
        assert_eq!(updated_role2.description, Some("New Description Only".to_string()));
    }

    #[test]
    fn test_role_crud_full_cycle() {
        let mut conn = setup_test_db();

        // Create
        let new_role = NewRole {
            name: "Full Cycle Role".to_string(),
            description: Some("Testing full CRUD cycle".to_string()),
        };
        let created_role = insert_role(&mut conn, new_role).unwrap();
        assert_eq!(created_role.name, "Full Cycle Role");

        // Read by ID
        let read_role = get_role(&mut conn, created_role.id).unwrap();
        assert_eq!(read_role.id, created_role.id);

        // Read by Name
        let read_by_name = get_role_by_name(&mut conn, "Full Cycle Role").unwrap();
        assert!(read_by_name.is_some());
        assert_eq!(read_by_name.unwrap().id, created_role.id);

        // Update
        let updated_role = update_role(
            &mut conn,
            created_role.id,
            Some("Updated Full Cycle Role".to_string()),
            Some(Some("Updated description".to_string())),
        ).unwrap();
        assert_eq!(updated_role.name, "Updated Full Cycle Role");
        assert_eq!(updated_role.description, Some("Updated description".to_string()));

        // Verify update by reading again
        let verified_role = get_role(&mut conn, created_role.id).unwrap();
        assert_eq!(verified_role.name, "Updated Full Cycle Role");

        // Delete
        let rows_deleted = delete_role(&mut conn, created_role.id).unwrap();
        assert_eq!(rows_deleted, 1);

        // Verify deletion
        let deleted_result = get_role(&mut conn, created_role.id);
        assert!(deleted_result.is_err());

        // Verify by name lookup also fails
        let deleted_by_name = get_role_by_name(&mut conn, "Updated Full Cycle Role").unwrap();
        assert!(deleted_by_name.is_none());
    }
}