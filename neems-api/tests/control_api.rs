//! Integration tests for the site input (control request) endpoints.
//!
//! The through-line is that a click is a *request*, and the endpoints report
//! only what became of it. Where the equipment then sits is a separate axis,
//! read from the control's readback point through the alarm endpoints — so even
//! the demo tests below, which do move equipment, assert the two separately and
//! never read one off the other.
//!
//! Which resolution a request gets depends on the deployment, and the tests are
//! split accordingly:
//!
//! - **Off demo mode** ([`fast_test_rocket_with_demo_mode`] with `false`) no
//!   control has a write register while the client's `Outputs` sheet is empty,
//!   so a request resolves `failed` with a reason an operator can read. That is
//!   the truth on a real deployment and the thing they must be shown rather
//!   than left guessing at.
//! - **On demo mode** ([`fast_test_rocket`], which enables it) there is no RTAC
//!   and no collector, so the API stands in for one: the request resolves
//!   `sent` and the readback point moves to match.

use neems_api::orm::testing::{fast_test_rocket, fast_test_rocket_with_demo_mode};
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

/// One alarm's data state, as `/Alarms/Active` reports it. `None` when the
/// alarm is not listed at all, which for a readback point means it has never
/// been set.
///
/// Reads `data_active` rather than mere presence, and that distinction is the
/// whole reason this helper exists: a point that has gone back to normal stays
/// listed until it is acknowledged, so presence answers "has this alarm been
/// dealt with", not "where is the equipment".
async fn readback_state(
    client: &Client,
    session: &rocket::http::Cookie<'static>,
    alarm_num: u16,
) -> Option<bool> {
    let resp = client.get("/api/1/Alarms/Active").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");
    body["alarms"]
        .as_array()
        .expect("alarms array")
        .iter()
        .find(|a| a["alarm_num"].as_u64() == Some(alarm_num as u64))
        .map(|a| a["data_active"].as_bool().expect("data_active"))
}

/// Every alarm the site is currently reporting, by number.
async fn firing_alarm_nums(client: &Client, session: &rocket::http::Cookie<'static>) -> Vec<u64> {
    let resp = client.get("/api/1/Alarms/Active").cookie(session.clone()).dispatch().await;
    assert_eq!(resp.status(), Status::Ok);
    let body: Value = resp.into_json().await.expect("json");
    body["alarms"]
        .as_array()
        .expect("alarms array")
        .iter()
        .filter(|a| a["data_active"].as_bool() == Some(true))
        .map(|a| a["alarm_num"].as_u64().expect("alarm_num"))
        .collect()
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
///
/// Off demo mode, because the demo answers this click by carrying it out.
#[tokio::test]
async fn a_request_that_cannot_be_sent_fails_immediately_with_a_reason() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
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
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
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
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
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

/// The demo's reason for existing: a click that resolves *and* moves the
/// equipment, with no RTAC anywhere.
///
/// Both halves are asserted, and separately. `sent` says the signal got out —
/// which on a demo means the API stood in for the collector. Alarm 607
/// (`ac_breaker_closed`) says where the breaker now sits, read back through the
/// alarm endpoint exactly as it would be against real hardware. A change that
/// resolved the request without moving the point would pass the first
/// assertion, and leave the diagram unchanged.
#[tokio::test]
async fn a_demo_request_is_sent_and_the_readback_follows_it() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    assert_eq!(readback_state(&client, &session, 607).await, None, "nothing reported yet");

    let (status, request) = request_control(&client, &session, "feeder-1a", "close").await;

    assert_eq!(status, Status::Ok);
    assert_eq!(request["status"], json!("sent"), "the demo is the collector");
    assert_ne!(request["sent_at"], json!(null), "a sent request says when");
    assert_eq!(request["failure_reason"], json!(null));

    assert_eq!(
        readback_state(&client, &session, 607).await,
        Some(true),
        "52-MP-1A reports closed once the demo has carried the request out"
    );
}

/// The polarity test, and the one worth having: the site does not report both
/// halves of the diagram the same way round.
///
/// The line switches report *open* (101 `bps_89l1_open`) and the feeder
/// breakers report *closed* (607 `ac_breaker_closed`), so the same pair of
/// actions has to drive the two points in opposite directions. A table with a
/// sense inverted would still pass every other test here, and would draw half
/// the diagram backwards.
#[tokio::test]
async fn a_demo_readback_moves_in_the_direction_the_site_reports_it() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // A point that reports *open*: set when open, clear when closed.
    request_control(&client, &session, "switch-89l-1", "open").await;
    assert_eq!(readback_state(&client, &session, 101).await, Some(true), "89L-1 reports open");

    request_control(&client, &session, "switch-89l-1", "close").await;
    assert_eq!(
        readback_state(&client, &session, 101).await,
        Some(false),
        "closing 89L-1 must clear the point that means open, not set it"
    );

    // A point that reports *closed*, from the same pair of actions.
    request_control(&client, &session, "feeder-2c", "close").await;
    assert_eq!(
        readback_state(&client, &session, 757).await,
        Some(true),
        "52-MP-2C reports closed"
    );

    request_control(&client, &session, "feeder-2c", "open").await;
    assert_eq!(
        readback_state(&client, &session, 757).await,
        Some(false),
        "opening 52-MP-2C must clear the point that means closed"
    );
}

