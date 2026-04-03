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

use super::alarm_definitions::{AlarmDefinition, AlarmZone};

/// Alarm severity levels
///
/// Mapped from the Newtown alarm level numbering:
/// - Level 1 → Emergency (fire alarms, emergency shutdown)
/// - Level 2 → Critical (equipment faults, E-stop, activate COF)
/// - Level 3 → Warning (communication issues, relay alarms)
/// - Level 4 → Info (operator troubleshooting, monitoring)
/// - Level 5 → Info (informational, unclassified)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AlarmSeverity {
    /// Informational - no action required (levels 4–5)
    Info,
    /// Warning - should be monitored (level 3)
    Warning,
    /// Critical - requires immediate attention (level 2)
    Critical,
    /// Emergency - system safety at risk (level 1)
    Emergency,
}

impl AlarmSeverity {
    /// Convert a Newtown alarm level (1–5) to a severity
    pub fn from_level(level: u8) -> Self {
        match level {
            1 => Self::Emergency,
            2 => Self::Critical,
            3 => Self::Warning,
            _ => Self::Info, // levels 4, 5, and any unrecognized
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
    /// Unique alarm number from the Newtown alarm matrix
    pub alarm_num: u16,
    /// Zone this alarm belongs to
    pub zone: AlarmZone,
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
    /// Create a new active alarm from an alarm definition
    pub fn new(def: &AlarmDefinition) -> Self {
        Self {
            name: def.name.to_string(),
            alarm_num: def.alarm_num,
            zone: def.zone,
            state: AlarmState::Active,
            severity: AlarmSeverity::from_level(def.level),
            timestamp: Utc::now(),
            message: None,
        }
    }

    /// Create an alarm cleared event from an alarm definition
    pub fn cleared(def: &AlarmDefinition) -> Self {
        Self {
            name: def.name.to_string(),
            alarm_num: def.alarm_num,
            zone: def.zone,
            state: AlarmState::Cleared,
            severity: AlarmSeverity::from_level(def.level),
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
                    alarm_num = alarm.alarm_num,
                    zone = %alarm.zone,
                    severity = "emergency",
                    message = ?alarm.message,
                    "EMERGENCY ALARM ACTIVATED"
                );
            }
            (AlarmState::Active, AlarmSeverity::Critical) => {
                error!(
                    alarm = alarm.name,
                    alarm_num = alarm.alarm_num,
                    zone = %alarm.zone,
                    severity = "critical",
                    message = ?alarm.message,
                    "Critical alarm activated"
                );
            }
            (AlarmState::Active, AlarmSeverity::Warning) => {
                warn!(
                    alarm = alarm.name,
                    alarm_num = alarm.alarm_num,
                    zone = %alarm.zone,
                    severity = "warning",
                    message = ?alarm.message,
                    "Warning alarm activated"
                );
            }
            (AlarmState::Active, AlarmSeverity::Info) => {
                info!(
                    alarm = alarm.name,
                    alarm_num = alarm.alarm_num,
                    zone = %alarm.zone,
                    severity = "info",
                    message = ?alarm.message,
                    "Info alarm activated"
                );
            }
            (AlarmState::Cleared, _) => {
                info!(
                    alarm = alarm.name,
                    alarm_num = alarm.alarm_num,
                    zone = %alarm.zone,
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
    /// Currently active alarms by alarm number
    active_alarms: HashMap<u16, Alarm>,
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
                self.active_alarms.insert(alarm.alarm_num, alarm.clone());
            }
            AlarmState::Cleared => {
                self.active_alarms.remove(&alarm.alarm_num);
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
            alarm_num = alarm.alarm_num,
            zone = %alarm.zone,
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
                    alarm_num = alarm.alarm_num,
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
    use crate::rtac::alarm_definitions::{ESTOP_ALARM_NUM, FIRE_ALARM_NUM, find_by_alarm_num};

    #[test]
    fn test_alarm_severity_from_level() {
        assert_eq!(AlarmSeverity::from_level(1), AlarmSeverity::Emergency);
        assert_eq!(AlarmSeverity::from_level(2), AlarmSeverity::Critical);
        assert_eq!(AlarmSeverity::from_level(3), AlarmSeverity::Warning);
        assert_eq!(AlarmSeverity::from_level(4), AlarmSeverity::Info);
        assert_eq!(AlarmSeverity::from_level(5), AlarmSeverity::Info);
    }

    #[test]
    fn test_alarm_creation_from_definition() {
        let def = find_by_alarm_num(FIRE_ALARM_NUM).unwrap();
        let alarm = Alarm::new(def);
        assert_eq!(alarm.name, "fire_alarm");
        assert_eq!(alarm.alarm_num, 401);
        assert_eq!(alarm.zone, AlarmZone::Facp);
        assert_eq!(alarm.state, AlarmState::Active);
        assert_eq!(alarm.severity, AlarmSeverity::Emergency);
        assert!(alarm.is_active());
        assert!(alarm.is_critical_or_higher());

        let cleared = Alarm::cleared(def);
        assert_eq!(cleared.state, AlarmState::Cleared);
        assert!(!cleared.is_active());
    }

    #[test]
    fn test_alarm_estop() {
        let def = find_by_alarm_num(ESTOP_ALARM_NUM).unwrap();
        let alarm = Alarm::new(def);
        assert_eq!(alarm.name, "estop");
        assert_eq!(alarm.severity, AlarmSeverity::Critical);
        assert!(alarm.is_critical_or_higher());
    }

    #[test]
    fn test_alarm_history() {
        let fire_def = find_by_alarm_num(FIRE_ALARM_NUM).unwrap();
        let site_def = find_by_alarm_num(1).unwrap(); // loss_fiber, level 3

        let mut history = AlarmHistory::new(10);

        // Add active alarm
        history.record(Alarm::new(fire_def));
        assert_eq!(history.active_alarms().len(), 1);
        assert!(history.has_critical_alarms());

        // Add another alarm
        history.record(Alarm::new(site_def));
        assert_eq!(history.active_alarms().len(), 2);

        // Clear first alarm
        history.record(Alarm::cleared(fire_def));
        assert_eq!(history.active_alarms().len(), 1);
        assert!(!history.has_critical_alarms()); // loss_fiber is only Warning

        // Check event count
        assert_eq!(history.recent_events(10).len(), 3);
    }

    #[test]
    fn test_alarm_history_max_events() {
        let def = find_by_alarm_num(1).unwrap();
        let mut history = AlarmHistory::new(3);

        for i in 0..5 {
            history.record(Alarm::new(def).with_message(&format!("event {}", i)));
        }

        assert_eq!(history.recent_events(10).len(), 3);
    }

    #[test]
    fn test_logging_handler() {
        let mut handler = LoggingAlarmHandler;
        let fire_def = find_by_alarm_num(FIRE_ALARM_NUM).unwrap();

        // This should not panic
        handler.handle(&Alarm::new(fire_def)).unwrap();
        handler.handle(&Alarm::cleared(fire_def)).unwrap();
    }
}
