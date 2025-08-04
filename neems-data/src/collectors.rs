use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use serde_json::{Value as JsonValue, json};

pub mod data_sources {
    use super::*;


    /// Ping localhost several times and get statistics using ping's built-in capabilities
    pub async fn ping_localhost(source_id: i32) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        ping_target(source_id, "127.0.0.1").await
    }

    /// Ping a specific target and get statistics using ping's built-in capabilities
    pub async fn ping_target(source_id: i32, target: &str) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let attempts = 3;
        
        let output = tokio::process::Command::new("ping")
            .args(&["-c", &attempts.to_string(), "-W", "500", target])
            .output()
            .await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // Parse ping statistics from output
            let mut min_ms: Option<f64> = None;
            let mut avg_ms: Option<f64> = None;
            let mut max_ms: Option<f64> = None;
            let mut mdev_ms: Option<f64> = None;
            let mut packets_transmitted = 0;
            let mut packets_received = 0;
            
            for line in stdout.lines() {
                // Parse packet statistics: "3 packets transmitted, 3 received, 0% packet loss"
                if line.contains("packets transmitted") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        packets_transmitted = parts[0].parse().unwrap_or(0);
                        packets_received = parts[3].parse().unwrap_or(0);
                    }
                }
                
                // Parse timing statistics: "rtt min/avg/max/mdev = 0.123/0.456/0.789/0.123 ms"
                if line.contains("rtt min/avg/max/mdev") {
                    if let Some(stats_part) = line.split(" = ").nth(1) {
                        if let Some(numbers_part) = stats_part.split(" ms").nth(0) {
                            let values: Vec<&str> = numbers_part.split('/').collect();
                            if values.len() >= 4 {
                                min_ms = values[0].parse().ok();
                                avg_ms = values[1].parse().ok();
                                max_ms = values[2].parse().ok();
                                mdev_ms = values[3].parse().ok();
                            }
                        }
                    }
                }
            }

            Ok(json!({
                "source_id": source_id,
                "target": target,
                "packets_transmitted": packets_transmitted,
                "packets_received": packets_received,
                "packet_loss_percent": if packets_transmitted > 0 {
                    ((packets_transmitted - packets_received) as f64 / packets_transmitted as f64) * 100.0
                } else { 0.0 },
                "min_ms": min_ms,
                "avg_ms": avg_ms,
                "max_ms": max_ms,
                "mdev_ms": mdev_ms,
                "successful_pings": packets_received,
                "total_attempts": packets_transmitted
            }))
        } else {
            // Ping command failed, return error info
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(json!({
                "source_id": source_id,
                "target": target,
                "packets_transmitted": 0,
                "packets_received": 0,
                "packet_loss_percent": 100.0,
                "min_ms": null,
                "avg_ms": null,
                "max_ms": null,
                "mdev_ms": null,
                "successful_pings": 0,
                "total_attempts": attempts,
                "error": stderr.trim()
            }))
        }
    }




    /// Determine the charging state based on the current time.
    /// This is the public-facing collector function.
    pub async fn charging_state(source_id: i32) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        charging_state_for_battery(source_id, "default").await
    }

    /// Determine the charging state for a specific battery based on the current time.
    pub async fn charging_state_for_battery(source_id: i32, battery_id: &str) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let (state, level) = charging_state_with_level(now, battery_id);
        Ok(json!({
            "source_id": source_id,
            "battery_id": battery_id,
            "state": state,
            "level": level,
            "timestamp_utc": now.to_rfc3339()
        }))
    }


    /// Enhanced function that returns both state and battery level percentage
    pub fn charging_state_with_level(now: DateTime<Utc>, _battery_id: &str) -> (&'static str, f64) {
        let weekday = now.weekday();
        let hour = now.hour();
        let minute = now.minute();
        let total_minutes = hour * 60 + minute;

        // Discharging: M-F, 4 PM to 8 PM (16:00 - 19:59)
        if weekday.number_from_monday() >= 1
            && weekday.number_from_monday() <= 5
            && hour >= 16
            && hour < 20
        {
            // Linear discharge from 85% to 12% over 4 hours (240 minutes)
            let discharge_start = 16 * 60; // 4 PM in minutes
            let discharge_duration = 4 * 60; // 4 hours in minutes
            let progress = (total_minutes - discharge_start) as f64 / discharge_duration as f64;
            let level = 85.0 - (85.0 - 12.0) * progress.clamp(0.0, 1.0);
            return ("discharging", level);
        }

        // Charging: Sat-Thurs, 12 AM to 8 AM (00:00 - 07:59)
        // Note: This includes Saturday, Sunday, Monday, Tuesday, Wednesday, Thursday
        if weekday != Weekday::Fri && hour < 8 {
            // Linear charge from 12% to 85% over 8 hours (480 minutes)
            let charge_duration = 8 * 60; // 8 hours in minutes
            let progress = total_minutes as f64 / charge_duration as f64;
            let level = 12.0 + (85.0 - 12.0) * progress.clamp(0.0, 1.0);
            return ("charging", level);
        }

        // Hold: All other times
        // During hold after charging (non-Friday early morning to 4 PM): 85%
        // During hold after discharging (Friday 8 PM to Saturday midnight): 12%
        let level = if weekday == Weekday::Fri && hour >= 20 {
            12.0 // Hold at low level after discharge
        } else {
            85.0 // Hold at high level after charge
        };
        
        ("hold", level)
    }
}

/// Data collector that manages async polling of various data sources
pub struct DataCollector {
    pub name: String,
    pub source_id: i32,
}

impl DataCollector {
    pub fn new(name: String, source_id: i32) -> Self {
        Self {
            name,
            source_id,
        }
    }

    /// Collect data based on the collector type
    pub async fn collect(&self) -> Result<JsonValue, Box<dyn std::error::Error + Send + Sync>> {
        match self.name.as_str() {
            "ping_localhost" => data_sources::ping_localhost(self.source_id).await,
            "charging_state" => data_sources::charging_state(self.source_id).await,
            name if name.starts_with("charging_state_") => {
                // Extract battery_id from the name for backward compatibility
                let battery_id = name.strip_prefix("charging_state_").unwrap_or("default");
                data_sources::charging_state_for_battery(self.source_id, battery_id).await
            }
            name if name.starts_with("ping_") => {
                // Extract target from the name for backward compatibility
                let target = name.strip_prefix("ping_").unwrap_or("127.0.0.1");
                data_sources::ping_target(self.source_id, target).await
            }
            _ => Err(format!("Unknown collector type: {}", self.name).into()),
        }
    }
}
