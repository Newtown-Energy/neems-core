use crate::config::{ByteOrder, DataType, RegisterType, SiteConfig, TagMapping, WordOrder};
use std::net::SocketAddr;
use tokio::net::lookup_host;
use tokio_modbus::prelude::*;

/// Resolve a host:port string to a SocketAddr, supporting both IP addresses and hostnames.
async fn resolve_host(host: &str, port: u16) -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let addr_string = format!("{}:{}", host, port);
    let socket_addr = lookup_host(&addr_string)
        .await?
        .next()
        .ok_or_else(|| format!("Could not resolve '{}'", addr_string))?;
    Ok(socket_addr)
}

/// Read holding registers from a Modbus TCP server
pub async fn read_registers(
    host: &str,
    port: u16,
    unit: u8,
    address: u16,
    count: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr = resolve_host(host, port).await?;

    println!("Connecting to Modbus server at {}...", socket_addr);

    let mut ctx = tcp::connect_slave(socket_addr, Slave(unit)).await?;

    println!(
        "Reading {} holding register(s) starting at address {}...",
        count, address
    );

    let registers = ctx.read_holding_registers(address, count).await??;

    println!("Successfully read {} register(s):", registers.len());
    for (i, value) in registers.iter().enumerate() {
        let reg_addr = address + i as u16;
        println!("  Register {}: {} (0x{:04X})", reg_addr, *value, *value);
    }

    ctx.disconnect().await?;
    println!("Disconnected.");

    Ok(())
}

/// Write a single holding register
pub async fn write_register(
    host: &str,
    port: u16,
    unit: u8,
    address: u16,
    value: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr = resolve_host(host, port).await?;

    println!("Connecting to Modbus server at {}...", socket_addr);

    let mut ctx = tcp::connect_slave(socket_addr, Slave(unit)).await?;

    println!(
        "Writing value {} (0x{:04X}) to register {}...",
        value, value, address
    );

    ctx.write_single_register(address, value).await??;

    println!("Successfully wrote to register {}.", address);

    ctx.disconnect().await?;
    println!("Disconnected.");

    Ok(())
}

/// Write multiple holding registers
pub async fn write_registers(
    host: &str,
    port: u16,
    unit: u8,
    address: u16,
    values: &[u16],
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr = resolve_host(host, port).await?;

    println!("Connecting to Modbus server at {}...", socket_addr);

    let mut ctx = tcp::connect_slave(socket_addr, Slave(unit)).await?;

    println!(
        "Writing {} value(s) starting at register {}...",
        values.len(),
        address
    );
    for (i, value) in values.iter().enumerate() {
        let reg_addr = address + i as u16;
        println!("  Register {}: {} (0x{:04X})", reg_addr, value, value);
    }

    ctx.write_multiple_registers(address, values).await??;

    println!("Successfully wrote {} register(s).", values.len());

    ctx.disconnect().await?;
    println!("Disconnected.");

    Ok(())
}

/// Scan a range of registers to discover which ones have non-zero values
pub async fn scan_registers(
    host: &str,
    port: u16,
    unit: u8,
    start: u16,
    end: u16,
    batch_size: u16,
    register_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let use_input = match register_type {
        "holding" => false,
        "input" => true,
        _ => return Err(format!("Unknown register type '{}'. Use 'holding' or 'input'.", register_type).into()),
    };

    let type_name = if use_input { "input" } else { "holding" };
    let socket_addr = resolve_host(host, port).await?;

    println!("Scanning {} registers {}-{} on {}...", type_name, start, end, socket_addr);
    println!();

    let mut ctx = tcp::connect_slave(socket_addr, Slave(unit)).await?;

    let mut non_zero_count: u32 = 0;
    let mut total_scanned: u32 = 0;
    let mut addr = start;

    while addr <= end {
        let count = batch_size.min(end - addr + 1);

        let batch_result = if use_input {
            ctx.read_input_registers(addr, count).await
        } else {
            ctx.read_holding_registers(addr, count).await
        };

        match batch_result {
            Ok(Ok(registers)) => {
                for (i, value) in registers.iter().enumerate() {
                    let reg_addr = addr + i as u16;
                    total_scanned += 1;
                    if *value != 0 {
                        non_zero_count += 1;
                        println!("  Register {:5}: {:6} (0x{:04X})", reg_addr, value, value);
                    }
                }
            }
            _ => {
                // Batch failed — try individual reads within this range
                for offset in 0..count {
                    let reg_addr = addr + offset;
                    let single_result = if use_input {
                        ctx.read_input_registers(reg_addr, 1).await
                    } else {
                        ctx.read_holding_registers(reg_addr, 1).await
                    };

                    total_scanned += 1;
                    match single_result {
                        Ok(Ok(registers)) => {
                            if let Some(value) = registers.first() {
                                if *value != 0 {
                                    non_zero_count += 1;
                                    println!("  Register {:5}: {:6} (0x{:04X})", reg_addr, value, value);
                                }
                            }
                        }
                        _ => {
                            // Individual register unreadable — skip silently
                        }
                    }
                }
            }
        }

        addr = addr.saturating_add(count);
        if count == 0 {
            break;
        }
    }

    ctx.disconnect().await?;

    println!();
    println!("Scan complete.");
    println!("  Total registers scanned: {}", total_scanned);
    println!("  Non-zero registers found: {}", non_zero_count);

    Ok(())
}

