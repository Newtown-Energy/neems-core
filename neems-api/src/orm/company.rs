use diesel::QueryableByName;
use diesel::prelude::*;
use diesel::sql_types::BigInt;

use crate::models::{Company, CompanyInput, CompanyWithTimestamps, NewCompany};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Try to find a company by name (case-sensitive).
/// Returns Ok(Some(Company)) if found, Ok(None) if not, Err on DB error.
pub fn get_company_by_name(
    conn: &mut SqliteConnection,
    comp: &CompanyInput,
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

/// Insert a new company (timestamps handled automatically by database triggers)
pub fn insert_company(
    conn: &mut SqliteConnection,
    comp_name: String,
    acting_user_id: Option<i32>,
) -> Result<Company, diesel::result::Error> {
    use crate::schema::companies::dsl::*;

    let new_comp = NewCompany {
        name: comp_name,
    };

    diesel::insert_into(companies)
        .values(&new_comp)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    let company = companies
        .filter(id.eq(last_id as i32))
        .first::<Company>(conn)?;
    
    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "companies", company.id, "create", user_id);
    }
    
    Ok(company)
}

/// Get a company with computed timestamps from activity log
pub fn get_company_with_timestamps(
    conn: &mut SqliteConnection,
    company_id: i32,
) -> Result<Option<CompanyWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity;
    
    // First get the company
    let company = match get_company_by_id(conn, company_id)? {
        Some(comp) => comp,
        None => return Ok(None),
    };

    // Get timestamps from activity log
    let created_at = entity_activity::get_created_at(conn, "companies", company_id)?;
    let updated_at = entity_activity::get_updated_at(conn, "companies", company_id)?;

    Ok(Some(CompanyWithTimestamps {
        id: company.id,
        name: company.name,
        created_at,
        updated_at,
    }))
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
    acting_user_id: Option<i32>,
) -> Result<bool, diesel::result::Error> {
    // First check if the company exists and get it for archiving
    use crate::schema::companies::dsl::*;
    let company_to_delete = match companies.filter(id.eq(company_id)).first::<crate::models::Company>(conn) {
        Ok(company) => company,
        Err(diesel::result::Error::NotFound) => {
            // Company doesn't exist, return false (not found)
            return Ok(false);
        }
        Err(e) => return Err(e), // Other database errors
    };
    
    // Insert into deleted_companies table
    use crate::models::NewDeletedCompany;
    use crate::schema::deleted_companies;
    let archived_company = NewDeletedCompany {
        id: company_to_delete.id,
        name: company_to_delete.name,
        deleted_by: acting_user_id,
    };
    
    diesel::insert_into(deleted_companies::table)
        .values(&archived_company)
        .execute(conn)?;

    // Delete the company
    let rows_affected = diesel::delete(companies.filter(id.eq(company_id))).execute(conn)?;
    
    // Update the trigger-created activity entry with user information
    if rows_affected > 0 {
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ = update_latest_activity_user(conn, "companies", company_id, "delete", user_id);
        }
    }
    
    Ok(rows_affected > 0)
}

/// Gets company information for audit purposes, checking both active and deleted companies.
///
/// This function first checks the active companies table, and if not found, 
/// checks the deleted_companies table to provide information for audit trails.
///
/// # Arguments
/// * `conn` - Database connection
/// * `company_id` - ID of the company to look up
///
/// # Returns
/// * `Ok(Some((name, is_deleted)))` - Company found with name and deletion status
/// * `Ok(None)` - Company not found in either table
/// * `Err(diesel::result::Error)` - Database error
pub fn get_company_for_audit(
    conn: &mut SqliteConnection,
    company_id: i32,
) -> Result<Option<(String, bool)>, diesel::result::Error> {
    // First check active companies
    use crate::schema::companies::dsl::{companies, id as companies_id};
    if let Ok(company) = companies.filter(companies_id.eq(company_id)).first::<crate::models::Company>(conn) {
        return Ok(Some((company.name, false))); // Found active company
    }
    
    // If not found in active companies, check deleted companies
    use crate::schema::deleted_companies::dsl::{deleted_companies, id as deleted_companies_id};
    if let Ok(deleted_company) = deleted_companies.filter(deleted_companies_id.eq(company_id)).first::<crate::models::DeletedCompany>(conn) {
        return Ok(Some((deleted_company.name, true))); // Found deleted company
    }
    
    // Not found in either table
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;

    #[test]
    fn test_insert_company() {
        let mut conn = setup_test_db();
        let result = insert_company(&mut conn, "Test Company".to_string(), None);
        assert!(result.is_ok());
        let comp = result.unwrap();
        assert_eq!(comp.name, "Test Company");
        assert!(comp.id > 0);
    }

    #[test]
    fn test_company_with_timestamps() {
        let mut conn = setup_test_db();
        
        // Insert company
        let company = insert_company(&mut conn, "Timestamp Test Company".to_string(), None).unwrap();
        
        // Get company with timestamps
        let company_with_timestamps = get_company_with_timestamps(&mut conn, company.id)
            .expect("Should get timestamps")
            .expect("Company should exist");
            
        assert_eq!(company_with_timestamps.id, company.id);
        assert_eq!(company_with_timestamps.name, "Timestamp Test Company");
        
        // Timestamps should be recent (within last few seconds)
        let now = chrono::Utc::now().naive_utc();
        let created_diff = (company_with_timestamps.created_at - now).num_seconds().abs();
        let updated_diff = (company_with_timestamps.updated_at - now).num_seconds().abs();
        
        assert!(created_diff <= 5, "Created timestamp should be recent");
        assert!(updated_diff <= 5, "Updated timestamp should be recent");
    }

    #[test]
    fn test_get_company_by_name_case_insensitive() {
        let mut conn = setup_test_db();

        // Insert a company with mixed case name
        let created_company = insert_company(&mut conn, "Test Company Name".to_string(), None)
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