//! Site designs: everything that differs from one site to the next.
//!
//! A design is a site's alarm matrix, the SLD metadata that goes with it, its
//! analog points and its operator controls, plus the few alarm numbers the
//! rest of the system has to single out. Each design lives in its own module
//! (for now only [`newtown`]) and is registered in [`DESIGNS`].
//!
//! A deployment runs exactly one design, chosen at startup by
//! [`NEEMS_SITE_DESIGN`](ENV_VAR) and fixed for the life of the process.
//! Everything reads it through [`active`]. It is per deployment rather than per
//! site because stored history — raw alarm bitfields, `alarm_state` rows,
//! acknowledgements — is keyed by alarm number and only means anything under
//! the design that wrote it. neems-api, neems-data and neems-rtac-sim share
//! that history, so all three must be started with the same design.
//!
//! Not everything site-specific has moved here yet. The zone set is still the
//! closed [`AlarmZone`](super::alarm_definitions::AlarmZone) enum, and the
//! alarm register count and Megapack block layout in
//! [`protocol`](super::protocol) still size fixed arrays. A design that
//! differs in those needs them to become per-design as well.

use std::{fmt, sync::OnceLock};

use super::{
    alarm_definitions::AlarmDefinition, alarm_sld_meta::AlarmSldMeta, analog_points::AnalogPoint,
    protocol::MP_ANALOG_POINT_COUNT, site_controls::SiteControl,
};

pub mod newtown;

/// The environment variable that selects the design.
pub const ENV_VAR: &str = "NEEMS_SITE_DESIGN";

/// The design used when [`ENV_VAR`] is unset or empty.
pub const DEFAULT_ID: &str = "newtown";

/// Every design this build knows about.
pub const DESIGNS: &[&SiteDesign] = &[&newtown::DESIGN];

/// One site's design.
pub struct SiteDesign {
    /// Stable identifier, as set in [`ENV_VAR`] and reported to clients.
    pub id: &'static str,
    /// Every digital alarm the site reports, in alarm-number order.
    pub alarm_definitions: &'static [AlarmDefinition],
    /// Operator message and SLD targets per alarm number.
    pub alarm_sld_meta: &'static [AlarmSldMeta],
    /// Every analog measurement the site reports, in point-number order.
    pub analog_points: &'static [AnalogPoint],
    /// Per-Megapack measurement names, in block order. Sized by the Megapack
    /// block, so a design with a name missing does not compile — a short list
    /// would otherwise silently drop measurements from every response.
    pub megapack_analog_names: &'static [&'static str; MP_ANALOG_POINT_COUNT],
    /// Every interactable element on the site's diagram.
    pub site_controls: &'static [SiteControl],
    /// The alarm that reports the site's physical E-stop.
    pub estop_alarm_num: u16,
}

impl fmt::Debug for SiteDesign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SiteDesign").field("id", &self.id).finish_non_exhaustive()
    }
}

/// Why a design could not be selected.
#[derive(Clone, PartialEq, Eq)]
pub enum DesignError {
    /// No registered design has this id.
    Unknown(String),
    /// A different design was already selected for this process.
    AlreadySelected {
        active: &'static str,
        requested: &'static str,
    },
}

impl fmt::Display for DesignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => {
                let known: Vec<_> = DESIGNS.iter().map(|d| d.id).collect();
                write!(f, "{ENV_VAR}={id:?} names no site design (known: {})", known.join(", "))
            }
            Self::AlreadySelected { active, requested } => write!(
                f,
                "site design {active:?} is already active; cannot switch to {requested:?}"
            ),
        }
    }
}

/// The message, not the variant: a binary's `main` returning this prints its
/// `Debug`, and someone who mistyped the design needs to read which ones exist.
impl fmt::Debug for DesignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for DesignError {}

static ACTIVE: OnceLock<&'static SiteDesign> = OnceLock::new();

/// Look up a registered design by id.
pub fn by_id(id: &str) -> Option<&'static SiteDesign> {
    DESIGNS.iter().copied().find(|d| d.id == id)
}

/// The design id an [`ENV_VAR`] value asks for: the trimmed value, or
/// [`DEFAULT_ID`] when it is unset or blank.
pub fn id_from_env_value(value: Option<&str>) -> &str {
    match value.map(str::trim) {
        Some(id) if !id.is_empty() => id,
        _ => DEFAULT_ID,
    }
}

/// Fix the process's design.
///
/// Idempotent for the design already active. Asking for a different one is an
/// error rather than a switch: code that has already read the old design's
/// tables would otherwise be mixing two sites.
pub fn select(id: &str) -> Result<&'static SiteDesign, DesignError> {
    let requested = by_id(id).ok_or_else(|| DesignError::Unknown(id.to_string()))?;
    let active = *ACTIVE.get_or_init(|| requested);
    if active.id == requested.id {
        Ok(active)
    } else {
        Err(DesignError::AlreadySelected {
            active: active.id,
            requested: requested.id,
        })
    }
}

/// Fix the process's design from [`ENV_VAR`].
///
/// Call once at startup, so a mistyped design fails the launch with a clear
/// message instead of panicking in whatever first reads a table.
pub fn select_from_env() -> Result<&'static SiteDesign, DesignError> {
    let value = std::env::var(ENV_VAR).ok();
    select(id_from_env_value(value.as_deref()))
}

