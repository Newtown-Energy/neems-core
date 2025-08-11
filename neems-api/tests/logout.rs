use rocket::http::Status;
use rocket::tokio;
use serde_json::json;

use neems_api::orm::testing::fast_test_rocket;

#[tokio::test]
async fn test_logout() {
    let client = rocket::local::asynchronous::Client::tracked(fast_test_rocket())
        .await
        .unwrap();

    let response = client
        .post("/api/1/login")
        .json(&json!({
            "email": "superadmin@example.com",
            "password": "admin"
        }))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    assert!(response.cookies().get("session").is_some());

    //Now we are logged in, so let's test logout
    let session_cookie = response
        .cookies()
        .get("session")
        .expect("Session cookie should be set after login");

    // Test logout endpoint
    let logout_response = client
        .post("/api/1/logout")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(logout_response.status(), Status::Ok);

    // Verify session cookie is removed/invalidated
    let session_cookie_after_logout = logout_response.cookies().get("session");
    assert!(
        session_cookie_after_logout.is_none()
            || session_cookie_after_logout.unwrap().value().is_empty()
    );

    // Test that accessing protected endpoint fails after logout
    let protected_response = client
        .get("/api/1/hello")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(protected_response.status(), Status::Unauthorized);
}
