use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_api::models::{Command, Company, Site};
use neems_api::orm::testing::fast_test_rocket;

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
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

    assert_eq!(response.status(), Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

/// Helper to get a test company by name
async fn get_company_by_name(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    name: &str,
) -> Company {
    let response = client
        .get("/api/1/Companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value =
        response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    companies
        .into_iter()
        .find(|c| c.name == name)
        .expect(&format!(
            "Company '{}' should exist from test data initialization",
            name
        ))
}

/// Helper to get a test site by name
async fn get_site_by_name(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    name: &str,
) -> Site {
    let response = client
        .get("/api/1/Sites")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value =
        response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    sites
        .into_iter()
        .find(|s| s.name == name)
        .expect(&format!(
            "Site '{}' should exist from test data initialization",
            name
        ))
}

/// Helper to login with specific credentials and get session cookie
async fn login_user(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": email,
        "password": password
    });

    let response = client
        .post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

#[rocket::async_test]
async fn test_command_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test all endpoints require authentication
    let response = client.get("/api/1/Sites/1/Commands").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/Commands/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let new_command = json!({
        "site_id": 1,
        "name": "Test Command",
        "description": "A test command",
        "equipment_type": "inverter",
        "equipment_id": "inverter-a",
        "action": "turn_on",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/Commands")
        .json(&new_command)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    let update_command = json!({
        "name": "Updated Command"
    });

    let response = client
        .put("/api/1/Commands/1")
        .json(&update_command)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.delete("/api/1/Commands/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_company_admin_can_crud_own_company_commands() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company and site
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let site = get_site_by_name(&client, &admin_cookie, "Test Site 1").await;

    assert_eq!(site.company_id, company.id);

    // Login as pre-created company admin
    let admin_session = login_user(&client, "user@testcompany.com", "admin").await;

    // Create a command
    let new_command = json!({
        "site_id": site.id,
        "name": "Test Inverter On",
        "description": "Turn on the test inverter",
        "equipment_type": "inverter",
        "equipment_id": "inverter-test-1",
        "action": "turn_on",
        "parameters": json!({"power_level": 100}).to_string(),
        "is_active": true
    });

    let response = client
        .post("/api/1/Commands")
        .cookie(admin_session.clone())
        .json(&new_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_command: Command = response.into_json().await.expect("valid command JSON");
    assert_eq!(created_command.name, "Test Inverter On");
    assert_eq!(created_command.equipment_type, "inverter");
    assert_eq!(created_command.equipment_id, "inverter-test-1");
    assert_eq!(created_command.action, "turn_on");

    // Read the command
    let url = format!("/api/1/Commands/{}", created_command.id);
    let response = client
        .get(&url)
        .cookie(admin_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_command: Command = response.into_json().await.expect("valid command JSON");
    assert_eq!(retrieved_command.id, created_command.id);

    // List commands for the site
    let url = format!("/api/1/Sites/{}/Commands", site.id);
    let response = client
        .get(&url)
        .cookie(admin_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value =
        response.into_json().await.expect("valid OData JSON");
    let commands: Vec<Command> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid commands array");
    assert!(commands.iter().any(|c| c.id == created_command.id));

    // List only active commands
    let url = format!("/api/1/Sites/{}/Commands?active=true", site.id);
    let response = client
        .get(&url)
        .cookie(admin_session.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value =
        response.into_json().await.expect("valid OData JSON");
    let active_commands: Vec<Command> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid commands array");
    assert!(active_commands.iter().all(|c| c.is_active));

    // Update the command
    let update_data = json!({
        "name": "Updated Test Inverter On",
        "description": "Updated description",
        "is_active": false
    });

    let url = format!("/api/1/Commands/{}", created_command.id);
    let response = client
        .put(&url)
        .cookie(admin_session.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let updated_command: Command = response.into_json().await.expect("valid command JSON");
    assert_eq!(updated_command.name, "Updated Test Inverter On");
    assert_eq!(updated_command.description, Some("Updated description".to_string()));
    assert_eq!(updated_command.is_active, false);

    // Delete the command
    let response = client
        .delete(&url)
        .cookie(admin_session)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify command is deleted
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_company_admin_cannot_access_different_company_commands() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test companies and sites
    let _company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;
    let site2 = get_site_by_name(&client, &admin_cookie, "Test Site 2").await;

    assert_eq!(site2.company_id, company2.id);

    // Login as pre-created company1 admin
    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Admin1 tries to create command for company2's site (should fail)
    let new_command = json!({
        "site_id": site2.id,
        "name": "Forbidden Command",
        "description": "Should not be allowed",
        "equipment_type": "inverter",
        "equipment_id": "inverter-forbidden",
        "action": "turn_on",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/Commands")
        .cookie(admin1_session.clone())
        .json(&new_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);

    // Admin1 tries to list company2's site commands (should fail)
    let url = format!("/api/1/Sites/{}/Commands", site2.id);
    let response = client.get(&url).cookie(admin1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_admin_can_access_all_commands() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test site
    let site = get_site_by_name(&client, &admin_cookie, "Test Site 1").await;

    // Create a command as newtown admin
    let new_command = json!({
        "site_id": site.id,
        "name": "Newtown Admin Command",
        "description": "Created by newtown admin",
        "equipment_type": "battery",
        "equipment_id": "battery-1",
        "action": "charge",
        "parameters": json!({"rate": "fast"}).to_string(),
        "is_active": true
    });

    let response = client
        .post("/api/1/Commands")
        .cookie(admin_cookie.clone())
        .json(&new_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_command: Command = response.into_json().await.expect("valid command JSON");

    // Newtown admin can access any command
    let url = format!("/api/1/Commands/{}", created_command.id);
    let response = client
        .get(&url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Newtown admin can update any command
    let update_data = json!({
        "name": "Updated by Newtown Admin"
    });

    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Newtown admin can delete any command
    let response = client
        .delete(&url)
        .cookie(admin_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::NoContent);
}

#[rocket::async_test]
async fn test_duplicate_command_names_rejected() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let site = get_site_by_name(&client, &admin_cookie, "Test Site 1").await;

    // Create first command
    let command1 = json!({
        "site_id": site.id,
        "name": "Duplicate Test Command",
        "description": "First command",
        "equipment_type": "inverter",
        "equipment_id": "inverter-1",
        "action": "turn_on",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/Commands")
        .cookie(admin_cookie.clone())
        .json(&command1)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);

    // Try to create second command with same name at same site (should fail)
    let command2 = json!({
        "site_id": site.id,
        "name": "Duplicate Test Command",
        "description": "Second command - should fail",
        "equipment_type": "battery",
        "equipment_id": "battery-1",
        "action": "charge",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/Commands")
        .cookie(admin_cookie)
        .json(&command2)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Conflict);
}

#[rocket::async_test]
async fn test_command_requires_valid_site() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Try to create command for non-existent site
    let new_command = json!({
        "site_id": 99999,
        "name": "Invalid Site Command",
        "description": "Should fail",
        "equipment_type": "inverter",
        "equipment_id": "inverter-1",
        "action": "turn_on",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/Commands")
        .cookie(admin_cookie)
        .json(&new_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}
