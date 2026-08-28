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
//! only thing in the active set. The demo endpoints write real edges through
//! `upsert_alarm_transition`, the same path the RTAC collector uses, so an
//! alarm driven from here latches exactly like one driven by hardware.

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

/// Set one alarm's data state directly, the way the RTAC collector reports it.
/// Unlike [`force_alarms`] this does not diff against the current state, so it
/// can assert a state the alarm is already in.
async fn set_alarm_state(
    client: &Client,
    session: &rocket::http::Cookie<'static>,
    alarm_num: u16,
    active: bool,
) {
    let resp = client
        .post("/api/1/Demo/AlarmState")
        .cookie(session.clone())
        .json(&json!({ "alarm_num": alarm_num, "active": active }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "setting alarm {} to {} failed", alarm_num, active);
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

/// The core round trip: an active alarm reports unacknowledged, and after
/// acknowledgement it stays visible and still active — the condition has not
/// gone anywhere — carrying who acknowledged it and when.
#[tokio::test]
async fn acknowledging_an_active_alarm_latches_it_as_acknowledged() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    force_alarms(&client, &session, &[ALARM]).await;

    // Before acknowledgement: active and unacknowledged.
    let before = active_entry(&client, &session, ALARM)
        .await
        .expect("alarm 401 should be active");
    assert_eq!(before["data_active"], json!(true));
    assert_eq!(before["acknowledged"], json!(false));
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
    assert_eq!(after["data_active"], json!(true));
    assert_eq!(after["acknowledged"], json!(true));
    assert_eq!(after["acknowledged_by_user_id"], json!(ack_user_id));
    assert_eq!(after["acknowledged_by_email"], json!("newtown_superadmin@example.com"));
    assert!(
        after["acknowledged_at"].as_str().is_some_and(|s| !s.is_empty()),
        "expected an acknowledged_at timestamp, got {:?}",
        after["acknowledged_at"]
    );
}

/// Acknowledging an active alarm settles that activation for good: once the
/// condition goes away the alarm is finished and drops off the active list.
///
/// This is the fix for issue #106. The previous rule ("require 2nd ack", from
/// issue #76) left the alarm demanding a second acknowledgement after it
/// returned to normal, which meant an operator who had already responded got
/// asked again about an alarm that was over.
#[tokio::test]
async fn acknowledging_then_clearing_finishes_the_alarm() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    force_alarms(&client, &session, &[ALARM]).await;
    assert_eq!(acknowledge(&client, &session, ALARM, None).await.status(), Status::Ok);
    force_alarms(&client, &session, &[]).await;

    assert!(
        active_entry(&client, &session, ALARM).await.is_none(),
        "an acknowledged alarm that has returned to normal is finished"
    );
}

/// A clear splits the timeline into separate instances: the second activation
/// is a new event, so the acknowledgement of the first does not carry over.
#[tokio::test]
async fn reactivation_after_an_ack_requires_a_new_ack() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    force_alarms(&client, &session, &[ALARM]).await;
    assert_eq!(acknowledge(&client, &session, ALARM, None).await.status(), Status::Ok);
    force_alarms(&client, &session, &[]).await;

    // Second instance: fires again, then clears again on its own.
    force_alarms(&client, &session, &[ALARM]).await;
    force_alarms(&client, &session, &[]).await;

    let entry = active_entry(&client, &session, ALARM)
        .await
        .expect("the second activation still needs acknowledging");
    assert_eq!(entry["data_active"], json!(false));
    assert_eq!(entry["acknowledged"], json!(false));
    // The earlier acknowledgement belongs to the previous instance, so it must
    // not be reported against this one.
    assert_eq!(entry["acknowledged_by_user_id"], Value::Null);
    assert_eq!(entry["acknowledged_at"], Value::Null);
}

/// An alarm that fires and clears unattended stays visible, so an operator
/// coming on shift is still told it happened.
#[tokio::test]
async fn an_unacknowledged_alarm_stays_visible_after_it_clears() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    force_alarms(&client, &session, &[ALARM]).await;
    force_alarms(&client, &session, &[]).await;

    let entry = active_entry(&client, &session, ALARM)
        .await
        .expect("an unacknowledged alarm stays visible after returning to normal");
    assert_eq!(entry["data_active"], json!(false));
    assert_eq!(entry["acknowledged"], json!(false));
}

/// Re-reporting a state the alarm is already in is not a new activation, so it
/// must not throw away the acknowledgement.
///
/// Two things do exactly this: `POST /Demo/AlarmState` writes whatever it is
/// handed, and the RTAC collector re-reports every still-active alarm as a
/// rising edge after a restart (`RtacWorker::last_alarm_flags` starts
/// all-clear). Both would otherwise re-stamp `last_rising_at` mid-activation
/// and silently un-acknowledge an alarm nobody had touched.
#[tokio::test]
async fn a_repeated_active_report_keeps_the_acknowledgement() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    set_alarm_state(&client, &session, ALARM, true).await;
    assert_eq!(acknowledge(&client, &session, ALARM, None).await.status(), Status::Ok);

    // Still the same activation, reported again.
    set_alarm_state(&client, &session, ALARM, true).await;

    let entry = active_entry(&client, &session, ALARM).await.expect("alarm still active");
    assert_eq!(entry["data_active"], json!(true));
    assert_eq!(
        entry["acknowledged"],
        json!(true),
        "a repeated active report is not a new activation"
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
