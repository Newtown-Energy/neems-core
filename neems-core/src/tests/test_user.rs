use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use rocket::serde::json::json;
use serde_json::json;

use neems_core::institution::routes as institution_routes;
use neems_core::models::{Institution, User};
use neems_core::user::routes as user_routes;

/// Helper to create a Rocket instance with both user and institution routes mounted.
fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .mount("/", user_routes())
        .mount("/", institution_routes())
        // .attach(DbConn::fairing()) // Uncomment if you use a DB fairing
}

/// Helper to seed the test DB with "Newtown Energy" and return its ID.
async fn seed_institution(client: &Client) -> i32 {
    let new_inst = json!({ "name": "Newtown Energy" });

    let response = client.post("/institutions")
        .header(ContentType::JSON)
        .body(new_inst.to_string())
        .dispatch()
        .await;

    assert!(response.status().code < 400, "Institution creation failed");

    let institution: Institution = response.into_json().await.expect("valid JSON");
    institution.id.expect("Institution should have an ID")
}

#[rocket::async_test]
async fn test_create_user_with_seeded_institution() {
    let client = Client::tracked(rocket()).await.expect("valid rocket instance");

    // Seed the institution and get its ID
    let institution_id = seed_institution(&client).await;

    // Now create a user using that institution_id
    let new_user = json!({
        "username": "testuser",
        "email": "testuser@example.com",
        "password_hash": "hashed_pw",
        "institution_id": institution_id,
        "totp_secret": "SECRET123"
    });

    let response = client.post("/users")
        .header(ContentType::JSON)
        .body(new_user.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Ok);

    let returned: User = response.into_json().await.expect("valid JSON response");
    assert_eq!(returned.username, "testuser");
    assert_eq!(returned.institution_id, institution_id);
}

#[rocket::async_test]
async fn test_list_users_with_seeded_institution() {
    let client = Client::tracked(rocket()).await.expect("valid rocket instance");

    // Seed institution and create a user
    let institution_id = seed_institution(&client).await;

    let new_user = json!({
        "username": "listuser",
        "email": "listuser@example.com",
        "password_hash": "hashed_pw2",
        "institution_id": institution_id,
        "totp_secret": "SECRET456"
    });
    client.post("/users")
        .header(ContentType::JSON)
        .body(new_user.to_string())
        .dispatch()
        .await;

    let response = client.get("/users").dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Ok);

    let list: Vec<User> = response.into_json().await.expect("valid JSON response");
    assert!(!list.is_empty());
    assert!(list.iter().any(|u| u.username == "listuser"));
}
