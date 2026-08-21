//! Storage Writer Task
//!
//! This module implements the storage task that:
//! - Receives readings from the Modbus worker via mpsc channel
//! - Batches readings for efficient database writes
//! - Supports configurable sample rates and decimation

use std::{collections::VecDeque, time::Duration};

use chrono::{DateTime, Utc};
use diesel::{Connection, sqlite::SqliteConnection};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::{alarm_definitions::ALARM_REGISTER_COUNT, state::RtacReading};
use crate::{insert_readings_batch, models::NewReading};

/// Configuration for the storage writer task
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// How often to flush batched readings to storage (default: 1 second)
    pub flush_interval: Duration,
    /// Maximum batch size before forcing a flush
    pub max_batch_size: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            flush_interval: Duration::from_secs(1),
            max_batch_size: 100,
        }
    }
}

/// Statistics for the storage writer
#[derive(Debug, Default, Clone)]
pub struct StorageStats {
    /// Total readings received
    pub readings_received: u64,
    /// Total readings written to storage
    pub readings_written: u64,
    /// Number of batch writes
    pub batch_writes: u64,
    /// Number of failed writes
    pub failed_writes: u64,
}

/// Trait for storage backends
///
/// This trait allows different storage implementations (database, file, etc.)
/// The write_batch method is async to prevent blocking the Tokio runtime
/// when performing I/O operations (e.g., database writes).
pub trait StorageBackend: Send + Sync {
    /// Write a batch of readings asynchronously
    ///
    /// For backends that perform blocking I/O, implementations should use
    /// `tokio::task::spawn_blocking` internally to avoid blocking the runtime.
    fn write_batch(
        &mut self,
        readings: Vec<StorageReading>,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send;
}

/// A reading prepared for storage
#[derive(Debug, Clone)]
pub struct StorageReading {
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

/// Map an RTAC operating-mode string to the `state` value used by the
/// `charging_state` SoC history (charging / discharging / hold).
fn soc_state_from_mode(mode: &str) -> &'static str {
    match mode {
        "charging" | "trickle_charge" => "charging",
        "discharging" => "discharging",
        _ => "hold",
    }
}

impl From<RtacReading> for StorageReading {
    fn from(reading: RtacReading) -> Self {
        // `level` and `state` mirror the shape the `charging_state` collector
        // writes, so RTAC readings are served by the SoC history endpoint
        // (which reads `level`/`state`). The remaining fields preserve the full
        // RTAC reading for richer consumers.
        // Per-pack analogs, keyed by zone so consumers can address a single
        // Megapack. A pack that did not answer is absent from the map rather
        // than present with zeros — on a charge gauge those read as opposite
        // claims.
        let megapacks: serde_json::Map<String, serde_json::Value> = reading
            .megapack_analogs
            .iter()
            .map(|mp| {
                (
                    format!("{:?}", mp.zone),
                    json!({
                        "state_of_energy": mp.state_of_energy_percent,
                        "ac_voltage": mp.ac_voltage_v,
                        "max_battery_temperature": mp.max_battery_temperature_c,
                    }),
                )
            })
            .collect();

        let data = json!({
            "level": reading.soc_percent,
            "state": soc_state_from_mode(&reading.mode),
            "soc_percent": reading.soc_percent,
            "power_kw": reading.power_kw,
            "mode": reading.mode,
            "voltage_v": reading.voltage_v,
            "current_a": reading.current_a,
            "temperature_c": reading.temperature_c,
            "grid_frequency_hz": reading.grid_frequency_hz,
            "alarm_registers": reading.alarm_registers,
            "megapacks": megapacks,
            "sequence": reading.sequence,
        });

        Self { timestamp: reading.timestamp, data }
    }
}

/// In-memory storage backend for testing
///
/// Uses `VecDeque` for O(1) removal of oldest readings when at capacity.
pub struct InMemoryStorage {
    readings: VecDeque<StorageReading>,
    max_readings: usize,
}

impl InMemoryStorage {
    pub fn new(max_readings: usize) -> Self {
        Self {
            readings: VecDeque::with_capacity(max_readings),
            max_readings,
        }
    }

    pub fn readings(&self) -> impl Iterator<Item = &StorageReading> {
        self.readings.iter()
    }

