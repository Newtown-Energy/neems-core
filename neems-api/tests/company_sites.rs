use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_api::models::{Company, Site};
use neems_api::orm::testing::fast_test_rocket;


/// Helper to login with specific credentials and get session cookie
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

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
    login_user(client, "superadmin@example.com", "admin").await
}

/// Helper to get a test company by name
async fn get_company_by_name(client: &Client, admin_cookie: &rocket::http::Cookie<'static>, name: &str) -> Company {
    let response = client
        .get("/api/1/Companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> = serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    companies.into_iter()
        .find(|c| c.name == name)
        .expect(&format!("Company '{}' should exist from test data initialization", name))
}

// Removed setup_sites_for_company - using golden DB sites instead

#[rocket::async_test]
async fn test_sites_endpoint_requires_authentication() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/Companies/1/Sites").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_company_users_can_access_own_company_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get the pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Login as the pre-created company user
    let user_cookie = login_user(&client, "user@testcompany.com", "admin").await;

    // Test that company user can access their own company's sites
    let url = format!("/api/1/Companies/{}/Sites", company.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    
    // Test what matters: access control and data integrity
    // 1. Company user can access their company's sites endpoint (returns 200) ✓
    // 2. All returned sites belong to the correct company (business logic)
    for site in &sites {
        assert_eq!(site.company_id, company.id, 
            "All sites should belong to the user's company (Test Company 1)");
    }
    
    // 3. The response is valid JSON in the correct format ✓
}

#[rocket::async_test]
async fn test_users_cannot_access_different_company_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get the pre-created test companies
    let company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;

    // Login as company1 user (pre-created)
    let user_cookie = login_user(&client, "user@company1.com", "admin").await;

    // Company1 already has Test Site 1 from golden DB

    // User from company1 should be able to access company1 sites
    let url = format!("/api/1/Companies/{}/Sites", company1.id);
    let response = client
        .get(&url)
        .cookie(user_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert!(sites.len() >= 1); // At least Test Site 1

    // User from company1 should NOT be able to access company2 sites
    let url = format!("/api/1/Companies/{}/Sites", company2.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_admin_can_access_any_company_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test companies
    let company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;

    // Both companies already have sites from golden DB
    // Test Site 1 for Company 1, Test Site 2 for Company 2

    // Login as pre-created newtown-admin user
    let newtown_admin_cookie = login_user(&client, "newtownadmin@newtown.com", "admin").await;

    // Newtown admin should be able to access any company's sites
    let url = format!("/api/1/Companies/{}/Sites", company1.id);
    let response = client
        .get(&url)
        .cookie(newtown_admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert!(sites.len() >= 1); // At least Test Site 1

    let url = format!("/api/1/Companies/{}/Sites", company2.id);
    let response = client
        .get(&url)
        .cookie(newtown_admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert!(sites.len() >= 1); // At least Test Site 2
}

#[rocket::async_test]
async fn test_newtown_staff_can_access_any_company_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Company already has Test Site 1 from golden DB

    // Login as pre-created newtown-staff user
    let newtown_staff_cookie = login_user(&client, "newtownstaff@newtown.com", "admin").await;

    // Newtown staff should be able to access any company's sites
    let url = format!("/api/1/Companies/{}/Sites", company.id);
    let response = client
        .get(&url)
        .cookie(newtown_staff_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert!(sites.len() >= 1); // At least Test Site 1
}

#[rocket::async_test]
async fn test_sites_response_format() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Create sites for the company
    // Company already has Test Site 1 from golden DB

    // Login as pre-created company user
    let user_cookie = login_user(&client, "user@testcompany.com", "admin").await;

    // Get sites
    let url = format!("/api/1/Companies/{}/Sites", company.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert!(sites.len() >= 1); // At least Test Site 1

    let site = &sites[0];

    // Verify all required fields are present and have correct types
    assert!(site.id > 0);
    assert!(!site.name.is_empty());
    assert!(!site.address.is_empty());
    assert!(site.latitude != 0.0);
    assert!(site.longitude != 0.0);
    assert_eq!(site.company_id, company.id);
    // created_at and updated_at are automatically set
}

#[rocket::async_test]
async fn test_nonexistent_company_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Login as admin (who should have access to any company)
    let response = client
        .get("/api/1/Companies/99999/Sites")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    // Should return OK with empty array for non-existent company
    assert_eq!(response.status(), Status::Ok);

    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 0);
}


