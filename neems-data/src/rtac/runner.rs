//! RTAC collector orchestration.
//!
//! Wires the [`ModbusWorker`](super::worker::ModbusWorker) together with the
//! storage, alarm-handler, and control-logic tasks and runs them as a unit.
//! This is the entry point used by `neems-data monitor` to poll the RTAC (or
//! the simulated RTAC), persist State-of-Charge readings, and write the active
//! schedule command back to the RTAC.
//!
//! The active command is sourced from neems-api over HTTP (see
//! [`schedule_http`](super::schedule_http)); the [`ControlLogicTask`] turns it
//! into RTAC commands and also applies reactive SoC/alarm safety overrides.

use std::{
    env,
    error::Error,
    sync::{Arc, Mutex},
};

use diesel::{Connection, sqlite::SqliteConnection};
use tracing::{error, info};

use super::{
    alarms::{AlarmConfig, AlarmHandlerTask, create_alarm_channel},
    control::{ControlConfig, ControlLogicTask},
    schedule_http::{ApiClientConfig, HttpScheduleProvider, run_active_command_poller},
    state::PendingCommand,
    storage::{DatabaseStorageBackend, StorageConfig, StorageWriterTask, create_storage_channel},
    worker::{ModbusWorker, RtacConfig, create_worker_channels},
};
use crate::{NewSource, create_source, get_source_by_name};

/// Name of the source the RTAC worker writes SoC readings to.
const RTAC_SOURCE_NAME: &str = "rtac";

type DynError = Box<dyn Error + Send + Sync>;

/// Read an integer environment variable, falling back to `default`.
fn env_i32(key: &str, default: i32) -> i32 {
    env::var(key).ok().and_then(|v| v.parse::<i32>().ok()).unwrap_or(default)
}

/// Ensure a `charging_state` source exists for the RTAC and return its id.
///
/// The source is created `active = false` so the generic `DataAggregator`
/// poller does not also write to it — the RTAC worker is its sole writer. The
/// SoC history endpoint serves its readings regardless of the `active` flag.
fn ensure_rtac_source(database_url: &str, site_id: i32, company_id: i32) -> Result<i32, DynError> {
    let mut conn = SqliteConnection::establish(database_url)?;

    if let Some(existing) = get_source_by_name(&mut conn, RTAC_SOURCE_NAME)? {
        if let Some(id) = existing.id {
            return Ok(id);
        }
    }

    let new_source = NewSource {
        name: RTAC_SOURCE_NAME.to_string(),
        description: Some("RTAC State-of-Charge readings (Modbus worker)".to_string()),
        active: Some(false),
        interval_seconds: Some(1),
        test_type: Some("charging_state".to_string()),
        arguments: None,
        site_id: Some(site_id),
        company_id: Some(company_id),
    };

    let source = create_source(&mut conn, new_source)?;
    source.id.ok_or_else(|| "created RTAC source has no id".into())
}

/// Run the RTAC collector until the worker stops.
///
/// `database_url` is the site database URL (e.g. `sqlite:///app/data/...`).
/// The RTAC endpoint and slave id come from the environment via
/// [`RtacConfig::from_env`].
pub async fn run_rtac_collector(database_url: String) -> Result<(), DynError> {
    let site_id = env_i32("NEEMS_DEFAULT_SITE", 1);
    let company_id = env_i32("NEEMS_DEFAULT_COMPANY", 1);

    // Ensure the destination source exists (blocking DB work off the runtime).
    let source_id = {
        let database_url = database_url.clone();
        tokio::task::spawn_blocking(move || ensure_rtac_source(&database_url, site_id, company_id))
            .await??
    };

    let config = RtacConfig::from_env();
    info!(
        address = %config.rtac_address,
        slave_id = config.slave_id,
        source_id,
        "Starting RTAC collector"
    );

    // Build the inter-task channels. `command_tx` goes to the control logic,
    // `command_rx` to the worker; `shutdown_tx` is held for the lifetime of the
    // collector so the worker's shutdown channel is not seen as closed.
    let (command_tx, command_rx) = tokio::sync::watch::channel::<Option<PendingCommand>>(None);
    let (storage_tx, storage_rx) = create_storage_channel(256);
    let (alarm_tx, alarm_rx) = create_alarm_channel();
    let (channels, shutdown_tx) = create_worker_channels(command_rx, storage_tx, alarm_tx);
    // The control logic reads the same shared state the worker updates.
    let shared_state = channels.state.clone();
    let _shutdown_tx = shutdown_tx;

    // Storage task: persist readings to the site database.
    let backend = DatabaseStorageBackend::new(database_url, source_id);
    let mut storage_task =
        StorageWriterTask::new(StorageConfig::default(), backend, storage_rx, None);
    tokio::spawn(async move {
        if let Err(e) = storage_task.run().await {
            error!(error = %e, "RTAC storage task stopped");
        }
    });

    // Alarm task: log alarm transitions.
    let mut alarm_task = AlarmHandlerTask::new(AlarmConfig::default(), alarm_rx);
    tokio::spawn(async move {
        if let Err(e) = alarm_task.run().await {
            error!(error = %e, "RTAC alarm task stopped");
        }
    });

    // Active-command poller: fetch the current schedule command from neems-api
    // into a shared cache that the control logic reads.
    let command_cache = Arc::new(Mutex::new(None));
    let api_config = ApiClientConfig::from_env(site_id);
    tokio::spawn(run_active_command_poller(api_config, command_cache.clone()));

    // Control logic: turn the active command into RTAC commands (with reactive
    // SoC/alarm safety overrides) and write them via the command channel.
    let schedule_provider = HttpScheduleProvider::new(command_cache);
    let mut control_task = ControlLogicTask::new(
        ControlConfig::default(),
        schedule_provider,
        shared_state,
        command_tx,
    );
    tokio::spawn(async move {
        if let Err(e) = control_task.run().await {
            error!(error = %e, "RTAC control logic task stopped");
        }
    });

    // Worker loop: reads status, samples to storage, forwards alarm changes,
    // and writes the active command to the RTAC.
    let mut worker = ModbusWorker::new(config, channels);
    worker.run().await?;

    Ok(())
}
