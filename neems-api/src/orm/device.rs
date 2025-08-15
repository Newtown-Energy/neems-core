use diesel::prelude::*;

use crate::models::{Device, DeviceInput, DeviceWithTimestamps, NewDevice};

/// Gets all devices for a specific site.
pub fn get_devices_by_site(
    conn: &mut SqliteConnection,
    device_site_id: i32,
) -> Result<Vec<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices
        .filter(crate::schema::devices::site_id.eq(device_site_id))
        .order(id.asc())
        .select(Device::as_select())
        .load(conn)
}

/// Gets all devices for a specific company.
pub fn get_devices_by_company(
    conn: &mut SqliteConnection,
    comp_id: i32,
) -> Result<Vec<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices
        .filter(company_id.eq(comp_id))
        .order(id.asc())
        .select(Device::as_select())
        .load(conn)
}

/// Creates a new device in the database (timestamps handled automatically by database triggers)
/// If name is not provided, it defaults to the device type
pub fn insert_device(
    conn: &mut SqliteConnection,
    device_input: DeviceInput,
    acting_user_id: Option<i32>,
) -> Result<Device, diesel::result::Error> {
    use crate::schema::devices::dsl::*;

    // Use provided name or default to type
    let device_name = device_input.name.unwrap_or_else(|| device_input.type_.clone());

    let new_device = NewDevice {
        name: device_name,
        description: device_input.description,
        type_: device_input.type_,
        model: device_input.model,
        serial: device_input.serial,
        ip_address: device_input.ip_address,
        install_date: device_input.install_date,
        company_id: device_input.company_id,
        site_id: device_input.site_id,
    };

    diesel::insert_into(devices).values(&new_device).execute(conn)?;

    // Return the inserted device
    let device = devices.order(id.desc()).select(Device::as_select()).first(conn)?;
    
    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "devices", device.id, "create", user_id);
    }
    
    Ok(device)
}

/// Gets a device by its ID.
pub fn get_device_by_id(
    conn: &mut SqliteConnection,
    device_id: i32,
) -> Result<Option<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices.filter(id.eq(device_id)).select(Device::as_select()).first(conn).optional()
}

/// Gets a device by site ID and name (case-insensitive).
pub fn get_device_by_site_and_name(
    conn: &mut SqliteConnection,
    device_site_id: i32,
    device_name: &str,
) -> Result<Option<Device>, diesel::result::Error> {
    // Use raw SQL for case-insensitive comparison
    diesel::sql_query("SELECT id, name, description, type as type_, model, serial, ip_address, install_date, company_id, site_id FROM devices WHERE site_id = ? AND LOWER(name) = LOWER(?)")
        .bind::<diesel::sql_types::Integer, _>(device_site_id)
        .bind::<diesel::sql_types::Text, _>(device_name)
        .get_result::<Device>(conn)
        .optional()
}

/// Gets all devices in the system.
pub fn get_all_devices(conn: &mut SqliteConnection) -> Result<Vec<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices.order(id.asc()).select(Device::as_select()).load(conn)
}

/// Updates a device in the database (timestamps handled automatically by database triggers)
pub fn update_device(
    conn: &mut SqliteConnection,
    device_id: i32,
    new_name: Option<String>,
    new_description: Option<Option<String>>, // Double Option to distinguish between "don't change" and "set to null"
    new_type: Option<String>,
    new_model: Option<String>,
    new_serial: Option<Option<String>>,
    new_ip_address: Option<Option<String>>,
    new_install_date: Option<Option<chrono::NaiveDateTime>>,
    new_company_id: Option<i32>,
    new_site_id: Option<i32>,
    acting_user_id: Option<i32>,
) -> Result<Device, diesel::result::Error> {
    use crate::schema::devices::dsl::*;

    // First, get the current device to preserve existing values
    let current_device = devices.filter(id.eq(device_id)).select(Device::as_select()).first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(devices.filter(id.eq(device_id)))
        .set((
            name.eq(new_name.unwrap_or(current_device.name)),
            description.eq(new_description.unwrap_or(current_device.description)),
            type_.eq(new_type.unwrap_or(current_device.type_)),
            model.eq(new_model.unwrap_or(current_device.model)),
            serial.eq(new_serial.unwrap_or(current_device.serial)),
            ip_address.eq(new_ip_address.unwrap_or(current_device.ip_address)),
            install_date.eq(new_install_date.unwrap_or(current_device.install_date)),
            company_id.eq(new_company_id.unwrap_or(current_device.company_id)),
            site_id.eq(new_site_id.unwrap_or(current_device.site_id)),
        ))
        .execute(conn)?;

    // Return the updated device
    let device = devices.filter(id.eq(device_id)).select(Device::as_select()).first(conn)?;
    
    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "devices", device_id, "update", user_id);
    }
    
    Ok(device)
}

