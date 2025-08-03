use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use rand::Rng;
use serde_json::{json, Value as JsonValue};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;
use sha1::Digest;

pub mod data_sources {
    use super::*;

    /// Get current UTC timestamp
    pub async fn current_time() -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        Ok(json!({
            "timestamp_utc": now.to_rfc3339(),
            "unix_timestamp": now.timestamp(),
            "milliseconds": now.timestamp_millis()
        }))
    }

    /// Ping localhost 3 times and get average response time
    pub async fn ping_localhost() -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let mut times = Vec::new();
        
        for _ in 0..3 {
            let start = Instant::now();
            
            // Try to connect to localhost on port 22 (SSH) as a simple connectivity test
            let connect_result = timeout(Duration::from_millis(500), TcpStream::connect("127.0.0.1:22")).await;
            
            match connect_result {
                Ok(Ok(_)) => {
                    let duration = start.elapsed();
                    times.push(duration.as_micros() as f64 / 1000.0); // Convert to milliseconds
                },
                Ok(Err(_)) | Err(_) => {
                    // If SSH port is not available, try a simple ping using system command
                    let output = tokio::process::Command::new("ping")
                        .args(&["-c", "1", "-W", "500", "127.0.0.1"])
                        .output()
                        .await;
                    
                    if let Ok(output) = output {
                        if output.status.success() {
                            let duration = start.elapsed();
                            times.push(duration.as_millis() as f64);
                        }
                    }
                }
            }
        }

        let average = if times.is_empty() {
            None
        } else {
            Some(times.iter().sum::<f64>() / times.len() as f64)
        };

        Ok(json!({
            "ping_times_ms": times,
            "average_ms": average,
            "successful_pings": times.len(),
            "total_attempts": 3
        }))
    }

    /// Generate some random digits
    pub async fn random_digits() -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let mut rng = rand::thread_rng();
        
        let random_int: u32 = rng.random_range(0..10000);
        let random_float: f64 = rng.random();
        let random_bytes: Vec<u8> = (0..8).map(|_| rng.random()).collect();
        
        Ok(json!({
            "random_integer": random_int,
            "random_float": random_float,
            "random_bytes": random_bytes,
            "timestamp": Utc::now().to_rfc3339()
        }))
    }

    /// Get modification time of the database file
    pub async fn database_modtime(db_path: &str) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let path = Path::new(db_path);
        
        if path.exists() {
            let metadata = fs::metadata(path)?;
            let modified = metadata.modified()?;
            let system_time_modified = modified.duration_since(std::time::UNIX_EPOCH)?;
            
            Ok(json!({
                "file_exists": true,
                "modified_timestamp": system_time_modified.as_secs(),
                "modified_timestamp_ms": system_time_modified.as_millis(),
                "file_size_bytes": metadata.len(),
                "file_path": db_path
            }))
        } else {
            Ok(json!({
                "file_exists": false,
                "file_path": db_path,
                "error": "File not found"
            }))
        }
    }

    /// Get SHA1 hash of the database file
    pub async fn database_sha1(db_path: &str) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let path = Path::new(db_path);
        
        if path.exists() {
            let contents = tokio::fs::read(path).await?;
            let mut hasher = sha1::Sha1::new();
            hasher.update(&contents);
            let hash = hasher.finalize();
            let hash_hex = format!("{:x}", hash);
            
            Ok(json!({
                "file_exists": true,
                "sha1_hash": hash_hex,
                "file_size_bytes": contents.len(),
                "file_path": db_path
            }))
        } else {
            Ok(json!({
                "file_exists": false,
                "file_path": db_path,
                "error": "File not found"
            }))
        }
    }

    /// Determine the charging state based on the current time.
    /// This is the public-facing collector function.
    pub async fn charging_state() -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let state = charging_state_logic(now);
        Ok(json!({
            "state": state,
            "timestamp_utc": now.to_rfc3339()
        }))
    }

    /// This function contains the core logic for determining the charging state.
    /// It's separate from the async collector function to make it easily testable.
    pub fn charging_state_logic(now: DateTime<Utc>) -> &'static str {
        let weekday = now.weekday();
        let hour = now.hour();

        // Discharging: M-F, 4 PM to 8 PM (16:00 - 19:59)
        if weekday.number_from_monday() >= 1
            && weekday.number_from_monday() <= 5
            && hour >= 16
            && hour < 20
        {
            return "discharging";
        }

        // Charging: Sat-Thurs, 12 AM to 8 AM (00:00 - 07:59)
        // Note: This includes Saturday, Sunday, Monday, Tuesday, Wednesday, Thursday
        if weekday != Weekday::Fri && hour < 8 {
            return "charging";
        }

        // Hold: All other times
        "hold"
    }
}

/// Data collector that manages async polling of various data sources
pub struct DataCollector {
    pub name: String,
    pub source_id: i32,
    db_path: String,
}

impl DataCollector {
    pub fn new(name: String, source_id: i32, db_path: String) -> Self {
        Self {
            name,
            source_id,
            db_path,
        }
    }

    /// Collect data based on the collector type
    pub async fn collect(&self) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        match self.name.as_str() {
            "current_time" => data_sources::current_time().await,
            "ping_localhost" => data_sources::ping_localhost().await,
            "random_digits" => data_sources::random_digits().await,
            "database_modtime" => data_sources::database_modtime(&self.db_path).await,
            "database_sha1" => data_sources::database_sha1(&self.db_path).await,
            "charging_state" => data_sources::charging_state().await,
            _ => Err(format!("Unknown collector type: {}", self.name).into()),
        }
    }
}