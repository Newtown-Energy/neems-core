//! A simulated RTAC: a Modbus TCP server backed by a simple internal model of a
//! battery energy storage system.
//!
//! The simulator exposes the same Modbus register map the real RTAC uses (see
//! [`neems_data::rtac::protocol`]) so the RTAC integration can be exercised
//! end-to-end without hardware. Command-register writes (target charge,
//! operating mode) drive the simulated state of charge and alarm list, which
//! advance once per [tick](state::SimState::tick).

pub mod config;
pub mod control;
pub mod server;
pub mod state;
