use reqwest::Client;
use rocket::config::{Config, LogLevel};
use rocket::figment::Figment;
use rocket::http::Status;
use rocket::tokio;
use serde_json::json;

use neems_core::db::test_rocket;
use neems_core::models::{InstitutionNoTime, UserNoTime};
mod institution;
use institution::create_institution_by_api;

#[tokio::test]
async fn test_successful_login_with_reqwest() {
    // --- 1. Start Rocket on a random port ---
    let figment = Figment::from(Config::default())
        .merge(("port", 0)) // 0 = random port
        .merge(("log_level", LogLevel::Off));
    let rocket = test_rocket().configure(figment);
    let rocket = rocket.launch().await.expect("launch rocket");

    // Get the port Rocket actually bound to
    let port = rocket.config().port;
    let base_url = format!("http://localhost:{}", port);

    // --- 2. Seed the DB with institution and user ---
    // Use a Rocket client to seed via API
    let client = rocket::local::asynchronous::Client::tracked(rocket).await.unwrap();

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
    let password = "testpass";
    let salt = "somesalt"; // In production, use random salt
    let hash = "somehash"; // In production, use argon2
    let password_hash = format!("argon2:{}:{}", salt, hash);

    // Create user
    let user = UserNoTime {
        username: "reqwestuser".to_string(),
        email: "reqwestuser@example.com".to_string(),
        password_hash: password_hash.clone(),
        institution_id,
        totp_secret: "dummysecret".to_string(),
    };
    let user_resp = client.post("/api/1/users")
        .json(&json!({
            "username": user.username,
            "email": user.email,
            "password_hash": user.password_hash,
            "institution_id": user.institution_id,
            "totp_secret": user.totp_secret,
        }))
        .dispatch()
        .await;
    assert!(user_resp.status().code < 400);

    // --- 3. Use reqwest to POST to /api/1/login ---
    let reqwest_client = Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    let login_body = json!({
        "username": user.username,
        "password_hash": password_hash,
    });

    let resp = reqwest_client
        .post(&format!("{}/api/1/login", base_url))
        .json(&login_body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // --- 4. Check for session cookie ---
    let cookies = resp.cookies().collect::<Vec<_>>();
    let session_cookie = cookies.iter().find(|c| c.name() == "session");
    assert!(session_cookie.is_some(), "Session cookie should be set");
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
	"password_hash": "argon2:...:..." // Use a real hash for your test user
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
