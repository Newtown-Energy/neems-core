use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_api::models::{Company, Site, UserWithRoles};
use neems_api::orm::testing::test_rocket;

/// Helper to create a user and return login credentials
async fn create_user_with_role(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    email: &str,
    company_id: i32,
    role_name: &str,
) -> (String, String) {
    // Create user with properly hashed password
    let password_hash = neems_api::orm::login::hash_password("admin");
    let new_user = json!({
        "email": email,
        "password_hash": password_hash,
        "company_id": company_id,
        "totp_secret": "",
        "role_names": [role_name]
    });

    let response = client
        .post("/api/1/users")
        .cookie(admin_cookie.clone())
        .json(&new_user)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let _created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");

    // Role is already assigned during user creation, no need for separate assignment

    (email.to_string(), "admin".to_string()) // Use default password
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

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
    login_user(client, "superadmin@example.com", "admin").await
}

/// Helper to create a company
async fn create_company(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    name: &str,
) -> Company {
    let new_comp = json!({"name": name});

    let response = client
        .post("/api/1/companies")
        .cookie(admin_cookie.clone())
        .json(&new_comp)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    response.into_json().await.expect("valid company JSON")
}

/// Helper to create sites using the site CRUD API
async fn setup_sites_for_company(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    company_id: i32,
    count: usize,
) -> Vec<Site> {
    let mut sites = Vec::new();

    for i in 0..count {
        let new_site = json!({
            "name": format!("Site {}", i + 1),
            "address": format!("{} Main St, City, State", (i + 1) * 100),
            "latitude": 40.7128 + (i as f64 * 0.01),
            "longitude": -74.0060 + (i as f64 * 0.01),
            "company_id": company_id
        });

        let response = client
            .post("/api/1/sites")
            .cookie(admin_cookie.clone())
            .json(&new_site)
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Created);
        let site: Site = response.into_json().await.expect("valid site JSON");
        sites.push(site);
    }

    sites
}

#[rocket::async_test]
async fn test_sites_endpoint_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/company/1/sites").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_company_users_can_access_own_company_sites() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a company
    let company = create_company(&client, &admin_cookie, "Test Company").await;

    // Note: We're testing with an empty company (no sites) to focus on authorization logic
    // The endpoint should return an empty array for a company with no sites

    // Create a user for this company
    let (_email, password) = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        company.id,
        "admin",
    )
    .await;

    // Login as company user
    let user_cookie = login_user(&client, "user@testcompany.com", &password).await;

    // Test that company user can access their own company's sites
    let url = format!("/api/1/company/{}/sites", company.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 0); // No sites created, so should be empty

    // The test verifies that:
    // 1. Company user can access their company's sites endpoint (returns 200)
    // 2. The response is valid JSON in the correct format (empty array)
}

#[rocket::async_test]
async fn test_users_cannot_access_different_company_sites() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Note: Testing authorization without actually creating sites

    // Create user for company1
    let (_email, password) = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company1.com",
        company1.id,
        "admin",
    )
    .await;

    // Login as company1 user
    let user_cookie = login_user(&client, "user@company1.com", &password).await;

    // Create sites for company1 to test access
    let _sites = setup_sites_for_company(&client, &admin_cookie, company1.id, 2).await;

    // User from company1 should be able to access company1 sites
    let url = format!("/api/1/company/{}/sites", company1.id);
    let response = client
        .get(&url)
        .cookie(user_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 2);

    // User from company1 should NOT be able to access company2 sites
    let url = format!("/api/1/company/{}/sites", company2.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_admin_can_access_any_company_sites() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Create sites for both companies
    let _sites1 = setup_sites_for_company(&client, &admin_cookie, company1.id, 2).await;
    let _sites2 = setup_sites_for_company(&client, &admin_cookie, company2.id, 3).await;

    // Create newtown-admin user (use existing Newtown Energy company)
    // Get Newtown Energy company (created by admin init fairing)
    let companies_response = client
        .get("/api/1/companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(companies_response.status(), Status::Ok);
    let companies: Vec<Company> = companies_response
        .into_json()
        .await
        .expect("valid companies JSON");
    let newtown_company = companies
        .iter()
        .find(|c| c.name == "Newtown Energy")
        .expect("Newtown Energy company should exist");

    let (_email, password) = create_user_with_role(
        &client,
        &admin_cookie,
        "newtownadmin@newtown.com",
        newtown_company.id,
        "newtown-admin",
    )
    .await;

    // Login as newtown-admin
    let newtown_admin_cookie = login_user(&client, "newtownadmin@newtown.com", &password).await;

    // Newtown admin should be able to access any company's sites
    let url = format!("/api/1/company/{}/sites", company1.id);
    let response = client
        .get(&url)
        .cookie(newtown_admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 2);

    let url = format!("/api/1/company/{}/sites", company2.id);
    let response = client
        .get(&url)
        .cookie(newtown_admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 3);
}

#[rocket::async_test]
async fn test_newtown_staff_can_access_any_company_sites() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create company
    let company = create_company(&client, &admin_cookie, "Test Company").await;

    // Create sites for the company
    let _sites = setup_sites_for_company(&client, &admin_cookie, company.id, 2).await;

    // Create newtown-staff user (use existing Newtown Energy company)
    // Get Newtown Energy company (created by admin init fairing)
    let companies_response = client
        .get("/api/1/companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(companies_response.status(), Status::Ok);
    let companies: Vec<Company> = companies_response
        .into_json()
        .await
        .expect("valid companies JSON");
    let newtown_company = companies
        .iter()
        .find(|c| c.name == "Newtown Energy")
        .expect("Newtown Energy company should exist");

    let (_email, password) = create_user_with_role(
        &client,
        &admin_cookie,
        "newtownstaff@newtown.com",
        newtown_company.id,
        "newtown-staff",
    )
    .await;

    // Login as newtown-staff
    let newtown_staff_cookie = login_user(&client, "newtownstaff@newtown.com", &password).await;

    // Newtown staff should be able to access any company's sites
    let url = format!("/api/1/company/{}/sites", company.id);
    let response = client
        .get(&url)
        .cookie(newtown_staff_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 2);
}

#[rocket::async_test]
async fn test_sites_response_format() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create company
    let company = create_company(&client, &admin_cookie, "Test Company").await;

    // Create sites for the company
    let _sites = setup_sites_for_company(&client, &admin_cookie, company.id, 1).await;

    // Create user for this company
    let (_email, password) = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        company.id,
        "admin",
    )
    .await;

    // Login as company user
    let user_cookie = login_user(&client, "user@testcompany.com", &password).await;

    // Get sites
    let url = format!("/api/1/company/{}/sites", company.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 1);

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
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Login as admin (who should have access to any company)
    let response = client
        .get("/api/1/company/99999/sites")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    // Should return OK with empty array for non-existent company
    assert_eq!(response.status(), Status::Ok);

    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 0);
}

#[rocket::async_test]
async fn test_empty_sites_for_existing_company() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create company but no sites
    let company = create_company(&client, &admin_cookie, "Empty Company").await;

    // Create user for this company
    let (_email, password) = create_user_with_role(
        &client,
        &admin_cookie,
        "user@empty.com",
        company.id,
        "admin",
    )
    .await;

    // Login as company user
    let user_cookie = login_user(&client, "user@empty.com", &password).await;

    // Get sites for company with no sites
    let url = format!("/api/1/company/{}/sites", company.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let sites: Vec<Site> = response.into_json().await.expect("valid sites JSON");
    assert_eq!(sites.len(), 0);
}
