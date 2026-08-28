//! Synthesis of plausible Megapack analog blocks.
//!
//! Shared by the Modbus simulator and the demo history seeder so the two
//! cannot drift: a frontend built against `neems-rtac-sim` sees the same
//! shapes and magnitudes as one built against seeded history, and a change to
//! either lands in both.
//!
//! Values are written through [`MP_ANALOG_ENCODING`], the same table the API
//! decodes with, rather than as decoded numbers. A mistake in that table
//! therefore shows up as a wrong number on the demo instead of cancelling
//! itself out between a writer and a reader that share the error.
//!
//! Everything here is deterministic in its inputs. Nothing is random, because
//! a demo has to replay identically and a screenshot taken twice has to match.

use super::protocol::{MEGAPACK_ZONES, MP_ANALOG_ENCODING, MP_ANALOG_POINT_COUNT};

/// Nameplate energy of one Megapack, kWh.
pub const PACK_ENERGY_KWH: f32 = 3900.0;
/// Nameplate power of one Megapack, kW.
pub const PACK_POWER_KW: f32 = 1900.0;

/// How far pack `pack_index` sits from the site-level figure.
///
/// Spans -2.5..=2.5 so the six packs straddle the site value rather than all
/// sitting to one side of it. Six identical gauges would hide the one thing
/// per-pack readings exist to show.
pub fn pack_spread(pack_index: usize) -> f32 {
    pack_index as f32 - 2.5
}

/// Build one pack's 30-register analog block.
///
/// `power_kw` is that pack's share of site power, signed: negative while
/// charging, because the sign is what makes a charging demo read as charging
/// rather than merely move.
///
/// The spare points stay zero. The client reserved them and defined nothing,
/// and a demo inventing values for them would be inventing a spec.
pub fn synthesize_megapack_block(
    pack_index: usize,
    soc_percent: f32,
    power_kw: f32,
    voltage_v: f32,
    ambient_c: f32,
) -> [u16; MP_ANALOG_POINT_COUNT] {
    let spread = pack_spread(pack_index);
    let soc = soc_percent.clamp(0.0, 100.0);
    // Proportional, not additive. An offset of +/-20 kW per pack reads fine
    // while the site is working, but around zero it straddles the sign: at
    // standby the six packs came out -50, -30, -10, +10, +30, +50 kW, so the
    // diagram showed three packs charging and three discharging while the
    // site did nothing. Scaling instead means an idle site leaves every pack
    // idle, and a working one still spreads them.
    let power = power_kw * (1.0 + spread * 0.02);

    // Three-phase: I = P / (V * sqrt(3)).
    let current_a = if voltage_v != 0.0 {
        power * 1000.0 / (voltage_v * 1.732)
    } else {
        0.0
    };
    // A working pack runs hotter, and each pack sits slightly differently.
    let battery_c = ambient_c + 4.0 + (power.abs() / PACK_POWER_KW) * 8.0 + spread * 0.4;
    let energy_remaining = PACK_ENERGY_KWH * soc / 100.0;
    let volts = voltage_v + spread * 0.6;

    let mut regs = [0u16; MP_ANALOG_POINT_COUNT];
    let mut set = |offset: usize, value: f32| {
        regs[offset] = MP_ANALOG_ENCODING[offset].encode(value);
    };

    set(0, power); // real_power_target
    set(1, power); // real_power_output
    set(2, power * 0.05); // reactive_power_target
    set(3, power * 0.05); // reactive_power_output
    set(4, soc); // state_of_energy
    set(5, energy_remaining); // energy_remaining
    set(6, PACK_ENERGY_KWH - energy_remaining); // energy_to_full_SOC
    set(7, PACK_ENERGY_KWH); // full_pack_energy
    set(8, PACK_ENERGY_KWH); // nominal_full_pack
    set(9, 60.0 + spread * 0.004); // frequency
    set(10, volts); // ac_voltage
    set(11, volts + 0.8); // ac_voltage_phaseA
    set(12, volts); // ac_voltage_phaseB
    set(13, volts - 0.8); // ac_voltage_phaseC
    set(14, current_a); // inverter_phaseA_current
    set(15, current_a * 0.99); // inverter_phaseB_current
    set(16, current_a * 1.01); // inverter_phaseC_current
    set(18, battery_c); // max_battery_temperature
    set(19, ambient_c); // ambient_temperature
    // Headroom shrinks as the pack fills or empties, which is what makes these
    // worth plotting rather than flat lines.
    set(20, PACK_POWER_KW * (1.0 - soc / 100.0).max(0.05)); // available_charge_power
    set(21, PACK_POWER_KW * (soc / 100.0).max(0.05)); // available_discharge_power
    set(22, PACK_POWER_KW); // nominal_charge_power
    set(23, PACK_POWER_KW); // nominal_discharge_power

    regs
}

/// A slow daily temperature cycle, for callers with no ambient of their own.
///
/// Driven by absolute time rather than a stored phase so any two callers at
/// the same instant agree.
pub fn ambient_at(unix_seconds: i64) -> f32 {
    let hours = unix_seconds as f32 / 3600.0;
    18.0 + 6.0 * (hours * std::f32::consts::TAU / 24.0).sin()
}

