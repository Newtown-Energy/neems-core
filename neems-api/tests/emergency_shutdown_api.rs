//! Integration tests for the operator emergency shutdown request endpoints.
//!
//! The through-line of these tests is that the *request* and the *state* are
//! separate things. The request records whether the operator's signal reached
//! the RTAC — the whole of what this system undertakes to do. Whether the plant
//! then tripped is reported independently as alarm 104, and neither answer is
//! allowed to stand in for the other.
//!
//! Alarm 104 is driven here through the demo alarm-state endpoint, which writes
//! the same `alarm_state` that `/EmergencyShutdown` and `/Alarms/Active` both
//! read.
//!
//! Which resolution a request gets depends on the deployment, and the tests are
//! split accordingly, as in `control_api.rs`:
//!
//! - **Off demo mode** ([`fast_test_rocket_with_demo_mode`] with `false`) a
//!   request waits `pending` for the collector, which reports it through
//!   `/Dispatch`. These are the collector-path tests.
//! - **On demo mode** ([`fast_test_rocket`], which enables it) there is no
//!   collector and no RTAC, so the API stands in for both: the request resolves
//!   `dispatched` and alarm 104 is raised. Driving 104 directly also needs demo
//!   mode, since the demo alarm routes are demo-only.

use neems_api::orm::testing::{fast_test_rocket, fast_test_rocket_with_demo_mode};
use rocket::{http::Status, local::asynchronous::Client, tokio};
use serde_json::{Value, json};

const ESTOP_ALARM_NUM: u16 = 104;

async fn login_as(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let body = json!({ "email": email, "password": password });
    let resp = client.post("/api/1/login").json(&body).dispatch().await;
    assert_eq!(resp.status(), Status::Ok, "login failed for {}", email);
    resp.cookies().get("session").expect("session cookie").clone().into_owned()
}

/// Raise or lower alarm 104 — the site tripping, or being reset at the panel.
async fn set_estop_alarm(client: &Client, session: &rocket::http::Cookie<'static>, active: bool) {
    set_demo_alarm(client, session, ESTOP_ALARM_NUM, active).await;
}

/// Drive one alarm through `/1/Demo/AlarmState` — the Demo Controls drawer's
/// path, and so the demo's stand-in for the panel on site.
async fn set_demo_alarm(
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
    assert_eq!(resp.status(), Status::Ok, "failed to set demo alarm {alarm_num}");
}

async fn get_status(client: &Client, session: &rocket::http::Cookie<'static>) -> Value {
    let resp = client
        .get("/api/1/Sites/1/EmergencyShutdown")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    resp.into_json().await.expect("json")
}

async fn request_emergency_shutdown(
    client: &Client,
    session: &rocket::http::Cookie<'static>,
) -> Value {
    let resp = client
        .post("/api/1/Sites/1/EmergencyShutdown")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    resp.into_json().await.expect("json")
}

#[tokio::test]
async fn status_reports_no_request_and_no_trip_initially() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let status = get_status(&client, &session).await;
    assert_eq!(status["site_id"], json!(1));
    assert_eq!(status["observed_active"], json!(false));
    assert_eq!(status["request"], json!(null));
    // No readings carry alarm data in the fast fixture, so nothing is known
    // about the site rather than the site being known to be running.
    assert_eq!(status["observed_at"], json!(null));
}

#[tokio::test]
async fn requesting_an_emergency_shutdown_records_a_pending_request() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let body = request_emergency_shutdown(&client, &session).await;

    assert_eq!(body["request"]["status"], json!("pending"));
    assert_eq!(body["request"]["site_id"], json!(1));
    assert!(body["request"]["requested_at"].is_string());
    assert_eq!(body["request"]["dispatched_at"], json!(null));
    assert_eq!(body["request"]["resolved_at"], json!(null));

    // The request must not make the site read as tripped.
    assert_eq!(
        body["observed_active"],
        json!(false),
        "a request is not a trip; only the RTAC decides that"
    );
}

#[tokio::test]
async fn repeated_requests_coalesce_onto_the_one_in_flight() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let first = request_emergency_shutdown(&client, &session).await;
    let second = request_emergency_shutdown(&client, &session).await;

    assert_eq!(
        first["request"]["id"], second["request"]["id"],
        "a second click while the first trip is in flight is the same ask"
    );
}