/// Deletes a device from the database.
pub fn delete_device(
    conn: &mut SqliteConnection,
    device_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    let result = diesel::delete(devices.filter(id.eq(device_id))).execute(conn)?;
    
    // Update the trigger-created activity entry with user information
    if result > 0 {
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ = update_latest_activity_user(conn, "devices", device_id, "delete", user_id);
        }
    }
    
    Ok(result)
}

/// Get a device with computed timestamps from activity log
pub fn get_device_with_timestamps(
    conn: &mut SqliteConnection,
    device_id: i32,
) -> Result<Option<DeviceWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity;
    
    // First get the device
    let device = match get_device_by_id(conn, device_id)? {
        Some(d) => d,
        None => return Ok(None),
    };

    // Get timestamps from activity log
    let created_at = entity_activity::get_created_at(conn, "devices", device_id)?;
    let updated_at = entity_activity::get_updated_at(conn, "devices", device_id)?;

    Ok(Some(DeviceWithTimestamps {
        id: device.id,
        name: device.name,
        description: device.description,
        type_: device.type_,
        model: device.model,
        serial: device.serial,
        ip_address: device.ip_address,
        install_date: device.install_date,
        company_id: device.company_id,
        site_id: device.site_id,
        created_at,
        updated_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;
    use crate::orm::company::insert_company;
    use crate::orm::site::insert_site;

    #[test]
    fn test_insert_device_with_all_fields() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let install_date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(10, 30, 0)
            .unwrap();

        let device_input = DeviceInput {
            name: Some("Solar Inverter 1".to_string()),
            description: Some("Main solar inverter for building A".to_string()),
            type_: "Inverter".to_string(),
            model: "SUN2000-100KTL".to_string(),
            serial: Some("INV123456789".to_string()),
            ip_address: Some("192.168.1.100".to_string()),
            install_date: Some(install_date),
            company_id: company.id,
            site_id: site.id,
        };

        let device = insert_device(&mut conn, device_input, None)
            .expect("Failed to insert device");

        assert_eq!(device.name, "Solar Inverter 1");
        assert_eq!(device.description, Some("Main solar inverter for building A".to_string()));
        assert_eq!(device.type_, "Inverter");
        assert_eq!(device.model, "SUN2000-100KTL");
        assert_eq!(device.serial, Some("INV123456789".to_string()));
        assert_eq!(device.ip_address, Some("192.168.1.100".to_string()));
        assert_eq!(device.install_date, Some(install_date));
        assert_eq!(device.company_id, company.id);
        assert_eq!(device.site_id, site.id);
        assert!(device.id > 0);
    }

    #[test]
    fn test_insert_device_name_defaults_to_type() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input = DeviceInput {
            name: None, // No name provided
            description: None,
            type_: "Battery".to_string(),
            model: "PowerWall 2".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let device = insert_device(&mut conn, device_input, None)
            .expect("Failed to insert device");

        assert_eq!(device.name, "Battery"); // Name should default to type
        assert_eq!(device.type_, "Battery");
        assert_eq!(device.model, "PowerWall 2");
    }

    #[test]
    fn test_unique_constraint_company_site_name() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input1 = DeviceInput {
            name: Some("Device A".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model X".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        insert_device(&mut conn, device_input1, None)
            .expect("Failed to insert first device");

        // Try to insert device with same name at same site
        let device_input2 = DeviceInput {
            name: Some("Device A".to_string()), // Same name
            description: None,
            type_: "Meter".to_string(),
            model: "Model Y".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id, // Same site
        };

        let result = insert_device(&mut conn, device_input2, None);
        assert!(result.is_err()); // Should fail due to unique constraint

        // Create another site
        let site2 = insert_site(
            &mut conn,
            "Test Site 2".to_string(),
            "456 Test Ave".to_string(),
            40.7589,
            -73.9851,
            company.id,
            None,
        )
        .expect("Failed to insert second site");

        // Same device name at different site should work
        let device_input3 = DeviceInput {
            name: Some("Device A".to_string()), // Same name
            description: None,
            type_: "Controller".to_string(),
            model: "Model Z".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site2.id, // Different site
        };

        let device3 = insert_device(&mut conn, device_input3, None)
            .expect("Failed to insert device at different site");

        assert_eq!(device3.name, "Device A");
        assert_eq!(device3.site_id, site2.id);
    }

    #[test]
    fn test_get_device_by_id() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input = DeviceInput {
            name: Some("Test Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let created_device = insert_device(&mut conn, device_input, None)
            .expect("Failed to insert device");

        // Test getting existing device
        let retrieved_device = get_device_by_id(&mut conn, created_device.id)
            .expect("Failed to get device")
            .expect("Device should exist");

        assert_eq!(retrieved_device.id, created_device.id);
        assert_eq!(retrieved_device.name, "Test Device");

        // Test getting non-existent device
        let non_existent = get_device_by_id(&mut conn, 99999).expect("Query should succeed");
        assert!(non_existent.is_none());
    }

    #[test]
    fn test_get_device_by_site_and_name_case_insensitive() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input = DeviceInput {
            name: Some("Test Device Name".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let created_device = insert_device(&mut conn, device_input, None)
            .expect("Failed to insert device");

        // Test case-insensitive lookup with different cases
        let test_cases = vec![
            "test device name",
            "TEST DEVICE NAME",
            "Test Device Name",
            "tEsT dEvIcE nAmE",
        ];

        for test_name in test_cases {
            let retrieved_device = get_device_by_site_and_name(&mut conn, site.id, test_name)
                .expect("Query should succeed")
                .expect("Device should be found");
            assert_eq!(retrieved_device.id, created_device.id);
            assert_eq!(retrieved_device.name, "Test Device Name"); // Original case preserved
        }

        // Test non-existent device name
        let result = get_device_by_site_and_name(&mut conn, site.id, "Non-existent Device")
            .expect("Query should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_devices_by_site() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site1 = insert_site(
            &mut conn,
            "Site 1".to_string(),
            "123 Main St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site 1");

        let site2 = insert_site(
            &mut conn,
            "Site 2".to_string(),
            "456 Oak Ave".to_string(),
            40.7589,
            -73.9851,
            company.id,
            None,
        )
        .expect("Failed to insert site 2");

        // Insert devices for site1
        let device_input1 = DeviceInput {
            name: Some("Device 1".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site1.id,
        };

        let device_input2 = DeviceInput {
            name: Some("Device 2".to_string()),
            description: None,
            type_: "Meter".to_string(),
            model: "Model B".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site1.id,
        };

        // Insert device for site2
        let device_input3 = DeviceInput {
            name: Some("Device 3".to_string()),
            description: None,
            type_: "Controller".to_string(),
            model: "Model C".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site2.id,
        };

        insert_device(&mut conn, device_input1, None).expect("Failed to insert device 1");
        insert_device(&mut conn, device_input2, None).expect("Failed to insert device 2");
        insert_device(&mut conn, device_input3, None).expect("Failed to insert device 3");

        // Get devices for site1
        let site1_devices = get_devices_by_site(&mut conn, site1.id).unwrap();
        assert_eq!(site1_devices.len(), 2);
        assert_eq!(site1_devices[0].name, "Device 1");
        assert_eq!(site1_devices[1].name, "Device 2");

        // Get devices for site2
        let site2_devices = get_devices_by_site(&mut conn, site2.id).unwrap();
        assert_eq!(site2_devices.len(), 1);
        assert_eq!(site2_devices[0].name, "Device 3");
    }

    #[test]
    fn test_get_devices_by_company() {
        let mut conn = setup_test_db();

        let company1 = insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to insert company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to insert company 2");

        let site1 = insert_site(
            &mut conn,
            "Site 1".to_string(),
            "Address 1".to_string(),
            40.0,
            -74.0,
            company1.id,
            None,
        )
        .expect("Failed to insert site 1");

        let site2 = insert_site(
            &mut conn,
            "Site 2".to_string(),
            "Address 2".to_string(),
            41.0,
            -75.0,
            company2.id,
            None,
        )
        .expect("Failed to insert site 2");

        // Insert devices for company1
        let device_input1 = DeviceInput {
            name: Some("Company1 Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company1.id,
            site_id: site1.id,
        };

        // Insert device for company2
        let device_input2 = DeviceInput {
            name: Some("Company2 Device".to_string()),
            description: None,
            type_: "Meter".to_string(),
            model: "Model B".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company2.id,
            site_id: site2.id,
        };

        insert_device(&mut conn, device_input1, None).expect("Failed to insert device 1");
        insert_device(&mut conn, device_input2, None).expect("Failed to insert device 2");

        // Get devices for company1
        let company1_devices = get_devices_by_company(&mut conn, company1.id).unwrap();
        assert_eq!(company1_devices.len(), 1);
        assert_eq!(company1_devices[0].name, "Company1 Device");

        // Get devices for company2
        let company2_devices = get_devices_by_company(&mut conn, company2.id).unwrap();
        assert_eq!(company2_devices.len(), 1);
        assert_eq!(company2_devices[0].name, "Company2 Device");
    }

    #[test]
    fn test_update_device() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input = DeviceInput {
            name: Some("Original Name".to_string()),
            description: Some("Original Description".to_string()),
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: Some("SN123".to_string()),
            ip_address: Some("192.168.1.1".to_string()),
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let created_device = insert_device(&mut conn, device_input, None)
            .expect("Failed to insert device");

        // Get timestamps from activity log
        let original_created_at = crate::orm::entity_activity::get_created_at(&mut conn, "devices", created_device.id)
            .expect("Should have created timestamp");
        let original_updated_at = crate::orm::entity_activity::get_updated_at(&mut conn, "devices", created_device.id)
            .expect("Should have updated timestamp");

        // Wait a moment to ensure updated_at changes
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Test partial update (only name and description)
        let updated_device = update_device(
            &mut conn,
            created_device.id,
            Some("Updated Name".to_string()),
            Some(None), // Set description to null
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to update device");

        assert_eq!(updated_device.name, "Updated Name");
        assert_eq!(updated_device.description, None); // Should be null
        assert_eq!(updated_device.type_, "Sensor"); // Should remain unchanged
        assert_eq!(updated_device.model, "Model A");
        assert_eq!(updated_device.serial, Some("SN123".to_string()));

        // Check timestamps from activity log
        let new_created_at = crate::orm::entity_activity::get_created_at(&mut conn, "devices", created_device.id)
            .expect("Should have created timestamp");
        let new_updated_at = crate::orm::entity_activity::get_updated_at(&mut conn, "devices", created_device.id)
            .expect("Should have updated timestamp");
            
        assert_eq!(new_created_at, original_created_at); // Should not change
        assert!(new_updated_at > original_updated_at); // Should be updated
    }

    #[test]
    fn test_delete_device() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input1 = DeviceInput {
            name: Some("Device 1".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let device_input2 = DeviceInput {
            name: Some("Device 2".to_string()),
            description: None,
            type_: "Meter".to_string(),
            model: "Model B".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let device1 = insert_device(&mut conn, device_input1, None)
            .expect("Failed to insert device 1");
        let device2 = insert_device(&mut conn, device_input2, None)
            .expect("Failed to insert device 2");

        // Verify both devices exist
        let all_devices_before = get_all_devices(&mut conn).expect("Failed to get devices");
        assert!(all_devices_before.iter().any(|d| d.id == device1.id));
        assert!(all_devices_before.iter().any(|d| d.id == device2.id));

        // Delete one device
        let deleted_count = delete_device(&mut conn, device1.id, None).expect("Failed to delete device");
        assert_eq!(deleted_count, 1);

        // Verify only one device remains
        let all_devices_after = get_all_devices(&mut conn).expect("Failed to get devices");
        assert!(!all_devices_after.iter().any(|d| d.id == device1.id));
        assert!(all_devices_after.iter().any(|d| d.id == device2.id));

        // Verify the deleted device is gone
        let deleted_device = get_device_by_id(&mut conn, device1.id).expect("Query should succeed");
        assert!(deleted_device.is_none());
    }

    #[test]
    fn test_device_with_timestamps() {
        let mut conn = setup_test_db();
        
        let company = insert_company(&mut conn, "Timestamp Test Company".to_string(), None)
            .expect("Failed to insert company");
        
        let site = insert_site(
            &mut conn,
            "Timestamp Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input = DeviceInput {
            name: Some("Timestamp Test Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let device = insert_device(&mut conn, device_input, None).unwrap();
        
        // Get device with timestamps
        let device_with_timestamps = get_device_with_timestamps(&mut conn, device.id)
            .expect("Should get timestamps")
            .expect("Device should exist");
            
        assert_eq!(device_with_timestamps.id, device.id);
        assert_eq!(device_with_timestamps.name, "Timestamp Test Device");
        assert_eq!(device_with_timestamps.type_, "Sensor");
        assert_eq!(device_with_timestamps.model, "Model A");
        assert_eq!(device_with_timestamps.company_id, company.id);
        assert_eq!(device_with_timestamps.site_id, site.id);
        
        // Timestamps should be recent (within last few seconds)
        let now = chrono::Utc::now().naive_utc();
        let created_diff = (device_with_timestamps.created_at - now).num_seconds().abs();
        let updated_diff = (device_with_timestamps.updated_at - now).num_seconds().abs();
        
        assert!(created_diff <= 5, "Created timestamp should be recent");
        assert!(updated_diff <= 5, "Updated timestamp should be recent");
    }

    #[test]
    fn test_cascade_delete_when_site_deleted() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input = DeviceInput {
            name: Some("Test Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let device = insert_device(&mut conn, device_input, None)
            .expect("Failed to insert device");

        // Verify device exists
        assert!(get_device_by_id(&mut conn, device.id).unwrap().is_some());

        // Delete the site
        crate::orm::site::delete_site(&mut conn, site.id, None)
            .expect("Failed to delete site");

        // Device should be gone due to cascade delete
        assert!(get_device_by_id(&mut conn, device.id).unwrap().is_none());
    }

    #[test]
    fn test_cascade_delete_when_company_deleted() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to insert company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            None,
        )
        .expect("Failed to insert site");

        let device_input = DeviceInput {
            name: Some("Test Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let device = insert_device(&mut conn, device_input, None)
            .expect("Failed to insert device");

        // Verify device exists
        assert!(get_device_by_id(&mut conn, device.id).unwrap().is_some());

        // Delete the company
        crate::orm::company::delete_company(&mut conn, company.id, None)
            .expect("Failed to delete company");

        // Device should be gone due to cascade delete (company -> site -> device)
        assert!(get_device_by_id(&mut conn, device.id).unwrap().is_none());
    }
}