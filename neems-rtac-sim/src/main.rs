//! Entry point for the simulated RTAC.
//!
//! Starts a Modbus TCP server, advances the simulated physics once per tick,
//! and (unless `--no-stdin` is given) reads interactive control commands from
//! stdin. See [`neems_rtac_sim::control`] for the command list.

use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use clap::Parser;
use neems_rtac_sim::{
    config::SimConfig,
    control::{self, ControlOutcome},
    server,
    state::SimState,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "neems-rtac-sim")]
#[command(about = "Simulated RTAC Modbus server for exercising the NEEMS RTAC integration")]
struct Cli {
    /// Address to bind the Modbus TCP server to.
    #[arg(long, default_value = "127.0.0.1:502")]
    bind: SocketAddr,

    /// Modbus unit/slave identifier.
    #[arg(long, default_value_t = 1)]
    unit_id: u8,

    /// Milliseconds between simulated physics ticks.
    #[arg(long, default_value_t = 1000)]
    tick_ms: u64,

    /// Initial state of charge (percent).
    #[arg(long, default_value_t = 50.0)]
    initial_soc: f32,

    /// SoC gained per tick while charging (percent).
    #[arg(long, default_value_t = 1.0)]
    charge_rate: f32,

    /// SoC lost per tick while discharging (percent).
    #[arg(long, default_value_t = 1.0)]
    discharge_rate: f32,

    /// SoC gained per tick while trickle charging (percent).
    #[arg(long, default_value_t = 0.1)]
    trickle_rate: f32,

    /// Lowest SoC the battery will discharge to (percent).
    #[arg(long, default_value_t = 0.0)]
    soc_floor: f32,

    /// Highest SoC the battery will charge to (percent).
    #[arg(long, default_value_t = 100.0)]
    soc_ceiling: f32,

    /// Disable the interactive stdin control loop (for CI / headless use).
    #[arg(long)]
    no_stdin: bool,
}

impl Cli {
    fn into_config(self) -> SimConfig {
        let defaults = SimConfig::default();
        SimConfig {
            bind_address: self.bind,
            unit_id: self.unit_id,
            tick_interval: Duration::from_millis(self.tick_ms),
            initial_soc_percent: self.initial_soc,
            soc_floor_percent: self.soc_floor,
            soc_ceiling_percent: self.soc_ceiling,
            charge_rate_pct: self.charge_rate,
            discharge_rate_pct: self.discharge_rate,
            trickle_rate_pct: self.trickle_rate,
            ..defaults
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let no_stdin = cli.no_stdin;
    let config = cli.into_config();

    let (listener, local_addr) = server::bind(config.bind_address).await?;
    let tick_interval = config.tick_interval;
    let state = Arc::new(Mutex::new(SimState::new(config)));

    info!(address = %local_addr, "simulated RTAC listening");

    // Advance the simulated physics on a fixed interval.
    let tick_state = state.clone();
    let tick_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick_interval);
        loop {
            interval.tick().await;
            tick_state.lock().unwrap().tick();
        }
    });

    // Serve Modbus requests.
    let server_state = state.clone();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server::serve(listener, server_state).await {
            error!(error = %e, "modbus server stopped");
        }
    });

    if no_stdin {
        tokio::select! {
            _ = server_handle => {}
            _ = tokio::signal::ctrl_c() => info!("received ctrl-c, shutting down"),
        }
    } else {
        run_control_loop(&state).await;
    }

    tick_handle.abort();
    Ok(())
}

/// Read control commands from stdin until EOF, `quit`, or ctrl-c.
async fn run_control_loop(state: &server::SharedState) {
    println!("Simulated RTAC ready. Type 'help' for commands, 'quit' to stop.");
    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    loop {
        tokio::select! {
            next = lines.next_line() => match next {
                Ok(Some(line)) => match control::handle_line(state, &line) {
                    Some(ControlOutcome::Quit) => break,
                    Some(ControlOutcome::Message(msg)) => println!("{msg}"),
                    None => {}
                },
                Ok(None) => break, // EOF
                Err(e) => {
                    error!(error = %e, "stdin read error");
                    break;
                }
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    info!("shutting down");
}
