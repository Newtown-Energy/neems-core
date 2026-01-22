# Architecture: Modbus Integration with NEEMS

This document describes how Modbus communication fits into the NEEMS architecture and the design decisions made in this proof of concept.

## NEEMS Data Model

NEEMS (Newtown Energy Management System) organizes data hierarchically:

```
Company
└── Site (physical location)
    └── Equipment (battery, inverter, meter, etc.)
        └── Data Points (SOC, power, voltage, etc.)
```

## How Modbus Maps to NEEMS

### Site Level
- Each **site** has one or more **RTAC connections**
- An RTAC is the communication gateway between NEEMS and field equipment
- Connection details (IP, port) are site-specific

### Equipment Level
- **Equipment** in NEEMS corresponds to physical devices (batteries, inverters)
- Each equipment item has multiple **tags** (data points)
- Tags are mapped to Modbus registers via configuration

### Data Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              NEEMS System                                    │
│                                                                             │
│  ┌─────────────────────┐      ┌─────────────────────┐                      │
│  │      neems-api      │      │     neems-react     │                      │
│  │   (REST API Server) │◄────►│   (Web Dashboard)   │                      │
│  └──────────┬──────────┘      └─────────────────────┘                      │
│             │                                                               │
│             │ Database                                                      │
│             ▼                                                               │
│  ┌─────────────────────┐                                                   │
│  │     neems-data      │◄─── Configuration (TOML files)                    │
│  │  (Data Aggregator)  │                                                   │
│  │                     │                                                   │
│  │  ┌───────────────┐  │                                                   │
│  │  │ Modbus Client │  │                                                   │
│  │  └───────┬───────┘  │                                                   │
│  └──────────┼──────────┘                                                   │
│             │                                                               │
└─────────────┼───────────────────────────────────────────────────────────────┘
              │ Modbus TCP
              │
┌─────────────┼───────────────────────────────────────────────────────────────┐
│             ▼                             Site Infrastructure               │
│  ┌─────────────────────┐                                                   │
│  │      SEL-RTAC       │                                                   │
│  │  (Modbus Server)    │                                                   │
│  └──────────┬──────────┘                                                   │
│             │ Various Protocols (DNP3, IEC 61850, Modbus RTU)              │
│             │                                                               │
│    ┌────────┴────────┬────────────────┐                                    │
│    ▼                 ▼                ▼                                    │
│  ┌─────┐         ┌─────┐         ┌─────┐                                   │
│  │ BMS │         │ PCS │         │Meter│                                   │
│  └─────┘         └─────┘         └─────┘                                   │
│  Battery         Inverter        Revenue                                   │
│  Management      Power           Meter                                     │
│  System          Conversion                                                │
│                  System                                                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Configuration-Driven Design

### Why Configuration Files?

1. **Separation of Concerns**: Code doesn't need to know specific addresses
2. **Flexibility**: Different sites have different RTAC configurations
3. **Maintainability**: Tag mappings can change without code changes
4. **Documentation**: Config files document the Modbus map
5. **Version Control**: Changes to mappings are tracked in git

### Configuration Hierarchy

```
Site Configuration (TOML)
├── site_id: Links to NEEMS site
├── name: Human-readable
└── connections[]: One per RTAC
    ├── id: Unique identifier
    ├── host/port: Network address
    └── tags[]: Tag mappings
        ├── name: Unique tag name
        ├── equipment_id: Links to NEEMS equipment
        ├── address: Modbus register
        ├── data_type: How to interpret bytes
        ├── scale/offset: Value conversion
        └── writable: Control point flag
```

## Proof of Concept Components

### Current Structure

```
modbus-poc/
├── src/
│   ├── main.rs      # CLI entry point
│   ├── config.rs    # Configuration parsing and types
│   ├── client.rs    # Modbus client operations
│   └── server.rs    # Mock server for testing
├── docs/
│   ├── modbus-overview.md   # Protocol explanation
│   ├── configuration.md     # Config file guide
│   └── architecture.md      # This document
└── example-config.toml      # Sample configuration
```

### Key Design Decisions

#### 1. Tag-Based Abstraction
Instead of working with raw addresses, users work with meaningful tag names:

```rust
// Raw (error-prone, hard to maintain)
client::read_registers("192.168.1.100", 502, 1, 100, 2).await?;

// Tag-based (clear intent, maintainable)
client::read_tag(&config, "battery1_soc").await?;
```

#### 2. Engineering Value Conversion
The client handles scaling automatically:

