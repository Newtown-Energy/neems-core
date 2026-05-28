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

#[rocket::async_test]
async fn test_users_filter_eq_email() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let response = client
        .get("/api/1/Users?$filter=email%20eq%20%27superadmin@example.com%27")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: Value = response.into_json().await.expect("valid OData JSON");
    let users = odata_response["value"].as_array().expect("users array");

    assert_eq!(users.len(), 1, "$filter=email eq should return exactly one match");
    assert_eq!(users[0]["email"].as_str().unwrap(), "superadmin@example.com");
}

#[rocket::async_test]
async fn test_users_filter_ne_email() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let all_response = client.get("/api/1/Users").cookie(admin_cookie.clone()).dispatch().await;
    let all: Value = all_response.into_json().await.expect("valid OData JSON");
    let total = all["value"].as_array().unwrap().len();
    assert!(total >= 2, "golden db should seed at least 2 users");

    let response = client
        .get("/api/1/Users?$filter=email%20ne%20%27superadmin@example.com%27")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: Value = response.into_json().await.expect("valid OData JSON");
    let users = odata_response["value"].as_array().expect("users array");

    assert_eq!(users.len(), total - 1, "$filter=email ne should exclude exactly one");
    assert!(users.iter().all(|u| u["email"].as_str().unwrap() != "superadmin@example.com"));
}

#[rocket::async_test]
async fn test_users_orderby_email_asc_and_desc() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let asc_resp = client
        .get("/api/1/Users?$orderby=email%20asc")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(asc_resp.status(), Status::Ok);
    let asc: Value = asc_resp.into_json().await.expect("valid OData JSON");
    let asc_emails: Vec<String> = asc["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["email"].as_str().unwrap().to_string())
        .collect();
    let mut sorted_asc = asc_emails.clone();
    sorted_asc.sort();
    assert_eq!(asc_emails, sorted_asc, "$orderby=email asc must be ascending");

    let desc_resp = client
        .get("/api/1/Users?$orderby=email%20desc")
        .cookie(admin_cookie)
        .dispatch()
        .await;
    assert_eq!(desc_resp.status(), Status::Ok);
    let desc: Value = desc_resp.into_json().await.expect("valid OData JSON");
    let desc_emails: Vec<String> = desc["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["email"].as_str().unwrap().to_string())
        .collect();
    let mut sorted_desc = desc_emails.clone();
    sorted_desc.sort_by(|a, b| b.cmp(a));
    assert_eq!(desc_emails, sorted_desc, "$orderby=email desc must be descending");
}

#[rocket::async_test]
async fn test_users_orderby_id_desc() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let resp = client
        .get("/api/1/Users?$orderby=id%20desc")
        .cookie(admin_cookie)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("valid OData JSON");
    let ids: Vec<i64> = body["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["id"].as_i64().unwrap())
        .collect();

    assert!(ids.len() >= 2, "need at least 2 users to check ordering");
    for pair in ids.windows(2) {
        assert!(pair[0] > pair[1], "$orderby=id desc must be strictly descending");
    }
}

#[rocket::async_test]
async fn test_users_skip_and_top_pagination() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let full = client
        .get("/api/1/Users?$orderby=id%20asc")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .expect("valid JSON");
    let full_ids: Vec<i64> = full["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["id"].as_i64().unwrap())
        .collect();
    assert!(full_ids.len() >= 4, "need at least 4 users for pagination test");

    let page = client
        .get("/api/1/Users?$orderby=id%20asc&$skip=2&$top=2")
        .cookie(admin_cookie)
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .expect("valid JSON");
    let page_ids: Vec<i64> = page["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["id"].as_i64().unwrap())
        .collect();

    assert_eq!(page_ids.len(), 2, "$top=2 must cap the page at 2 items");
    assert_eq!(page_ids, full_ids[2..4], "$skip=2&$top=2 must return items 2..4");
}

#[rocket::async_test]
async fn test_users_count_reflects_total_before_paging() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let unpaged = client
        .get("/api/1/Users")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await
        .into_json::<Value>()
        .await
        .expect("valid JSON");
    let total = unpaged["value"].as_array().unwrap().len() as i64;
    assert!(total >= 2);

    let resp = client
        .get("/api/1/Users?$top=1&$count=true")
        .cookie(admin_cookie)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("valid OData JSON");

    assert_eq!(body["value"].as_array().unwrap().len(), 1, "$top=1 limits page size");
    assert_eq!(
        body["@odata.count"].as_i64(),
        Some(total),
        "@odata.count must equal pre-paging total when $count=true"
    );
}

#[rocket::async_test]
async fn test_users_count_omitted_when_not_requested() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let body: Value = client
        .get("/api/1/Users?$top=1")
        .cookie(admin_cookie)
        .dispatch()
        .await
        .into_json()
        .await
        .expect("valid OData JSON");

    assert!(
        body.get("@odata.count").is_none(),
        "@odata.count must be omitted when $count is not requested"
    );
}