#[tokio::test]
async fn dispatch_resolves_the_request_and_is_idempotent() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let requested = request_emergency_shutdown(&client, &session).await;
    let id = requested["request"]["id"].as_i64().expect("request id");

    let url = format!("/api/1/Sites/1/EmergencyShutdown/{id}/Dispatch");
    let resp = client.post(&url).cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::Ok);
    let dispatched: Value = resp.into_json().await.expect("json");
    assert_eq!(dispatched["status"], json!("dispatched"));
    let first_dispatched_at = dispatched["dispatched_at"].clone();
    assert!(first_dispatched_at.is_string());
    assert!(
        dispatched["resolved_at"].is_string(),
        "the signal is out, so there is nothing further to wait for"
    );

    // Re-reporting must not restate when the trip went out.
    let resp2 = client.post(&url).cookie(session.clone()).dispatch().await;
    assert_eq!(resp2.status(), Status::Ok);
    let again: Value = resp2.into_json().await.expect("json");
    assert_eq!(again["status"], json!("dispatched"));
    assert_eq!(again["dispatched_at"], first_dispatched_at);
}

#[tokio::test]
async fn dispatch_rejects_an_unknown_request() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let resp = client
        .post("/api/1/Sites/1/EmergencyShutdown/9999/Dispatch")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::NotFound);
}

#[tokio::test]
async fn the_pending_endpoint_holds_a_request_only_until_it_is_sent() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Nothing outstanding to begin with.
    let empty = client
        .get("/api/1/Sites/1/EmergencyShutdown/Pending")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(empty.status(), Status::Ok);
    assert_eq!(empty.into_json::<Value>().await.expect("json"), json!(null));

    let requested = request_emergency_shutdown(&client, &session).await;
    let id = requested["request"]["id"].as_i64().expect("request id");

    let pending: Value = client
        .get("/api/1/Sites/1/EmergencyShutdown/Pending")
        .cookie(session.clone())
        .dispatch()
        .await
        .into_json()
        .await
        .expect("json");
    assert_eq!(pending["id"], json!(id));
    assert_eq!(pending["status"], json!("pending"));

    // Once the signal is out there is nothing left for the collector to do,
    // whatever the RTAC subsequently does about it.
    client
        .post(format!("/api/1/Sites/1/EmergencyShutdown/{id}/Dispatch"))
        .cookie(session.clone())
        .dispatch()
        .await;
    let after: Value = client
        .get("/api/1/Sites/1/EmergencyShutdown/Pending")
        .cookie(session.clone())
        .dispatch()
        .await
        .into_json()
        .await
        .expect("json");
    assert_eq!(after, json!(null));
}

/// The request records that the signal was sent. It is not a claim about the
/// plant, and it does not wait on one: an RTAC that never trips leaves the
/// request sent and `observed_active` false, which is the honest answer.
///
/// The other direction — the plant moving after the request has resolved — is
/// `a_demo_trip_is_reset_from_the_site_side_and_the_request_stands`, since 104
/// can only be driven where demo mode is on.
#[tokio::test]
async fn a_sent_request_is_not_a_claim_that_the_site_tripped() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let requested = request_emergency_shutdown(&client, &session).await;
    let id = requested["request"]["id"].as_i64().expect("request id");
    client
        .post(format!("/api/1/Sites/1/EmergencyShutdown/{id}/Dispatch"))
        .cookie(session.clone())
        .dispatch()
        .await;

    // Signal sent, RTAC has not tripped. The request is done; the site is not
    // stopped; neither statement is allowed to contaminate the other.
    let status = get_status(&client, &session).await;
    assert_eq!(status["request"]["status"], json!("dispatched"));
    assert!(status["request"]["resolved_at"].is_string());
    assert_eq!(status["request"]["failure_reason"], json!(null));
    assert_eq!(status["observed_active"], json!(false));
}

/// An already-tripped site is still asked. Whether the RTAC needs the signal is
/// its business, and refusing to pass on an operator's request because we think
/// it is redundant is not a call this system gets to make.
#[tokio::test]
async fn requesting_while_already_tripped_still_records_a_request_to_send() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Someone already hit the physical button. Driving 104 needs demo mode, so
    // the API is the collector here: "still asked" means the request is carried
    // out rather than refused or swallowed, which is the same decision
    // `request_emergency_shutdown` makes on either path.
    set_estop_alarm(&client, &session, true).await;

    let body = request_emergency_shutdown(&client, &session).await;
    assert_eq!(body["observed_active"], json!(true));
    assert_eq!(body["request"]["status"], json!("dispatched"), "the ask still went out");
    assert!(body["request"]["dispatched_at"].is_string());
}

