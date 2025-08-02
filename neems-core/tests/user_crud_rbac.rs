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

// CREATE USER RBAC TESTS

#[rocket::async_test]
async fn test_create_user_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    let new_user = json!({
        "email": "test@example.com",
        "password_hash": "hash",
        "company_id": 1,
        "totp_secret": "",
        "role_names": ["staff"]
    });

    let response = client.post("/api/1/users").json(&new_user).dispatch().await;

    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_regular_users_cannot_create_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and regular user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let _regular_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user_session = login_user(&client, "user@testcompany.com", "admin").await;

    let new_user = json!({
        "email": "shouldnotwork@example.com",
        "password_hash": "hash",
        "company_id": test_company.id,
        "totp_secret": "",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/users")
        .cookie(user_session)
        .json(&new_user)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_admin_can_create_users_for_own_company_only() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Create admin for company1
    let _company1_admin = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@company1.com",
        company1.id,
        "admin",
    )
    .await;
    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Should be able to create user for own company
    let new_user_own_company = json!({
        "email": "newuser@company1.com",
        "password_hash": neems_core::orm::login::hash_password("password"),
        "company_id": company1.id,
        "totp_secret": "",
        "role_names": ["admin"]
    });

    let response = client
        .post("/api/1/users")
        .cookie(admin1_session.clone())
        .json(&new_user_own_company)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(created_user.company_id, company1.id);

    // Should NOT be able to create user for different company
    let new_user_other_company = json!({
        "email": "unauthorized@company2.com",
        "password_hash": neems_core::orm::login::hash_password("password"),
        "company_id": company2.id,
        "totp_secret": "",
        "role_names": ["admin"]
    });

    let response = client
        .post("/api/1/users")
        .cookie(admin1_session)
        .json(&new_user_other_company)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_staff_can_create_users_for_any_company() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get Newtown Energy company
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

    // Create a test company
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;

    // Create newtown-staff user
    let _newtown_staff = create_user_with_role(
        &client,
        &admin_cookie,
        "staff@newtown.com",
        newtown_company.id,
        "newtown-staff",
    )
    .await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;

    // Should be able to create user for any company
    let new_user = json!({
        "email": "newuser@testcompany.com",
        "password_hash": neems_core::orm::login::hash_password("password"),
        "company_id": test_company.id,
        "totp_secret": "",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/users")
        .cookie(staff_session)
        .json(&new_user)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(created_user.company_id, test_company.id);
}

#[rocket::async_test]
async fn test_newtown_admin_can_create_users_for_any_company() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;

    // newtown-admin (superadmin@example.com) should be able to create user for any company
    let new_user = json!({
        "email": "newuser@testcompany.com",
        "password_hash": neems_core::orm::login::hash_password("password"),
        "company_id": test_company.id,
        "totp_secret": "",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/users")
        .cookie(admin_cookie)
        .json(&new_user)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(created_user.company_id, test_company.id);
}

// LIST USERS RBAC TESTS

#[rocket::async_test]
async fn test_list_users_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    let response = client.get("/api/1/users").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_regular_users_cannot_list_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and regular user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let _regular_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user_session = login_user(&client, "user@testcompany.com", "admin").await;

    let response = client
        .get("/api/1/users")
        .cookie(user_session)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_admin_can_list_users_from_own_company_only() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Create users for both companies
    let _company1_admin = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@company1.com",
        company1.id,
        "admin",
    )
    .await;
    let _company1_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company1.com",
        company1.id,
        "staff",
    )
    .await;
    let _company2_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company2.com",
        company2.id,
        "staff",
    )
    .await;

    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Admin should only see users from their own company
    let response = client
        .get("/api/1/users")
        .cookie(admin1_session)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");

    // Should see exactly 2 users (admin and user from company1)
    assert_eq!(users.len(), 2);
    for user in &users {
        assert_eq!(user.company_id, company1.id);
    }

    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"admin@company1.com".to_string()));
    assert!(emails.contains(&&"user@company1.com".to_string()));
    assert!(!emails.contains(&&"user@company2.com".to_string()));
}

