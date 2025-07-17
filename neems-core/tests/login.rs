#[macro_use] extern crate time_test;

use rocket::http::{Status};
use rocket::tokio;
use serde_json::json;

use neems_core::orm::login::hash_password;
use neems_core::orm::{DbConn};
use neems_core::orm::testing::test_rocket;
use neems_core::orm::institution::insert_institution;
use neems_core::orm::user::insert_user;
use neems_core::models::{UserNoTime};
mod institution;
use neems_core::institution::{random_energy_company_names};

/// Creates dummy data for testing by directly inserting test institution and user into the database.
/// This function uses ORM functions directly instead of API endpoints.
pub async fn add_dummy_data(client: &rocket::local::asynchronous::Client) -> &rocket::local::asynchronous::Client {
    // Get database connection from the same pool that the client uses
    let db_conn = DbConn::get_one(client.rocket()).await
        .expect("database connection for add_dummy_data");
    
    db_conn.run(|conn| {
        // Create institution directly using ORM
        let inst = insert_institution(conn, random_energy_company_names(1)[0].to_string())
            .expect("Failed to insert institution");

        // Create test user directly using ORM
        insert_user(conn, UserNoTime {
            email: "testuser@example.com".to_string(),
            password_hash: hash_password("testpassword"),
            institution_id: inst.id.expect("Institution must have an ID"),
            totp_secret: "dummy_secret".to_string(),
        }).expect("Failed to insert user");
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
