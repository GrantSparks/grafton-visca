//! Typed exposure requests and inquiries.
//!
//! Each type corresponds to the method with the same name on the camera accessor.
//! Checked values are preserved until preparation applies the selected target profile.

use crate::{
    command::{
        exposure_wire as legacy, gain, image_wire, inquiry_structs as inquiries,
        sony_fr7_exposure as fr7,
    },
    request::builtin,
    types,
};

/// Reads the active exposure mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModeInquiry;
crate::command::nouns::forward_inquiry!(ModeInquiry, inquiries::ExposureModeInquiry; inquiries::ExposureModeInquiry);

/// Reads the shutter speed.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutterInquiry;
crate::command::nouns::forward_inquiry!(ShutterInquiry, inquiries::ShutterInquiry; inquiries::ShutterInquiry);

/// Reads the exposure-compensation level.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompensationInquiry;
crate::command::nouns::forward_inquiry!(CompensationInquiry, inquiries::ExposureCompensationInquiry; inquiries::ExposureCompensationInquiry);

/// Reads whether exposure compensation is enabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompensationEnabledInquiry;
crate::command::nouns::forward_inquiry!(CompensationEnabledInquiry, inquiries::ExposureCompensationModeInquiry; inquiries::ExposureCompensationModeInquiry);

/// Reads the iris level.
#[derive(Debug, Clone, Copy, Default)]
pub struct IrisInquiry;
crate::command::nouns::forward_inquiry!(IrisInquiry, inquiries::IrisInquiry; inquiries::IrisInquiry);

/// Reads the exposure-brightness level.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrightnessInquiry;
crate::command::nouns::forward_inquiry!(BrightnessInquiry, inquiries::BrightnessInquiry; inquiries::BrightnessInquiry);

/// Reads the gain level.
#[derive(Debug, Clone, Copy, Default)]
pub struct GainInquiry;
crate::command::nouns::forward_inquiry!(GainInquiry, inquiries::GainInquiry; inquiries::GainInquiry);

/// Reads the automatic-gain limit.
#[derive(Debug, Clone, Copy, Default)]
pub struct GainLimitInquiry;
crate::command::nouns::forward_inquiry!(GainLimitInquiry, inquiries::GainLimitInquiry; inquiries::GainLimitInquiry);

/// Reads the anti-flicker mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct AntiFlickerInquiry;
crate::command::nouns::forward_inquiry!(AntiFlickerInquiry, inquiries::FlickerModeInquiry; inquiries::FlickerModeInquiry);

/// Reads the AE adjustment in quarter-EV steps.
#[derive(Debug, Clone, Copy, Default)]
pub struct AeLevelInquiry;
crate::command::nouns::forward_inquiry!(AeLevelInquiry, inquiries::Fr7AeLevelInquiry; inquiries::Fr7AeLevelInquiry);

/// Reads whether automatic gain is enabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoGainInquiry;
crate::command::nouns::forward_inquiry!(AutoGainInquiry, inquiries::Fr7AutoGainInquiry; inquiries::Fr7AutoGainInquiry);

/// Reads whether automatic shutter is enabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoShutterInquiry;
crate::command::nouns::forward_inquiry!(AutoShutterInquiry, inquiries::Fr7AutoShutterInquiry; inquiries::Fr7AutoShutterInquiry);

/// Reads whether automatic iris is enabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoIrisInquiry;
crate::command::nouns::forward_inquiry!(AutoIrisInquiry, inquiries::Fr7AutoIrisInquiry; inquiries::Fr7AutoIrisInquiry);

/// Reads whether backlight compensation is enabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct BacklightCompensationInquiry;
crate::command::nouns::forward_inquiry!(BacklightCompensationInquiry, inquiries::BacklightInquiry; inquiries::BacklightInquiry);

/// Restores the camera default for shutter.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutterReset;
crate::command::nouns::forward_request!(ShutterReset, legacy::Shutter, Plain, rows = [ShutterReset]; legacy::Shutter::Reset);

/// Increases shutter by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutterUp;
crate::command::nouns::forward_request!(ShutterUp, legacy::Shutter, Plain, rows = [ShutterUp]; legacy::Shutter::Up);

/// Decreases shutter by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutterDown;
crate::command::nouns::forward_request!(ShutterDown, legacy::Shutter, Plain, rows = [ShutterDown]; legacy::Shutter::Down);

