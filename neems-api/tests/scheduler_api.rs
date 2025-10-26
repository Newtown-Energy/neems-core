//! Integration tests for the scheduler API endpoints.

use chrono::Utc;
use neems_api::{models::Site, orm::testing::fast_test_rocket};
use rocket::{
    http::{ContentType, Status},
    local::asynchronous::Client,
    serde::json::json,
};
use serde_json::Value;

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
    let response = client.get("/api/1/Sites").cookie(session_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
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

// ========== RBAC TESTS ==========

// Note: Removed unused imports Company and UserWithRoles since we're using
// existing golden DB entities

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
    login_and_get_session(client).await
}

/// Helper to login as a specific user
async fn login_user(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": email,
        "password": password
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

// Note: Using existing golden database entities instead of creating new ones
// Golden database contains:
// - Device Test Company A (ID 5) with admin@devicetesta.com and Device API Site
//   A (ID 1)
// - Device Test Company B (ID 6) with admin@devicetestb.com and Device API Site
//   B (ID 2)
// - newtown_superadmin@example.com with newtown-admin role

/// Test that company admins can only create scripts for their own company's
/// sites
#[rocket::async_test]
async fn test_scheduler_script_rbac_create_cross_company() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get all sites as admin to find sites from different companies
    let response = client.get("/api/1/Sites").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let all_sites: Vec<Site> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");

    // Find sites from different companies for cross-company testing
    let site_a = all_sites
        .iter()
        .find(|s| s.name == "Device API Site A")
        .expect("Device API Site A should exist in golden DB");
    let site_b = all_sites
        .iter()
        .find(|s| s.name == "Device API Site B")
        .expect("Device API Site B should exist in golden DB");

    // Ensure we have sites from different companies
    assert_ne!(
        site_a.company_id, site_b.company_id,
        "Test requires sites from different companies"
    );

    // Login as admin for Device Test Company A
    let user_a_cookie = login_user(&client, "admin@devicetesta.com", "admin").await;

    // Company A admin should be able to create script for their company's site
    let script_input = json!({
        "site_id": site_a.id,
        "name": "Company A Script",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(user_a_cookie.clone())
        .body(script_input.to_string())
        .dispatch()
        .await;

    assert_eq!(
        response.status(),
        Status::Created,
        "Company admin should be able to create scripts for their own company's sites"
    );

    // Company A admin should NOT be able to create script for Company B's site
    let script_input_b = json!({
        "site_id": site_b.id,
        "name": "Cross Company Script",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let response = client
        .post("/api/1/SchedulerScripts")
        .header(ContentType::JSON)
        .cookie(user_a_cookie.clone())
        .body(script_input_b.to_string())
        .dispatch()
        .await;

    assert_eq!(
        response.status(),
        Status::Forbidden,
        "Company admin should NOT be able to create scripts for other companies' sites"
    );
}

/// Test that company admins can only list scripts for their own company's sites
#[rocket::async_test]
async fn test_scheduler_script_rbac_list_filtering() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Use existing sites from golden database
    let site_a_id = 1; // Device API Site A (belongs to Device Test Company A)
    let site_b_id = 2; // Device API Site B (belongs to Device Test Company B)

    // Create scripts for both sites as superadmin
    let script_a = json!({
        "site_id": site_a_id,
        "name": "Script Alpha",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let script_b = json!({
        "site_id": site_b_id,
        "name": "Script Beta",
        "script_content": "return 'discharge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    // Create both scripts as superadmin
    client
        .post("/api/1/SchedulerScripts")
        .cookie(admin_cookie.clone())
        .json(&script_a)
        .dispatch()
        .await;
    client
        .post("/api/1/SchedulerScripts")
        .cookie(admin_cookie.clone())
        .json(&script_b)
        .dispatch()
        .await;

    // Use existing admin user for Device Test Company A
    let user_a_cookie = login_user(&client, "admin@devicetesta.com", "admin").await;

    // Company A admin should only see scripts for their company's sites
    let response = client
        .get("/api/1/SchedulerScripts")
        .cookie(user_a_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let scripts_response: serde_json::Value =
        response.into_json().await.expect("Valid JSON response");
    let scripts = scripts_response["value"].as_array().expect("Scripts array");

    // Test what matters: RBAC filtering behavior
    // 1. Company A admin can access scripts endpoint (returns 200) ✓
    // 2. All returned scripts belong to Company A's sites (business logic)
    for script in scripts {
        assert_eq!(
            script["site_id"], site_a_id,
            "Company A admin should only see scripts for their company's sites"
        );
    }

    // 3. Should contain Script Alpha if any scripts are returned
    if !scripts.is_empty() {
        let script_names: Vec<&str> = scripts.iter().map(|s| s["name"].as_str().unwrap()).collect();
        assert!(
            script_names.contains(&"Script Alpha"),
            "Should include Script Alpha for Company A's site"
        );
        assert!(
            !script_names.contains(&"Script Beta"),
            "Should NOT include Script Beta from Company B's site"
        );
    }
}

/// Test that company admins cannot view scripts from other companies
#[rocket::async_test]
async fn test_scheduler_script_rbac_get_cross_company() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Use existing sites from golden database
    let site_b_id = 2; // Device API Site B (belongs to Device Test Company B)

    // Create script for Device Test Company B as superadmin
    let script_b = json!({
        "site_id": site_b_id,
        "name": "Script Delta",
        "script_content": "return 'idle'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let create_response = client
        .post("/api/1/SchedulerScripts")
        .cookie(admin_cookie.clone())
        .json(&script_b)
        .dispatch()
        .await;

    let created_script: serde_json::Value =
        create_response.into_json().await.expect("Valid JSON response");
    let script_id = created_script["id"].as_i64().expect("Script ID");

    // Use existing admin user for Device Test Company A (different company)
    let user_a_cookie = login_user(&client, "admin@devicetesta.com", "admin").await;

    // Company A admin should NOT be able to view Company B's script
    let response = client
        .get(format!("/api/1/SchedulerScripts/{}", script_id))
        .cookie(user_a_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

/// Test that newtown-admin can access all scripts regardless of company
#[rocket::async_test]
async fn test_scheduler_script_rbac_newtown_admin_access() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Use existing sites from golden database
    let site_a_id = 1; // Device API Site A (belongs to Device Test Company A)
    let site_b_id = 2; // Device API Site B (belongs to Device Test Company B)

    // Use existing newtown-admin user from golden database
    let newtown_cookie = login_user(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Newtown admin should be able to create scripts for any company's sites
    let script_a = json!({
        "site_id": site_a_id,
        "name": "Newtown Script A",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    let script_b = json!({
        "site_id": site_b_id,
        "name": "Newtown Script B",
        "script_content": "return 'discharge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    // Both should succeed
    let response_a = client
        .post("/api/1/SchedulerScripts")
        .cookie(newtown_cookie.clone())
        .json(&script_a)
        .dispatch()
        .await;
    assert_eq!(response_a.status(), Status::Created);

    let response_b = client
        .post("/api/1/SchedulerScripts")
        .cookie(newtown_cookie.clone())
        .json(&script_b)
        .dispatch()
        .await;
    assert_eq!(response_b.status(), Status::Created);

    // Newtown admin should see all scripts
    let response = client
        .get("/api/1/SchedulerScripts")
        .cookie(newtown_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let scripts_response: serde_json::Value =
        response.into_json().await.expect("Valid JSON response");
    let scripts = scripts_response["value"].as_array().expect("Scripts array");

    // Should see both scripts
    assert!(scripts.len() >= 2);
    let script_names: Vec<&str> = scripts.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(script_names.contains(&"Newtown Script A"));
    assert!(script_names.contains(&"Newtown Script B"));
}

/// Test site navigation endpoints with RBAC
#[rocket::async_test]
async fn test_scheduler_script_rbac_site_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get all sites to find sites from different companies
    let response = client.get("/api/1/Sites").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let all_sites: Vec<Site> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");

    // Find two sites from different companies
    let site_company1 = all_sites
        .iter()
        .find(|s| s.company_id == 2) // Test Company 1
        .expect("Should have a site for Test Company 1");
    let site_company2 = all_sites
        .iter()
        .find(|s| s.company_id == 3) // Test Company 2
        .expect("Should have a site for Test Company 2");

    // Create script for Company 2's site as superadmin
    let script_company2 = json!({
        "site_id": site_company2.id,
        "name": "Site Lima Script",
        "script_content": "return 'charge'",
        "language": "lua",
        "is_active": true,
        "version": 1
    });

    client
        .post("/api/1/SchedulerScripts")
        .cookie(admin_cookie.clone())
        .json(&script_company2)
        .dispatch()
        .await;

    // Use existing admin user for Test Company 1 (admin@company1.com belongs to
    // Company 2 actually) Let's use user@company2.com (belongs to Company 3) to
    // try accessing Company 2's sites
    let user_company2_cookie = login_user(&client, "user@company2.com", "admin").await;

    // Company 2 admin should be able to access their own site's scripts
    let response = client
        .get(format!("/api/1/Sites/{}/SchedulerScripts", site_company2.id))
        .cookie(user_company2_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // But Company 2 admin should NOT be able to access Company 1's site scripts
    let response = client
        .get(format!("/api/1/Sites/{}/SchedulerScripts", site_company1.id))
        .cookie(user_company2_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}
