use diesel::prelude::*;

use crate::models::{NewSite, Site, SiteWithTimestamps};

/// Partial update payload for [`update_site`]. Any field left `None` is
/// preserved at its current value; nullable demo fields cannot be cleared
/// through this struct (a future API can grow a double-`Option` if needed).
#[derive(Default, Debug, Clone)]
pub struct SiteUpdate {
    pub name: Option<String>,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub company_id: Option<i32>,
    pub ramp_duration_seconds: Option<i32>,
    pub power_kw: Option<f64>,
    pub capacity_kwh: Option<f64>,
    pub closed_loop_enabled: Option<bool>,
    pub off_peak_start_minutes: Option<i32>,
    pub off_peak_end_minutes: Option<i32>,
    pub peak_revenue_start_minutes: Option<i32>,
    pub peak_revenue_end_minutes: Option<i32>,
    pub interconnection_max_output_kw: Option<f64>,
    pub rebound_protection_soc_floor_percent: Option<f64>,
    pub site_variant: Option<String>,
}

/// Gets all sites for a specific company ID.
pub fn get_sites_by_company(
    conn: &mut SqliteConnection,
    comp_id: i32,
) -> Result<Vec<Site>, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    sites
        .filter(company_id.eq(comp_id))
        .order(id.asc())
        .select(Site::as_select())
        .load(conn)
}

/// Creates a new site in the database (timestamps handled automatically by
/// database triggers)
pub fn insert_site(
    conn: &mut SqliteConnection,
    site_name: String,
    site_address: String,
    site_latitude: f64,
    site_longitude: f64,
    site_company_id: i32,
    site_ramp_duration_seconds: i32,
    acting_user_id: Option<i32>,
) -> Result<Site, diesel::result::Error> {
    use crate::schema::sites::dsl::*;

    let new_site = NewSite {
        name: site_name,
        address: site_address,
        latitude: site_latitude,
        longitude: site_longitude,
        company_id: site_company_id,
        ramp_duration_seconds: site_ramp_duration_seconds,
    };

    diesel::insert_into(sites).values(&new_site).execute(conn)?;

    // Return the inserted site
    let site = sites.order(id.desc()).select(Site::as_select()).first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "sites", site.id, "create", user_id);
    }

    Ok(site)
}

/// Gets a site by its ID.
pub fn get_site_by_id(
    conn: &mut SqliteConnection,
    site_id: i32,
) -> Result<Option<Site>, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    sites.filter(id.eq(site_id)).select(Site::as_select()).first(conn).optional()
}

/// Gets a site by company ID and name (case-insensitive).
pub fn get_site_by_company_and_name(
    conn: &mut SqliteConnection,
    site_company_id: i32,
    site_name: &str,
) -> Result<Option<Site>, diesel::result::Error> {
    // Use raw SQL for case-insensitive comparison
    diesel::sql_query(
        "SELECT id, name, address, latitude, longitude, company_id, ramp_duration_seconds, \
         power_kw, capacity_kwh, closed_loop_enabled, off_peak_start_minutes, \
         off_peak_end_minutes, peak_revenue_start_minutes, peak_revenue_end_minutes, \
         interconnection_max_output_kw, rebound_protection_soc_floor_percent, site_variant \
         FROM sites WHERE company_id = ? AND LOWER(name) = LOWER(?)",
    )
    .bind::<diesel::sql_types::Integer, _>(site_company_id)
    .bind::<diesel::sql_types::Text, _>(site_name)
    .get_result::<Site>(conn)
    .optional()
}

/// Gets all sites in the system.
pub fn get_all_sites(conn: &mut SqliteConnection) -> Result<Vec<Site>, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    sites.order(id.asc()).select(Site::as_select()).load(conn)
}