/// Tripping leaves equipment open, which is what lets the lockout relay share a
/// readback vocabulary with the switches despite accepting neither open nor
/// close.
#[tokio::test]
async fn a_demo_trip_leaves_the_equipment_open() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let (status, request) = request_control(&client, &session, "lockout-relay", "trip").await;

    assert_eq!(status, Status::Ok);
    assert_eq!(request["status"], json!("sent"));
    assert_eq!(
        readback_state(&client, &session, 103).await,
        Some(true),
        "86-M1 set is the relay in its tripped, open position"
    );
}

/// One element's click must not move another's. The demo writes a snapshot of
/// every alarm on each request — a snapshot that dropped the others would read
/// as every breaker on the site changing at once.
#[tokio::test]
async fn a_demo_request_moves_only_its_own_readback() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    request_control(&client, &session, "feeder-1a", "close").await;
    request_control(&client, &session, "feeder-1b", "close").await;

    assert_eq!(
        readback_state(&client, &session, 607).await,
        Some(true),
        "52-MP-1A still closed"
    );
    assert_eq!(readback_state(&client, &session, 637).await, Some(true), "52-MP-1B closed");
    assert_eq!(readback_state(&client, &session, 667).await, None, "52-MP-1C was never asked");
}

/// End to end over the demo's actual order of operations: seed history, then
/// click a breaker. Everything the site was reporting before the click is still
/// reporting after it.
///
/// Asserts the property rather than a list of alarm numbers because which
/// seeded alarms are up depends on where the seeded pattern falls against the
/// clock — but the property is exactly what broke. A snapshot built from
/// `alarm_state` alone carried none of them, so one click read as the whole
/// site returning to normal at once, in `/Alarms/Active` and in the history the
/// FDNY timeline is drawn from. The deterministic version of this lives in
/// `api::demo`'s unit tests, which write the seeded reading by hand.
#[tokio::test]
async fn clicking_a_breaker_does_not_clear_the_seeded_alarms() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let resp = client
        .post("/api/1/Demo/InjectHistory")
        .cookie(session.clone())
        .json(&json!({ "site_id": 1, "days": 1 }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "seed the demo's history");

    let firing_before = firing_alarm_nums(&client, &session).await;

    request_control(&client, &session, "feeder-1a", "close").await;

    let firing_after = firing_alarm_nums(&client, &session).await;
    for alarm_num in &firing_before {
        assert!(
            firing_after.contains(alarm_num),
            "alarm {alarm_num} was firing before the click and is not after it"
        );
    }
    assert!(firing_after.contains(&607), "and the breaker's own readback moved");
}

/// Demo mode must not invent a write path. A control still has no write
/// register, the collector is still told there is nothing to do, and the
/// difference is only in who resolved the request.
#[tokio::test]
async fn a_demo_deployment_still_advertises_no_writable_control() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    request_control(&client, &session, "feeder-2b", "close").await;

    for control in list_controls(&client, &session).await.as_array().unwrap() {
        assert_eq!(control["writable"], json!(false), "{} claims a write register", control["id"]);
    }

    let resp = client
        .get("/api/1/Sites/1/Controls/Pending")
        .cookie(session.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(
        resp.into_json::<Value>().await.expect("json"),
        json!([]),
        "nothing left pending"
    );
}

/// A demo request moves its readback before it is reported sent, so it has
/// registered by the time the response comes back, and the next poll agrees.
#[tokio::test]
async fn a_demo_request_has_registered_once_its_readback_moves() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let (_, request) = request_control(&client, &session, "feeder-1a", "close").await;
    assert_eq!(request["status"], json!("sent"));
    assert_eq!(request["registered"], json!(true), "the response already says so");

    let controls = list_controls(&client, &session).await;
    let feeder = controls
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == json!("feeder-1a"))
        .unwrap();
    assert_eq!(feeder["latest_request"]["registered"], json!(true), "and so does the next poll");
}

/// Registered means the equipment got there, not that it is still there. A
/// breaker closed on request and opened again on site must not read as a
/// request still waiting — which is all its current position would say.
#[tokio::test]
async fn a_request_stays_registered_after_the_equipment_moves_back() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    request_control(&client, &session, "feeder-1a", "close").await;

    // Someone on site opens it again. On a demo, that is the drawer.
    let resp = client
        .post("/api/1/Demo/AlarmState")
        .cookie(session.clone())
        .json(&json!({ "alarm_num": 607, "active": false }))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(
        readback_state(&client, &session, 607).await,
        Some(false),
        "52-MP-1A is open again"
    );

    let controls = list_controls(&client, &session).await;
    let feeder = controls
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == json!("feeder-1a"))
        .unwrap();
    assert_eq!(feeder["latest_request"]["registered"], json!(true));
}

/// A request that never reached the site has nothing to have registered.
#[tokio::test]
async fn a_failed_request_has_not_registered() {
    let client = Client::tracked(fast_test_rocket_with_demo_mode(false)).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    let (_, request) = request_control(&client, &session, "feeder-1a", "open").await;
    assert_eq!(request["status"], json!("failed"));
    assert_eq!(request["registered"], json!(false));
}
