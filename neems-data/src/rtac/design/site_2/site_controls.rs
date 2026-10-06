//! The second site's controls. See [`crate::rtac::site_controls`] for the
//! vocabulary and `docs/site-inputs.md` for the request lifecycle.

use crate::rtac::site_controls::{EquipmentPosition, Readback, SiteControl, SiteControlAction};

const OPEN_CLOSE: &[SiteControlAction] = &[SiteControlAction::Open, SiteControlAction::Close];

/// Every interactable element on the second site's diagram.
///
/// The readbacks are placeholders: this site's points are not specified yet,
/// so each medium-voltage switch reads the borrowed Newtown point for the line
/// switch in the same position (ahead of transformer 1 and 2). The main
/// breaker 52-R1 is left out until it has a point of its own to read.
pub const SITE_CONTROLS: &[SiteControl] = &[
    SiteControl {
        id: "switch-mvsw-1",
        label: "52-MVSW-1",
        sld_token: "52-MVSW-1",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 101,
            active_means: EquipmentPosition::Open,
        }),
        write_register: None,
    },
    SiteControl {
        id: "switch-mvsw-2",
        label: "52-MVSW-2",
        sld_token: "52-MVSW-2",
        actions: OPEN_CLOSE,
        readback: Some(Readback {
            alarm_num: 102,
            active_means: EquipmentPosition::Open,
        }),
        write_register: None,
    },
];
