use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::orm::test_rocket;
use neems_core::models::{Institution, InstitutionNoTime};

/// Helper to create an institution via the API and return the created Institution
pub async fn create_institution_by_api(
    client: &Client,
    inst: &InstitutionNoTime,
) -> Institution {
    let body = json!({ "name": &inst.name }).to_string();
    let response = client
        .post("/api/1/institutions")
        .header(ContentType::JSON)
        .body(body)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Created);

    response
        .into_json::<Institution>()
        .await
        .expect("valid Institution JSON response")
}

#[rocket::async_test]
async fn test_create_institution() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    let new_inst = InstitutionNoTime { name: "Test Company".to_string() };

    let returned: Institution = create_institution_by_api(&client, &new_inst).await;

    assert_eq!(returned.name, "Test Company");
    assert!(returned.id.is_some());
    assert!(returned.created_at <= returned.updated_at);
}



#[rocket::async_test]
async fn test_list_institutions() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");

    // 1. First create a test institution
    let new_inst = InstitutionNoTime { name: "Test LLC".to_string() };
    let create_response = client.post("/api/1/institutions")
        .json(&new_inst)
        .dispatch()
        .await;
    assert_eq!(create_response.status(), Status::Created);

    // 2. Now get the list
    let response = client.get("/api/1/institutions").dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let list: Vec<Institution> = response.into_json().await.expect("valid JSON response");
    dbg!(&list);  // Debug output shows what we got

    assert!(!list.is_empty());
    assert!(list.iter().any(|i| i.name == "Test LLC"));
}
