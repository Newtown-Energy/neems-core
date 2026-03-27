//! RTAC Modbus Communication Module
//!
//! This module provides bidirectional Modbus TCP communication with the RTAC
//! for:
//! - Executing site schedules by writing commands at 2Hz (500ms intervals)
//! - Collecting operational data and alarm statuses at 10Hz (100ms intervals)
//! - Enabling reactive control where write operations adjust based on read
//!   values

pub mod alarm_definitions;
pub mod alarms;
pub mod control;
pub mod modbus_client;
pub mod protocol;
pub mod state;
pub mod storage;
pub mod worker;

pub use alarm_definitions::{AlarmDefinition, AlarmZone, ALARM_REGISTER_COUNT};
pub use alarms::{Alarm, AlarmHandlerTask, AlarmSeverity};
pub use control::ControlLogicTask;
pub use modbus_client::ModbusClient;
pub use protocol::{CommandType, OperatingMode, RegisterMap};
pub use state::{AlarmFlags, ConnectionStatus, PendingCommand, RtacReading, RtacState};
pub use storage::{DataSampler, StorageWriterTask};
pub use worker::{ModbusWorker, RtacConfig};
