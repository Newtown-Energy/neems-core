use diesel::prelude::*;
use crate::models::{Role, NewUserRole};

/// Assigns a role to a user
pub fn assign_user_role(
    conn: &mut SqliteConnection,
    user_id_param: i32,
    role_id_param: i32,
) -> Result<(), diesel::result::Error> {
    use crate::schema::user_roles::dsl::*;

    let new_user_role = NewUserRole {
        user_id: user_id_param,
        role_id: role_id_param,
    };

    diesel::insert_into(user_roles)
        .values(&new_user_role)
        .execute(conn)?;

    Ok(())
}

/// Removes a role from a user
pub fn remove_user_role(
    conn: &mut SqliteConnection,
    user_id_param: i32,
    role_id_param: i32,
) -> Result<(), diesel::result::Error> {
    use crate::schema::user_roles::dsl::*;

    diesel::delete(
        user_roles
            .filter(user_id.eq(user_id_param))
            .filter(role_id.eq(role_id_param))
    )
    .execute(conn)?;

    Ok(())
}

/// Gets all roles for a specific user
pub fn get_user_roles(
    conn: &mut SqliteConnection,
    user_id_param: i32,
) -> Result<Vec<Role>, diesel::result::Error> {
    use crate::schema::roles::dsl::*;
    use crate::schema::user_roles;

    roles
        .inner_join(user_roles::table.on(id.eq(user_roles::role_id)))
        .filter(user_roles::user_id.eq(user_id_param))
        .select((id, name, description))
        .load::<Role>(conn)
}

/// Checks if a user has a specific role by role name
pub fn user_has_role(
    conn: &mut SqliteConnection,
    user_id_param: i32,
    role_name: &str,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::roles::dsl::*;
    use crate::schema::user_roles;

    let count: i64 = roles
        .inner_join(user_roles::table.on(id.eq(user_roles::role_id)))
        .filter(user_roles::user_id.eq(user_id_param))
        .filter(name.eq(role_name))
        .count()
        .get_result(conn)?;

    Ok(count > 0)
}

/// Assigns a role to a user by role name (convenience function)
pub fn assign_user_role_by_name(
    conn: &mut SqliteConnection,
    user_id_param: i32,
    role_name: &str,
) -> Result<(), diesel::result::Error> {
    use crate::schema::roles::dsl::*;

    let role = roles
        .filter(name.eq(role_name))
        .first::<Role>(conn)?;

    assign_user_role(conn, user_id_param, role.id)
}

/// Removes a role from a user by role name (convenience function)
pub fn remove_user_role_by_name(
    conn: &mut SqliteConnection,
    user_id_param: i32,
    role_name: &str,
) -> Result<(), diesel::result::Error> {
    use crate::schema::roles::dsl::*;

    let role = roles
        .filter(name.eq(role_name))
        .first::<Role>(conn)?;

    remove_user_role(conn, user_id_param, role.id)
}

/// Removes all roles from a user
/// 
/// This function removes all role assignments for a specific user.
/// Used primarily when deleting a user to ensure referential integrity.
///
/// # Arguments
/// * `conn` - Database connection
/// * `user_id_param` - ID of the user whose roles to remove
///
/// # Returns
/// * `Ok(usize)` - Number of role assignments removed
/// * `Err(diesel::result::Error)` - Database error
pub fn remove_all_user_roles(
    conn: &mut SqliteConnection,
    user_id_param: i32,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::user_roles::dsl::*;

    diesel::delete(user_roles.filter(user_id.eq(user_id_param)))
        .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;
    use crate::orm::role::get_all_roles;
    use crate::orm::user::insert_user;
    use crate::models::UserNoTime;
    use crate::orm::login::hash_password;

    #[test]
    fn test_assign_and_get_user_roles() {
        let mut conn = setup_test_db();
        
        // Get available roles
        let roles = get_all_roles(&mut conn).unwrap();
        let admin_role = roles.iter().find(|r| r.name == "newtown-admin").unwrap();
        let staff_role = roles.iter().find(|r| r.name == "newtown-staff").unwrap();

        // Create a test user
        let user = insert_user(&mut conn, UserNoTime {
            email: "test@example.com".to_string(),
            password_hash: hash_password("password"),
            company_id: 1,
            totp_secret: "secret".to_string(),
        }).unwrap();

        // Assign roles
        assign_user_role(&mut conn, user.id, admin_role.id).unwrap();
        assign_user_role(&mut conn, user.id, staff_role.id).unwrap();

        // Get user roles
        let user_roles = get_user_roles(&mut conn, user.id).unwrap();
        assert_eq!(user_roles.len(), 2);
        
        let role_names: Vec<&str> = user_roles.iter().map(|r| r.name.as_str()).collect();
        assert!(role_names.contains(&"newtown-admin"));
        assert!(role_names.contains(&"newtown-staff"));
    }

    #[test]
    fn test_user_has_role() {
        let mut conn = setup_test_db();
        
        // Create a test user
        let user = insert_user(&mut conn, UserNoTime {
            email: "test2@example.com".to_string(),
            password_hash: hash_password("password"),
            company_id: 1,
            totp_secret: "secret".to_string(),
        }).unwrap();

        // Initially user has no roles
        assert!(!user_has_role(&mut conn, user.id, "newtown-admin").unwrap());

        // Assign a role
        assign_user_role_by_name(&mut conn, user.id, "newtown-admin").unwrap();

        // Now user has the role
        assert!(user_has_role(&mut conn, user.id, "newtown-admin").unwrap());
        assert!(!user_has_role(&mut conn, user.id, "newtown-staff").unwrap());
    }

    #[test]
    fn test_remove_user_role() {
        let mut conn = setup_test_db();
        
        // Create a test user
        let user = insert_user(&mut conn, UserNoTime {
            email: "test3@example.com".to_string(),
            password_hash: hash_password("password"),
            company_id: 1,
            totp_secret: "secret".to_string(),
        }).unwrap();

        // Assign multiple roles so we can safely remove one
        assign_user_role_by_name(&mut conn, user.id, "newtown-admin").unwrap();
        assign_user_role_by_name(&mut conn, user.id, "newtown-staff").unwrap();
        assert!(user_has_role(&mut conn, user.id, "newtown-admin").unwrap());
        assert!(user_has_role(&mut conn, user.id, "newtown-staff").unwrap());

        // Remove one role (user still has another)
        remove_user_role_by_name(&mut conn, user.id, "newtown-admin").unwrap();
        assert!(!user_has_role(&mut conn, user.id, "newtown-admin").unwrap());
        assert!(user_has_role(&mut conn, user.id, "newtown-staff").unwrap());
    }

    #[test]
    fn test_cannot_remove_last_role() {
        let mut conn = setup_test_db();
        
        // Create a test user
        let user = insert_user(&mut conn, UserNoTime {
            email: "test4@example.com".to_string(),
            password_hash: hash_password("password"),
            company_id: 1,
            totp_secret: "secret".to_string(),
        }).unwrap();

        // Assign only one role
        assign_user_role_by_name(&mut conn, user.id, "newtown-admin").unwrap();
        assert!(user_has_role(&mut conn, user.id, "newtown-admin").unwrap());

        // Try to remove the last role - this should fail due to our constraint
        let result = remove_user_role_by_name(&mut conn, user.id, "newtown-admin");
        assert!(result.is_err());
        
        // User should still have the role
        assert!(user_has_role(&mut conn, user.id, "newtown-admin").unwrap());
    }
}