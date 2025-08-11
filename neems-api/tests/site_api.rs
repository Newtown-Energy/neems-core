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
        .get("/api/1/companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let companies: Vec<Company> = response.into_json().await.expect("valid companies JSON");
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
    let response = client.get("/api/1/sites").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/sites/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let new_site = json!({
        "name": "Test Site",
        "address": "123 Test St",
        "latitude": 40.7128,
        "longitude": -74.0060,
        "company_id": 1
    });

    let response = client.post("/api/1/sites").json(&new_site).dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let update_site = json!({
        "name": "Updated Site"
    });

    let response = client
        .put("/api/1/sites/1")
        .json(&update_site)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.delete("/api/1/sites/1").dispatch().await;
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

    // Create a site
    let new_site = json!({
        "name": "Company Site",
        "address": "123 Company St",
        "latitude": 40.7128,
        "longitude": -74.0060,
        "company_id": company.id
    });

    let response = client
        .post("/api/1/sites")
        .cookie(admin_session.clone())
        .json(&new_site)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_site: Site = response.into_json().await.expect("valid site JSON");

    assert_eq!(created_site.name, "Company Site");
    assert_eq!(created_site.company_id, company.id);

    // Read the site
    let url = format!("/api/1/sites/{}", created_site.id);
    let response = client
        .get(&url)
        .cookie(admin_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_site: Site = response.into_json().await.expect("valid site JSON");
    assert_eq!(retrieved_site.id, created_site.id);

    // Update the site
    let update_data = json!({
        "name": "Updated Company Site",
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
    assert_eq!(updated_site.name, "Updated Company Site");
    assert_eq!(updated_site.address, "456 Updated St");

    // List sites (should see their company's site)
    let response = client
        .get("/api/1/sites")
        .cookie(admin_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].id, created_site.id);

    // Delete the site
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
        .post("/api/1/sites")
        .cookie(admin1_session.clone())
        .json(&new_site)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);

    // Create a site for company2 using super admin
    let response = client
        .post("/api/1/sites")
        .cookie(admin_cookie.clone())
        .json(&new_site)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let company2_site: Site = response.into_json().await.expect("valid site JSON");

    // Company1 admin should not be able to read company2's site
    let url = format!("/api/1/sites/{}", company2_site.id);
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

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Create a site using super admin (who has newtown-admin role)
    let new_site = json!({
        "name": "Admin Site",
        "address": "123 Admin St",
        "latitude": 40.7128,
        "longitude": -74.0060,
        "company_id": company.id
    });

    let response = client
        .post("/api/1/sites")
        .cookie(admin_cookie.clone())
        .json(&new_site)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_site: Site = response.into_json().await.expect("valid site JSON");

    // Newtown admin can read any site
    let url = format!("/api/1/sites/{}", created_site.id);
    let response = client
        .get(&url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Newtown admin can update any site
    let update_data = json!({
        "name": "Updated Admin Site"
    });

    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Newtown admin can see all sites
    let response = client
        .get("/api/1/sites")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert!(!sites.is_empty());

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
        .get("/api/1/sites")
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
        .post("/api/1/sites")
        .cookie(user_session)
        .json(&new_site)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}
