//! Typed image requests and inquiries.
//!
//! Each type corresponds to the method with the same name on the camera accessor.
//! Checked values are preserved until preparation applies the selected target profile.

use crate::{
    command::{
        color, flip, image_wire as legacy, inquiry_structs as inquiries, sony_fr7_image as fr7,
    },
    request::builtin,
    types,
};

/// Reads the saturation.
#[derive(Debug, Clone, Copy, Default)]
pub struct SaturationInquiry;
crate::command::nouns::forward_inquiry!(SaturationInquiry, inquiries::SaturationInquiry; inquiries::SaturationInquiry);

/// Reads the hue.
#[derive(Debug, Clone, Copy, Default)]
pub struct HueInquiry;
crate::command::nouns::forward_inquiry!(HueInquiry, inquiries::HueInquiry; inquiries::HueInquiry);

/// Reads the luminance.
#[derive(Debug, Clone, Copy, Default)]
pub struct LuminanceInquiry;
crate::command::nouns::forward_inquiry!(LuminanceInquiry, inquiries::LuminanceInquiry; inquiries::LuminanceInquiry);

/// Reads the contrast.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContrastInquiry;
crate::command::nouns::forward_inquiry!(ContrastInquiry, inquiries::ContrastInquiry; inquiries::ContrastInquiry);

/// Reads the gamma.
#[derive(Debug, Clone, Copy, Default)]
pub struct GammaInquiry;
crate::command::nouns::forward_inquiry!(GammaInquiry, inquiries::GammaInquiry; inquiries::GammaInquiry);

/// Reads the sharpness mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct SharpnessModeInquiry;
crate::command::nouns::forward_inquiry!(SharpnessModeInquiry, inquiries::SharpnessModeInquiry; inquiries::SharpnessModeInquiry);

/// Reads the sharpness.
#[derive(Debug, Clone, Copy, Default)]
pub struct SharpnessInquiry;
crate::command::nouns::forward_inquiry!(SharpnessInquiry, inquiries::SharpnessPositionInquiry; inquiries::SharpnessPositionInquiry);

/// Reads the noise reduction2d.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoiseReduction2dInquiry;
crate::command::nouns::forward_inquiry!(NoiseReduction2dInquiry, inquiries::NoiseReduction2DInquiry; inquiries::NoiseReduction2DInquiry);

/// Reads the noise reduction2d mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoiseReduction2dModeInquiry;
crate::command::nouns::forward_inquiry!(NoiseReduction2dModeInquiry, inquiries::NoiseReduction2DModeInquiry; inquiries::NoiseReduction2DModeInquiry);

/// Reads the noise reduction3d.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoiseReduction3dInquiry;
crate::command::nouns::forward_inquiry!(NoiseReduction3dInquiry, inquiries::NoiseReduction3DInquiry; inquiries::NoiseReduction3DInquiry);

/// Reads the flip.
#[derive(Debug, Clone, Copy, Default)]
pub struct FlipInquiry;
crate::command::nouns::forward_inquiry!(FlipInquiry, inquiries::ImageFlipInquiry; inquiries::ImageFlipInquiry);

/// Reads the flip mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct FlipModeInquiry;
crate::command::nouns::forward_inquiry!(FlipModeInquiry, inquiries::FlipStateInquiry; inquiries::FlipStateInquiry);

/// Reads the picture effect.
#[derive(Debug, Clone, Copy, Default)]
pub struct PictureEffectInquiry;
crate::command::nouns::forward_inquiry!(PictureEffectInquiry, inquiries::PictureEffectInquiry; inquiries::PictureEffectInquiry);

/// Reads the master black.
#[derive(Debug, Clone, Copy, Default)]
pub struct MasterBlackInquiry;
crate::command::nouns::forward_inquiry!(MasterBlackInquiry, inquiries::Fr7MasterBlackInquiry; inquiries::Fr7MasterBlackInquiry);

/// Reads the red black.
#[derive(Debug, Clone, Copy, Default)]
pub struct RedBlackInquiry;
crate::command::nouns::forward_inquiry!(RedBlackInquiry, inquiries::Fr7RedBlackInquiry; inquiries::Fr7RedBlackInquiry);

/// Reads the blue black.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlueBlackInquiry;
crate::command::nouns::forward_inquiry!(BlueBlackInquiry, inquiries::Fr7BlueBlackInquiry; inquiries::Fr7BlueBlackInquiry);

