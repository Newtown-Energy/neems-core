use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_api::models::{Company, CompanyNoTime};
use neems_api::orm::testing::test_rocket;

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
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let new_comp = CompanyNoTime {
        name: "Test Company".to_string(),
    };

    let response = client
        .post("/api/1/companies")
        .json(&new_comp)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Unauthorized);

    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;

    let response = client
        .post("/api/1/companies")
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
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/companies").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Login
    let session_cookie = login_and_get_session(&client).await;

    // 1. Create a test company
    let new_comp = CompanyNoTime {
        name: "Test LLC".to_string(),
    };
    let create_response = client
        .post("/api/1/companies")
        .cookie(session_cookie.clone())
        .json(&new_comp)
        .dispatch()
        .await;
    assert_eq!(create_response.status(), Status::Created);

    // 2. Now get the list
    let response = client
        .get("/api/1/companies")
        .cookie(session_cookie)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let list: Vec<Company> = response.into_json().await.expect("valid JSON response");
    dbg!(&list); // Debug output shows what we got

    assert!(!list.is_empty());
    assert!(list.iter().any(|i| i.name == "Test LLC"));
}

#[rocket::async_test]
async fn test_delete_company() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.delete("/api/1/companies/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Login
    let session_cookie = login_and_get_session(&client).await;

    // 1. Create a test company
    let new_comp = CompanyNoTime {
        name: "Company to Delete".to_string(),
    };
    let create_response = client
        .post("/api/1/companies")
        .cookie(session_cookie.clone())
        .json(&new_comp)
        .dispatch()
        .await;
    assert_eq!(create_response.status(), Status::Created);

    let created_company: Company = create_response
        .into_json()
        .await
        .expect("valid JSON response");

    // 2. Delete the company
    let delete_url = format!("/api/1/companies/{}", created_company.id);
    let delete_response = client
        .delete(&delete_url)
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(delete_response.status(), Status::NoContent);

    // 3. Verify company is deleted by trying to get all companies
    let list_response = client
        .get("/api/1/companies")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(list_response.status(), Status::Ok);

    let list: Vec<Company> = list_response
        .into_json()
        .await
        .expect("valid JSON response");
    assert!(!list.iter().any(|c| c.id == created_company.id));
}

#[rocket::async_test]
async fn test_delete_nonexistent_company() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;

    // Try to delete a company that doesn't exist (using a very high ID)
    let delete_response = client
        .delete("/api/1/companies/99999")
        .cookie(session_cookie)
        .dispatch()
        .await;
    assert_eq!(delete_response.status(), Status::NotFound);
}
