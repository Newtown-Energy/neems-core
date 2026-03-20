//! Modbus TCP client wrapper for RTAC communication
//!
//! This module provides a connection wrapper that handles:
//! - Connection establishment and management
//! - Automatic reconnection on disconnect
//! - Read and write operations with proper error handling

use std::{io, net::SocketAddr, time::Duration};

use tokio::{net::TcpStream, time::timeout};
use tokio_modbus::{client::tcp, prelude::*};
use tracing::{debug, error, info, trace, warn};

use super::{
    protocol::{ParsedStatus, RegisterMap, build_command_registers},
    state::{ConnectionStatus, PendingCommand},
};

/// Configuration for the Modbus client
#[derive(Debug, Clone)]
pub struct ModbusClientConfig {
    /// RTAC IP address and port
    pub address: SocketAddr,
    /// Modbus slave ID (unit identifier)
    pub slave_id: u8,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Read/write operation timeout
    pub operation_timeout: Duration,
    /// Maximum reconnection attempts before giving up
    pub max_reconnect_attempts: u32,
    /// Delay between reconnection attempts
    pub reconnect_delay: Duration,
}

impl Default for ModbusClientConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:502".parse().unwrap(),
            slave_id: 1,
            connect_timeout: Duration::from_secs(5),
            operation_timeout: Duration::from_millis(500),
            max_reconnect_attempts: 10,
            reconnect_delay: Duration::from_secs(1),
        }
    }
}

/// Error types for Modbus operations
#[derive(Debug)]
pub enum ModbusError {
    /// Connection failed
    ConnectionFailed(io::Error),
    /// Connection timeout
    ConnectionTimeout,
    /// Operation timeout
    OperationTimeout,
    /// Read operation failed
    ReadFailed(String),
    /// Write operation failed
    WriteFailed(String),
    /// Modbus exception response
    Exception(String),
    /// Invalid response (wrong number of registers)
    InvalidResponse { expected: usize, got: usize },
    /// Client not connected
    NotConnected,
    /// Reconnection failed after max attempts
    ReconnectFailed,
}

impl std::fmt::Display for ModbusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(e) => write!(f, "Connection failed: {}", e),
            Self::ConnectionTimeout => write!(f, "Connection timeout"),
            Self::OperationTimeout => write!(f, "Operation timeout"),
            Self::ReadFailed(e) => write!(f, "Read failed: {}", e),
            Self::WriteFailed(e) => write!(f, "Write failed: {}", e),
            Self::Exception(e) => write!(f, "Modbus exception: {}", e),
            Self::InvalidResponse { expected, got } => {
                write!(f, "Invalid response: expected {} registers, got {}", expected, got)
            }
            Self::NotConnected => write!(f, "Client not connected"),
            Self::ReconnectFailed => write!(f, "Reconnection failed after max attempts"),
        }
    }
}

impl std::error::Error for ModbusError {}

/// Modbus TCP client for RTAC communication
pub struct ModbusClient {
    config: ModbusClientConfig,
    context: Option<client::Context>,
    connection_status: ConnectionStatus,
    reconnect_attempts: u32,
}

impl ModbusClient {
    /// Create a new Modbus client with the given configuration
    pub fn new(config: ModbusClientConfig) -> Self {
        Self {
            config,
            context: None,
            connection_status: ConnectionStatus::Disconnected,
            reconnect_attempts: 0,
        }
    }

    /// Get the current connection status
    pub fn connection_status(&self) -> ConnectionStatus {
        self.connection_status
    }

    /// Check if the client is connected
    pub fn is_connected(&self) -> bool {
        self.connection_status == ConnectionStatus::Connected && self.context.is_some()
    }

    /// Connect to the RTAC
    pub async fn connect(&mut self) -> Result<(), ModbusError> {
        info!(address = %self.config.address, "Connecting to RTAC");

        let connect_result = timeout(self.config.connect_timeout, async {
            let stream = TcpStream::connect(self.config.address).await?;
            let slave = Slave(self.config.slave_id);
            Ok::<_, io::Error>(tcp::attach_slave(stream, slave))
        })
        .await;

        match connect_result {
            Ok(Ok(ctx)) => {
                self.context = Some(ctx);
                self.connection_status = ConnectionStatus::Connected;
                self.reconnect_attempts = 0;
                info!(address = %self.config.address, "Connected to RTAC");
                Ok(())
            }
            Ok(Err(e)) => {
                self.connection_status = ConnectionStatus::Disconnected;
                error!(address = %self.config.address, error = %e, "Failed to connect to RTAC");
                Err(ModbusError::ConnectionFailed(e))
            }
            Err(_) => {
                self.connection_status = ConnectionStatus::Disconnected;
                error!(address = %self.config.address, "Connection timeout");
                Err(ModbusError::ConnectionTimeout)
            }
        }
    }

    /// Disconnect from the RTAC
    pub async fn disconnect(&mut self) {
        if let Some(ctx) = self.context.take() {
            debug!("Disconnecting from RTAC");
            drop(ctx);
        }
        self.connection_status = ConnectionStatus::Disconnected;
    }

