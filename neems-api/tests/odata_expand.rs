//! Integration tests for OData $expand functionality
//!
//! This module tests the OData $expand query option to ensure that
//! navigation properties are correctly expanded in API responses.

use neems_api::orm::testing::fast_test_rocket;
use rocket::{
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use serde_json::{Value, json};

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
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

#[rocket::async_test]
async fn test_users_expand_company() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test $expand=company on users endpoint
    let response = client
        .get("/api/1/Users?$expand=company&$top=1")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    // Verify OData response structure
    assert!(odata_response.get("@odata.context").is_some());
    assert!(odata_response.get("value").is_some());

    let users = odata_response["value"].as_array().expect("users array");
    if let Some(user) = users.first() {
        // Verify that company data is expanded
        assert!(user.get("Company").is_some(), "Company should be expanded");

        let company = &user["Company"];
        assert!(company.get("id").is_some());
        assert!(company.get("name").is_some());

        println!("✓ User with expanded company: {}", serde_json::to_string_pretty(&user).unwrap());
    }
}

#[rocket::async_test]
async fn test_companies_expand_users() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test $expand=users on companies endpoint
    let response = client
        .get("/api/1/Companies?$expand=users&$top=1")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    // Verify OData response structure
    assert!(odata_response.get("@odata.context").is_some());
    assert!(odata_response.get("value").is_some());

    let companies = odata_response["value"].as_array().expect("companies array");
    if let Some(company) = companies.first() {
        // Verify that users data is expanded
        assert!(company.get("Users").is_some(), "Users should be expanded");

        let users = company["Users"].as_array().expect("users array");
        if let Some(user) = users.first() {
            assert!(user.get("id").is_some());
            assert!(user.get("email").is_some());
        }

        println!(
            "✓ Company with expanded users: {}",
            serde_json::to_string_pretty(&company).unwrap()
        );
    }
}

#[rocket::async_test]
async fn test_companies_expand_sites() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test $expand=sites on companies endpoint
    let response = client
        .get("/api/1/Companies?$expand=sites&$top=1")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    // Verify OData response structure
    assert!(odata_response.get("@odata.context").is_some());
    assert!(odata_response.get("value").is_some());

    let companies = odata_response["value"].as_array().expect("companies array");
    if let Some(company) = companies.first() {
        // Verify that sites data is expanded (may be empty)
        assert!(company.get("Sites").is_some(), "Sites should be expanded");

        let sites = company["Sites"].as_array().expect("sites array");
        println!(
            "✓ Company with expanded sites (count: {}): {}",
            sites.len(),
            serde_json::to_string_pretty(&company).unwrap()
        );
    }
}

#[rocket::async_test]
async fn test_expand_multiple_properties() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test multiple expansions: $expand=users,sites
    let response = client
        .get("/api/1/Companies?$expand=users,sites&$top=1")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    let companies = odata_response["value"].as_array().expect("companies array");
    if let Some(company) = companies.first() {
        // Verify both properties are expanded
        assert!(company.get("Users").is_some(), "Users should be expanded");
        assert!(company.get("Sites").is_some(), "Sites should be expanded");

        println!(
            "✓ Company with multiple expansions: {}",
            serde_json::to_string_pretty(&company).unwrap()
        );
    }
}

#[rocket::async_test]
async fn test_expand_with_select() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test $expand combined with $select
    let response = client
        .get("/api/1/Users?$expand=company&$select=id,email,Company&$top=1")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    let users = odata_response["value"].as_array().expect("users array");
    if let Some(user) = users.first() {
        // Should only have selected properties
        assert!(user.get("id").is_some());
        assert!(user.get("email").is_some());
        assert!(user.get("Company").is_some());

        // Should not have non-selected properties
        assert!(user.get("password_hash").is_none());
        assert!(user.get("company_id").is_none());

        println!(
            "✓ User with expand and select: {}",
            serde_json::to_string_pretty(&user).unwrap()
        );
    }
}

#[rocket::async_test]
async fn test_select_activity_timestamps() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test $select with activity timestamps
    let response = client
        .get("/api/1/Users?$select=id,email,activity_created_at,activity_updated_at&$top=1")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    let users = odata_response["value"].as_array().expect("users array");
    if let Some(user) = users.first() {
        // Should have selected basic properties
        assert!(user.get("id").is_some());
        assert!(user.get("email").is_some());

        // Should have activity timestamps (may be null if no activity logged)
        assert!(user.get("activity_created_at").is_some());
        assert!(user.get("activity_updated_at").is_some());

        // Should not have non-selected properties
        assert!(user.get("password_hash").is_none());
        assert!(user.get("company_id").is_none());
        assert!(user.get("created_at").is_none());
        assert!(user.get("updated_at").is_none());

        println!(
            "✓ User with activity timestamps: {}",
            serde_json::to_string_pretty(&user).unwrap()
        );
    }
}

#[rocket::async_test]
async fn test_select_without_activity_timestamps() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test default select (should not include activity timestamps for performance)
    let response = client.get("/api/1/Users?$top=1").cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    let users = odata_response["value"].as_array().expect("users array");
    if let Some(user) = users.first() {
        // Should have all regular UserWithRoles properties
        assert!(user.get("id").is_some());
        assert!(user.get("email").is_some());
        assert!(user.get("password_hash").is_some());
        assert!(user.get("company_id").is_some());
        assert!(user.get("roles").is_some());

        // UserWithRoles doesn't include regular created_at/updated_at
        assert!(user.get("created_at").is_none());
        assert!(user.get("updated_at").is_none());

        // Should NOT have activity timestamps by default
        assert!(user.get("activity_created_at").is_none());
        assert!(user.get("activity_updated_at").is_none());

        println!(
            "✓ Default user response (no activity timestamps): {}",
            serde_json::to_string_pretty(&user).unwrap()
        );
    }
}

#[rocket::async_test]
async fn test_activity_timestamps_for_existing_users() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Query for activity timestamps on existing test users
    let response = client
        .get("/api/1/Users?$select=id,email,activity_created_at,activity_updated_at&$top=3")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let odata_response: Value = response.into_json().await.expect("valid OData JSON");

    let users = odata_response["value"].as_array().expect("users array");
    assert!(!users.is_empty(), "Should have test users in golden database");

    for user in users {
        // Should have selected properties
        assert!(user.get("id").is_some());
        assert!(user.get("email").is_some());

        // Should have activity timestamps (may be null if no activity logged for these
        // test users)
        assert!(user.get("activity_created_at").is_some());
        assert!(user.get("activity_updated_at").is_some());

        // Should not have non-selected properties
        assert!(user.get("password_hash").is_none());
        assert!(user.get("company_id").is_none());

        println!(
            "✓ Test user with activity timestamps: {}",
            serde_json::to_string_pretty(&user).unwrap()
        );
    }
}
