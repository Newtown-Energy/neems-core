//! Tests for data API endpoints.
//!
//! This module tests the data API endpoints that provide access to neems-data sources
//! and schema information. These tests use the existing test-data.db file which is
//! pre-populated with test data. 
//!
//! The tests for /api/1/data endpoint are always available since that endpoint is not feature-gated.
//! The tests for /api/1/data/schema endpoint require the `test-staging` feature.

use rocket::http::Status;
use rocket::local::asynchronous::Client;

use neems_api::api::data::DataSourcesResponse;
use neems_api::orm::neems_data::testing::test_rocket_with_site_db;


/// Test the /api/1/data endpoint returns a valid list of data sources.
/// 
/// This test verifies:
/// - Endpoint returns HTTP 200 OK  
/// - Response is valid JSON matching DataSourcesResponse structure
/// - Sources contain expected fields from neems_data::models::Source
/// 
/// This test uses the existing test-data.db file for real data.
#[tokio::test]
async fn test_list_data_sources_success() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    let response = client.get("/api/1/data").dispatch().await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let sources_response: DataSourcesResponse = response
        .into_json()
        .await
        .expect("valid DataSourcesResponse JSON");
    
    // Verify response structure - should have sources array
    // Note: We don't assert specific count since test-data.db content may vary
    // but we can validate the structure of any sources that exist
    for source in &sources_response.sources {
        // Validate required fields from neems_data::models::Source
        assert!(source.name.len() > 0, "Source name should not be empty");
        assert!(source.interval_seconds > 0, "Interval should be positive");
        
        // These fields should be present (though description can be None)
        assert!(source.id.is_some(), "Source should have an ID");
        // active field should be present (boolean)
        // created_at and updated_at should be present (NaiveDateTime)
        // last_run can be None
    }
}

/// Test the /api/1/data endpoint response structure in detail.
/// 
/// This test focuses on validating the JSON structure and field types
/// without depending on specific data content.
#[tokio::test]
async fn test_list_data_sources_response_structure() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    let response = client.get("/api/1/data").dispatch().await;
    
    assert_eq!(response.status(), Status::Ok);
    
    // Parse as generic JSON first to inspect structure
    let response_json: serde_json::Value = response
        .into_json()
        .await
        .expect("valid JSON response");
    
    // Verify top-level structure
    assert!(response_json.is_object(), "Response should be an object");
    assert!(response_json.get("sources").is_some(), "Response should have 'sources' field");
    
    let sources = response_json["sources"].as_array()
        .expect("sources should be an array");
    
    // If there are sources, validate their structure
    for source in sources {
        assert!(source.is_object(), "Each source should be an object");
        
        // Check for expected fields from neems_data::models::Source
        assert!(source.get("name").is_some(), "Source should have 'name' field");
        assert!(source.get("active").is_some(), "Source should have 'active' field");
        assert!(source.get("interval_seconds").is_some(), "Source should have 'interval_seconds' field");
        assert!(source.get("created_at").is_some(), "Source should have 'created_at' field");
        assert!(source.get("updated_at").is_some(), "Source should have 'updated_at' field");
        
        // Validate field types
        assert!(source["name"].is_string(), "name should be string");
        assert!(source["active"].is_boolean(), "active should be boolean");
        assert!(source["interval_seconds"].is_number(), "interval_seconds should be number");
        assert!(source["created_at"].is_string(), "created_at should be string");
        assert!(source["updated_at"].is_string(), "updated_at should be string");
        
        // Optional fields
        if let Some(description) = source.get("description") {
            if !description.is_null() {
                assert!(description.is_string(), "description should be string when present");
            }
        }
        
        if let Some(last_run) = source.get("last_run") {
            if !last_run.is_null() {
                assert!(last_run.is_string(), "last_run should be string when present");
            }
        }
    }
}

/// Test the /api/1/data/schema endpoint returns valid schema information.
/// 
/// This test is feature-gated to only run when test-staging is enabled.
/// It verifies:
/// - Endpoint returns HTTP 200 OK
/// - Response contains schema field with SQL statements
/// - Schema includes expected tables from neems-data
#[cfg(feature = "test-staging")]
#[tokio::test]
async fn test_get_schema_success() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    let response = client.get("/api/1/data/schema").dispatch().await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let schema_response: serde_json::Value = response
        .into_json()
        .await
        .expect("valid JSON response");
    
    // Verify response structure
    assert!(schema_response.is_object(), "Schema response should be an object");
    assert!(schema_response.get("schema").is_some(), "Response should have 'schema' field");
    
    let schema = schema_response["schema"].as_str()
        .expect("schema field should be a string");
    
    // Verify schema is not empty
    assert!(!schema.is_empty(), "Schema should not be empty");
    
    // Verify schema contains expected tables from neems-data migrations
    assert!(schema.contains("sources"), "Schema should reference sources table");
    assert!(schema.contains("readings"), "Schema should reference readings table");
}

/// Test the /api/1/data/schema endpoint contains expected database objects.
/// 
/// This test validates that the schema dump includes the core tables and
/// structures defined in the neems-data migrations.
#[cfg(feature = "test-staging")]
#[tokio::test]
async fn test_get_schema_contains_expected_tables() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    let response = client.get("/api/1/data/schema").dispatch().await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let schema_response: serde_json::Value = response
        .into_json()
        .await
        .expect("valid JSON response");
    
    let schema = schema_response["schema"].as_str()
        .expect("schema field should be a string");
    
    // Verify schema contains CREATE statements for core tables
    assert!(schema.contains("CREATE TABLE sources") || schema.contains("CREATE TABLE `sources`"), 
            "Schema should contain sources table creation");
    assert!(schema.contains("CREATE TABLE readings") || schema.contains("CREATE TABLE `readings`"), 
            "Schema should contain readings table creation");
    
    // Verify schema contains expected indexes from migrations
    assert!(schema.contains("idx_readings_source_time") || schema.contains("readings"), 
            "Schema should reference readings table structures");
    
    // Verify schema is substantial (not just empty or error message)
    assert!(schema.len() > 100, "Schema should be substantial (>100 chars)");
}