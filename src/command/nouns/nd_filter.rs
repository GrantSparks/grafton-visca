//! Typed neutral-density filter requests and inquiries.
//!
//! Each type corresponds to the method with the same name on the camera accessor.
//! Checked values are preserved until preparation applies the selected target profile.

use crate::{
    command::{inquiry_structs as inquiries, nd_filter_wire as legacy},
    request::builtin,
    types,
};

/// Reads the variable ND-filter density.
#[derive(Debug, Clone, Copy, Default)]
pub struct DensityInquiry;
crate::command::nouns::forward_inquiry!(DensityInquiry, inquiries::NdFilterInquiry; inquiries::NdFilterInquiry);

/// Reads the selected ND-filter preset.
#[derive(Debug, Clone, Copy, Default)]
pub struct PresetInquiry;
crate::command::nouns::forward_inquiry!(PresetInquiry, inquiries::NdFilterPresetInquiry; inquiries::NdFilterPresetInquiry);

/// Increases ND-filter density by one step.
#[derive(Debug, Clone, Copy, Default)]
pub struct DensityUp;
crate::command::nouns::forward_request!(DensityUp, builtin::NdFilterStepUp, Targeted, rows = [NdFilterStepUp]; builtin::NdFilterStepUp);

/// Decreases ND-filter density by one step.
#[derive(Debug, Clone, Copy, Default)]
pub struct DensityDown;
crate::command::nouns::forward_request!(DensityDown, builtin::NdFilterStepDown, Targeted, rows = [NdFilterStepDown]; builtin::NdFilterStepDown);

/// Selects preset or variable ND-filter mode.
#[derive(Debug, Clone, Copy)]
pub struct SetMode {
    mode: types::NdFilterMode,
}

impl SetMode {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::NdFilterMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::NdFilterMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetMode, legacy::NdFilterModeCommand, Plain, rows = [NdFilterMode]; |request| legacy::NdFilterModeCommand::new(request.mode));

/// Sets a direct variable ND-filter density.
#[derive(Debug, Clone, Copy)]
pub struct SetDensity {
    density: types::NdFilterDensity,
}

impl SetDensity {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(density: types::NdFilterDensity) -> Self {
        Self { density }
    }

    /// Returns the requested density.
    #[must_use]
    pub const fn density(self) -> types::NdFilterDensity {
        self.density
    }
}
crate::command::nouns::forward_request!(SetDensity, builtin::NdFilterDirect, Targeted, rows = [NdFilterDirect]; |request| builtin::NdFilterDirect::new(request.density));

/// Enables or disables automatic ND filtering.
#[derive(Debug, Clone, Copy)]
pub struct SetAuto {
    enabled: bool,
}

impl SetAuto {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Returns the requested enabled.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}
crate::command::nouns::forward_request!(SetAuto, legacy::AutoNdCommand, Plain, rows = [NdFilterAutoOn, NdFilterAutoOff]; |request| legacy::AutoNdCommand::new(request.enabled));

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Inquiry, Request};

    fn packet(request: &impl Request) -> Vec<u8> {
        let mut buffer = [0; 32];
        let written = request
            .write_into(crate::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        buffer[..written].to_vec()
    }

    #[test]
    fn every_nd_density_readback_feeds_the_density_setter() {
        for raw in 0..=types::NdFilterDensity::MAX.value() {
            let value: types::NdFilterDensity = DensityInquiry
                .decoder()
                .decode(&[0, 0, (raw >> 4) as u8, (raw & 0x0f) as u8])
                .unwrap();
            let request = SetDensity::new(value);
            assert_eq!(request.density(), value);
            assert_eq!(
                packet(&request),
                [
                    0x81,
                    1,
                    0x7e,
                    4,
                    0x42,
                    0,
                    0,
                    (raw >> 4) as u8,
                    (raw & 0x0f) as u8,
                    0xff
                ]
            );
        }
    }

    #[test]
    fn density_steps_and_auto_switch_preserve_their_wire_commands() {
        assert_eq!(packet(&DensityUp), [0x81, 1, 0x7e, 4, 0x12, 2, 0xff]);
        assert_eq!(packet(&DensityDown), [0x81, 1, 0x7e, 4, 0x12, 3, 0xff]);
        for enabled in [false, true] {
            let request = SetAuto::new(enabled);
            assert_eq!(request.enabled(), enabled);
            assert_eq!(
                packet(&request),
                [0x81, 1, 0x7e, 4, 0x53, if enabled { 2 } else { 3 }, 0xff]
            );
        }
    }
}
