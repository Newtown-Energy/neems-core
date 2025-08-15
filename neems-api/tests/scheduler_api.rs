//! Integration tests for the scheduler API endpoints.

use chrono::Utc;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::serde::json::json;
use serde_json::Value;

use neems_api::models::Site;
use neems_api::orm::testing::fast_test_rocket;

/// Helper to login and get session cookie
async fn login_and_get_session(client: &Client) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": "superadmin@example.com",
        "password": "admin"
    });

    let response = client
        .post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}


/// Helper to get a test site
async fn get_test_site(client: &Client, session_cookie: &rocket::http::Cookie<'static>) -> Site {
    let response = client
        .get("/api/1/Sites")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> = serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    sites.into_iter().next().expect("At least one site should exist")
}

/// Test creating a scheduler script
#[rocket::async_test]
async fn test_create_scheduler_script() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    let script_input = json!({
        "site_id": site.id,
        "name": "Test Script",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    
    let script: Value = response.into_json().await.expect("Valid JSON response");
    assert_eq!(script["name"], "Test Script");
    assert_eq!(script["site_id"], site.id);
    assert_eq!(script["script_content"], "return 'charge'");
    assert_eq!(script["language"], "lua");
}

/// Test creating a scheduler script with duplicate name should fail
#[rocket::async_test]
async fn test_create_duplicate_scheduler_script() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    let script_input = json!({
        "site_id": site.id,
        "name": "Duplicate Script",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    // Create first script
    let response1 = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;
    assert_eq!(response1.status(), Status::Created);

    // Try to create second script with same name
    let response2 = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;
    assert_eq!(response2.status(), Status::Conflict);
}

/// Test listing scheduler scripts
#[rocket::async_test]
async fn test_list_scheduler_scripts() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Create multiple scripts
    for i in 1..=3 {
        let script_input = json!({
            "site_id": site.id,
            "name": format!("Test Script {}", i),
            "script_content": "return 'idle'",
            "language": "lua",
            "is_active": true,
            "version": 1
        });

        let response = client
            .post("/api/1/SchedulerScripts")
            .header(ContentType::JSON)
            .cookie(session_cookie.clone())
            .body(script_input.to_string())
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Created);
    }

    // List scripts
    let response = client
        .get("/api/1/SchedulerScripts")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    
    let scripts_response: Value = response.into_json().await.expect("Valid JSON response");
    let scripts = scripts_response["value"].as_array().expect("Scripts array");
    assert!(scripts.len() >= 3);
}

