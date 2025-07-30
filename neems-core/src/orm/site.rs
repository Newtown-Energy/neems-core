use diesel::prelude::*;
use chrono::Utc;

use crate::models::{Site, NewSite};

/// Gets all sites for a specific company ID.
pub fn get_sites_by_company(
    conn: &mut SqliteConnection,
    comp_id: i32,
) -> Result<Vec<Site>, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    sites
        .filter(company_id.eq(comp_id))
        .order(id.asc())
        .load::<Site>(conn)
}

/// Creates a new site in the database.
pub fn insert_site(
    conn: &mut SqliteConnection,
    site_name: String,
    site_address: String,
    site_latitude: f64,
    site_longitude: f64,
    site_company_id: i32,
) -> Result<Site, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    
    let now = Utc::now().naive_utc();
    let new_site = NewSite {
        name: site_name,
        address: site_address,
        latitude: site_latitude,
        longitude: site_longitude,
        company_id: site_company_id,
        created_at: now,
        updated_at: now,
    };
    
    diesel::insert_into(sites)
        .values(&new_site)
        .execute(conn)?;
    
    // Return the inserted site
    sites.order(id.desc()).first::<Site>(conn)
}

/// Gets a site by its ID.
pub fn get_site_by_id(
    conn: &mut SqliteConnection,
    site_id: i32,
) -> Result<Option<Site>, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    sites
        .filter(id.eq(site_id))
        .first::<Site>(conn)
        .optional()
}

/// Gets a site by company ID and name (case-insensitive).
pub fn get_site_by_company_and_name(
    conn: &mut SqliteConnection,
    site_company_id: i32,
    site_name: &str,
) -> Result<Option<Site>, diesel::result::Error> {
    // Use raw SQL for case-insensitive comparison
    diesel::sql_query("SELECT * FROM sites WHERE company_id = ? AND LOWER(name) = LOWER(?)")
        .bind::<diesel::sql_types::Integer, _>(site_company_id)
        .bind::<diesel::sql_types::Text, _>(site_name)
        .get_result::<Site>(conn)
        .optional()
}

/// Gets all sites in the system.
pub fn get_all_sites(
    conn: &mut SqliteConnection,
) -> Result<Vec<Site>, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    sites.order(id.asc()).load::<Site>(conn)
}

/// Updates a site in the database.
pub fn update_site(
    conn: &mut SqliteConnection,
    site_id: i32,
    new_name: Option<String>,
    new_address: Option<String>,
    new_latitude: Option<f64>,
    new_longitude: Option<f64>,
    new_company_id: Option<i32>,
) -> Result<Site, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    
    let now = Utc::now().naive_utc();
    
    // First, get the current site to preserve existing values
    let current_site = sites.filter(id.eq(site_id)).first::<Site>(conn)?;
    
    // Update with new values or keep existing ones
    diesel::update(sites.filter(id.eq(site_id)))
        .set((
            name.eq(new_name.unwrap_or(current_site.name)),
            address.eq(new_address.unwrap_or(current_site.address)),
            latitude.eq(new_latitude.unwrap_or(current_site.latitude)),
            longitude.eq(new_longitude.unwrap_or(current_site.longitude)),
            company_id.eq(new_company_id.unwrap_or(current_site.company_id)),
            updated_at.eq(now),
        ))
        .execute(conn)?;
    
    // Return the updated site
    sites.filter(id.eq(site_id)).first::<Site>(conn)
}

