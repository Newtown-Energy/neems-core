//! Alarm Handler Task
//!
//! This module implements the alarm handler that:
//! - Receives alarm notifications via unbounded mpsc channel (never blocks the
//!   worker)
//! - Processes alarms immediately without waiting for batch cycles
//! - Supports different alarm severities and handlers

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Alarm severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AlarmSeverity {
    /// Informational - no action required
    Info,
    /// Warning - should be monitored
    Warning,
    /// Critical - requires immediate attention
    Critical,
    /// Emergency - system safety at risk
    Emergency,
}

impl AlarmSeverity {
    /// Get the severity for a named alarm
    pub fn for_alarm(name: &str) -> Self {
        match name {
            "emergency_stop" => Self::Emergency,
            "over_temperature" | "over_voltage" | "over_current" => Self::Critical,
            "bms_fault" | "inverter_fault" => Self::Critical,
            "under_temperature" | "under_voltage" => Self::Warning,
            "communication_fault" | "grid_fault" => Self::Warning,
            "isolation_fault" | "cooling_fault" => Self::Warning,
            _ => Self::Info,
        }
    }
}

/// Alarm state (active or cleared)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmState {
    /// Alarm is currently active
    Active,
    /// Alarm has been cleared
    Cleared,
}

/// An alarm event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alarm {
    /// Alarm name/identifier
    pub name: String,
    /// Current state
    pub state: AlarmState,
    /// Severity level
    pub severity: AlarmSeverity,
    /// When the alarm was triggered
    pub timestamp: DateTime<Utc>,
    /// Additional context/message
    pub message: Option<String>,
}

impl Alarm {
    /// Create a new active alarm
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: AlarmState::Active,
            severity: AlarmSeverity::for_alarm(name),
            timestamp: Utc::now(),
            message: None,
        }
    }

    /// Create an alarm cleared event
    pub fn cleared(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: AlarmState::Cleared,
            severity: AlarmSeverity::for_alarm(name),
            timestamp: Utc::now(),
            message: None,
        }
    }

    /// Create an alarm with a custom message
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }

    /// Check if this alarm is active
    pub fn is_active(&self) -> bool {
        self.state == AlarmState::Active
    }

    /// Check if this is a critical or higher severity
    pub fn is_critical_or_higher(&self) -> bool {
        self.severity >= AlarmSeverity::Critical
    }
}

/// Trait for alarm handlers
///
/// Implement this trait to handle alarms in different ways (logging,
/// notifications, etc.)
pub trait AlarmHandler: Send + Sync {
    /// Handle an alarm event
    fn handle(&mut self, alarm: &Alarm) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Logging alarm handler - logs alarms using tracing
pub struct LoggingAlarmHandler;

impl AlarmHandler for LoggingAlarmHandler {
    fn handle(&mut self, alarm: &Alarm) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match (alarm.state, alarm.severity) {
            (AlarmState::Active, AlarmSeverity::Emergency) => {
                error!(
                    alarm = alarm.name,
                    severity = "emergency",
                    message = ?alarm.message,
                    "EMERGENCY ALARM ACTIVATED"
                );
            }
            (AlarmState::Active, AlarmSeverity::Critical) => {
                error!(
                    alarm = alarm.name,
                    severity = "critical",
                    message = ?alarm.message,
                    "Critical alarm activated"
                );
            }
            (AlarmState::Active, AlarmSeverity::Warning) => {
                warn!(
                    alarm = alarm.name,
                    severity = "warning",
                    message = ?alarm.message,
                    "Warning alarm activated"
                );
            }
            (AlarmState::Active, AlarmSeverity::Info) => {
                info!(
                    alarm = alarm.name,
                    severity = "info",
                    message = ?alarm.message,
                    "Info alarm activated"
                );
            }
            (AlarmState::Cleared, _) => {
                info!(
                    alarm = alarm.name,
                    severity = ?alarm.severity,
                    "Alarm cleared"
                );
            }
        }
        Ok(())
    }
}

/// In-memory alarm history for tracking
pub struct AlarmHistory {
    /// History of alarm events
    events: Vec<Alarm>,
    /// Maximum number of events to keep
    max_events: usize,
    /// Currently active alarms by name
    active_alarms: HashMap<String, Alarm>,
}

impl AlarmHistory {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Vec::new(),
            max_events,
            active_alarms: HashMap::new(),
        }
    }

    /// Record an alarm event
    pub fn record(&mut self, alarm: Alarm) {
        match alarm.state {
            AlarmState::Active => {
                self.active_alarms.insert(alarm.name.clone(), alarm.clone());
            }
            AlarmState::Cleared => {
                self.active_alarms.remove(&alarm.name);
            }
        }

        if self.events.len() >= self.max_events {
            self.events.remove(0);
        }
        self.events.push(alarm);
    }

    /// Get all currently active alarms
    pub fn active_alarms(&self) -> Vec<&Alarm> {
        self.active_alarms.values().collect()
    }

    /// Get recent alarm events
    pub fn recent_events(&self, count: usize) -> Vec<&Alarm> {
        self.events.iter().rev().take(count).collect()
    }

    /// Check if any critical or higher alarms are active
    pub fn has_critical_alarms(&self) -> bool {
        self.active_alarms.values().any(|a| a.is_critical_or_higher())
    }

    /// Get count of active alarms by severity
    pub fn count_by_severity(&self) -> HashMap<AlarmSeverity, usize> {
        let mut counts = HashMap::new();
        for alarm in self.active_alarms.values() {
            *counts.entry(alarm.severity).or_insert(0) += 1;
        }
        counts
    }
}

