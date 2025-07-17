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
        // id should be Some and > 0
        assert!(role.id.is_some());
        assert!(role.id.unwrap() > 0);
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
	assert!(roles[0].id.is_some());
	assert!(roles[1].id.is_some());
	assert!(roles[0].id < roles[1].id);
    }
}