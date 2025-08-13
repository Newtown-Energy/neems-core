use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_api::models::{Company, CompanyInput};
use neems_api::orm::testing::fast_test_rocket;

/// Helper to get a test company by name
async fn get_company_by_name(client: &Client, session_cookie: &rocket::http::Cookie<'static>, name: &str) -> Company {
    let response = client
        .get("/api/1/Companies")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> = serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    companies.into_iter()
        .find(|c| c.name == name)
        .expect(&format!("Company '{}' should exist from test data initialization", name))
}

/// Helper to login and get session cookie
async fn login_and_get_session(client: &Client) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": "superadmin@example.com",
        "password": "admin"
    });

    let response = client
        .post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

#[rocket::async_test]
async fn test_create_company() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let new_comp = CompanyInput {
        name: "Test Company".to_string(),
    };

    let response = client
        .post("/api/1/Companies")
        .json(&new_comp)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Unauthorized);

    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;

    let response = client
        .post("/api/1/Companies")
        .json(&new_comp)
        .cookie(session_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);

    let returned: Company = response.into_json().await.expect("valid JSON response");
    assert_eq!(returned.name, "Test Company");
}

#[rocket::async_test]
async fn test_list_companies() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/Companies").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Login
    let session_cookie = login_and_get_session(&client).await;

    // Get the list of companies (should include pre-created test companies)
    let response = client
        .get("/api/1/Companies")
        .cookie(session_cookie)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let odata_response: serde_json::Value = response.into_json().await.expect("valid JSON response");
    let list: Vec<Company> = serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");

    assert!(!list.is_empty(), "Should have some companies from test data initialization");
    // Verify we have the expected pre-created companies
    assert!(list.iter().any(|c| c.name == "Test Company 1"), "Should contain Test Company 1");
    assert!(list.iter().any(|c| c.name == "Newtown Energy"), "Should contain Newtown Energy");
}

#[rocket::async_test]
async fn test_delete_company() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.delete("/api/1/Companies/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Login
    let session_cookie = login_and_get_session(&client).await;

    // Get a pre-created test company to delete (Removable LLC has no users assigned)
    let company_to_delete = get_company_by_name(&client, &session_cookie, "Removable LLC").await;

    // Delete the company
    let delete_url = format!("/api/1/Companies/{}", company_to_delete.id);
    let delete_response = client
        .delete(&delete_url)
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(delete_response.status(), Status::NoContent);

    // Verify company is deleted by trying to get all companies
    let list_response = client
        .get("/api/1/Companies")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(list_response.status(), Status::Ok);

    let odata_response: serde_json::Value = list_response
        .into_json()
        .await
        .expect("valid JSON response");
    let list: Vec<Company> = serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    assert!(!list.iter().any(|c| c.id == company_to_delete.id), "Deleted company should not appear in list");
}

#[rocket::async_test]
async fn test_delete_nonexistent_company() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;

    // Try to delete a company that doesn't exist (using a very high ID)
    let delete_response = client
        .delete("/api/1/Companies/99999")
        .cookie(session_cookie)
        .dispatch()
        .await;
    assert_eq!(delete_response.status(), Status::NotFound);
}