#[rocket::async_test]
async fn test_newtown_staff_can_list_all_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get Newtown Energy company
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

    // Create test company and users
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let _test_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;

    // Create newtown-staff user
    let _newtown_staff = create_user_with_role(
        &client,
        &admin_cookie,
        "staff@newtown.com",
        newtown_company.id,
        "newtown-staff",
    )
    .await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;

    // Should be able to see all users
    let response = client
        .get("/api/1/users")
        .cookie(staff_session)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");

    // Should see users from multiple companies (at least 3: superadmin, staff, test_user)
    assert!(users.len() >= 3);

    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"superadmin@example.com".to_string()));
    assert!(emails.contains(&&"staff@newtown.com".to_string()));
    assert!(emails.contains(&&"user@testcompany.com".to_string()));
}

#[rocket::async_test]
async fn test_newtown_admin_can_list_all_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create test company and user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let _test_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;

    // newtown-admin should be able to see all users
    let response = client
        .get("/api/1/users")
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let users: Vec<UserWithRoles> = response.into_json().await.expect("valid users JSON");

    // Should see users from multiple companies (at least 2: superadmin, test_user)
    assert!(users.len() >= 2);

    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"superadmin@example.com".to_string()));
    assert!(emails.contains(&&"user@testcompany.com".to_string()));
}

// GET USER RBAC TESTS

#[rocket::async_test]
async fn test_get_user_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    let response = client.get("/api/1/users/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_users_can_view_own_profile() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let test_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user_session = login_user(&client, "user@testcompany.com", "admin").await;

    // User should be able to view their own profile
    let url = format!("/api/1/users/{}", test_user.id);
    let response = client.get(&url).cookie(user_session).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(retrieved_user.id, test_user.id);
    assert_eq!(retrieved_user.email, "user@testcompany.com");
}

