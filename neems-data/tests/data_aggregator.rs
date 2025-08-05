//! tests/data_aggregator.rs

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use neems_data::collectors::DataCollector;
use neems_data::models::{NewReading, NewSource, UpdateSource};
use neems_data::{
    MIGRATIONS, create_source, get_recent_readings, get_source_by_name, insert_reading,
    list_sources, update_source,
};

/// Helper function to set up an in-memory SQLite database for testing.
/// It establishes a connection and runs the embedded migrations.
fn setup_test_db() -> SqliteConnection {
    let mut connection =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory db");
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
    connection
}

#[test]
fn test_create_and_list_sources() {
    let mut conn = setup_test_db();

    let new_source = NewSource {
        name: "test_source".to_string(),
        description: Some("A test source".to_string()),
        active: Some(true),
        interval_seconds: Some(1),
        test_type: Some("ping".to_string()),
        arguments: Some("{}".to_string()),
    };

    // Create a source
    let created_source = create_source(&mut conn, new_source).expect("Failed to create source");
    assert_eq!(created_source.name, "test_source");
    assert_eq!(created_source.description.clone().unwrap(), "A test source");
    assert!(created_source.active);

    // List sources
    let sources = list_sources(&mut conn).expect("Failed to list sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "test_source");
    assert_eq!(sources[0].id, created_source.id);
}

#[test]
fn test_get_source_by_name() {
    let mut conn = setup_test_db();

    let source_name = "find_me";
    let new_source = NewSource {
        name: source_name.to_string(),
        description: None,
        active: None,
        interval_seconds: Some(1),
        test_type: Some("ping".to_string()),
        arguments: Some("{}".to_string()),
    };
    create_source(&mut conn, new_source).unwrap();

    // Find the source
    let found_source = get_source_by_name(&mut conn, source_name)
        .expect("Failed to get source by name")
        .unwrap();
    assert_eq!(found_source.name, source_name);

    // Look for a source that doesn't exist
    let not_found = get_source_by_name(&mut conn, "not_real").expect("Query should not fail");
    assert!(not_found.is_none());
}

#[test]
fn test_update_source() {
    let mut conn = setup_test_db();

    // Create an initial source
    let initial_source = NewSource {
        name: "initial_name".to_string(),
        description: Some("Initial description".to_string()),
        active: Some(true),
        interval_seconds: Some(1),
        test_type: Some("ping".to_string()),
        arguments: Some("{}".to_string()),
    };
    let source = create_source(&mut conn, initial_source).unwrap();
    let source_id = source.id.unwrap();

    // Update the source
    let updated_name = "updated_name".to_string();
    let updated_description = "Updated description".to_string();
    let source_updates = UpdateSource {
        name: Some(updated_name.clone()),
        description: Some(Some(updated_description.clone())),
        active: Some(false),
        interval_seconds: Some(5),
        last_run: None,
        test_type: None,
        arguments: None,
    };

    let updated_source =
        update_source(&mut conn, source_id, source_updates).expect("Failed to update source");

    assert_eq!(updated_source.id, Some(source_id));
    assert_eq!(updated_source.name, updated_name);
    assert_eq!(updated_source.description, Some(updated_description));
    assert!(!updated_source.active);
}

#[test]
fn test_insert_and_get_reading() {
    let mut conn = setup_test_db();

    // 1. Create a source first
    let new_source = NewSource {
        name: "test_source_for_readings".to_string(),
        description: None,
        active: Some(true),
        interval_seconds: Some(1),
        test_type: Some("ping".to_string()),
        arguments: Some("{}".to_string()),
    };
    let source = create_source(&mut conn, new_source).expect("Failed to create source");
    let source_id = source.id.unwrap();

    // 2. Insert a reading
    let data = serde_json::json!({ "value": 123, "status": "ok" });
    let new_reading = NewReading::with_json_data(source_id, &data).unwrap();
    insert_reading(&mut conn, new_reading).expect("Failed to insert reading");

    // 3. Get recent readings
    let readings =
        get_recent_readings(&mut conn, source_id, 5).expect("Failed to get recent readings");
    assert_eq!(readings.len(), 1);

    let reading = &readings[0];
    assert_eq!(reading.source_id, source_id);
    assert_eq!(reading.quality_flags, 0); // Default value

    // 4. Verify the data
    let parsed_data: serde_json::Value = serde_json::from_str(&reading.data).unwrap();
    assert_eq!(parsed_data, data);
}

#[tokio::test]
async fn test_charging_state_source_integration() {
    let mut conn = setup_test_db();

    // 1. Create the "charging_state" source
    let source_name = "charging_state";
    let new_source = NewSource {
        name: source_name.to_string(),
        description: Some("Test charging state".to_string()),
        active: Some(true),
        interval_seconds: Some(1),
        test_type: Some("charging_state".to_string()),
        arguments: Some("{}".to_string()),
    };
    let source = create_source(&mut conn, new_source).expect("Failed to create source");
    let source_id = source.id.unwrap();

    // 2. Use the DataCollector to get data for this source
    let collector = DataCollector::new(source_name.to_string(), source_id);
    let collected_data = collector.collect().await.expect("Collector failed");

    // 3. Insert the collected data as a new reading
    let new_reading = NewReading::with_json_data(source_id, &collected_data).unwrap();
    insert_reading(&mut conn, new_reading).expect("Failed to insert reading");

    // 4. Retrieve the reading and verify its contents
    let readings =
        get_recent_readings(&mut conn, source_id, 1).expect("Failed to get recent readings");
    assert_eq!(readings.len(), 1);

    let reading = &readings[0];
    let parsed_data: serde_json::Value = serde_json::from_str(&reading.data).unwrap();

    // Check that the data has the expected structure
    assert!(parsed_data.get("state").is_some());
    assert!(parsed_data.get("timestamp_utc").is_some());

    // Check that the state is one of the valid options
    let state = parsed_data["state"].as_str().unwrap();
    assert!(["charging", "discharging", "hold"].contains(&state));
}