```rust
// Configuration defines: scale = 0.1, units = "%"
// Raw Modbus value: 850
// Returned engineering value: 85.0%

let result = client::read_tag(&config, "battery1_soc").await?;
println!("{} {}", result.engineering_value, result.units); // "85.0 %"
```

#### 3. Write Validation
Writable tags can define limits:

```toml
[[connections.tags]]
name = "power_setpoint"
writable = true
min_value = -500.0
max_value = 500.0
```

```rust
// This will fail with validation error
client::write_tag(&config, "power_setpoint", 1000.0).await?; // Error: exceeds max
```

#### 4. Equipment Grouping
Tags can be grouped by equipment for bulk operations:

```rust
// Read all tags for battery-1
let readings = client::read_equipment_tags(&config, "battery-1").await?;
for reading in readings {
    println!("{}: {}", reading.tag_name, reading.engineering_value);
}
```

## Future Integration with neems-data

### Current State: Standalone PoC
The modbus-poc is currently a standalone tool for testing and experimentation.

### Planned Integration
The Modbus client will become a **collector type** in neems-data:

```
neems-data (Data Aggregator)
├── Existing Collectors
│   ├── ping (network connectivity)
│   ├── disk_space (system monitoring)
│   └── charging_state (demo/simulation)
└── New Collectors
    └── modbus (RTAC communication) ← Integration point
```

### Integration Approach

1. **Add Modbus TestType**: New variant in `DataCollector` enum
2. **Configuration Loading**: Load site config from file or database
3. **Tag Polling**: Poll configured tags at specified intervals
4. **Data Storage**: Store readings in time-series database
5. **Equipment Association**: Link readings to NEEMS equipment IDs

### Example Future Usage

```bash
# Add a Modbus data source to neems-data
neems-data add rtac-battery -t modbus \
  -a config_file=/etc/neems/site-001.toml \
  -a tags=battery1_soc,battery1_power,battery1_voltage \
  --interval 5
```

## Control Path (Schedules to Equipment)

### Current NEEMS Schedule System
NEEMS has a schedule system for planned operations:

```
Schedule
├── site_id
├── schedule_type (e.g., "charge", "discharge")
├── start_time / end_time
└── parameters (power levels, etc.)
```

### Control Flow (Future)

```
┌──────────────────┐
│  NEEMS Schedule  │
│  (API/Database)  │
└────────┬─────────┘
         │ Schedule becomes active
         ▼
┌──────────────────┐
│  Schedule Runner │
│  (neems-data?)   │
└────────┬─────────┘
         │ Determine required setpoints
         ▼
┌──────────────────┐
│   Modbus Writer  │
│ (write_tag API)  │
└────────┬─────────┘
         │ Modbus TCP
         ▼
┌──────────────────┐
│    SEL-RTAC      │
│  (Field Device)  │
└──────────────────┘
```

### Safety Considerations

1. **Validation**: Engineering limits in configuration
2. **Authentication**: Future: require auth for write operations
3. **Audit Logging**: Track all control commands
4. **Rate Limiting**: Prevent rapid-fire commands
5. **Confirmation**: Optionally require confirmation for critical commands

## Testing Strategy

### Mock Server
The built-in mock server enables testing without hardware:

```bash
# Terminal 1: Start mock server
modbus-poc server --port 5502

# Terminal 2: Test operations
modbus-poc read-tag --config example-config.toml --tag battery1_soc
```

### Integration Testing
1. **Unit Tests**: Test config parsing, value conversion
2. **Mock Tests**: Test client against mock server
3. **Integration Tests**: Test against real RTAC (manual/CI)

### Configuration Validation
Before deployment:
1. Validate TOML syntax
2. Check for duplicate tag names
3. Verify address ranges
4. Test connection to RTAC
5. Verify sample reads return expected data types

## Security Considerations

### Network Security
- Modbus TCP has no built-in authentication
- Rely on network segmentation (RTAC on isolated network)
- Use VPN for remote access
- Consider Modbus/TCP security extensions (if RTAC supports)

### Application Security
- Configuration files may contain sensitive network info
- Store configs with appropriate permissions
- Log access to control operations
- Implement role-based access in NEEMS for write operations

## Summary

This proof of concept demonstrates:

1. **Modbus TCP communication** with RTAC devices
2. **Configuration-driven tag mapping** for maintainability
3. **Engineering value conversion** with scaling
4. **Write validation** with configurable limits
5. **Equipment grouping** for organized data access

The design prepares for future integration with neems-data while providing immediate utility for testing and development.