/// Reads the detail.
#[derive(Debug, Clone, Copy, Default)]
pub struct DetailInquiry;
crate::command::nouns::forward_inquiry!(DetailInquiry, inquiries::Fr7DetailInquiry; inquiries::Fr7DetailInquiry);

/// Reads the knee.
#[derive(Debug, Clone, Copy, Default)]
pub struct KneeInquiry;
crate::command::nouns::forward_inquiry!(KneeInquiry, inquiries::Fr7KneeInquiry; inquiries::Fr7KneeInquiry);

/// Reads the knee mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct KneeModeInquiry;
crate::command::nouns::forward_inquiry!(KneeModeInquiry, inquiries::Fr7KneeModeInquiry; inquiries::Fr7KneeModeInquiry);

/// Reads the knee slope.
#[derive(Debug, Clone, Copy, Default)]
pub struct KneeSlopeInquiry;
crate::command::nouns::forward_inquiry!(KneeSlopeInquiry, inquiries::Fr7KneeSlopeInquiry; inquiries::Fr7KneeSlopeInquiry);

/// Reads the knee point.
#[derive(Debug, Clone, Copy, Default)]
pub struct KneePointInquiry;
crate::command::nouns::forward_inquiry!(KneePointInquiry, inquiries::Fr7KneePointInquiry; inquiries::Fr7KneePointInquiry);

/// Restores the default sharpness.
#[derive(Debug, Clone, Copy, Default)]
pub struct SharpnessReset;
crate::command::nouns::forward_request!(SharpnessReset, legacy::Sharpness, Plain, rows = [SharpnessReset]; legacy::Sharpness::Reset);

/// Increases sharpness by one step.
#[derive(Debug, Clone, Copy, Default)]
pub struct SharpnessUp;
crate::command::nouns::forward_request!(SharpnessUp, legacy::Sharpness, Plain, rows = [SharpnessUp]; legacy::Sharpness::Up);

/// Decreases sharpness by one step.
#[derive(Debug, Clone, Copy, Default)]
pub struct SharpnessDown;
crate::command::nouns::forward_request!(SharpnessDown, legacy::Sharpness, Plain, rows = [SharpnessDown]; legacy::Sharpness::Down);

/// Set saturation.
#[derive(Debug, Clone, Copy)]
pub struct SetSaturation {
    level: types::SaturationLevel,
}

impl SetSaturation {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::SaturationLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::SaturationLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetSaturation, color::SaturationCommand, Plain, rows = [Saturation]; |request| color::SaturationCommand::new(request.level));

/// Set hue.
#[derive(Debug, Clone, Copy)]
pub struct SetHue {
    level: types::HueLevel,
}

impl SetHue {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::HueLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::HueLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetHue, color::HueCommand, Plain, rows = [Hue]; |request| color::HueCommand::new(request.level));

/// Set luminance.
#[derive(Debug, Clone, Copy)]
pub struct SetLuminance {
    level: types::LuminanceLevel,
}

impl SetLuminance {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::LuminanceLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::LuminanceLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetLuminance, legacy::Luminance, Plain, rows = [Luminance]; |request| legacy::Luminance::new(request.level));

/// Set contrast.
#[derive(Debug, Clone, Copy)]
pub struct SetContrast {
    level: types::ContrastLevel,
}

impl SetContrast {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::ContrastLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::ContrastLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetContrast, legacy::Contrast, Plain, rows = [Contrast]; |request| legacy::Contrast::new(request.level));

/// Set gamma.
#[derive(Debug, Clone, Copy)]
pub struct SetGamma {
    level: types::GammaLevel,
}

impl SetGamma {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::GammaLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::GammaLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetGamma, legacy::GammaCommand, Plain, rows = [Gamma]; |request| legacy::GammaCommand::new(request.level));

/// Set sharpness mode.
#[derive(Debug, Clone, Copy)]
pub struct SetSharpnessMode {
    mode: types::SharpnessMode,
}

impl SetSharpnessMode {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::SharpnessMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::SharpnessMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetSharpnessMode, legacy::Sharpness, Plain, rows = [SharpnessMode]; |request| legacy::Sharpness::Mode(request.mode));

/// Set sharpness.
#[derive(Debug, Clone, Copy)]
pub struct SetSharpness {
    level: types::SharpnessLevel,
}

impl SetSharpness {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(level: types::SharpnessLevel) -> Self {
        Self { level }
    }

    /// Returns the requested level.
    #[must_use]
    pub const fn level(self) -> types::SharpnessLevel {
        self.level
    }
}
crate::command::nouns::forward_request!(SetSharpness, legacy::Sharpness, Plain, rows = [SharpnessDirect]; |request| legacy::Sharpness::SetLevel { value: request.level });

