use std::collections::HashMap;

use neems_api::{
    models::{
        ApplicationRule, CalendarDaySchedule, EffectiveScheduleResponse, RuleType,
        ScheduleLibraryItem,
    },
    orm::testing::fast_test_rocket,
};
use rocket::{
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use serde_json::json;

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": "superadmin@example.com",
        "password": "admin"
    });

    let response = client
        .post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

/// Helper to create a library item
async fn create_library_item(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    name: &str,
) -> ScheduleLibraryItem {
    let new_item = json!({
        "name": name,
        "commands": []
    });

    let response = client
        .post("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .json(&new_item)
        .dispatch()
        .await;

    response.into_json().await.expect("valid JSON")
}

#[rocket::async_test]
async fn test_application_rule_endpoints_require_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Test all endpoints require authentication
    let response = client.get("/api/1/ScheduleLibraryItems/1/ApplicationRules").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/Sites/1/ApplicationRules").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    let response = client.get("/api/1/Sites/1/EffectiveSchedule?date=2025-01-15").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_create_default_rule() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a library item
    let item = create_library_item(&client, &admin_cookie, "Test Schedule").await;

    // Create default rule
    let rule_request = json!({
        "rule_type": "default",
        "days_of_week": null,
        "specific_dates": null,
        "override_reason": null
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item.id);
    let response = client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&rule_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let rule: ApplicationRule = response.into_json().await.expect("valid JSON");
    assert_eq!(rule.rule_type, RuleType::Default);
    assert_eq!(rule.library_item_id, item.id);
}

#[rocket::async_test]
async fn test_create_day_of_week_rule() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a library item
    let item = create_library_item(&client, &admin_cookie, "Weekday Schedule").await;

    // Create day-of-week rule for weekdays (Monday-Friday)
    let rule_request = json!({
        "rule_type": "day_of_week",
        "days_of_week": [1, 2, 3, 4, 5], // Monday-Friday
        "specific_dates": null,
        "override_reason": null
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item.id);
    let response = client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&rule_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let rule: ApplicationRule = response.into_json().await.expect("valid JSON");
    assert_eq!(rule.rule_type, RuleType::DayOfWeek);
    assert_eq!(rule.days_of_week, Some(vec![1, 2, 3, 4, 5]));
}

#[rocket::async_test]
async fn test_create_specific_date_rule() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a library item
    let item = create_library_item(&client, &admin_cookie, "Holiday Schedule").await;

    // Create specific-date rule
    let rule_request = json!({
        "rule_type": "specific_date",
        "days_of_week": null,
        "specific_dates": ["2025-12-25", "2025-01-01"],
        "override_reason": "Holiday override"
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item.id);
    let response = client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&rule_request)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Created);
    let rule: ApplicationRule = response.into_json().await.expect("valid JSON");
    assert_eq!(rule.rule_type, RuleType::SpecificDate);
    assert_eq!(
        rule.specific_dates,
        Some(vec!["2025-12-25".to_string(), "2025-01-01".to_string()])
    );
    assert_eq!(rule.override_reason, Some("Holiday override".to_string()));
}

#[rocket::async_test]
async fn test_only_one_default_rule_per_site() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create two library items
    let item1 = create_library_item(&client, &admin_cookie, "Schedule 1").await;
    let item2 = create_library_item(&client, &admin_cookie, "Schedule 2").await;

    // Create default rule for item1
    let rule_request = json!({
        "rule_type": "default",
        "days_of_week": null,
        "specific_dates": null,
        "override_reason": null
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item1.id);
    let response = client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&rule_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);

    // Try to create default rule for item2 - should succeed but delete the old one
    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item2.id);
    let response = client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&rule_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Created);

    // Verify only item2 has the default rule
    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item1.id);
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;
    let rules: Vec<ApplicationRule> = response.into_json().await.expect("valid JSON");
    assert!(rules.is_empty(), "item1 should have no rules");

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item2.id);
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;
    let rules: Vec<ApplicationRule> = response.into_json().await.expect("valid JSON");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].rule_type, RuleType::Default);
}

#[rocket::async_test]
async fn test_get_application_rules_for_library_item() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a library item
    let item = create_library_item(&client, &admin_cookie, "Multi Rule Schedule").await;

    // Create multiple rules
    let day_rule = json!({
        "rule_type": "day_of_week",
        "days_of_week": [0, 6], // Weekend
        "specific_dates": null,
        "override_reason": null
    });

    let date_rule = json!({
        "rule_type": "specific_date",
        "days_of_week": null,
        "specific_dates": ["2025-07-04"],
        "override_reason": "Independence Day"
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item.id);
    client.post(&url).cookie(admin_cookie.clone()).json(&day_rule).dispatch().await;
    client.post(&url).cookie(admin_cookie.clone()).json(&date_rule).dispatch().await;

    // Get all rules for this item
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let rules: Vec<ApplicationRule> = response.into_json().await.expect("valid JSON");
    assert_eq!(rules.len(), 2);
}

