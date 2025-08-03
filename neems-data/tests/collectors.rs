//! tests/collectors.rs

use chrono::{NaiveDate, TimeZone, Timelike, Utc};
use neems_data::collectors::{DataCollector, data_sources};
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_collector_current_time() {
    let result = data_sources::current_time().await;
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.get("timestamp_utc").is_some());
    assert!(json.get("unix_timestamp").is_some());
    assert!(json.get("milliseconds").is_some());
}

#[tokio::test]
async fn test_collector_random_digits() {
    let result = data_sources::random_digits().await;
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.get("random_integer").is_some());
    assert!(json.get("random_float").is_some());
}

#[tokio::test]
async fn test_ping_localhost_collector() {
    // This test is designed to be non-intrusive and likely to pass in most dev environments.
    let result = data_sources::ping_localhost().await;
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.get("successful_pings").is_some());
}

#[tokio::test]
async fn test_database_file_collectors() {
    // Create a temporary file to act as a mock database file
    let mut tmpfile = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(tmpfile, "some test data for sha1").unwrap();
    let path_str = tmpfile.path().to_str().unwrap().to_string();

    // Test database_modtime collector
    let modtime_result = data_sources::database_modtime(&path_str).await;
    assert!(modtime_result.is_ok(), "modtime check failed");
    let modtime_json = modtime_result.unwrap();
    assert_eq!(modtime_json["file_exists"], true);
    assert!(modtime_json["modified_timestamp_ms"].is_u64());
    assert_eq!(modtime_json["file_path"], path_str, "modtime path mismatch");

    // Test database_sha1 collector
    let sha1_result = data_sources::database_sha1(&path_str).await;
    assert!(sha1_result.is_ok(), "sha1 check failed");
    let sha1_json = sha1_result.unwrap();
    assert_eq!(sha1_json["file_exists"], true);
    assert!(sha1_json["sha1_hash"].is_string());
    assert_eq!(sha1_json["file_path"], path_str, "sha1 path mismatch");
}

#[tokio::test]
async fn test_database_file_collectors_file_not_found() {
    let path_str = "/a/path/that/does/not/exist/file.sqlite";

    // Test modtime on non-existent file
    let modtime_result = data_sources::database_modtime(path_str).await;
    assert!(modtime_result.is_ok());
    assert_eq!(modtime_result.unwrap()["file_exists"], false);

    // Test sha1 on non-existent file
    let sha1_result = data_sources::database_sha1(path_str).await;
    assert!(sha1_result.is_ok());
    assert_eq!(sha1_result.unwrap()["file_exists"], false);
}

#[tokio::test]
async fn test_data_collector_dispatch() {
    // Test a known collector
    let collector_time = DataCollector::new("current_time".to_string(), 1, "".to_string());
    let result_time = collector_time.collect().await;
    assert!(result_time.is_ok());
    assert!(result_time.unwrap().get("unix_timestamp").is_some());

    // Test a collector that requires a db_path
    let collector_sha =
        DataCollector::new("database_sha1".to_string(), 2, "dummy_path".to_string());
    let result_sha = collector_sha.collect().await;
    assert!(result_sha.is_ok()); // The collector itself handles the error gracefully
    assert_eq!(result_sha.unwrap()["file_exists"], false);

    // Test an unknown collector
    let collector_unknown = DataCollector::new("unknown_collector".to_string(), 3, "".to_string());
    let result_unknown = collector_unknown.collect().await;
    assert!(result_unknown.is_err());
}

#[test]
fn test_charging_state() {
    // Test cases for the "discharging" state: Mon-Fri, 4 PM - 8 PM
    let monday_afternoon = NaiveDate::from_ymd_opt(2025, 8, 4) // A Monday
        .unwrap()
        .and_hms_opt(16, 0, 0)
        .unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&monday_afternoon)).0,
        "discharging"
    );

    let friday_evening = NaiveDate::from_ymd_opt(2025, 8, 8) // A Friday
        .unwrap()
        .and_hms_opt(19, 59, 59)
        .unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&friday_evening)).0,
        "discharging"
    );

    // Test cases for the "charging" state: Sat-Thurs, 12 AM - 8 AM
    let saturday_morning = NaiveDate::from_ymd_opt(2025, 8, 9) // A Saturday
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&saturday_morning)).0,
        "charging"
    );

    let thursday_morning = NaiveDate::from_ymd_opt(2025, 8, 7) // A Thursday
        .unwrap()
        .and_hms_opt(7, 59, 59)
        .unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&thursday_morning)).0,
        "charging"
    );

    // Test cases for the "hold" state (outside of other windows)
    let monday_morning = monday_afternoon.with_hour(9).unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&monday_morning)).0,
        "hold"
    );

    let friday_night = friday_evening.with_hour(20).unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&friday_night)).0,
        "hold"
    );

    let friday_morning = friday_evening.with_hour(4).unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&friday_morning)).0,
        "hold" // Friday is not in the "charging" day set
    );
}