/// Set noise reduction2d mode.
#[derive(Debug, Clone, Copy)]
pub struct SetNoiseReduction2dMode {
    mode: types::NoiseReduction2DMode,
}

impl SetNoiseReduction2dMode {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::NoiseReduction2DMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::NoiseReduction2DMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetNoiseReduction2dMode, legacy::NoiseReduction2DModeCommand, Plain, rows = [NoiseReduction2dMode]; |request| legacy::NoiseReduction2DModeCommand::new(request.mode));

/// Set noise reduction2d.
#[derive(Debug, Clone, Copy)]
pub struct SetNoiseReduction2d {
    setting: types::NoiseReduction2D,
}

impl SetNoiseReduction2d {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(setting: types::NoiseReduction2D) -> Self {
        Self { setting }
    }

    /// Returns the requested setting.
    #[must_use]
    pub const fn setting(self) -> types::NoiseReduction2D {
        self.setting
    }
}
crate::command::nouns::forward_request!(SetNoiseReduction2d, legacy::NoiseReduction2DCommand, Plain, rows = [NoiseReduction2d, NoiseReduction2dOff]; |request| legacy::NoiseReduction2DCommand::new(request.setting));

/// Set noise reduction3d.
#[derive(Debug, Clone, Copy)]
pub struct SetNoiseReduction3d {
    setting: types::NoiseReduction3D,
}

impl SetNoiseReduction3d {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(setting: types::NoiseReduction3D) -> Self {
        Self { setting }
    }

    /// Returns the requested setting.
    #[must_use]
    pub const fn setting(self) -> types::NoiseReduction3D {
        self.setting
    }
}
crate::command::nouns::forward_request!(SetNoiseReduction3d, legacy::NoiseReduction3DCommand, Plain, rows = [NoiseReduction3d, NoiseReduction3dOff]; |request| legacy::NoiseReduction3DCommand::new(request.setting));

/// Set flip.
#[derive(Debug, Clone, Copy)]
pub struct SetFlip {
    enabled: bool,
}

impl SetFlip {
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
crate::command::nouns::forward_request!(SetFlip, builtin::ImageFlipCommand, Plain, rows = [ImageFlipVertical, ImageFlipOff]; |request| builtin::ImageFlipCommand::new(request.enabled));

/// Set mirror.
#[derive(Debug, Clone, Copy)]
pub struct SetMirror {
    enabled: bool,
}

impl SetMirror {
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
crate::command::nouns::forward_request!(SetMirror, builtin::ImageMirrorCommand, Plain, rows = [ImageFlipHorizontal, ImageFlipHorizontalOff]; |request| builtin::ImageMirrorCommand::new(request.enabled));

/// Set flip mode.
#[derive(Debug, Clone, Copy)]
pub struct SetFlipMode {
    mode: types::ImageFlipMode,
}

impl SetFlipMode {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::ImageFlipMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::ImageFlipMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetFlipMode, legacy::ImageFlipCombinedCommand, Plain, rows = [ImageFlipBoth, ImageFlipCombined]; |request| legacy::ImageFlipCombinedCommand::new(request.mode));

/// Set freeze.
#[derive(Debug, Clone, Copy)]
pub struct SetFreeze {
    enabled: bool,
}

impl SetFreeze {
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
crate::command::nouns::forward_request!(SetFreeze, flip::ImageFreeze, Plain, rows = [ImageFreezeOn, ImageFreezeOff]; |request| flip::ImageFreeze::new(request.enabled));

/// Set picture effect.
#[derive(Debug, Clone, Copy)]
pub struct SetPictureEffect {
    mode: types::PictureEffectMode,
}

impl SetPictureEffect {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::PictureEffectMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::PictureEffectMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetPictureEffect, legacy::PictureEffectCommand, Plain, rows = [PictureEffect]; |request| legacy::PictureEffectCommand::new(request.mode));

/// Master black up by.
#[derive(Debug, Clone, Copy)]
pub struct MasterBlackUpBy {
    steps: types::BlackStep,
}

impl MasterBlackUpBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::BlackStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::BlackStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(MasterBlackUpBy, fr7::Fr7MasterBlack, Plain, rows = [Fr7MasterBlackUp]; |request| fr7::Fr7MasterBlack::Up(request.steps));

/// Master black down by.
#[derive(Debug, Clone, Copy)]
pub struct MasterBlackDownBy {
    steps: types::BlackStep,
}

impl MasterBlackDownBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::BlackStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::BlackStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(MasterBlackDownBy, fr7::Fr7MasterBlack, Plain, rows = [Fr7MasterBlackDown]; |request| fr7::Fr7MasterBlack::Down(request.steps));

/// Set master black.
#[derive(Debug, Clone, Copy)]
pub struct SetMasterBlack {
    value: types::BlackLevel,
}

impl SetMasterBlack {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(value: types::BlackLevel) -> Self {
        Self { value }
    }