    /// Attempt to reconnect to the RTAC
    pub async fn reconnect(&mut self) -> Result<(), ModbusError> {
        self.connection_status = ConnectionStatus::Reconnecting;

        while self.reconnect_attempts < self.config.max_reconnect_attempts {
            self.reconnect_attempts += 1;
            warn!(
                attempt = self.reconnect_attempts,
                max_attempts = self.config.max_reconnect_attempts,
                "Attempting to reconnect to RTAC"
            );

            // Drop existing context if any
            self.context.take();

            match self.connect().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(error = %e, "Reconnection attempt failed");
                    if self.reconnect_attempts < self.config.max_reconnect_attempts {
                        tokio::time::sleep(self.config.reconnect_delay).await;
                    }
                }
            }
        }

        self.connection_status = ConnectionStatus::Failed;
        error!("Reconnection failed after {} attempts", self.config.max_reconnect_attempts);
        Err(ModbusError::ReconnectFailed)
    }

    /// Read status registers from the RTAC
    pub async fn read_status(&mut self) -> Result<ParsedStatus, ModbusError> {
        let ctx = self.context.as_mut().ok_or(ModbusError::NotConnected)?;

        trace!(
            start_address = RegisterMap::STATUS_MODE,
            count = RegisterMap::STATUS_READ_COUNT,
            "Reading status registers"
        );

        let read_result = timeout(
            self.config.operation_timeout,
            ctx.read_holding_registers(RegisterMap::STATUS_MODE, RegisterMap::STATUS_READ_COUNT),
        )
        .await;

        match read_result {
            Ok(Ok(Ok(registers))) => {
                trace!(registers = ?registers, "Received status registers");

                if registers.len() < RegisterMap::STATUS_READ_COUNT as usize {
                    warn!(
                        expected = RegisterMap::STATUS_READ_COUNT,
                        got = registers.len(),
                        "Received fewer registers than expected"
                    );
                    return Err(ModbusError::InvalidResponse {
                        expected: RegisterMap::STATUS_READ_COUNT as usize,
                        got: registers.len(),
                    });
                }

                ParsedStatus::from_registers(&registers).ok_or(ModbusError::InvalidResponse {
                    expected: RegisterMap::STATUS_READ_COUNT as usize,
                    got: registers.len(),
                })
            }
            Ok(Ok(Err(exception))) => {
                warn!(exception = ?exception, "Modbus exception on read");
                Err(ModbusError::Exception(format!("{:?}", exception)))
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to read status registers");
                // Mark as disconnected so reconnect will be attempted
                self.connection_status = ConnectionStatus::Disconnected;
                self.context.take();
                Err(ModbusError::ReadFailed(e.to_string()))
            }
            Err(_) => {
                warn!("Status read operation timeout");
                // Mark as disconnected so reconnect will be attempted on timeout
                self.connection_status = ConnectionStatus::Disconnected;
                self.context.take();
                Err(ModbusError::OperationTimeout)
            }
        }
    }

    /// Write a command to the RTAC
    pub async fn write_command(&mut self, command: &PendingCommand) -> Result<(), ModbusError> {
        let ctx = self.context.as_mut().ok_or(ModbusError::NotConnected)?;

        let registers = build_command_registers(
            command.command_type,
            command.target_soc_percent,
            command.duration_seconds,
            command.ramp_duration_seconds,
        );

        debug!(
            command_type = %command.command_type,
            target_soc = ?command.target_soc_percent,
            duration = ?command.duration_seconds,
            ramp_duration = command.ramp_duration_seconds,
            "Writing command to RTAC"
        );
        trace!(start_address = RegisterMap::CMD_START_ADDRESS, registers = ?registers, "Command registers");

        let write_result = timeout(
            self.config.operation_timeout,
            ctx.write_multiple_registers(RegisterMap::CMD_START_ADDRESS, &registers),
        )
        .await;

        match write_result {
            Ok(Ok(Ok(()))) => {
                info!(command_type = %command.command_type, "Command written successfully");
                Ok(())
            }
            Ok(Ok(Err(exception))) => {
                warn!(exception = ?exception, "Modbus exception on write");
                Err(ModbusError::Exception(format!("{:?}", exception)))
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to write command");
                // Mark as disconnected so reconnect will be attempted
                self.connection_status = ConnectionStatus::Disconnected;
                self.context.take();
                Err(ModbusError::WriteFailed(e.to_string()))
            }
            Err(_) => {
                warn!("Command write operation timeout");
                // Mark as disconnected so reconnect will be attempted on timeout
                self.connection_status = ConnectionStatus::Disconnected;
                self.context.take();
                Err(ModbusError::OperationTimeout)
            }
        }
    }

    /// Read status and handle connection errors with automatic reconnect
    pub async fn read_status_with_reconnect(&mut self) -> Result<ParsedStatus, ModbusError> {
        if !self.is_connected() {
            self.reconnect().await?;
        }

        match self.read_status().await {
            Ok(status) => Ok(status),
            Err(ModbusError::ReadFailed(_) | ModbusError::NotConnected) => {
                // Try to reconnect and read again
                self.reconnect().await?;
                self.read_status().await
            }
            Err(e) => Err(e),
        }
    }

    /// Write command and handle connection errors with automatic reconnect
    pub async fn write_command_with_reconnect(
        &mut self,
        command: &PendingCommand,
    ) -> Result<(), ModbusError> {
        if !self.is_connected() {
            self.reconnect().await?;
        }

        match self.write_command(command).await {
            Ok(()) => Ok(()),
            Err(ModbusError::WriteFailed(_) | ModbusError::NotConnected) => {
                // Try to reconnect and write again
                self.reconnect().await?;
                self.write_command(command).await
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_default() {
        let config = ModbusClientConfig::default();
        assert_eq!(config.slave_id, 1);
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.operation_timeout, Duration::from_millis(500));
    }

    #[test]
    fn test_client_initial_state() {
        let config = ModbusClientConfig::default();
        let client = ModbusClient::new(config);
        assert_eq!(client.connection_status(), ConnectionStatus::Disconnected);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_modbus_error_display() {
        let err = ModbusError::ConnectionTimeout;
        assert_eq!(format!("{}", err), "Connection timeout");

        let err = ModbusError::InvalidResponse { expected: 9, got: 5 };
        assert_eq!(format!("{}", err), "Invalid response: expected 9 registers, got 5");
    }
}
