//! Comprehensive user management tests combining CRUD operations and RBAC
//!
//! This module combines all user-related tests including:
//! - Basic CRUD operations (create, read, update, delete)
//! - Role-based access control (RBAC) for all operations
//! - User role management
//! - Admin user creation and verification

use diesel::prelude::*;
use rocket::http::ContentType;
use rocket::local::asynchronous::Client;
use rocket::serde::json::json;
use rocket::tokio;

use neems_api::models::{Role, User, UserWithRoles, Company};
use neems_api::orm::testing::fast_test_rocket;

/// Unified helper to login with specific credentials and get session cookie
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

    assert_eq!(response.status(), rocket::http::Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

/// Helper to get a pre-existing test company ID from golden database
fn get_test_company_id() -> i32 {
    2 // Test Company 1 from golden database
}


/// Helper to get a test company by name
async fn get_company_by_name(client: &Client, admin_cookie: &rocket::http::Cookie<'static>, name: &str) -> Company {
    let response = client
        .get("/api/1/Companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> = serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    companies.into_iter()
        .find(|c| c.name == name)
        .expect(&format!("Company '{}' should exist", name))
}


// ADMIN USER CREATION TESTS

#[tokio::test]
async fn test_admin_user_is_created() {
    // Start Rocket with the admin fairing attached
    let rocket = fast_test_rocket();
    let client = Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Get a DB connection from the pool
    let conn = neems_api::orm::DbConn::get_one(client.rocket())
        .await
        .expect("get db connection");

    // Use the default admin email (from env or fallback)
    let admin_email = std::env::var("NEEMS_DEFAULT_USER")
        .unwrap_or_else(|_| "superadmin@example.com".to_string());

    // Query for the admin user and verify it has the newtown-admin role
    let (found_user, has_admin_role) = conn
        .run(move |c| {
            use neems_api::schema::users::dsl::*;
            use neems_api::schema::{roles, user_roles};
            
            // Find the admin user
            let user = users
                .filter(email.eq(admin_email))
                .first::<User>(c)
                .optional()
                .expect("user query should not fail");

            let has_role = if let Some(ref u) = user {
                // Check if the user has the newtown-admin role
                let role_exists = user_roles::table
                    .inner_join(roles::table)
                    .filter(user_roles::user_id.eq(u.id))
                    .filter(roles::name.eq("newtown-admin"))
                    .first::<(neems_api::models::UserRole, Role)>(c)
                    .optional()
                    .expect("role query should not fail");

                role_exists.is_some()
            } else {
                false
            };

            (user, has_role)
        })
        .await;

    assert!(
        found_user.is_some(),
        "Admin user should exist after fairing runs"
    );
    assert!(
        has_admin_role,
        "Admin user should have the newtown-admin role"
    );
}

// COMPREHENSIVE USER CRUD WITH RBAC TESTS

#[rocket::async_test]
async fn test_user_crud_operations_and_authentication() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test CREATE user requires authentication
    let new_user = json!({
        "email": "newuser@test.com",
        "password_hash": "hashed_pw",
        "company_id": 1,
        "totp_secret": "SECRET123"
    });

    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .body(new_user.to_string())
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Unauthorized);

    // Test LIST users requires authentication
    let response = client.get("/api/1/Users").dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Unauthorized);

    // Login as admin for authenticated tests
    let session_cookie = login_user(&client, "superadmin@example.com", "admin").await;
    
    // Test authenticated CREATE user succeeds
    let new_user_auth = json!({
        "email": "newuser@test.com",
        "password_hash": "hashed_pw",
        "company_id": get_test_company_id(),
        "totp_secret": "SECRET123",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(new_user_auth.to_string())
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Created,
        "Authenticated user should be able to create new users");

    // Test authenticated LIST users succeeds
    let response = client
        .get("/api/1/Users")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Ok);

    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let list: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");
    assert!(!list.is_empty()); // Should have at least the admin user

    // Test CRUD operations on existing golden DB user
    // Find the golden DB test user
    let test_user = list.iter()
        .find(|u| u.email == "user@empty.com")
        .expect("Golden DB user 'user@empty.com' should exist");

    // Test GET single user
    let url = format!("/api/1/Users/{}", test_user.id);
    let response = client
        .get(&url)
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let retrieved_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(retrieved_user.id, test_user.id);
    assert_eq!(retrieved_user.email, "user@empty.com");

    // Test PUT update user
    let update_data = json!({
        "email": "user@modified.com",
        "totp_secret": "updatedsecret"
    });

    let response = client
        .put(&url)
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(update_data.to_string())
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let updated_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(updated_user.email, "user@modified.com");
    assert_eq!(updated_user.totp_secret, Some("updatedsecret".to_string()));

    // Test DELETE user
    let response = client.delete(&url).cookie(session_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::NoContent);
    
    // Verify deletion worked
    let response = client
        .get(&url)
        .cookie(session_cookie)
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::NotFound);
}

