use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::models::Role;
use neems_core::orm::test_rocket;

/// Helper to login and get session cookie
async fn login_and_get_session(client: &Client) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": "admin@example.com",
        "password": "admin"
    });
    
    let response = client.post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Ok);
    response.cookies().get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

#[rocket::async_test]
async fn test_create_role() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated request fails
    let new_role = json!({
        "name": "test_role",
        "description": "A test role"
    });
    
    let response = client.post("/api/1/roles")
        .header(ContentType::JSON)
        .body(new_role.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Unauthorized);
    
    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;
    
    let response = client.post("/api/1/roles")
        .header(ContentType::JSON)
        .cookie(session_cookie)
        .body(new_role.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let returned: Role = response.into_json().await.expect("valid JSON response");
    assert_eq!(returned.name, "test_role");
}

#[rocket::async_test]
async fn test_list_roles() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated request fails
    let response = client.get("/api/1/roles").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
    
    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;
    
    let response = client.get("/api/1/roles")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let list: Vec<Role> = response.into_json().await.expect("valid JSON response");
    // Should have at least the default admin role
    assert!(!list.is_empty());
}