/// The process's design.
///
/// Selected from [`ENV_VAR`] on first use if nothing selected it earlier.
///
/// # Panics
///
/// If [`ENV_VAR`] names no design and [`select_from_env`] was not called
/// first to report that properly.
pub fn active() -> &'static SiteDesign {
    if let Some(design) = ACTIVE.get() {
        return design;
    }
    // The same resolution the binaries run at startup, so a process that
    // reaches a table first cannot pick a different design from one that
    // selected explicitly.
    select_from_env().unwrap_or_else(|e| panic!("{e}"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::rtac::alarm_definitions::ALARM_REGISTER_COUNT;

    #[test]
    fn ids_are_unique() {
        let mut seen = HashSet::new();
        for design in DESIGNS {
            assert!(seen.insert(design.id), "duplicate design id {}", design.id);
        }
    }

    #[test]
    fn the_default_design_exists() {
        assert!(by_id(DEFAULT_ID).is_some());
    }

    #[test]
    fn an_unset_or_blank_variable_means_the_default() {
        assert_eq!(id_from_env_value(None), DEFAULT_ID);
        assert_eq!(id_from_env_value(Some("")), DEFAULT_ID);
        assert_eq!(id_from_env_value(Some("  ")), DEFAULT_ID);
        assert_eq!(id_from_env_value(Some(" newtown ")), "newtown");
    }

    #[test]
    fn an_unknown_design_is_an_error_not_a_fallback() {
        assert_eq!(select("atlantis").unwrap_err(), DesignError::Unknown("atlantis".into()));
    }

    // The invariants below hold for every design, so a new design is checked
    // by being registered.

    #[test]
    fn every_definition_has_sld_meta() {
        for design in DESIGNS {
            for def in design.alarm_definitions {
                assert!(
                    design.alarm_sld_meta.iter().any(|m| m.alarm_num == def.alarm_num),
                    "{}: alarm {} ({}) has no SLD metadata entry",
                    design.id,
                    def.alarm_num,
                    def.name,
                );
            }
        }
    }

    #[test]
    fn alarm_numbers_and_addresses_are_unique() {
        for design in DESIGNS {
            let mut nums = HashSet::new();
            let mut addresses = HashSet::new();
            for def in design.alarm_definitions {
                assert!(
                    nums.insert(def.alarm_num),
                    "{}: duplicate alarm {}",
                    design.id,
                    def.alarm_num
                );
                // Uniqueness of the number is not enough on its own: this is
                // the property that matters on the wire, so that if the point
                // numbering base ever becomes non-injective the failure lands
                // here rather than on two zones silently sharing a bit.
                assert!(
                    addresses.insert(def.discrete_address()),
                    "{}: discrete address {} assigned twice ({})",
                    design.id,
                    def.discrete_address(),
                    def.qualified_name()
                );
                assert_eq!(
                    def.discrete_address(),
                    def.alarm_num,
                    "{}: {} address drifted from its alarm number",
                    design.id,
                    def.qualified_name()
                );
            }
        }
    }

    #[test]
    fn register_positions_are_valid_and_unique() {
        for design in DESIGNS {
            let mut seen = HashSet::new();
            for def in design.alarm_definitions {
                assert!(
                    def.register_index < ALARM_REGISTER_COUNT && def.bit < 16,
                    "{}: alarm {} ({}) is at register {} bit {}, outside the alarm block",
                    design.id,
                    def.alarm_num,
                    def.name,
                    def.register_index,
                    def.bit,
                );
                assert!(
                    seen.insert((def.register_index, def.bit)),
                    "{}: duplicate bit position: alarm {} at register {} bit {}",
                    design.id,
                    def.alarm_num,
                    def.register_index,
                    def.bit,
                );
            }
        }
    }

    #[test]
    fn levels_are_valid() {
        for design in DESIGNS {
            for def in design.alarm_definitions {
                assert!(
                    (1..=5).contains(&def.level),
                    "{}: alarm {} ({}) has invalid level {}",
                    design.id,
                    def.alarm_num,
                    def.name,
                    def.level,
                );
            }
        }
    }

    #[test]
    fn the_estop_alarm_is_defined() {
        for design in DESIGNS {
            assert!(
                design.alarm_definitions.iter().any(|d| d.alarm_num == design.estop_alarm_num),
                "{}: E-stop alarm {} is not defined",
                design.id,
                design.estop_alarm_num
            );
        }
    }

    /// Ids are the join key with the React SLD and with the request rows in
    /// neems-api. A duplicate would make `site_control_by_id` return whichever
    /// came first and silently route one element's requests to another.
    #[test]
    fn control_ids_are_unique() {
        for design in DESIGNS {
            let mut seen = HashSet::new();
            for input in design.site_controls {
                assert!(seen.insert(input.id), "{}: duplicate control id {}", design.id, input.id);
            }
        }
    }

    /// A readback that names a point the site does not report is worse than
    /// none: the diagram would wait forever for a value nothing sends.
    #[test]
    fn every_readback_names_a_real_alarm() {
        for design in DESIGNS {
            for input in design.site_controls {
                let Some(num) = input.readback_alarm_num() else {
                    continue;
                };
                assert!(
                    design.alarm_definitions.iter().any(|d| d.alarm_num == num),
                    "{}: {} reads back alarm {num}, which no definition covers",
                    design.id,
                    input.id
                );
            }
        }
    }

    /// Every control accepts at least one action, and none accepts an action
    /// twice.
    #[test]
    fn control_actions_are_present_and_distinct() {
        for design in DESIGNS {
            for input in design.site_controls {
                assert!(!input.actions.is_empty(), "{}: {} accepts no action", design.id, input.id);
                let mut seen = HashSet::new();
                for action in input.actions {
                    assert!(
                        seen.insert(action),
                        "{}: {} repeats action {action}",
                        design.id,
                        input.id
                    );
                }
            }
        }
    }
}
