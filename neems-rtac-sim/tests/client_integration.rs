//! End-to-end test: drive the simulated RTAC with the production
//! `neems_data::rtac::ModbusClient`, validating the Modbus wire layer.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use neems_data::rtac::{
    modbus_client::{ModbusClient, ModbusClientConfig},
    protocol::{OperatingMode, ParsedStatus},
    state::PendingCommand,
};
use neems_rtac_sim::{config::SimConfig, server, state::SimState};

/// Connect the client, retrying until the server is accepting or the timeout
/// elapses. Avoids a fixed startup sleep that can be too short under CI load.
async fn connect_with_retry(config: ModbusClientConfig, timeout: Duration) -> ModbusClient {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let mut client = ModbusClient::new(config.clone());
        match client.connect().await {
            Ok(()) => return client,
            Err(e) => {
                if tokio::time::Instant::now() >= deadline {
                    panic!("client never connected to simulator: {e}");
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

/// Poll `read_status` until `pred` holds or the timeout elapses, so the test
/// waits exactly as long as the simulator needs (and fails with a clear message
/// if it never advances) rather than relying on a fixed sleep.
async fn wait_for_status<F>(
    client: &mut ModbusClient,
    timeout: Duration,
    label: &str,
    mut pred: F,
) -> ParsedStatus
where
    F: FnMut(&ParsedStatus) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let status = client.read_status().await.expect("read status");
        if pred(&status) {
            return status;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for {label}; last status: {status:?}");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_can_read_and_command_the_simulator() {
    let config = SimConfig {
        bind_address: "127.0.0.1:0".parse().unwrap(),
        initial_soc_percent: 50.0,
        charge_rate_pct: 5.0,
        tick_interval: Duration::from_millis(50),
        ..SimConfig::default()
    };
    let tick_interval = config.tick_interval;
    let state = Arc::new(Mutex::new(SimState::new(config.clone())));

    // Bind first so the listener accepts connections immediately.
    let (listener, addr) = server::bind(config.bind_address).await.unwrap();

    // Physics tick loop.
    let tick_state = state.clone();
    let tick = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick_interval);
        loop {
            interval.tick().await;
            tick_state.lock().unwrap().tick();
        }
    });

    // Modbus server.
    let server_state = state.clone();
    let srv = tokio::spawn(async move {
        let _ = server::serve(listener, server_state).await;
    });

    // Connect the real production client to the simulator.
    let client_config = ModbusClientConfig { address: addr, ..Default::default() };
    let mut client = connect_with_retry(client_config, Duration::from_secs(5)).await;

    // Initial read: standby at the configured initial SoC.
    let status = client.read_status().await.expect("read status");
    assert_eq!(status.mode, OperatingMode::Standby);
    let initial_soc = status.soc_percent;
    assert!((initial_soc - 50.0).abs() < 0.01);

    // Command a charge to 90% and let the simulation run.
    let charge = PendingCommand::charge(Some(90), None, 0);
    client.write_command(&charge).await.expect("write charge command");
    let status =
        wait_for_status(&mut client, Duration::from_secs(5), "charging SoC to rise", |s| {
            s.mode == OperatingMode::Charging && s.soc_percent > initial_soc
        })
        .await;
    assert_eq!(status.mode, OperatingMode::Charging, "should be charging");
    assert!(
        status.soc_percent > initial_soc,
        "SoC should have trickled up: {} > {}",
        status.soc_percent,
        initial_soc
    );

    // Now discharge and confirm the SoC trends back down.
    let high_soc = status.soc_percent;
    let discharge = PendingCommand::discharge(None, 0);
    client.write_command(&discharge).await.expect("write discharge command");
    let status =
        wait_for_status(&mut client, Duration::from_secs(5), "discharging SoC to fall", |s| {
            s.mode == OperatingMode::Discharging && s.soc_percent < high_soc
        })
        .await;
    assert_eq!(status.mode, OperatingMode::Discharging, "should be discharging");
    assert!(
        status.soc_percent < high_soc,
        "SoC should have trickled down: {} < {}",
        status.soc_percent,
        high_soc
    );

    tick.abort();
    srv.abort();
}