    /// Returns the requested value.
    #[must_use]
    pub const fn value(self) -> types::BlackLevel {
        self.value
    }
}
crate::command::nouns::forward_request!(SetMasterBlack, fr7::Fr7MasterBlack, Plain, rows = [Fr7MasterBlackDirect]; |request| fr7::Fr7MasterBlack::Direct(request.value));

/// Red black up by.
#[derive(Debug, Clone, Copy)]
pub struct RedBlackUpBy {
    steps: types::BlackStep,
}

impl RedBlackUpBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::BlackStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::BlackStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(RedBlackUpBy, fr7::Fr7RedBlack, Plain, rows = [Fr7RedBlackUp]; |request| fr7::Fr7RedBlack::Up(request.steps));

/// Red black down by.
#[derive(Debug, Clone, Copy)]
pub struct RedBlackDownBy {
    steps: types::BlackStep,
}

impl RedBlackDownBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::BlackStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::BlackStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(RedBlackDownBy, fr7::Fr7RedBlack, Plain, rows = [Fr7RedBlackDown]; |request| fr7::Fr7RedBlack::Down(request.steps));

/// Set red black.
#[derive(Debug, Clone, Copy)]
pub struct SetRedBlack {
    value: types::BlackLevel,
}

impl SetRedBlack {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(value: types::BlackLevel) -> Self {
        Self { value }
    }

    /// Returns the requested value.
    #[must_use]
    pub const fn value(self) -> types::BlackLevel {
        self.value
    }
}
crate::command::nouns::forward_request!(SetRedBlack, fr7::Fr7RedBlack, Plain, rows = [Fr7RedBlackDirect]; |request| fr7::Fr7RedBlack::Direct(request.value));

/// Blue black up by.
#[derive(Debug, Clone, Copy)]
pub struct BlueBlackUpBy {
    steps: types::BlackStep,
}

impl BlueBlackUpBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::BlackStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::BlackStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(BlueBlackUpBy, fr7::Fr7BlueBlack, Plain, rows = [Fr7BlueBlackUp]; |request| fr7::Fr7BlueBlack::Up(request.steps));

/// Blue black down by.
#[derive(Debug, Clone, Copy)]
pub struct BlueBlackDownBy {
    steps: types::BlackStep,
}

impl BlueBlackDownBy {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(steps: types::BlackStep) -> Self {
        Self { steps }
    }

    /// Returns the requested steps.
    #[must_use]
    pub const fn steps(self) -> types::BlackStep {
        self.steps
    }
}
crate::command::nouns::forward_request!(BlueBlackDownBy, fr7::Fr7BlueBlack, Plain, rows = [Fr7BlueBlackDown]; |request| fr7::Fr7BlueBlack::Down(request.steps));

/// Set blue black.
#[derive(Debug, Clone, Copy)]
pub struct SetBlueBlack {
    value: types::BlackLevel,
}

impl SetBlueBlack {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(value: types::BlackLevel) -> Self {
        Self { value }
    }

    /// Returns the requested value.
    #[must_use]
    pub const fn value(self) -> types::BlackLevel {
        self.value
    }
}
crate::command::nouns::forward_request!(SetBlueBlack, fr7::Fr7BlueBlack, Plain, rows = [Fr7BlueBlackDirect]; |request| fr7::Fr7BlueBlack::Direct(request.value));

/// Set detail.
#[derive(Debug, Clone, Copy)]
pub struct SetDetail {
    enabled: bool,
}

impl SetDetail {
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
crate::command::nouns::forward_request!(SetDetail, fr7::Fr7DetailSetting, Plain, rows = [Fr7DetailSetting]; |request| fr7::Fr7DetailSetting::new(request.enabled));

/// Set knee.
#[derive(Debug, Clone, Copy)]
pub struct SetKnee {
    enabled: bool,
}

impl SetKnee {
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
crate::command::nouns::forward_request!(SetKnee, fr7::Fr7KneeSetting, Plain, rows = [Fr7KneeSetting]; |request| fr7::Fr7KneeSetting::new(request.enabled));

/// Set knee mode.
#[derive(Debug, Clone, Copy)]
pub struct SetKneeMode {
    mode: types::KneeMode,
}

impl SetKneeMode {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(mode: types::KneeMode) -> Self {
        Self { mode }
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> types::KneeMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(SetKneeMode, fr7::Fr7SetKneeMode, Plain, rows = [Fr7SetKneeMode]; |request| fr7::Fr7SetKneeMode::new(request.mode));

/// Set knee slope.
#[derive(Debug, Clone, Copy)]
pub struct SetKneeSlope {
    slope: types::KneeSlope,
}

impl SetKneeSlope {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(slope: types::KneeSlope) -> Self {
        Self { slope }
    }

