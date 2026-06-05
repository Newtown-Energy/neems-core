//! End-to-end test: drive the simulated RTAC with the production
//! `neems_data::rtac::ModbusClient`, validating the Modbus wire layer.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use neems_data::rtac::{
    modbus_client::{ModbusClient, ModbusClientConfig},
    protocol::OperatingMode,
    state::PendingCommand,
};
use neems_rtac_sim::{config::SimConfig, server, state::SimState};

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

    // Give the server a moment to start accepting.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect the real production client to the simulator.
    let client_config = ModbusClientConfig { address: addr, ..Default::default() };
    let mut client = ModbusClient::new(client_config);
    client.connect().await.expect("client should connect to simulator");

    // Initial read: standby at the configured initial SoC.
    let status = client.read_status().await.expect("read status");
    assert_eq!(status.mode, OperatingMode::Standby);
    let initial_soc = status.soc_percent;
    assert!((initial_soc - 50.0).abs() < 0.01);

    // Command a charge to 90% and let the simulation run.
    let charge = PendingCommand::charge(Some(90), None, 0);
    client.write_command(&charge).await.expect("write charge command");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let status = client.read_status().await.expect("read status after charge");
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
    tokio::time::sleep(Duration::from_millis(300)).await;

    let status = client.read_status().await.expect("read status after discharge");
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
