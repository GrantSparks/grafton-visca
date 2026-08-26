//! Object-safe noun projection for the owner-backed dynamic camera.
//!
//! This module is deliberately a mechanical projection of the closed static
//! surface in [`crate::command::surface`].  Every method routes through the
//! generic preparation/admission methods on [`super::DynSessionCamera`]; the
//! dynamic layer does not duplicate capability or profile validation.

#![cfg(feature = "dyn-api")]
// The 212 noun methods are a mechanical projection of the static command
// ledger. Their shared return and lifecycle contracts are documented on the
// public dynamic module and operation types; repeating per-method prose here
// would duplicate that authority and make the two surfaces drift.
#![allow(missing_docs)]

use crate::{
    camera::{IdleWait, MotionQuery, PanTiltPosition},
    command,
    completion::{AppliedOnly, Targeted},
    request::builtin,
    requests::{Inquiry, OperationCommand, PlainCommand},
    types,
    units::Degrees,
    Error, Result,
};

use super::{DynAppliedOperation, DynFuture, DynSessionCamera, DynTargetedOperation};

/// Number of target-facing built-in command methods in this projection.
pub const DYN_NOUN_TARGET_METHOD_COUNT: usize = 143;

/// Number of typed inquiry methods in this projection.
pub const DYN_NOUN_INQUIRY_METHOD_COUNT: usize = 66;

