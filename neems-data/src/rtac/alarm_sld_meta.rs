//! SLD presentation metadata for alarms (operator message + target SLD
//! objects).
//!
//! The tables themselves are per site design, generated into each design's
//! module (for Newtown, [`crate::rtac::design::newtown::alarm_sld_meta`]); this
//! module holds the shape and the lookup, which reads the active design.
//!
//! `message` is the operator-facing mouseover text from the spreadsheet (empty
//! when the spreadsheet left it blank). `sld_targets` are the raw "Related SLD
//! Object" tokens; mapping them to concrete UI elements is the frontend's job,
//! so this module stays UI-agnostic.

/// Per-alarm SLD presentation metadata, keyed by `alarm_num`.
pub struct AlarmSldMeta {
    /// Unique alarm number (matches `AlarmDefinition::alarm_num`).
    pub alarm_num: u16,
    /// Operator-facing message; empty string when the spreadsheet had none.
    pub message: &'static str,
    /// Raw "Related SLD Object" tokens (e.g. `"Net"`, `"Border"`,
    /// `"52-MAIN-1"`).
    pub sld_targets: &'static [&'static str],
}

impl AlarmSldMeta {
    /// The message as an `Option`, treating the empty string as "no message".
    pub fn message_opt(&self) -> Option<&'static str> {
        if self.message.is_empty() {
            None
        } else {
            Some(self.message)
        }
    }
}

/// Look up SLD metadata for an alarm number.
pub fn sld_meta_for(alarm_num: u16) -> Option<&'static AlarmSldMeta> {
    super::design::active().alarm_sld_meta.iter().find(|m| m.alarm_num == alarm_num)
}
