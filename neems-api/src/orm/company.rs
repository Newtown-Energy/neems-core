use diesel::QueryableByName;
use diesel::prelude::*;
use diesel::sql_types::BigInt;

use crate::models::{Company, CompanyNoTime, NewCompany};

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

/// Try to find a company by name (case-insensitive).
/// Returns Ok(Some(Company)) if found, Ok(None) if not, Err on DB error.
pub fn get_company_by_name_case_insensitive(
    conn: &mut SqliteConnection,
    company_name: &str,
) -> Result<Option<Company>, diesel::result::Error> {
    // Use raw SQL for case-insensitive comparison
    diesel::sql_query("SELECT * FROM companies WHERE LOWER(name) = LOWER(?)")
        .bind::<diesel::sql_types::Text, _>(company_name)
        .get_result::<Company>(conn)
        .optional()
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
    companies.order(id.asc()).load::<Company>(conn)
}

/// Delete a company by id.
/// Returns Ok(true) if company was found and deleted, Ok(false) if not found, Err on DB error.
pub fn delete_company(
    conn: &mut SqliteConnection,
    company_id: i32,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::companies::dsl::*;
    let rows_affected = diesel::delete(companies.filter(id.eq(company_id))).execute(conn)?;
    Ok(rows_affected > 0)
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

    #[test]
    fn test_get_company_by_name_case_insensitive() {
        let mut conn = setup_test_db();

        // Insert a company with mixed case name
        let created_company = insert_company(&mut conn, "Test Company Name".to_string())
            .expect("Failed to insert company");

        // Test case-insensitive lookup with different cases
        let test_cases = vec![
            "test company name",
            "TEST COMPANY NAME",
            "Test Company Name",
            "tEsT cOmPaNy NaMe",
        ];

        for test_name in test_cases {
            let retrieved_company = get_company_by_name_case_insensitive(&mut conn, test_name)
                .expect("Query should succeed")
                .expect("Company should be found");
            assert_eq!(retrieved_company.id, created_company.id);
            assert_eq!(retrieved_company.name, "Test Company Name"); // Original case preserved
        }

        // Test non-existent company name
        let result = get_company_by_name_case_insensitive(&mut conn, "Non-existent Company")
            .expect("Query should succeed");
        assert!(result.is_none());
    }
}
