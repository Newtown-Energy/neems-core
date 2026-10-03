//! Integration tests for the site design endpoint.

use neems_api::orm::testing::fast_test_rocket;
use rocket::{http::Status, local::asynchronous::Client, tokio};
use serde_json::Value;

/// A client may need the design before anyone logs in, and the test
/// environment runs the default design.
#[tokio::test]
async fn reports_the_active_design_without_a_login() {
    let client = Client::tracked(fast_test_rocket()).await.expect("client");
    let response = client.get("/api/1/SiteDesign").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: Value = response.into_json().await.expect("json");
    assert_eq!(body["id"], "newtown");
    assert_eq!(body["estop_alarm_num"], 104);
}
