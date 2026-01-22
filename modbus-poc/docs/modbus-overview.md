# Modbus Protocol Overview

This document provides an overview of the Modbus protocol as it relates to communicating with SEL-RTAC devices and battery energy storage systems.

## What is Modbus?

Modbus is an industrial communication protocol developed by Modicon in 1979. It has become a de facto standard for connecting industrial electronic devices, particularly in SCADA (Supervisory Control and Data Acquisition) systems.

### Key Characteristics

- **Simple**: Easy to implement and understand
- **Open**: No licensing fees, publicly available specification
- **Widely Supported**: Nearly universal support in industrial equipment
- **Master/Slave Architecture**: One master initiates requests, slaves respond

## Modbus Variants

### Modbus RTU (Serial)
- Uses RS-485 or RS-232 serial connections
- Binary encoding for efficiency
- Uses device addresses (1-247) on shared bus
- Common in legacy systems and field devices

### Modbus TCP (Ethernet)
- Runs over TCP/IP networks
- Standard port: **502**
- Wraps Modbus RTU frames in TCP packets
- **This is what we use with SEL-RTAC devices**

### Modbus ASCII
- Serial variant using ASCII encoding
- Slower but human-readable
- Rarely used in modern systems

## Addressing Model

Modbus uses a hierarchical addressing model:

```
┌─────────────────────────────────────────────────────────────┐
│                    Network Level                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Host:Port (e.g., 192.168.1.100:502)                │    │
│  │  ┌─────────────────────────────────────────────┐    │    │
│  │  │  Unit ID (1-247)                            │    │    │
│  │  │  ┌─────────────────────────────────────┐    │    │    │
│  │  │  │  Register Type + Address (0-65535)  │    │    │    │
│  │  │  └─────────────────────────────────────┘    │    │    │
│  │  └─────────────────────────────────────────────┘    │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Host and Port
- The TCP endpoint of the Modbus server
- Standard port is 502, but can vary
- One RTAC = one IP address (typically)

### Unit ID (Slave Address)
- 1-byte identifier (1-247)
- Originally for addressing multiple devices on serial bus
- In TCP, used for:
  - Gateways bridging to serial devices
  - Logical device groupings within a server
  - RTACs may use different unit IDs for different subsystems

### Register Address
- 16-bit address (0-65535)
- Identifies specific data within the device

## Register Types (Address Spaces)

Modbus defines **four separate address spaces**, each for different purposes:

| Type | Address Range | Access | Size | Typical Use |
|------|--------------|--------|------|-------------|
| **Coils** | 0xxxx (0-65535) | Read/Write | 1 bit | Digital outputs, control flags |
| **Discrete Inputs** | 1xxxx (0-65535) | Read-only | 1 bit | Digital inputs, status bits |
| **Input Registers** | 3xxxx (0-65535) | Read-only | 16 bits | Sensor readings, measurements |
| **Holding Registers** | 4xxxx (0-65535) | Read/Write | 16 bits | Setpoints, configuration, bidirectional data |

### Important Notes
- The address ranges (0xxxx, 1xxxx, etc.) are conventions, not wire protocol
- On the wire, all addresses are 0-65535
- The **function code** determines which address space is accessed
- Most battery/inverter data uses **Holding Registers** or **Input Registers**

## Function Codes

Function codes specify the operation to perform:

| Code | Name | Description |
|------|------|-------------|
| 01 | Read Coils | Read 1-2000 coils (bits) |
| 02 | Read Discrete Inputs | Read 1-2000 discrete inputs |
| **03** | **Read Holding Registers** | Read 1-125 holding registers |
| **04** | **Read Input Registers** | Read 1-125 input registers |
| 05 | Write Single Coil | Write one coil |
| **06** | **Write Single Register** | Write one holding register |
| 15 | Write Multiple Coils | Write multiple coils |
| **16** | **Write Multiple Registers** | Write multiple holding registers |

**Bold** = commonly used for BESS/RTAC communication

## Data Types and Register Mapping

### Single-Register Types (16 bits)
```
Register N
┌───────────────────────────────┐
│  UINT16: 0 to 65,535          │
│  INT16: -32,768 to 32,767     │
└───────────────────────────────┘
```

### Multi-Register Types

**32-bit values (2 registers):**
```
Register N        Register N+1
┌─────────────────┬─────────────────┐
│   High Word     │    Low Word     │  ← Big-endian (most common)
└─────────────────┴─────────────────┘

