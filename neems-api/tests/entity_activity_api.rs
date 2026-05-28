//! Integration tests for the EntityActivity audit-log read endpoint
//! (B6 in the demo implementation plan).
//!
//! The endpoint is read-only; tests verify that
//!   1. unauthenticated callers get 401,
//!   2. activity rows surface for an entity created during the test,
//!   3. the acting user's email is resolved server-side,
//!   4. row ordering is oldest-first,
//!   5. unknown entities return an empty list (no 404).

use neems_api::{
    api::entity_activity::EntityActivityWithUser, models::ScheduleLibraryItem,
    orm::testing::fast_test_rocket,
};
use rocket::{
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use serde_json::json;

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
async fn entity_activity_requires_auth() {
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let response = client
        .get("/api/1/EntityActivity?table_name=schedule_templates&entity_id=1")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn entity_activity_returns_history_with_user_email() {
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let admin = login_admin(&client).await;

    // Create a library item — that should write a 'create' activity row.
    let new_item = json!({
        "name": "EntityActivity Test Schedule",
        "commands": []
    });
    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin.clone())
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    let item: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    // Update the item — that should write an 'update' activity row.
    let update_body = json!({
        "name": "EntityActivity Test Schedule (renamed)",
        "description": null,
        "commands": null
    });
    let response = client
        .put(format!("/api/1/ScheduleLibraryItems/{}", item.id))
        .cookie(admin.clone())
        .json(&update_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    // Read the audit log back.
    let url = format!("/api/1/EntityActivity?table_name=schedule_templates&entity_id={}", item.id);
    let response = client.get(&url).cookie(admin.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let rows: Vec<EntityActivityWithUser> = response.into_json().await.expect("valid JSON");

    // We expect at least one create + one update row, ordered oldest first.
    assert!(rows.len() >= 2, "expected ≥2 activity rows for item, got {}", rows.len());

    let create_row = rows.iter().find(|r| r.operation_type == "create");
    let update_row = rows.iter().find(|r| r.operation_type == "update");
    assert!(create_row.is_some(), "no 'create' activity row found");
    assert!(update_row.is_some(), "no 'update' activity row found");

    // The acting user's email should be resolved server-side. The
    // existing trigger + backfill helper writes user_id within ~2s of
    // the trigger firing, and the resolver looks the email up off
    // user_id, so by the time this read lands it should be populated.
    let actor_emails: Vec<_> = rows.iter().filter_map(|r| r.user_email.as_deref()).collect();
    assert!(
        actor_emails.iter().any(|e| e.contains("superadmin")),
        "expected superadmin@example.com to appear as actor, got {actor_emails:?}"
    );

    // Oldest-first ordering: the timestamps should be monotonically
    // non-decreasing as we walk the list.
    for pair in rows.windows(2) {
        assert!(
            pair[0].timestamp <= pair[1].timestamp,
            "rows not ordered oldest-first: {} > {}",
            pair[0].timestamp,
            pair[1].timestamp
        );
    }
}

#[rocket::async_test]
async fn entity_activity_records_commands_only_update() {
    // Regression: a library-item edit that only swaps commands (name &
    // description unchanged — the F4 inline-edit path on the calendar)
    // used to skip the schedule_templates update entirely, so the
    // update trigger never fired and the Resulting Schedule pane's
    // audit timeline stayed stuck at the original 'create' row.
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let admin = login_admin(&client).await;

    let new_item = json!({
        "name": "Commands-only update test",
        "commands": [
            { "execution_offset_seconds": 0, "command_type": "charge", "duration_seconds": 3600, "target_soc_percent": null }
        ]
    });
    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin.clone())
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    let item: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    // Update only the commands. name and description stay null — this
    // mirrors what DayDetailsDialog sends from the inline-edit path.
    let update_body = json!({
        "name": null,
        "description": null,
        "commands": [
            { "execution_offset_seconds": 0, "command_type": "charge", "duration_seconds": 7200, "target_soc_percent": null }
        ]
    });
    let response = client
        .put(format!("/api/1/ScheduleLibraryItems/{}", item.id))
        .cookie(admin.clone())
        .json(&update_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    // Audit log should now have both a 'create' and an 'update' row.
    let url = format!("/api/1/EntityActivity?table_name=schedule_templates&entity_id={}", item.id);
    let response = client.get(&url).cookie(admin.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let rows: Vec<EntityActivityWithUser> = response.into_json().await.expect("valid JSON");

    let creates = rows.iter().filter(|r| r.operation_type == "create").count();
    let updates = rows.iter().filter(|r| r.operation_type == "update").count();
    assert!(creates >= 1, "expected ≥1 create row, got {creates} in {rows:?}");
    assert!(
        updates >= 1,
        "expected ≥1 update row for a commands-only edit, got {updates} in {rows:?}"
    );

    // The update row should have the acting user backfilled.
    let update_row = rows.iter().find(|r| r.operation_type == "update").unwrap();
    assert!(
        update_row.user_email.as_deref().is_some_and(|e| e.contains("superadmin")),
        "expected superadmin email on commands-only update row, got {:?}",
        update_row.user_email
    );
}

#[rocket::async_test]
async fn entity_activity_unknown_entity_returns_empty_list() {
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let admin = login_admin(&client).await;

    // entity_id 9_999_999 should not exist — the endpoint should still
    // return 200 with an empty array, not 404. This keeps the frontend
    // code path simple (single render path for "no audit history").
    let response = client
        .get("/api/1/EntityActivity?table_name=schedule_templates&entity_id=9999999")
        .cookie(admin.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let rows: Vec<EntityActivityWithUser> = response.into_json().await.expect("valid JSON");
    assert!(
        rows.is_empty(),
        "expected empty list for unknown entity, got {} rows",
        rows.len()
    );
}

#[rocket::async_test]
async fn entity_activity_missing_query_params_returns_400_or_422() {
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let admin = login_admin(&client).await;

    // table_name omitted — Rocket's FromForm rejects the request before
    // it reaches the handler; we just verify the response is some 4xx
    // and not 5xx or 2xx (Status doesn't implement structural
    // PartialEq, so we compare codes directly).
    let response = client
        .get("/api/1/EntityActivity?entity_id=1")
        .cookie(admin.clone())
        .dispatch()
        .await;
    let code = response.status().code;
    assert!((400..500).contains(&code), "expected 4xx for missing query param, got {code}");
}
