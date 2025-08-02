use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::models::{Company, UserWithRoles};
use neems_core::orm::testing::test_rocket;

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

/// Helper to create a user and assign role
async fn create_user_with_role(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    email: &str,
    company_id: i32,
    role_name: &str,
) -> UserWithRoles {
    // Create user with properly hashed password
    let password_hash = neems_core::orm::login::hash_password("admin");
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
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");

    // Role is already assigned during user creation, no need for separate assignment

    created_user
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
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/company/1/users").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_company_users_can_access_own_company_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a company
    let company = create_company(&client, &admin_cookie, "Test Company").await;

    // Create multiple users for this company
    let _user1 = create_user_with_role(
        &client,
        &admin_cookie,
        "user1@testcompany.com",
        company.id,
        "staff",
    )
    .await;
    let _user2 = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@testcompany.com",
        company.id,
        "admin",
    )
    .await;

    // Login as company admin
    let admin_session = login_user(&client, "admin@testcompany.com", "admin").await;

    // Test that company admin can access their own company's users
    let url = format!("/api/1/company/{}/users", company.id);
    let response = client.get(&url).cookie(admin_session).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 2); // Should see both users

    // Verify the users belong to the correct company
    for user in &users {
        assert_eq!(user.company_id, company.id);
    }

    // Check that we got the expected users
    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"user1@testcompany.com".to_string()));
    assert!(emails.contains(&&"admin@testcompany.com".to_string()));
}

#[rocket::async_test]
async fn test_users_cannot_access_different_company_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Create users for each company
    let _user1_company1 = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@company1.com",
        company1.id,
        "admin",
    )
    .await;
    let _user1_company2 = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@company2.com",
        company2.id,
        "admin",
    )
    .await;

    // Login as company1 admin
    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // User from company1 should be able to access company1 users
    let url = format!("/api/1/company/{}/users", company1.id);
    let response = client
        .get(&url)
        .cookie(admin1_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].email, "admin@company1.com");

    // User from company1 should NOT be able to access company2 users
    let url = format!("/api/1/company/{}/users", company2.id);
    let response = client.get(&url).cookie(admin1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_admin_can_access_any_company_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a company and users
    let company = create_company(&client, &admin_cookie, "Test Company").await;
    let _user1 = create_user_with_role(
        &client,
        &admin_cookie,
        "user1@testcompany.com",
        company.id,
        "staff",
    )
    .await;
    let _user2 = create_user_with_role(
        &client,
        &admin_cookie,
        "user2@testcompany.com",
        company.id,
        "admin",
    )
    .await;

    // Newtown admin should be able to access any company's users
    let url = format!("/api/1/company/{}/users", company.id);
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 2);

    // Verify users belong to the company
    for user in &users {
        assert_eq!(user.company_id, company.id);
    }
}

#[rocket::async_test]
async fn test_newtown_staff_can_access_any_company_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and users
    let company = create_company(&client, &admin_cookie, "Test Company").await;
    let _user1 = create_user_with_role(
        &client,
        &admin_cookie,
        "user1@testcompany.com",
        company.id,
        "staff",
    )
    .await;

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

    // Create newtown-staff user
    let _newtown_staff = create_user_with_role(
        &client,
        &admin_cookie,
        "newtownstaff@newtown.com",
        newtown_company.id,
        "newtown-staff",
    )
    .await;
    let staff_session = login_user(&client, "newtownstaff@newtown.com", "admin").await;

    // Newtown staff should be able to access any company's users
    let url = format!("/api/1/company/{}/users", company.id);
    let response = client.get(&url).cookie(staff_session).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].email, "user1@testcompany.com");
}

#[rocket::async_test]
async fn test_users_response_format() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create company and user
    let company = create_company(&client, &admin_cookie, "Test Company").await;
    let _user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        company.id,
        "admin",
    )
    .await;

    // Login as company user
    let user_cookie = login_user(&client, "user@testcompany.com", "admin").await;

    // Get users
    let url = format!("/api/1/company/{}/users", company.id);
    let response = client.get(&url).cookie(user_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 1);

    let user = &users[0];

    // Verify all required fields are present and have correct types
    assert!(user.id > 0);
    assert!(!user.email.is_empty());
    assert!(!user.password_hash.is_empty());
    assert_eq!(user.company_id, company.id);
    // created_at and updated_at are automatically set
}

#[rocket::async_test]
async fn test_nonexistent_company_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Login as admin (who should have access to any company)
    let response = client
        .get("/api/1/company/99999/users")
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
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create company but no users
    let company = create_company(&client, &admin_cookie, "Empty Company").await;

    // Get users for company with no users (using admin access)
    let url = format!("/api/1/company/{}/users", company.id);
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");
    assert_eq!(users.len(), 0);
}