#[rocket::async_test]
async fn test_users_cannot_view_other_users_profiles() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and two users
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let _user1 = create_user_with_role(
        &client,
        &admin_cookie,
        "user1@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user2 = create_user_with_role(
        &client,
        &admin_cookie,
        "user2@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user1_session = login_user(&client, "user1@testcompany.com", "admin").await;

    // User1 should NOT be able to view user2's profile
    let url = format!("/api/1/users/{}", user2.id);
    let response = client.get(&url).cookie(user1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_admin_can_view_users_from_own_company_only() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Create users for both companies
    let _company1_admin = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@company1.com",
        company1.id,
        "admin",
    )
    .await;
    let company1_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company1.com",
        company1.id,
        "staff",
    )
    .await;
    let company2_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company2.com",
        company2.id,
        "staff",
    )
    .await;

    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Admin should be able to view users from own company
    let url = format!("/api/1/users/{}", company1_user.id);
    let response = client
        .get(&url)
        .cookie(admin1_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(retrieved_user.id, company1_user.id);

    // Admin should NOT be able to view users from different company
    let url = format!("/api/1/users/{}", company2_user.id);
    let response = client.get(&url).cookie(admin1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_staff_can_view_any_user() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get Newtown Energy company
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

    // Create test company and user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let test_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;

    // Create newtown-staff user
    let _newtown_staff = create_user_with_role(
        &client,
        &admin_cookie,
        "staff@newtown.com",
        newtown_company.id,
        "newtown-staff",
    )
    .await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;

    // Should be able to view any user
    let url = format!("/api/1/users/{}", test_user.id);
    let response = client.get(&url).cookie(staff_session).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(retrieved_user.id, test_user.id);
}

// UPDATE USER RBAC TESTS

#[rocket::async_test]
async fn test_update_user_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    let update_request = json!({
        "email": "newemail@example.com"
    });

    let response = client
        .put("/api/1/users/1")
        .json(&update_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_users_can_update_own_profile() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let test_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user_session = login_user(&client, "user@testcompany.com", "admin").await;

    // User should be able to update their own profile
    let update_request = json!({
        "email": "newuser@testcompany.com"
    });

    let url = format!("/api/1/users/{}", test_user.id);
    let response = client
        .put(&url)
        .cookie(user_session)
        .json(&update_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let updated_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(updated_user.email, "newuser@testcompany.com");
}

#[rocket::async_test]
async fn test_users_cannot_update_other_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and two users
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let _user1 = create_user_with_role(
        &client,
        &admin_cookie,
        "user1@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user2 = create_user_with_role(
        &client,
        &admin_cookie,
        "user2@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user1_session = login_user(&client, "user1@testcompany.com", "admin").await;

    let update_request = json!({
        "email": "hacked@testcompany.com"
    });

    // User1 should NOT be able to update user2
    let url = format!("/api/1/users/{}", user2.id);
    let response = client
        .put(&url)
        .cookie(user1_session)
        .json(&update_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_admin_can_update_users_from_own_company_only() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Create users for both companies
    let _company1_admin = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@company1.com",
        company1.id,
        "admin",
    )
    .await;
    let company1_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company1.com",
        company1.id,
        "staff",
    )
    .await;
    let company2_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company2.com",
        company2.id,
        "staff",
    )
    .await;

    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    let update_request = json!({
        "email": "updated@company.com"
    });

    // Admin should be able to update users from own company
    let url = format!("/api/1/users/{}", company1_user.id);
    let response = client
        .put(&url)
        .cookie(admin1_session.clone())
        .json(&update_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let updated_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(updated_user.email, "updated@company.com");

    // Admin should NOT be able to update users from different company
    let url = format!("/api/1/users/{}", company2_user.id);
    let response = client
        .put(&url)
        .cookie(admin1_session)
        .json(&update_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

// DELETE USER RBAC TESTS

#[rocket::async_test]
async fn test_delete_user_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    let response = client.delete("/api/1/users/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_regular_users_cannot_delete_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a test company and users
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let _user1 = create_user_with_role(
        &client,
        &admin_cookie,
        "user1@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user2 = create_user_with_role(
        &client,
        &admin_cookie,
        "user2@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;
    let user1_session = login_user(&client, "user1@testcompany.com", "admin").await;

    // Regular user should NOT be able to delete anyone
    let url = format!("/api/1/users/{}", user2.id);
    let response = client.delete(&url).cookie(user1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_admin_can_delete_users_from_own_company_only() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two companies
    let company1 = create_company(&client, &admin_cookie, "Company 1").await;
    let company2 = create_company(&client, &admin_cookie, "Company 2").await;

    // Create users for both companies
    let _company1_admin = create_user_with_role(
        &client,
        &admin_cookie,
        "admin@company1.com",
        company1.id,
        "admin",
    )
    .await;
    let company1_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company1.com",
        company1.id,
        "staff",
    )
    .await;
    let company2_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@company2.com",
        company2.id,
        "staff",
    )
    .await;

    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Admin should be able to delete users from own company
    let url = format!("/api/1/users/{}", company1_user.id);
    let response = client
        .delete(&url)
        .cookie(admin1_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify user was deleted
    let get_response = client
        .get(&url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(get_response.status(), Status::NotFound);

    // Admin should NOT be able to delete users from different company
    let url = format!("/api/1/users/{}", company2_user.id);
    let response = client.delete(&url).cookie(admin1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_staff_can_delete_any_user() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get Newtown Energy company
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

    // Create test company and user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let test_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;

    // Create newtown-staff user
    let _newtown_staff = create_user_with_role(
        &client,
        &admin_cookie,
        "staff@newtown.com",
        newtown_company.id,
        "newtown-staff",
    )
    .await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;

    // Should be able to delete any user
    let url = format!("/api/1/users/{}", test_user.id);
    let response = client.delete(&url).cookie(staff_session).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify user was deleted
    let get_response = client.get(&url).cookie(admin_cookie).dispatch().await;
    assert_eq!(get_response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_newtown_admin_can_delete_any_user() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create test company and user
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    let test_user = create_user_with_role(
        &client,
        &admin_cookie,
        "user@testcompany.com",
        test_company.id,
        "staff",
    )
    .await;

    // newtown-admin should be able to delete any user
    let url = format!("/api/1/users/{}", test_user.id);
    let response = client
        .delete(&url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify user was deleted
    let get_response = client.get(&url).cookie(admin_cookie).dispatch().await;
    assert_eq!(get_response.status(), Status::NotFound);
}
