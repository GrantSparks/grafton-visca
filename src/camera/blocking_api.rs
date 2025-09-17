//! Blocking-specific wrapper providing direct `Result<T, E>` returns.
//!
//! This module provides a thin wrapper around `Camera<Blocking, _, _, _>` that
//! converts the `Ready<T>` futures to direct `Result<T, E>` values, restoring
//! traditional blocking ergonomics while maintaining the unified Mode-generic design.

use crate::{
    camera::Camera,
    mode::{Blocking, BlockingFutureExt},
    Error,
};
use std::ops::Deref;

/// Zero-cost wrapper for blocking cameras providing direct method access.
///
/// This type wraps a `Camera<Blocking, P, Tr, ()>` and provides methods that
/// return `Result<T, Error>` directly instead of `Ready<Result<T, Error>>`,
/// eliminating the need for `.block()` calls at every usage site.
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{
///     BlockingClient, CameraBuilder, Error,
///     camera::profiles::PtzOpticsG2,
///     transport::Transport,
///     PowerControl, ZoomControl,
/// };
///
/// fn main() -> Result<(), Error> {
///     let transport = Transport::tcp()
///         .address("192.168.0.110:5678")
///         .connect_blocking()?;
///
///     // Create wrapped camera for ergonomic blocking API
///     let camera = BlockingClient::new::<PtzOpticsG2, _>(transport)?;
///
///     // Direct Result<T, Error> returns - no .block() needed!
///     camera.power_on()?;
///     camera.zoom_stop()?;
///     camera.zoom_absolute(0.5.into())?;
///
///     Ok(())
/// }
/// ```
#[repr(transparent)]
#[derive(Debug)]
pub struct BlockingClient<P, Tr>
where
    P: crate::capabilities::Profile,
{
    inner: Camera<Blocking, P, Tr, ()>,
}

impl<P, Tr> BlockingClient<P, Tr>
where
    P: crate::capabilities::Profile,
{
    /// Create a new blocking camera wrapper from a transport.
    pub fn new(transport: Tr) -> Result<Self, Error>
    where
        P: Default,
        Tr: crate::transport::BlockingTransport
            + crate::transport::HasTransportConfig
            + Send
            + 'static,
    {
        Ok(Self {
            inner: Camera::<Blocking, P, Tr, ()>::new_blocking(transport)?,
        })
    }

    /// Create a new blocking camera wrapper from a transport with explicit protocol style.
    pub fn new_with_style(
        transport: Tr,
        protocol_style: crate::capabilities::ProtocolStyle,
    ) -> Result<Self, Error>
    where
        P: Default,
        Tr: crate::transport::BlockingTransport
            + crate::transport::HasTransportConfig
            + Send
            + 'static,
    {
        Ok(Self {
            inner: Camera::<Blocking, P, Tr, ()>::new_blocking_with_style(
                transport,
                protocol_style,
            )?,
        })
    }

    /// Convert from an existing blocking camera.
    pub fn from_camera(camera: Camera<Blocking, P, Tr, ()>) -> Self {
        Self { inner: camera }
    }

    /// Get the inner camera for advanced operations.
    pub fn into_inner(self) -> Camera<Blocking, P, Tr, ()> {
        self.inner
    }

    /// Get a reference to the inner camera.
    pub fn inner(&self) -> &Camera<Blocking, P, Tr, ()> {
        &self.inner
    }

    /// Get a mutable reference to the inner camera for advanced operations.
    pub fn inner_mut(&mut self) -> &mut Camera<Blocking, P, Tr, ()> {
        &mut self.inner
    }

    /// Wait for pan/tilt movement to complete.
    ///
    /// This method polls the camera position until movement stops or timeout occurs.
    pub fn await_pan_tilt_idle(&mut self, timeout: std::time::Duration) -> Result<(), Error>
    where
        P: crate::capabilities::ProfileMetadata + Default,
        Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        self.inner.await_pan_tilt_idle(timeout)
    }

    /// Wait for zoom movement to complete.
    ///
    /// This method polls the camera zoom position until movement stops or timeout occurs.
    pub fn await_zoom_idle(&mut self, timeout: std::time::Duration) -> Result<(), Error>
    where
        P: crate::capabilities::ProfileMetadata + Default,
        Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        self.inner.await_zoom_idle(timeout)
    }

    /// Wait for all camera movements to complete.
    ///
    /// This unified method polls the camera for any ongoing movements (pan/tilt, zoom, focus)
    /// and waits until all movements have stopped or the timeout occurs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::{BlockingClient, camera::profiles::PtzOpticsG2};
    /// use std::time::Duration;
    ///
    /// let camera = Camera::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
    /// camera.zoom().tele()?;
    /// camera.await_idle(Duration::from_secs(10))?;
    /// // Camera has finished zooming
    /// ```
    pub fn await_idle(&mut self, timeout: std::time::Duration) -> Result<(), Error>
    where
        P: crate::capabilities::ProfileMetadata + Default,
        Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        self.inner.await_idle(timeout)
    }

    /// Close the camera connection gracefully.
    ///
    /// This method performs an orderly shutdown of the camera connection,
    /// ensuring any pending operations are completed before closing.
    /// The camera object is consumed and cannot be used after this call.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::{BlockingClient, camera::profiles::PtzOpticsG2};
    ///
    /// let camera = Camera::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
    /// // Use the camera...
    /// camera.close()?;
    /// // Camera is now closed and cannot be used
    /// ```
    pub fn close(self) -> Result<(), Error>
    where
        Tr: crate::transport::BlockingTransport,
    {
        self.inner.close()
    }
}

