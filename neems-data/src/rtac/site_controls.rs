//! Site inputs: the writable points an operator can act on.
//!
//! The read-only counterpart of this table is
//! [`alarm_definitions`](super::alarm_definitions) — every point the site
//! *reports*. This one lists every point an operator can *ask the site to
//! change*: one entry per interactable element on the single-line diagram.
//!
//! Nothing here can be written yet, and that is deliberate rather than
//! unfinished. [`SiteControl::write_register`] is `None` for every entry
//! because the client's workbook has an `Outputs` sheet that carries no rows;
//! inventing addresses for it would produce writes into registers that mean
//! something else. The API refuses a request against a control with no write
//! register and says so, which is the truthful answer until the sheet arrives.
//!
//! See `docs/site-inputs.md` for the request lifecycle these controls feed and
//! the open questions behind the empty column.

use std::fmt;

use serde::{Deserialize, Serialize};

/// What an operator can ask a control to do.
///
/// One request carries one action. Whether the site ultimately wants paired
/// momentary open/close points rather than a requested position is a question
/// for the `Outputs` sheet; that would change the wire encoding, not this
/// vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SiteControlAction {
    /// Open the switch or breaker.
    Open,
    /// Close the switch or breaker.
    Close,
    /// Trip — engage-only, used by the E-stop and the lockout relay. There is
    /// no matching reset: a latched trip is cleared on site.
    Trip,
}

impl SiteControlAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Close => "close",
            Self::Trip => "trip",
        }
    }
}

impl fmt::Display for SiteControlAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SiteControlAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Self::Open),
            "close" => Ok(Self::Close),
            "trip" => Ok(Self::Trip),
            other => Err(format!("unknown site input action: {other}")),
        }
    }
}

/// One interactable element: what it is called, what it accepts, and how the
/// site reports the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiteControl {
    /// Stable identifier, matching the React SLD component id so both sides
    /// key on the same string. This is what an API path carries.
    pub id: &'static str,
    /// Operator-facing label, as drawn on the diagram.
    pub label: &'static str,
    /// The spreadsheet's "Related SLD Object" token. Differs from `id`, and is
    /// kept so a populated `Outputs` sheet can be reconciled against this
    /// table.
    pub sld_token: &'static str,
    /// Actions this control accepts. A request for anything else is refused.
    pub actions: &'static [SiteControlAction],
    /// The read-only digital point reporting the resulting state. This — never
    /// the request — is what may drive the position drawn on the diagram.
    pub readback_alarm_num: Option<u16>,
    /// Modbus register to write the request to. `None` until the client's
    /// `Outputs` sheet defines one, which is every control today.
    pub write_register: Option<u16>,
}

impl SiteControl {
    /// Whether this control accepts `action`.
    pub fn accepts(&self, action: SiteControlAction) -> bool {
        self.actions.contains(&action)
    }

    /// Whether a request against this control could reach the site at all.
    ///
    /// False for every control today. The API checks this before leaving a
    /// request pending, so an operator is told the signal has nowhere to go
    /// rather than watching it time out.
    pub fn is_writable(&self) -> bool {
        self.write_register.is_some()
    }
}

const OPEN_CLOSE: &[SiteControlAction] = &[SiteControlAction::Open, SiteControlAction::Close];
const TRIP_ONLY: &[SiteControlAction] = &[SiteControlAction::Trip];

