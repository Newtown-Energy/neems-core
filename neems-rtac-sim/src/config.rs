//! Configuration for the simulated RTAC.
//!
//! [`SimConfig`] captures the tunable parameters of the simulation: how fast
//! the state of charge moves in each mode, the nominal electrical readings the
//! device reports, and the bounds the SoC is clamped to.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

/// Tunable parameters for the simulated RTAC.
#[derive(Debug, Clone)]
pub struct SimConfig {
    /// Address the Modbus TCP server binds to.
    pub bind_address: SocketAddr,
    /// Modbus unit/slave identifier the server responds as.
    pub unit_id: u8,
    /// How often the simulated physics advances (the "once-per-second" tick).
    pub tick_interval: Duration,

    /// State of charge the simulation starts at (percent, 0-100).
    pub initial_soc_percent: f32,
    /// Lowest SoC the battery will discharge to (percent).
    pub soc_floor_percent: f32,
    /// Highest SoC the battery will charge to (percent).
    pub soc_ceiling_percent: f32,

    /// SoC gained per tick while charging (percent per tick).
    pub charge_rate_pct: f32,
    /// SoC lost per tick while discharging (percent per tick).
    pub discharge_rate_pct: f32,
    /// SoC gained per tick while trickle charging (percent per tick).
    pub trickle_rate_pct: f32,

    /// Power drawn while charging (kW). Reported as negative power.
    pub charge_power_kw: f32,
    /// Power delivered while discharging (kW). Reported as positive power.
    pub discharge_power_kw: f32,

    /// Nominal DC bus voltage reported (volts).
    pub nominal_voltage_v: f32,
    /// Baseline temperature reported when idle (Celsius).
    pub idle_temperature_c: f32,
    /// Additional temperature reported while actively charging/discharging.
    pub active_temperature_rise_c: f32,
    /// Nominal grid frequency reported (Hz).
    pub nominal_frequency_hz: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            // Matches the default the real `ModbusClient` connects to.
            bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 502),
            unit_id: 1,
            tick_interval: Duration::from_secs(1),

            initial_soc_percent: 50.0,
            soc_floor_percent: 0.0,
            soc_ceiling_percent: 100.0,

            charge_rate_pct: 1.0,
            discharge_rate_pct: 1.0,
            trickle_rate_pct: 0.1,

            charge_power_kw: 250.0,
            discharge_power_kw: 250.0,

            nominal_voltage_v: 480.0,
            idle_temperature_c: 25.0,
            active_temperature_rise_c: 5.0,
            nominal_frequency_hz: 60.0,
        }
    }
}
