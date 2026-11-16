//! Tally indication, brightness and readback requests.

use crate::command::nouns::{forward_inquiry, forward_request};
#[cfg(test)]
use crate::command::semantics::BuiltinCommand;
use crate::types::{TallyBrightness, TallyMode};
#[cfg(test)]
use crate::{CameraId, Request};

/// Reads the red tally indication.
#[derive(Debug, Clone, Copy)]
pub struct RedInquiry;
forward_inquiry!(RedInquiry, crate::command::TallyRedInquiry; crate::command::TallyRedInquiry);

/// Reads the green tally indication.
#[derive(Debug, Clone, Copy)]
pub struct GreenInquiry;
forward_inquiry!(GreenInquiry, crate::command::TallyGreenInquiry; crate::command::TallyGreenInquiry);

/// Enables or disables the red tally indication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetRed {
    enabled: bool,
}
impl SetRed {
    /// Creates the checked tally setting.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}
forward_request!(SetRed, crate::command::TallyRedOn, Inherited;
    select |request, target| (request.enabled) default (request.enabled) {
        true => (crate::command::TallyRedOn, TallyRedOn, crate::command::TallyRedOn::new()),
        false => (crate::command::TallyRedOff, TallyRedOff, crate::command::TallyRedOff::new()),
    }
);

/// Enables or disables the green tally indication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetGreen {
    enabled: bool,
}
impl SetGreen {
    /// Creates the checked tally setting.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}
forward_request!(SetGreen, crate::command::TallyGreenOn, Inherited;
    select |request, target| (request.enabled) default (request.enabled) {
        true => (crate::command::TallyGreenOn, TallyGreenOn, crate::command::TallyGreenOn::new()),
        false => (crate::command::TallyGreenOff, TallyGreenOff, crate::command::TallyGreenOff::new()),
    }
);

/// Selects low or high tally brightness without changing its documented bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetBrightness {
    brightness: TallyBrightness,
}
impl SetBrightness {
    /// Creates the checked tally setting.
    #[must_use]
    pub const fn new(brightness: TallyBrightness) -> Self {
        Self { brightness }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn brightness(self) -> TallyBrightness {
        self.brightness
    }
}
forward_request!(SetBrightness, crate::command::TallyBrightLo, Inherited;
    select |request, target| (request.brightness) default (request.brightness) {
        TallyBrightness::Low => (crate::command::TallyBrightLo, TallyBrightLow, crate::command::TallyBrightLo::new()),
        TallyBrightness::High => (crate::command::TallyBrightHi, TallyBrightHigh, crate::command::TallyBrightHi::new()),
    }
);

/// Selects a steady, flashing or disabled tally indication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMode {
    mode: TallyMode,
}
impl SetMode {
    /// Creates the checked tally setting.
    #[must_use]
    pub const fn new(mode: TallyMode) -> Self {
        Self { mode }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn mode(self) -> TallyMode {
        self.mode
    }
}
forward_request!(SetMode, crate::command::TallyOn, Inherited;
    select |request, target| (request.mode) default (request.mode) {
        TallyMode::Off => (crate::command::TallyOff, TallyOff, crate::command::TallyOff::new()),
        TallyMode::On => (crate::command::TallyOn, TallyOn, crate::command::TallyOn::new()),
        TallyMode::Flash => (crate::command::TallyFlash, TallyFlash, crate::command::TallyFlash::new()),
    }
);

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn wire(request: &impl Request) -> Vec<u8> {
        let mut bytes = [0; 16];
        let written = request
            .write_into(CameraId::CAMERA_1, &mut bytes)
            .expect("tally frame");
        bytes[..written].to_vec()
    }

    #[test]
    fn merged_tally_setters_preserve_every_literal_packet_and_selected_row() {
        for (enabled, code, red, green) in [
            (
                true,
                0x02,
                BuiltinCommand::TallyRedOn,
                BuiltinCommand::TallyGreenOn,
            ),
            (
                false,
                0x03,
                BuiltinCommand::TallyRedOff,
                BuiltinCommand::TallyGreenOff,
            ),
        ] {
            let red_request = SetRed::new(enabled);
            let green_request = SetGreen::new(enabled);
            assert_eq!(red_request.ledger_row(), red);
            assert_eq!(green_request.ledger_row(), green);
            assert_eq!(
                wire(&red_request),
                vec![0x81, 1, 0x7E, 1, 0x0A, 0, code, 0xFF]
            );
            assert_eq!(
                wire(&green_request),
                vec![0x81, 1, 0x7E, 4, 0x1A, 0, code, 0xFF]
            );
        }
        for (brightness, code) in [(TallyBrightness::Low, 4), (TallyBrightness::High, 5)] {
            assert_eq!(
                wire(&SetBrightness::new(brightness)),
                vec![0x81, 1, 0x7E, 1, 0x0A, 1, code, 0xFF]
            );
        }
        for (mode, code, row) in [
            (TallyMode::Off, 3, BuiltinCommand::TallyOff),
            (TallyMode::On, 2, BuiltinCommand::TallyOn),
            (TallyMode::Flash, 1, BuiltinCommand::TallyFlash),
        ] {
            let request = SetMode::new(mode);
            assert_eq!(request.ledger_row(), row);
            assert_eq!(wire(&request), vec![0x81, 0x0A, 2, 2, code, 0xFF]);
        }
    }
    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn merged_tally_setters_project_the_same_typed_setting_including_flash() {
        use crate::runtime::engine::AppliedStateProjection;
        use crate::{requests::AppliedStateAuthority, KnownState};
        for mode in [TallyMode::Off, TallyMode::On, TallyMode::Flash] {
            assert_eq!(
                SetMode::new(mode).applied_state_projection(AppliedStateAuthority::new()),
                Some(AppliedStateProjection::set(KnownState::TallyMode(mode)))
            );
        }
        for brightness in [TallyBrightness::Low, TallyBrightness::High] {
            assert_eq!(
                SetBrightness::new(brightness)
                    .applied_state_projection(AppliedStateAuthority::new()),
                Some(AppliedStateProjection::set(KnownState::TallyBrightness(
                    brightness
                )))
            );
        }
    }
}
