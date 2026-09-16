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
    api::entity_activity::EntityActivityWithUser,
    models::{ApplicationRule, CommandChangeKind, CommandType, ScheduleLibraryItem},
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

#[rocket::async_test]
async fn entity_activity_details_describe_a_command_edit() {
    // #136: the row used to say only that an update happened. It now
    // carries the before/after of every command that moved, so the UI
    // can say "Removed the 21:00 charge" instead of "Edited commands".
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let admin = login_admin(&client).await;

    let new_item = json!({
        "name": "Change details test schedule",
        "commands": [
            // 16:00 discharge for 4 h — this one gets shortened.
            { "execution_offset_seconds": 57600, "command_type": "discharge", "duration_seconds": 14400, "target_soc_percent": null },
            // 21:00 charge — this one gets dropped.
            { "execution_offset_seconds": 75600, "command_type": "charge", "duration_seconds": 3600, "target_soc_percent": 80 }
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

    let update_body = json!({
        "name": null,
        "description": null,
        "commands": [
            { "execution_offset_seconds": 57600, "command_type": "discharge", "duration_seconds": 7200, "target_soc_percent": null },
            // A new 02:00 trickle charge.
            { "execution_offset_seconds": 7200, "command_type": "trickle_charge", "duration_seconds": 1800, "target_soc_percent": null }
        ]
    });
    let response = client
        .put(format!("/api/1/ScheduleLibraryItems/{}", item.id))
        .cookie(admin.clone())
        .json(&update_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let url = format!("/api/1/EntityActivity?table_name=schedule_templates&entity_id={}", item.id);
    let response = client.get(&url).cookie(admin.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let rows: Vec<EntityActivityWithUser> = response.into_json().await.expect("valid JSON");

    // The create row records the shape the schedule started with —
    // once it has been edited, nothing else remembers that.
    let create_row = rows.iter().find(|r| r.operation_type == "create").expect("a create row");
    let created = create_row.change_details.as_ref().expect("details on the create row");
    assert_eq!(created.commands.len(), 2);
    assert!(created.commands.iter().all(|c| c.kind == CommandChangeKind::Added));

    let update_row = rows.iter().find(|r| r.operation_type == "update").expect("an update row");
    let details = update_row.change_details.as_ref().expect("details on the update row");
    assert!(details.fields.is_empty(), "nothing but commands changed");

    let by_offset = |offset: i32| {
        details
            .commands
            .iter()
            .find(|c| c.command.execution_offset_seconds == offset)
            .unwrap_or_else(|| panic!("no change recorded at offset {offset}: {details:?}"))
    };

    let shortened = by_offset(57600);
    assert_eq!(shortened.kind, CommandChangeKind::Modified);
    assert_eq!(shortened.command.duration_seconds, Some(7200));
    assert_eq!(shortened.previous.as_ref().and_then(|p| p.duration_seconds), Some(14400));

    let dropped = by_offset(75600);
    assert_eq!(dropped.kind, CommandChangeKind::Removed);
    assert_eq!(dropped.command.command_type, CommandType::Charge);
    assert_eq!(dropped.command.target_soc_percent, Some(80));

    let added = by_offset(7200);
    assert_eq!(added.kind, CommandChangeKind::Added);
    assert_eq!(added.command.command_type, CommandType::TrickleCharge);
}

#[rocket::async_test]
async fn entity_activity_details_name_the_dates_a_rule_covers() {
    // #136: the site-wide activity feed has no calendar context, so
    // without the rule's dates on the row it can only say "Applied".
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let admin = login_admin(&client).await;

    let new_item = json!({ "name": "Rule details test schedule", "commands": [] });
    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin.clone())
        .json(&new_item)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    let item: ScheduleLibraryItem = response.into_json().await.expect("valid JSON");

    let rule_body = json!({
        "rule_type": "specific_date",
        "days_of_week": null,
        "specific_dates": ["2026-07-04"],
        "override_reason": "Holiday load profile"
    });
    let response = client
        .post(format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item.id))
        .cookie(admin.clone())
        .json(&rule_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);
    let rule: ApplicationRule = response.into_json().await.expect("valid JSON");

    let url = format!("/api/1/EntityActivity?table_name=application_rules&entity_id={}", rule.id);
    let response = client.get(&url).cookie(admin.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let rows: Vec<EntityActivityWithUser> = response.into_json().await.expect("valid JSON");

    let create_row = rows.iter().find(|r| r.operation_type == "create").expect("a create row");
    let details = create_row.change_details.as_ref().expect("details on the create row");
    let date_field = details
        .fields
        .iter()
        .find(|f| f.field == "specific_dates")
        .expect("specific_dates field");
    assert_eq!(date_field.to.as_deref(), Some("2026-07-04"));
    assert!(date_field.from.is_none(), "a created rule has no previous value");

    // Deleting the rule records the same values on the other side, so
    // "Removed the 2026-07-04 override" survives the row's deletion.
    let response = client
        .delete(format!("/api/1/ApplicationRules/{}", rule.id))
        .cookie(admin.clone())
        .dispatch()
        .await;
    assert!(response.status() == Status::Ok || response.status() == Status::NoContent);

    let response = client.get(&url).cookie(admin.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let rows: Vec<EntityActivityWithUser> = response.into_json().await.expect("valid JSON");
    let delete_row = rows.iter().find(|r| r.operation_type == "delete").expect("a delete row");
    let details = delete_row.change_details.as_ref().expect("details on the delete row");
    let date_field = details
        .fields
        .iter()
        .find(|f| f.field == "specific_dates")
        .expect("specific_dates field");
    assert_eq!(date_field.from.as_deref(), Some("2026-07-04"));
    assert!(date_field.to.is_none(), "a deleted rule has no new value");
}

#[rocket::async_test]
async fn entity_activity_records_one_row_for_one_save() {
    // Regression: name and description used to be written by separate
    // UPDATE statements, so the trigger fired twice and one save left
    // two activity rows — the backfill helpers landed on one of them
    // and the other rendered as a change with nothing in it.
    let client = Client::tracked(fast_test_rocket()).await.expect("rocket");
    let admin = login_admin(&client).await;

    let new_item = json!({
        "name": "One row per save",
        "description": "before",
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

    let update_body = json!({
        "name": "One row per save (renamed)",
        "description": "after",
        "commands": null,
        "change_reason": "renaming and redescribing at once"
    });
    let response = client
        .put(format!("/api/1/ScheduleLibraryItems/{}", item.id))
        .cookie(admin.clone())
        .json(&update_body)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);

    let url = format!("/api/1/EntityActivity?table_name=schedule_templates&entity_id={}", item.id);
    let response = client.get(&url).cookie(admin.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let rows: Vec<EntityActivityWithUser> = response.into_json().await.expect("valid JSON");

    let updates: Vec<_> = rows.iter().filter(|r| r.operation_type == "update").collect();
    assert_eq!(updates.len(), 1, "one save should leave one update row, got {updates:?}");

    // And that single row accounts for both fields, rather than one of
    // them going missing with the row that no longer exists.
    let details = updates[0].change_details.as_ref().expect("details on the update row");
    let changed: Vec<&str> = details.fields.iter().map(|f| f.field.as_str()).collect();
    assert!(changed.contains(&"name"), "name change missing from {details:?}");
    assert!(changed.contains(&"description"), "description change missing from {details:?}");
    assert_eq!(updates[0].change_reason.as_deref(), Some("renaming and redescribing at once"));
}
