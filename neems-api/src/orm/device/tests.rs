use super::*;
use crate::orm::{company::insert_company, site::insert_site, testing::setup_test_db};

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
        120,
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

    let device = insert_device(&mut conn, device_input, None).expect("Failed to insert device");

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
        120,
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

    let device = insert_device(&mut conn, device_input, None).expect("Failed to insert device");

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
        120,
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

    insert_device(&mut conn, device_input1, None).expect("Failed to insert first device");

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
        120,
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
        120,
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

    let created_device =
        insert_device(&mut conn, device_input, None).expect("Failed to insert device");

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
        120,
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

    let created_device =
        insert_device(&mut conn, device_input, None).expect("Failed to insert device");

    // Test case-insensitive lookup with different cases
    let test_cases =
        vec!["test device name", "TEST DEVICE NAME", "Test Device Name", "tEsT dEvIcE nAmE"];

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
        120,
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
        120,
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
        company2.id,
        120,
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
        120,
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

    let created_device =
        insert_device(&mut conn, device_input, None).expect("Failed to insert device");

    // Get timestamps from activity log
    let original_created_at =
        crate::orm::entity_activity::get_created_at(&mut conn, "devices", created_device.id)
            .expect("Should have created timestamp");
    let original_updated_at =
        crate::orm::entity_activity::get_updated_at(&mut conn, "devices", created_device.id)
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
    let new_created_at =
        crate::orm::entity_activity::get_created_at(&mut conn, "devices", created_device.id)
            .expect("Should have created timestamp");
    let new_updated_at =
        crate::orm::entity_activity::get_updated_at(&mut conn, "devices", created_device.id)
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
        120,
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

    let device1 = insert_device(&mut conn, device_input1, None).expect("Failed to insert device 1");
    let device2 = insert_device(&mut conn, device_input2, None).expect("Failed to insert device 2");

    // Verify both devices exist
    let all_devices_before = get_all_devices(&mut conn).expect("Failed to get devices");
    assert!(all_devices_before.iter().any(|d| d.id == device1.id));
    assert!(all_devices_before.iter().any(|d| d.id == device2.id));

    // Delete one device
    let deleted_count =
        delete_device(&mut conn, device1.id, None).expect("Failed to delete device");
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
        120,
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
        120,
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

    let device = insert_device(&mut conn, device_input, None).expect("Failed to insert device");

    // Verify device exists
    assert!(get_device_by_id(&mut conn, device.id).unwrap().is_some());

    // Delete the site
    crate::orm::site::delete_site(&mut conn, site.id, None).expect("Failed to delete site");

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
        120,
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

    let device = insert_device(&mut conn, device_input, None).expect("Failed to insert device");

    // Verify device exists
    assert!(get_device_by_id(&mut conn, device.id).unwrap().is_some());

    // Delete the company
    crate::orm::company::delete_company(&mut conn, company.id, None)
        .expect("Failed to delete company");

    // Device should be gone due to cascade delete (company -> site -> device)
    assert!(get_device_by_id(&mut conn, device.id).unwrap().is_none());
}
