
use neems_data::{DataAggregator, NewSource, get_source_by_name, get_recent_readings};
use nix::sys::signal::{self, Signal};
use nix::unistd;
use tempfile::NamedTempFile;
use tokio::time::{sleep, Duration};

fn setup_test_db_for_signal_test() -> (DataAggregator, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    let aggregator = DataAggregator::new(Some(db_path));
    let mut conn = aggregator.establish_connection().unwrap();

    // Create an initial source with a reliable, pure-Rust collector name
    neems_data::create_source(
        &mut conn,
        NewSource {
            name: "charging_state".to_string(),
            description: Some("Initial source".to_string()),
            active: Some(true),
            interval_seconds: Some(1),
        },
    )
    .unwrap();

    (aggregator, temp_file)
}

#[tokio::test]
#[ignore] // This test sends a signal to the whole process, so it should be run carefully.
async fn test_sighup_reloads_sources_and_collects_data() {
    let (aggregator, _temp_file) = setup_test_db_for_signal_test();
    let mut conn = aggregator.establish_connection().unwrap();

    // Start the aggregator in the background
    let aggregation_task = tokio::spawn(async move {
        // The `true` flag enables verbose logging, which helps debug failures.
        let _ = aggregator.start_aggregation(true).await;
    });

    // Allow some time for the aggregator to start and collect from the first source
    sleep(Duration::from_secs(2)).await;

    // Verify that the first source has successfully produced a reading
    let source1 = get_source_by_name(&mut conn, "charging_state").unwrap().unwrap();
    let readings1 = get_recent_readings(&mut conn, source1.id.unwrap(), 1).unwrap();
    assert!(!readings1.is_empty(), "source1 should have produced a reading before SIGHUP");

    // Add a new, valid source to the database
    neems_data::create_source(
        &mut conn,
        NewSource {
            name: "charging_state_battery2".to_string(),
            description: Some("Second source added dynamically".to_string()),
            active: Some(true),
            interval_seconds: Some(1),
        },
    )
    .unwrap();

    // Send SIGHUP to our own process to trigger the source reload
    println!("Sending SIGHUP to process id {}", unistd::getpid());
    signal::kill(unistd::getpid(), Some(Signal::SIGHUP)).unwrap();

    // Allow some time for the aggregator to reload and poll the new source
    sleep(Duration::from_secs(2)).await;

    // Verify that the second source has now successfully produced a reading
    let source2 = get_source_by_name(&mut conn, "charging_state_battery2").unwrap().unwrap();
    let readings2 = get_recent_readings(&mut conn, source2.id.unwrap(), 1).unwrap();
    assert!(!readings2.is_empty(), "source2 should have produced a reading after SIGHUP");

    // Clean up
    aggregation_task.abort();
}
