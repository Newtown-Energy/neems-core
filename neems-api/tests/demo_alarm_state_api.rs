//! Integration tests for the demo alarm-state endpoints.
//!
//! `/1/Demo/AlarmState` exists so a hardware-free demo can raise and clear
//! alarms through the *same* data-state path the RTAC collector writes
//! (`alarm_state`), rather than an in-memory overlay. That is what lets a demo
//! alarm latch: the headline test here is the full round trip an operator
//! makes — raise, acknowledge, clear, and find the alarm still waiting for its
//! second acknowledgement.
//!
//! Alarm 401 (`fire_alarm`, Emergency) is used throughout; any defined
//! alarm_num would do. The fast test fixture has no readings carrying alarm
//! registers, so demo-driven state is the only thing in the active set.

use chrono::{Duration, SecondsFormat, Utc};
use neems_api::orm::testing::{fast_test_rocket, fast_test_rocket_with_demo_mode};
use rocket::{http::Status, local::asynchronous::Client, tokio};
use serde_json::{Value, json};

const ALARM: u16 = 401;

async fn login_as(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let body = json!({ "email": email, "password": password });
    let resp = client.post("/api/1/login").json(&body).dispatch().await;
    assert_eq!(resp.status(), Status::Ok, "login failed for {}", email);
    resp.cookies().get("session").expect("session cookie").clone().into_owned()
}

/// Drive one alarm's data state via the demo endpoint.
async fn set_alarm_state<'c>(
    client: &'c Client,
    session: &rocket::http::Cookie<'static>,
    alarm_num: u16,
    active: bool,
) -> rocket::local::asynchronous::LocalResponse<'c> {
    client
        .post("/api/1/Demo/AlarmState")
        .cookie(session.clone())
        .json(&json!({ "alarm_num": alarm_num, "active": active }))
        .dispatch()
        .await
}

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

/// The capability this endpoint exists for: an alarm raised and cleared
/// through the demo path latches exactly like a real one.
///
/// Raise -> active, unacknowledged. Clear without acknowledging -> *still
/// visible*, no longer firing but still owed an acknowledgement. The old
/// in-memory forced-alarm overlay made the alarm vanish at this step, which is
/// precisely the bug this replaces. Acknowledging is what finally ends it.
#[tokio::test]
async fn clearing_an_unacknowledged_demo_alarm_keeps_it_visible() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Raise it.
    let resp = set_alarm_state(&client, &session, ALARM, true).await;
    assert_eq!(resp.status(), Status::Ok);
    let raised = active_entry(&client, &session, ALARM).await.expect("alarm should be active");
    assert_eq!(raised["data_active"], json!(true));
    assert_eq!(raised["acknowledged"], json!(false));

    // Return it to normal with nobody having seen it. It must stay visible.
    let resp = set_alarm_state(&client, &session, ALARM, false).await;
    assert_eq!(resp.status(), Status::Ok);
    let returned = active_entry(&client, &session, ALARM)
        .await
        .expect("a returned-but-unacknowledged alarm must stay visible");
    assert_eq!(returned["data_active"], json!(false));
    assert_eq!(returned["acknowledged"], json!(false));

    // Acknowledging it is what ends it.
    let ack = client
        .post("/api/1/Alarms/Acknowledge")
        .cookie(session.clone())
        .json(&json!({ "alarm_num": ALARM }))
        .dispatch()
        .await;
    assert_eq!(ack.status(), Status::Ok);
    assert!(
        active_entry(&client, &session, ALARM).await.is_none(),
        "alarm should clear once acknowledged after returning to normal"
    );
}

/// Acknowledging while the alarm is still firing does not clear it — the
/// condition is still physically present — but it does settle that activation,
/// so the alarm ends when the condition goes away rather than asking the
/// operator again (issue #106).
#[tokio::test]
async fn acknowledging_a_firing_demo_alarm_settles_that_activation() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    assert_eq!(set_alarm_state(&client, &session, ALARM, true).await.status(), Status::Ok);
    let ack = client
        .post("/api/1/Alarms/Acknowledge")
        .cookie(session.clone())
        .json(&json!({ "alarm_num": ALARM }))
        .dispatch()
        .await;
    assert_eq!(ack.status(), Status::Ok);

    let acked = active_entry(&client, &session, ALARM).await.expect("alarm is still firing");
    assert_eq!(acked["data_active"], json!(true));
    assert_eq!(acked["acknowledged"], json!(true));

    assert_eq!(set_alarm_state(&client, &session, ALARM, false).await.status(), Status::Ok);
    assert!(
        active_entry(&client, &session, ALARM).await.is_none(),
        "an acknowledged alarm is finished once the condition clears"
    );
}

