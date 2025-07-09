use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::models::Institution;
use neems_core::db::test_rocket;

#[rocket::async_test]
async fn test_create_institution() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    let new_inst = json!({
        "name": "Test University"
    });

    let response = client.post("/api/1/institutions")
        .header(ContentType::JSON)
        .body(new_inst.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let returned: Institution = response.into_json().await.expect("valid JSON response");
    assert_eq!(returned.name, "Test University");
    assert!(returned.id.is_some());
    assert!(returned.created_at <= returned.updated_at);
}

#[ignore]
#[rocket::async_test]
async fn test_list_institutions() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    // Optionally, create an institution first
    let new_inst = json!({ "name": "List Test College" });
    client.post("/institutions")
        .header(ContentType::JSON)
        .body(new_inst.to_string())
        .dispatch()
        .await;

    let response = client.get("/api/1/institutions").dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let list: Vec<Institution> = response.into_json().await.expect("valid JSON response");
    assert!(!list.is_empty());
    assert!(list.iter().any(|i| i.name == "List Test College"));
}
