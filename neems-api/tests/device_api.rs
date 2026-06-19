use neems_api::{
    models::{Company, Device, Site},
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
async fn test_device_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Test all endpoints require authentication
    let response = client.get("/api/1/Devices").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/Devices/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let new_device = json!({
        "name": "Test Inverter",
        "type_": "Inverter",
        "model": "SUN2000-100KTL",
        "company_id": 1,
        "site_id": 1
    });

    let response = client.post("/api/1/Devices").json(&new_device).dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let update_device = json!({
        "name": "Updated Inverter"
    });

    let response = client.put("/api/1/Devices/1").json(&update_device).dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.delete("/api/1/Devices/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/Devices/1/Site").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_create_device_success() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    let new_device = json!({
        "name": "Main Solar Inverter",
        "description": "Primary inverter for rooftop solar array",
        "type_": "Inverter",
        "model": "SUN2000-100KTL",
        "serial": "INV20240001",
        "ip_address": "192.168.1.100",
        "install_date": "2024-03-15T10:00:00",
        "company_id": company.id,
        "site_id": site.id
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_device: Device = response.into_json().await.expect("valid device JSON");

    assert_eq!(created_device.name, "Main Solar Inverter");
    assert_eq!(
        created_device.description,
        Some("Primary inverter for rooftop solar array".to_string())
    );
    assert_eq!(created_device.type_, "Inverter");
    assert_eq!(created_device.model, "SUN2000-100KTL");
    assert_eq!(created_device.serial, Some("INV20240001".to_string()));
    assert_eq!(created_device.ip_address, Some("192.168.1.100".to_string()));
    assert_eq!(created_device.company_id, company.id);
    assert_eq!(created_device.site_id, site.id);
}

#[rocket::async_test]
async fn test_create_device_defaults_name_to_type() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    let new_device = json!({
        "type_": "Battery",
        "model": "PowerWall-2",
        "company_id": company.id,
        "site_id": site.id
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_device: Device = response.into_json().await.expect("valid device JSON");

    assert_eq!(created_device.name, "Battery"); // Should default to type_
    assert_eq!(created_device.type_, "Battery");
    assert_eq!(created_device.model, "PowerWall-2");
}

#[rocket::async_test]
async fn test_create_device_duplicate_name_fails() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    let device1 = json!({
        "name": "Unique Device Name",
        "type_": "Meter",
        "model": "SEL-735",
        "company_id": company.id,
        "site_id": site.id
    });

    // Create first device
    let response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&device1)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);

    // Try to create second device with same name at same site
    let device2 = json!({
        "name": "Unique Device Name",
        "type_": "Inverter",
        "model": "Different Model",
        "company_id": company.id,
        "site_id": site.id
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&device2)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
    let error: serde_json::Value = response.into_json().await.expect("valid error JSON");
    assert!(error["error"].as_str().unwrap().contains("already exists"));
}

#[rocket::async_test]
async fn test_create_device_invalid_site_fails() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;

    let new_device = json!({
        "name": "Test Device",
        "type_": "Meter",
        "model": "SEL-735",
        "company_id": company.id,
        "site_id": 99999 // Non-existent site
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::NotFound);
    let error: serde_json::Value = response.into_json().await.expect("valid error JSON");
    assert!(error["error"].as_str().unwrap().contains("Site not found"));
}

#[rocket::async_test]
async fn test_list_devices_success() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;

    let response = client.get("/api/1/Devices").cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");

    // Check OData structure
    assert!(odata_response["@odata.context"].is_string());
    assert!(odata_response["value"].is_array());

    let context = odata_response["@odata.context"].as_str().unwrap();
    assert!(context.contains("#Devices"));
}

#[rocket::async_test]
async fn test_get_device_success() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    // Create a device first
    let new_device = json!({
        "name": "Test Device for Get",
        "type_": "Sensor",
        "model": "TEMP-01",
        "company_id": company.id,
        "site_id": site.id
    });

    let create_response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(create_response.status(), Status::Created);
    let created_device: Device = create_response.into_json().await.expect("valid device JSON");

    // Now get the device
    let url = format!("/api/1/Devices/{}", created_device.id);
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_device: Device = response.into_json().await.expect("valid device JSON");

    assert_eq!(retrieved_device.id, created_device.id);
    assert_eq!(retrieved_device.name, "Test Device for Get");
    assert_eq!(retrieved_device.type_, "Sensor");
    assert_eq!(retrieved_device.model, "TEMP-01");
}

