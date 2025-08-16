use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_api::models::{Company, Site};
use neems_api::orm::testing::fast_test_rocket;

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

#[rocket::async_test]
async fn test_site_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test all endpoints require authentication
    let response = client.get("/api/1/Sites").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/Sites/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let new_site = json!({
        "name": "Test Site",
        "address": "123 Test St",
        "latitude": 40.7128,
        "longitude": -74.0060,
        "company_id": 1
    });

    let response = client.post("/api/1/Sites").json(&new_site).dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let update_site = json!({
        "name": "Updated Site"
    });

    let response = client
        .put("/api/1/Sites/1")
        .json(&update_site)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.delete("/api/1/Sites/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_company_admin_can_crud_own_company_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Login as pre-created company admin (user@testcompany.com has admin role)
    let admin_session = login_user(&client, "user@testcompany.com", "admin").await;

    // Get the pre-created Test Site 1 from golden DB
    let response = client
        .get("/api/1/Sites")
        .cookie(admin_session.clone())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> = serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    
    // Find Test Site 1 (belongs to Test Company 1)
    let test_site = sites.iter()
        .find(|s| s.name == "Test Site 1")
        .expect("Test Site 1 should exist in golden DB");
    
    assert_eq!(test_site.company_id, company.id);

    // Read the site
    let url = format!("/api/1/Sites/{}", test_site.id);
    let response = client
        .get(&url)
        .cookie(admin_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_site: Site = response.into_json().await.expect("valid site JSON");
    assert_eq!(retrieved_site.id, test_site.id);

    // Update the site (modifying golden DB is fine - next test gets fresh copy)
    let update_data = json!({
        "name": "Updated Test Site 1",
        "address": "456 Updated St"
    });

    let response = client
        .put(&url)
        .cookie(admin_session.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let updated_site: Site = response.into_json().await.expect("valid site JSON");
    assert_eq!(updated_site.name, "Updated Test Site 1");
    assert_eq!(updated_site.address, "456 Updated St");

    // Delete the site (it's ok - next test gets fresh golden DB)
    let response = client.delete(&url).cookie(admin_session).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify site is deleted
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_company_admin_cannot_access_different_company_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test companies
    let _company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;

    // Login as pre-created company1 admin
    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Admin tries to create site for company2 (should fail)
    let new_site = json!({
        "name": "Forbidden Site",
        "address": "123 Forbidden St",
        "latitude": 40.7128,
        "longitude": -74.0060,
        "company_id": company2.id
    });

    let response = client
        .post("/api/1/Sites")
        .cookie(admin1_session.clone())
        .json(&new_site)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);

    // Get Test Site 2 from golden DB (belongs to Test Company 2)
    let response = client
        .get("/api/1/Sites")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> = serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    
    let company2_site = sites.iter()
        .find(|s| s.name == "Test Site 2" && s.company_id == company2.id)
        .expect("Test Site 2 should exist in golden DB");

    // Company1 admin should not be able to read company2's site (Test Site 2)
    let url = format!("/api/1/Sites/{}", company2_site.id);
    let response = client
        .get(&url)
        .cookie(admin1_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);

    // Company1 admin should not be able to update company2's site
    let update_data = json!({
        "name": "Hacked Site"
    });

    let response = client
        .put(&url)
        .cookie(admin1_session.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);

    // Company1 admin should not be able to delete company2's site
    let response = client.delete(&url).cookie(admin1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_admin_can_crud_any_site() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get all sites from golden DB
    let response = client
        .get("/api/1/Sites")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> = serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    
    // Use Test Site 1 for testing (any site works for newtown admin)
    let test_site = sites.iter()
        .find(|s| s.name == "Test Site 1")
        .expect("Test Site 1 should exist in golden DB");

    // Newtown admin can read any site
    let url = format!("/api/1/Sites/{}", test_site.id);
    let response = client
        .get(&url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Newtown admin can update any site
    let update_data = json!({
        "name": "Newtown Updated Site"
    });

    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let updated_site: Site = response.into_json().await.expect("valid site JSON");
    assert_eq!(updated_site.name, "Newtown Updated Site");

    // Newtown admin can see all sites
    let response = client
        .get("/api/1/Sites")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let all_sites: Vec<Site> = serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    assert!(!all_sites.is_empty());

    // Newtown admin can delete any site
    let response = client.delete(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);
}

#[rocket::async_test]
async fn test_regular_user_cannot_crud_sites() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Login as pre-created staff user (has staff role, not admin)
    let user_session = login_user(&client, "staff@testcompany.com", "admin").await;

    // Regular user cannot list sites
    let response = client
        .get("/api/1/Sites")
        .cookie(user_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);

    // Regular user cannot create sites
    let new_site = json!({
        "name": "User Site",
        "address": "123 User St",
        "latitude": 40.7128,
        "longitude": -74.0060,
        "company_id": company.id
    });

    let response = client
        .post("/api/1/Sites")
        .cookie(user_session)
        .json(&new_site)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}
