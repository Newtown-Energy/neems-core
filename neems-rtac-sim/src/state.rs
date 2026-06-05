//! Simulated internal state of the RTAC.
//!
//! [`SimState`] holds the device's "physical" state (state of charge, power,
//! electrical readings and the alarm list) together with the raw command
//! registers most recently written over Modbus. The Modbus server reads and
//! writes registers through [`SimState::read_registers`] /
//! [`SimState::write_registers`], and [`SimState::tick`] advances the simulated
//! physics once per tick (driven at 1 Hz by the binary).
//!
//! The register layout is taken directly from [`neems_data::rtac::protocol`] so
//! the simulator and the real client can never drift apart.

use neems_data::rtac::{
    alarm_definitions::{ALARM_REGISTER_COUNT, ESTOP_ALARM_NUM},
    protocol::{
        CommandType, OperatingMode, RegisterMap, current_to_register, grid_frequency_to_register,
        parse_soc, power_kw_to_registers, soc_to_register, temperature_to_register,
        voltage_to_register,
    },
    state::AlarmFlags,
};

use crate::config::SimConfig;

/// Number of command registers (mirrors [`RegisterMap::CMD_WRITE_COUNT`]).
const CMD_COUNT: usize = RegisterMap::CMD_WRITE_COUNT as usize;

/// The simulated internal state of the RTAC.
#[derive(Debug, Clone)]
pub struct SimState {
    /// Simulation tuning parameters.
    pub config: SimConfig,

    // --- Reported "physical" state (status registers) ---
    /// Current operating mode.
    pub mode: OperatingMode,
    /// State of charge (percent, 0-100).
    pub soc_percent: f32,
    /// Active power in kW (positive = discharging, negative = charging).
    pub power_kw: f32,
    /// DC bus voltage (volts).
    pub voltage_v: f32,
    /// Current (amps, signed; negative while charging).
    pub current_a: f32,
    /// Temperature (Celsius).
    pub temperature_c: f32,
    /// Grid frequency (Hz).
    pub grid_frequency_hz: f32,
    /// Active alarm list.
    pub alarms: AlarmFlags,

    // --- Inputs (command registers, as written over Modbus) ---
    /// Raw command registers (addresses 100-104).
    pub cmd_regs: [u16; CMD_COUNT],
}

impl SimState {
    /// Create a fresh simulation in standby at the configured initial SoC.
    pub fn new(config: SimConfig) -> Self {
        let mut state = Self {
            mode: OperatingMode::Standby,
            soc_percent: config.initial_soc_percent.clamp(0.0, 100.0),
            power_kw: 0.0,
            voltage_v: config.nominal_voltage_v,
            current_a: 0.0,
            temperature_c: config.idle_temperature_c,
            grid_frequency_hz: config.nominal_frequency_hz,
            alarms: AlarmFlags::default(),
            cmd_regs: [0u16; CMD_COUNT],
            config,
        };
        state.set_idle_electrical();
        state
    }