/// Raising an alarm stamps a rising edge and reports it as active; clearing it
/// stamps a falling edge. The GET reflects both.
#[tokio::test]
async fn get_reports_current_state_and_edges() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let resp = set_alarm_state(&client, &session, ALARM, true).await;
    let body: Value = resp.into_json().await.expect("json");
    assert_eq!(body["active_alarm_nums"], json!([ALARM]));

    let resp = client.get("/api/1/Demo/AlarmState").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");
    let entry = body["alarms"]
        .as_array()
        .expect("alarms array")
        .iter()
        .find(|a| a["alarm_num"].as_u64() == Some(ALARM as u64))
        .cloned()
        .expect("alarm should have state");
    assert_eq!(entry["active"], json!(true));
    assert!(entry["last_rising_at"].as_str().is_some(), "expected a rising edge timestamp");

    // Clearing records the falling edge and empties the active set.
    let resp = set_alarm_state(&client, &session, ALARM, false).await;
    let body: Value = resp.into_json().await.expect("json");
    assert_eq!(body["active_alarm_nums"], json!([]));
    let entry = body["alarms"]
        .as_array()
        .expect("alarms array")
        .iter()
        .find(|a| a["alarm_num"].as_u64() == Some(ALARM as u64))
        .cloned()
        .expect("alarm should have state");
    assert_eq!(entry["active"], json!(false));
    assert!(entry["last_falling_at"].as_str().is_some(), "expected a falling edge timestamp");
}

/// A demo alarm change must appear in `/Alarms/History` as a real
/// Activated/Cleared transition, not just in `/Alarms/Active`.
///
/// History is derived by diffing consecutive readings, so driving alarm state
/// without writing a reading leaves the FDNY timeline empty — which is exactly
/// what the in-memory forced set used to do.
#[tokio::test]
async fn demo_alarm_changes_appear_in_history() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let from = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);

    set_alarm_state(&client, &session, ALARM, true).await;
    // A second reading is what makes the first one a diffable baseline.
    set_alarm_state(&client, &session, ALARM, false).await;

    let to = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let resp = client
        .get(format!("/api/1/Alarms/History?from={from}&to={to}&alarm_nums={ALARM}"))
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");
    let events: Vec<String> = body["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .map(|e| e["event"].as_str().unwrap_or_default().to_string())
        .collect();

    assert!(
        events.iter().any(|e| e == "Cleared"),
        "expected a Cleared transition in history, got {events:?}"
    );
}

/// Demo-only means demo-only: with demo mode off the routes are not there at
/// all. 404 rather than 403 so a production deployment does not advertise them.
#[tokio::test]
async fn demo_routes_are_hidden_when_demo_mode_is_off() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let resp = set_alarm_state(&client, &session, ALARM, true).await;
    assert_eq!(resp.status(), Status::NotFound);

    let resp = client.get("/api/1/Demo/AlarmState").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::NotFound);
}

/// Acknowledgement is open to any authenticated operator, but driving demo
/// state is not.
#[tokio::test]
async fn setting_alarm_state_requires_a_demo_role() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "staff@example.com", "staffpass").await;

    let resp = set_alarm_state(&client, &session, ALARM, true).await;
    assert_eq!(resp.status(), Status::Forbidden);
}

#[tokio::test]
async fn setting_alarm_state_requires_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let resp = client
        .post("/api/1/Demo/AlarmState")
        .json(&json!({ "alarm_num": ALARM, "active": true }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[tokio::test]
async fn setting_alarm_state_rejects_unknown_alarm_num() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let resp = set_alarm_state(&client, &session, 65000, true).await;
    assert_eq!(resp.status(), Status::BadRequest);
}

/// `/1/Alarms/Forced` is retired (#122). It replaced the whole demo alarm set,
/// which since control readbacks moved into `alarm_state` would open every
/// closed breaker and clear a demo trip. `/1/Demo/AlarmState` is the only way
/// to drive demo alarms now.
#[tokio::test]
async fn the_retired_forced_alarm_routes_are_gone() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let resp = client.get("/api/1/Alarms/Forced").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::NotFound);

    let resp = client
        .put("/api/1/Alarms/Forced")
        .cookie(session.clone())
        .json(&json!({ "alarm_nums": [ALARM] }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::NotFound);
}
