use diesel::prelude::*;
use diesel::QueryableByName;
use diesel::sql_types::BigInt;

use crate::models::{Institution, NewInstitution, InstitutionNoTime};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Try to find an institution by name (case-sensitive).
/// Returns Ok(Some(Institution)) if found, Ok(None) if not, Err on DB error.
pub fn get_institution_by_name(
    conn: &mut SqliteConnection,
    inst: &InstitutionNoTime,
) -> Result<Option<Institution>, diesel::result::Error> {
    use crate::schema::institutions::dsl::*;
    let result = institutions
        .filter(name.eq(&inst.name))
        .first::<Institution>(conn)
        .optional()?;
    Ok(result)
}

pub fn insert_institution(
    conn: &mut SqliteConnection, 
    inst_name: String,
) -> Result<Institution, diesel::result::Error> {
    use crate::schema::institutions::dsl::*;
    let now = chrono::Utc::now().naive_utc();

    let new_inst = NewInstitution {
        name: inst_name,
        created_at: Some(now),
        updated_at: Some(now),
    };

    diesel::insert_into(institutions)
        .values(&new_inst)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    institutions
        .filter(id.eq(last_id as i32))
        .first::<Institution>(conn)
}

/// Returns all institutions in ascending order by id.
pub fn get_all_institutions(
    conn: &mut SqliteConnection,
) -> Result<Vec<Institution>, diesel::result::Error> {
    use crate::schema::institutions::dsl::*;
    institutions
        .order(id.asc())
        .load::<Institution>(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db; 

    #[test]
    fn test_insert_institution() {
	let mut conn = setup_test_db();
	let result = insert_institution(&mut conn, "Test Institution".to_string());
	assert!(result.is_ok());
	let inst = result.unwrap();
	assert_eq!(inst.name, "Test Institution");

	let now = chrono::Utc::now().naive_utc();
	let diff_created = (inst.created_at - now).num_seconds().abs();
	let diff_updated = (inst.updated_at - now).num_seconds().abs();

	assert!(
	    diff_created <= 1,
	    "created_at should be within 1 second of now (diff: {})",
	    diff_created
	);
	assert!(
	    diff_updated <= 1,
	    "updated_at should be within 1 second of now (diff: {})",
	    diff_updated
	);
    }
}