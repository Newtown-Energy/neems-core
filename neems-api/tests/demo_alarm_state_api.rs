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
/// Raise -> `Active`. Acknowledge -> `AcknowledgedActive`, still visible.
/// Clear -> `ReturnedUnacknowledged`, *still visible*, because the operator
/// acknowledged it while it was firing and owes a second acknowledgement now
/// that it has returned to normal. The old in-memory forced-alarm overlay made
/// the alarm vanish at this step, which is precisely the bug this replaces.
#[tokio::test]
async fn clearing_a_demo_alarm_latches_it_as_returned_unacknowledged() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Raise it.
    let resp = set_alarm_state(&client, &session, ALARM, true).await;
    assert_eq!(resp.status(), Status::Ok);
    let raised = active_entry(&client, &session, ALARM).await.expect("alarm should be active");
    assert_eq!(raised["status"], json!("Active"));
    assert_eq!(raised["data_active"], json!(true));

    // Acknowledge while still firing — must not clear it.
    let ack = client
        .post("/api/1/Alarms/Acknowledge")
        .cookie(session.clone())
        .json(&json!({ "alarm_num": ALARM }))
        .dispatch()
        .await;
    assert_eq!(ack.status(), Status::Ok);
    let acked = active_entry(&client, &session, ALARM).await.expect("alarm should still show");
    assert_eq!(acked["status"], json!("AcknowledgedActive"));

    // Return it to normal. It must remain visible, awaiting a second ack.
    let resp = set_alarm_state(&client, &session, ALARM, false).await;
    assert_eq!(resp.status(), Status::Ok);
    let returned = active_entry(&client, &session, ALARM)
        .await
        .expect("a returned-but-unacknowledged alarm must stay visible");
    assert_eq!(returned["status"], json!("ReturnedUnacknowledged"));
    assert_eq!(returned["data_active"], json!(false));

    // The second acknowledgement is what finally clears it.
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

/// The set-replace view over the same table stays in agreement with the
/// per-alarm endpoint.
#[tokio::test]
async fn forced_set_view_agrees_with_per_alarm_writes() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    set_alarm_state(&client, &session, ALARM, true).await;

    let resp = client.get("/api/1/Alarms/Forced").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");
    assert_eq!(body["alarm_nums"], json!([ALARM]));

    // Clearing through the set endpoint must leave the alarm latched, not gone.
    let resp = client
        .put("/api/1/Alarms/Forced")
        .cookie(session.clone())
        .json(&json!({ "alarm_nums": [] }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);

    let entry = active_entry(&client, &session, ALARM)
        .await
        .expect("cleared alarm should latch as returned");
    assert_eq!(entry["status"], json!("ReturnedUnacknowledged"));
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

    let resp = client.get("/api/1/Alarms/Forced").cookie(session.clone()).dispatch().await;
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