// ============================================================================
// Tag-based operations (configuration-driven)
// ============================================================================

/// Result of reading a tag
#[derive(Debug)]
pub struct TagReadResult {
    pub tag_name: String,
    pub raw_value: f64,
    pub engineering_value: f64,
    pub units: Option<String>,
}

/// Read a tag by name using configuration
pub async fn read_tag(
    config: &SiteConfig,
    tag_name: &str,
) -> Result<TagReadResult, Box<dyn std::error::Error>> {
    let (conn, tag) = config
        .find_tag(tag_name)
        .ok_or_else(|| format!("Tag '{}' not found in configuration", tag_name))?;

    let socket_addr: SocketAddr = conn.socket_addr().parse()?;
    println!(
        "Reading tag '{}' from {} (unit={}, address={})...",
        tag_name, socket_addr, tag.unit_id, tag.address
    );

    let mut ctx = tcp::connect_slave(socket_addr, Slave(tag.unit_id)).await?;

    let raw_value = read_tag_raw(&mut ctx, tag).await?;
    let engineering_value = tag.to_engineering(raw_value);

    ctx.disconnect().await?;

    let result = TagReadResult {
        tag_name: tag_name.to_string(),
        raw_value,
        engineering_value,
        units: tag.units.clone(),
    };

    println!(
        "  Raw: {}, Engineering: {}{}",
        result.raw_value,
        result.engineering_value,
        result.units.as_deref().map(|u| format!(" {}", u)).unwrap_or_default()
    );

    Ok(result)
}

/// Write a tag by name using configuration (engineering value)
pub async fn write_tag(
    config: &SiteConfig,
    tag_name: &str,
    engineering_value: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, tag) = config
        .find_tag(tag_name)
        .ok_or_else(|| format!("Tag '{}' not found in configuration", tag_name))?;

    if !tag.writable {
        return Err(format!("Tag '{}' is not writable", tag_name).into());
    }

    // Validate the value
    tag.validate_value(engineering_value)
        .map_err(|e| format!("Validation failed for tag '{}': {}", tag_name, e))?;

    let socket_addr: SocketAddr = conn.socket_addr().parse()?;
    let raw_value = tag.to_raw(engineering_value);

    println!(
        "Writing tag '{}' to {} (unit={}, address={})...",
        tag_name, socket_addr, tag.unit_id, tag.address
    );
    println!(
        "  Engineering: {}{} -> Raw: {}",
        engineering_value,
        tag.units.as_deref().map(|u| format!(" {}", u)).unwrap_or_default(),
        raw_value
    );

    let mut ctx = tcp::connect_slave(socket_addr, Slave(tag.unit_id)).await?;

    write_tag_raw(&mut ctx, tag, raw_value).await?;

    ctx.disconnect().await?;

    println!("Successfully wrote tag '{}'.", tag_name);

    Ok(())
}

