//! Object-safe noun projection for the owner-backed dynamic camera.
//!
//! This module is deliberately a mechanical projection of the closed static
//! surface in [`crate::command::surface`].  Every method routes through the
//! generic preparation/admission methods on [`super::DynSessionCamera`]; the
//! dynamic layer does not duplicate capability or profile validation.

#![cfg(feature = "dyn-api")]

use crate::{
    camera::{IdleWait, MotionQuery, PanTiltPosition},
    command,
    completion::{AppliedOnly, Targeted},
    request::builtin,
    requests::{Inquiry, OperationCommand, PlainCommand},
    types,
    units::{Degrees, UnitInterval},
    Error, Result, ZoomDomain,
};

use super::{DynAppliedOperation, DynFuture, DynSessionCamera, DynTargetedOperation};

/// Number of target-facing built-in command methods in this projection.
pub const DYN_NOUN_TARGET_METHOD_COUNT: usize = 143;

/// Number of typed inquiry methods in this projection.
pub const DYN_NOUN_INQUIRY_METHOD_COUNT: usize = 66;

/// Number of domain nouns (motion is a separate safety/observation view).
pub const DYN_NOUN_COUNT: usize = 14;

/// Number of declared non-ledger convenience methods on the noun traits.
///
/// A dynamic noun method is one of exactly three things: a projection of a
/// built-in command row, a typed inquiry accessor, or one of the hand-written
/// convenience wrappers named in [`DYN_NOUN_CONVENIENCE_METHODS`]. Keeping the
/// third category declared is what lets the inventory gate keep checking that
/// the projection carries nothing else.
pub const DYN_NOUN_CONVENIENCE_METHOD_COUNT: usize = 9;

/// The non-ledger convenience wrappers carried by the dynamic noun traits.
///
/// Each entry delegates to a ledger method with a fixed argument, so it adds
/// ergonomics without adding a command row. `src/noun_parity.rs` separately
/// requires the async and blocking facades to expose the same method set, so
/// none of these can become dynamic-only.
pub const DYN_NOUN_CONVENIENCE_METHODS: &[(&str, &str)] = &[
    ("DynZoom", "set_normalized"),
    ("DynZoom", "set_normalized_in_domain"),
    ("DynPanTilt", "up"),
    ("DynPanTilt", "down"),
    ("DynPanTilt", "left"),
    ("DynPanTilt", "right"),
    ("DynNdFilter", "set_stops"),
    ("DynMotionSync", "set_speed"),
    ("DynMenu", "toggle_display"),
];

fn plain<'a, C>(camera: &'a DynSessionCamera, command: C) -> DynFuture<'a, Result<(), Error>>
where
    C: PlainCommand + Send + 'a,
{
    Box::pin(async move { camera.execute(&command).await })
}

fn inquire<'a, Q>(
    camera: &'a DynSessionCamera,
    inquiry: Q,
) -> DynFuture<'a, Result<Q::Response, Error>>
where
    Q: Inquiry + Send + 'a,
{
    Box::pin(async move { camera.inquire(&inquiry).await })
}

fn targeted<'a, O>(
    camera: &'a DynSessionCamera,
    operation: O,
) -> DynFuture<'a, Result<DynTargetedOperation, Error>>
where
    O: OperationCommand<Targeted> + Send + Sync + 'a,
{
    Box::pin(async move { camera.submit_targeted(&operation).await })
}

fn applied<'a, O>(
    camera: &'a DynSessionCamera,
    operation: O,
) -> DynFuture<'a, Result<DynAppliedOperation, Error>>
where
    O: OperationCommand<AppliedOnly> + Send + Sync + 'a,
{
    Box::pin(async move { camera.submit_applied(&operation).await })
}

fn applied_result<'a, O>(
    camera: &'a DynSessionCamera,
    operation: Result<O, Error>,
) -> DynFuture<'a, Result<DynAppliedOperation, Error>>
where
    O: OperationCommand<AppliedOnly> + Send + Sync + 'a,
{
    Box::pin(async move {
        let operation = operation?;
        camera.submit_applied(&operation).await
    })
}

fn targeted_result<'a, O>(
    camera: &'a DynSessionCamera,
    operation: Result<O, Error>,
) -> DynFuture<'a, Result<DynTargetedOperation, Error>>
where
    O: OperationCommand<Targeted> + Send + Sync + 'a,
{
    Box::pin(async move {
        let operation = operation?;
        camera.submit_targeted(&operation).await
    })
}

fn plain_result<'a, C>(
    camera: &'a DynSessionCamera,
    command: Result<C, Error>,
) -> DynFuture<'a, Result<(), Error>>
where
    C: PlainCommand + Send + 'a,
{
    Box::pin(async move {
        let command = command?;
        camera.execute(&command).await
    })
}

