use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::orm::test_rocket;
use neems_core::models::{Institution, InstitutionNoTime};

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
async fn test_create_institution() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated request fails
    let new_inst = InstitutionNoTime { name: "Test Company".to_string() };
    
    let response = client.post("/api/1/institutions")
        .json(&new_inst)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Unauthorized);
    
    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;
    
    let response = client.post("/api/1/institutions")
        .json(&new_inst)
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Created);
    
    let returned: Institution = response.into_json().await.expect("valid JSON response");
    assert_eq!(returned.name, "Test Company");
}


#[rocket::async_test]
async fn test_list_institutions() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/institutions").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
 
    // Login
    let session_cookie = login_and_get_session(&client).await;

    // 1. Create a test institution
    let new_inst = InstitutionNoTime { name: "Test LLC".to_string() };
    let create_response = client.post("/api/1/institutions")
        .cookie(session_cookie.clone())
        .json(&new_inst)
        .dispatch()
        .await;
    assert_eq!(create_response.status(), Status::Created);

    // 2. Now get the list
    let response = client.get("/api/1/institutions")
        .cookie(session_cookie)
	.dispatch()
	.await;
    assert_eq!(response.status(), Status::Ok);

    let list: Vec<Institution> = response.into_json().await.expect("valid JSON response");
    dbg!(&list);  // Debug output shows what we got

    assert!(!list.is_empty());
    assert!(list.iter().any(|i| i.name == "Test LLC"));
}
