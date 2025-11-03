use neems_api::{
    models::{Company, ScheduleCommand, Site, schedule_command::CommandType},
    orm::testing::fast_test_rocket,
};
use rocket::{
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use serde_json::json;

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
    let response = client.get("/api/1/Companies").cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    companies
        .into_iter()
        .find(|c| c.name == name)
        .expect(&format!("Company '{}' should exist from test data initialization", name))
}

/// Helper to get a test site by name
async fn get_site_by_name(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    name: &str,
) -> Site {
    let response = client.get("/api/1/Sites").cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    sites
        .into_iter()
        .find(|s| s.name == name)
        .expect(&format!("Site '{}' should exist from test data initialization", name))
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
async fn test_schedule_command_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Test all endpoints require authentication
    let response = client.get("/api/1/Sites/1/ScheduleCommands").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/ScheduleCommands/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let new_schedule_command = json!({
        "site_id": 1,
        "type": "charge",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/ScheduleCommands")
        .json(&new_schedule_command)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    let update_schedule_command = json!({
        "type": "discharge"
    });

    let response = client
        .put("/api/1/ScheduleCommands/1")
        .json(&update_schedule_command)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.delete("/api/1/ScheduleCommands/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_company_admin_can_crud_own_company_schedule_commands() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test company and site
    let company = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let site = get_site_by_name(&client, &admin_cookie, "Test Site 1").await;

    assert_eq!(site.company_id, company.id);

    // Login as pre-created company admin
    let admin_session = login_user(&client, "user@testcompany.com", "admin").await;

    // Create a schedule_command
    let new_schedule_command = json!({
        "site_id": site.id,
        "type": "charge",
        "parameters": json!({"power_level": 100}).to_string(),
        "is_active": true
    });

    let response = client
        .post("/api/1/ScheduleCommands")
        .cookie(admin_session.clone())
        .json(&new_schedule_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_schedule_command: ScheduleCommand =
        response.into_json().await.expect("valid schedule_command JSON");
    assert_eq!(created_schedule_command.type_, CommandType::Charge);
    assert_eq!(
        created_schedule_command.parameters,
        Some(json!({"power_level": 100}).to_string())
    );
    assert!(created_schedule_command.is_active);

    // Read the schedule_command
    let url = format!("/api/1/ScheduleCommands/{}", created_schedule_command.id);
    let response = client.get(&url).cookie(admin_session.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_schedule_command: ScheduleCommand =
        response.into_json().await.expect("valid schedule_command JSON");
    assert_eq!(retrieved_schedule_command.id, created_schedule_command.id);

    // List schedule_commands for the site
    let url = format!("/api/1/Sites/{}/ScheduleCommands", site.id);
    let response = client.get(&url).cookie(admin_session.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let schedule_commands: Vec<ScheduleCommand> =
        serde_json::from_value(odata_response["value"].clone())
            .expect("valid schedule_commands array");
    assert!(schedule_commands.iter().any(|c| c.id == created_schedule_command.id));

    // List only active schedule_commands
    let url = format!("/api/1/Sites/{}/ScheduleCommands?active=true", site.id);
    let response = client.get(&url).cookie(admin_session.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let active_schedule_commands: Vec<ScheduleCommand> =
        serde_json::from_value(odata_response["value"].clone())
            .expect("valid schedule_commands array");
    assert!(active_schedule_commands.iter().all(|c| c.is_active));

    // Update the schedule_command
    let update_data = json!({
        "type": "discharge",
        "parameters": json!({"power_level": 50}).to_string(),
        "is_active": false
    });

    let url = format!("/api/1/ScheduleCommands/{}", created_schedule_command.id);
    let response = client
        .put(&url)
        .cookie(admin_session.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let updated_schedule_command: ScheduleCommand =
        response.into_json().await.expect("valid schedule_command JSON");
    assert_eq!(updated_schedule_command.type_, CommandType::Discharge);
    assert_eq!(
        updated_schedule_command.parameters,
        Some(json!({"power_level": 50}).to_string())
    );
    assert!(!updated_schedule_command.is_active);

    // Delete the schedule_command
    let response = client.delete(&url).cookie(admin_session).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify schedule_command is deleted
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_company_admin_cannot_access_different_company_schedule_commands() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test companies and sites
    let _company1 = get_company_by_name(&client, &admin_cookie, "Test Company 1").await;
    let company2 = get_company_by_name(&client, &admin_cookie, "Test Company 2").await;
    let site2 = get_site_by_name(&client, &admin_cookie, "Test Site 2").await;

    assert_eq!(site2.company_id, company2.id);

    // Login as pre-created company1 admin
    let admin1_session = login_user(&client, "admin@company1.com", "admin").await;

    // Admin1 tries to create schedule_command for company2's site (should fail)
    let new_schedule_command = json!({
        "site_id": site2.id,
        "type": "charge",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/ScheduleCommands")
        .cookie(admin1_session.clone())
        .json(&new_schedule_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);

    // Admin1 tries to list company2's site schedule_commands (should fail)
    let url = format!("/api/1/Sites/{}/ScheduleCommands", site2.id);
    let response = client.get(&url).cookie(admin1_session).dispatch().await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_newtown_admin_can_access_all_schedule_commands() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get pre-created test site
    let site = get_site_by_name(&client, &admin_cookie, "Test Site 1").await;

    // Create a schedule_command as newtown admin
    let new_schedule_command = json!({
        "site_id": site.id,
        "type": "charge",
        "parameters": json!({"rate": "fast"}).to_string(),
        "is_active": true
    });

    let response = client
        .post("/api/1/ScheduleCommands")
        .cookie(admin_cookie.clone())
        .json(&new_schedule_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_schedule_command: ScheduleCommand =
        response.into_json().await.expect("valid schedule_command JSON");

    // Newtown admin can access any schedule_command
    let url = format!("/api/1/ScheduleCommands/{}", created_schedule_command.id);
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    // Newtown admin can update any schedule_command
    let update_data = json!({
        "type": "discharge"
    });

    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Newtown admin can delete any schedule_command
    let response = client.delete(&url).cookie(admin_cookie).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);
}

#[rocket::async_test]
async fn test_multiple_schedule_commands_same_site() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let site = get_site_by_name(&client, &admin_cookie, "Test Site 1").await;

    // Create first schedule_command
    let schedule_command1 = json!({
        "site_id": site.id,
        "type": "charge",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/ScheduleCommands")
        .cookie(admin_cookie.clone())
        .json(&schedule_command1)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);

    // Create second schedule_command with same type at same site (should succeed -
    // no unique constraint)
    let schedule_command2 = json!({
        "site_id": site.id,
        "type": "charge",
        "parameters": json!({"rate": "fast"}).to_string(),
        "is_active": true
    });

    let response = client
        .post("/api/1/ScheduleCommands")
        .cookie(admin_cookie)
        .json(&schedule_command2)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
}

#[rocket::async_test]
async fn test_schedule_command_requires_valid_site() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Try to create schedule_command for non-existent site
    let new_schedule_command = json!({
        "site_id": 99999,
        "type": "charge",
        "parameters": null,
        "is_active": true
    });

    let response = client
        .post("/api/1/ScheduleCommands")
        .cookie(admin_cookie)
        .json(&new_schedule_command)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}
