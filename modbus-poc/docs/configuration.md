# Configuration Guide

This document describes how to configure the Modbus PoC for communicating with RTAC devices.

## Configuration File Format

Configuration files use [TOML](https://toml.io/) format. Each file defines the tag mappings for a single site.

## Configuration Structure

```toml
# Site identification
site_id = "site-001"           # Matches NEEMS site ID
name = "Example Solar Farm"     # Human-readable name

# One or more RTAC connections
[[connections]]
id = "rtac-main"
name = "Main RTAC"
host = "192.168.1.100"
port = 502
timeout_ms = 5000

# Tags for this connection
[[connections.tags]]
name = "battery_soc"
# ... tag configuration
```

## Site-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `site_id` | string | Yes | Unique identifier matching NEEMS site |
| `name` | string | Yes | Human-readable site name |
| `connections` | array | Yes | List of RTAC connections |

## Connection Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | string | Yes | - | Unique identifier for this connection |
| `name` | string | Yes | - | Human-readable connection name |
| `host` | string | Yes | - | Hostname or IP address of RTAC |
| `port` | integer | No | 502 | TCP port number |
| `timeout_ms` | integer | No | 5000 | Connection timeout in milliseconds |
| `tags` | array | Yes | - | List of tag mappings |

## Tag Mapping Fields

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Unique tag name (e.g., "battery1_soc") |
| `address` | integer | Modbus register address (0-65535) |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `description` | string | none | Human-readable description |
| `equipment_id` | string | none | NEEMS equipment ID this tag belongs to |
| `unit_id` | integer | 1 | Modbus unit/slave ID (1-247) |
| `register_type` | string | "holding_register" | Type of Modbus register |
| `data_type` | string | "uint16" | Data type for interpretation |
| `byte_order` | string | "big_endian" | Byte order within registers |
| `word_order` | string | "big_endian" | Word order for multi-register values |
| `scale` | float | 1.0 | Scale factor for conversion |
| `offset` | float | 0.0 | Offset for conversion |
| `units` | string | none | Engineering units (e.g., "V", "kW") |
| `writable` | boolean | false | Whether tag can be written |
| `min_value` | float | none | Minimum allowed value (for writes) |
| `max_value` | float | none | Maximum allowed value (for writes) |

## Register Types

| Value | Modbus Type | Access | Description |
|-------|-------------|--------|-------------|
| `holding_register` | 4xxxx | Read/Write | Most common, 16-bit values |
| `input_register` | 3xxxx | Read-only | 16-bit sensor values |
| `coil` | 0xxxx | Read/Write | Single-bit outputs |
| `discrete_input` | 1xxxx | Read-only | Single-bit inputs |

## Data Types

| Value | Registers | Description |
|-------|-----------|-------------|
| `bool` | 1 | Boolean (for coils/discrete inputs) |
| `uint16` | 1 | Unsigned 16-bit integer (0-65535) |
| `int16` | 1 | Signed 16-bit integer (±32767) |
| `uint32` | 2 | Unsigned 32-bit integer |
| `int32` | 2 | Signed 32-bit integer |
| `float32` | 2 | IEEE 754 single precision float |
| `uint64` | 4 | Unsigned 64-bit integer |
| `int64` | 4 | Signed 64-bit integer |
| `float64` | 4 | IEEE 754 double precision float |

## Byte and Word Order

### Byte Order (within each 16-bit register)
| Value | Description |
|-------|-------------|
| `big_endian` | Most significant byte first (Modbus standard) |
| `little_endian` | Least significant byte first |

### Word Order (for multi-register values)
| Value | Description |
|-------|-------------|
| `big_endian` | Most significant register first |
| `little_endian` | Least significant register first |

**Note:** Most devices use big-endian for both, but always verify with device documentation.

## Scaling and Offset

Values are converted using:

```
Engineering Value = (Raw Value × scale) + offset
Raw Value = (Engineering Value - offset) / scale
```

### Examples

**Voltage with 0.1V resolution:**
```toml
[[connections.tags]]
name = "battery_voltage"
address = 100
data_type = "uint16"
scale = 0.1      # Raw 4800 → 480.0V
units = "V"
```

**Temperature with offset:**
```toml
[[connections.tags]]
name = "ambient_temp"
address = 200
data_type = "int16"
scale = 0.1
offset = -40.0   # Raw 650 → (650 × 0.1) - 40 = 25.0°C
units = "°C"
```

**Percentage:**
```toml
[[connections.tags]]
name = "battery_soc"
address = 0
data_type = "uint16"
scale = 0.1      # Raw 850 → 85.0%
units = "%"
```

## Writable Tags

For control points, set `writable = true` and optionally define limits:

```toml
[[connections.tags]]
name = "power_setpoint"
description = "Battery power command (+ charge, - discharge)"
address = 100
data_type = "int16"
scale = 1.0
units = "kW"
writable = true
min_value = -500.0   # Max discharge
max_value = 500.0    # Max charge
```

The system will:
1. Validate the engineering value is within `min_value` to `max_value`
2. Convert to raw value using inverse scaling
3. Write to the Modbus register

## Equipment Grouping

Use `equipment_id` to group related tags:

```toml
[[connections.tags]]
name = "battery1_soc"
equipment_id = "battery-1"
# ...

[[connections.tags]]
name = "battery1_voltage"
equipment_id = "battery-1"
# ...

[[connections.tags]]
name = "battery2_soc"
equipment_id = "battery-2"
# ...
```

This enables reading all tags for a specific equipment:

```bash
modbus-poc read-equipment --config site.toml --equipment battery-1
```

## Complete Example

```toml
site_id = "newtown-farm"
name = "Newtown Solar Farm"

[[connections]]
id = "main-rtac"
name = "Main RTAC Controller"
host = "192.168.10.100"
port = 502
timeout_ms = 5000

# Battery 1 - Measurements
[[connections.tags]]
name = "battery1_soc"
description = "Battery 1 State of Charge"
equipment_id = "battery-1"
unit_id = 1
address = 0
register_type = "holding_register"
data_type = "uint16"
scale = 0.1
units = "%"
writable = false

[[connections.tags]]
name = "battery1_power"
description = "Battery 1 Real Power (+ charging)"
equipment_id = "battery-1"
unit_id = 1
address = 10
register_type = "holding_register"
data_type = "int32"
word_order = "big_endian"
scale = 0.001
units = "kW"
writable = false

# Battery 1 - Controls
[[connections.tags]]
name = "battery1_power_cmd"
description = "Battery 1 Power Command"
equipment_id = "battery-1"
unit_id = 1
address = 100
register_type = "holding_register"
data_type = "int16"
scale = 1.0
units = "kW"
writable = true
min_value = -1000.0
max_value = 1000.0

# Inverter
[[connections.tags]]
name = "inverter1_ac_power"
description = "Inverter 1 AC Output Power"
equipment_id = "inverter-1"
unit_id = 2
address = 0
register_type = "input_register"
data_type = "float32"
word_order = "big_endian"
scale = 1.0
units = "kW"
writable = false
```

## CLI Usage

### List all configured tags
```bash
modbus-poc list-tags --config site.toml
```

### Read a specific tag
```bash
modbus-poc read-tag --config site.toml --tag battery1_soc
```

### Write a tag (engineering value)
```bash
modbus-poc write-tag --config site.toml --tag battery1_power_cmd --value 500.0
```

### Read all tags for equipment
```bash
modbus-poc read-equipment --config site.toml --equipment battery-1
```

## Validating Configuration

1. **Test with mock server first**: Use the built-in mock server for initial testing
2. **Verify addresses**: Ensure addresses match RTAC Modbus map
3. **Check data types**: Verify scaling produces expected engineering values
4. **Test writable limits**: Confirm min/max values are appropriate

## Troubleshooting

### "Tag not found"
- Check tag name spelling matches configuration
- Verify configuration file path is correct

### "Connection refused"
- Verify RTAC IP address and port
- Check network connectivity
- Ensure RTAC Modbus server is enabled

### "Illegal data address"
- Address may be out of range for device
- Wrong register type (holding vs input)

### Wrong values
- Check byte/word order matches device
- Verify scale and offset
- Confirm data type (signed vs unsigned)
