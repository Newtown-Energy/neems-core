use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::models::Role;
use neems_core::orm::test_rocket;

#[rocket::async_test]
async fn test_create_role() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    let new_role = json!({
        "name": "test_role",
        "description": "A test role"
    });

    let response = client.post("/api/1/roles")
        .header(ContentType::JSON)
        .body(new_role.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let returned: Role = response.into_json().await.expect("valid JSON response");
    assert_eq!(returned.name, "test_role");
    assert_eq!(returned.description.as_deref(), Some("A test role"));
    assert!(returned.id.is_some());
}

#[rocket::async_test]
async fn test_list_roles() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    // Optionally, create a role first
    let new_role = json!({ "name": "list_test_role", "description": "For listing" });
    client.post("/api/1/roles")
        .header(ContentType::JSON)
        .body(new_role.to_string())
        .dispatch()
        .await;

    let response = client.get("/api/1/roles").dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let list: Vec<Role> = response.into_json().await.expect("valid JSON response");
    assert!(!list.is_empty());
    assert!(list.iter().any(|r| r.name == "list_test_role"));
}