Register N        Register N+1
┌─────────────────┬─────────────────┐
│   Low Word      │    High Word    │  ← Little-endian (some devices)
└─────────────────┴─────────────────┘
```

**64-bit values (4 registers):**
```
Register N    N+1           N+2           N+3
┌────────────┬────────────┬────────────┬────────────┐
│  Highest   │    High    │    Low     │   Lowest   │  ← Big-endian
└────────────┴────────────┴────────────┴────────────┘
```

### Common Data Types

| Type | Registers | Range/Notes |
|------|-----------|-------------|
| UINT16 | 1 | 0 to 65,535 |
| INT16 | 1 | -32,768 to 32,767 |
| UINT32 | 2 | 0 to 4,294,967,295 |
| INT32 | 2 | ±2.1 billion |
| FLOAT32 | 2 | IEEE 754 single precision |
| UINT64 | 4 | Very large integers |
| FLOAT64 | 4 | IEEE 754 double precision |

### Byte and Word Order

Devices vary in how they arrange bytes:

- **Byte Order** (within a register): Big-endian (Modbus standard) or Little-endian
- **Word Order** (for multi-register values): Big-endian or Little-endian

**Example: Representing 0x12345678 as two registers**

| Word Order | Register N | Register N+1 |
|------------|------------|--------------|
| Big-endian | 0x1234 | 0x5678 |
| Little-endian | 0x5678 | 0x1234 |

Always verify with device documentation!

## Scaling and Engineering Units

Raw Modbus values often need conversion to engineering units:

```
Engineering Value = (Raw Value × Scale) + Offset
```

**Examples:**

| Description | Raw Value | Scale | Offset | Engineering Value |
|-------------|-----------|-------|--------|-------------------|
| Voltage (0.1V resolution) | 4800 | 0.1 | 0 | 480.0 V |
| Temperature (0.1°C, -40 offset) | 650 | 0.1 | -40 | 25.0°C |
| Power (kW, signed) | -500 | 1.0 | 0 | -500 kW (discharging) |
| SOC (0.1% resolution) | 850 | 0.1 | 0 | 85.0% |

## SEL-RTAC Specifics

### What is an RTAC?

The SEL Real-Time Automation Controller (RTAC) is a programmable automation controller used in substations and industrial facilities. It can:

- Communicate with various field devices (relays, meters, PLCs)
- Run custom logic programs
- Expose data via Modbus TCP server
- Act as a Modbus gateway to other devices

### RTAC Modbus Configuration

RTACs are configured using **ACSELERATOR RTAC** software:

1. **Define Tags**: Create internal tags for data points
2. **Map to Modbus**: Assign tags to Modbus addresses
3. **Configure Server**: Set up Modbus TCP server settings
4. **Deploy**: Download configuration to RTAC

The RTAC's Modbus map (which addresses correspond to which tags) is defined during programming. Our NEEMS configuration mirrors this mapping on the client side.

### Typical RTAC Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        NEEMS System                          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              modbus-poc / neems-data                │    │
│  │         (Modbus TCP Client)                         │    │
│  └──────────────────────┬──────────────────────────────┘    │
│                         │ Modbus TCP (port 502)              │
└─────────────────────────┼───────────────────────────────────┘
                          │
┌─────────────────────────┼───────────────────────────────────┐
│                    SEL-RTAC                                  │
│  ┌──────────────────────┴──────────────────────────────┐    │
│  │              Modbus TCP Server                       │    │
│  │         (Exposes configured tags)                    │    │
│  └──────────────────────┬──────────────────────────────┘    │
│                         │ Internal Tag Database              │
│  ┌──────────────────────┴──────────────────────────────┐    │
│  │              RTAC Logic Programs                     │    │
│  │         (Process data, run control logic)            │    │
│  └───────────┬────────────────────────────┬────────────┘    │
│              │ DNP3/IEC 61850/Modbus      │                  │
└──────────────┼────────────────────────────┼─────────────────┘
               │                            │
    ┌──────────┴──────────┐      ┌─────────┴─────────┐
    │   Battery BMS       │      │    Inverter PCS    │
    │   (Field Device)    │      │   (Field Device)   │
    └─────────────────────┘      └───────────────────┘
```

## Error Handling

### Exception Responses

When a request fails, the server returns an exception response with an error code:

| Code | Name | Description |
|------|------|-------------|
| 01 | Illegal Function | Function code not supported |
| 02 | Illegal Data Address | Address out of range |
| 03 | Illegal Data Value | Value out of range |
| 04 | Slave Device Failure | Internal device error |
| 06 | Slave Device Busy | Device is processing another request |

### Common Issues

1. **Connection Timeout**: Network issue or wrong IP/port
2. **Illegal Data Address**: Wrong address or register count
3. **Illegal Function**: Wrong register type for operation
4. **Wrong Data**: Incorrect byte/word order or data type

## Best Practices

1. **Verify Configuration**: Always test with device documentation
2. **Batch Reads**: Read multiple consecutive registers in one request (more efficient)
3. **Polling Intervals**: Don't poll faster than needed (1-10 seconds typical)
4. **Error Handling**: Always handle exceptions gracefully
5. **Timeouts**: Set appropriate timeouts (2-5 seconds typical)
6. **Connection Management**: Reuse connections when possible, but handle disconnects

## References

- [Modbus Application Protocol Specification](https://modbus.org/docs/Modbus_Application_Protocol_V1_1b3.pdf)
- [Modbus Messaging on TCP/IP Implementation Guide](https://modbus.org/docs/Modbus_Messaging_Implementation_Guide_V1_0b.pdf)
- [SEL-RTAC Documentation](https://selinc.com/products/RTAC/)