/// Updates a site in the database (timestamps handled automatically by database
/// triggers).
pub fn update_site(
    conn: &mut SqliteConnection,
    site_id: i32,
    update: SiteUpdate,
    acting_user_id: Option<i32>,
) -> Result<Site, diesel::result::Error> {
    use crate::schema::sites::dsl::*;

    // First, get the current site to preserve existing values
    let current_site = sites.filter(id.eq(site_id)).select(Site::as_select()).first(conn)?;

    diesel::update(sites.filter(id.eq(site_id)))
        .set((
            name.eq(update.name.unwrap_or(current_site.name)),
            address.eq(update.address.unwrap_or(current_site.address)),
            latitude.eq(update.latitude.unwrap_or(current_site.latitude)),
            longitude.eq(update.longitude.unwrap_or(current_site.longitude)),
            company_id.eq(update.company_id.unwrap_or(current_site.company_id)),
            ramp_duration_seconds
                .eq(update.ramp_duration_seconds.unwrap_or(current_site.ramp_duration_seconds)),
            power_kw.eq(update.power_kw.or(current_site.power_kw)),
            capacity_kwh.eq(update.capacity_kwh.or(current_site.capacity_kwh)),
            closed_loop_enabled
                .eq(update.closed_loop_enabled.unwrap_or(current_site.closed_loop_enabled)),
            off_peak_start_minutes
                .eq(update.off_peak_start_minutes.or(current_site.off_peak_start_minutes)),
            off_peak_end_minutes
                .eq(update.off_peak_end_minutes.or(current_site.off_peak_end_minutes)),
            peak_revenue_start_minutes
                .eq(update.peak_revenue_start_minutes.or(current_site.peak_revenue_start_minutes)),
            peak_revenue_end_minutes
                .eq(update.peak_revenue_end_minutes.or(current_site.peak_revenue_end_minutes)),
            interconnection_max_output_kw.eq(update
                .interconnection_max_output_kw
                .or(current_site.interconnection_max_output_kw)),
            rebound_protection_soc_floor_percent.eq(update
                .rebound_protection_soc_floor_percent
                .unwrap_or(current_site.rebound_protection_soc_floor_percent)),
            site_variant.eq(update.site_variant.unwrap_or(current_site.site_variant)),
        ))
        .execute(conn)?;

    let site = sites.filter(id.eq(site_id)).select(Site::as_select()).first(conn)?;

    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "sites", site_id, "update", user_id);
    }

    Ok(site)
}

/// Deletes a site from the database.
pub fn delete_site(
    conn: &mut SqliteConnection,
    site_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::sites::dsl::*;
    let result = diesel::delete(sites.filter(id.eq(site_id))).execute(conn)?;

    // Update the trigger-created activity entry with user information
    if result > 0
        && let Some(user_id) = acting_user_id
    {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "sites", site_id, "delete", user_id);
    }

    Ok(result)
}

