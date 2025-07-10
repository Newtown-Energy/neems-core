use rand::prelude::IndexedRandom;
use rocket::http::Status;
use rocket::tokio;
use serde_json::json;

use neems_core::db::test_rocket;
use neems_core::models::{InstitutionNoTime, UserNoTime};
mod institution;
use institution::create_institution_by_api;
use neems_core::institution::{random_energy_company_names};
use neems_core::user::{create_user_by_api, random_usernames};

async fn add_dummy_data(client: &rocket::local::asynchronous::Client) -> &rocket::local::asynchronous::Client {
    // First create institutions
    let inst_names = random_energy_company_names(2);
    let mut institutions = Vec::new();
    
    for name in inst_names {
        let inst = create_institution_by_api(&client, &InstitutionNoTime { name: name.to_string() }).await;
	institutions.push(inst);
    }

    // Then create users with proper UserNoTime data
    for username in random_usernames(50) {
        // Get a random institution from those we created
	let inst = institutions.choose(&mut rand::rng()).expect("No institutions available");

	// Gran the id from that institution
        let institution_id = inst.id.expect("Institution must have an ID");

        // Create random email based on username + @ + institution + .com
        let email = format!("{}@{}.com", username.to_lowercase(), inst.name.to_lowercase().replace(" ", "-"));
        
        create_user_by_api(&client, &UserNoTime {
            email,
            password_hash: "dummy_hash".to_string(),
            institution_id,
            totp_secret: "dummy_secret".to_string(),
        }).await;
    }
    
    // One more user, this time with a predefined username
    let inst = institutions.choose(&mut rand::rng()).expect("No institutions available");
    create_user_by_api(&client, &UserNoTime {
	email: "testuser@example.com".to_string(),
	password_hash: "dummy_hash".to_string(), 
	institution_id: inst.id.expect("Institution must have an ID"),
	totp_secret: "dummy_secret".to_string(),
    }).await;

    client
}

/// Tests the /api/1/login endpoint for various failure scenarios.
///
/// This test verifies that:
/// - A valid login request succeeds (sanity check).
/// - Logging in with a non-existent email returns 401 Unauthorized and an appropriate error message.
/// - Logging in with an incorrect password returns 401 Unauthorized and an appropriate error message.
/// - Logging in with an empty email returns 400 Bad Request.
/// - Logging in with an empty password returns 400 Bad Request.
///
/// The test ensures that authentication and input validation behave as expected
/// and that error responses contain the correct status codes and messages.
#[tokio::test]
async fn test_login_fail() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    add_dummy_data(&client).await;

    // Test successful login first to ensure our test user exists
    let valid_login_body = json!({
        "email": "testuser@example.com",
        "password_hash": "dummy_hash"
    });
    let response = client.post("/api/1/login")
        .json(&valid_login_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    // Test wrong email
    let wrong_email_body = json!({
        "email": "nonexistent@example.com",
        "password_hash": "dummy_hash"
    });
    let response = client.post("/api/1/login")
        .json(&wrong_email_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
    let response_body = response.into_string().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&response_body).unwrap();
    assert_eq!(json["error"], "Invalid credentials");

    // Test wrong password
    let wrong_password_body = json!({
        "email": "testuser@example.com",
        "password_hash": "wrong_password"
    });
    let response = client.post("/api/1/login")
        .json(&wrong_password_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
    let response_body = response.into_string().await.unwrap();
    assert!(response_body.contains("Invalid credentials"));

    // Test empty email
    let empty_email_body = json!({
        "email": "",
        "password_hash": "dummy_hash"
    });
    let response = client.post("/api/1/login")
        .json(&empty_email_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);

    // Test empty password
    let empty_password_body = json!({
        "email": "testuser@example.com",
        "password_hash": ""
    });
    let response = client.post("/api/1/login")
        .json(&empty_password_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
}


#[tokio::test]
async fn test_secure_hello_requires_auth() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    add_dummy_data(&client).await;

    // 1. Unauthenticated request should fail
    let response = client.get("/api/1/hello")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    // 2. login to get session cookie
    let login_body = json!({
	"email": "testuser@example.com",
	"password_hash": "dummy_hash" // Use a real hash for your test user
    });
    let response = client.post("/api/1/login")
	.json(&login_body)
	.dispatch()
	.await;
    assert_eq!(response.status(), Status::Ok);

    let session_cookie = response.cookies().get("session");
    assert!(session_cookie.is_some(), "Session cookie should be set");
    let session_cookie = session_cookie.unwrap().clone();

    // 3. Authenticated request should succeed
    let response = client.get("/api/1/hello")
	.cookie(session_cookie)
	.dispatch()
	.await;
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().await.unwrap();
    assert!(body.contains("Hello, testuser@example.com"));
}
