#[macro_use] extern crate time_test;

use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2
};
use rand_core::OsRng;
use rocket::http::Status;
use rocket::tokio;
use serde_json::json;

use neems_core::db::test_rocket;
use neems_core::models::{InstitutionNoTime, UserNoTime};
mod institution;
use institution::create_institution_by_api;
use neems_core::institution::{random_energy_company_names};
use neems_core::user::{create_user_by_api};


/// Hash passwords with Argon2
fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
	.hash_password(password.as_bytes(), &salt)
	.expect("Hashing should succeed")
	.to_string()
}

async fn add_dummy_data(client: &rocket::local::asynchronous::Client) -> &rocket::local::asynchronous::Client {
    let name = random_energy_company_names(1)[0];
    let inst = create_institution_by_api(&client, &InstitutionNoTime { name: name.to_string() }).await;
    let test_password_hash = hash_password("testpassword");
    create_user_by_api(&client, &UserNoTime {
        email: "testuser@example.com".to_string(),
        password_hash: test_password_hash,
        institution_id: inst.id.expect("Institution must have an ID"),
        totp_secret: "dummy_secret".to_string(),
    }).await;

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

    // 2. Login with correct credentials
    let login_body = json!({
        "email": "testuser@example.com",
        "password": "testpassword"  // Using plaintext password that matches hashed version
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