/// Read all tags for an equipment ID
pub async fn read_equipment_tags(
    config: &SiteConfig,
    equipment_id: &str,
) -> Result<Vec<TagReadResult>, Box<dyn std::error::Error>> {
    let tags = config.tags_for_equipment(equipment_id);

    if tags.is_empty() {
        return Err(format!("No tags found for equipment '{}'", equipment_id).into());
    }

    println!("Reading {} tags for equipment '{}'...", tags.len(), equipment_id);

    let mut results = Vec::new();

    // Group tags by connection to minimize connections
    let mut by_conn: std::collections::HashMap<String, Vec<&TagMapping>> =
        std::collections::HashMap::new();
    for (conn, tag) in &tags {
        by_conn.entry(conn.id.clone()).or_default().push(tag);
    }

    for (conn, _tag) in &tags {
        if let Some(conn_tags) = by_conn.remove(&conn.id) {
            let socket_addr: SocketAddr = conn.socket_addr().parse()?;
            let mut ctx = tcp::connect_slave(socket_addr, Slave(1)).await?;

            for tag in conn_tags {
                // Reconnect with correct unit ID if needed
                ctx.disconnect().await?;
                ctx = tcp::connect_slave(socket_addr, Slave(tag.unit_id)).await?;

                match read_tag_raw(&mut ctx, tag).await {
                    Ok(raw_value) => {
                        let engineering_value = tag.to_engineering(raw_value);
                        results.push(TagReadResult {
                            tag_name: tag.name.clone(),
                            raw_value,
                            engineering_value,
                            units: tag.units.clone(),
                        });
                    }
                    Err(e) => {
                        eprintln!("  Warning: Failed to read tag '{}': {}", tag.name, e);
                    }
                }
            }

            ctx.disconnect().await?;
        }
    }

    Ok(results)
}

/// List all tags in the configuration
pub fn list_tags(config: &SiteConfig) {
    println!("Site: {} ({})", config.name, config.site_id);
    println!();

    for conn in &config.connections {
        println!("Connection: {} ({})", conn.name, conn.socket_addr());
        println!("{:-<60}", "");

        for tag in &conn.tags {
            let rw = if tag.writable { "RW" } else { "RO" };
            let units = tag.units.as_deref().unwrap_or("");
            let equip = tag
                .equipment_id
                .as_deref()
                .map(|e| format!(" [{}]", e))
                .unwrap_or_default();

            println!(
                "  {} ({}) - unit:{} addr:{} type:{:?} {}{}",
                tag.name, rw, tag.unit_id, tag.address, tag.data_type, units, equip
            );

            if let Some(desc) = &tag.description {
                println!("    {}", desc);
            }
        }
        println!();
    }
}

// ============================================================================
// Internal helpers for raw register operations
// ============================================================================

async fn read_tag_raw(
    ctx: &mut tokio_modbus::client::Context,
    tag: &TagMapping,
) -> Result<f64, Box<dyn std::error::Error>> {
    let count = tag.data_type.register_count();

    let registers = match tag.register_type {
        RegisterType::HoldingRegister => ctx.read_holding_registers(tag.address, count).await??,
        RegisterType::InputRegister => ctx.read_input_registers(tag.address, count).await??,
        RegisterType::Coil => {
            let coils = ctx.read_coils(tag.address, 1).await??;
            return Ok(if coils.first().copied().unwrap_or(false) {
                1.0
            } else {
                0.0
            });
        }
        RegisterType::DiscreteInput => {
            let inputs = ctx.read_discrete_inputs(tag.address, 1).await??;
            return Ok(if inputs.first().copied().unwrap_or(false) {
                1.0
            } else {
                0.0
            });
        }
    };

    Ok(decode_registers(&registers, tag.data_type, tag.byte_order, tag.word_order))
}

async fn write_tag_raw(
    ctx: &mut tokio_modbus::client::Context,
    tag: &TagMapping,
    value: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    match tag.register_type {
        RegisterType::HoldingRegister => {
            let registers = encode_value(value, tag.data_type, tag.byte_order, tag.word_order);
            if registers.len() == 1 {
                ctx.write_single_register(tag.address, registers[0]).await??;
            } else {
                ctx.write_multiple_registers(tag.address, &registers).await??;
            }
        }
        RegisterType::Coil => {
            ctx.write_single_coil(tag.address, value != 0.0).await??;
        }
        RegisterType::InputRegister | RegisterType::DiscreteInput => {
            return Err("Cannot write to read-only register type".into());
        }
    }

    Ok(())
}

