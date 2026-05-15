//! Integration tests for the demo forced-alarms endpoint.
//!
//! The endpoint is temporary scaffolding for demos — it lets an
//! admin / newtown-admin / newtown-staff push a synthetic set of alarm
//! numbers that get unioned into the `/Alarms/Active` response, without
//! needing the RTAC feed to be live.

use neems_api::orm::testing::fast_test_rocket;
use rocket::{http::Status, local::asynchronous::Client, tokio};
use serde_json::{Value, json};

async fn login_as(
    client: &Client,
    email: &str,
    password: &str,
) -> rocket::http::Cookie<'static> {
    let body = json!({ "email": email, "password": password });
    let resp = client.post("/api/1/login").json(&body).dispatch().await;
    assert_eq!(resp.status(), Status::Ok, "login failed for {}", email);
    resp.cookies().get("session").expect("session cookie").clone().into_owned()
}

#[tokio::test]
async fn forced_alarms_round_trip_as_newtown_admin() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "newtown_superadmin@example.com", "newtownpass").await;

    // Start with an empty forced set.
    let get_resp = client.get("/api/1/Alarms/Forced").cookie(session.clone()).dispatch().await;
    assert_eq!(get_resp.status(), Status::Ok);
    let initial: Value = get_resp.into_json().await.expect("json");
    assert_eq!(initial["alarm_nums"], json!([]));

    // Force alarm 401 (fire_alarm — Emergency level) on. PUT replaces the
    // set wholesale.
    let put_resp = client
        .put("/api/1/Alarms/Forced")
        .cookie(session.clone())
        .json(&json!({ "alarm_nums": [401] }))
        .dispatch()
        .await;
    assert_eq!(put_resp.status(), Status::Ok);
    let after_put: Value = put_resp.into_json().await.expect("json");
    assert_eq!(after_put["alarm_nums"], json!([401]));

    // GET reflects the new state.
    let get2 = client.get("/api/1/Alarms/Forced").cookie(session.clone()).dispatch().await;
    let after_get: Value = get2.into_json().await.expect("json");
    assert_eq!(after_get["alarm_nums"], json!([401]));

    // /Alarms/Active overlays the forced alarm into its response.
    let active = client.get("/api/1/Alarms/Active").cookie(session.clone()).dispatch().await;
    assert_eq!(active.status(), Status::Ok);
    let active_body: Value = active.into_json().await.expect("json");
    let nums: Vec<u16> = active_body["alarms"]
        .as_array()
        .expect("alarms array")
        .iter()
        .map(|a| a["alarm_num"].as_u64().expect("alarm_num") as u16)
        .collect();
    assert!(nums.contains(&401), "expected forced alarm 401 in /Active, got {:?}", nums);
    assert_eq!(active_body["has_emergency"], json!(true));

    // Clear by PUTing an empty list.
    let clear = client
        .put("/api/1/Alarms/Forced")
        .cookie(session.clone())
        .json(&json!({ "alarm_nums": [] }))
        .dispatch()
        .await;
    let cleared: Value = clear.into_json().await.expect("json");
    assert_eq!(cleared["alarm_nums"], json!([]));

    // /Active no longer shows the forced alarm.
    let active2 = client.get("/api/1/Alarms/Active").cookie(session).dispatch().await;
    let active2_body: Value = active2.into_json().await.expect("json");
    let nums2: Vec<u16> = active2_body["alarms"]
        .as_array()
        .map(|arr| arr.iter().map(|a| a["alarm_num"].as_u64().unwrap_or(0) as u16).collect())
        .unwrap_or_default();
    assert!(!nums2.contains(&401));
}

#[tokio::test]
async fn forced_alarms_filters_unknown_numbers() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "test_superadmin@example.com", "adminpass").await;

    // 65000 is well outside any defined alarm_num. The server should
    // silently filter it out and persist only known numbers.
    let put = client
        .put("/api/1/Alarms/Forced")
        .cookie(session.clone())
        .json(&json!({ "alarm_nums": [104, 65000] }))
        .dispatch()
        .await;
    let body: Value = put.into_json().await.expect("json");
    assert_eq!(body["alarm_nums"], json!([104]));

    // Reset to leave the in-memory state clean for other tests.
    let _ = client
        .put("/api/1/Alarms/Forced")
        .cookie(session)
        .json(&json!({ "alarm_nums": [] }))
        .dispatch()
        .await;
}

#[tokio::test]
async fn forced_alarms_rejects_non_demo_role() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let session = login_as(&client, "staff@example.com", "staffpass").await;

    let get_resp = client.get("/api/1/Alarms/Forced").cookie(session.clone()).dispatch().await;
    assert_eq!(get_resp.status(), Status::Forbidden);

    let put_resp = client
        .put("/api/1/Alarms/Forced")
        .cookie(session)
        .json(&json!({ "alarm_nums": [104] }))
        .dispatch()
        .await;
    assert_eq!(put_resp.status(), Status::Forbidden);
}

#[tokio::test]
async fn forced_alarms_requires_auth() {
    let client = Client::tracked(fast_test_rocket()).await.unwrap();
    let resp = client.get("/api/1/Alarms/Forced").dispatch().await;
    assert_eq!(resp.status(), Status::Unauthorized);
}
