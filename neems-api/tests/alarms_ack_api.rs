//! Integration tests for the alarm acknowledgement endpoints.
//!
//! Covers the round trip an operator actually makes: see an alarm on
//! `/Alarms/Active`, acknowledge it via `/Alarms/Acknowledge`, and observe the
//! latched status change — plus the audit trail that acknowledgement leaves in
//! `/Alarms/History`.
//!
//! Alarm 401 (`fire_alarm`, Emergency) is driven through the demo forced-alarm
//! endpoint so these tests don't depend on a live RTAC feed. The fast test
//! fixture has no readings carrying alarm registers, so a forced alarm is the
//! only thing in the active set and there are no recorded rising/falling edges.

use chrono::{Duration, SecondsFormat, Utc};
use neems_api::orm::testing::fast_test_rocket;
use rocket::{http::Status, local::asynchronous::Client, tokio};
use serde_json::{Value, json};

/// Emergency-level alarm used throughout; any defined alarm_num would do.
const ALARM: u16 = 401;

async fn login_as(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let body = json!({ "email": email, "password": password });
    let resp = client.post("/api/1/login").json(&body).dispatch().await;
    assert_eq!(resp.status(), Status::Ok, "login failed for {}", email);
    resp.cookies().get("session").expect("session cookie").clone().into_owned()
}

/// Force `alarm_nums` on via the demo endpoint (requires a demo-capable role).
async fn force_alarms(client: &Client, session: &rocket::http::Cookie<'static>, nums: &[u16]) {
    let resp = client
        .put("/api/1/Alarms/Forced")
        .cookie(session.clone())
        .json(&json!({ "alarm_nums": nums }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "forcing alarms {:?} failed", nums);
}

/// Fetch `/Alarms/Active` and return the entry for `alarm_num`, if visible.
async fn active_entry(
    client: &Client,
    session: &rocket::http::Cookie<'static>,
    alarm_num: u16,
) -> Option<Value> {
    let resp = client.get("/api/1/Alarms/Active").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");
    body["alarms"]
        .as_array()
        .expect("alarms array")
        .iter()
        .find(|a| a["alarm_num"].as_u64() == Some(alarm_num as u64))
        .cloned()
}

async fn acknowledge<'c>(
    client: &'c Client,
    session: &rocket::http::Cookie<'static>,
    alarm_num: u16,
    note: Option<&str>,
) -> rocket::local::asynchronous::LocalResponse<'c> {
    let body = match note {
        Some(n) => json!({ "alarm_num": alarm_num, "note": n }),
        None => json!({ "alarm_num": alarm_num }),
    };
    client
        .post("/api/1/Alarms/Acknowledge")
        .cookie(session.clone())
        .json(&body)
        .dispatch()
        .await
}

/// The core round trip: an unacknowledged active alarm reports `Active`, and
/// after acknowledgement it stays visible as `AcknowledgedActive` (still
/// physically present) carrying who acknowledged it and when.
#[tokio::test]
async fn acknowledging_an_active_alarm_latches_it_as_acknowledged() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    force_alarms(&client, &session, &[ALARM]).await;

    // Before acknowledgement: active and unacknowledged.
    let before = active_entry(&client, &session, ALARM)
        .await
        .expect("alarm 401 should be active");
    assert_eq!(before["status"], json!("Active"));
    assert_eq!(before["data_active"], json!(true));
    assert_eq!(before["acknowledged_at"], Value::Null);
    assert_eq!(before["acknowledged_by_user_id"], Value::Null);

    // Acknowledge it.
    let ack_resp = acknowledge(&client, &session, ALARM, Some("on my way")).await;
    assert_eq!(ack_resp.status(), Status::Ok);
    let ack: Value = ack_resp.into_json().await.expect("json");
    assert_eq!(ack["alarm_num"], json!(ALARM));
    assert_eq!(ack["note"], json!("on my way"));
    assert_eq!(ack["acknowledged_by_email"], json!("newtown_superadmin@example.com"));
    let ack_user_id = ack["acknowledged_by_user_id"].as_i64().expect("acknowledged_by_user_id");

    // After acknowledgement: still visible (the condition is still present),
    // but now attributed. Acking an active alarm must not clear it.
    let after = active_entry(&client, &session, ALARM)
        .await
        .expect("alarm 401 should still be visible after ack");
    assert_eq!(after["status"], json!("AcknowledgedActive"));
    assert_eq!(after["data_active"], json!(true));
    assert_eq!(after["acknowledged_by_user_id"], json!(ack_user_id));
    assert_eq!(after["acknowledged_by_email"], json!("newtown_superadmin@example.com"));
    assert!(
        after["acknowledged_at"].as_str().is_some_and(|s| !s.is_empty()),
        "expected an acknowledged_at timestamp, got {:?}",
        after["acknowledged_at"]
    );
}