impl<P, Tr> From<Camera<Blocking, P, Tr, ()>> for BlockingClient<P, Tr>
where
    P: crate::capabilities::Profile,
{
    fn from(camera: Camera<Blocking, P, Tr, ()>) -> Self {
        Self { inner: camera }
    }
}

impl<P, Tr> Deref for BlockingClient<P, Tr>
where
    P: crate::capabilities::Profile,
{
    type Target = Camera<Blocking, P, Tr, ()>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<P, Tr> AsRef<Camera<Blocking, P, Tr, ()>> for BlockingClient<P, Tr>
where
    P: crate::capabilities::Profile,
{
    fn as_ref(&self) -> &Camera<Blocking, P, Tr, ()> {
        &self.inner
    }
}

/// Macro to generate blocking wrapper methods for control traits.
///
/// This macro reduces boilerplate by generating methods that:
/// 1. Call the unified trait method on the inner camera
/// 2. Use `.block()` to convert the `Ready<T>` to `T`
/// 3. Return the result directly
macro_rules! impl_blocking_methods {
    (
        $(
            $(#[$meta:meta])*
            fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret_ok:ty;
        )+
    ) => {
        $(
            $(#[$meta])*
            pub fn $name(&self, $($arg: $ty),*) -> Result<$ret_ok, Error> {
                self.inner.$name($($arg),*).block()
            }
        )+
    };
}

// Import all control traits
use crate::camera::controls::{
    color::ColorControl,
    exposure::{ExposureCompensationControl, ExposureControl},
    focus::FocusControl,
    image_processing::ImageProcessingControl,
    inquiry::{InquiryControl, PanTiltInquiryControl},
    menu::{DirectMenuControl, MenuControl},
    motion_sync::MotionSyncControl,
    nd_filter::NdFilterControl,
    pan_tilt::PanTiltControl,
    power::PowerControl,
    presets::PresetsControl,
    streaming::StreamingControl,
    system::SystemControl,
    tally::TallyControl,
    variable_speed::VariableSpeedControl,
    white_balance::WhiteBalanceControl,
    zoom::ZoomControl,
};

// ============================================================================
// ZoomControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ZoomControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
{
    impl_blocking_methods! {
        /// Stop zoom movement.
        fn zoom_stop() -> ();

        /// Start zooming in at standard speed.
        fn zoom_tele_std() -> ();

        /// Start zooming out at standard speed.
        fn zoom_wide_std() -> ();

        /// Start zooming in at variable speed.
        fn zoom_tele_variable(speed: crate::command::zoom::ZoomSpeed) -> ();

        /// Start zooming out at variable speed.
        fn zoom_wide_variable(speed: crate::command::zoom::ZoomSpeed) -> ();

        /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
        fn zoom_absolute(position: crate::units::Normalized) -> ();

        /// Set zoom to a specific position value.
        fn zoom_position(position: crate::types::ZoomPosition) -> ();

        /// Set digital zoom on or off.
        fn set_digital_zoom(enabled: bool) -> ();
    }
}

// ============================================================================
// PowerControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: PowerControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::power::Power,
{
    impl_blocking_methods! {
        /// Power on the camera.
        fn power_on() -> ();

        /// Power off the camera.
        fn power_off() -> ();
    }
}

// ============================================================================
// FocusControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: FocusControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::focus::Focus,
{
    impl_blocking_methods! {
        /// Set focus to auto mode.
        fn focus_auto() -> ();

        /// Set focus to manual mode.
        fn focus_manual() -> ();

        /// Focus near at specified speed.
        fn focus_near(speed: crate::types::SpeedLevel) -> ();

        /// Focus far at specified speed.
        fn focus_far(speed: crate::types::SpeedLevel) -> ();

        /// Stop focus movement.
        fn focus_stop() -> ();

        /// Trigger one-push auto focus.
        fn focus_one_push() -> ();

        /// Set focus position.
        fn set_focus(position: crate::types::FocusPosition) -> ();

        /// Set focus to infinity.
        fn focus_infinity() -> ();

        /// Enable focus lock.
        fn enable_focus_lock() -> ();

        /// Disable focus lock.
        fn disable_focus_lock() -> ();

        /// Push AF press.
        fn push_af_press() -> ();

        /// Push AF release.
        fn push_af_release() -> ();

        /// Set focus zone.
        fn set_focus_zone(zone: crate::command::focus::FocusZone) -> ();

        /// Set auto focus sensitivity.
        fn set_auto_focus_sensitivity(sensitivity: crate::command::focus::AutoFocusSensitivity) -> ();

        /// Set focus near limit.
        fn set_focus_near_limit(position: crate::types::FocusPosition) -> ();
    }
}

// ============================================================================
// ExposureControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ExposureControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::exposure::Exposure,
{
    impl_blocking_methods! {
        /// Set exposure mode.
        fn set_exposure_mode(mode: crate::command::exposure::ExposureMode) -> ();

        /// Set exposure to auto mode.
        fn exposure_auto() -> ();

        /// Set exposure to manual mode.
        fn exposure_manual() -> ();

        /// Set exposure to shutter priority mode.
        fn exposure_shutter_priority() -> ();

        /// Set exposure to iris priority mode.
        fn exposure_iris_priority() -> ();

        /// Set exposure to bright mode.
        fn exposure_bright_mode() -> ();

        /// Set iris level.
        fn set_iris(level: crate::types::IrisLevel) -> ();

        /// Reset iris to default.
        fn reset_iris() -> ();

        /// Increase iris (open).
        fn increase_iris() -> ();

        /// Decrease iris (close).
        fn decrease_iris() -> ();

        /// Set brightness level.
        fn set_brightness(level: crate::types::BrightnessLevel) -> ();

        /// Reset brightness to default.
        fn reset_brightness() -> ();

        /// Increase brightness.
        fn increase_brightness() -> ();

        /// Decrease brightness.
        fn decrease_brightness() -> ();

        /// Enable/disable backlight.
        fn set_backlight(enabled: bool) -> ();

        /// Set gain level.
        fn set_gain(gain: crate::types::GainLevel) -> ();

        /// Reset gain to default.
        fn reset_gain() -> ();

        /// Increase gain.
        fn increase_gain() -> ();

        /// Decrease gain.
        fn decrease_gain() -> ();

        /// Set gain limit.
        fn set_gain_limit(limit: crate::types::GainLimit) -> ();

        /// Set dynamic range level.
        fn set_dynamic_range(level: crate::types::DynamicRangeLevel) -> ();

        // Note: set_color_temperature is implemented via ColorControl

        /// Set shutter speed.
        fn set_shutter_speed(speed: crate::types::ShutterSpeed) -> ();

        /// Reset shutter speed to default.
        fn reset_shutter_speed() -> ();

        /// Increase shutter speed.
        fn increase_shutter_speed() -> ();

        /// Decrease shutter speed.
        fn decrease_shutter_speed() -> ();

        /// Enable spotlight.
        fn enable_spotlight() -> ();

        /// Disable spotlight.
        fn disable_spotlight() -> ();

        /// Enable auto slow shutter.
        fn enable_auto_slow_shutter() -> ();

        /// Disable auto slow shutter.
        fn disable_auto_slow_shutter() -> ();

        /// Set brightness directly.
        fn set_brightness_direct(level: crate::types::BrightnessLevel) -> ();
    }
}

// ============================================================================
// ExposureCompensationControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ExposureCompensationControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default,
{
    impl_blocking_methods! {
        /// Enable exposure compensation.
        fn enable_exposure_compensation() -> ();

        /// Disable exposure compensation.
        fn disable_exposure_compensation() -> ();

        /// Reset exposure compensation.
        fn reset_exposure_compensation() -> ();

        /// Increase exposure compensation.
        fn increase_exposure_compensation() -> ();

        /// Decrease exposure compensation.
        fn decrease_exposure_compensation() -> ();

        /// Set exposure compensation level.
        fn set_exposure_compensation_level(level: i8) -> ();
    }
}

// ============================================================================
// PanTiltControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: PanTiltControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::pan_tilt::PanTilt,
{
    impl_blocking_methods! {
        /// Stop all pan/tilt movement.
        fn pan_tilt_stop() -> ();

        /// Move to home position (0, 0).
        fn pan_tilt_home() -> ();

        /// Move to absolute pan/tilt position in degrees.
        fn pan_tilt_absolute(pan: crate::units::Degrees, tilt: crate::units::Degrees, speed: crate::types::SpeedLevel) -> ();

        /// Move relative to current position in degrees.
        fn pan_tilt_relative(pan: crate::units::Degrees, tilt: crate::units::Degrees, speed: crate::types::SpeedLevel) -> ();

        /// Move pan/tilt in a specific direction.
        fn pan_tilt_move(direction: crate::command::pan_tilt::PanTiltDirection, pan_speed: crate::types::PanSpeed, tilt_speed: crate::types::TiltSpeed) -> ();

        /// Reset pan/tilt to default position.
        fn pan_tilt_reset() -> ();

        /// Set pan/tilt limit at a specific corner.
        fn pan_tilt_limit_set(corner: crate::command::pan_tilt::PanTiltLimitCorner, pan: crate::types::PanPosition, tilt: crate::types::TiltPosition) -> ();

        /// Clear pan/tilt limit for a specific corner.
        fn pan_tilt_limit_clear(corner: crate::command::pan_tilt::PanTiltLimitCorner) -> ();
    }
}

// ============================================================================
// PresetsControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: PresetsControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::presets::Presets,
{
    impl_blocking_methods! {
        /// Recall a preset position.
        fn preset_recall(preset: crate::command::preset::PresetNumber) -> ();

        /// Set (save) current position as preset.
        fn preset_set(preset: crate::command::preset::PresetNumber) -> ();

        /// Reset (clear) a preset.
        fn preset_reset(preset: crate::command::preset::PresetNumber) -> ();
    }
}

// ============================================================================
// WhiteBalanceControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: WhiteBalanceControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::white_balance::WhiteBalance,
{
    impl_blocking_methods! {
        /// Set white balance mode.
        fn set_white_balance_mode(mode: crate::command::white_balance::WhiteBalanceMode) -> ();

        /// Set white balance to auto mode.
        fn white_balance_auto() -> ();

        /// Set indoor white balance preset.
        fn white_balance_indoor() -> ();

        /// Set outdoor white balance preset.
        fn white_balance_outdoor() -> ();

        /// Set one-push white balance mode.
        fn white_balance_one_push() -> ();

        /// Set auto tracking white balance.
        fn white_balance_atw() -> ();

        /// Set manual white balance mode.
        fn white_balance_manual() -> ();

        /// Set color temperature white balance mode.
        fn white_balance_color_temperature() -> ();

        /// Set AWB sensitivity level.
        fn set_awb_sensitivity(sensitivity: crate::command::white_balance::AutoWhiteBalanceSensitivity) -> ();
    }
}

// ============================================================================
// MenuControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: MenuControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::MenuCapability,
{
    impl_blocking_methods! {
        /// Set menu display on/off.
        fn set_menu_display(display: bool) -> ();

        /// Navigate menu.
        fn menu_navigate(direction: crate::command::menu::MenuDirection) -> ();

        /// Perform menu action.
        fn menu_action(action: crate::command::menu::MenuAction) -> ();
    }
}

// ============================================================================
// DirectMenuControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: DirectMenuControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::MenuCapability,
{
    impl_blocking_methods! {
        /// Direct menu control command.
        fn direct_menu_control(control1: u8, control2: u8) -> ();

        /// Toggle menu display.
        fn toggle_menu() -> ();
    }
}

// ============================================================================
// SystemControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: SystemControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default,
{
    impl_blocking_methods! {
        /// Trigger address assignment.
        fn trigger_address_assignment() -> ();

        /// Clear interface.
        fn interface_clear() -> ();

        /// Cancel a command.
        fn cancel_command(socket: crate::ViscaSocket) -> ();
    }
}

// ============================================================================
// TallyControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: TallyControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default,
{
    impl_blocking_methods! {
        /// Turn on red tally light.
        fn tally_red_on() -> ();

        /// Turn off red tally light.
        fn tally_red_off() -> ();

        /// Set tally brightness to low.
        fn tally_bright_lo() -> ();

        /// Set tally brightness to high.
        fn tally_bright_hi() -> ();

        /// Turn on green tally light.
        fn tally_green_on() -> ();

        /// Turn off green tally light.
        fn tally_green_off() -> ();

        /// Flash tally light.
        fn tally_flash() -> ();

        /// Turn on tally light.
        fn tally_on() -> ();

        /// Turn off tally light.
        fn tally_off() -> ();

        /// Get tally status (red and green states).
        fn get_tally_status() -> crate::command::typed::TallyStatusState;

        /// Get green tally status (FR7 specific).
        fn get_green_tally_status() -> bool;
    }
}

// ============================================================================
// ColorControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ColorControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default,
{
    impl_blocking_methods! {
        /// Trigger one-push white balance.
        fn one_push_trigger() -> ();

        /// Set color temperature.
        fn set_color_temperature(temp: crate::types::ColorTemp) -> ();

        /// Reset color temperature.
        fn reset_color_temperature() -> ();

        /// Increase color temperature.
        fn increase_color_temperature() -> ();

        /// Decrease color temperature.
        fn decrease_color_temperature() -> ();

        /// Set or query color temperature.
        fn color_temperature(temp: Option<crate::types::ColorTemp>) -> ();

        /// Set red gain.
        fn set_red_gain(gain: crate::types::RedChannel) -> ();

        /// Control red gain.
        fn red_gain(command: crate::command::color::RedGain) -> ();

        /// Set blue gain.
        fn set_blue_gain(gain: crate::types::BlueChannel) -> ();

        /// Control blue gain.
        fn blue_gain(command: crate::command::color::BlueGain) -> ();

        /// Set red tuning.
        fn set_red_tuning(tuning: crate::types::RedTuning) -> ();

        /// Set blue tuning.
        fn set_blue_tuning(tuning: crate::types::BlueTuning) -> ();
    }
}

// ============================================================================
// ImageProcessingControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ImageProcessingControl<Mode = Blocking>,
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::image_processing::ImageProcessing,
{
    impl_blocking_methods! {
        /// Enable image flip.
        fn enable_flip() -> ();

        /// Disable image flip.
        fn disable_flip() -> ();

        /// Enable horizontal flip.
        fn enable_horizontal_flip() -> ();

        /// Disable horizontal flip.
        fn disable_horizontal_flip() -> ();

        /// Set contrast level.
        fn set_contrast(level: crate::types::ContrastLevel) -> ();

        /// Set sharpness level.
        fn set_sharpness(level: crate::types::SharpnessLevel) -> ();

        /// Set sharpness mode.
        fn set_sharpness_mode(mode: crate::command::SharpnessMode) -> ();

        /// Reset sharpness.
        fn reset_sharpness() -> ();

        /// Increase sharpness.
        fn increase_sharpness() -> ();

        /// Decrease sharpness.
        fn decrease_sharpness() -> ();

        /// Set saturation level.
        fn set_saturation(level: crate::types::SaturationLevel) -> ();

        /// Set hue level.
        fn set_hue(level: crate::types::HueLevel) -> ();

        /// Set 2D noise reduction level.
        fn set_noise_reduction_2d(level: crate::types::NoiseReduction2DLevel) -> ();

        /// Disable 2D noise reduction.
        fn disable_noise_reduction_2d() -> ();

        /// Set 3D noise reduction level.
        fn set_noise_reduction_3d(level: crate::types::NoiseReduction3DLevel) -> ();

        /// Disable 3D noise reduction.
        fn disable_noise_reduction_3d() -> ();

        /// Set image flip mode.
        fn set_image_flip(mode: crate::command::image::ImageFlipMode) -> ();

        /// Set luminance level.
        fn set_luminance(level: crate::types::LuminanceLevel) -> ();

        /// Enable freeze frame.
        fn enable_freeze() -> ();

        /// Disable freeze frame.
        fn disable_freeze() -> ();

        /// Enable black and white mode.
        fn enable_black_white() -> ();

        /// Disable black and white mode.
        fn disable_black_white() -> ();

        /// Set picture effect mode.
        fn set_picture_effect(mode: crate::command::resolution::PictureEffectMode) -> ();
    }
}

// ============================================================================
// StreamingControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: StreamingControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default,
{
    impl_blocking_methods! {
        /// Enable multicast streaming.
        fn enable_multicast() -> ();

        /// Disable multicast streaming.
        fn disable_multicast() -> ();

        /// Set NDI quality.
        fn set_ndi_quality(quality: crate::types::NdiQuality) -> ();
    }
}

// ============================================================================
// VariableSpeedControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: VariableSpeedControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::variable_speed::VariableSpeed,
{
    impl_blocking_methods! {
        /// Set variable speed mode.
        fn set_variable_speed_mode(mode: crate::command::variable_speed::VariableSpeedMode) -> ();
    }
}

// ============================================================================
// MotionSyncControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: MotionSyncControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::motion_sync::MotionSync,
{
    impl_blocking_methods! {
        /// Set motion sync mode.
        fn set_motion_sync_mode(mode: crate::command::system::MotionSyncMode) -> ();

        /// Set motion sync speed.
        fn set_motion_sync_speed(speed: crate::types::MotionSyncSpeed) -> ();

        /// Set motion sync preset speed.
        fn set_motion_sync_preset_speed(speed: crate::command::system::MotionSyncPreset) -> ();

        /// Get motion sync mode.
        fn get_motion_sync_mode() -> crate::command::system::MotionSyncMode;

        /// Get motion sync speed.
        fn get_motion_sync_speed() -> crate::command::system::MotionSyncPreset;
    }
}

// ============================================================================
// NdFilterControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NdFilterControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::nd_filter::NdFilter,
{
    impl_blocking_methods! {
        /// Set ND filter mode.
        fn set_nd_filter_mode(mode: crate::command::nd_filter::NdFilterMode) -> ();

        /// Set ND filter value.
        fn set_nd_filter_value(value: u16) -> ();

        /// Set ND filter in f-stops.
        fn set_nd_filter_stops(stops: f32) -> ();

        /// Step ND filter.
        fn step_nd_filter(direction: crate::command::nd_filter::NdFilterStep) -> ();

        /// Enable/disable auto ND.
        fn set_auto_nd(enabled: bool) -> ();

        /// Get ND filter value.
        fn get_nd_filter() -> u8;
    }
}

// ============================================================================
// InquiryControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: InquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default,
{
    impl_blocking_methods! {
        /// Get power state.
        fn get_power_state() -> bool;

        /// Get zoom position.
        fn get_zoom_position() -> crate::types::ZoomPosition;

        /// Get focus position.
        fn get_focus_position() -> u16;

        /// Get focus near limit.
        fn get_focus_near_limit() -> u16;

        /// Get focus zone.
        fn get_focus_zone() -> crate::command::focus::FocusZone;

        /// Get exposure mode.
        fn get_exposure_mode() -> crate::command::exposure::ExposureMode;

        /// Get exposure compensation.
        fn get_exposure_compensation() -> i8;

        /// Get exposure compensation enabled status.
        fn get_exposure_compensation_enabled() -> bool;

        /// Get iris value.
        fn get_iris() -> u8;

        /// Get shutter value.
        fn get_shutter() -> u16;

        /// Get gain value.
        fn get_gain() -> u8;

        /// Get gain limit.
        fn get_gain_limit() -> u8;

        /// Get white balance mode.
        fn get_white_balance_mode() -> crate::command::white_balance::WhiteBalanceMode;

        /// Get red gain.
        fn get_red_gain() -> i8;

        /// Get blue gain.
        fn get_blue_gain() -> i8;

        /// Get red tuning.
        fn get_red_tuning() -> u8;

        /// Get blue tuning.
        fn get_blue_tuning() -> u8;

        /// Get color temperature.
        fn get_color_temperature() -> u16;

        /// Get gamma value.
        fn get_gamma() -> u8;

        /// Get brightness.
        fn get_brightness() -> u16;

        /// Get sharpness mode.
        fn get_sharpness_mode() -> crate::command::SharpnessMode;

        /// Get saturation.
        fn get_saturation() -> u8;

        /// Get hue.
        fn get_hue() -> u8;

        /// Get black and white mode.
        fn get_black_white() -> bool;

        /// Get resolution.
        fn get_resolution() -> u8;

        /// Get picture effect.
        fn get_picture_effect() -> u8;

        /// Get ND filter position.
        fn get_nd_filter_position() -> u8;

        /// Get camera version information.
        fn get_version() -> crate::command::typed::VersionInfo;

        /// Get backlight enabled status.
        fn get_backlight_enabled() -> bool;

        /// Get image flip state.
        fn get_image_flip() -> crate::command::FlipState;

        /// Get focus mode.
        fn get_focus_mode() -> crate::command::focus::FocusMode;

        /// Get menu status.
        fn get_menu_status() -> bool;

        /// Get tally light status.
        fn get_tally_light_status() -> crate::command::typed::TallyStatusState;

        /// Get night/day mode.
        fn get_night_day_mode() -> bool;

        /// Get flip mode.
        fn get_flip_mode() -> crate::command::FlipState;

        /// Get standby enabled status.
        fn get_standby_enabled() -> bool;

        /// Get iris control status.
        fn get_iris_control() -> bool;

        /// Get defog level.
        fn get_defog_level() -> u8;

        /// Get digital PTZ enabled status.
        fn get_digital_ptz_enabled() -> bool;

        /// Get exposure compensation position.
        fn get_exposure_compensation_position() -> u16;

        /// Get auto trace enabled status.
        fn get_auto_trace_enabled() -> bool;

        /// Get focus unlock status.
        fn get_focus_unlock() -> bool;

        /// Get noise reduction level.
        fn get_noise_reduction_level() -> u8;

        /// Get 2D noise reduction level.
        fn get_noise_reduction_2d() -> u8;

        /// Get 3D noise reduction level.
        fn get_noise_reduction_3d() -> u8;

        /// Get broadcast domain.
        fn get_broadcast_domain() -> u8;

        /// Get noise reduction mode.
        fn get_noise_reduction_mode() -> crate::command::NoiseReductionMode;

        /// Get black and white mode.
        fn get_black_white_mode() -> crate::command::BlackWhiteMode;

        /// Get USB audio enabled status.
        fn get_usb_audio_enabled() -> bool;

        /// Get two-tone mode enabled status.
        fn get_two_tone_mode_enabled() -> bool;

        /// Get ND filter preset.
        fn get_nd_filter_preset() -> u8;

        /// Get digital mode enabled status.
        fn get_digital_mode_enabled() -> bool;

        /// Get tally auto adjust enabled status.
        fn get_tally_auto_adjust_enabled() -> bool;

        // Note: get_motion_sync_mode is implemented via MotionSyncControl
    }
}

// ============================================================================
// PanTiltInquiryControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: PanTiltInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default,
{
    impl_blocking_methods! {
        /// Get pan/tilt position.
        fn get_pan_tilt_position() -> crate::camera::PanTiltPosition;
    }
}
