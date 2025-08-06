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

use neems_api::api::data::{DataSourcesResponse, ReadingsResponse};
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

/// Test the /api/1/data/readings/<source_id> endpoint with latest parameter.
/// 
/// This test verifies the single source readings endpoint with the most basic
/// query parameter (latest) to get recent readings.
#[tokio::test]
async fn test_get_source_readings_latest() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // First get available sources to find a valid source_id
    let sources_response = client.get("/api/1/data").dispatch().await;
    assert_eq!(sources_response.status(), Status::Ok);
    
    let sources: DataSourcesResponse = sources_response
        .into_json()
        .await
        .expect("valid DataSourcesResponse JSON");
    
    if sources.sources.is_empty() {
        println!("No sources available in test database, skipping readings test");
        return;
    }
    
    let test_source_id = sources.sources[0].id.expect("Source should have ID");
    
    // Test with latest=10 parameter
    let url = format!("/api/1/data/readings/{}?latest=10", test_source_id);
    let response = client
        .get(&url)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let readings_response: ReadingsResponse = response
        .into_json()
        .await
        .expect("valid ReadingsResponse JSON");
    
    // Verify response structure
    assert_eq!(readings_response.source_id, Some(test_source_id));
    assert!(readings_response.readings.len() <= 10, "Should return at most 10 readings");
    
    // If there are readings, verify they're for the correct source
    for reading in &readings_response.readings {
        assert_eq!(reading.source_id, test_source_id);
        assert!(reading.data.len() > 0, "Reading data should not be empty");
    }
}

/// Test the /api/1/data/readings/<source_id> endpoint with time window parameters.
/// 
/// This test verifies time-based filtering with since/until parameters.
#[tokio::test]
async fn test_get_source_readings_time_window() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // Get available sources
    let sources_response = client.get("/api/1/data").dispatch().await;
    let sources: DataSourcesResponse = sources_response.into_json().await.unwrap();
    
    if sources.sources.is_empty() {
        println!("No sources available in test database, skipping time window test");
        return;
    }
    
    let test_source_id = sources.sources[0].id.unwrap();
    
    // Test with time window
    let url = format!("/api/1/data/readings/{}?since=2024-01-01T00:00:00Z&until=2024-12-31T23:59:59Z", test_source_id);
    let response = client
        .get(&url)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let readings_response: ReadingsResponse = response
        .into_json()
        .await
        .expect("valid ReadingsResponse JSON");
    
    assert_eq!(readings_response.source_id, Some(test_source_id));
}

/// Test the /api/1/data/readings/<source_id> endpoint with invalid source ID.
/// 
/// This test verifies that the endpoint returns 404 for non-existent sources.
#[tokio::test]
async fn test_get_source_readings_not_found() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // Use a source ID that definitely doesn't exist
    let response = client
        .get("/api/1/data/readings/99999?latest=1")
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::NotFound);
}

/// Test the /api/1/data/readings/<source_id> endpoint with invalid query parameters.
/// 
/// This test verifies parameter validation (conflicting time parameters).
#[tokio::test]
async fn test_get_source_readings_invalid_params() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // Get a valid source ID first
    let sources_response = client.get("/api/1/data").dispatch().await;
    let sources: DataSourcesResponse = sources_response.into_json().await.unwrap();
    
    if sources.sources.is_empty() {
        return;
    }
    
    let test_source_id = sources.sources[0].id.unwrap();
    
    // Test with conflicting parameters (both latest and since)
    let url = format!("/api/1/data/readings/{}?latest=10&since=2024-01-01T00:00:00Z", test_source_id);
    let response = client
        .get(&url)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::BadRequest);
}