/// Acknowledgement is append-only and attributed to the acting user, so a
/// second acknowledgement by a different user replaces the reported
/// attribution.
#[tokio::test]
async fn latest_acknowledgement_wins_attribution() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let first = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    force_alarms(&client, &first, &[ALARM]).await;
    assert_eq!(acknowledge(&client, &first, ALARM, None).await.status(), Status::Ok);

    let second = login_as(&client, "test_superadmin@example.com", "adminpass").await;
    assert_eq!(acknowledge(&client, &second, ALARM, None).await.status(), Status::Ok);

    let entry = active_entry(&client, &second, ALARM).await.expect("alarm 401 visible");
    assert_eq!(entry["acknowledged_by_email"], json!("test_superadmin@example.com"));
}

/// An acknowledgement lands on the history timeline as its own event, carrying
/// the acknowledger and note, so the audit trail is visible to operators.
#[tokio::test]
async fn acknowledgement_appears_in_history() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    assert_eq!(
        acknowledge(&client, &session, ALARM, Some("checked panel")).await.status(),
        Status::Ok
    );

    let from = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let to = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let resp = client
        .get(format!("/api/1/Alarms/History?from={}&to={}", from, to))
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");

    let ack_entry = body["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .find(|e| e["event"] == json!("Acknowledged") && e["alarm_num"] == json!(ALARM))
        .expect("expected an Acknowledged history entry for alarm 401");

    assert_eq!(ack_entry["note"], json!("checked panel"));
    assert_eq!(ack_entry["acknowledged_by_email"], json!("newtown_superadmin@example.com"));
    // `active` is the legacy two-state view; acknowledgements are not
    // activations, so it reports false.
    assert_eq!(ack_entry["active"], json!(false));
    assert_eq!(ack_entry["name"], json!("fire_alarm"));
}

/// The `alarm_nums` filter applies to interleaved acknowledgements too, not
/// just to reading-derived transitions.
#[tokio::test]
async fn history_alarm_nums_filter_excludes_other_acknowledgements() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    assert_eq!(acknowledge(&client, &session, ALARM, None).await.status(), Status::Ok);

    let from = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let to = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    // 104 is a different defined alarm; filtering to it must hide 401's ack.
    let resp = client
        .get(format!("/api/1/Alarms/History?from={}&to={}&alarm_nums=104", from, to))
        .cookie(session)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");

    let leaked = body["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .any(|e| e["alarm_num"] == json!(ALARM));
    assert!(!leaked, "alarm 401 should be filtered out, got {:?}", body["entries"]);
}

/// An acknowledgement outside the requested window is not reported.
#[tokio::test]
async fn history_excludes_acknowledgements_outside_the_range() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    assert_eq!(acknowledge(&client, &session, ALARM, None).await.status(), Status::Ok);

    let from = (Utc::now() - Duration::days(3)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let to = (Utc::now() - Duration::days(2)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let resp = client
        .get(format!("/api/1/Alarms/History?from={}&to={}", from, to))
        .cookie(session)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");

    let found = body["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .any(|e| e["event"] == json!("Acknowledged"));
    assert!(!found, "no acknowledgement should fall in a past window");
}

/// Acknowledging an alarm number that isn't in the spec is a client error, not
/// a silently recorded row.
#[tokio::test]
async fn acknowledge_rejects_unknown_alarm_num() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let resp = acknowledge(&client, &session, 65000, None).await;
    assert_eq!(resp.status(), Status::BadRequest);
}

/// Acknowledgement is attributed to a user, so it requires authentication.
#[tokio::test]
async fn acknowledge_requires_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();

    let resp = client
        .post("/api/1/Alarms/Acknowledge")
        .json(&json!({ "alarm_num": ALARM }))
        .dispatch()
        .await;
    assert_ne!(resp.status(), Status::Ok, "unauthenticated acknowledgement must not succeed");
}

/// Any authenticated user can acknowledge — unlike the demo-only forced-alarm
/// controls, acknowledgement is a normal operator action.
#[tokio::test]
async fn acknowledge_allows_non_admin_roles() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "staff@example.com", "staffpass").await;

    let resp = acknowledge(&client, &session, ALARM, None).await;
    assert_eq!(resp.status(), Status::Ok);
}