/// Restores the camera default for compensation.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompensationReset;
crate::command::nouns::forward_request!(CompensationReset, legacy::ExposureCompensation, Plain, rows = [ExposureCompensationReset]; legacy::ExposureCompensation::Reset);

/// Increases compensation by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompensationUp;
crate::command::nouns::forward_request!(CompensationUp, legacy::ExposureCompensation, Plain, rows = [ExposureCompensationUp]; legacy::ExposureCompensation::Up);

/// Decreases compensation by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompensationDown;
crate::command::nouns::forward_request!(CompensationDown, legacy::ExposureCompensation, Plain, rows = [ExposureCompensationDown]; legacy::ExposureCompensation::Down);

/// Restores the camera default for brightness.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrightnessReset;
crate::command::nouns::forward_request!(BrightnessReset, legacy::Brightness, Plain, rows = [BrightnessReset]; legacy::Brightness::Reset);

/// Increases brightness by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrightnessUp;
crate::command::nouns::forward_request!(BrightnessUp, legacy::Brightness, Plain, rows = [BrightnessUp]; legacy::Brightness::Up);

/// Decreases brightness by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrightnessDown;
crate::command::nouns::forward_request!(BrightnessDown, legacy::Brightness, Plain, rows = [BrightnessDown]; legacy::Brightness::Down);

/// Restores the camera default for gain.
#[derive(Debug, Clone, Copy, Default)]
pub struct GainReset;
crate::command::nouns::forward_request!(GainReset, gain::Gain, Plain, rows = [GainReset]; gain::Gain::Reset);

/// Increases gain by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct GainUp;
crate::command::nouns::forward_request!(GainUp, gain::Gain, Plain, rows = [GainUp]; gain::Gain::Up);

/// Decreases gain by one camera-defined step.
#[derive(Debug, Clone, Copy, Default)]
pub struct GainDown;
crate::command::nouns::forward_request!(GainDown, gain::Gain, Plain, rows = [GainDown]; gain::Gain::Down);

/// Resets the iris using the target profile's completion policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct IrisReset;
crate::command::nouns::forward_request!(IrisReset, builtin::IrisReset, Targeted, rows = [IrisReset]; builtin::IrisReset);

/// Ups the iris using the target profile's completion policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct IrisUp;
crate::command::nouns::forward_request!(IrisUp, builtin::IrisUp, Targeted, rows = [IrisUp]; builtin::IrisUp);

/// Downs the iris using the target profile's completion policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct IrisDown;
crate::command::nouns::forward_request!(IrisDown, builtin::IrisDown, Targeted, rows = [IrisDown]; builtin::IrisDown);

/// Increases AE adjustment by one quarter EV.
#[derive(Debug, Clone, Copy, Default)]
pub struct AeLevelUp;
crate::command::nouns::forward_request!(AeLevelUp, fr7::Fr7AeLevel, Plain, rows = [Fr7AeLevelUp]; fr7::Fr7AeLevel::Up);

/// Decreases AE adjustment by one quarter EV.
#[derive(Debug, Clone, Copy, Default)]
pub struct AeLevelDown;
crate::command::nouns::forward_request!(AeLevelDown, fr7::Fr7AeLevel, Plain, rows = [Fr7AeLevelDown]; fr7::Fr7AeLevel::Down);

/// Selects the exposure mode.
#[derive(Debug, Clone, Copy)]
pub struct SetMode {
    mode: types::ExposureMode,
}

impl SetMode {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::ExposureMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::ExposureMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetMode, legacy::ExposureCommand, Plain, rows = [ExposureMode]; |request| legacy::ExposureCommand::new(request.mode));

/// Sets the shutter speed.
#[derive(Debug, Clone, Copy)]
pub struct SetShutter {
    speed: types::ShutterSpeed,
}

impl SetShutter {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(speed: types::ShutterSpeed) -> Self {
        Self { speed }
    }

    /// Returns the requested speed.
    #[must_use]
    pub const fn speed(self) -> types::ShutterSpeed {
        self.speed
    }
}
crate::command::nouns::forward_request!(SetShutter, legacy::Shutter, Plain, rows = [ShutterDirect]; |request| legacy::Shutter::SetSpeed(request.speed));

/// Enables or disables exposure compensation.
#[derive(Debug, Clone, Copy)]
pub struct SetCompensationEnabled {
    enabled: bool,
}