/// Deletes a site from the database.
pub fn delete_site(
    conn: &mut SqliteConnection,
    site_id: i32,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    diesel::delete(sites.filter(id.eq(site_id))).execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;

    #[test]
    fn test_get_sites_by_company() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        // Use insert_site function instead of manual insertion
        insert_site(
            &mut conn,
            "Site 1".to_string(),
            "123 Main St".to_string(),
            40.7128,
            -74.0060,
            company.id,
        ).expect("Failed to insert site 1");

        insert_site(
            &mut conn,
            "Site 2".to_string(),
            "456 Oak Ave".to_string(),
            40.7589,
            -73.9851,
            company.id,
        ).expect("Failed to insert site 2");

        let sites = get_sites_by_company(&mut conn, company.id).unwrap();
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].name, "Site 1");
        assert_eq!(sites[1].name, "Site 2");
        assert!(sites[0].id < sites[1].id);
    }

    #[test]
    fn test_insert_site() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
        ).expect("Failed to insert site");

        assert_eq!(site.name, "Test Site");
        assert_eq!(site.address, "123 Test St");
        assert_eq!(site.latitude, 40.7128);
        assert_eq!(site.longitude, -74.0060);
        assert_eq!(site.company_id, company.id);
        assert!(site.id > 0);
    }

    #[test]
    fn test_get_site_by_id() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let created_site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
        ).expect("Failed to insert site");

        // Test getting existing site
        let retrieved_site = get_site_by_id(&mut conn, created_site.id)
            .expect("Failed to get site")
            .expect("Site should exist");

        assert_eq!(retrieved_site.id, created_site.id);
        assert_eq!(retrieved_site.name, "Test Site");
        assert_eq!(retrieved_site.address, "123 Test St");

        // Test getting non-existent site
        let non_existent = get_site_by_id(&mut conn, 99999)
            .expect("Query should succeed");
        assert!(non_existent.is_none());
    }

    #[test]
    fn test_get_all_sites() {
        let mut conn = setup_test_db();

        let company1 = crate::company::insert_company(&mut conn, "Company 1".to_string())
            .expect("Failed to insert company 1");
        let company2 = crate::company::insert_company(&mut conn, "Company 2".to_string())
            .expect("Failed to insert company 2");

        // Insert sites for different companies
        insert_site(&mut conn, "Site 1".to_string(), "Address 1".to_string(), 40.0, -74.0, company1.id)
            .expect("Failed to insert site 1");
        insert_site(&mut conn, "Site 2".to_string(), "Address 2".to_string(), 41.0, -75.0, company2.id)
            .expect("Failed to insert site 2");
        insert_site(&mut conn, "Site 3".to_string(), "Address 3".to_string(), 42.0, -76.0, company1.id)
            .expect("Failed to insert site 3");

        let all_sites = get_all_sites(&mut conn).expect("Failed to get all sites");
        assert_eq!(all_sites.len(), 3);

        // Should be ordered by id
        assert_eq!(all_sites[0].name, "Site 1");
        assert_eq!(all_sites[1].name, "Site 2");
        assert_eq!(all_sites[2].name, "Site 3");
    }

    #[test]
    fn test_update_site() {
        let mut conn = setup_test_db();

        let company1 = crate::company::insert_company(&mut conn, "Company 1".to_string())
            .expect("Failed to insert company 1");
        let company2 = crate::company::insert_company(&mut conn, "Company 2".to_string())
            .expect("Failed to insert company 2");

        let created_site = insert_site(
            &mut conn,
            "Original Site".to_string(),
            "Original Address".to_string(),
            40.0,
            -74.0,
            company1.id,
        ).expect("Failed to insert site");

        let original_created_at = created_site.created_at;
        let original_updated_at = created_site.updated_at;

        // Wait a moment to ensure updated_at changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Test partial update (only name)
        let updated_site = update_site(
            &mut conn,
            created_site.id,
            Some("Updated Site".to_string()),
            None,
            None,
            None,
            None,
        ).expect("Failed to update site");

        assert_eq!(updated_site.name, "Updated Site");
        assert_eq!(updated_site.address, "Original Address"); // Should remain unchanged
        assert_eq!(updated_site.latitude, 40.0);
        assert_eq!(updated_site.longitude, -74.0);
        assert_eq!(updated_site.company_id, company1.id);
        assert_eq!(updated_site.created_at, original_created_at); // Should not change
        assert!(updated_site.updated_at > original_updated_at); // Should be updated

        // Test full update
        let fully_updated_site = update_site(
            &mut conn,
            created_site.id,
            Some("Fully Updated Site".to_string()),
            Some("New Address".to_string()),
            Some(41.0),
            Some(-75.0),
            Some(company2.id),
        ).expect("Failed to fully update site");

        assert_eq!(fully_updated_site.name, "Fully Updated Site");
        assert_eq!(fully_updated_site.address, "New Address");
        assert_eq!(fully_updated_site.latitude, 41.0);
        assert_eq!(fully_updated_site.longitude, -75.0);
        assert_eq!(fully_updated_site.company_id, company2.id);
    }

    #[test]
    fn test_update_nonexistent_site() {
        let mut conn = setup_test_db();

        let result = update_site(
            &mut conn,
            99999,
            Some("Test".to_string()),
            None,
            None,
            None,
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_delete_site() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        let site1 = insert_site(&mut conn, "Site 1".to_string(), "Address 1".to_string(), 40.0, -74.0, company.id)
            .expect("Failed to insert site 1");
        let site2 = insert_site(&mut conn, "Site 2".to_string(), "Address 2".to_string(), 41.0, -75.0, company.id)
            .expect("Failed to insert site 2");

        // Verify both sites exist
        let all_sites_before = get_all_sites(&mut conn).expect("Failed to get sites");
        assert_eq!(all_sites_before.len(), 2);

        // Delete one site
        let deleted_count = delete_site(&mut conn, site1.id).expect("Failed to delete site");
        assert_eq!(deleted_count, 1);

        // Verify only one site remains
        let all_sites_after = get_all_sites(&mut conn).expect("Failed to get sites");
        assert_eq!(all_sites_after.len(), 1);
        assert_eq!(all_sites_after[0].id, site2.id);

        // Verify the deleted site is gone
        let deleted_site = get_site_by_id(&mut conn, site1.id).expect("Query should succeed");
        assert!(deleted_site.is_none());
    }

    #[test]
    fn test_delete_nonexistent_site() {
        let mut conn = setup_test_db();

        let deleted_count = delete_site(&mut conn, 99999).expect("Delete should succeed");
        assert_eq!(deleted_count, 0); // No rows affected
    }

    #[test]
    fn test_get_site_by_company_and_name_case_insensitive() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string())
            .expect("Failed to insert company");

        // Insert a site with mixed case name
        let created_site = insert_site(
            &mut conn,
            "Test Site Name".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
        ).expect("Failed to insert site");

        // Test case-insensitive lookup with different cases
        let test_cases = vec![
            "test site name",
            "TEST SITE NAME", 
            "Test Site Name",
            "tEsT sItE nAmE"
        ];

        for test_name in test_cases {
            let retrieved_site = get_site_by_company_and_name(&mut conn, company.id, test_name)
                .expect("Query should succeed")
                .expect("Site should be found");
            assert_eq!(retrieved_site.id, created_site.id);
            assert_eq!(retrieved_site.name, "Test Site Name"); // Original case preserved
        }

        // Test non-existent site name
        let result = get_site_by_company_and_name(&mut conn, company.id, "Non-existent Site")
            .expect("Query should succeed");
        assert!(result.is_none());

        // Test with different company (should not find the site)
        let other_company = crate::company::insert_company(&mut conn, "Other Company".to_string())
            .expect("Failed to insert other company");
        let result = get_site_by_company_and_name(&mut conn, other_company.id, "Test Site Name")
            .expect("Query should succeed");
        assert!(result.is_none());
    }
}
