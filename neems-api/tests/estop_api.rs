//! Integration tests for the operator E-stop request endpoints.
//!
//! The through-line of these tests is that the *request* and the *state* are
//! separate things. The request records whether the operator's signal reached
//! the RTAC — the whole of what this system undertakes to do. Whether the plant
//! then tripped is reported independently as alarm 104, and neither answer is
//! allowed to stand in for the other.
//!
//! Alarm 104 is driven here through the demo forced-alarm set, which
//! `/EmergencyStop` overlays exactly as `/Alarms/Active` does.

use neems_api::orm::testing::fast_test_rocket;
use rocket::{http::Status, local::asynchronous::Client, tokio};
use serde_json::{Value, json};

const ESTOP_ALARM_NUM: u16 = 104;

async fn login_as(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let body = json!({ "email": email, "password": password });
    let resp = client.post("/api/1/login").json(&body).dispatch().await;
    assert_eq!(resp.status(), Status::Ok, "login failed for {}", email);
    resp.cookies().get("session").expect("session cookie").clone().into_owned()
}

/// Drive alarm 104 through the demo forced-alarm set.
async fn set_estop_alarm(client: &Client, session: &rocket::http::Cookie<'static>, active: bool) {
    let nums = if active {
        json!([ESTOP_ALARM_NUM])
    } else {
        json!([])
    };
    let resp = client
        .put("/api/1/Alarms/Forced")
        .cookie(session.clone())
        .json(&json!({ "alarm_nums": nums }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "failed to set forced alarms");
}

async fn get_status(client: &Client, session: &rocket::http::Cookie<'static>) -> Value {
    let resp = client
        .get("/api/1/Sites/1/EmergencyStop")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    resp.into_json().await.expect("json")
}

async fn request_estop(client: &Client, session: &rocket::http::Cookie<'static>) -> Value {
    let resp = client
        .post("/api/1/Sites/1/EmergencyStop")
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
async fn requesting_an_estop_records_a_pending_request() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let body = request_estop(&client, &session).await;

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
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let first = request_estop(&client, &session).await;
    let second = request_estop(&client, &session).await;

    assert_eq!(
        first["request"]["id"], second["request"]["id"],
        "a second click while the first trip is in flight is the same ask"
    );
}

#[tokio::test]
async fn dispatch_resolves_the_request_and_is_idempotent() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let requested = request_estop(&client, &session).await;
    let id = requested["request"]["id"].as_i64().expect("request id");

    let url = format!("/api/1/Sites/1/EmergencyStop/{id}/Dispatch");
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
        .post("/api/1/Sites/1/EmergencyStop/9999/Dispatch")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::NotFound);
}

#[tokio::test]
async fn the_pending_endpoint_holds_a_request_only_until_it_is_sent() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Nothing outstanding to begin with.
    let empty = client
        .get("/api/1/Sites/1/EmergencyStop/Pending")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(empty.status(), Status::Ok);
    assert_eq!(empty.into_json::<Value>().await.expect("json"), json!(null));

    let requested = request_estop(&client, &session).await;
    let id = requested["request"]["id"].as_i64().expect("request id");

    let pending: Value = client
        .get("/api/1/Sites/1/EmergencyStop/Pending")
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
        .post(format!("/api/1/Sites/1/EmergencyStop/{id}/Dispatch"))
        .cookie(session.clone())
        .dispatch()
        .await;
    let after: Value = client
        .get("/api/1/Sites/1/EmergencyStop/Pending")
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
#[tokio::test]
async fn a_sent_request_stands_whether_or_not_the_rtac_trips() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let requested = request_estop(&client, &session).await;
    let id = requested["request"]["id"].as_i64().expect("request id");
    client
        .post(format!("/api/1/Sites/1/EmergencyStop/{id}/Dispatch"))
        .cookie(session.clone())
        .dispatch()
        .await;

    // Signal sent, RTAC has not tripped. The request is done; the site is not
    // stopped; neither statement is allowed to contaminate the other.
    let mid = get_status(&client, &session).await;
    assert_eq!(mid["request"]["status"], json!("dispatched"));
    assert!(mid["request"]["resolved_at"].is_string());
    assert_eq!(mid["request"]["failure_reason"], json!(null));
    assert_eq!(mid["observed_active"], json!(false));

    // The RTAC then raises alarm 104. Only the observed state changes.
    set_estop_alarm(&client, &session, true).await;

    let done = get_status(&client, &session).await;
    assert_eq!(done["observed_active"], json!(true));
    assert_eq!(done["request"]["id"], json!(id));
    assert_eq!(done["request"]["status"], json!("dispatched"));
    assert_eq!(done["request"]["resolved_at"], mid["request"]["resolved_at"]);
}

/// An already-tripped site is still asked. Whether the RTAC needs the signal is
/// its business, and refusing to pass on an operator's request because we think
/// it is redundant is not a call this system gets to make.
#[tokio::test]
async fn requesting_while_already_tripped_still_records_a_request_to_send() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Someone already hit the physical button.
    set_estop_alarm(&client, &session, true).await;

    let body = request_estop(&client, &session).await;
    assert_eq!(body["observed_active"], json!(true));
    assert_eq!(body["request"]["status"], json!("pending"));

    let pending: Value = client
        .get("/api/1/Sites/1/EmergencyStop/Pending")
        .cookie(session.clone())
        .dispatch()
        .await
        .into_json()
        .await
        .expect("json");
    assert_eq!(pending["id"], body["request"]["id"], "the collector still has a signal to send");
}

/// A fresh ask after the last signal went out is a new request, and gets its
/// own signal.
#[tokio::test]
async fn a_request_after_dispatch_starts_a_new_one() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let first = request_estop(&client, &session).await;
    let first_id = first["request"]["id"].as_i64().expect("request id");
    client
        .post(format!("/api/1/Sites/1/EmergencyStop/{first_id}/Dispatch"))
        .cookie(session.clone())
        .dispatch()
        .await;

    let second = request_estop(&client, &session).await;
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
async fn estop_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();

    let get = client.get("/api/1/Sites/1/EmergencyStop").dispatch().await;
    assert_ne!(get.status(), Status::Ok, "unauthenticated read must not succeed");

    let post = client.post("/api/1/Sites/1/EmergencyStop").dispatch().await;
    assert_ne!(post.status(), Status::Ok, "unauthenticated trip must not succeed");
}