impl SetCompensationEnabled {
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
crate::command::nouns::forward_request!(SetCompensationEnabled, legacy::ExposureCompensation, Plain, rows = [ExposureCompensationOn, ExposureCompensationOff]; |request| if request.enabled { legacy::ExposureCompensation::On } else { legacy::ExposureCompensation::Off });

/// Sets the exposure-compensation level.
#[derive(Debug, Clone, Copy)]
pub struct SetCompensation {
    level: types::ExposureCompensationLevel,
}

impl SetCompensation {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::ExposureCompensationLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::ExposureCompensationLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetCompensation, legacy::ExposureCompensation, Plain, rows = [ExposureCompensationDirect]; |request| legacy::ExposureCompensation::SetLevel(request.level));

/// Sets the wide-dynamic-range level.
#[derive(Debug, Clone, Copy)]
pub struct SetWideDynamicRange {
    level: types::WideDynamicRangeLevel,
}

impl SetWideDynamicRange {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::WideDynamicRangeLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::WideDynamicRangeLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetWideDynamicRange, legacy::DynamicRange, Plain, rows = [DynamicRange]; |request| legacy::DynamicRange::new(request.level));

/// Sets a direct iris level.
#[derive(Debug, Clone, Copy)]
pub struct SetIris {
    level: types::IrisLevel,
}

impl SetIris {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::IrisLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::IrisLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetIris, builtin::IrisDirect, Targeted, rows = [IrisDirect]; |request| builtin::IrisDirect::new(request.level));

/// Sets the exposure-brightness level.
#[derive(Debug, Clone, Copy)]
pub struct SetBrightness {
    level: types::BrightnessLevel,
}

impl SetBrightness {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::BrightnessLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::BrightnessLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetBrightness, legacy::Brightness, Plain, rows = [BrightnessSet]; |request| legacy::Brightness::SetLevel(request.level));

/// Sets the gain level.
#[derive(Debug, Clone, Copy)]
pub struct SetGain {
    level: types::GainLevel,
}

impl SetGain {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::GainLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::GainLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetGain, gain::Gain, Plain, rows = [GainDirect]; |request| gain::Gain::SetValue(request.level));

/// Sets the automatic-gain limit.
#[derive(Debug, Clone, Copy)]
pub struct SetGainLimit {
    limit: types::GainLimit,
}

impl SetGainLimit {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(limit: types::GainLimit) -> Self {
        Self { limit }
    }

    /// Returns the requested limit.
    #[must_use]
    pub const fn limit(self) -> types::GainLimit {
        self.limit
    }
}
crate::command::nouns::forward_request!(SetGainLimit, gain::GainLimitCommand, Plain, rows = [GainLimit]; |request| gain::GainLimitCommand::new(request.limit));

/// Sets the anti-flicker mode.
#[derive(Debug, Clone, Copy)]
pub struct SetAntiFlicker {
    mode: types::AntiFlickerMode,
}

impl SetAntiFlicker {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::AntiFlickerMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::AntiFlickerMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetAntiFlicker, legacy::AntiFlickerCommand, Plain, rows = [AntiFlicker]; |request| legacy::AntiFlickerCommand::new(request.mode));

/// Enables or disables spotlight exposure.
#[derive(Debug, Clone, Copy)]
pub struct SetSpotlight {
    enabled: bool,
}

impl SetSpotlight {
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
crate::command::nouns::forward_request!(SetSpotlight, builtin::SpotlightToggle, Plain, rows = [SpotlightOn, SpotlightOff]; |request| builtin::SpotlightToggle::new(request.enabled));

/// Enables or disables automatic slow shutter.
#[derive(Debug, Clone, Copy)]
pub struct SetAutoSlowShutter {
    enabled: bool,
}

impl SetAutoSlowShutter {
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
crate::command::nouns::forward_request!(SetAutoSlowShutter, builtin::AutoSlowShutterToggle, Plain, rows = [AutoSlowShutterOn, AutoSlowShutterOff]; |request| builtin::AutoSlowShutterToggle::new(request.enabled));

/// Enables or disables automatic gain.
#[derive(Debug, Clone, Copy)]
pub struct SetAutoGain {
    enabled: bool,
}

impl SetAutoGain {
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
crate::command::nouns::forward_request!(SetAutoGain, fr7::Fr7AutoGain, Plain, rows = [Fr7AutoGain]; |request| fr7::Fr7AutoGain::new(request.enabled));

/// Enables or disables automatic shutter.
#[derive(Debug, Clone, Copy)]
pub struct SetAutoShutter {
    enabled: bool,
}

impl SetAutoShutter {
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
crate::command::nouns::forward_request!(SetAutoShutter, fr7::Fr7AutoShutter, Plain, rows = [Fr7AutoShutter]; |request| fr7::Fr7AutoShutter::new(request.enabled));

/// Enables or disables automatic iris.
#[derive(Debug, Clone, Copy)]
pub struct SetAutoIris {
    enabled: bool,
}

impl SetAutoIris {
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
crate::command::nouns::forward_request!(SetAutoIris, fr7::Fr7AutoIris, Plain, rows = [Fr7AutoIris]; |request| fr7::Fr7AutoIris::new(request.enabled));

/// Opens the iris by the requested relative count.
#[derive(Debug, Clone, Copy)]
pub struct IrisUpBy {
    steps: types::IrisStep,
}

impl IrisUpBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::IrisStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::IrisStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(IrisUpBy, fr7::Fr7IrisUp, AppliedOnly, rows = [Fr7IrisUp]; |request| fr7::Fr7IrisUp::new(request.steps));

/// Closes the iris by the requested relative count.
#[derive(Debug, Clone, Copy)]
pub struct IrisDownBy {
    steps: types::IrisStep,
}

impl IrisDownBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::IrisStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::IrisStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(IrisDownBy, fr7::Fr7IrisDown, AppliedOnly, rows = [Fr7IrisDown]; |request| fr7::Fr7IrisDown::new(request.steps));

/// Enables or disables backlight compensation.
#[derive(Debug, Clone, Copy)]
pub struct SetBacklightCompensation {
    enabled: bool,
}

impl SetBacklightCompensation {
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
crate::command::nouns::forward_request!(SetBacklightCompensation, image_wire::BacklightCommand, Plain, rows = [Backlight]; |request| image_wire::BacklightCommand::new(request.enabled));

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        command::semantics::{BuiltinCommand, BuiltinRequestContract},
        Inquiry, Request,
    };

