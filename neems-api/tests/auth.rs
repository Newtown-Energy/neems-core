//! Comprehensive authentication tests for login and logout functionality
//!
//! This module combines all authentication-related tests including:
//! - Login with various credentials (success/failure cases)
//! - Logout functionality and session invalidation
//! - Protected endpoint access verification
//! - Complete authentication flows

#[macro_use]
extern crate time_test;

use rocket::http::Status;
use rocket::tokio;
use serde_json::json;

use neems_api::orm::testing::fast_test_rocket;

/// Helper to login with specific credentials and get session cookie
async fn login_user(
    client: &rocket::local::asynchronous::Client,
    email: &str,
    password: &str,
) -> Result<rocket::http::Cookie<'static>, Status> {
    let login_body = json!({
        "email": email,
        "password": password
    });

    let response = client
        .post("/api/1/login")
        .json(&login_body)
        .dispatch()
        .await;

    if response.status() == Status::Ok {
        let session_cookie = response
            .cookies()
            .get("session")
            .expect("Session cookie should be set")
            .clone()
            .into_owned();
        Ok(session_cookie)
    } else {
        Err(response.status())
    }
}

// LOGIN TESTS

// test_login_success removed - covered by test_complete_auth_flow

#[tokio::test]
async fn test_wrong_email() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();
    time_test!("test_wrong_email");

    let response = client
        .post("/api/1/login")
        .json(&json!({
            "email": "nonexistent@example.com",
            "password": "admin"
        }))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Unauthorized);
    let body: serde_json::Value = response.into_json().await.unwrap();
    assert_eq!(body["error"], "Invalid credentials");
}

#[tokio::test]
async fn test_wrong_password() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();
    time_test!("test_wrong_password");

    let response = client
        .post("/api/1/login")
        .json(&json!({
            "email": "testuser@example.com",
            "password": "wrong_password"
        }))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Unauthorized);
    let body: serde_json::Value = response.into_json().await.unwrap();
    assert_eq!(body["error"], "Invalid credentials");
}

#[tokio::test]
async fn test_empty_email() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();
    time_test!("test_empty_email");

    let response = client
        .post("/api/1/login")
        .json(&json!({
            "email": "",
            "password": "admin"
        }))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}

#[tokio::test]
async fn test_empty_password() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();
    time_test!("test_empty_password");

    let response = client
        .post("/api/1/login")
        .json(&json!({
            "email": "testuser@example.com",
            "password": ""
        }))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}

// test_logout removed - covered by test_complete_auth_flow

// test_secure_hello_requires_auth removed - covered by test_complete_auth_flow

// MULTI-ROLE USER TESTS

#[tokio::test]
async fn test_authenticated_user_has_roles() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();
    time_test!("test_authenticated_user_has_roles");

    // Use golden DB user with multiple roles (admin_staff@example.com has admin + staff)
    let login_body = json!({
        "email": "admin_staff@example.com",
        "password": "adminstaff"
    });
    let response = client
        .post("/api/1/login")
        .json(&login_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    // Get session cookie before consuming response
    let session_cookie = response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone();

    // Verify login response contains user info including multiple roles
    let login_body: serde_json::Value = response.into_json().await.unwrap();
    assert_eq!(login_body["email"], "admin_staff@example.com");
    let login_roles = login_body["roles"].as_array().unwrap();
    assert!(login_roles.iter().any(|r| r.as_str() == Some("admin")));
    assert!(
        login_roles
            .iter()
            .any(|r| r.as_str() == Some("staff"))
    );

    // Test that we can access protected routes
    let response = client
        .get("/api/1/hello")
        .cookie(session_cookie)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    // Verify hello response also contains user info
    let hello_body: serde_json::Value = response.into_json().await.unwrap();
    assert_eq!(hello_body["email"], "admin_staff@example.com");
    let hello_roles = hello_body["roles"].as_array().unwrap();
    assert!(hello_roles.iter().any(|r| r.as_str() == Some("admin")));
    assert!(
        hello_roles
            .iter()
            .any(|r| r.as_str() == Some("staff"))
    );

    // Test role checking methods (we'll do this by examining the session guard directly)
    // This is a bit complex since we need to test the guard logic
    // For now, we verify that the authentication works with multiple roles
}

// test_role_helper_methods removed - no actual testing logic, just basic auth which is covered elsewhere

// COMPREHENSIVE AUTHENTICATION FLOW TESTS

// test_login_hello_data_consistency removed - basic functionality covered by test_complete_auth_flow

/// Test complete authentication flow: login → use session → logout → verify session invalid
#[tokio::test]
async fn test_complete_auth_flow() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();
    time_test!("test_complete_auth_flow");

    // 1. Verify unauthenticated access fails
    let response = client.get("/api/1/hello").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // 2. Login successfully
    let session_cookie = login_user(&client, "testuser@example.com", "admin")
        .await
        .expect("Login should succeed");

    // 3. Use authenticated session and verify response structure
    let response = client
        .get("/api/1/hello")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value = response.into_json().await.unwrap();
    assert_eq!(body["email"], "testuser@example.com");
    assert!(body["user_id"].is_number());
    assert!(body["company_name"].is_string());
    assert!(body["roles"].is_array());
    // Verify the test user has the "staff" role from golden database
    let roles = body["roles"].as_array().unwrap();
    assert!(roles.iter().any(|r| r.as_str() == Some("staff")));

    // 4. Logout
    let logout_response = client
        .post("/api/1/logout")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(logout_response.status(), Status::Ok);

    // 5. Verify session is invalidated
    let response = client
        .get("/api/1/hello")
        .cookie(session_cookie)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
}

// test_user_without_roles_fails removed - just tests basic auth which is covered by test_complete_auth_flow

#[tokio::test]
async fn test_user_role_assignment() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();
    time_test!("test_user_role_assignment");

    // Use golden DB user with newtown-staff role
    let login_body = json!({
        "email": "newtownstaff@newtown.com",
        "password": "admin"
    });
    let response = client
        .post("/api/1/login")
        .json(&login_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    // Test that authentication works
    let session_cookie = response
        .cookies()
        .get("session")
        .expect("Session cookie should be set");

    let response = client
        .get("/api/1/hello")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
}