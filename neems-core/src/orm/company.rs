use diesel::prelude::*;
use diesel::QueryableByName;
use diesel::sql_types::BigInt;

use crate::models::{Company, NewCompany, CompanyNoTime};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Try to find a company by name (case-sensitive).
/// Returns Ok(Some(Company)) if found, Ok(None) if not, Err on DB error.
pub fn get_company_by_name(
    conn: &mut SqliteConnection,
    comp: &CompanyNoTime,
) -> Result<Option<Company>, diesel::result::Error> {
    use crate::schema::companies::dsl::*;
    let result = companies
        .filter(name.eq(&comp.name))
        .first::<Company>(conn)
        .optional()?;
    Ok(result)
}

pub fn insert_company(
    conn: &mut SqliteConnection, 
    comp_name: String,
) -> Result<Company, diesel::result::Error> {
    use crate::schema::companies::dsl::*;
    let now = chrono::Utc::now().naive_utc();

    let new_comp = NewCompany {
        name: comp_name,
        created_at: Some(now),
        updated_at: Some(now),
    };

    diesel::insert_into(companies)
        .values(&new_comp)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    companies
        .filter(id.eq(last_id as i32))
        .first::<Company>(conn)
}

/// Returns all companies in ascending order by id.
/// Try to find a company by id.
/// Returns Ok(Some(Company)) if found, Ok(None) if not, Err on DB error.
pub fn get_company_by_id(
    conn: &mut SqliteConnection,
    company_id: i32,
) -> Result<Option<Company>, diesel::result::Error> {
    use crate::schema::companies::dsl::*;
    let result = companies
        .filter(id.eq(company_id))
        .first::<Company>(conn)
        .optional()?;
    Ok(result)
}

pub fn get_all_companies(
    conn: &mut SqliteConnection,
) -> Result<Vec<Company>, diesel::result::Error> {
    use crate::schema::companies::dsl::*;
    companies
        .order(id.asc())
        .load::<Company>(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db; 

    #[test]
    fn test_insert_company() {
	let mut conn = setup_test_db();
	let result = insert_company(&mut conn, "Test Company".to_string());
	assert!(result.is_ok());
	let comp = result.unwrap();
	assert_eq!(comp.name, "Test Company");

	let now = chrono::Utc::now().naive_utc();
	let diff_created = (comp.created_at - now).num_seconds().abs();
	let diff_updated = (comp.updated_at - now).num_seconds().abs();

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