    fn packet(request: &impl Request) -> Vec<u8> {
        let mut buffer = [0; 32];
        let written = request
            .write_into(crate::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        buffer[..written].to_vec()
    }

    #[test]
    fn gain_and_iris_readbacks_feed_their_setters_without_losing_high_nibbles() {
        for raw in 0..=types::GainLevel::MAX.value() {
            let value: types::GainLevel = GainInquiry
                .decoder()
                .decode(&[0, 0, raw >> 4, raw & 0x0f])
                .unwrap();
            let request = SetGain::new(value);
            assert_eq!(request.level(), value);
            assert_eq!(
                packet(&request),
                [0x81, 1, 4, 0x4c, 0, 0, raw >> 4, raw & 0x0f, 0xff]
            );
        }
        for raw in 0..=types::IrisLevel::MAX.value() {
            let value: types::IrisLevel = IrisInquiry
                .decoder()
                .decode(&[0, 0, raw >> 4, raw & 0x0f])
                .unwrap();
            let request = SetIris::new(value);
            assert_eq!(request.level(), value);
            assert_eq!(
                packet(&request),
                [0x81, 1, 4, 0x4b, 0, 0, raw >> 4, raw & 0x0f, 0xff]
            );
        }
    }

    #[test]
    fn boolean_setters_keep_the_original_selected_ledger_rows() {
        for enabled in [false, true] {
            let spotlight = SetSpotlight::new(enabled);
            assert_eq!(spotlight.enabled(), enabled);
            assert_eq!(
                spotlight.selected_ledger_row(),
                if enabled {
                    BuiltinCommand::SpotlightOn
                } else {
                    BuiltinCommand::SpotlightOff
                }
            );
            assert_eq!(
                packet(&spotlight),
                [0x81, 1, 4, 0x3a, if enabled { 2 } else { 3 }, 0xff]
            );
            let slow = SetAutoSlowShutter::new(enabled);
            assert_eq!(slow.enabled(), enabled);
            assert_eq!(
                slow.selected_ledger_row(),
                if enabled {
                    BuiltinCommand::AutoSlowShutterOn
                } else {
                    BuiltinCommand::AutoSlowShutterOff
                }
            );
            assert_eq!(
                packet(&slow),
                [0x81, 1, 4, 0x5a, if enabled { 2 } else { 3 }, 0xff]
            );
            let compensation = SetCompensationEnabled::new(enabled);
            assert_eq!(compensation.enabled(), enabled);
            assert_eq!(
                compensation.selected_ledger_row(),
                if enabled {
                    BuiltinCommand::ExposureCompensationOn
                } else {
                    BuiltinCommand::ExposureCompensationOff
                }
            );
        }
    }
}