impl AlarmHandler for AlarmHistory {
    fn handle(&mut self, alarm: &Alarm) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.record(alarm.clone());
        Ok(())
    }
}

/// Configuration for the alarm handler task
#[derive(Debug, Clone)]
pub struct AlarmConfig {
    /// Maximum number of events to keep in history
    pub max_history_events: usize,
}

impl Default for AlarmConfig {
    fn default() -> Self {
        Self { max_history_events: 1000 }
    }
}

/// The alarm handler task
pub struct AlarmHandlerTask {
    rx: mpsc::UnboundedReceiver<Alarm>,
    handlers: Vec<Box<dyn AlarmHandler>>,
    history: AlarmHistory,
}

impl AlarmHandlerTask {
    /// Create a new alarm handler task
    pub fn new(config: AlarmConfig, rx: mpsc::UnboundedReceiver<Alarm>) -> Self {
        let history = AlarmHistory::new(config.max_history_events);

        Self {
            rx,
            handlers: vec![Box::new(LoggingAlarmHandler)],
            history,
        }
    }

    /// Add an alarm handler
    pub fn add_handler(&mut self, handler: Box<dyn AlarmHandler>) {
        self.handlers.push(handler);
    }

    /// Run the alarm handler loop
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting alarm handler task");

        while let Some(alarm) = self.rx.recv().await {
            self.process_alarm(alarm);
        }

        info!("Alarm handler task stopped");
        Ok(())
    }

    /// Process a single alarm
    fn process_alarm(&mut self, alarm: Alarm) {
        debug!(
            alarm = alarm.name,
            state = ?alarm.state,
            severity = ?alarm.severity,
            "Processing alarm"
        );

        // Record in history
        self.history.record(alarm.clone());

        // Call all handlers
        for handler in &mut self.handlers {
            if let Err(e) = handler.handle(&alarm) {
                error!(
                    error = %e,
                    alarm = alarm.name,
                    "Alarm handler failed"
                );
            }
        }
    }

    /// Get the alarm history
    pub fn history(&self) -> &AlarmHistory {
        &self.history
    }
}

/// Create a channel for sending alarms to the handler task
pub fn create_alarm_channel() -> (mpsc::UnboundedSender<Alarm>, mpsc::UnboundedReceiver<Alarm>) {
    mpsc::unbounded_channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alarm_severity() {
        assert_eq!(AlarmSeverity::for_alarm("emergency_stop"), AlarmSeverity::Emergency);
        assert_eq!(AlarmSeverity::for_alarm("over_temperature"), AlarmSeverity::Critical);
        assert_eq!(AlarmSeverity::for_alarm("under_voltage"), AlarmSeverity::Warning);
        assert_eq!(AlarmSeverity::for_alarm("unknown_alarm"), AlarmSeverity::Info);
    }

    #[test]
    fn test_alarm_creation() {
        let alarm = Alarm::new("over_temperature");
        assert_eq!(alarm.name, "over_temperature");
        assert_eq!(alarm.state, AlarmState::Active);
        assert_eq!(alarm.severity, AlarmSeverity::Critical);
        assert!(alarm.is_active());
        assert!(alarm.is_critical_or_higher());

        let cleared = Alarm::cleared("over_temperature");
        assert_eq!(cleared.state, AlarmState::Cleared);
        assert!(!cleared.is_active());
    }

    #[test]
    fn test_alarm_history() {
        let mut history = AlarmHistory::new(10);

        // Add active alarm
        history.record(Alarm::new("over_temperature"));
        assert_eq!(history.active_alarms().len(), 1);
        assert!(history.has_critical_alarms());

        // Add another alarm
        history.record(Alarm::new("under_voltage"));
        assert_eq!(history.active_alarms().len(), 2);

        // Clear first alarm
        history.record(Alarm::cleared("over_temperature"));
        assert_eq!(history.active_alarms().len(), 1);
        assert!(!history.has_critical_alarms()); // under_voltage is only Warning

        // Check event count
        assert_eq!(history.recent_events(10).len(), 3);
    }

    #[test]
    fn test_alarm_history_max_events() {
        let mut history = AlarmHistory::new(3);

        for i in 0..5 {
            history.record(Alarm::new("test").with_message(&format!("event {}", i)));
        }

        assert_eq!(history.recent_events(10).len(), 3);
    }

    #[test]
    fn test_logging_handler() {
        let mut handler = LoggingAlarmHandler;

        // This should not panic
        handler.handle(&Alarm::new("over_temperature")).unwrap();
        handler.handle(&Alarm::cleared("over_temperature")).unwrap();
    }
}