#[rocket::async_test]
async fn test_get_application_rules_for_site() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create items and rules
    let item1 = create_library_item(&client, &admin_cookie, "Site Rule Test 1").await;
    let item2 = create_library_item(&client, &admin_cookie, "Site Rule Test 2").await;

    let rule1 = json!({
        "rule_type": "day_of_week",
        "days_of_week": [1, 2, 3, 4, 5],
        "specific_dates": null,
        "override_reason": null
    });

    let rule2 = json!({
        "rule_type": "day_of_week",
        "days_of_week": [0, 6],
        "specific_dates": null,
        "override_reason": null
    });

    let url1 = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item1.id);
    client.post(&url1).cookie(admin_cookie.clone()).json(&rule1).dispatch().await;

    let url2 = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item2.id);
    client.post(&url2).cookie(admin_cookie.clone()).json(&rule2).dispatch().await;

    // Get all rules for the site
    let response = client
        .get("/api/1/Sites/1/ApplicationRules")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let rules: Vec<ApplicationRule> = response.into_json().await.expect("valid JSON");
    assert!(rules.len() >= 2);
}

#[rocket::async_test]
async fn test_delete_application_rule() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create a library item and rule
    let item = create_library_item(&client, &admin_cookie, "Delete Rule Test").await;

    let rule_request = json!({
        "rule_type": "day_of_week",
        "days_of_week": [1],
        "specific_dates": null,
        "override_reason": null
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", item.id);
    let response = client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&rule_request)
        .dispatch()
        .await;
    let rule: ApplicationRule = response.into_json().await.expect("valid JSON");

    // Delete the rule
    let delete_url = format!("/api/1/ApplicationRules/{}", rule.id);
    let response = client.delete(&delete_url).cookie(admin_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), Status::NoContent);

    // Verify it's gone
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;
    let rules: Vec<ApplicationRule> = response.into_json().await.expect("valid JSON");
    assert_eq!(rules.len(), 0);
}

#[rocket::async_test]
async fn test_effective_schedule_default() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get library items (ensures default exists)
    let response = client
        .get("/api/1/Sites/1/ScheduleLibraryItems")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    let items: Vec<ScheduleLibraryItem> = response.into_json().await.expect("valid JSON");
    let default_item = items.iter().find(|i| i.name == "Default").expect("Default should exist");

    // Get effective schedule - should return default
    let response = client
        .get("/api/1/Sites/1/EffectiveSchedule?date=2025-01-15")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let effective: EffectiveScheduleResponse = response.into_json().await.expect("valid JSON");
    assert_eq!(effective.library_item.id, default_item.id);
    assert_eq!(effective.specificity, 0); // Default rule
}

#[rocket::async_test]
async fn test_effective_schedule_precedence() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Create schedules with different precedence levels
    let weekday = create_library_item(&client, &admin_cookie, "Weekday").await;
    let holiday = create_library_item(&client, &admin_cookie, "Holiday").await;

    // Weekday rule (Monday = 1)
    let weekday_rule = json!({
        "rule_type": "day_of_week",
        "days_of_week": [1],
        "specific_dates": null,
        "override_reason": null
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", weekday.id);
    client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&weekday_rule)
        .dispatch()
        .await;

    // Specific date rule for Monday 2025-01-13
    let holiday_rule = json!({
        "rule_type": "specific_date",
        "days_of_week": null,
        "specific_dates": ["2025-01-13"],
        "override_reason": "Holiday"
    });

    let url = format!("/api/1/ScheduleLibraryItems/{}/ApplicationRules", holiday.id);
    client
        .post(&url)
        .cookie(admin_cookie.clone())
        .json(&holiday_rule)
        .dispatch()
        .await;

    // 2025-01-13 is a Monday, both rules match
    // Specific date should win (higher precedence)
    let response = client
        .get("/api/1/Sites/1/EffectiveSchedule?date=2025-01-13")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let effective: EffectiveScheduleResponse = response.into_json().await.expect("valid JSON");
    assert_eq!(effective.library_item.id, holiday.id);
    assert_eq!(effective.specificity, 2); // Specific date
}

#[rocket::async_test]
async fn test_calendar_schedules() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get calendar for January 2025
    let response = client
        .get("/api/1/Sites/1/CalendarSchedules?year=2025&month=1")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let calendar: HashMap<String, CalendarDaySchedule> =
        response.into_json().await.expect("valid JSON");

    // January has 31 days, should have entries for all days
    assert_eq!(calendar.len(), 31);

    // Check a specific date
    let jan_15 = calendar.get("2025-01-15").expect("2025-01-15 should exist");
    assert!(jan_15.library_item_id > 0);
    assert_eq!(jan_15.specificity, 0); // Default rule
}

#[rocket::async_test]
async fn test_calendar_schedules_invalid_month() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Invalid month (13)
    let response = client
        .get("/api/1/Sites/1/CalendarSchedules?year=2025&month=13")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::InternalServerError);
}

#[rocket::async_test]
async fn test_effective_schedule_invalid_date_format() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Invalid date format
    let response = client
        .get("/api/1/Sites/1/EffectiveSchedule?date=01-15-2025")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}
