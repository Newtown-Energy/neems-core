use rocket::http::Status;
use serde_json::json;

use neems_core::db::test_rocket;
use neems_core::models::InstitutionNoTime;
mod institution;
use institution::create_institution_by_api;

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