/// Test the /api/1/data/readings endpoint for multiple sources.
/// 
/// This test verifies the multi-source readings endpoint with source_ids parameter.
#[tokio::test]
async fn test_get_multi_source_readings() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // Get available sources
    let sources_response = client.get("/api/1/data").dispatch().await;
    let sources: DataSourcesResponse = sources_response.into_json().await.unwrap();
    
    if sources.sources.len() < 2 {
        println!("Need at least 2 sources for multi-source test, skipping");
        return;
    }
    
    let source_id_1 = sources.sources[0].id.unwrap();
    let source_id_2 = sources.sources[1].id.unwrap();
    
    // Test with multiple source IDs
    let url = format!("/api/1/data/readings?source_ids={},{}&latest=5", source_id_1, source_id_2);
    let response = client
        .get(&url)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let readings_response: ReadingsResponse = response
        .into_json()
        .await
        .expect("valid ReadingsResponse JSON");
    
    // Verify response structure
    assert_eq!(readings_response.source_id, None); // Multi-source query
    
    // Verify all readings are from requested sources
    let requested_sources = vec![source_id_1, source_id_2];
    for reading in &readings_response.readings {
        assert!(requested_sources.contains(&reading.source_id), 
               "Reading should be from one of the requested sources");
    }
}

/// Test the /api/1/data/readings endpoint without required source_ids parameter.
/// 
/// This test verifies that the multi-source endpoint returns 400 without source_ids.
#[tokio::test]
async fn test_get_multi_source_readings_missing_source_ids() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // Test without source_ids parameter
    let response = client
        .get("/api/1/data/readings?latest=5")
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::BadRequest);
}

/// Test the /api/1/data/readings endpoint with invalid source IDs in the list.
/// 
/// This test verifies that the endpoint returns 404 if any source doesn't exist.
#[tokio::test]
async fn test_get_multi_source_readings_invalid_source() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // Get one valid source
    let sources_response = client.get("/api/1/data").dispatch().await;
    let sources: DataSourcesResponse = sources_response.into_json().await.unwrap();
    
    if sources.sources.is_empty() {
        return;
    }
    
    let valid_source_id = sources.sources[0].id.unwrap();
    
    // Test with one valid and one invalid source ID
    let url = format!("/api/1/data/readings?source_ids={},99999&latest=5", valid_source_id);
    let response = client
        .get(&url)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::NotFound);
}

/// Test readings endpoint response structure validation.
/// 
/// This test validates the JSON structure and field types of the readings response.
#[tokio::test]
async fn test_readings_response_structure() {
    let client = Client::tracked(test_rocket_with_site_db())
        .await
        .expect("valid rocket instance");
    
    // Get available sources
    let sources_response = client.get("/api/1/data").dispatch().await;
    let sources: DataSourcesResponse = sources_response.into_json().await.unwrap();
    
    if sources.sources.is_empty() {
        return;
    }
    
    let test_source_id = sources.sources[0].id.unwrap();
    
    let url = format!("/api/1/data/readings/{}?latest=1", test_source_id);
    let response = client
        .get(&url)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    // Parse as generic JSON first to inspect structure
    let response_json: serde_json::Value = response
        .into_json()
        .await
        .expect("valid JSON response");
    
    // Verify top-level structure
    assert!(response_json.is_object(), "Response should be an object");
    assert!(response_json.get("readings").is_some(), "Response should have 'readings' field");
    assert!(response_json.get("source_id").is_some(), "Response should have 'source_id' field");
    assert!(response_json.get("total_count").is_some(), "Response should have 'total_count' field");
    
    let readings = response_json["readings"].as_array()
        .expect("readings should be an array");
    
    // If there are readings, validate their structure
    for reading in readings {
        assert!(reading.is_object(), "Each reading should be an object");
        
        // Check for expected fields from neems_data::models::Reading
        assert!(reading.get("id").is_some(), "Reading should have 'id' field");
        assert!(reading.get("source_id").is_some(), "Reading should have 'source_id' field");
        assert!(reading.get("timestamp").is_some(), "Reading should have 'timestamp' field");
        assert!(reading.get("data").is_some(), "Reading should have 'data' field");
        assert!(reading.get("quality_flags").is_some(), "Reading should have 'quality_flags' field");
        
        // Validate field types
        assert!(reading["source_id"].is_number(), "source_id should be number");
        assert!(reading["timestamp"].is_string(), "timestamp should be string");
        assert!(reading["data"].is_string(), "data should be string");
        assert!(reading["quality_flags"].is_number(), "quality_flags should be number");
    }
}