use neems_api::{
    models::{CommandType, ScheduleLibraryItem},
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

#[rocket::async_test]
async fn test_schedule_library_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Test all endpoints require authentication
    let response = client.get("/api/1/Sites/1/ScheduleLibraryItems").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/ScheduleLibraryItems/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let new_item = json!({
        "name": "Test Schedule",
        "commands": []
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_default_schedule_auto_created() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // List schedules for a site - should auto-create default
    let response = client
        .get("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let items: Vec<ScheduleLibraryItem> = response.into_json().await.expect("valid JSON");

    // Should have at least one schedule (the auto-created default)
    assert!(!items.is_empty());

    // Find the default schedule
    let default = items.iter().find(|item| item.name == "Default");
    assert!(default.is_some(), "Default schedule should be auto-created");
}

#[rocket::async_test]
async fn test_create_library_item() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let new_item = json!({
        "name": "Morning Schedule",
        "description": "Charge in morning, discharge in evening",
        "commands": [
            {
                "execution_offset_seconds": 28800, // 8:00 AM
                "command_type": "charge"
            },
            {
                "execution_offset_seconds": 64800, // 6:00 PM
                "command_type": "discharge"
            }
        ]
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let item: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    assert_eq!(item.name, "Morning Schedule");
    assert_eq!(item.description, Some("Charge in morning, discharge in evening".to_string()));
    assert_eq!(item.commands.len(), 2);
    assert_eq!(item.commands[0].execution_offset_seconds, 28800);
    assert_eq!(item.commands[0].command_type, CommandType::Charge);
}

#[rocket::async_test]
async fn test_create_library_item_duplicate_name() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let new_item = json!({
        "name": "Duplicate Test",
        "commands": []
    });

    // Create first item
    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);

    // Try to create second item with same name
    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
}

#[rocket::async_test]
async fn test_create_library_item_invalid_execution_time() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Execution offset >= 86400 (24 hours) should fail
    let new_item = json!({
        "name": "Invalid Time",
        "commands": [
            {
                "execution_offset_seconds": 90000, // > 24 hours
                "command_type": "charge"
            }
        ]
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::InternalServerError);
}

#[rocket::async_test]
async fn test_create_library_item_duplicate_execution_times() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    let new_item = json!({
        "name": "Duplicate Times",
        "commands": [
            {
                "execution_offset_seconds": 28800,
                "command_type": "charge"
            },
            {
                "execution_offset_seconds": 28800, // Same time!
                "command_type": "discharge"
            }
        ]
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::InternalServerError);
}

#[rocket::async_test]
async fn test_get_library_item() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create an item first
    let new_item = json!({
        "name": "Get Test",
        "commands": []
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    let created: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    // Get it by ID
    let url = format!("/api/1/ScheduleLibraryItems/{}", created.id);
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let item: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");
    assert_eq!(item.id, created.id);
    assert_eq!(item.name, "Get Test");
}

#[rocket::async_test]
async fn test_update_library_item() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create an item
    let new_item = json!({
        "name": "Update Test",
        "commands": []
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    let created: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    // Update it
    let update = json!({
        "name": "Updated Name",
        "description": "New description",
        "commands": [
            {
                "execution_offset_seconds": 43200,
                "command_type": "trickle_charge"
            }
        ]
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}", created.id);
    let response = client.put(&url).cookie(admin_cookie.clone()).json(&update).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let updated: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");
    assert_eq!(updated.name, "Updated Name");
    assert_eq!(updated.description, Some("New description".to_string()));
    assert_eq!(updated.commands.len(), 1);
}

#[rocket::async_test]
async fn test_update_default_schedule_allowed() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get the default schedule
    let response = client
        .get("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    let items: Vec<ScheduleLibraryItem> = response.into_json().await.expect("valid JSON");
    let default = items.iter().find(|item| item.name == "Default").expect("Default should exist");

    // Update the default schedule - should be allowed
    let update = json!({
        "name": "Custom Default Name",
        "commands": [
            {
                "execution_offset_seconds": 0,
                "command_type": "charge"
            }
        ]
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}", default.id);
    let response = client.put(&url).cookie(admin_cookie.clone()).json(&update).dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let updated: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");
    assert_eq!(updated.name, "Custom Default Name");
    assert_eq!(updated.commands.len(), 1);
}

#[rocket::async_test]
async fn test_delete_library_item() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create an item
    let new_item = json!({
        "name": "Delete Test",
        "commands": []
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    let created: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    // Delete it
    let url = format!("/api/1/ScheduleLibraryItems/{}", created.id);
    let response = client.delete(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify it's gone
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_cannot_delete_default_schedule() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get the default schedule
    let response = client
        .get("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    let items: Vec<ScheduleLibraryItem> = response.into_json().await.expect("valid JSON");
    let default = items.iter().find(|item| item.name == "Default").expect("Default should exist");

    // Try to delete it - should fail
    let url = format!("/api/1/ScheduleLibraryItems/{}", default.id);
    let response = client.delete(&url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::BadRequest);
}

#[rocket::async_test]
async fn test_clone_library_item() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create an item
    let new_item = json!({
        "name": "Clone Source",
        "description": "Original",
        "commands": [
            {
                "execution_offset_seconds": 10800,
                "command_type": "charge"
            }
        ]
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;
    let created: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    // Clone it
    let clone_request = json!({
        "name": "Cloned Schedule",
        "description": "Cloned version"
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/Clone", created.id);
    let response = client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&clone_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let cloned: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    assert_eq!(cloned.name, "Cloned Schedule");
    assert_eq!(cloned.description, Some("Cloned version".to_string()));
    assert_eq!(cloned.commands.len(), 1);
    assert_eq!(cloned.commands[0].execution_offset_seconds, 10800);
    assert_ne!(cloned.id, created.id);
}

#[rocket::async_test]
async fn test_list_library_items() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a few items
    for i in 1..=3 {
        let new_item = json!({
            "name": format!("Schedule {}", i),
            "commands": []
        });

        client
            .post("/api/1/Sites/1/ScheduleLibraryItems")
            .cookie(admin_cookie.clone())
            .json(&new_item)
            .dispatch()
            .await;
    }

    // List all items
    let response = client
        .get("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let items: Vec<ScheduleLibraryItem> = response.into_json().await.expect("valid JSON");

    // Should have at least 4 items (3 created + 1 auto-created default)
    assert!(items.len() >= 4);
}