#[rocket::async_test]
async fn test_user_creation_with_unique_email() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let session_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Use a simple non-existent email - golden DB is fresh for this test
    let unique_email = "brandnew@test.com";
    
    // First verify the email doesn't exist in the database
    let conn = neems_api::orm::DbConn::get_one(client.rocket())
        .await
        .expect("get db connection");
    
    let email_for_check = unique_email.to_string();
    let existing_user = conn.run(move |c| {
        neems_api::orm::user::get_user_by_email(c, &email_for_check)
    }).await.expect("database query should work");
    
    assert!(existing_user.is_none(), "Email should not exist in database");

    // Now try to create a user with this email - it should succeed
    let new_user = json!({
        "email": unique_email,
        "password_hash": "hashed_pw",
        "company_id": get_test_company_id(),
        "totp_secret": "testsecret",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .cookie(session_cookie)
        .body(new_user.to_string())
        .dispatch()
        .await;

    // This should succeed (Created), not fail with Conflict
    assert_eq!(response.status(), rocket::http::Status::Created, 
               "Creating user with unique email should succeed");
    
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(created_user.email, unique_email);
}

// RBAC TESTS FOR USER OPERATIONS

#[rocket::async_test]
async fn test_regular_users_cannot_create_users() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Use existing golden database company and user
    let test_company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let user_session = login_user(&client, "staff@testcompany.com", "admin").await;

    let new_user = json!({
        "email": "shouldnotwork@example.com",
        "password_hash": "hash",
        "company_id": test_company.id,
        "totp_secret": "",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/Users")
        .cookie(user_session)
        .json(&new_user)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Forbidden);
}

#[rocket::async_test]
async fn test_admin_user_operations_by_company() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Use existing golden database companies and users
    let company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;

    // Use existing admin@company1.com from golden database (no need to create)
    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Should be able to create user for own company
    let new_user_own_company = json!({
        "email": "newuser_totally_unique_2024@company1.com",
        "password_hash": neems_api::orm::login::hash_password("password"),
        "company_id": company1.id,
        "totp_secret": "",
        "role_names": ["admin"]
    });

    let response = client
        .post("/api/1/Users")
        .cookie(admin1_session.clone())
        .json(&new_user_own_company)
        .dispatch()
        .await;

    // Accept both Created (new user) and Conflict (user already exists)
    assert!(
        response.status() == rocket::http::Status::Created || response.status() == rocket::http::Status::Conflict,
        "Expected 201 Created or 409 Conflict, got: {}",
        response.status()
    );

    // Should NOT be able to create user for different company
    let new_user_other_company = json!({
        "email": "unauthorized@company2.com",
        "password_hash": neems_api::orm::login::hash_password("password"),
        "company_id": company2.id,
        "totp_secret": "",
        "role_names": ["admin"]
    });

    let response = client
        .post("/api/1/Users")
        .cookie(admin1_session.clone())
        .json(&new_user_other_company)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Forbidden);

    // Admin should only see users from their own company when listing
    let response = client
        .get("/api/1/Users")
        .cookie(admin1_session)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");

    // Admin should only see users from their own company (company1)
    for user in &users {
        assert_eq!(user.company_id, company1.id, 
                  "Admin should only see users from company {}, but saw user {} from company {}", 
                  company1.id, user.email, user.company_id);
    }
}