fn decode_registers(registers: &[u16], data_type: DataType, _byte_order: ByteOrder, word_order: WordOrder) -> f64 {
    match data_type {
        DataType::Bool => {
            if registers.first().copied().unwrap_or(0) != 0 {
                1.0
            } else {
                0.0
            }
        }
        DataType::Uint16 => registers.first().copied().unwrap_or(0) as f64,
        DataType::Int16 => registers.first().copied().unwrap_or(0) as i16 as f64,
        DataType::Uint32 => {
            let regs = order_words(registers, word_order);
            let value = ((regs[0] as u32) << 16) | (regs[1] as u32);
            value as f64
        }
        DataType::Int32 => {
            let regs = order_words(registers, word_order);
            let value = ((regs[0] as u32) << 16) | (regs[1] as u32);
            (value as i32) as f64
        }
        DataType::Float32 => {
            let regs = order_words(registers, word_order);
            let bits = ((regs[0] as u32) << 16) | (regs[1] as u32);
            f32::from_bits(bits) as f64
        }
        DataType::Uint64 => {
            let regs = order_words(registers, word_order);
            let value = ((regs[0] as u64) << 48)
                | ((regs[1] as u64) << 32)
                | ((regs[2] as u64) << 16)
                | (regs[3] as u64);
            value as f64
        }
        DataType::Int64 => {
            let regs = order_words(registers, word_order);
            let value = ((regs[0] as u64) << 48)
                | ((regs[1] as u64) << 32)
                | ((regs[2] as u64) << 16)
                | (regs[3] as u64);
            (value as i64) as f64
        }
        DataType::Float64 => {
            let regs = order_words(registers, word_order);
            let bits = ((regs[0] as u64) << 48)
                | ((regs[1] as u64) << 32)
                | ((regs[2] as u64) << 16)
                | (regs[3] as u64);
            f64::from_bits(bits)
        }
    }
}

fn encode_value(value: f64, data_type: DataType, _byte_order: ByteOrder, word_order: WordOrder) -> Vec<u16> {
    match data_type {
        DataType::Bool => vec![if value != 0.0 { 1 } else { 0 }],
        DataType::Uint16 => vec![value as u16],
        DataType::Int16 => vec![(value as i16) as u16],
        DataType::Uint32 => {
            let v = value as u32;
            let regs = vec![(v >> 16) as u16, v as u16];
            order_words_vec(regs, word_order)
        }
        DataType::Int32 => {
            let v = (value as i32) as u32;
            let regs = vec![(v >> 16) as u16, v as u16];
            order_words_vec(regs, word_order)
        }
        DataType::Float32 => {
            let bits = (value as f32).to_bits();
            let regs = vec![(bits >> 16) as u16, bits as u16];
            order_words_vec(regs, word_order)
        }
        DataType::Uint64 => {
            let v = value as u64;
            let regs = vec![
                (v >> 48) as u16,
                (v >> 32) as u16,
                (v >> 16) as u16,
                v as u16,
            ];
            order_words_vec(regs, word_order)
        }
        DataType::Int64 => {
            let v = (value as i64) as u64;
            let regs = vec![
                (v >> 48) as u16,
                (v >> 32) as u16,
                (v >> 16) as u16,
                v as u16,
            ];
            order_words_vec(regs, word_order)
        }
        DataType::Float64 => {
            let bits = value.to_bits();
            let regs = vec![
                (bits >> 48) as u16,
                (bits >> 32) as u16,
                (bits >> 16) as u16,
                bits as u16,
            ];
            order_words_vec(regs, word_order)
        }
    }
}

fn order_words(registers: &[u16], word_order: WordOrder) -> Vec<u16> {
    match word_order {
        WordOrder::BigEndian => registers.to_vec(),
        WordOrder::LittleEndian => registers.iter().copied().rev().collect(),
    }
}

fn order_words_vec(mut registers: Vec<u16>, word_order: WordOrder) -> Vec<u16> {
    match word_order {
        WordOrder::BigEndian => registers,
        WordOrder::LittleEndian => {
            registers.reverse();
            registers
        }
    }
}
