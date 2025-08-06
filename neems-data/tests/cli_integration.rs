//! Integration tests for CLI functionality

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use neems_data::{MIGRATIONS, create_source, list_sources};
use neems_data::models::{NewSource};
use std::collections::HashMap;

/// Helper function to set up an in-memory SQLite database for testing
fn setup_test_db() -> SqliteConnection {
    let mut connection =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory db");
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
    connection
}

#[test]
fn test_new_source_creation_with_test_type() {
    let mut conn = setup_test_db();

    // Test creating a ping source with arguments
    let mut args = HashMap::new();
    args.insert("target".to_string(), "example.com".to_string());
    
    let new_source = NewSource {
        name: "test_ping".to_string(),
        description: Some("Test ping source".to_string()),
        active: Some(true),
        interval_seconds: Some(5),
        test_type: Some("ping".to_string()),
        arguments: Some(serde_json::to_string(&args).unwrap()),
        site_id: None,
        company_id: None,
    };

    let created = create_source(&mut conn, new_source).expect("Failed to create source");
    assert_eq!(created.name, "test_ping");
    assert_eq!(created.test_type, Some("ping".to_string()));
    
    // Verify arguments were stored correctly
    let stored_args: HashMap<String, String> = serde_json::from_str(
        &created.arguments.unwrap()
    ).unwrap();
    assert_eq!(stored_args.get("target"), Some(&"example.com".to_string()));
}

#[test]
fn test_charging_state_source_with_battery_id() {
    let mut conn = setup_test_db();

    let mut args = HashMap::new();
    args.insert("battery_id".to_string(), "battery1".to_string());
    
    let new_source = NewSource {
        name: "test_battery".to_string(),
        description: Some("Test battery source".to_string()),
        active: Some(true),
        interval_seconds: Some(60),
        test_type: Some("charging_state".to_string()),
        arguments: Some(serde_json::to_string(&args).unwrap()),
        site_id: None,
        company_id: None,
    };

    let created = create_source(&mut conn, new_source).expect("Failed to create source");
    assert_eq!(created.test_type, Some("charging_state".to_string()));
    
    let stored_args: HashMap<String, String> = serde_json::from_str(
        &created.arguments.unwrap()
    ).unwrap();
    assert_eq!(stored_args.get("battery_id"), Some(&"battery1".to_string()));
}

#[test]
fn test_disk_space_source_no_args() {
    let mut conn = setup_test_db();

    let args: HashMap<String, String> = HashMap::new(); // Empty arguments
    
    let new_source = NewSource {
        name: "test_disk".to_string(),
        description: Some("Test disk source".to_string()),
        active: Some(true),
        interval_seconds: Some(30),
        test_type: Some("disk_space".to_string()),
        arguments: Some(serde_json::to_string(&args).unwrap()),
        site_id: None,
        company_id: None,
    };

    let created = create_source(&mut conn, new_source).expect("Failed to create source");
    assert_eq!(created.test_type, Some("disk_space".to_string()));
    
    let stored_args: HashMap<String, String> = serde_json::from_str(
        &created.arguments.unwrap()
    ).unwrap();
    assert!(stored_args.is_empty());
}