#[rocket::async_test]
async fn test_newtown_staff_can_manage_users_across_companies() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Use existing golden database company
    let test_company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;

    // Use pre-existing newtown-staff user from golden database
    let staff_session = login_user(&client, "newtownstaff@newtown.com", "admin").await;

    // Should be able to create user for any company
    let new_user = json!({
        "email": "newuser@testcompany.com",
        "password_hash": neems_api::orm::login::hash_password("password"),
        "company_id": test_company.id,
        "totp_secret": "",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/Users")
        .cookie(staff_session.clone())
        .json(&new_user)
        .dispatch()
        .await;

    // Should be able to create user for any company (accept both Created and Conflict for existing users)
    assert!(
        response.status() == rocket::http::Status::Created || response.status() == rocket::http::Status::Conflict,
        "Newtown staff should be able to create users for any company, got: {}",
        response.status()
    );

    // Should be able to see all users
    let response = client
        .get("/api/1/Users")
        .cookie(staff_session)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");

    // Should see users from multiple companies (at least 3: superadmin, staff, test_user)
    assert!(users.len() >= 3);

    let emails: Vec<&String> = users.iter().map(|u| &u.email).collect();
    assert!(emails.contains(&&"superadmin@example.com".to_string()));
    assert!(emails.contains(&&"newtownstaff@newtown.com".to_string()));
}

#[rocket::async_test]
async fn test_user_profile_access_permissions() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Use existing golden database user
    let users_response = client.get("/api/1/Users").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(users_response.status(), rocket::http::Status::Ok);
    let odata_response: serde_json::Value = users_response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");
    
    let test_user = users.iter().find(|u| u.email == "staff@testcompany.com")
        .expect("staff@testcompany.com should exist in golden DB");
    let user_session = login_user(&client, "staff@testcompany.com", "admin").await;

    // User should be able to view their own profile
    let url = format!("/api/1/Users/{}", test_user.id);
    let response = client.get(&url).cookie(user_session.clone()).dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let retrieved_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(retrieved_user.id, test_user.id);
    assert_eq!(retrieved_user.email, "staff@testcompany.com");

    // User should be able to update their own profile
    let update_request = json!({
        "email": "staff_updated@testcompany.com"
    });

    let response = client
        .put(&url)
        .cookie(user_session.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let updated_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(updated_user.email, "staff_updated@testcompany.com");

    // Users cannot view other users' profiles
    // Use a different existing user from golden database  
    let user1_session = login_user(&client, "user@company1.com", "admin").await;
    
    // Get testuser@example.com to try to view their profile
    let users_response = client.get("/api/1/Users").cookie(admin_cookie).dispatch().await;
    assert_eq!(users_response.status(), rocket::http::Status::Ok);
    let odata_response: serde_json::Value = users_response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");
    let user2 = users.into_iter().find(|u| u.email == "testuser@example.com")
        .expect("testuser@example.com should exist in golden DB");

    // User1 should NOT be able to view user2's profile
    let url = format!("/api/1/Users/{}", user2.id);
    let response = client.get(&url).cookie(user1_session).dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Forbidden);
}

#[rocket::async_test]
async fn test_user_deletion_permissions() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Use existing golden database companies (company names referenced for clarity)
    let _company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let _company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;

    // Use existing golden database users
    let users_response = client.get("/api/1/Users").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(users_response.status(), rocket::http::Status::Ok);
    let odata_response: serde_json::Value = users_response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");
    
    let company1_user = users.iter().find(|u| u.email == "user@company1.com")
        .expect("user@company1.com should exist in golden DB");
    let company2_user = users.iter().find(|u| u.email == "user@company2.com")
        .expect("user@company2.com should exist in golden DB");

    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Admin should be able to delete users from own company
    let url = format!("/api/1/Users/{}", company1_user.id);
    let response = client
        .delete(&url)
        .cookie(admin1_session.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), rocket::http::Status::NoContent);

    // Verify user was deleted
    let get_response = client
        .get(&url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(get_response.status(), rocket::http::Status::NotFound);

    // Admin should NOT be able to delete users from different company
    let url = format!("/api/1/Users/{}", company2_user.id);
    let response = client.delete(&url).cookie(admin1_session).dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Forbidden);

    // Regular users should NOT be able to delete anyone
    let user1_session = login_user(&client, "staff@testcompany.com", "admin").await;
    let response = client.delete(&url).cookie(user1_session).dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Forbidden);
}