/// A fresh ask after the last signal went out is a new request, and gets its
/// own signal.
#[tokio::test]
async fn a_request_after_dispatch_starts_a_new_one() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let first = request_emergency_shutdown(&client, &session).await;
    let first_id = first["request"]["id"].as_i64().expect("request id");
    client
        .post(format!("/api/1/Sites/1/EmergencyShutdown/{first_id}/Dispatch"))
        .cookie(session.clone())
        .dispatch()
        .await;

    let second = request_emergency_shutdown(&client, &session).await;
    assert_ne!(second["request"]["id"], json!(first_id));
    assert_eq!(second["request"]["status"], json!("pending"));
}

#[tokio::test]
async fn observed_state_follows_the_rtac_back_down() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    set_estop_alarm(&client, &session, true).await;
    assert_eq!(get_status(&client, &session).await["observed_active"], json!(true));

    // Cleared at the panel: no API call resets it, the alarm simply drops.
    set_estop_alarm(&client, &session, false).await;
    let after = get_status(&client, &session).await;
    assert_eq!(after["observed_active"], json!(false));
}

#[tokio::test]
async fn emergency_shutdown_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();

    let get = client.get("/api/1/Sites/1/EmergencyShutdown").dispatch().await;
    assert_ne!(get.status(), Status::Ok, "unauthenticated read must not succeed");

    let post = client.post("/api/1/Sites/1/EmergencyShutdown").dispatch().await;
    assert_ne!(post.status(), Status::Ok, "unauthenticated trip must not succeed");
}

/// The demo's reason for existing, for the emergency shutdown: pressing it
/// trips the site, with no RTAC anywhere.
///
/// Both halves, asserted separately. `dispatched` says the signal got out —
/// which on a demo means the API stood in for the collector — and
/// `observed_active` says the site is tripped because alarm 104 is really set.
/// The trip is reported by the same response that carried it out, so the page
/// never shows "sent, not tripped" for the instant between the two.
#[tokio::test]
async fn a_demo_emergency_shutdown_request_is_dispatched_and_trips_the_site() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let body = request_emergency_shutdown(&client, &session).await;

    assert_eq!(body["request"]["status"], json!("dispatched"), "the demo is the collector");
    assert!(body["request"]["dispatched_at"].is_string());
    assert_eq!(body["request"]["failure_reason"], json!(null));
    assert_eq!(body["observed_active"], json!(true), "and the site reports the trip");

    // Read back through the alarm feed, as the diagram does.
    let resp = client.get("/api/1/Alarms/Active").cookie(session.clone()).dispatch().await;
    let alarms: Value = resp.into_json().await.expect("json");
    let estop = alarms["alarms"]
        .as_array()
        .expect("alarms")
        .iter()
        .find(|a| a["alarm_num"] == json!(ESTOP_ALARM_NUM))
        .cloned()
        .expect("alarm 104 is listed");
    assert_eq!(estop["data_active"], json!(true));

    // Nothing is left for a collector the demo does not run.
    let pending: Value = client
        .get("/api/1/Sites/1/EmergencyShutdown/Pending")
        .cookie(session.clone())
        .dispatch()
        .await
        .into_json()
        .await
        .expect("json");
    assert_eq!(pending, json!(null));
}

/// Engage-only survives demo mode. A demo trip is lowered from the site side —
/// the drawer, standing in for the panel — and the request that caused it is
/// left exactly as it was. The plant moving is news about the plant, never a
/// rewrite of whether the operator's signal got out.
#[tokio::test]
async fn a_demo_trip_is_reset_from_the_site_side_and_the_request_stands() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let tripped = request_emergency_shutdown(&client, &session).await;
    assert_eq!(tripped["observed_active"], json!(true));

    set_demo_alarm(&client, &session, ESTOP_ALARM_NUM, false).await;

    let after = get_status(&client, &session).await;
    assert_eq!(after["observed_active"], json!(false), "the site is running again");
    assert_eq!(after["request"]["id"], tripped["request"]["id"]);
    assert_eq!(after["request"]["status"], json!("dispatched"));
    assert_eq!(after["request"]["resolved_at"], tripped["request"]["resolved_at"]);
}

/// Off demo mode, pressing Emergency Shutdown never raises 104 on its own. The
/// API only carries requests; a real trip is the RTAC's to report. This is the
/// guard that the demo path cannot leak into a real deployment.
#[tokio::test]
async fn off_demo_mode_a_request_does_not_trip_the_site() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let body = request_emergency_shutdown(&client, &session).await;
    assert_eq!(body["request"]["status"], json!("pending"));
    assert_eq!(body["observed_active"], json!(false), "only the RTAC decides that");
}