/// Object-safe power noun.
pub trait DynPower: Send + Sync {
    /// Inquires the camera's current power state.
    fn state(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Powers the camera on.
    fn on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Places the camera in standby.
    fn off(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe zoom noun.
pub trait DynZoom: Send + Sync {
    /// Inquires the current optical/digital zoom position.
    fn position(&self) -> DynFuture<'_, Result<types::ZoomPosition, Error>>;
    /// Drives toward telephoto at standard speed.
    fn tele(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Drives toward wide angle at standard speed.
    fn wide(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Stops zoom movement.
    fn stop(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Drives toward telephoto at a validated variable speed.
    fn tele_variable(
        &self,
        speed: types::ZoomSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Drives toward wide angle at a validated variable speed.
    fn wide_variable(
        &self,
        speed: types::ZoomSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Moves to an absolute zoom position.
    fn set_position(
        &self,
        position: types::ZoomPosition,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Moves to a normalized position across the optical zoom range.
    ///
    /// `0.0` is the wide end and `1.0` the telephoto end of the profile's
    /// documented optical range.
    fn set_normalized(
        &self,
        position: UnitInterval,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Moves to a normalized position across a documented zoom domain.
    ///
    /// [`ZoomDomain::OpticalPlusDigital`] requires the profile to document a
    /// digital maximum and never falls back to the optical range.
    fn set_normalized_in_domain(
        &self,
        position: UnitInterval,
        domain: ZoomDomain,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Enables or disables digital zoom.
    fn set_digital_zoom(&self, enabled: bool) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe system noun.
pub trait DynSystem: Send + Sync {
    /// Inquires the camera firmware/version information.
    fn version(&self) -> DynFuture<'_, Result<command::VersionInfo, Error>>;
    /// Saves the camera's current settings to non-volatile storage.
    fn save_settings(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe pan/tilt noun.
pub trait DynPanTilt: Send + Sync {
    /// Inquires the current pan/tilt position.
    fn position(&self) -> DynFuture<'_, Result<PanTiltPosition, Error>>;
    /// Moves the pan/tilt mechanism to its home position.
    fn home(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Resets the pan/tilt mechanism.
    fn reset(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Starts a directional pan/tilt drive.
    fn move_direction(
        &self,
        direction: command::PanTiltDirection,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Starts an upward pan/tilt drive.
    fn up(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Starts a downward pan/tilt drive.
    fn down(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Starts a leftward pan/tilt drive.
    fn left(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Starts a rightward pan/tilt drive.
    fn right(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Stops pan/tilt movement using profile-safe stop speeds.
    fn stop(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Moves to an absolute degree position at the selected speed.
    fn absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Moves by a relative degree offset at the selected speed.
    fn relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Sets one pan/tilt movement-limit corner.
    fn limit_set(
        &self,
        corner: command::PanTiltLimitCorner,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Clears one pan/tilt movement-limit corner.
    fn limit_clear(&self, corner: command::PanTiltLimitCorner) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe focus noun.
pub trait DynFocus: Send + Sync {
    /// Inquires the current focus position.
    fn position(&self) -> DynFuture<'_, Result<types::FocusPosition, Error>>;
    /// Inquires the current focus mode.
    fn mode(&self) -> DynFuture<'_, Result<command::FocusMode, Error>>;
    /// Drives focus farther at standard speed.
    fn far(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Drives focus nearer at standard speed.
    fn near(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Drives focus farther at a variable speed.
    fn far_variable(
        &self,
        speed: command::FocusSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Drives focus nearer at a variable speed.
    fn near_variable(
        &self,
        speed: command::FocusSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Stops focus movement.
    fn stop(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Moves focus to an absolute position.
    fn set_position(
        &self,
        position: types::FocusPosition,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Enables automatic focus mode.
    fn auto(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Enables manual focus mode.
    fn manual(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Triggers one-push autofocus.
    fn one_push(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Moves focus to infinity.
    fn infinity(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Toggles automatic/manual focus mode.
    fn toggle(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Triggers vendor snap focus.
    fn snap(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Selects a focus zone.
    fn set_zone(&self, zone: command::FocusZone) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the autofocus sensitivity.
    fn set_sensitivity(
        &self,
        sensitivity: command::AutoFocusSensitivity,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the minimum focus distance.
    fn set_near_limit(&self, position: types::FocusPosition) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the focus-lock mode.
    fn set_lock(&self, mode: command::FocusLock) -> DynFuture<'_, Result<(), Error>>;
    /// Presses the vendor Push-AF control.
    fn push_af_press(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Releases the vendor Push-AF control.
    fn push_af_release(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    /// Inquires the configured focus near limit.
    fn near_limit(&self) -> DynFuture<'_, Result<types::FocusPosition, Error>>;
    /// Inquires the configured focus zone.
    fn zone(&self) -> DynFuture<'_, Result<command::FocusZone, Error>>;
    /// Inquires the autofocus sensitivity.
    fn sensitivity(&self) -> DynFuture<'_, Result<command::AutoFocusSensitivity, Error>>;
    /// Inquires the configured focus range.
    fn range(&self) -> DynFuture<'_, Result<command::FocusRange, Error>>;
}

/// Object-safe preset noun.
pub trait DynPresets: Send + Sync {
    /// Recalls a stored preset as a targeted operation.
    fn recall(
        &self,
        preset: command::PresetNumber,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Sets the preset-recall speed.
    fn set_recall_speed(
        &self,
        speed: command::PresetRecallSpeed,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Stores the current camera state in a preset.
    fn set(&self, preset: command::PresetNumber) -> DynFuture<'_, Result<(), Error>>;
    /// Clears a stored preset.
    fn reset(&self, preset: command::PresetNumber) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe exposure noun.
pub trait DynExposure: Send + Sync {
    /// Inquires the active exposure mode.
    fn mode(&self) -> DynFuture<'_, Result<command::ExposureMode, Error>>;
    /// Sets the exposure mode.
    fn set_mode(&self, mode: command::ExposureMode) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the shutter speed.
    fn shutter(&self) -> DynFuture<'_, Result<types::ShutterSpeed, Error>>;
    /// Restores the camera's shutter default.
    fn shutter_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases shutter speed by one camera-defined step.
    fn shutter_up(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases shutter speed by one camera-defined step.
    fn shutter_down(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets an explicit shutter speed.
    fn shutter_direct(&self, speed: types::ShutterSpeed) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires exposure compensation value.
    fn compensation(&self) -> DynFuture<'_, Result<types::ExposureCompensationLevel, Error>>;
    /// Inquires whether exposure compensation is enabled.
    fn compensation_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires the camera's exposure compensation position.
    fn compensation_position(
        &self,
    ) -> DynFuture<'_, Result<types::ExposureCompensationPosition, Error>>;
    /// Enables exposure compensation.
    fn compensation_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Disables exposure compensation.
    fn compensation_off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Resets exposure compensation.
    fn compensation_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases exposure compensation by one step.
    fn compensation_up(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases exposure compensation by one step.
    fn compensation_down(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets direct exposure compensation.
    fn compensation_direct(
        &self,
        level: types::ExposureCompensationLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the wide-dynamic-range level.
    fn dynamic_range(&self) -> DynFuture<'_, Result<types::DynamicRangeLevel, Error>>;
    /// Sets the wide-dynamic-range level.
    fn set_dynamic_range(
        &self,
        level: types::DynamicRangeLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires whether iris control is automatic.
    fn iris_control(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires the iris level.
    fn iris(&self) -> DynFuture<'_, Result<types::IrisLevel, Error>>;
    /// Resets the iris.
    fn iris_reset(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Increases the iris by one step.
    fn iris_up(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Decreases the iris by one step.
    fn iris_down(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Sets a direct iris level.
    fn iris_direct(
        &self,
        level: types::IrisLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Inquires exposure brightness.
    fn brightness(&self) -> DynFuture<'_, Result<types::BrightnessLevel, Error>>;
    /// Resets exposure brightness.
    fn brightness_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases exposure brightness.
    fn brightness_up(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases exposure brightness.
    fn brightness_down(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets exposure brightness through the bright-direct command.
    fn brightness_set(&self, level: types::BrightnessLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the camera's direct brightness value.
    fn brightness_direct(&self, level: types::BrightnessLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires gain.
    fn gain(&self) -> DynFuture<'_, Result<types::GainLevel, Error>>;
    /// Resets gain.
    fn gain_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases gain.
    fn gain_up(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases gain.
    fn gain_down(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets direct gain.
    fn gain_direct(&self, level: types::GainLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the configured gain limit.
    fn gain_limit(&self) -> DynFuture<'_, Result<types::GainLimit, Error>>;
    /// Sets the configured gain limit.
    fn set_gain_limit(&self, limit: types::GainLimit) -> DynFuture<'_, Result<(), Error>>;
    /// Sets anti-flicker mode.
    fn set_anti_flicker(&self, mode: command::AntiFlickerMode) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the configured anti-flicker mode.
    fn flicker_mode(&self) -> DynFuture<'_, Result<command::AntiFlickerMode, Error>>;
    /// Enables spotlight mode.
    fn spotlight_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Disables spotlight mode.
    fn spotlight_off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Enables automatic slow shutter.
    fn auto_slow_shutter_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Disables automatic slow shutter.
    fn auto_slow_shutter_off(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe white-balance noun.
pub trait DynWhiteBalance: Send + Sync {
    /// Inquires the active white-balance mode.
    fn mode(&self) -> DynFuture<'_, Result<command::WhiteBalanceMode, Error>>;
    /// Selects automatic white balance.
    fn auto(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Selects the indoor white-balance preset.
    fn indoor(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Selects the outdoor white-balance preset.
    fn outdoor(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Selects one-push white balance.
    fn one_push(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Selects auto-tracking white balance.
    fn atw(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Selects manual white balance.
    fn manual(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Selects color-temperature white balance mode.
    fn color_temperature_mode(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets automatic white-balance sensitivity.
    fn set_sensitivity(
        &self,
        sensitivity: command::AutoWhiteBalanceSensitivity,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires automatic white-balance sensitivity.
    fn sensitivity(&self) -> DynFuture<'_, Result<command::AutoWhiteBalanceSensitivity, Error>>;
    /// Triggers one-push white-balance calibration.
    fn one_push_trigger(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets red-channel white-balance tuning.
    fn set_red_tuning(&self, level: types::RedTuning) -> DynFuture<'_, Result<(), Error>>;
    /// Sets blue-channel white-balance tuning.
    fn set_blue_tuning(&self, level: types::BlueTuning) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the color temperature.
    fn color_temperature(&self) -> DynFuture<'_, Result<types::ColorTemp, Error>>;
    /// Resets color temperature.
    fn reset_color_temperature(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases color temperature.
    fn increase_color_temperature(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases color temperature.
    fn decrease_color_temperature(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets a direct color-temperature value.
    fn set_color_temperature(
        &self,
        temperature: types::ColorTemp,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the red-channel gain.
    fn red_gain(&self) -> DynFuture<'_, Result<types::RedChannel, Error>>;
    /// Resets red-channel gain.
    fn reset_red_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases red-channel gain.
    fn increase_red_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases red-channel gain.
    fn decrease_red_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets direct red-channel gain.
    fn set_red_gain(&self, value: types::RedChannel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the blue-channel gain.
    fn blue_gain(&self) -> DynFuture<'_, Result<types::BlueChannel, Error>>;
    /// Resets blue-channel gain.
    fn reset_blue_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases blue-channel gain.
    fn increase_blue_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases blue-channel gain.
    fn decrease_blue_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets direct blue-channel gain.
    fn set_blue_gain(&self, value: types::BlueChannel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires red-channel tuning.
    fn red_tuning(&self) -> DynFuture<'_, Result<types::RedTuning, Error>>;
    /// Inquires blue-channel tuning.
    fn blue_tuning(&self) -> DynFuture<'_, Result<types::BlueTuning, Error>>;
}

/// Object-safe image-processing noun.
pub trait DynImage: Send + Sync {
    /// Inquires the camera resolution mode.
    fn resolution(&self) -> DynFuture<'_, Result<command::ResolutionMode, Error>>;
    /// Inquires image saturation.
    fn saturation(&self) -> DynFuture<'_, Result<types::SaturationLevel, Error>>;
    /// Sets image saturation.
    fn set_saturation(&self, level: types::SaturationLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires image hue.
    fn hue(&self) -> DynFuture<'_, Result<types::HueLevel, Error>>;
    /// Sets image hue.
    fn set_hue(&self, level: types::HueLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires image luminance.
    fn luminance(&self) -> DynFuture<'_, Result<types::LuminanceLevel, Error>>;
    /// Sets image luminance.
    fn set_luminance(&self, level: types::LuminanceLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires image contrast.
    fn contrast(&self) -> DynFuture<'_, Result<types::ContrastLevel, Error>>;
    /// Sets image contrast.
    fn set_contrast(&self, level: types::ContrastLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the gamma curve.
    fn gamma(&self) -> DynFuture<'_, Result<types::GammaLevel, Error>>;
    /// Sets the gamma curve.
    fn set_gamma(&self, level: types::GammaLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the sharpness mode.
    fn sharpness_mode(&self) -> DynFuture<'_, Result<command::SharpnessMode, Error>>;
    /// Inquires the sharpness level.
    fn sharpness_level(&self) -> DynFuture<'_, Result<types::SharpnessLevel, Error>>;
    /// Sets the sharpness mode.
    fn set_sharpness_mode(&self, mode: command::SharpnessMode) -> DynFuture<'_, Result<(), Error>>;
    /// Resets sharpness.
    fn reset_sharpness(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Increases sharpness by one step.
    fn increase_sharpness(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Decreases sharpness by one step.
    fn decrease_sharpness(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets a direct sharpness level.
    fn set_sharpness(&self, level: types::SharpnessLevel) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires backlight compensation state.
    fn backlight(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Enables or disables backlight compensation.
    fn set_backlight(&self, enabled: bool) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires 2D noise reduction level.
    fn noise_reduction_2d(&self) -> DynFuture<'_, Result<types::NoiseReduction2DLevel, Error>>;
    /// Sets 2D noise reduction level.
    fn set_noise_reduction_2d(
        &self,
        level: types::NoiseReduction2DLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires 3D noise reduction level.
    fn noise_reduction_3d(&self) -> DynFuture<'_, Result<types::NoiseReduction3DLevel, Error>>;
    /// Sets 3D noise reduction level.
    fn set_noise_reduction_3d(
        &self,
        level: types::NoiseReduction3DLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the aggregate noise-reduction level.
    fn noise_reduction_level(&self) -> DynFuture<'_, Result<types::NoiseReductionLevel, Error>>;
    /// Inquires the aggregate noise-reduction mode.
    fn noise_reduction_mode(&self) -> DynFuture<'_, Result<command::NoiseReductionMode, Error>>;
    /// Disables vertical image flip.
    fn disable_flip(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Enables vertical image flip.
    fn enable_flip(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Enables horizontal image mirroring.
    fn enable_horizontal_flip(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the combined image-flip mode to both axes.
    fn set_flip_both(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the combined image-flip mode.
    fn set_flip_mode(&self, mode: command::ImageFlipMode) -> DynFuture<'_, Result<(), Error>>;
    /// Freezes the image.
    fn freeze_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Resumes live image output.
    fn freeze_off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the canonical image-flip state.
    fn flip(&self) -> DynFuture<'_, Result<command::FlipState, Error>>;
    /// Inquires the combined image-flip mode.
    fn flip_mode(&self) -> DynFuture<'_, Result<command::FlipState, Error>>;
    /// Inquires whether black-and-white mode is active.
    fn black_white(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires black-and-white mode.
    fn black_white_mode(&self) -> DynFuture<'_, Result<command::BlackWhiteMode, Error>>;
    /// Inquires picture-effect mode.
    fn picture_effect(&self) -> DynFuture<'_, Result<command::PictureEffectMode, Error>>;
    /// Sets picture-effect mode.
    fn set_picture_effect(
        &self,
        mode: command::PictureEffectMode,
    ) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires the camera's defog level.
    fn defog_level(&self) -> DynFuture<'_, Result<types::DefogLevel, Error>>;
}

/// Object-safe tally noun.
pub trait DynTally: Send + Sync {
    /// Inquires all tally light state.
    fn status(&self) -> DynFuture<'_, Result<command::TallyStatusState, Error>>;
    /// Turns the red tally on.
    fn red_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Turns the red tally off.
    fn red_off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets low tally brightness.
    fn bright_lo(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets high tally brightness.
    fn bright_hi(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Turns the green tally on.
    fn green_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Turns the green tally off.
    fn green_off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets tally flash mode.
    fn flash(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets tally solid-on mode.
    fn on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Turns tally output off.
    fn off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Inquires red tally state.
    fn red_status(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires green tally state.
    fn green_status(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires automatic tally adjustment state.
    fn auto_adjust_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
}

/// Object-safe neutral-density filter noun.
pub trait DynNdFilter: Send + Sync {
    /// Inquires the current ND-filter position.
    fn position(&self) -> DynFuture<'_, Result<command::NdFilterPosition, Error>>;
    /// Inquires the current ND-filter preset.
    fn preset(&self) -> DynFuture<'_, Result<types::NdFilterPreset, Error>>;
    /// Selects preset or variable ND-filter mode.
    fn set_mode(&self, mode: command::NdFilterMode) -> DynFuture<'_, Result<(), Error>>;
    /// Sets a direct variable ND-filter value.
    fn set_value(&self, value: u16) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Sets a direct variable ND-filter value in photographic stops.
    ///
    /// `stops` is the light reduction in stops and must lie in `2.0..=7.0`.
    /// Each raw unit is a quarter stop, so `2.0` maps to the minimum density
    /// and `7.0` to the maximum.
    fn set_stops(&self, stops: f32) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Increases ND-filter density by one step.
    fn step_up(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Decreases ND-filter density by one step.
    fn step_down(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    /// Enables automatic ND filtering.
    fn auto_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Disables automatic ND filtering.
    fn auto_off(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe motion-sync noun.
pub trait DynMotionSync: Send + Sync {
    /// Inquires the motion-sync mode.
    fn mode(&self) -> DynFuture<'_, Result<command::MotionSyncMode, Error>>;
    /// Inquires the motion-sync preset speed.
    fn preset(&self) -> DynFuture<'_, Result<command::MotionSyncPreset, Error>>;
    /// Enables or disables motion synchronization.
    fn set_mode(&self, mode: command::MotionSyncMode) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the motion-sync speed preset.
    fn set_preset(&self, speed: u8) -> DynFuture<'_, Result<(), Error>>;
    /// Sets the motion-sync speed from a range-checked speed value.
    ///
    /// This is [`Self::set_preset`] with the `1..=24` bound moved into the
    /// argument type, so an out-of-range speed cannot be constructed.
    fn set_speed(&self, speed: types::MotionSyncSpeed) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe on-screen menu noun.
pub trait DynMenu: Send + Sync {
    /// Inquires whether the on-screen menu is open.
    fn status(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Displays or hides the on-screen menu.
    fn display(&self, on: bool) -> DynFuture<'_, Result<(), Error>>;
    /// Moves the menu cursor.
    fn navigate(&self, direction: command::MenuDirection) -> DynFuture<'_, Result<(), Error>>;
    /// Selects the current menu item.
    fn select(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Cancels or returns from the current menu item.
    fn cancel(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sends a vendor-specific direct menu control.
    fn direct(&self, control1: u8, control2: u8) -> DynFuture<'_, Result<(), Error>>;
    /// Toggles the on-screen menu open or closed.
    ///
    /// This is the vendor open/close direct control, so it needs no prior
    /// [`Self::status`] round trip to decide which way to move.
    fn toggle_display(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe advanced/vendor noun.
pub trait DynAdvanced: Send + Sync {
    /// Inquires night/day mode.
    fn night_day_mode(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires standby state.
    fn standby_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires digital PTZ state.
    fn digital_ptz_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires auto-trace state.
    fn auto_trace_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires focus-unlock state.
    fn focus_unlock(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires the broadcast domain.
    fn broadcast_domain(&self) -> DynFuture<'_, Result<types::BroadcastDomain, Error>>;
    /// Inquires USB-audio state.
    fn usb_audio_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires two-tone mode.
    fn two_tone_mode_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Inquires digital mode.
    fn digital_mode_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Enables multicast streaming.
    fn multicast_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Disables multicast streaming.
    fn multicast_off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets NDI streaming quality.
    fn set_ndi_quality(&self, quality: types::NdiQuality) -> DynFuture<'_, Result<(), Error>>;
    /// Enables USB audio.
    fn usb_audio_on(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Disables USB audio.
    fn usb_audio_off(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Sets pan/tilt variable-speed mode.
    fn set_variable_speed_mode(
        &self,
        mode: command::VariableSpeedMode,
    ) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe motion safety and observation noun.
pub trait DynMotion: Send + Sync {
    /// Stops all supported pan/tilt, zoom, and focus movement.
    fn stop_all_motion(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Reports whether any mechanical movement axis is moving.
    ///
    /// This samples [`AffectedAxes::MOVEMENT`] with the default tolerance; use
    /// [`Self::is_moving_axes`] to pick the axes or the tolerance.
    ///
    /// [`AffectedAxes::MOVEMENT`]: crate::AffectedAxes::MOVEMENT
    fn is_moving(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Reports whether the selected physical axes are moving.
    fn is_moving_axes(&self, query: MotionQuery) -> DynFuture<'_, Result<bool, Error>>;
    /// Waits until the selected physical axes become idle.
    fn wait_until_idle(&self, wait: IdleWait) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe accessors for all final dynamic nouns.
pub trait DynSessionCameraNouns: Send + Sync {
    /// Returns the dynamic power noun.
    fn power(&self) -> &dyn DynPower;
    /// Returns the dynamic zoom noun.
    fn zoom(&self) -> &dyn DynZoom;
    /// Returns the dynamic system noun.
    fn system(&self) -> &dyn DynSystem;
    /// Returns the dynamic pan/tilt noun.
    fn pan_tilt(&self) -> &dyn DynPanTilt;
    /// Returns the dynamic focus noun.
    fn focus(&self) -> &dyn DynFocus;
    /// Returns the dynamic exposure noun.
    fn exposure(&self) -> &dyn DynExposure;
    /// Returns the dynamic white-balance noun.
    fn white_balance(&self) -> &dyn DynWhiteBalance;
    /// Returns the dynamic image noun.
    fn image(&self) -> &dyn DynImage;
    /// Returns the dynamic preset noun.
    fn presets(&self) -> &dyn DynPresets;
    /// Returns the dynamic tally noun.
    fn tally(&self) -> &dyn DynTally;
    /// Returns the dynamic ND-filter noun.
    fn nd_filter(&self) -> &dyn DynNdFilter;
    /// Returns the dynamic motion-sync noun.
    fn motion_sync(&self) -> &dyn DynMotionSync;
    /// Returns the dynamic menu noun.
    fn menu(&self) -> &dyn DynMenu;
    /// Returns the dynamic advanced/vendor noun.
    fn advanced(&self) -> &dyn DynAdvanced;
    /// Returns the dynamic motion safety and observation noun.
    fn motion(&self) -> &dyn DynMotion;
}

impl DynSessionCamera {
    /// Returns the dynamic power noun.
    pub fn power(&self) -> &dyn DynPower {
        self
    }

    /// Returns the dynamic zoom noun.
    pub fn zoom(&self) -> &dyn DynZoom {
        self
    }

    /// Returns the dynamic system noun.
    pub fn system(&self) -> &dyn DynSystem {
        self
    }

    /// Returns the dynamic pan/tilt noun.
    pub fn pan_tilt(&self) -> &dyn DynPanTilt {
        self
    }

    /// Returns the dynamic focus noun.
    pub fn focus(&self) -> &dyn DynFocus {
        self
    }

    /// Returns the dynamic exposure noun.
    pub fn exposure(&self) -> &dyn DynExposure {
        self
    }

    /// Returns the dynamic white-balance noun.
    pub fn white_balance(&self) -> &dyn DynWhiteBalance {
        self
    }

    /// Returns the dynamic image noun.
    pub fn image(&self) -> &dyn DynImage {
        self
    }

    /// Returns the dynamic preset noun.
    pub fn presets(&self) -> &dyn DynPresets {
        self
    }

    /// Returns the dynamic tally noun.
    pub fn tally(&self) -> &dyn DynTally {
        self
    }

    /// Returns the dynamic ND-filter noun.
    pub fn nd_filter(&self) -> &dyn DynNdFilter {
        self
    }

    /// Returns the dynamic motion-sync noun.
    pub fn motion_sync(&self) -> &dyn DynMotionSync {
        self
    }

    /// Returns the dynamic menu noun.
    pub fn menu(&self) -> &dyn DynMenu {
        self
    }

    /// Returns the dynamic advanced/vendor noun.
    pub fn advanced(&self) -> &dyn DynAdvanced {
        self
    }
}

impl DynSessionCameraNouns for DynSessionCamera {
    fn power(&self) -> &dyn DynPower {
        Self::power(self)
    }
    fn zoom(&self) -> &dyn DynZoom {
        Self::zoom(self)
    }
    fn system(&self) -> &dyn DynSystem {
        Self::system(self)
    }
    fn pan_tilt(&self) -> &dyn DynPanTilt {
        Self::pan_tilt(self)
    }
    fn focus(&self) -> &dyn DynFocus {
        Self::focus(self)
    }
    fn exposure(&self) -> &dyn DynExposure {
        Self::exposure(self)
    }
    fn white_balance(&self) -> &dyn DynWhiteBalance {
        Self::white_balance(self)
    }
    fn image(&self) -> &dyn DynImage {
        Self::image(self)
    }
    fn presets(&self) -> &dyn DynPresets {
        Self::presets(self)
    }
    fn tally(&self) -> &dyn DynTally {
        Self::tally(self)
    }
    fn nd_filter(&self) -> &dyn DynNdFilter {
        Self::nd_filter(self)
    }
    fn motion_sync(&self) -> &dyn DynMotionSync {
        Self::motion_sync(self)
    }
    fn menu(&self) -> &dyn DynMenu {
        Self::menu(self)
    }
    fn advanced(&self) -> &dyn DynAdvanced {
        Self::advanced(self)
    }
    fn motion(&self) -> &dyn DynMotion {
        self
    }
}

macro_rules! plain_methods {
    ($(fn $name:ident($($arg:ident : $ty:ty),*) => $command:expr;)+) => {
        $(
            fn $name(&self, $($arg: $ty),*) -> DynFuture<'_, Result<(), Error>> {
                plain(self, $command)
            }
        )+
    };
}

macro_rules! inquiry_methods {
    ($(fn $name:ident() -> $response:ty = $inquiry:path;)+) => {
        $(
            fn $name(&self) -> DynFuture<'_, Result<$response, Error>> {
                inquire(self, $inquiry)
            }
        )+
    };
}

macro_rules! applied_methods {
    ($(fn $name:ident($($arg:ident : $ty:ty),*) => $operation:expr;)+) => {
        $(
            fn $name(
                &self,
                $($arg: $ty),*
            ) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
                applied(self, $operation)
            }
        )+
    };
}

macro_rules! targeted_methods {
    ($(fn $name:ident($($arg:ident : $ty:ty),*) => $operation:expr;)+) => {
        $(
            fn $name(
                &self,
                $($arg: $ty),*
            ) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
                targeted(self, $operation)
            }
        )+
    };
}

impl DynPower for DynSessionCamera {
    inquiry_methods! {
        fn state() -> bool = command::PowerInquiry;
    }
    plain_methods! {
        fn on() => command::PowerOn::new();
        fn off() => command::PowerStandby::new();
    }
}

impl DynZoom for DynSessionCamera {
    inquiry_methods! {
        fn position() -> types::ZoomPosition = command::ZoomPositionInquiry;
    }
    applied_methods! {
        fn tele() => builtin::ZoomDrive::Tele;
        fn wide() => builtin::ZoomDrive::Wide;
        fn stop() => builtin::ZoomStop;
        fn tele_variable(speed: types::ZoomSpeed) => builtin::ZoomDrive::TeleVariable(speed);
        fn wide_variable(speed: types::ZoomSpeed) => builtin::ZoomDrive::WideVariable(speed);
    }
    targeted_methods! {
        fn set_position(position: types::ZoomPosition) => builtin::ZoomTarget::new(position);
    }
    plain_methods! {
        fn set_digital_zoom(enabled: bool) => command::DigitalZoom::new(enabled);
    }

    fn set_normalized(
        &self,
        position: UnitInterval,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        targeted_result(
            self,
            builtin::ZoomTarget::from_normalized(position, ZoomDomain::Optical, self.profile()),
        )
    }

    fn set_normalized_in_domain(
        &self,
        position: UnitInterval,
        domain: ZoomDomain,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        targeted_result(
            self,
            builtin::ZoomTarget::from_normalized(position, domain, self.profile()),
        )
    }
}

impl DynSystem for DynSessionCamera {
    inquiry_methods! {
        fn version() -> command::VersionInfo = command::VersionInquiry;
    }
    plain_methods! {
        fn save_settings() => command::SettingsSaveCommand::new();
    }
}

impl DynPanTilt for DynSessionCamera {
    inquiry_methods! {
        fn position() -> PanTiltPosition = command::PanTiltPositionInquiry;
    }
    targeted_methods! {
        fn home() => builtin::PanTiltHome;
        fn reset() => builtin::PanTiltReset;
    }
    fn move_direction(
        &self,
        direction: command::PanTiltDirection,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
        applied_result(
            self,
            builtin::PanTiltDrive::new(direction, pan_speed, tilt_speed),
        )
    }

    fn up(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
        DynPanTilt::move_direction(self, command::PanTiltDirection::Up, pan_speed, tilt_speed)
    }

    fn down(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
        DynPanTilt::move_direction(self, command::PanTiltDirection::Down, pan_speed, tilt_speed)
    }

    fn left(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
        DynPanTilt::move_direction(self, command::PanTiltDirection::Left, pan_speed, tilt_speed)
    }

    fn right(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
        DynPanTilt::move_direction(
            self,
            command::PanTiltDirection::Right,
            pan_speed,
            tilt_speed,
        )
    }

    fn stop(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
        applied_result(self, self.core().pan_tilt_stop_request())
    }

    fn absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        Box::pin(async move {
            let pan_speed = types::PanSpeed::from(speed);
            let tilt_speed = types::TiltSpeed::from(speed);
            let command = builtin::PanTiltAbsolute::for_profile(
                pan,
                tilt,
                pan_speed,
                tilt_speed,
                self.profile(),
            )?;
            self.submit_targeted(&command).await
        })
    }

    fn relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        Box::pin(async move {
            let pan_speed = types::PanSpeed::from(speed);
            let tilt_speed = types::TiltSpeed::from(speed);
            let command = builtin::PanTiltRelative::for_profile(
                pan,
                tilt,
                pan_speed,
                tilt_speed,
                self.profile(),
            )?;
            self.submit_targeted(&command).await
        })
    }

    fn limit_set(
        &self,
        corner: command::PanTiltLimitCorner,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
    ) -> DynFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let command = builtin::PanTiltLimitSet::for_profile(corner, pan, tilt, self.profile())?;
            self.execute(&command).await
        })
    }

    fn limit_clear(&self, corner: command::PanTiltLimitCorner) -> DynFuture<'_, Result<(), Error>> {
        plain(self, builtin::PanTiltLimitClear::new(corner))
    }
}

impl DynFocus for DynSessionCamera {
    inquiry_methods! {
        fn position() -> types::FocusPosition = command::FocusPositionInquiry;
        fn mode() -> command::FocusMode = command::FocusModeInquiry;
        fn near_limit() -> types::FocusPosition = command::FocusNearLimitInquiry;
        fn zone() -> command::FocusZone = command::FocusZoneInquiry;
        fn sensitivity() -> command::AutoFocusSensitivity = command::AutoFocusSensitivityInquiry;
        fn range() -> command::FocusRange = command::FocusRangeInquiry;
    }
    applied_methods! {
        fn far() => builtin::FocusDrive::Far;
        fn near() => builtin::FocusDrive::Near;
        fn far_variable(speed: command::FocusSpeed) => builtin::FocusDrive::FarVariable(speed);
        fn near_variable(speed: command::FocusSpeed) => builtin::FocusDrive::NearVariable(speed);
        fn stop() => builtin::FocusStop;
        fn one_push() => builtin::FocusTrigger::OnePush;
        fn snap() => builtin::FocusTrigger::Snap;
        fn push_af_press() => builtin::PushAfPress::new();
        fn push_af_release() => builtin::PushAfRelease::new();
    }
    targeted_methods! {
        fn set_position(position: types::FocusPosition) => builtin::FocusTarget::new(position);
        fn infinity() => builtin::FocusInfinity;
    }
    plain_methods! {
        fn auto() => builtin::FocusModeCommand::Auto;
        fn manual() => builtin::FocusModeCommand::Manual;
        fn toggle() => builtin::FocusModeCommand::Toggle;
        fn set_zone(zone: command::FocusZone) => command::FocusZoneCommand::new(zone);
        fn set_sensitivity(sensitivity: command::AutoFocusSensitivity) => command::AutoFocusSensitivityCommand::new(sensitivity);
        fn set_near_limit(position: types::FocusPosition) => command::FocusNearLimitCommand::new(position);
        fn set_lock(mode: command::FocusLock) => mode;
    }
}

impl DynPresets for DynSessionCamera {
    fn recall(
        &self,
        preset: command::PresetNumber,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        targeted_result(
            self,
            builtin::PresetRecall::for_profile(preset, self.profile()),
        )
    }

    fn set_recall_speed(
        &self,
        speed: command::PresetRecallSpeed,
    ) -> DynFuture<'_, Result<(), Error>> {
        plain(self, command::PresetRecallSpeedCommand::new(speed))
    }

    fn set(&self, preset: command::PresetNumber) -> DynFuture<'_, Result<(), Error>> {
        plain(self, builtin::PresetSet::new(preset))
    }

    fn reset(&self, preset: command::PresetNumber) -> DynFuture<'_, Result<(), Error>> {
        plain(self, builtin::PresetReset::new(preset))
    }
}

impl DynExposure for DynSessionCamera {
    inquiry_methods! {
        fn mode() -> command::ExposureMode = command::ExposureModeInquiry;
        fn shutter() -> types::ShutterSpeed = command::ShutterInquiry;
        fn compensation() -> types::ExposureCompensationLevel = command::ExposureCompensationInquiry;
        fn compensation_enabled() -> bool = command::ExposureCompensationModeInquiry;
        fn compensation_position() -> types::ExposureCompensationPosition = command::ExposureCompensationPositionInquiry;
        fn dynamic_range() -> types::DynamicRangeLevel = command::DynamicRangeInquiry;
        fn iris_control() -> bool = command::IrisControlInquiry;
        fn iris() -> types::IrisLevel = command::IrisInquiry;
        fn brightness() -> types::BrightnessLevel = command::BrightnessInquiry;
        fn gain() -> types::GainLevel = command::GainInquiry;
        fn gain_limit() -> types::GainLimit = command::GainLimitInquiry;
        fn flicker_mode() -> command::AntiFlickerMode = command::FlickerModeInquiry;
    }
    plain_methods! {
        fn set_mode(mode: command::ExposureMode) => command::ExposureCommand::new(mode);
        fn shutter_reset() => command::Shutter::Reset;
        fn shutter_up() => command::Shutter::Up;
        fn shutter_down() => command::Shutter::Down;
        fn shutter_direct(speed: types::ShutterSpeed) => command::Shutter::SetSpeed(speed);
        fn compensation_on() => command::ExposureCompensation::On;
        fn compensation_off() => command::ExposureCompensation::Off;
        fn compensation_reset() => command::ExposureCompensation::Reset;
        fn compensation_up() => command::ExposureCompensation::Up;
        fn compensation_down() => command::ExposureCompensation::Down;
        fn compensation_direct(level: types::ExposureCompensationLevel) => command::ExposureCompensation::SetLevel(level);
        fn set_dynamic_range(level: types::DynamicRangeLevel) => command::DynamicRange::new(level);
        fn brightness_reset() => command::Brightness::Reset;
        fn brightness_up() => command::Brightness::Up;
        fn brightness_down() => command::Brightness::Down;
        fn brightness_set(level: types::BrightnessLevel) => command::Brightness::SetLevel(level);
        fn brightness_direct(level: types::BrightnessLevel) => command::Brightness::Direct(level);
        fn gain_reset() => command::Gain::Reset;
        fn gain_up() => command::Gain::Up;
        fn gain_down() => command::Gain::Down;
        fn gain_direct(level: types::GainLevel) => command::Gain::SetValue(level);
        fn set_gain_limit(limit: types::GainLimit) => command::GainLimitCommand::new(limit);
        fn set_anti_flicker(mode: command::AntiFlickerMode) => command::AntiFlickerCommand::new(mode);
        fn spotlight_on() => command::SpotlightOn::new();
        fn spotlight_off() => command::SpotlightOff::new();
        fn auto_slow_shutter_on() => command::AutoSlowShutterOn::new();
        fn auto_slow_shutter_off() => command::AutoSlowShutterOff::new();
    }
    targeted_methods! {
        fn iris_reset() => builtin::IrisReset::new();
        fn iris_up() => builtin::IrisUp::new();
        fn iris_down() => builtin::IrisDown::new();
        fn iris_direct(level: types::IrisLevel) => builtin::IrisDirect::new(level);
    }
}

impl DynWhiteBalance for DynSessionCamera {
    inquiry_methods! {
        fn mode() -> command::WhiteBalanceMode = command::WhiteBalanceModeInquiry;
        fn sensitivity() -> command::AutoWhiteBalanceSensitivity = command::AutoWhiteBalanceSensitivityInquiry;
        fn color_temperature() -> types::ColorTemp = command::ColorTemperatureInquiry;
        fn red_gain() -> types::RedChannel = command::RedGainInquiry;
        fn blue_gain() -> types::BlueChannel = command::BlueGainInquiry;
        fn red_tuning() -> types::RedTuning = command::RedTuningInquiry;
        fn blue_tuning() -> types::BlueTuning = command::BlueTuningInquiry;
    }
    plain_methods! {
        fn auto() => command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Auto);
        fn indoor() => command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Indoor);
        fn outdoor() => command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Outdoor);
        fn one_push() => command::WhiteBalanceCommand::new(command::WhiteBalanceMode::OnePush);
        fn atw() => command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ATW);
        fn manual() => command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Manual);
        fn color_temperature_mode() => command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ColorTemperature);
        fn set_sensitivity(sensitivity: command::AutoWhiteBalanceSensitivity) => command::AWBSensitivityCommand::new(sensitivity);
        fn one_push_trigger() => command::OnePushTriggerCommand::new();
        fn set_red_tuning(level: types::RedTuning) => command::RedTuningCommand::new(level);
        fn set_blue_tuning(level: types::BlueTuning) => command::BlueTuningCommand::new(level);
        fn reset_color_temperature() => command::ColorTemperature::Reset;
        fn increase_color_temperature() => command::ColorTemperature::Up;
        fn decrease_color_temperature() => command::ColorTemperature::Down;
        fn set_color_temperature(temperature: types::ColorTemp) => command::ColorTemperature::SetTemperature(temperature);
        fn reset_red_gain() => command::RedGain::Reset;
        fn increase_red_gain() => command::RedGain::Up;
        fn decrease_red_gain() => command::RedGain::Down;
        fn set_red_gain(value: types::RedChannel) => command::RedGain::SetValue(value);
        fn reset_blue_gain() => command::BlueGain::Reset;
        fn increase_blue_gain() => command::BlueGain::Up;
        fn decrease_blue_gain() => command::BlueGain::Down;
        fn set_blue_gain(value: types::BlueChannel) => command::BlueGain::SetValue(value);
    }
}

impl DynImage for DynSessionCamera {
    inquiry_methods! {
        fn resolution() -> command::ResolutionMode = command::ResolutionInquiry;
        fn saturation() -> types::SaturationLevel = command::SaturationInquiry;
        fn hue() -> types::HueLevel = command::HueInquiry;
        fn luminance() -> types::LuminanceLevel = command::LuminanceInquiry;
        fn contrast() -> types::ContrastLevel = command::ContrastInquiry;
        fn gamma() -> types::GammaLevel = command::GammaInquiry;
        fn sharpness_mode() -> command::SharpnessMode = command::SharpnessModeInquiry;
        fn sharpness_level() -> types::SharpnessLevel = command::SharpnessPositionInquiry;
        fn backlight() -> bool = command::BacklightInquiry;
        fn noise_reduction_2d() -> types::NoiseReduction2DLevel = command::NoiseReduction2DInquiry;
        fn noise_reduction_3d() -> types::NoiseReduction3DLevel = command::NoiseReduction3DInquiry;
        fn noise_reduction_level() -> types::NoiseReductionLevel = command::NrLevelInquiry;
        fn noise_reduction_mode() -> command::NoiseReductionMode = command::NrModeInquiry;
        fn flip() -> command::FlipState = command::ImageFlipInquiry;
        fn flip_mode() -> command::FlipState = command::FlipStateInquiry;
        fn black_white() -> bool = command::BlackWhiteInquiry;
        fn black_white_mode() -> command::BlackWhiteMode = command::BlackWhiteModeInquiry;
        fn picture_effect() -> command::PictureEffectMode = command::PictureEffectInquiry;
        fn defog_level() -> types::DefogLevel = command::DefogLevelInquiry;
    }
    plain_methods! {
        fn set_saturation(level: types::SaturationLevel) => command::SaturationCommand::new(level);
        fn set_hue(level: types::HueLevel) => command::HueCommand::new(level);
        fn set_luminance(level: types::LuminanceLevel) => command::Luminance::new(level);
        fn set_contrast(level: types::ContrastLevel) => command::Contrast::new(level);
        fn set_gamma(level: types::GammaLevel) => command::GammaCommand::new(level);
        fn set_sharpness_mode(mode: command::SharpnessMode) => command::Sharpness::Mode(mode);
        fn reset_sharpness() => command::Sharpness::Reset;
        fn increase_sharpness() => command::Sharpness::Up;
        fn decrease_sharpness() => command::Sharpness::Down;
        fn set_sharpness(level: types::SharpnessLevel) => command::Sharpness::SetLevel { value: level.value() };
        fn set_backlight(enabled: bool) => command::BacklightCommand::new(enabled);
        fn set_noise_reduction_2d(level: types::NoiseReduction2DLevel) => command::NoiseReduction2D::with_level(level);
        fn set_noise_reduction_3d(level: types::NoiseReduction3DLevel) => command::NoiseReduction3D::with_level(level);
        fn disable_flip() => builtin::ImageFlipCommand::new(command::Flip::Off);
        fn enable_flip() => builtin::ImageFlipCommand::new(command::Flip::On);
        fn enable_horizontal_flip() => builtin::ImageMirrorCommand::new(true);
        fn set_flip_both() => command::ImageFlipCombinedCommand::new(command::ImageFlipMode::Both);
        fn set_flip_mode(mode: command::ImageFlipMode) => command::ImageFlipCombinedCommand::new(mode);
        fn freeze_on() => command::ImageFreeze::on();
        fn freeze_off() => command::ImageFreeze::off();
        fn set_picture_effect(mode: command::PictureEffectMode) => command::PictureEffectCommand::new(mode);
    }
}

impl DynMenu for DynSessionCamera {
    inquiry_methods! {
        fn status() -> bool = command::MenuOpenCloseInquiry;
    }
    plain_methods! {
        fn display(on: bool) => command::SetMenuDisplay::new(on);
        fn navigate(direction: command::MenuDirection) => command::MenuNavigate::new(direction);
        fn select() => command::PerformMenuAction::new(command::MenuAction::Select);
        fn cancel() => command::PerformMenuAction::new(command::MenuAction::Cancel);
        fn direct(control1: u8, control2: u8) => command::DirectMenuControl::new(control1, control2);
        fn toggle_display() => command::DirectMenuControl::open_close();
    }
}

impl DynAdvanced for DynSessionCamera {
    inquiry_methods! {
        fn night_day_mode() -> bool = command::NightDayModeInquiry;
        fn standby_enabled() -> bool = command::StandbyInquiry;
        fn digital_ptz_enabled() -> bool = command::DigitalPtzInquiry;
        fn auto_trace_enabled() -> bool = command::AutoTraceInquiry;
        fn focus_unlock() -> bool = command::FocusUnlockInquiry;
        fn broadcast_domain() -> types::BroadcastDomain = command::BroadcastDomainInquiry;
        fn usb_audio_enabled() -> bool = command::UsbAudioInquiry;
        fn two_tone_mode_enabled() -> bool = command::TwoToneModeInquiry;
        fn digital_mode_enabled() -> bool = command::DigitalInquiry;
    }
    plain_methods! {
        fn multicast_on() => command::MulticastStreaming::On;
        fn multicast_off() => command::MulticastStreaming::Off;
        fn set_ndi_quality(quality: types::NdiQuality) => command::SetNdiQuality::new(quality);
        fn usb_audio_on() => command::UsbAudio::On;
        fn usb_audio_off() => command::UsbAudio::Off;
        fn set_variable_speed_mode(mode: command::VariableSpeedMode) => command::SetVariableSpeedMode::new(mode);
    }
}

impl DynTally for DynSessionCamera {
    inquiry_methods! {
        fn status() -> command::TallyStatusState = command::TallyStatusInquiry;
        fn red_status() -> bool = command::TallyRedInquiry;
        fn green_status() -> bool = command::TallyGreenInquiry;
        fn auto_adjust_enabled() -> bool = command::TallyAutoAdjustInquiry;
    }
    plain_methods! {
        fn red_on() => command::TallyRedOn::new();
        fn red_off() => command::TallyRedOff::new();
        fn bright_lo() => command::TallyBrightLo::new();
        fn bright_hi() => command::TallyBrightHi::new();
        fn green_on() => command::TallyGreenOn::new();
        fn green_off() => command::TallyGreenOff::new();
        fn flash() => command::TallyFlash::new();
        fn on() => command::TallyOn::new();
        fn off() => command::TallyOff::new();
    }
}

impl DynNdFilter for DynSessionCamera {
    inquiry_methods! {
        fn position() -> command::NdFilterPosition = command::NdFilterInquiry;
        fn preset() -> types::NdFilterPreset = command::NdFilterPresetInquiry;
    }
    plain_methods! {
        fn set_mode(mode: command::NdFilterMode) => command::NdFilterModeCommand::new(mode);
        fn auto_on() => command::AutoNdCommand::new(true);
        fn auto_off() => command::AutoNdCommand::new(false);
    }
    fn set_value(&self, value: u16) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        targeted_result(
            self,
            command::NdFilterValue::new(value).map(builtin::NdFilterDirect::new),
        )
    }

    fn set_stops(&self, stops: f32) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        targeted_result(
            self,
            command::NdFilterValue::from_stops(stops).map(builtin::NdFilterDirect::new),
        )
    }

    fn step_up(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        targeted(self, builtin::NdFilterStepUp::new())
    }

    fn step_down(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
        targeted(self, builtin::NdFilterStepDown::new())
    }
}

impl DynMotionSync for DynSessionCamera {
    inquiry_methods! {
        fn mode() -> command::MotionSyncMode = command::MotionSyncModeInquiry;
        fn preset() -> command::MotionSyncPreset = command::MotionSyncPresetInquiry;
    }
    plain_methods! {
        fn set_mode(mode: command::MotionSyncMode) => command::SetMotionSyncMode::new(mode);
    }

    fn set_preset(&self, speed: u8) -> DynFuture<'_, Result<(), Error>> {
        plain_result(self, command::SetMotionSyncPreset::new(speed))
    }

    fn set_speed(&self, speed: types::MotionSyncSpeed) -> DynFuture<'_, Result<(), Error>> {
        plain_result(self, command::SetMotionSyncPreset::new(speed.value()))
    }
}

impl DynMotion for DynSessionCamera {
    fn stop_all_motion(&self) -> DynFuture<'_, Result<(), Error>> {
        self.stop_all_motion()
    }

    fn is_moving(&self) -> DynFuture<'_, Result<bool, Error>> {
        DynSessionCamera::is_moving(self, MotionQuery::default())
    }

    fn is_moving_axes(&self, query: MotionQuery) -> DynFuture<'_, Result<bool, Error>> {
        DynSessionCamera::is_moving(self, query)
    }

    fn wait_until_idle(&self, wait: IdleWait) -> DynFuture<'_, Result<(), Error>> {
        self.wait_until_idle(wait)
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use std::collections::HashSet;

    use crate::command::{
        inquiry_structs::{BuiltinInquiryQuery, BUILTIN_INQUIRIES, BUILTIN_INQUIRY_ACCESSORS},
        semantics::{BuiltinCommand, BuiltinRequestClass},
        surface::{surface_entry, StaticSurfaceDisposition},
    };

    use super::{
        DYN_NOUN_CONVENIENCE_METHODS, DYN_NOUN_CONVENIENCE_METHOD_COUNT, DYN_NOUN_COUNT,
        DYN_NOUN_INQUIRY_METHOD_COUNT, DYN_NOUN_TARGET_METHOD_COUNT,
    };

    #[test]
    fn declared_convenience_wrappers_exist_and_are_disjoint_from_the_ledger() {
        let source = include_str!("nouns.rs");

        assert_eq!(
            DYN_NOUN_CONVENIENCE_METHODS.len(),
            DYN_NOUN_CONVENIENCE_METHOD_COUNT,
        );

        let mut ledger_methods = HashSet::new();
        for command in BuiltinCommand::ALL {
            if let StaticSurfaceDisposition::Noun { method, .. } =
                surface_entry(*command).disposition
            {
                ledger_methods.insert(method);
            }
        }

        let mut declared = HashSet::new();
        for (noun, method) in DYN_NOUN_CONVENIENCE_METHODS {
            assert!(
                declared.insert((*noun, *method)),
                "duplicate convenience entry {noun}::{method}",
            );
            assert!(
                source.contains(&format!("pub trait {noun}:")),
                "convenience entry names an unknown dynamic noun {noun}",
            );
            assert!(
                source.contains(&format!("fn {method}(")),
                "convenience method {noun}::{method} is not declared",
            );
            // A convenience wrapper must never shadow a ledger spelling: that
            // would let a command row silently lose its own method.
            assert!(
                !ledger_methods.contains(method),
                "convenience method {noun}::{method} collides with a ledger row spelling",
            );
        }
    }

    #[test]
    fn dynamic_command_counts_follow_static_surface_ledger() {
        // The public trybuild fixture type-checks every dynamic method.  Keep
        // its expected class totals anchored here to the authoritative
        // command ledger rather than relying only on dynamic constants.
        let mut target_facing = 0;
        let mut target_plain = 0;
        let mut target_applied_only = 0;
        let mut target_targeted = 0;
        let mut all_plain = 0;
        let mut all_applied_only = 0;
        let mut all_targeted = 0;
        let mut noun_names = HashSet::new();
        let mut commands = HashSet::new();
        let mut exceptions = Vec::new();

        for command in BuiltinCommand::ALL {
            assert!(commands.insert(*command), "duplicate command ledger row");
            let entry = surface_entry(*command);

            match entry.class {
                BuiltinRequestClass::Plain { .. } => all_plain += 1,
                BuiltinRequestClass::AppliedOnly { .. } => all_applied_only += 1,
                BuiltinRequestClass::Targeted { .. } => all_targeted += 1,
            }

            match entry.disposition {
                StaticSurfaceDisposition::Noun { noun, .. } => {
                    target_facing += 1;
                    noun_names.insert(noun);
                    match entry.class {
                        BuiltinRequestClass::Plain { .. } => target_plain += 1,
                        BuiltinRequestClass::AppliedOnly { .. } => target_applied_only += 1,
                        BuiltinRequestClass::Targeted { .. } => target_targeted += 1,
                    }
                }
                StaticSurfaceDisposition::BroadcastHandshake { .. }
                | StaticSurfaceDisposition::InternalCancellation { .. } => {
                    exceptions.push(*command);
                }
            }
        }

        assert_eq!(
            exceptions,
            vec![
                BuiltinCommand::AddressSet,
                BuiltinCommand::InterfaceClear,
                BuiltinCommand::CommandCancel,
            ]
        );

        // Every total below is derived from `BuiltinCommand::ALL`; none of
        // them is written down, so adding or removing a ledger row moves the
        // declared projection sizes rather than breaking a literal.
        assert_eq!(noun_names.len(), DYN_NOUN_COUNT);
        assert_eq!(DYN_NOUN_TARGET_METHOD_COUNT, target_facing);
        assert_eq!(target_facing + exceptions.len(), BuiltinCommand::ALL.len());
        assert_eq!(
            all_plain + all_applied_only + all_targeted,
            BuiltinCommand::ALL.len()
        );
        // Both broadcast handshakes and the cancellation primitive are plain
        // rows, so the noun split moves only the plain class.
        assert_eq!(
            (
                target_plain + exceptions.len(),
                target_applied_only,
                target_targeted
            ),
            (all_plain, all_applied_only, all_targeted)
        );
    }

    #[test]
    fn dynamic_inquiry_count_follows_generated_typed_accessor_ledger() {
        // The generated inquiry table is shared by the static accessors; the
        // dynamic fixture type-checks the corresponding erased response types.
        let typed_queryable = BUILTIN_INQUIRIES
            .iter()
            .filter(|metadata| {
                !matches!(metadata.query, BuiltinInquiryQuery::DecodeOnly) && metadata.typed
            })
            .count();
        assert_eq!(typed_queryable, BUILTIN_INQUIRY_ACCESSORS.len());
        assert_eq!(
            DYN_NOUN_INQUIRY_METHOD_COUNT,
            BUILTIN_INQUIRY_ACCESSORS.len()
        );

        let mut mappings = HashSet::new();
        for accessor in BUILTIN_INQUIRY_ACCESSORS {
            assert!(
                mappings.insert((accessor.trait_name, accessor.method)),
                "duplicate inquiry mapping {}::{}",
                accessor.trait_name,
                accessor.method,
            );
            let metadata = BUILTIN_INQUIRIES
                .iter()
                .find(|metadata| metadata.name == accessor.command.name())
                .unwrap_or_else(|| {
                    panic!("missing inquiry metadata for {}", accessor.command.name())
                });
            assert!(metadata.typed, "{} must be typed", metadata.name);
            assert!(
                !matches!(metadata.query, BuiltinInquiryQuery::DecodeOnly),
                "{} must be queryable",
                metadata.name,
            );
        }

        let mut untyped_queryable = BUILTIN_INQUIRIES
            .iter()
            .filter(|metadata| {
                !matches!(metadata.query, BuiltinInquiryQuery::DecodeOnly) && !metadata.typed
            })
            .map(|metadata| metadata.name)
            .collect::<Vec<_>>();
        untyped_queryable.sort_unstable();
        assert_eq!(untyped_queryable, ["DefogModeInquiry", "NrSpeedInquiry"]);
    }
}