/// Get a site with computed timestamps from activity log
pub fn get_site_with_timestamps(
    conn: &mut SqliteConnection,
    site_id: i32,
) -> Result<Option<SiteWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity;

    // First get the site
    let site = match get_site_by_id(conn, site_id)? {
        Some(s) => s,
        None => return Ok(None),
    };

    // Get timestamps from activity log
    let created_at = entity_activity::get_created_at(conn, "sites", site_id)?;
    let updated_at = entity_activity::get_updated_at(conn, "sites", site_id)?;

    Ok(Some(SiteWithTimestamps {
        id: site.id,
        name: site.name,
        address: site.address,
        latitude: site.latitude,
        longitude: site.longitude,
        company_id: site.company_id,
        ramp_duration_seconds: site.ramp_duration_seconds,
        power_kw: site.power_kw,
        capacity_kwh: site.capacity_kwh,
        closed_loop_enabled: site.closed_loop_enabled,
        off_peak_start_minutes: site.off_peak_start_minutes,
        off_peak_end_minutes: site.off_peak_end_minutes,
        peak_revenue_start_minutes: site.peak_revenue_start_minutes,
        peak_revenue_end_minutes: site.peak_revenue_end_minutes,
        interconnection_max_output_kw: site.interconnection_max_output_kw,
        rebound_protection_soc_floor_percent: site.rebound_protection_soc_floor_percent,
        site_variant: site.site_variant,
        created_at,
        updated_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;

    #[test]
    fn test_get_sites_by_company() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        // Use insert_site function instead of manual insertion
        insert_site(
            &mut conn,
            "Site 1".to_string(),
            "123 Main St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .expect("Failed to insert site 1");

        insert_site(
            &mut conn,
            "Site 2".to_string(),
            "456 Oak Ave".to_string(),
            40.7589,
            -73.9851,
            company.id,
            120,
            None,
        )
        .expect("Failed to insert site 2");

        let sites = get_sites_by_company(&mut conn, company.id).unwrap();
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].name, "Site 1");
        assert_eq!(sites[1].name, "Site 2");
        assert!(sites[0].id < sites[1].id);
    }

    #[test]
    fn test_insert_site() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .expect("Failed to insert site");

        assert_eq!(site.name, "Test Site");
        assert_eq!(site.address, "123 Test St");
        assert_eq!(site.latitude, 40.7128);
        assert_eq!(site.longitude, -74.0060);
        assert_eq!(site.company_id, company.id);
        assert_eq!(site.ramp_duration_seconds, 120);
        assert!(site.id > 0);
    }

    #[test]
    fn test_get_site_by_id() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let created_site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .expect("Failed to insert site");

        // Test getting existing site
        let retrieved_site = get_site_by_id(&mut conn, created_site.id)
            .expect("Failed to get site")
            .expect("Site should exist");

        assert_eq!(retrieved_site.id, created_site.id);
        assert_eq!(retrieved_site.name, "Test Site");
        assert_eq!(retrieved_site.address, "123 Test St");

        // Test getting non-existent site
        let non_existent = get_site_by_id(&mut conn, 99999).expect("Query should succeed");
        assert!(non_existent.is_none());
    }

    #[test]
    fn test_get_all_sites() {
        let mut conn = setup_test_db();

        let company1 = crate::company::insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to insert company 1");
        let company2 = crate::company::insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to insert company 2");

        // Insert sites for different companies
        insert_site(
            &mut conn,
            "Site 1".to_string(),
            "Address 1".to_string(),
            40.0,
            -74.0,
            company1.id,
            120,
            None,
        )
        .expect("Failed to insert site 1");
        insert_site(
            &mut conn,
            "Site 2".to_string(),
            "Address 2".to_string(),
            41.0,
            -75.0,
            company2.id,
            120,
            None,
        )
        .expect("Failed to insert site 2");
        insert_site(
            &mut conn,
            "Site 3".to_string(),
            "Address 3".to_string(),
            42.0,
            -76.0,
            company1.id,
            120,
            None,
        )
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

        let company1 = crate::company::insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to insert company 1");
        let company2 = crate::company::insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to insert company 2");

        let created_site = insert_site(
            &mut conn,
            "Original Site".to_string(),
            "Original Address".to_string(),
            40.0,
            -74.0,
            company1.id,
            120,
            None,
        )
        .expect("Failed to insert site");

        // Get timestamps from activity log
        let original_created_at =
            crate::orm::entity_activity::get_created_at(&mut conn, "sites", created_site.id)
                .expect("Should have created timestamp");
        let original_updated_at =
            crate::orm::entity_activity::get_updated_at(&mut conn, "sites", created_site.id)
                .expect("Should have updated timestamp");

        // Wait a moment to ensure updated_at changes (SQLite timestamps have 1-second
        // resolution)
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Test partial update (only name)
        let updated_site = update_site(
            &mut conn,
            created_site.id,
            SiteUpdate {
                name: Some("Updated Site".to_string()),
                ..Default::default()
            },
            None,
        )
        .expect("Failed to update site");

        assert_eq!(updated_site.name, "Updated Site");
        assert_eq!(updated_site.address, "Original Address"); // Should remain unchanged
        assert_eq!(updated_site.latitude, 40.0);
        assert_eq!(updated_site.longitude, -74.0);
        assert_eq!(updated_site.company_id, company1.id);
        assert_eq!(updated_site.ramp_duration_seconds, 120); // Should remain unchanged

        // Check timestamps from activity log
        let new_created_at =
            crate::orm::entity_activity::get_created_at(&mut conn, "sites", created_site.id)
                .expect("Should have created timestamp");
        let new_updated_at =
            crate::orm::entity_activity::get_updated_at(&mut conn, "sites", created_site.id)
                .expect("Should have updated timestamp");

        assert_eq!(new_created_at, original_created_at); // Should not change
        assert!(new_updated_at > original_updated_at); // Should be updated

        // Test full update
        let fully_updated_site = update_site(
            &mut conn,
            created_site.id,
            SiteUpdate {
                name: Some("Fully Updated Site".to_string()),
                address: Some("New Address".to_string()),
                latitude: Some(41.0),
                longitude: Some(-75.0),
                company_id: Some(company2.id),
                ramp_duration_seconds: Some(180),
                ..Default::default()
            },
            None,
        )
        .expect("Failed to fully update site");

        assert_eq!(fully_updated_site.name, "Fully Updated Site");
        assert_eq!(fully_updated_site.address, "New Address");
        assert_eq!(fully_updated_site.latitude, 41.0);
        assert_eq!(fully_updated_site.longitude, -75.0);
        assert_eq!(fully_updated_site.company_id, company2.id);
        assert_eq!(fully_updated_site.ramp_duration_seconds, 180);

        // Test demo-defaults update
        let defaults_updated = update_site(
            &mut conn,
            created_site.id,
            SiteUpdate {
                power_kw: Some(5000.0),
                capacity_kwh: Some(23500.0),
                closed_loop_enabled: Some(false),
                off_peak_start_minutes: Some(0),
                off_peak_end_minutes: Some(8 * 60),
                peak_revenue_start_minutes: Some(16 * 60),
                peak_revenue_end_minutes: Some(20 * 60),
                interconnection_max_output_kw: Some(5000.0),
                rebound_protection_soc_floor_percent: Some(2.5),
                site_variant: Some("no_grid_charge".to_string()),
                ..Default::default()
            },
            None,
        )
        .expect("Failed to update demo defaults");

        assert_eq!(defaults_updated.power_kw, Some(5000.0));
        assert_eq!(defaults_updated.capacity_kwh, Some(23500.0));
        assert!(!defaults_updated.closed_loop_enabled);
        assert_eq!(defaults_updated.off_peak_start_minutes, Some(0));
        assert_eq!(defaults_updated.off_peak_end_minutes, Some(480));
        assert_eq!(defaults_updated.peak_revenue_start_minutes, Some(960));
        assert_eq!(defaults_updated.peak_revenue_end_minutes, Some(1200));
        assert_eq!(defaults_updated.interconnection_max_output_kw, Some(5000.0));
        assert!((defaults_updated.rebound_protection_soc_floor_percent - 2.5).abs() < 1e-6);
        assert_eq!(defaults_updated.site_variant, "no_grid_charge");

        // Demo defaults are sticky: a subsequent unrelated update keeps them.
        let after_name_change = update_site(
            &mut conn,
            created_site.id,
            SiteUpdate {
                name: Some("Still Updated".to_string()),
                ..Default::default()
            },
            None,
        )
        .expect("Failed to update name");
        assert_eq!(after_name_change.power_kw, Some(5000.0));
        assert_eq!(after_name_change.site_variant, "no_grid_charge");
    }

    #[test]
    fn test_update_nonexistent_site() {
        let mut conn = setup_test_db();

        let result = update_site(
            &mut conn,
            99999,
            SiteUpdate {
                name: Some("Test".to_string()),
                ..Default::default()
            },
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_delete_site() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site1 = insert_site(
            &mut conn,
            "Site 1".to_string(),
            "Address 1".to_string(),
            40.0,
            -74.0,
            company.id,
            120,
            None,
        )
        .expect("Failed to insert site 1");
        let site2 = insert_site(
            &mut conn,
            "Site 2".to_string(),
            "Address 2".to_string(),
            41.0,
            -75.0,
            company.id,
            120,
            None,
        )
        .expect("Failed to insert site 2");

        // Verify both sites exist
        let all_sites_before = get_all_sites(&mut conn).expect("Failed to get sites");
        assert_eq!(all_sites_before.len(), 2);

        // Delete one site
        let deleted_count = delete_site(&mut conn, site1.id, None).expect("Failed to delete site");
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

        let deleted_count = delete_site(&mut conn, 99999, None).expect("Delete should succeed");
        assert_eq!(deleted_count, 0); // No rows affected
    }

    #[test]
    fn test_site_with_timestamps() {
        let mut conn = setup_test_db();

        let company =
            crate::company::insert_company(&mut conn, "Timestamp Test Company".to_string(), None)
                .expect("Failed to insert company");

        // Insert site
        let site = insert_site(
            &mut conn,
            "Timestamp Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .unwrap();

        // Get site with timestamps
        let site_with_timestamps = get_site_with_timestamps(&mut conn, site.id)
            .expect("Should get timestamps")
            .expect("Site should exist");

        assert_eq!(site_with_timestamps.id, site.id);
        assert_eq!(site_with_timestamps.name, "Timestamp Test Site");
        assert_eq!(site_with_timestamps.address, "123 Test St");
        assert_eq!(site_with_timestamps.latitude, 40.7128);
        assert_eq!(site_with_timestamps.longitude, -74.0060);
        assert_eq!(site_with_timestamps.company_id, company.id);
        assert_eq!(site_with_timestamps.ramp_duration_seconds, 120);

        // Timestamps should be recent (within last few seconds)
        let now = chrono::Utc::now().naive_utc();
        let created_diff = (site_with_timestamps.created_at - now).num_seconds().abs();
        let updated_diff = (site_with_timestamps.updated_at - now).num_seconds().abs();

        assert!(created_diff <= 5, "Created timestamp should be recent");
        assert!(updated_diff <= 5, "Updated timestamp should be recent");
    }

    #[test]
    fn test_get_site_by_company_and_name_case_insensitive() {
        let mut conn = setup_test_db();

        let company = crate::company::insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        // Insert a site with mixed case name
        let created_site = insert_site(
            &mut conn,
            "Test Site Name".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .expect("Failed to insert site");

        // Test case-insensitive lookup with different cases
        let test_cases =
            vec!["test site name", "TEST SITE NAME", "Test Site Name", "tEsT sItE nAmE"];

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
        let other_company =
            crate::company::insert_company(&mut conn, "Other Company".to_string(), None)
                .expect("Failed to insert other company");
        let result = get_site_by_company_and_name(&mut conn, other_company.id, "Test Site Name")
            .expect("Query should succeed");
        assert!(result.is_none());
    }
}