    /// Advance the simulated physics by one tick.
    ///
    /// The most recently written command register drives the operating mode and
    /// nudges the state of charge. On reaching the target SoC (charge/trickle)
    /// or the floor (discharge) the SoC clamps and holds while the commanded
    /// mode is retained. An active emergency-stop alarm halts all movement
    /// until faults are cleared.
    pub fn tick(&mut self) {
        let command = CommandType::from_register(self.cmd_regs[0]).unwrap_or(CommandType::Standby);

        // ClearFaults always takes effect, even during an active estop: clear
        // the alarm list and return to standby. Reset the command register so
        // we don't re-clear every tick.
        if command == CommandType::ClearFaults {
            self.alarms = AlarmFlags::default();
            self.mode = OperatingMode::Standby;
            self.cmd_regs = [0u16; CMD_COUNT];
            self.set_idle_electrical();
            return;
        }

        // An active estop alarm overrides any other command until faults are
        // cleared.
        if self.alarms.is_estop_active() {
            self.mode = OperatingMode::EmergencyStop;
            self.set_idle_electrical();
            return;
        }

        match command {
            CommandType::Standby => {
                self.mode = OperatingMode::Standby;
                self.set_idle_electrical();
            }
            CommandType::Charge => {
                self.mode = OperatingMode::Charging;
                let target = self.target_soc().min(self.config.soc_ceiling_percent);
                if self.soc_percent < target {
                    self.soc_percent = (self.soc_percent + self.config.charge_rate_pct).min(target);
                    self.set_active_electrical(-self.config.charge_power_kw);
                } else {
                    // Clamp & hold: stay in Charging mode but stop moving.
                    self.set_idle_electrical();
                }
            }
            CommandType::Discharge => {
                self.mode = OperatingMode::Discharging;
                let floor = self.config.soc_floor_percent;
                if self.soc_percent > floor {
                    self.soc_percent =
                        (self.soc_percent - self.config.discharge_rate_pct).max(floor);
                    self.set_active_electrical(self.config.discharge_power_kw);
                } else {
                    self.set_idle_electrical();
                }
            }
            CommandType::TrickleCharge => {
                self.mode = OperatingMode::TrickleCharge;
                let target = self.target_soc().min(self.config.soc_ceiling_percent);
                if self.soc_percent < target {
                    self.soc_percent =
                        (self.soc_percent + self.config.trickle_rate_pct).min(target);
                    // Trickle draws a fraction of full charge power.
                    self.set_active_electrical(-self.config.charge_power_kw * 0.1);
                } else {
                    self.set_idle_electrical();
                }
            }
            CommandType::EmergencyStop => {
                self.mode = OperatingMode::EmergencyStop;
                self.alarms.set_alarm_num(ESTOP_ALARM_NUM, true);
                self.set_idle_electrical();
            }
            // Handled above before the estop guard.
            CommandType::ClearFaults => unreachable!("ClearFaults handled before match"),
        }

        self.soc_percent = self.soc_percent.clamp(0.0, 100.0);
    }

    /// Target SoC requested by the command registers.
    ///
    /// A target register value of 0 means "unspecified" (the client encodes
    /// `None` as 0), in which case we charge to the configured ceiling.
    fn target_soc(&self) -> f32 {
        let raw = self.cmd_regs[1];
        if raw == 0 {
            self.config.soc_ceiling_percent
        } else {
            parse_soc(raw)
        }
    }

    /// Electrical readings while idle (no power flow).
    fn set_idle_electrical(&mut self) {
        self.power_kw = 0.0;
        self.current_a = 0.0;
        self.voltage_v = self.config.nominal_voltage_v;
        self.temperature_c = self.config.idle_temperature_c;
        self.grid_frequency_hz = self.config.nominal_frequency_hz;
    }

    /// Electrical readings while actively charging or discharging at
    /// `power_kw`.
    fn set_active_electrical(&mut self, power_kw: f32) {
        self.power_kw = power_kw;
        self.voltage_v = self.config.nominal_voltage_v;
        self.current_a = if self.voltage_v != 0.0 {
            power_kw * 1000.0 / self.voltage_v
        } else {
            0.0
        };
        self.temperature_c = self.config.idle_temperature_c + self.config.active_temperature_rise_c;
        self.grid_frequency_hz = self.config.nominal_frequency_hz;
    }

    /// Value of a single register at `addr`, or 0 for unmapped addresses.
    pub fn register_at(&self, addr: u16) -> u16 {
        let (power_high, power_low) = power_kw_to_registers(self.power_kw);
        match addr {
            RegisterMap::STATUS_MODE => self.mode.to_register(),
            RegisterMap::STATUS_SOC => soc_to_register(self.soc_percent),
            RegisterMap::STATUS_POWER_HIGH => power_high,
            RegisterMap::STATUS_POWER_LOW => power_low,
            RegisterMap::STATUS_VOLTAGE => voltage_to_register(self.voltage_v),
            RegisterMap::STATUS_CURRENT => current_to_register(self.current_a),
            RegisterMap::STATUS_TEMPERATURE => temperature_to_register(self.temperature_c),
            RegisterMap::STATUS_GRID_FREQUENCY => {
                grid_frequency_to_register(self.grid_frequency_hz)
            }
            a if (RegisterMap::STATUS_ALARMS_START
                ..RegisterMap::STATUS_ALARMS_START + ALARM_REGISTER_COUNT as u16)
                .contains(&a) =>
            {
                self.alarms.registers[(a - RegisterMap::STATUS_ALARMS_START) as usize]
            }
            a if (RegisterMap::CMD_START_ADDRESS
                ..RegisterMap::CMD_START_ADDRESS + CMD_COUNT as u16)
                .contains(&a) =>
            {
                self.cmd_regs[(a - RegisterMap::CMD_START_ADDRESS) as usize]
            }
            _ => 0,
        }
    }