    pub fn len(&self) -> usize {
        self.readings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.readings.is_empty()
    }

    pub fn clear(&mut self) {
        self.readings.clear();
    }
}

impl StorageBackend for InMemoryStorage {
    async fn write_batch(
        &mut self,
        readings: Vec<StorageReading>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for reading in readings {
            if self.readings.len() >= self.max_readings {
                // Remove oldest reading (O(1) with VecDeque)
                self.readings.pop_front();
            }
            self.readings.push_back(reading);
        }
        Ok(())
    }
}

/// Database-backed storage that writes RTAC readings to the site SQLite DB.
///
/// Readings are written to the `readings` table under a fixed `source_id` (the
/// RTAC's `charging_state` source). Each write happens on a blocking thread via
/// `spawn_blocking` so the async runtime is never blocked on disk I/O.
pub struct DatabaseStorageBackend {
    database_url: String,
    source_id: i32,
}

impl DatabaseStorageBackend {
    /// Create a backend that writes readings under `source_id` to
    /// `database_url`.
    pub fn new(database_url: String, source_id: i32) -> Self {
        Self { database_url, source_id }
    }
}

impl StorageBackend for DatabaseStorageBackend {
    async fn write_batch(
        &mut self,
        readings: Vec<StorageReading>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if readings.is_empty() {
            return Ok(());
        }

        let database_url = self.database_url.clone();
        let source_id = self.source_id;

        tokio::task::spawn_blocking(
            move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                let new_readings: Vec<NewReading> = readings
                    .into_iter()
                    .map(|r| NewReading {
                        source_id,
                        timestamp: Some(r.timestamp.naive_utc()),
                        data: r.data.to_string(),
                        quality_flags: None,
                    })
                    .collect();

                let mut conn = SqliteConnection::establish(&database_url)?;
                insert_readings_batch(&mut conn, new_readings)?;
                Ok(())
            },
        )
        .await??;

        Ok(())
    }
}

/// Data sampler for decimation
///
/// Collects multiple readings and produces an averaged/decimated reading.
pub struct DataSampler {
    readings: Vec<RtacReading>,
    sample_count: usize,
}

impl DataSampler {
    /// Create a new sampler that averages `sample_count` readings
    pub fn new(sample_count: usize) -> Self {
        Self {
            readings: Vec::with_capacity(sample_count),
            sample_count,
        }
    }

    /// Add a reading to the sampler
    pub fn add(&mut self, reading: RtacReading) {
        self.readings.push(reading);
    }

    /// Check if the sampler has enough readings
    pub fn is_ready(&self) -> bool {
        self.readings.len() >= self.sample_count
    }

    /// Get the averaged/decimated reading and reset the sampler
    pub fn take_averaged(&mut self) -> Option<RtacReading> {
        if self.readings.is_empty() {
            return None;
        }

        let count = self.readings.len() as f32;

        // Use the timestamp of the last reading
        let timestamp = self.readings.last()?.timestamp;

        // Average numeric values
        let soc_percent = self.readings.iter().map(|r| r.soc_percent).sum::<f32>() / count;
        let power_kw = self.readings.iter().map(|r| r.power_kw).sum::<f32>() / count;
        let voltage_v = self.readings.iter().map(|r| r.voltage_v).sum::<f32>() / count;
        let current_a = self.readings.iter().map(|r| r.current_a).sum::<f32>() / count;
        let temperature_c = self.readings.iter().map(|r| r.temperature_c).sum::<f32>() / count;
        let grid_frequency_hz =
            self.readings.iter().map(|r| r.grid_frequency_hz).sum::<f32>() / count;

        // Use the mode from the last reading (can't average enums)
        let mode = self.readings.last()?.mode.clone();

        // OR all alarm registers together (any alarm that was active during the period)
        let alarm_registers =
            self.readings.iter().fold([0u16; ALARM_REGISTER_COUNT], |mut acc, r| {
                for (i, reg) in acc.iter_mut().enumerate() {
                    *reg |= r.alarm_registers[i];
                }
                acc
            });

        // Take the last reading's analogs rather than averaging them. They
        // share the timestamp and sequence we keep, and averaging would mean
        // pairing packs up across readings whose membership can differ — a
        // pack that dropped out mid-window would otherwise be averaged with
        // nothing and read low.
        let megapack_analogs = self.readings.last()?.megapack_analogs.clone();

        // Use the sequence of the last reading
        let sequence = self.readings.last()?.sequence;

        self.readings.clear();

        Some(RtacReading {
            timestamp,
            soc_percent,
            power_kw,
            mode,
            voltage_v,
            current_a,
            temperature_c,
            grid_frequency_hz,
            alarm_registers,
            megapack_analogs,
            sequence,
        })
    }
}

