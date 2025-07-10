use rocket::http::{ContentType};
use rocket::local::asynchronous::Client;
use rocket::serde::json::json;

use neems_core::db::{test_rocket};
use neems_core::models::{Institution, User};


/// Helper to seed the test DB with "Newtown Energy" and return its ID.
async fn seed_institution(client: &Client) -> i32 {
    let new_inst = json!({ "name": "Newtown Energy" });

    let response = client.post("/api/1/institutions")
        .header(ContentType::JSON)
        .body(new_inst.to_string())
        .dispatch()
        .await;

    assert!(response.status().code < 400, "Institution creation failed");

    let institution: Institution = response.into_json().await.expect("valid JSON");
    institution.id.expect("Institution should have an ID")
}

#[rocket::async_test]
async fn test_create_user() {
    // 1. Create a Rocket client
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    // 2. Create institution via API (ensures it exists in the same database)
    let institution_id = seed_institution(&client).await;

    // 3. Create a user using that institution_id
    let new_user = json!({
        "email": "testuser@example.com",
        "password_hash": "hashed_pw",
        "institution_id": institution_id,
        "totp_secret": "SECRET123"
    });

    let response = client.post("/api/1/users")
        .header(ContentType::JSON)
        .body(new_user.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Created);

    let returned: User = response.into_json().await.expect("valid JSON response");
    assert_eq!(returned.email, "testuser@example.com");
    assert_eq!(returned.institution_id, institution_id);
}



#[rocket::async_test]
async fn test_list_users() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    // Seed institution and create a user
    let institution_id = seed_institution(&client).await;

    let new_user = json!({
        "username": "listuser",
        "email": "listuser@example.com",
        "password_hash": "hashed_pw2",
        "institution_id": institution_id,
        "totp_secret": "SECRET456"
    });
    client.post("/api/1/users")
        .header(ContentType::JSON)
        .body(new_user.to_string())
        .dispatch()
        .await;

    let response = client.get("/api/1/users").dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Ok);

    let list: Vec<User> = response.into_json().await.expect("valid JSON response");
    assert!(!list.is_empty());
    assert!(list.iter().any(|u| u.email == "listuser@example.com"));
}