    /// Read `count` consecutive registers starting at `start`.
    pub fn read_registers(&self, start: u16, count: u16) -> Vec<u16> {
        (0..count).map(|i| self.register_at(start.wrapping_add(i))).collect()
    }

    /// Write `values` to consecutive registers starting at `start`.
    ///
    /// Only the command registers (100-104) are writable; writes to the
    /// read-only status registers are ignored.
    pub fn write_registers(&mut self, start: u16, values: &[u16]) {
        for (i, &value) in values.iter().enumerate() {
            let addr = start.wrapping_add(i as u16);
            if (RegisterMap::CMD_START_ADDRESS..RegisterMap::CMD_START_ADDRESS + CMD_COUNT as u16)
                .contains(&addr)
            {
                self.cmd_regs[(addr - RegisterMap::CMD_START_ADDRESS) as usize] = value;
            }
        }
    }

    // --- Control-interface helpers (used by the stdin control loop) ---

    /// Override the state of charge directly (for testing scenarios).
    pub fn set_soc(&mut self, percent: f32) {
        self.soc_percent = percent.clamp(0.0, 100.0);
    }

    /// Set the command register, as if a command had been written over Modbus.
    pub fn set_command(&mut self, command: CommandType) {
        self.cmd_regs[0] = command.to_register();
    }

    /// Set or clear an alarm by its alarm number.
    pub fn set_alarm(&mut self, alarm_num: u16, active: bool) {
        self.alarms.set_alarm_num(alarm_num, active);
    }