#[rocket::async_test]
async fn test_get_device_not_found() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;

    let response = client
        .get("/api/1/Devices/99999") // Non-existent device
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_update_device_success() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    // Create a device first
    let new_device = json!({
        "name": "Device to Update",
        "type_": "Meter",
        "model": "OLD-MODEL",
        "company_id": company.id,
        "site_id": site.id
    });

    let create_response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(create_response.status(), Status::Created);
    let created_device: Device = create_response.into_json().await.expect("valid device JSON");

    // Update the device
    let update_data = json!({
        "name": "Updated Device Name",
        "model": "NEW-MODEL",
        "description": "Updated description"
    });

    let url = format!("/api/1/Devices/{}", created_device.id);
    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let updated_device: Device = response.into_json().await.expect("valid device JSON");

    assert_eq!(updated_device.id, created_device.id);
    assert_eq!(updated_device.name, "Updated Device Name");
    assert_eq!(updated_device.model, "NEW-MODEL");
    assert_eq!(updated_device.description, Some("Updated description".to_string()));
    assert_eq!(updated_device.type_, "Meter"); // Should remain unchanged
}

#[rocket::async_test]
async fn test_delete_device_success() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    // Create a device first
    let new_device = json!({
        "name": "Device to Delete",
        "type_": "UPS",
        "model": "APC-1000",
        "company_id": company.id,
        "site_id": site.id
    });

    let create_response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(create_response.status(), Status::Created);
    let created_device: Device = create_response.into_json().await.expect("valid device JSON");

    // Delete the device
    let url = format!("/api/1/Devices/{}", created_device.id);
    let response = client.delete(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify device is deleted
    let url = format!("/api/1/Devices/{}", created_device.id);
    let get_response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(get_response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_get_device_site_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    // Create a device first
    let new_device = json!({
        "name": "Device for Navigation Test",
        "type_": "Gateway",
        "model": "COMM-100",
        "company_id": company.id,
        "site_id": site.id
    });

    let create_response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(create_response.status(), Status::Created);
    let created_device: Device = create_response.into_json().await.expect("valid device JSON");

    // Get the device's site via navigation
    let url = format!("/api/1/Devices/{}/Site", created_device.id);
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let retrieved_site: Site = response.into_json().await.expect("valid site JSON");

    assert_eq!(retrieved_site.id, site.id);
    assert_eq!(retrieved_site.name, site.name);
}

#[rocket::async_test]
async fn test_device_rbac_company_admin() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Login as company admin for Device Test Company A
    let company_admin_cookie = login_user(&client, "admin@devicetesta.com", "admin").await;

    let response = client
        .get("/api/1/Companies")
        .cookie(company_admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    let company1 = companies
        .iter()
        .find(|c| c.name == "Device Test Company A")
        .expect("Device Test Company A should exist");

    let response = client.get("/api/1/Sites").cookie(company_admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    let site1 = sites
        .iter()
        .find(|s| s.company_id == company1.id)
        .expect("Company 1 should have a site");

    // Company admin should be able to create devices in their company
    let new_device = json!({
        "name": "Company Admin Device",
        "type_": "Controller",
        "model": "CTRL-200",
        "company_id": company1.id,
        "site_id": site1.id
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(company_admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let created_device: Device = response.into_json().await.expect("valid device JSON");

    // Company admin should be able to view their devices
    let response = client
        .get("/api/1/Devices")
        .cookie(company_admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let devices: Vec<Device> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid devices array");

    // Should only see devices from their company
    for device in &devices {
        assert_eq!(device.company_id, company1.id);
    }

    // Company admin should be able to update their devices
    let update_data = json!({
        "description": "Updated by company admin"
    });

    let url = format!("/api/1/Devices/{}", created_device.id);
    let response = client
        .put(&url)
        .cookie(company_admin_cookie.clone())
        .json(&update_data)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Company admin should be able to delete their devices
    let url = format!("/api/1/Devices/{}", created_device.id);
    let response = client.delete(&url).cookie(company_admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);
}

#[rocket::async_test]
async fn test_device_rbac_regular_staff() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Login as regular staff member
    let staff_cookie = login_user(&client, "staff@testcompany.com", "admin").await;

    // Staff should be able to view devices (but only in their company)
    let response = client.get("/api/1/Devices").cookie(staff_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let devices: Vec<Device> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid devices array");

    // Staff user from Test Company 1 should see no devices (devices moved to Device
    // Test companies)
    let staff_company_id = 2; // Test Company 1 has ID 2 in test data
    assert_eq!(devices.len(), 0); // No devices should exist in Test Company 1 anymore
    for device in &devices {
        assert_eq!(device.company_id, staff_company_id);
    }

    // Staff should NOT be able to create devices
    let new_device = json!({
        "name": "Staff Device",
        "type_": "Sensor",
        "model": "SENS-100",
        "company_id": staff_company_id,
        "site_id": 1
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(staff_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[rocket::async_test]
async fn test_device_rbac_newtown_admin() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Login as newtown admin
    let newtown_admin_cookie = login_user(&client, "newtownadmin@newtown.com", "admin").await;

    // Newtown admin should be able to view ALL devices across companies
    let response = client
        .get("/api/1/Devices")
        .cookie(newtown_admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let devices: Vec<Device> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid devices array");

    // Should see devices from multiple companies
    let company_ids: std::collections::HashSet<i32> =
        devices.iter().map(|d| d.company_id).collect();
    // Should have devices from at least Test Company 1 and Test Company 2
    assert!(
        company_ids.len() >= 2,
        "Newtown admin should see devices from multiple companies"
    );

    // Newtown admin should be able to create devices in any company
    let response = client
        .get("/api/1/Companies")
        .cookie(newtown_admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");
    let company2 = companies
        .iter()
        .find(|c| c.name == "Device Test Company B")
        .expect("Device Test Company B should exist");

    let response = client.get("/api/1/Sites").cookie(newtown_admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let sites: Vec<Site> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid sites array");
    let site2 = sites
        .iter()
        .find(|s| s.company_id == company2.id)
        .expect("Device Test Company B should have a site");

    let new_device = json!({
        "name": "Newtown Admin Device",
        "type_": "Monitor",
        "model": "MON-500",
        "company_id": company2.id,
        "site_id": site2.id
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(newtown_admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
}

/// Helper to create a device and return the parsed result.
async fn create_device(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    company_id: i32,
    site_id: i32,
    name: &str,
    type_: &str,
    model: &str,
) -> Device {
    let new_device = json!({
        "name": name,
        "type_": type_,
        "model": model,
        "company_id": company_id,
        "site_id": site_id
    });

    let response = client
        .post("/api/1/Devices")
        .cookie(admin_cookie.clone())
        .json(&new_device)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.expect("valid device JSON")
}

/// Helper to GET /Devices with a query string and return the parsed OData body.
async fn list_devices_query(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    query: &str,
) -> serde_json::Value {
    let response = client
        .get(format!("/api/1/Devices?{query}"))
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.expect("valid OData JSON")
}

#[rocket::async_test]
async fn test_list_devices_odata_filter_and_select() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    create_device(&client, &admin_cookie, company.id, site.id, "ZZQ Alpha", "Inverter", "QM-1")
        .await;
    create_device(&client, &admin_cookie, company.id, site.id, "ZZQ Bravo", "Sensor", "QM-2").await;

    // $filter by exact name returns only the matching device (URI-encoded
    // spaces and quotes).
    let body =
        list_devices_query(&client, &admin_cookie, "$filter=name%20eq%20%27ZZQ%20Bravo%27").await;
    let value = body["value"].as_array().expect("value array");
    assert_eq!(value.len(), 1);
    assert_eq!(value[0]["name"], "ZZQ Bravo");

    // $filter eq combined with $select returns only the requested property.
    let body = list_devices_query(
        &client,
        &admin_cookie,
        "$filter=name%20eq%20%27ZZQ%20Alpha%27&$select=name",
    )
    .await;
    let value = body["value"].as_array().expect("value array");
    assert_eq!(value.len(), 1);
    let obj = value[0].as_object().expect("device object");
    assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("ZZQ Alpha"));
    assert!(obj.get("model").is_none(), "$select=name should drop other fields");
    assert!(obj.get("id").is_none(), "$select=name should drop other fields");
}

#[rocket::async_test]
async fn test_list_devices_odata_orderby_count_and_pagination() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    let company = get_company_by_name(&client, &admin_cookie, "Device Test Company A").await;
    let site = get_site_by_name(&client, &admin_cookie, "Device API Site A").await;

    create_device(&client, &admin_cookie, company.id, site.id, "PgN Gamma", "Sensor", "PG-3").await;
    create_device(&client, &admin_cookie, company.id, site.id, "PgN Alpha", "Sensor", "PG-1").await;
    create_device(&client, &admin_cookie, company.id, site.id, "PgN Beta", "Sensor", "PG-2").await;

    // $orderby ascending yields a name-sorted collection.
    let body = list_devices_query(&client, &admin_cookie, "$orderby=name").await;
    let names: Vec<String> = body["value"]
        .as_array()
        .expect("value array")
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_string())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "$orderby=name should return ascending order");

    // $count reports the full filtered total, independent of $top.
    let body = list_devices_query(&client, &admin_cookie, "$count=true&$top=1").await;
    let total = body["@odata.count"].as_i64().expect("@odata.count present");
    assert_eq!(body["value"].as_array().expect("value array").len(), 1);
    assert!(total >= 3, "count should reflect all devices, not just the $top page");

    // $skip + $top page through the ordered collection.
    let page = list_devices_query(&client, &admin_cookie, "$orderby=name&$top=2&$skip=1").await;
    let page_names: Vec<String> = page["value"]
        .as_array()
        .expect("value array")
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(page_names.len(), 2);
    assert_eq!(page_names, &sorted[1..3], "page should be items 2 and 3 of the sorted list");
}

#[rocket::async_test]
async fn test_list_devices_invalid_query_rejected() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // $top out of range is rejected by ODataQuery::validate.
    let response = client
        .get("/api/1/Devices?$top=99999")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
}
