use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::role::routes;
use neems_core::models::Role;

// Helper to create a Rocket instance for testing
fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .mount("/", routes())
        // .attach(DbConn::fairing()) // Uncomment if you use a fairing for DB
        // .manage(test_db_pool())    // Or however you provide a test DB
}

#[rocket::async_test]
async fn test_create_role() {
    let client = Client::tracked(rocket()).await.expect("valid rocket instance");

    let new_role = json!({
        "name": "test_role",
        "description": "A test role"
    });

    let response = client.post("/roles")
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
    let client = Client::tracked(rocket()).await.expect("valid rocket instance");

    // Optionally, create a role first
    let new_role = json!({ "name": "list_test_role", "description": "For listing" });
    client.post("/roles")
        .header(ContentType::JSON)
        .body(new_role.to_string())
        .dispatch()
        .await;

    let response = client.get("/roles").dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let list: Vec<Role> = response.into_json().await.expect("valid JSON response");
    assert!(!list.is_empty());
    assert!(list.iter().any(|r| r.name == "list_test_role"));
}