/// The storage writer task
pub struct StorageWriterTask<B: StorageBackend> {
    config: StorageConfig,
    backend: B,
    rx: mpsc::Receiver<RtacReading>,
    batch: Vec<StorageReading>,
    stats: StorageStats,
    sampler: Option<DataSampler>,
}

impl<B: StorageBackend> StorageWriterTask<B> {
    /// Create a new storage writer task
    pub fn new(
        config: StorageConfig,
        backend: B,
        rx: mpsc::Receiver<RtacReading>,
        decimation_factor: Option<usize>,
    ) -> Self {
        let sampler = decimation_factor.map(DataSampler::new);

        Self {
            config,
            backend,
            rx,
            batch: Vec::new(),
            stats: StorageStats::default(),
            sampler,
        }
    }

    /// Run the storage writer loop
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting storage writer task");

        let mut interval = tokio::time::interval(self.config.flush_interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Periodic flush
                    if !self.batch.is_empty() {
                        self.flush_batch().await;
                    }
                }
                reading = self.rx.recv() => {
                    match reading {
                        Some(reading) => {
                            self.stats.readings_received += 1;
                            self.process_reading(reading);

                            // Force flush if batch is full
                            if self.batch.len() >= self.config.max_batch_size {
                                self.flush_batch().await;
                            }
                        }
                        None => {
                            // Channel closed, flush remaining and exit
                            info!("Storage channel closed, flushing remaining readings");
                            self.flush_batch().await;
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Process a reading (potentially with decimation)
    fn process_reading(&mut self, reading: RtacReading) {
        if let Some(ref mut sampler) = self.sampler {
            sampler.add(reading);

            if sampler.is_ready() {
                if let Some(averaged) = sampler.take_averaged() {
                    self.batch.push(StorageReading::from(averaged));
                }
            }
        } else {
            self.batch.push(StorageReading::from(reading));
        }
    }

    /// Flush the current batch to storage
    async fn flush_batch(&mut self) {
        if self.batch.is_empty() {
            return;
        }

        let batch = std::mem::take(&mut self.batch);
        let count = batch.len();

        debug!(count = count, "Flushing batch to storage");

        match self.backend.write_batch(batch).await {
            Ok(()) => {
                self.stats.readings_written += count as u64;
                self.stats.batch_writes += 1;
                debug!(count = count, "Batch written successfully");
            }
            Err(e) => {
                self.stats.failed_writes += 1;
                error!(error = %e, count = count, "Failed to write batch to storage");
            }
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> &StorageStats {
        &self.stats
    }
}

/// Create a channel for sending readings to the storage task
pub fn create_storage_channel(
    buffer_size: usize,
) -> (mpsc::Sender<RtacReading>, mpsc::Receiver<RtacReading>) {
    mpsc::channel(buffer_size)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn make_test_reading(soc: f32, power: f32, sequence: u64) -> RtacReading {
        RtacReading {
            timestamp: Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
            soc_percent: soc,
            power_kw: power,
            mode: "charging".to_string(),
            voltage_v: 480.0,
            current_a: 100.0,
            temperature_c: 25.0,
            grid_frequency_hz: 60.0,
            alarm_registers: [0u16; ALARM_REGISTER_COUNT],
            megapack_analogs: Vec::new(),
            sequence,
        }
    }

    #[test]
    fn test_data_sampler() {
        let mut sampler = DataSampler::new(3);

        sampler.add(make_test_reading(50.0, 100.0, 1));
        assert!(!sampler.is_ready());

        sampler.add(make_test_reading(51.0, 110.0, 2));
        assert!(!sampler.is_ready());

        sampler.add(make_test_reading(52.0, 120.0, 3));
        assert!(sampler.is_ready());

        let averaged = sampler.take_averaged().unwrap();
        assert!((averaged.soc_percent - 51.0).abs() < 0.01);
        assert!((averaged.power_kw - 110.0).abs() < 0.01);
        assert_eq!(averaged.sequence, 3);
    }

    #[tokio::test]
    async fn test_in_memory_storage() {
        let mut storage = InMemoryStorage::new(100);

        let readings = vec![
            StorageReading::from(make_test_reading(50.0, 100.0, 1)),
            StorageReading::from(make_test_reading(51.0, 110.0, 2)),
        ];

        storage.write_batch(readings).await.unwrap();
        assert_eq!(storage.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_storage_max_limit() {
        let mut storage = InMemoryStorage::new(3);

        for i in 0..5 {
            let readings = vec![StorageReading::from(make_test_reading(50.0 + i as f32, 100.0, i))];
            storage.write_batch(readings).await.unwrap();
        }

        // Should only have the last 3
        assert_eq!(storage.len(), 3);
    }

    #[test]
    fn test_storage_reading_from_rtac_reading() {
        let rtac_reading = make_test_reading(75.5, -50.0, 42);
        let storage_reading = StorageReading::from(rtac_reading);

        assert_eq!(storage_reading.data["soc_percent"], 75.5);
        assert_eq!(storage_reading.data["power_kw"], -50.0);
        assert_eq!(storage_reading.data["sequence"], 42);
        // `level`/`state` make the reading consumable by the SoC history endpoint.
        // make_test_reading uses mode "charging".
        assert_eq!(storage_reading.data["level"], 75.5);
        assert_eq!(storage_reading.data["state"], "charging");
    }

    #[test]
    fn storage_reading_keys_megapack_analogs_by_zone() {
        use crate::rtac::{
            alarm_definitions::AlarmZone,
            protocol::{MP_ANALOG_POINT_COUNT, MegapackAnalogs},
        };

        let mut reading = make_test_reading(75.5, -50.0, 42);
        reading.megapack_analogs = vec![
            MegapackAnalogs {
                zone: AlarmZone::Mp1a,
                state_of_energy_percent: 82.5,
                ac_voltage_v: 479.6,
                max_battery_temperature_c: 27.4,
                raw_registers: [0u16; MP_ANALOG_POINT_COUNT],
            },
            MegapackAnalogs {
                zone: AlarmZone::Mp2c,
                state_of_energy_percent: 61.0,
                ac_voltage_v: 480.1,
                max_battery_temperature_c: 25.0,
                raw_registers: [0u16; MP_ANALOG_POINT_COUNT],
            },
        ];

        let data = StorageReading::from(reading).data;
        // f32 values widen to f64 in JSON, so compare with a tolerance rather
        // than against the literal.
        let close = |value: &serde_json::Value, expected: f64| {
            let got = value.as_f64().expect("numeric");
            assert!((got - expected).abs() < 1e-4, "{got} != {expected}");
        };
        close(&data["megapacks"]["Mp1a"]["state_of_energy"], 82.5);
        close(&data["megapacks"]["Mp1a"]["ac_voltage"], 479.6);
        close(&data["megapacks"]["Mp1a"]["max_battery_temperature"], 27.4);
        close(&data["megapacks"]["Mp2c"]["state_of_energy"], 61.0);

        // A pack that did not answer must be absent, not zero: a gauge reading
        // 0% and a gauge with no reading are opposite claims.
        assert!(data["megapacks"].get("Mp1b").is_none());
    }

    #[test]
    fn storage_reading_omits_megapacks_when_none_were_read() {
        let data = StorageReading::from(make_test_reading(75.5, -50.0, 42)).data;
        assert_eq!(data["megapacks"], serde_json::json!({}));
    }

    #[test]
    fn test_soc_state_from_mode() {
        assert_eq!(soc_state_from_mode("charging"), "charging");
        assert_eq!(soc_state_from_mode("trickle_charge"), "charging");
        assert_eq!(soc_state_from_mode("discharging"), "discharging");
        assert_eq!(soc_state_from_mode("standby"), "hold");
        assert_eq!(soc_state_from_mode("fault"), "hold");
    }
}