/// Every interactable element on the Newtown diagram.
///
/// The E-stop is deliberately absent: it is site-level rather than per-element,
/// engage-only, and already has its own table, endpoints and collector path
/// (`estop_requests`). Folding a working safety path into this one buys a
/// shared vocabulary at the cost of rewriting the one control that matters
/// most.
pub const SITE_CONTROLS: &[SiteControl] = &[
    SiteControl {
        id: "switch-89l-1",
        label: "89L-1",
        sld_token: "52-MAIN-1",
        actions: OPEN_CLOSE,
        // 101 reports *open*, not position: "not open" is inferred, so a stale
        // feed must render unknown rather than closed.
        readback_alarm_num: Some(101),
        write_register: None,
    },
    SiteControl {
        id: "switch-89l-2",
        label: "89L-2",
        sld_token: "52-MAIN-2",
        actions: OPEN_CLOSE,
        readback_alarm_num: Some(102),
        write_register: None,
    },
    SiteControl {
        id: "feeder-1a",
        label: "52-MP-1A",
        sld_token: "MP-1A",
        actions: OPEN_CLOSE,
        readback_alarm_num: Some(607),
        write_register: None,
    },
    SiteControl {
        id: "feeder-1b",
        label: "52-MP-1B",
        sld_token: "MP-1B",
        actions: OPEN_CLOSE,
        readback_alarm_num: Some(637),
        write_register: None,
    },
    SiteControl {
        id: "feeder-1c",
        label: "52-MP-1C",
        sld_token: "MP-1C",
        actions: OPEN_CLOSE,
        readback_alarm_num: Some(667),
        write_register: None,
    },
    SiteControl {
        id: "feeder-2a",
        label: "52-MP-2A",
        sld_token: "MP-2A",
        actions: OPEN_CLOSE,
        readback_alarm_num: Some(697),
        write_register: None,
    },
    SiteControl {
        id: "feeder-2b",
        label: "52-MP-2B",
        sld_token: "MP-2B",
        actions: OPEN_CLOSE,
        readback_alarm_num: Some(727),
        write_register: None,
    },
    SiteControl {
        id: "feeder-2c",
        label: "52-MP-2C",
        sld_token: "MP-2C",
        actions: OPEN_CLOSE,
        readback_alarm_num: Some(757),
        write_register: None,
    },
    SiteControl {
        id: "lockout-relay",
        label: "86-M1",
        sld_token: "LOR",
        // Trip only. Remotely *resetting* a lockout relay is a safety decision
        // the client has not asked for, and the UI keeps it disabled besides.
        actions: TRIP_ONLY,
        readback_alarm_num: Some(103),
        write_register: None,
    },
];

/// Look up a control by id. `None` for anything not in the table, which is how
/// the API rejects an unknown control rather than recording a request nothing
/// will ever act on.
pub fn site_control_by_id(id: &str) -> Option<&'static SiteControl> {
    SITE_CONTROLS.iter().find(|input| input.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtac::alarm_definitions::ALARM_DEFINITIONS;

    /// Ids are the join key with the React SLD and with the request rows in
    /// neems-api. A duplicate would make `site_control_by_id` return whichever
    /// came first and silently route one element's requests to another.
    #[test]
    fn ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for input in SITE_CONTROLS {
            assert!(seen.insert(input.id), "duplicate site input id: {}", input.id);
        }
    }

    /// A readback that names a point the site does not report is worse than
    /// none: the diagram would wait forever for a value nothing sends.
    #[test]
    fn every_readback_names_a_real_alarm() {
        for input in SITE_CONTROLS {
            let Some(num) = input.readback_alarm_num else {
                continue;
            };
            assert!(
                ALARM_DEFINITIONS.iter().any(|d| d.alarm_num == num),
                "{} reads back alarm {num}, which no definition covers",
                input.id
            );
        }
    }

    /// Every control accepts at least one action, and none accepts an action
    /// twice.
    #[test]
    fn actions_are_present_and_distinct() {
        for input in SITE_CONTROLS {
            assert!(!input.actions.is_empty(), "{} accepts no action", input.id);
            let mut seen = std::collections::HashSet::new();
            for action in input.actions {
                assert!(seen.insert(action), "{} repeats action {action}", input.id);
            }
        }
    }

    /// Pins the state this table is in, so the change that lands the `Outputs`
    /// sheet has to come past this test and update the documentation with it.
    #[test]
    fn nothing_is_writable_until_the_outputs_sheet_lands() {
        for input in SITE_CONTROLS {
            assert!(
                !input.is_writable(),
                "{} has a write register; update docs/site-inputs.md and this test",
                input.id
            );
        }
    }
}
