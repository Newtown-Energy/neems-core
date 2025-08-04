//! tests/collectors.rs

use chrono::{NaiveDate, TimeZone, Timelike, Utc};
use neems_data::collectors::{DataCollector, data_sources};



#[tokio::test]
async fn test_ping_localhost_collector() {
    // This test is designed to be non-intrusive and likely to pass in most dev environments.
    let result = data_sources::ping_localhost(1).await;
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.get("source_id").is_some());
    assert!(json.get("target").is_some());
    assert!(json.get("successful_pings").is_some());
}



#[tokio::test]
async fn test_data_collector_dispatch() {
    // Test a known collector
    let collector_ping = DataCollector::new("ping_localhost".to_string(), 1);
    let result_ping = collector_ping.collect().await;
    assert!(result_ping.is_ok());
    let json = result_ping.unwrap();
    assert_eq!(json["source_id"], 1);

    // Test an unknown collector
    let collector_unknown = DataCollector::new("unknown_collector".to_string(), 3);
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
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&monday_afternoon), "test").0,
        "discharging"
    );

    let friday_evening = NaiveDate::from_ymd_opt(2025, 8, 8) // A Friday
        .unwrap()
        .and_hms_opt(19, 59, 59)
        .unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&friday_evening), "test").0,
        "discharging"
    );

    // Test cases for the "charging" state: Sat-Thurs, 12 AM - 8 AM
    let saturday_morning = NaiveDate::from_ymd_opt(2025, 8, 9) // A Saturday
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&saturday_morning), "test").0,
        "charging"
    );

    let thursday_morning = NaiveDate::from_ymd_opt(2025, 8, 7) // A Thursday
        .unwrap()
        .and_hms_opt(7, 59, 59)
        .unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&thursday_morning), "test").0,
        "charging"
    );

    // Test cases for the "hold" state (outside of other windows)
    let monday_morning = monday_afternoon.with_hour(9).unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&monday_morning), "test").0,
        "hold"
    );

    let friday_night = friday_evening.with_hour(20).unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&friday_night), "test").0,
        "hold"
    );

    let friday_morning = friday_evening.with_hour(4).unwrap();
    assert_eq!(
        data_sources::charging_state_with_level(Utc.from_utc_datetime(&friday_morning), "test").0,
        "hold" // Friday is not in the "charging" day set
    );
}