    /// Returns the requested slope.
    #[must_use]
    pub const fn slope(self) -> types::KneeSlope {
        self.slope
    }
}
crate::command::nouns::forward_request!(SetKneeSlope, fr7::Fr7SetKneeSlope, Plain, rows = [Fr7SetKneeSlope]; |request| fr7::Fr7SetKneeSlope::new(request.slope));

/// Set knee point.
#[derive(Debug, Clone, Copy)]
pub struct SetKneePoint {
    point: types::KneePoint,
}

impl SetKneePoint {
    /// Creates a request carrying the checked setting.
    #[must_use]
    pub const fn new(point: types::KneePoint) -> Self {
        Self { point }
    }

    /// Returns the requested point.
    #[must_use]
    pub const fn point(self) -> types::KneePoint {
        self.point
    }
}
crate::command::nouns::forward_request!(SetKneePoint, fr7::Fr7SetKneePoint, Plain, rows = [Fr7SetKneePoint]; |request| fr7::Fr7SetKneePoint::new(request.point));

/// Reads whether the selected knee parameter can be configured.
#[derive(Debug, Clone, Copy)]
pub struct KneePermissionInquiry {
    inner: inquiries::Fr7KneePermissionInquiry,
}

impl KneePermissionInquiry {
    /// Creates an inquiry for the checked knee parameter.
    #[must_use]
    pub const fn new(parameter: types::KneeParameter) -> Self {
        Self {
            inner: inquiries::Fr7KneePermissionInquiry::new(parameter),
        }
    }

    /// Returns the knee parameter this inquiry addresses.
    #[must_use]
    pub const fn parameter(self) -> types::KneeParameter {
        self.inner.parameter()
    }
}
crate::command::nouns::forward_inquiry!(KneePermissionInquiry, inquiries::Fr7KneePermissionInquiry);

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
    fn noise_reduction_readbacks_round_trip_off_and_every_checked_level() {
        for raw in 0..=types::NoiseReduction2DLevel::MAX.value() {
            let setting: types::NoiseReduction2D =
                NoiseReduction2dInquiry.decoder().decode(&[raw]).unwrap();
            let request = SetNoiseReduction2d::new(setting);
            assert_eq!(request.setting(), setting);
            assert_eq!(packet(&request), [0x81, 1, 4, 0x53, raw, 0xff]);
        }
        for raw in 0..=types::NoiseReduction3DLevel::MAX.value() {
            let setting: types::NoiseReduction3D =
                NoiseReduction3dInquiry.decoder().decode(&[raw]).unwrap();
            let request = SetNoiseReduction3d::new(setting);
            assert_eq!(request.setting(), setting);
            assert_eq!(packet(&request), [0x81, 1, 4, 0x54, raw, 0xff]);
        }
    }

    #[test]
    fn combined_flip_mode_and_vertical_flip_keep_distinct_reply_types() {
        for (raw, mode) in [
            (0, types::ImageFlipMode::Off),
            (1, types::ImageFlipMode::Horizontal),
            (2, types::ImageFlipMode::Vertical),
            (3, types::ImageFlipMode::Both),
        ] {
            let value: types::ImageFlipMode = FlipModeInquiry.decoder().decode(&[raw]).unwrap();
            assert_eq!(value, mode);
            let request = SetFlipMode::new(value);
            assert_eq!(request.mode(), value);
            assert_eq!(packet(&request), [0x81, 1, 4, 0xa4, raw, 0xff]);
        }
        for (raw, enabled) in [(2, true), (3, false)] {
            let value: bool = FlipInquiry.decoder().decode(&[raw]).unwrap();
            assert_eq!(value, enabled);
            let request = SetFlip::new(value);
            assert_eq!(request.enabled(), value);
            assert_eq!(packet(&request), [0x81, 1, 4, 0x66, raw, 0xff]);
        }
    }
}
