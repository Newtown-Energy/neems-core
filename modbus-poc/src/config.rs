use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Root configuration for a site's Modbus connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    /// Site identifier (matches NEEMS site)
    pub site_id: String,

    /// Human-readable site name
    pub name: String,

    /// RTAC connections for this site
    pub connections: Vec<RtacConnection>,
}

/// Configuration for a single RTAC connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtacConnection {
    /// Unique identifier for this connection
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Hostname or IP address
    pub host: String,

    /// TCP port (default: 502)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Connection timeout in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Tag mappings for this connection
    pub tags: Vec<TagMapping>,
}

fn default_port() -> u16 {
    502
}

fn default_timeout_ms() -> u64 {
    5000
}

/// Mapping of a logical tag to a Modbus register
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagMapping {
    /// Unique tag name (e.g., "battery_soc", "inverter_power")
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Equipment ID this tag belongs to (matches NEEMS equipment)
    pub equipment_id: Option<String>,

    /// Modbus unit ID (slave address), 1-247
    #[serde(default = "default_unit_id")]
    pub unit_id: u8,

    /// Register address (0-65535)
    pub address: u16,

    /// Type of register
    #[serde(default)]
    pub register_type: RegisterType,

    /// Data type for interpreting the register value(s)
    #[serde(default)]
    pub data_type: DataType,

    /// Byte order for multi-byte values
    #[serde(default)]
    pub byte_order: ByteOrder,

    /// Word order for multi-register values (32-bit and larger)
    #[serde(default)]
    pub word_order: WordOrder,

    /// Scale factor: engineering_value = raw_value * scale + offset
    #[serde(default = "default_scale")]
    pub scale: f64,

    /// Offset: engineering_value = raw_value * scale + offset
    #[serde(default)]
    pub offset: f64,

    /// Engineering units (e.g., "V", "A", "kW", "%")
    pub units: Option<String>,

    /// Whether this tag can be written to
    #[serde(default)]
    pub writable: bool,

    /// Minimum allowed value (for writes)
    pub min_value: Option<f64>,

    /// Maximum allowed value (for writes)
    pub max_value: Option<f64>,
}

fn default_unit_id() -> u8 {
    1
}

fn default_scale() -> f64 {
    1.0
}

/// Modbus register types (address spaces)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RegisterType {
    /// Coils (0xxxx) - single bit, read/write
    Coil,
    /// Discrete Inputs (1xxxx) - single bit, read-only
    DiscreteInput,
    /// Input Registers (3xxxx) - 16-bit, read-only
    InputRegister,
    /// Holding Registers (4xxxx) - 16-bit, read/write
    #[default]
    HoldingRegister,
}

/// Data types for interpreting register values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    /// Single bit (for coils/discrete inputs)
    Bool,
    /// Unsigned 16-bit integer (1 register)
    #[default]
    Uint16,
    /// Signed 16-bit integer (1 register)
    Int16,
    /// Unsigned 32-bit integer (2 registers)
    Uint32,
    /// Signed 32-bit integer (2 registers)
    Int32,
    /// 32-bit IEEE 754 float (2 registers)
    Float32,
    /// Unsigned 64-bit integer (4 registers)
    Uint64,
    /// Signed 64-bit integer (4 registers)
    Int64,
    /// 64-bit IEEE 754 float (4 registers)
    Float64,
}

impl DataType {
    /// Returns the number of 16-bit registers needed for this data type
    pub fn register_count(&self) -> u16 {
        match self {
            DataType::Bool | DataType::Uint16 | DataType::Int16 => 1,
            DataType::Uint32 | DataType::Int32 | DataType::Float32 => 2,
            DataType::Uint64 | DataType::Int64 | DataType::Float64 => 4,
        }
    }
}

/// Byte order within a 16-bit register
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    /// Big-endian (most significant byte first) - Modbus standard
    #[default]
    BigEndian,
    /// Little-endian (least significant byte first)
    LittleEndian,
}

/// Word order for multi-register values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WordOrder {
    /// Big-endian (most significant word/register first)
    #[default]
    BigEndian,
    /// Little-endian (least significant word/register first)
    LittleEndian,
}

impl SiteConfig {
    /// Load configuration from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::Io {
            path: path.as_ref().to_string_lossy().to_string(),
            source: e,
        })?;

        toml::from_str(&content).map_err(|e| ConfigError::Parse {
            path: path.as_ref().to_string_lossy().to_string(),
            source: e,
        })
    }

    /// Find a tag by name across all connections
    pub fn find_tag(&self, tag_name: &str) -> Option<(&RtacConnection, &TagMapping)> {
        for conn in &self.connections {
            if let Some(tag) = conn.tags.iter().find(|t| t.name == tag_name) {
                return Some((conn, tag));
            }
        }
        None
    }

    /// Get all tags as a flat list with their connection info
    pub fn all_tags(&self) -> Vec<(&RtacConnection, &TagMapping)> {
        self.connections
            .iter()
            .flat_map(|conn| conn.tags.iter().map(move |tag| (conn, tag)))
            .collect()
    }

    /// Get all tags for a specific equipment ID
    pub fn tags_for_equipment(&self, equipment_id: &str) -> Vec<(&RtacConnection, &TagMapping)> {
        self.all_tags()
            .into_iter()
            .filter(|(_, tag)| tag.equipment_id.as_deref() == Some(equipment_id))
            .collect()
    }
}

impl RtacConnection {
    /// Get the socket address string
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Find a tag by name in this connection (for future use)
    #[allow(dead_code)]
    pub fn find_tag(&self, tag_name: &str) -> Option<&TagMapping> {
        self.tags.iter().find(|t| t.name == tag_name)
    }

    /// Get tags grouped by unit ID (for future use)
    #[allow(dead_code)]
    pub fn tags_by_unit(&self) -> HashMap<u8, Vec<&TagMapping>> {
        let mut by_unit: HashMap<u8, Vec<&TagMapping>> = HashMap::new();
        for tag in &self.tags {
            by_unit.entry(tag.unit_id).or_default().push(tag);
        }
        by_unit
    }
}

impl TagMapping {
    /// Apply scale and offset to convert raw value to engineering value
    pub fn to_engineering(&self, raw: f64) -> f64 {
        raw * self.scale + self.offset
    }

    /// Apply inverse scale and offset to convert engineering value to raw value
    pub fn to_raw(&self, engineering: f64) -> f64 {
        (engineering - self.offset) / self.scale
    }

    /// Check if a value is within the allowed range
    pub fn validate_value(&self, value: f64) -> Result<(), String> {
        if let Some(min) = self.min_value
            && value < min
        {
            return Err(format!("Value {} is below minimum {}", value, min));
        }
        if let Some(max) = self.max_value
            && value > max
        {
            return Err(format!("Value {} is above maximum {}", value, max));
        }
        Ok(())
    }
}

/// Configuration errors
#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse {
        path: String,
        source: toml::de::Error,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "Failed to read config file '{}': {}", path, source)
            }
            ConfigError::Parse { path, source } => {
                write!(f, "Failed to parse config file '{}': {}", path, source)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            ConfigError::Parse { source, .. } => Some(source),
        }
    }
}