/// Test getting a specific scheduler script
#[rocket::async_test]
async fn test_get_scheduler_script() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Create a script
    let script_input = json!({
        "site_id": site.id,
        "name": "Get Test Script",
        "script_content": "return 'discharge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let create_response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;
    
    let created_script: Value = create_response.into_json().await.expect("Valid JSON response");
    let script_id = created_script["id"].as_i64().expect("Script ID");

    // Get the script
    let response = client
        .get(format!("/api/1/SchedulerScripts/{}", script_id))
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    
    let script: Value = response.into_json().await.expect("Valid JSON response");
    assert_eq!(script["id"], script_id);
    assert_eq!(script["name"], "Get Test Script");
}

/// Test updating a scheduler script
#[rocket::async_test]
async fn test_update_scheduler_script() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Create a script
    let script_input = json!({
        "site_id": site.id,
        "name": "Update Test Script",
        "script_content": "return 'idle'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let create_response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;
    
    let created_script: Value = create_response.into_json().await.expect("Valid JSON response");
    let script_id = created_script["id"].as_i64().expect("Script ID");

    // Update the script
    let update_data = json!({
        "name": "Updated Script Name",
        "script_content": "return 'charge'",
        "is_active": false
    });

    let response = client
        .put(format!("/api/1/SchedulerScripts/{}", script_id))
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(update_data.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    
    let updated_script: Value = response.into_json().await.expect("Valid JSON response");
    assert_eq!(updated_script["name"], "Updated Script Name");
    assert_eq!(updated_script["script_content"], "return 'charge'");
    assert_eq!(updated_script["is_active"], false);
}

/// Test deleting a scheduler script
#[rocket::async_test]
async fn test_delete_scheduler_script() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Create a script
    let script_input = json!({
        "site_id": site.id,
        "name": "Delete Test Script",
        "script_content": "return 'idle'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let create_response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;
    
    let created_script: Value = create_response.into_json().await.expect("Valid JSON response");
    let script_id = created_script["id"].as_i64().expect("Script ID");

    // Delete the script
    let response = client
        .delete(format!("/api/1/SchedulerScripts/{}", script_id))
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify it's deleted
    let get_response = client
        .get(format!("/api/1/SchedulerScripts/{}", script_id))
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(get_response.status(), Status::NotFound);
}

/// Test creating a scheduler override
#[rocket::async_test]
async fn test_create_scheduler_override() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    let now = Utc::now().naive_utc();
    let start_time = now + chrono::Duration::hours(1);
    let end_time = start_time + chrono::Duration::hours(2);

    let override_input = json!({
        "site_id": site.id,
        "state": "charge",
        "start_time": start_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "end_time": end_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "reason": "Maintenance override",
        "is_active": true
    });

    let response = client
        .post("/api/1/SchedulerOverrides")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(override_input.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    
    let override_record: Value = response.into_json().await.expect("Valid JSON response");
    assert_eq!(override_record["site_id"], site.id);
    assert_eq!(override_record["state"], "charge");
    assert_eq!(override_record["reason"], "Maintenance override");
}

/// Test creating conflicting overrides should fail
#[rocket::async_test]
async fn test_create_conflicting_scheduler_override() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    let now = Utc::now().naive_utc();
    let start_time1 = now + chrono::Duration::hours(1);
    let end_time1 = start_time1 + chrono::Duration::hours(3);

    // Create first override
    let override_input1 = json!({
        "site_id": site.id,
        "state": "charge",
        "start_time": start_time1.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "end_time": end_time1.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "reason": "First override",
        "is_active": true
    });

    let response1 = client
        .post("/api/1/SchedulerOverrides")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(override_input1.to_string())
        .dispatch()
        .await;
    assert_eq!(response1.status(), Status::Created);

    // Try to create overlapping override
    let start_time2 = start_time1 + chrono::Duration::hours(1); // Overlaps with first
    let end_time2 = start_time2 + chrono::Duration::hours(2);

    let override_input2 = json!({
        "site_id": site.id,
        "state": "discharge",
        "start_time": start_time2.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "end_time": end_time2.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "reason": "Conflicting override",
        "is_active": true
    });

    let response2 = client
        .post("/api/1/SchedulerOverrides")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(override_input2.to_string())
        .dispatch()
        .await;
    assert_eq!(response2.status(), Status::Conflict);
}

/// Test validating a scheduler script
#[rocket::async_test]
async fn test_validate_scheduler_script() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Create a valid script
    let script_input = json!({
        "site_id": site.id,
        "name": "Validation Test Script",
        "script_content": "if datetime.hour >= 9 and datetime.hour < 17 then return 'charge' else return 'idle' end",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let create_response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;
    
    let created_script: Value = create_response.into_json().await.expect("Valid JSON response");
    let script_id = created_script["id"].as_i64().expect("Script ID");

    // Validate the script
    let response = client
        .post(format!("/api/1/SchedulerScripts/{}/validate", script_id))
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    
    let validation: Value = response.into_json().await.expect("Valid JSON response");
    assert_eq!(validation["is_valid"], true);
    assert!(validation["error"].is_null());
    assert!(validation["test_state"].is_string());
}

/// Test executing scheduler for a site
#[rocket::async_test]
async fn test_execute_site_scheduler() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Create a script for the site
    let script_input = json!({
        "site_id": site.id,
        "name": "Execution Test Script",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let create_response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;
    assert_eq!(create_response.status(), Status::Created);

    // Execute the scheduler
    let response = client
        .post(format!("/api/1/Sites/{}/scheduler/execute", site.id))
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body("{}")
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    
    let execution: Value = response.into_json().await.expect("Valid JSON response");
    assert_eq!(execution["state"], "charge");
    assert!(execution["source"].as_str().unwrap().starts_with("script:"));
}

/// Test getting site state
#[rocket::async_test]
async fn test_get_site_state() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Test default state (no scripts or overrides)
    let response = client
        .get(format!("/api/1/Sites/{}/scheduler/state", site.id))
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    
    let state: Value = response.into_json().await.expect("Valid JSON response");
    assert_eq!(state["site_id"], site.id);
    assert_eq!(state["state"], "idle"); // Default state
    assert_eq!(state["source"], "default");
}

/// Test site scheduler scripts navigation
#[rocket::async_test]
async fn test_site_scheduler_scripts_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    // Create scripts for the site
    for i in 1..=2 {
        let script_input = json!({
            "site_id": site.id,
            "name": format!("Site Script {}", i),
            "script_content": "return 'idle'",
            "language": "lua",
            "is_active": true,
            "version": 1
        });

        let response = client
            .post("/api/1/SchedulerScripts")
            .header(ContentType::JSON)
            .cookie(session_cookie.clone())
            .body(script_input.to_string())
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Created);
    }

    // Get scripts for the site via navigation
    let response = client
        .get(format!("/api/1/Sites/{}/SchedulerScripts", site.id))
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    
    let scripts: Value = response.into_json().await.expect("Valid JSON response");
    let scripts_array = scripts.as_array().expect("Scripts array");
    assert_eq!(scripts_array.len(), 2);
    
    // Verify all scripts belong to the site
    for script in scripts_array {
        assert_eq!(script["site_id"], site.id);
    }
}

/// Test creating override with invalid state should fail
#[rocket::async_test]
async fn test_create_override_invalid_state() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    let now = Utc::now().naive_utc();
    let start_time = now + chrono::Duration::hours(1);
    let end_time = start_time + chrono::Duration::hours(2);

    let override_input = json!({
        "site_id": site.id,
        "state": "invalid_state", // Invalid state
        "start_time": start_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "end_time": end_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "reason": "Invalid state test",
        "is_active": true
    });

    let response = client
        .post("/api/1/SchedulerOverrides")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(override_input.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}

/// Test creating override with end time before start time should fail
#[rocket::async_test]
async fn test_create_override_invalid_time_range() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let site = get_test_site(&client, &session_cookie).await;

    let now = Utc::now().naive_utc();
    let start_time = now + chrono::Duration::hours(2);
    let end_time = now + chrono::Duration::hours(1); // End before start

    let override_input = json!({
        "site_id": site.id,
        "state": "charge",
        "start_time": start_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "end_time": end_time.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "reason": "Invalid time range test",
        "is_active": true
    });

    let response = client
        .post("/api/1/SchedulerOverrides")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(override_input.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}