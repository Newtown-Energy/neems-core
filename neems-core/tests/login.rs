#[macro_use] extern crate time_test;

use rocket::http::{Status, ContentType};
use rocket::tokio;
use serde_json::json;

use neems_core::orm::login::hash_password;
use neems_core::orm::test_rocket;
use neems_core::models::{InstitutionNoTime, UserNoTime};
mod institution;
use institution::create_institution_by_api;
use neems_core::institution::{random_energy_company_names};
use neems_core::api::user::{create_user_by_api};

/// Creates dummy data for testing by first logging in as admin, then creating test institution and user.
/// This function demonstrates the authentication flow: admin user (created by fairing) -> create institution -> create test user.
pub async fn add_dummy_data(client: &rocket::local::asynchronous::Client) -> &rocket::local::asynchronous::Client {
    // First login as admin to get authentication cookie
    let admin_login = json!({
        "email": "admin@example.com",
        "password": "admin"
    });
    
    let login_response = client.post("/api/1/login")
        .json(&admin_login)
        .dispatch()
        .await;
    
    assert_eq!(login_response.status(), Status::Ok);
    let admin_cookie = login_response.cookies().get("session")
        .expect("Admin session cookie should be set")
        .clone();
    
    // Create institution using admin authentication
    let name = random_energy_company_names(1)[0];
    let inst_body = json!({ "name": name });
    let inst_response = client.post("/api/1/institutions")
        .cookie(admin_cookie.clone())
        .json(&inst_body)
        .dispatch()
        .await;
    
    assert_eq!(inst_response.status(), Status::Created);
    let inst: neems_core::models::Institution = inst_response.into_json().await.expect("valid institution");
    
    // Create test user using admin authentication
    let test_password_hash = hash_password("testpassword");
    let user_body = json!({
        "email": "testuser@example.com",
        "password_hash": test_password_hash,
        "institution_id": inst.id.expect("Institution must have an ID"),
        "totp_secret": "dummy_secret"
    });
    
    let user_response = client.post("/api/1/users")
        .cookie(admin_cookie)
        .json(&user_body)
        .dispatch()
        .await;
    
    assert_eq!(user_response.status(), Status::Created);
    
    client
}

#[tokio::test]
async fn test_login_success() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    time_test!("test_login_success");
    add_dummy_data(&client).await;

    let response = client.post("/api/1/login")
        .json(&json!({
            "email": "testuser@example.com",
            "password": "testpassword"
        }))
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    assert!(response.cookies().get("session").is_some());
}

#[tokio::test]
async fn test_wrong_email() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    add_dummy_data(&client).await;
    time_test!("test_wrong_email");

    let response = client.post("/api/1/login")
        .json(&json!({
            "email": "nonexistent@example.com",
            "password": "testpassword"
        }))
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Unauthorized);
    let body: serde_json::Value = response.into_json().await.unwrap();
    assert_eq!(body["error"], "Invalid credentials");
}

#[tokio::test]
async fn test_wrong_password() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    time_test!("test_wrong_password");
    add_dummy_data(&client).await;

    let response = client.post("/api/1/login")
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
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    add_dummy_data(&client).await;
    time_test!("test_empty_email");

    let response = client.post("/api/1/login")
        .json(&json!({
            "email": "",
            "password": "testpassword"
        }))
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::BadRequest);
}

#[tokio::test]
async fn test_empty_password() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    time_test!("test_empty_password");
    add_dummy_data(&client).await;

    let response = client.post("/api/1/login")
        .json(&json!({
            "email": "testuser@example.com",
            "password": ""
        }))
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::BadRequest);
}

#[tokio::test]
async fn test_secure_hello_requires_auth() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    add_dummy_data(&client).await;
    time_test!("test_secure_hello_requires_auth");

    // 1. Test unauthenticated request fails
    let response = client.get("/api/1/hello")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    // 2. Login with correct credentials (using the test user created by add_dummy_data)
    let login_body = json!({
        "email": "testuser@example.com",
        "password": "testpassword"  // Test user password from add_dummy_data
    });
    let response = client.post("/api/1/login")
        .json(&login_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    // Verify session cookie was set
    let session_cookie = response.cookies().get("session")
        .expect("Session cookie should be set");
    
    // 3. Test authenticated request succeeds
    let response = client.get("/api/1/hello")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    
    // Verify response contains user's email
    let body = response.into_string().await.unwrap();
    assert!(body.contains("Hello, testuser@example.com"));
}