/// Every pack's block for one moment, paired with its zone code.
///
/// `pack_power_kw` is per pack, not the site total: it reaches each pack's
/// block unchanged, give or take that pack's spread. Handing this the site
/// figure would report every pack at six times its real output.
pub fn synthesize_all_packs(
    soc_percent: f32,
    pack_power_kw: f32,
    voltage_v: f32,
    ambient_c: f32,
) -> Vec<(&'static str, [u16; MP_ANALOG_POINT_COUNT])> {
    MEGAPACK_ZONES
        .iter()
        .enumerate()
        .map(|(i, zone)| {
            let soc = soc_percent + pack_spread(i) * 1.5;
            (
                zone.code(),
                synthesize_megapack_block(i, soc, pack_power_kw, voltage_v, ambient_c),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtac::protocol::mp_analog_offset;

    fn decode(regs: &[u16; MP_ANALOG_POINT_COUNT], offset: usize) -> f32 {
        MP_ANALOG_ENCODING[offset].decode(regs[offset])
    }

    #[test]
    fn every_pack_reports_a_distinct_charge_level() {
        let packs = synthesize_all_packs(50.0, 0.0, 480.0, 20.0);
        assert_eq!(packs.len(), MEGAPACK_ZONES.len());
        let socs: Vec<f32> = packs
            .iter()
            .map(|(_, r)| decode(r, mp_analog_offset::STATE_OF_ENERGY as usize))
            .collect();
        for w in socs.windows(2) {
            assert_ne!(w[0], w[1]);
        }
        for soc in socs {
            assert!((0.0..=100.0).contains(&soc), "SoC {} out of range", soc);
        }
    }

    #[test]
    fn charge_level_stays_in_range_at_the_extremes() {
        // The per-pack spread pushes packs either side of the site figure, so
        // a full or empty site must not push any pack past the rails.
        for level in [0.0, 1.0, 99.0, 100.0] {
            for (_, regs) in synthesize_all_packs(level, 0.0, 480.0, 20.0) {
                let soc = decode(&regs, mp_analog_offset::STATE_OF_ENERGY as usize);
                assert!((0.0..=100.0).contains(&soc), "SoC {} out of range at {}", soc, level);
            }
        }
    }

    #[test]
    fn an_idle_site_leaves_every_pack_idle() {
        // The per-pack spread must not straddle zero. An additive offset put
        // three packs charging and three discharging while the site was in
        // standby, which reads as the packs shuttling power between each
        // other -- and "idle" is what the SLD prints for exactly 0.
        for (_, regs) in synthesize_all_packs(50.0, 0.0, 480.0, 20.0) {
            assert_eq!(decode(&regs, 1), 0.0, "an idle site produced pack power");
            assert_eq!(decode(&regs, 14), 0.0, "an idle site produced pack current");
        }
    }

    #[test]
    fn a_working_site_still_spreads_the_packs() {
        // Losing the straddle must not cost the spread: six identical power
        // readouts would hide what per-pack values exist to show.
        let powers: Vec<f32> = synthesize_all_packs(50.0, -1000.0, 480.0, 20.0)
            .iter()
            .map(|(_, r)| decode(r, 1))
            .collect();
        for w in powers.windows(2) {
            assert_ne!(w[0], w[1]);
        }
        // ...and every pack keeps the site's sign.
        for p in &powers {
            assert!(*p < 0.0, "pack power {} flipped against a charging site", p);
        }
    }

    #[test]
    fn power_keeps_its_sign_and_current_follows_it() {
        let charging = synthesize_megapack_block(0, 50.0, -1000.0, 480.0, 20.0);
        assert!(decode(&charging, 1) < 0.0, "charging power should be negative");
        assert!(decode(&charging, 14) < 0.0, "charging current should be negative");

        let discharging = synthesize_megapack_block(0, 50.0, 1000.0, 480.0, 20.0);
        assert!(decode(&discharging, 1) > 0.0);
        assert!(decode(&discharging, 14) > 0.0);
    }

    #[test]
    fn spare_points_stay_zero() {
        let regs = synthesize_megapack_block(2, 60.0, 500.0, 480.0, 20.0);
        for offset in [17, 24, 25, 26, 27, 28, 29] {
            assert_eq!(regs[offset], 0, "spare offset {} was given a value", offset);
        }
    }

    #[test]
    fn a_working_pack_runs_hotter_than_an_idle_one() {
        let idle = synthesize_megapack_block(0, 50.0, 0.0, 480.0, 20.0);
        let busy = synthesize_megapack_block(0, 50.0, 1800.0, 480.0, 20.0);
        assert!(decode(&busy, 18) > decode(&idle, 18));
        // Ambient is the same either way; only the pack heats up.
        assert_eq!(decode(&busy, 19), decode(&idle, 19));
    }

    #[test]
    fn energy_remaining_tracks_charge_level() {
        let full = synthesize_megapack_block(0, 100.0, 0.0, 480.0, 20.0);
        assert!((decode(&full, 5) - PACK_ENERGY_KWH).abs() < 1.0);
        assert!(decode(&full, 6) < 1.0, "a full pack needs no energy to fill");

        let empty = synthesize_megapack_block(0, 0.0, 0.0, 480.0, 20.0);
        assert!(decode(&empty, 5) < 1.0);
        assert!((decode(&empty, 6) - PACK_ENERGY_KWH).abs() < 1.0);
    }

    #[test]
    fn output_is_deterministic() {
        let a = synthesize_megapack_block(3, 61.0, -400.0, 480.0, 21.5);
        let b = synthesize_megapack_block(3, 61.0, -400.0, 480.0, 21.5);
        assert_eq!(a, b);
    }
}
