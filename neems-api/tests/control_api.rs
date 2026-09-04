//! Integration tests for the site input (control request) endpoints.
//!
//! The through-line is that a click is a *request*, and the endpoints report
//! only what became of it. Nothing here asserts a breaker position, because
//! nothing in this path knows one: where the equipment is comes from its
//! readback point, through the alarm endpoints, and the two are deliberately
//! separate axes.
//!
//! The other through-line is that a request which cannot get out says so
//! immediately and in words. No control has a write register while the client's
//! `Outputs` sheet is empty, so every request here resolves as `failed` — the
//! truth, and the thing an operator has to be shown rather than left guessing
//! at.

use neems_api::orm::testing::fast_test_rocket;
use rocket::{http::Status, local::asynchronous::Client, tokio};
use serde_json::{Value, json};

async fn login_as(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let body = json!({ "email": email, "password": password });
    let resp = client.post("/api/1/login").json(&body).dispatch().await;
    assert_eq!(resp.status(), Status::Ok, "login failed for {}", email);
    resp.cookies().get("session").expect("session cookie").clone().into_owned()
}

async fn list_controls(client: &Client, session: &rocket::http::Cookie<'static>) -> Value {
    let resp = client.get("/api/1/Sites/1/Controls").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::Ok);
    resp.into_json().await.expect("json")
}

async fn request_control(
    client: &Client,
    session: &rocket::http::Cookie<'static>,
    control: &str,
    action: &str,
) -> (Status, Value) {
    let resp = client
        .post(format!("/api/1/Sites/1/Controls/{control}/Requests"))
        .cookie(session.clone())
        .json(&json!({ "action": action }))
        .dispatch()
        .await;
    let status = resp.status();
    (status, resp.into_json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn the_control_list_describes_every_interactable_element() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let controls = list_controls(&client, &session).await;
    let controls = controls.as_array().expect("array");

    assert_eq!(controls.len(), 9, "the two line switches, six feeders and the lockout relay");

    let feeder = controls
        .iter()
        .find(|c| c["id"] == json!("feeder-1a"))
        .expect("feeder-1a is in the list");
    assert_eq!(feeder["label"], json!("52-MP-1A"));
    assert_eq!(feeder["actions"], json!(["open", "close"]));
    assert_eq!(feeder["readback_alarm_num"], json!(607));
    assert_eq!(feeder["latest_request"], json!(null), "nothing has been asked for yet");
}

/// The E-stop is not in this list. It is site-level, engage-only, and keeps its
/// own endpoints; a UI that found it here would offer it twice.
#[tokio::test]
async fn the_estop_is_not_a_control() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let controls = list_controls(&client, &session).await;
    assert!(
        !controls.as_array().unwrap().iter().any(|c| c["id"] == json!("estop")),
        "the E-stop belongs to /EmergencyStop, not /Controls"
    );
}

/// The point of the whole path: an operator is told, in words, that their click
/// went nowhere — rather than watching the diagram show a state the site never
/// confirmed.
#[tokio::test]
async fn a_request_that_cannot_be_sent_fails_immediately_with_a_reason() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let (status, request) = request_control(&client, &session, "feeder-1a", "open").await;

    assert_eq!(status, Status::Ok, "the request is recorded even though it cannot be sent");
    assert_eq!(request["status"], json!("failed"));
    assert_eq!(request["control_id"], json!("feeder-1a"));
    assert_eq!(request["action"], json!("open"));
    assert_eq!(request["sent_at"], json!(null));
    assert!(
        request["failure_reason"].as_str().unwrap_or("").contains("no RTAC point")
            || request["failure_reason"].as_str().unwrap_or("").contains("RTAC point"),
        "the reason must say why, in words: {:?}",
        request["failure_reason"]
    );
}

/// The failed request stays attached to its own element, so the diagram can
/// show the error against the breaker that was clicked.
#[tokio::test]
async fn the_latest_request_is_reported_against_its_control() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    request_control(&client, &session, "switch-89l-1", "open").await;

    let controls = list_controls(&client, &session).await;
    let controls = controls.as_array().unwrap();

    let clicked = controls.iter().find(|c| c["id"] == json!("switch-89l-1")).unwrap();
    assert_eq!(clicked["latest_request"]["status"], json!("failed"));
    assert_eq!(clicked["latest_request"]["action"], json!("open"));

    let untouched = controls.iter().find(|c| c["id"] == json!("switch-89l-2")).unwrap();
    assert_eq!(
        untouched["latest_request"],
        json!(null),
        "one click must not mark another element"
    );
}

/// A control that accepts only a trip must refuse to be opened or closed. The
/// registry is the authority on what each element accepts, so a UI bug cannot
/// turn into an unasked-for signal.
#[tokio::test]
async fn a_control_refuses_an_action_it_does_not_accept() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let (status, _) = request_control(&client, &session, "lockout-relay", "close").await;
    assert_eq!(status, Status::BadRequest);

    let (status, _) = request_control(&client, &session, "feeder-1a", "trip").await;
    assert_eq!(status, Status::BadRequest);
}

#[tokio::test]
async fn an_unknown_control_or_action_is_rejected() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let (status, _) = request_control(&client, &session, "transformer-1", "open").await;
    assert_eq!(status, Status::NotFound, "only listed controls may be requested");

    let (status, _) = request_control(&client, &session, "feeder-1a", "explode").await;
    assert_eq!(status, Status::BadRequest);
}

/// Nothing is left pending, because nothing can be written. The collector must
/// not be handed work it cannot do — it would report the same failure a minute
/// later, having told the operator nothing in the meantime.
#[tokio::test]
async fn nothing_is_left_pending_for_a_collector_that_cannot_write_it() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    request_control(&client, &session, "feeder-2b", "close").await;

    let resp = client
        .get("/api/1/Sites/1/Controls/Pending")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let pending: Value = resp.into_json().await.expect("json");
    assert_eq!(pending, json!([]));
}

#[tokio::test]
async fn controls_require_a_session() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();

    let resp = client.get("/api/1/Sites/1/Controls").dispatch().await;
    assert_ne!(resp.status(), Status::Ok, "an unauthenticated caller must not see the controls");

    let resp = client
        .post("/api/1/Sites/1/Controls/feeder-1a/Requests")
        .json(&json!({ "action": "open" }))
        .dispatch()
        .await;
    assert_ne!(resp.status(), Status::Ok, "an unauthenticated caller must not move equipment");
}