/// Number of domain nouns (motion is a separate safety/observation view).
pub const DYN_NOUN_COUNT: usize = 14;

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
    fn state(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn off(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe zoom noun.
pub trait DynZoom: Send + Sync {
    fn position(&self) -> DynFuture<'_, Result<types::ZoomPosition, Error>>;
    fn tele(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn wide(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn stop(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn tele_variable(
        &self,
        speed: types::ZoomSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn wide_variable(
        &self,
        speed: types::ZoomSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn set_position(
        &self,
        position: types::ZoomPosition,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn set_digital_zoom(&self, enabled: bool) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe system noun.
pub trait DynSystem: Send + Sync {
    fn version(&self) -> DynFuture<'_, Result<command::VersionInfo, Error>>;
    fn save_settings(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe pan/tilt noun.
pub trait DynPanTilt: Send + Sync {
    fn position(&self) -> DynFuture<'_, Result<PanTiltPosition, Error>>;
    fn home(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn reset(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn move_direction(
        &self,
        direction: command::PanTiltDirection,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn stop(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn limit_set(
        &self,
        corner: command::PanTiltLimitCorner,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn limit_clear(&self, corner: command::PanTiltLimitCorner) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe focus noun.
pub trait DynFocus: Send + Sync {
    fn position(&self) -> DynFuture<'_, Result<types::FocusPosition, Error>>;
    fn mode(&self) -> DynFuture<'_, Result<command::FocusMode, Error>>;
    fn far(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn near(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn far_variable(
        &self,
        speed: command::FocusSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn near_variable(
        &self,
        speed: command::FocusSpeed,
    ) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn stop(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn set_position(
        &self,
        position: types::FocusPosition,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn auto(&self) -> DynFuture<'_, Result<(), Error>>;
    fn manual(&self) -> DynFuture<'_, Result<(), Error>>;
    fn one_push(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn infinity(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn toggle(&self) -> DynFuture<'_, Result<(), Error>>;
    fn snap(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn set_zone(&self, zone: command::FocusZone) -> DynFuture<'_, Result<(), Error>>;
    fn set_sensitivity(
        &self,
        sensitivity: command::AutoFocusSensitivity,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn set_near_limit(&self, position: types::FocusPosition) -> DynFuture<'_, Result<(), Error>>;
    fn set_lock(&self, mode: command::FocusLock) -> DynFuture<'_, Result<(), Error>>;
    fn push_af_press(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn push_af_release(&self) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    fn near_limit(&self) -> DynFuture<'_, Result<types::FocusPosition, Error>>;
    fn zone(&self) -> DynFuture<'_, Result<command::FocusZone, Error>>;
    fn sensitivity(&self) -> DynFuture<'_, Result<command::AutoFocusSensitivity, Error>>;
    fn range(&self) -> DynFuture<'_, Result<command::FocusRange, Error>>;
}

/// Object-safe preset noun.
pub trait DynPresets: Send + Sync {
    fn recall(
        &self,
        preset: command::PresetNumber,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn set_recall_speed(
        &self,
        speed: command::PresetRecallSpeed,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn set(&self, preset: command::PresetNumber) -> DynFuture<'_, Result<(), Error>>;
    fn reset(&self, preset: command::PresetNumber) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe exposure noun.
pub trait DynExposure: Send + Sync {
    fn mode(&self) -> DynFuture<'_, Result<command::ExposureMode, Error>>;
    fn set_mode(&self, mode: command::ExposureMode) -> DynFuture<'_, Result<(), Error>>;
    fn shutter(&self) -> DynFuture<'_, Result<types::ShutterSpeed, Error>>;
    fn shutter_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    fn shutter_up(&self) -> DynFuture<'_, Result<(), Error>>;
    fn shutter_down(&self) -> DynFuture<'_, Result<(), Error>>;
    fn shutter_direct(&self, speed: types::ShutterSpeed) -> DynFuture<'_, Result<(), Error>>;
    fn compensation(&self) -> DynFuture<'_, Result<types::ExposureCompensationLevel, Error>>;
    fn compensation_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn compensation_position(
        &self,
    ) -> DynFuture<'_, Result<types::ExposureCompensationPosition, Error>>;
    fn compensation_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn compensation_off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn compensation_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    fn compensation_up(&self) -> DynFuture<'_, Result<(), Error>>;
    fn compensation_down(&self) -> DynFuture<'_, Result<(), Error>>;
    fn compensation_direct(
        &self,
        level: types::ExposureCompensationLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn dynamic_range(&self) -> DynFuture<'_, Result<types::DynamicRangeLevel, Error>>;
    fn set_dynamic_range(
        &self,
        level: types::DynamicRangeLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn iris_control(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn iris(&self) -> DynFuture<'_, Result<types::IrisLevel, Error>>;
    fn iris_reset(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn iris_up(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn iris_down(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn iris_direct(
        &self,
        level: types::IrisLevel,
    ) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn brightness(&self) -> DynFuture<'_, Result<types::BrightnessLevel, Error>>;
    fn brightness_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    fn brightness_up(&self) -> DynFuture<'_, Result<(), Error>>;
    fn brightness_down(&self) -> DynFuture<'_, Result<(), Error>>;
    fn brightness_set(&self, level: types::BrightnessLevel) -> DynFuture<'_, Result<(), Error>>;
    fn brightness_direct(&self, level: types::BrightnessLevel) -> DynFuture<'_, Result<(), Error>>;
    fn gain(&self) -> DynFuture<'_, Result<types::GainLevel, Error>>;
    fn gain_reset(&self) -> DynFuture<'_, Result<(), Error>>;
    fn gain_up(&self) -> DynFuture<'_, Result<(), Error>>;
    fn gain_down(&self) -> DynFuture<'_, Result<(), Error>>;
    fn gain_direct(&self, level: types::GainLevel) -> DynFuture<'_, Result<(), Error>>;
    fn gain_limit(&self) -> DynFuture<'_, Result<types::GainLimit, Error>>;
    fn set_gain_limit(&self, limit: types::GainLimit) -> DynFuture<'_, Result<(), Error>>;
    fn set_anti_flicker(&self, mode: command::AntiFlickerMode) -> DynFuture<'_, Result<(), Error>>;
    fn flicker_mode(&self) -> DynFuture<'_, Result<command::AntiFlickerMode, Error>>;
    fn spotlight_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn spotlight_off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn auto_slow_shutter_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn auto_slow_shutter_off(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe white-balance noun.
pub trait DynWhiteBalance: Send + Sync {
    fn mode(&self) -> DynFuture<'_, Result<command::WhiteBalanceMode, Error>>;
    fn auto(&self) -> DynFuture<'_, Result<(), Error>>;
    fn indoor(&self) -> DynFuture<'_, Result<(), Error>>;
    fn outdoor(&self) -> DynFuture<'_, Result<(), Error>>;
    fn one_push(&self) -> DynFuture<'_, Result<(), Error>>;
    fn atw(&self) -> DynFuture<'_, Result<(), Error>>;
    fn manual(&self) -> DynFuture<'_, Result<(), Error>>;
    fn color_temperature_mode(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_sensitivity(
        &self,
        sensitivity: command::AutoWhiteBalanceSensitivity,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn sensitivity(&self) -> DynFuture<'_, Result<command::AutoWhiteBalanceSensitivity, Error>>;
    fn one_push_trigger(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_red_tuning(&self, level: types::RedTuning) -> DynFuture<'_, Result<(), Error>>;
    fn set_blue_tuning(&self, level: types::BlueTuning) -> DynFuture<'_, Result<(), Error>>;
    fn color_temperature(&self) -> DynFuture<'_, Result<types::ColorTemp, Error>>;
    fn reset_color_temperature(&self) -> DynFuture<'_, Result<(), Error>>;
    fn increase_color_temperature(&self) -> DynFuture<'_, Result<(), Error>>;
    fn decrease_color_temperature(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_color_temperature(
        &self,
        temperature: types::ColorTemp,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn red_gain(&self) -> DynFuture<'_, Result<types::RedChannel, Error>>;
    fn reset_red_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    fn increase_red_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    fn decrease_red_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_red_gain(&self, value: types::RedChannel) -> DynFuture<'_, Result<(), Error>>;
    fn blue_gain(&self) -> DynFuture<'_, Result<types::BlueChannel, Error>>;
    fn reset_blue_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    fn increase_blue_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    fn decrease_blue_gain(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_blue_gain(&self, value: types::BlueChannel) -> DynFuture<'_, Result<(), Error>>;
    fn red_tuning(&self) -> DynFuture<'_, Result<types::RedTuning, Error>>;
    fn blue_tuning(&self) -> DynFuture<'_, Result<types::BlueTuning, Error>>;
}

/// Object-safe image-processing noun.
pub trait DynImage: Send + Sync {
    fn resolution(&self) -> DynFuture<'_, Result<command::ResolutionMode, Error>>;
    fn saturation(&self) -> DynFuture<'_, Result<types::SaturationLevel, Error>>;
    fn set_saturation(&self, level: types::SaturationLevel) -> DynFuture<'_, Result<(), Error>>;
    fn hue(&self) -> DynFuture<'_, Result<types::HueLevel, Error>>;
    fn set_hue(&self, level: types::HueLevel) -> DynFuture<'_, Result<(), Error>>;
    fn luminance(&self) -> DynFuture<'_, Result<types::LuminanceLevel, Error>>;
    fn set_luminance(&self, level: types::LuminanceLevel) -> DynFuture<'_, Result<(), Error>>;
    fn contrast(&self) -> DynFuture<'_, Result<types::ContrastLevel, Error>>;
    fn set_contrast(&self, level: types::ContrastLevel) -> DynFuture<'_, Result<(), Error>>;
    fn gamma(&self) -> DynFuture<'_, Result<types::GammaLevel, Error>>;
    fn set_gamma(&self, level: types::GammaLevel) -> DynFuture<'_, Result<(), Error>>;
    fn sharpness_mode(&self) -> DynFuture<'_, Result<command::SharpnessMode, Error>>;
    fn sharpness_level(&self) -> DynFuture<'_, Result<types::SharpnessLevel, Error>>;
    fn set_sharpness_mode(&self, mode: command::SharpnessMode) -> DynFuture<'_, Result<(), Error>>;
    fn reset_sharpness(&self) -> DynFuture<'_, Result<(), Error>>;
    fn increase_sharpness(&self) -> DynFuture<'_, Result<(), Error>>;
    fn decrease_sharpness(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_sharpness(&self, level: types::SharpnessLevel) -> DynFuture<'_, Result<(), Error>>;
    fn backlight(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn set_backlight(&self, enabled: bool) -> DynFuture<'_, Result<(), Error>>;
    fn noise_reduction_2d(&self) -> DynFuture<'_, Result<types::NoiseReduction2DLevel, Error>>;
    fn set_noise_reduction_2d(
        &self,
        level: types::NoiseReduction2DLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn noise_reduction_3d(&self) -> DynFuture<'_, Result<types::NoiseReduction3DLevel, Error>>;
    fn set_noise_reduction_3d(
        &self,
        level: types::NoiseReduction3DLevel,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn noise_reduction_level(&self) -> DynFuture<'_, Result<types::NoiseReductionLevel, Error>>;
    fn noise_reduction_mode(&self) -> DynFuture<'_, Result<command::NoiseReductionMode, Error>>;
    fn disable_flip(&self) -> DynFuture<'_, Result<(), Error>>;
    fn enable_flip(&self) -> DynFuture<'_, Result<(), Error>>;
    fn enable_horizontal_flip(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_flip_both(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_flip_mode(&self, mode: command::ImageFlipMode) -> DynFuture<'_, Result<(), Error>>;
    fn freeze_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn freeze_off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn flip(&self) -> DynFuture<'_, Result<command::FlipState, Error>>;
    fn flip_mode(&self) -> DynFuture<'_, Result<command::FlipState, Error>>;
    fn black_white(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn black_white_mode(&self) -> DynFuture<'_, Result<command::BlackWhiteMode, Error>>;
    fn picture_effect(&self) -> DynFuture<'_, Result<command::PictureEffectMode, Error>>;
    fn set_picture_effect(
        &self,
        mode: command::PictureEffectMode,
    ) -> DynFuture<'_, Result<(), Error>>;
    fn defog_level(&self) -> DynFuture<'_, Result<types::DefogLevel, Error>>;
}

/// Object-safe tally noun.
pub trait DynTally: Send + Sync {
    fn status(&self) -> DynFuture<'_, Result<command::TallyStatusState, Error>>;
    fn red_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn red_off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn bright_lo(&self) -> DynFuture<'_, Result<(), Error>>;
    fn bright_hi(&self) -> DynFuture<'_, Result<(), Error>>;
    fn green_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn green_off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn flash(&self) -> DynFuture<'_, Result<(), Error>>;
    fn on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn red_status(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn green_status(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn auto_adjust_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
}

/// Object-safe neutral-density filter noun.
pub trait DynNdFilter: Send + Sync {
    fn position(&self) -> DynFuture<'_, Result<command::NdFilterPosition, Error>>;
    fn preset(&self) -> DynFuture<'_, Result<types::NdFilterPreset, Error>>;
    fn set_mode(&self, mode: command::NdFilterMode) -> DynFuture<'_, Result<(), Error>>;
    fn set_value(&self, value: u16) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn step_up(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn step_down(&self) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    fn auto_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn auto_off(&self) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe motion-sync noun.
pub trait DynMotionSync: Send + Sync {
    fn mode(&self) -> DynFuture<'_, Result<command::MotionSyncMode, Error>>;
    fn preset(&self) -> DynFuture<'_, Result<command::MotionSyncPreset, Error>>;
    fn set_mode(&self, mode: command::MotionSyncMode) -> DynFuture<'_, Result<(), Error>>;
    fn set_preset(&self, speed: u8) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe on-screen menu noun.
pub trait DynMenu: Send + Sync {
    fn status(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn display(&self, on: bool) -> DynFuture<'_, Result<(), Error>>;
    fn navigate(&self, direction: command::MenuDirection) -> DynFuture<'_, Result<(), Error>>;
    fn select(&self) -> DynFuture<'_, Result<(), Error>>;
    fn cancel(&self) -> DynFuture<'_, Result<(), Error>>;
    fn direct(&self, control1: u8, control2: u8) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe advanced/vendor noun.
pub trait DynAdvanced: Send + Sync {
    fn night_day_mode(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn standby_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn digital_ptz_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn auto_trace_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn focus_unlock(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn broadcast_domain(&self) -> DynFuture<'_, Result<types::BroadcastDomain, Error>>;
    fn usb_audio_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn two_tone_mode_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn digital_mode_enabled(&self) -> DynFuture<'_, Result<bool, Error>>;
    fn multicast_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn multicast_off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_ndi_quality(&self, quality: types::NdiQuality) -> DynFuture<'_, Result<(), Error>>;
    fn usb_audio_on(&self) -> DynFuture<'_, Result<(), Error>>;
    fn usb_audio_off(&self) -> DynFuture<'_, Result<(), Error>>;
    fn set_variable_speed_mode(
        &self,
        mode: command::VariableSpeedMode,
    ) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe motion safety and observation noun.
pub trait DynMotion: Send + Sync {
    fn stop_all_motion(&self) -> DynFuture<'_, Result<(), Error>>;
    fn is_moving(&self, query: MotionQuery) -> DynFuture<'_, Result<bool, Error>>;
    fn wait_until_idle(&self, wait: IdleWait) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe accessors for all final dynamic nouns.
pub trait DynSessionCameraNouns: Send + Sync {
    fn power(&self) -> &dyn DynPower;
    fn zoom(&self) -> &dyn DynZoom;
    fn system(&self) -> &dyn DynSystem;
    fn pan_tilt(&self) -> &dyn DynPanTilt;
    fn focus(&self) -> &dyn DynFocus;
    fn exposure(&self) -> &dyn DynExposure;
    fn white_balance(&self) -> &dyn DynWhiteBalance;
    fn image(&self) -> &dyn DynImage;
    fn presets(&self) -> &dyn DynPresets;
    fn tally(&self) -> &dyn DynTally;
    fn nd_filter(&self) -> &dyn DynNdFilter;
    fn motion_sync(&self) -> &dyn DynMotionSync;
    fn menu(&self) -> &dyn DynMenu;
    fn advanced(&self) -> &dyn DynAdvanced;
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
}

impl DynMotion for DynSessionCamera {
    fn stop_all_motion(&self) -> DynFuture<'_, Result<(), Error>> {
        self.stop_all_motion()
    }

    fn is_moving(&self, query: MotionQuery) -> DynFuture<'_, Result<bool, Error>> {
        self.is_moving(query)
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

    use super::{DYN_NOUN_COUNT, DYN_NOUN_INQUIRY_METHOD_COUNT, DYN_NOUN_TARGET_METHOD_COUNT};

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

        assert_eq!(BuiltinCommand::ALL.len(), 146);
        assert_eq!(
            exceptions,
            vec![
                BuiltinCommand::AddressSet,
                BuiltinCommand::InterfaceClear,
                BuiltinCommand::CommandCancel,
            ]
        );
        assert_eq!(noun_names.len(), DYN_NOUN_COUNT);
        assert_eq!((target_facing, DYN_NOUN_TARGET_METHOD_COUNT), (143, 143));
        assert_eq!(
            (target_plain, target_applied_only, target_targeted),
            (112, 16, 15)
        );
        assert_eq!((all_plain, all_applied_only, all_targeted), (115, 16, 15));
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
        assert_eq!(typed_queryable, 66);
        assert_eq!(BUILTIN_INQUIRY_ACCESSORS.len(), 66);
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
