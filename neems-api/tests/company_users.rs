use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_api::models::{Company, UserWithRoles};
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
async fn test_users_endpoint_requires_authentication() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/Companies/1/Users").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_company_users_can_access_own_company_users() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Login as pre-created company admin (user@testcompany.com has admin role)
    let admin_session = login_user(&client, "user@testcompany.com", "admin").await;

    // Test that company admin can access their own company's users
    let url = format!("/api/1/Companies/{}/Users", company.id);
    let response = client.get(&url).cookie(admin_session).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert!(!users.is_empty(), "Should see some users for Test Company 1");

    // Verify the users belong to the correct company
    for user in &users {
        assert_eq!(user.company_id, company.id, "All users should belong to the correct company");
    }

    // Check that we got the expected users
    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"user@testcompany.com".to_string()));
    assert!(emails.contains(&&"user@company1.com".to_string()));
    assert!(emails.contains(&&"user@empty.com".to_string()));
}

#[rocket::async_test]
async fn test_users_cannot_access_different_company_users() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test companies
    let company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;

    // Login as pre-created company1 admin
    let admin1_session = login_user(&client, "user@company1.com", "admin").await;

    // User from company1 should be able to access company1 users
    let url = format!("/api/1/Companies/{}/Users", company1.id);
    let response = client
        .get(&url)
        .cookie(admin1_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert!(!users.is_empty(), "Should return at least some users for company1");
    
    // Verify all returned users belong to company1
    for user in &users {
        assert_eq!(user.company_id, company1.id, "All users should belong to company1");
    }
    
    // Check that we got some expected users from Test Company 1 (but don't require all)
    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"user@testcompany.com".to_string()), "Should contain user@testcompany.com");
    assert!(emails.contains(&&"user@company1.com".to_string()), "Should contain user@company1.com");

    // User from company1 should NOT be able to access company2 users
    let url = format!("/api/1/Companies/{}/Users", company2.id);
    let response = client.get(&url).cookie(admin1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_admin_can_access_any_company_users() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Newtown admin should be able to access any company's users
    let url = format!("/api/1/Companies/{}/Users", company.id);
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert!(!users.is_empty(), "Should see some users for Test Company 1");

    // Verify users belong to the company
    for user in &users {
        assert_eq!(user.company_id, company.id, "All users should belong to the correct company");
    }
}

#[rocket::async_test]
async fn test_newtown_staff_can_access_any_company_users() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Login as pre-created newtown-staff user
    let staff_session = login_user(&client, "newtownstaff@newtown.com", "admin").await;

    // Newtown staff should be able to access any company's users
    let url = format!("/api/1/Companies/{}/Users", company.id);
    let response = client.get(&url).cookie(staff_session).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert!(!users.is_empty(), "Test Company 1 should have some users");
    
    // Verify all users belong to the correct company
    for user in &users {
        assert_eq!(user.company_id, company.id, "All users should belong to Test Company 1");
    }
    
    // Check that we got some expected users from Test Company 1
    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"user@testcompany.com".to_string()), "Should contain user@testcompany.com");
}

#[rocket::async_test]
async fn test_users_response_format() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company and login as pre-created user
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let user_cookie = login_user(&client, "user@testcompany.com", "admin").await;

    // Get users
    let url = format!("/api/1/Companies/{}/Users", company.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert!(!users.is_empty(), "Test Company 1 should have some users");

    // Verify all users have required fields and belong to correct company
    for user in &users {
        assert!(user.id > 0, "User ID should be positive");
        assert!(!user.email.is_empty(), "User email should not be empty");
        assert!(!user.password_hash.is_empty(), "Password hash should not be empty");
        assert_eq!(user.company_id, company.id, "All users should belong to the correct company");
    }
    
    // Verify we have some expected users (business logic test)
    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"user@testcompany.com".to_string()), "Should contain user@testcompany.com");
    // created_at and updated_at are automatically set
}

#[rocket::async_test]
async fn test_nonexistent_company_users() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Login as admin (who should have access to any company)
    let response = client
        .get("/api/1/Companies/99999/Users")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    // Should return OK with empty array for non-existent company
    assert_eq!(response.status(), Status::Ok);

    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 0);
}

#[rocket::async_test]
async fn test_empty_users_for_existing_company() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get a pre-created test company (Test Company 2 has user@company2.com but for this test we'll use a company without users)
    // Actually, let's create a new company for this specific test case since we need an empty one
    let new_company = json!({"name": "Empty Company"});
    let response = client
        .post("/api/1/Companies")
        .cookie(admin_cookie.clone())
        .json(&new_company)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    let company: Company = response.into_json().await.expect("valid company JSON");

    // Get users for company with no users (using admin access)
    let url = format!("/api/1/Companies/{}/Users", company.id);
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 0);
}
