//! Newtown's site controls. See [`crate::rtac::site_controls`] for the
//! vocabulary and `docs/site-inputs.md` for the request lifecycle.

use crate::rtac::site_controls::{EquipmentPosition, Readback, SiteControl, SiteControlAction};

const OPEN_CLOSE: &[SiteControlAction] = &[SiteControlAction::Open, SiteControlAction::Close];
const TRIP_ONLY: &[SiteControlAction] = &[SiteControlAction::Trip];

/// Every interactable element on the Newtown diagram.
///
/// The emergency shutdown is deliberately absent: it is site-level rather than
/// per-element, engage-only, and already has its own table, endpoints and
/// collector path (`emergency_shutdown_requests`). Folding a working safety
/// path into this one buys a shared vocabulary at the cost of rewriting the one
/// control that matters most.
pub const SITE_CONTROLS: &[SiteControl] = &[
    SiteControl {
        id: "switch-89l-1",
        label: "89L-1",
        sld_token: "52-MAIN-1",
        actions: OPEN_CLOSE,
        // 101 reports *open*, not position: "not open" is inferred, so a stale
        // feed must render unknown rather than closed.
        readback: Some(Readback {
            alarm_num: 101,
            active_means: EquipmentPosition::Open,
        }),
        write_register: None,
    },
    SiteControl {
        id: "switch-89l-2",
        label: "89L-2",
        sld_token: "52-MAIN-2",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 102,
            active_means: EquipmentPosition::Open,
        }),
        write_register: None,
    },
    SiteControl {
        id: "feeder-1a",
        label: "52-MP-1A",
        sld_token: "MP-1A",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 607,
            active_means: EquipmentPosition::Closed,
        }),
        write_register: None,
    },
    SiteControl {
        id: "feeder-1b",
        label: "52-MP-1B",
        sld_token: "MP-1B",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 637,
            active_means: EquipmentPosition::Closed,
        }),
        write_register: None,
    },
    SiteControl {
        id: "feeder-1c",
        label: "52-MP-1C",
        sld_token: "MP-1C",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 667,
            active_means: EquipmentPosition::Closed,
        }),
        write_register: None,
    },
    SiteControl {
        id: "feeder-2a",
        label: "52-MP-2A",
        sld_token: "MP-2A",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 697,
            active_means: EquipmentPosition::Closed,
        }),
        write_register: None,
    },
    SiteControl {
        id: "feeder-2b",
        label: "52-MP-2B",
        sld_token: "MP-2B",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 727,
            active_means: EquipmentPosition::Closed,
        }),
        write_register: None,
    },
    SiteControl {
        id: "feeder-2c",
        label: "52-MP-2C",
        sld_token: "MP-2C",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 757,
            active_means: EquipmentPosition::Closed,
        }),
        write_register: None,
    },
    SiteControl {
        id: "lockout-relay",
        label: "86-M1",
        sld_token: "LOR",
        // Trip only. Remotely *resetting* a lockout relay is a safety decision
        // the client has not asked for, and the UI keeps it disabled besides.
        actions: TRIP_ONLY,
        readback: Some(Readback {
            alarm_num: 103,
            active_means: EquipmentPosition::Open,
        }),
        write_register: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

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