#[test]
fn test_list_sources_shows_test_type_and_args() {
    let mut conn = setup_test_db();

    // Create multiple sources with different test types
    let sources_to_create = vec![
        ("ping_test", "ping", vec![("target", "example.com")]),
        ("battery_test", "charging_state", vec![("battery_id", "main")]),
        ("disk_test", "disk_space", vec![]),
    ];

    for (name, test_type, args_vec) in sources_to_create {
        let mut args = HashMap::new();
        for (key, value) in args_vec {
            args.insert(key.to_string(), value.to_string());
        }
        
        let new_source = NewSource {
            name: name.to_string(),
            description: Some(format!("Test {}", test_type)),
            active: Some(true),
            interval_seconds: Some(5),
            test_type: Some(test_type.to_string()),
            arguments: Some(serde_json::to_string(&args).unwrap()),
            site_id: None,
            company_id: None,
        };
        
        create_source(&mut conn, new_source).expect("Failed to create source");
    }

    // List all sources and verify they have the correct test_type and arguments
    let sources = list_sources(&mut conn).expect("Failed to list sources");
    assert_eq!(sources.len(), 3);

    for source in sources {
        match source.name.as_str() {
            "ping_test" => {
                assert_eq!(source.test_type, Some("ping".to_string()));
                let args: HashMap<String, String> = serde_json::from_str(
                    &source.arguments.unwrap()
                ).unwrap();
                assert_eq!(args.get("target"), Some(&"example.com".to_string()));
            }
            "battery_test" => {
                assert_eq!(source.test_type, Some("charging_state".to_string()));
                let args: HashMap<String, String> = serde_json::from_str(
                    &source.arguments.unwrap()
                ).unwrap();
                assert_eq!(args.get("battery_id"), Some(&"main".to_string()));
            }
            "disk_test" => {
                assert_eq!(source.test_type, Some("disk_space".to_string()));
                let args: HashMap<String, String> = serde_json::from_str(
                    &source.arguments.unwrap()
                ).unwrap();
                assert!(args.is_empty());
            }
            _ => panic!("Unexpected source name: {}", source.name),
        }
    }
}

#[test]
fn test_source_get_arguments_helper() {
    let mut conn = setup_test_db();

    let mut expected_args = HashMap::new();
    expected_args.insert("target".to_string(), "test.com".to_string());
    expected_args.insert("timeout".to_string(), "5".to_string());
    
    let new_source = NewSource {
        name: "test_multi_args".to_string(),
        description: Some("Test multiple arguments".to_string()),
        active: Some(true),
        interval_seconds: Some(10),
        test_type: Some("ping".to_string()),
        arguments: Some(serde_json::to_string(&expected_args).unwrap()),
        site_id: None,
        company_id: None,
    };

    let created = create_source(&mut conn, new_source).expect("Failed to create source");
    
    // Test the get_arguments helper method
    let parsed_args = created.get_arguments().expect("Failed to parse arguments");
    assert_eq!(parsed_args, expected_args);
}

#[test]
fn test_invalid_json_arguments_handling() {
    let mut conn = setup_test_db();

    let new_source = NewSource {
        name: "test_invalid_json".to_string(),
        description: Some("Test invalid JSON".to_string()),
        active: Some(true),
        interval_seconds: Some(10),
        test_type: Some("ping".to_string()),
        arguments: Some("invalid json".to_string()),
        site_id: None,
        company_id: None,
    };

    let created = create_source(&mut conn, new_source).expect("Failed to create source");
    
    // The get_arguments method should return an error for invalid JSON
    let result = created.get_arguments();
    assert!(result.is_err());
}

#[test]
fn test_legacy_and_new_format_coexistence() {
    let mut conn = setup_test_db();

    // Create a "legacy" source (NULL test_type and arguments)
    let legacy_source = NewSource {
        name: "legacy_source".to_string(),
        description: Some("Legacy source".to_string()),
        active: Some(true),
        interval_seconds: Some(5),
        test_type: None,
        arguments: None,
        site_id: None,
        company_id: None,
    };

    let legacy_created = create_source(&mut conn, legacy_source).expect("Failed to create legacy source");
    assert_eq!(legacy_created.test_type, None);
    assert_eq!(legacy_created.arguments, None);

    // Create a new format source
    let mut args = HashMap::new();
    args.insert("target".to_string(), "example.com".to_string());
    
    let new_source = NewSource {
        name: "new_source".to_string(),
        description: Some("New format source".to_string()),
        active: Some(true),
        interval_seconds: Some(5),
        test_type: Some("ping".to_string()),
        arguments: Some(serde_json::to_string(&args).unwrap()),
        site_id: None,
        company_id: None,
    };

    let new_created = create_source(&mut conn, new_source).expect("Failed to create new source");
    assert_eq!(new_created.test_type, Some("ping".to_string()));
    assert!(new_created.arguments.is_some());

    // Both should coexist in the database
    let sources = list_sources(&mut conn).expect("Failed to list sources");
    assert_eq!(sources.len(), 2);
}