    /// One-line human-readable summary of the current state.
    pub fn summary(&self) -> String {
        let active: Vec<u16> = self.alarms.active_alarms().iter().map(|d| d.alarm_num).collect();
        format!(
            "mode={} soc={:.1}% power={:.1}kW current={:.1}A temp={:.1}C freq={:.2}Hz alarms={:?}",
            self.mode,
            self.soc_percent,
            self.power_kw,
            self.current_a,
            self.temperature_c,
            self.grid_frequency_hz,
            active,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast_config() -> SimConfig {
        SimConfig {
            charge_rate_pct: 10.0,
            discharge_rate_pct: 10.0,
            ..SimConfig::default()
        }
    }

    #[test]
    fn charge_increases_soc() {
        let mut state = SimState::new(fast_config());
        state.set_soc(50.0);
        state.set_command(CommandType::Charge);
        state.tick();
        assert_eq!(state.soc_percent, 60.0);
        assert_eq!(state.mode, OperatingMode::Charging);
        assert!(state.power_kw < 0.0, "charging should report negative power");
        assert!(state.current_a < 0.0, "charging should report negative current");
    }

    #[test]
    fn discharge_decreases_soc() {
        let mut state = SimState::new(fast_config());
        state.set_soc(50.0);
        state.set_command(CommandType::Discharge);
        state.tick();
        assert_eq!(state.soc_percent, 40.0);
        assert_eq!(state.mode, OperatingMode::Discharging);
        assert!(state.power_kw > 0.0, "discharging should report positive power");
    }

    #[test]
    fn standby_holds_soc() {
        let mut state = SimState::new(fast_config());
        state.set_soc(42.0);
        state.set_command(CommandType::Standby);
        state.tick();
        assert_eq!(state.soc_percent, 42.0);
        assert_eq!(state.mode, OperatingMode::Standby);
        assert_eq!(state.power_kw, 0.0);
    }

    #[test]
    fn charge_clamps_and_holds_mode_at_target() {
        let mut config = fast_config();
        config.soc_ceiling_percent = 100.0;
        let mut state = SimState::new(config);
        state.set_soc(95.0);
        // Charge with no explicit target -> ceiling (100%).
        state.set_command(CommandType::Charge);
        state.tick(); // 95 -> 100 (clamped at ceiling)
        assert_eq!(state.soc_percent, 100.0);
        state.tick(); // already at ceiling: hold, but stay in Charging mode
        assert_eq!(state.soc_percent, 100.0);
        assert_eq!(state.mode, OperatingMode::Charging, "mode held at target");
        assert_eq!(state.power_kw, 0.0, "no power flow once clamped");
    }

    #[test]
    fn charge_respects_explicit_target() {
        let mut state = SimState::new(fast_config());
        state.set_soc(50.0);
        // Target 70% encoded into the command target register (percent * 100).
        state.cmd_regs[1] = 7000;
        state.set_command(CommandType::Charge);
        state.tick(); // 60
        state.tick(); // 70 (clamped at target)
        state.tick(); // hold
        assert_eq!(state.soc_percent, 70.0);
        assert_eq!(state.mode, OperatingMode::Charging);
    }

    #[test]
    fn discharge_clamps_at_floor() {
        let mut config = fast_config();
        config.soc_floor_percent = 10.0;
        let mut state = SimState::new(config);
        state.set_soc(15.0);
        state.set_command(CommandType::Discharge);
        state.tick(); // 15 -> 10 (clamped at floor)
        state.tick(); // hold
        assert_eq!(state.soc_percent, 10.0);
        assert_eq!(state.mode, OperatingMode::Discharging);
    }

    #[test]
    fn estop_command_sets_alarm_and_halts() {
        let mut state = SimState::new(fast_config());
        state.set_soc(50.0);
        state.set_command(CommandType::EmergencyStop);
        state.tick();
        assert!(state.alarms.is_estop_active());
        assert_eq!(state.mode, OperatingMode::EmergencyStop);

        // Even if a charge command is issued, the estop alarm halts movement.
        state.set_command(CommandType::Charge);
        state.tick();
        assert_eq!(state.soc_percent, 50.0, "estop halts SoC movement");
        assert_eq!(state.mode, OperatingMode::EmergencyStop);
    }

    #[test]
    fn injected_alarm_halts_until_cleared() {
        let mut state = SimState::new(fast_config());
        state.set_soc(50.0);
        state.set_alarm(ESTOP_ALARM_NUM, true);
        state.set_command(CommandType::Charge);
        state.tick();
        assert_eq!(state.soc_percent, 50.0);

        // ClearFaults wipes the alarm list and returns to standby.
        state.set_command(CommandType::ClearFaults);
        state.tick();
        assert!(!state.alarms.has_any_alarm());
        assert_eq!(state.mode, OperatingMode::Standby);
    }

    #[test]
    fn register_read_write_roundtrip() {
        let mut state = SimState::new(fast_config());
        state.set_soc(50.0);

        // Write a charge command to 70% via the command registers.
        state.write_registers(RegisterMap::CMD_START_ADDRESS, &[1, 7000, 0, 0, 30]);
        assert_eq!(state.register_at(RegisterMap::CMD_COMMAND), 1);
        assert_eq!(state.register_at(RegisterMap::CMD_TARGET_SOC), 7000);

        state.tick();

        // Status registers reflect the new state.
        assert_eq!(
            state.register_at(RegisterMap::STATUS_MODE),
            OperatingMode::Charging.to_register()
        );
        assert_eq!(state.register_at(RegisterMap::STATUS_SOC), 6000); // 60.0%

        // Writes to read-only status registers are ignored.
        state.write_registers(RegisterMap::STATUS_SOC, &[1234]);
        assert_eq!(state.register_at(RegisterMap::STATUS_SOC), 6000);
    }
}
