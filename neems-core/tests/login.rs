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

#[ignore]
#[tokio::test]
async fn _test_successful_login_old() {
    // --- 1. Create Rocket client for testing ---
    let rocket = test_rocket();
    let client = rocket::local::asynchronous::Client::tracked(rocket).await.unwrap();

    // --- 2. Seed the DB with institution and user ---

    // Create institution
    let inst = InstitutionNoTime { name: "Reqwest Test Inst".to_string() };
    let inst_resp = client.post("/api/1/institutions")
        .json(&json!({ "name": inst.name }))
        .dispatch()
        .await;
    assert!(inst_resp.status().code < 400);
    let inst_json: serde_json::Value = inst_resp.into_json().await.unwrap();
    let institution_id = inst_json["id"].as_i64().unwrap() as i32;

    // Hash a password as your login expects
    let _password = "testpass";
    let salt = "somesalt"; // In production, use random salt
    let hash = "somehash"; // In production, use argon2
    let password_hash = format!("argon2:{}:{}", salt, hash);

    // Create user
    let user = UserNoTime {
        email: "reqwestuser@example.com".to_string(),
        password_hash: password_hash.clone(),
        institution_id,
        totp_secret: "dummysecret".to_string(),
    };
    let user_resp = client.post("/api/1/users")
        .json(&json!({
            "email": user.email,
            "password_hash": user.password_hash,
            "institution_id": user.institution_id,
            "totp_secret": user.totp_secret,
        }))
        .dispatch()
        .await;
    assert!(user_resp.status().code < 400);

    // --- 3. Test login with local client ---
    let login_body = json!({
        "email": user.email,
        "password_hash": password_hash,
    });

    let resp = client.post("/api/1/login")
        .json(&login_body)
        .dispatch()
        .await;

    assert_eq!(resp.status(), Status::Ok);

    // --- 4. Check for session cookie ---
    let session_cookie = resp.cookies().get("session");
    assert!(session_cookie.is_some(), "Session cookie should be set");
}

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

#[tokio::test]
async fn test_successful_login() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();
    add_dummy_data(&client).await;

}

#[ignore]
#[tokio::test]
async fn test_secure_hello_requires_auth() {
    let client = rocket::local::asynchronous::Client::tracked(test_rocket()).await.unwrap();

    let new_inst = InstitutionNoTime { name: "Newtown Energy".to_string() };
    create_institution_by_api(&client, &new_inst).await;

    // 1. Unauthenticated request should fail
    let response = client.get("/api/1/hello").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // 2. Simulate login to get session cookie
    let login_body = json!({
	"username": "testuser",
	"password_hash": "dummy_hash" // Use a real hash for your test user
    });
    let response = client.post("/api/1/login")
	.json(&login_body)
	.dispatch()
	.await;
    assert_eq!(response.status(), Status::Ok);

    let session_cookie = response.cookies().get("session").unwrap().clone();

    // 3. Authenticated request should succeed
    let response = client.get("/api/1/hello")
	.cookie(session_cookie)
	.dispatch()
	.await;
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().await.unwrap();
    assert!(body.contains("Hello, testuser"));
}
