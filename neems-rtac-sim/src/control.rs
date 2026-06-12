//! Interactive control of the simulated RTAC over stdin.
//!
//! Each line typed into the running process is parsed into a [`ControlCommand`]
//! and applied to the shared state. This lets a developer inject alarms, force
//! commands, or jump the SoC while exercising the RTAC integration.
//!
//! Supported commands:
//! - `soc <percent>`            — set the state of charge directly
//! - `charge | discharge | trickle | standby | estop | clear` — issue a command
//! - `alarm set <num>` / `alarm clear <num>` — toggle an alarm by number
//! - `status`                   — print the current state
//! - `help`                     — print the command list
//! - `quit` / `exit`            — stop the simulator

use neems_data::rtac::protocol::CommandType;

use crate::server::SharedState;

/// The outcome of handling one control line.
#[derive(Debug, PartialEq)]
pub enum ControlOutcome {
    /// A message to print back to the operator.
    Message(String),
    /// The operator asked to quit.
    Quit,
}

/// Parse and apply a single control line, returning a response.
pub fn handle_line(state: &SharedState, line: &str) -> Option<ControlOutcome> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap().to_ascii_lowercase();
    let rest: Vec<&str> = parts.collect();

    let msg = match cmd.as_str() {
        "quit" | "exit" => return Some(ControlOutcome::Quit),
        "help" | "?" => help_text(),
        "status" => state.lock().unwrap().summary(),
        "soc" => match rest.first().and_then(|s| s.parse::<f32>().ok()) {
            Some(pct) => {
                state.lock().unwrap().set_soc(pct);
                format!("soc set to {:.1}%", pct.clamp(0.0, 100.0))
            }
            None => "usage: soc <percent>".to_string(),
        },
        "charge" => apply_command(state, CommandType::Charge),
        "discharge" => apply_command(state, CommandType::Discharge),
        "trickle" => apply_command(state, CommandType::TrickleCharge),
        "standby" => apply_command(state, CommandType::Standby),
        "estop" => apply_command(state, CommandType::EmergencyStop),
        "clear" => apply_command(state, CommandType::ClearFaults),
        "alarm" => handle_alarm(state, &rest),
        other => format!("unknown command '{}' (try 'help')", other),
    };

    Some(ControlOutcome::Message(msg))
}

fn apply_command(state: &SharedState, command: CommandType) -> String {
    state.lock().unwrap().set_command(command);
    format!("command set to {}", command)
}

fn handle_alarm(state: &SharedState, rest: &[&str]) -> String {
    let action = rest.first().copied();
    let num = rest.get(1).and_then(|s| s.parse::<u16>().ok());
    match (action, num) {
        (Some("set"), Some(n)) => {
            state.lock().unwrap().set_alarm(n, true);
            format!("alarm {} set", n)
        }
        (Some("clear"), Some(n)) => {
            state.lock().unwrap().set_alarm(n, false);
            format!("alarm {} cleared", n)
        }
        _ => "usage: alarm set <num> | alarm clear <num>".to_string(),
    }
}

fn help_text() -> String {
    [
        "commands:",
        "  soc <percent>          set state of charge",
        "  charge | discharge     issue charge / discharge command",
        "  trickle | standby      issue trickle-charge / standby command",
        "  estop | clear          emergency stop / clear faults",
        "  alarm set <num>        set an alarm by number",
        "  alarm clear <num>      clear an alarm by number",
        "  status                 print current state",
        "  quit                   stop the simulator",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use neems_data::rtac::{alarm_definitions::ESTOP_ALARM_NUM, protocol::OperatingMode};

    use super::*;
    use crate::{config::SimConfig, state::SimState};

    fn shared() -> SharedState {
        Arc::new(Mutex::new(SimState::new(SimConfig::default())))
    }

    #[test]
    fn empty_line_is_ignored() {
        let state = shared();
        assert_eq!(handle_line(&state, "   "), None);
    }

    #[test]
    fn quit_is_recognized() {
        let state = shared();
        assert_eq!(handle_line(&state, "quit"), Some(ControlOutcome::Quit));
        assert_eq!(handle_line(&state, "exit"), Some(ControlOutcome::Quit));
    }

    #[test]
    fn soc_command_sets_soc() {
        let state = shared();
        handle_line(&state, "soc 73.5");
        assert_eq!(state.lock().unwrap().soc_percent, 73.5);
    }

    #[test]
    fn charge_command_sets_command_register_and_ticks() {
        let state = shared();
        handle_line(&state, "soc 50");
        handle_line(&state, "charge");
        state.lock().unwrap().tick();
        assert_eq!(state.lock().unwrap().mode, OperatingMode::Charging);
    }

    #[test]
    fn alarm_set_and_clear() {
        let state = shared();
        handle_line(&state, &format!("alarm set {}", ESTOP_ALARM_NUM));
        assert!(state.lock().unwrap().alarms.is_estop_active());
        handle_line(&state, &format!("alarm clear {}", ESTOP_ALARM_NUM));
        assert!(!state.lock().unwrap().alarms.is_estop_active());
    }
}
