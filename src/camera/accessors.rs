//! Accessor structs for noun-based control trait access.
//!
//! This module provides accessor structs that group related control methods
//! under logical nouns, providing a more intuitive API for camera control.
//!
//! Instead of using `camera.get_power_state()`, users can use `camera.power().state()`.

use crate::{
    camera::{
        controls::{
            color::ColorControl,
            exposure::ExposureControl,
            focus::FocusControl,
            inquiry::{InquiryControl, PanTiltInquiryControl},
            menu::MenuControl,
            nd_filter::NdFilterControl,
            pan_tilt::PanTiltControl,
            power::PowerControl,
            presets::PresetsControl,
            system::SystemControl,
            tally::TallyControl,
            white_balance::WhiteBalanceControl,
            zoom::ZoomControl,
        },
        Camera, ViscaClient,
    },
    capabilities::Profile,
    executor::Executor,
    mode::Mode,
    Error,
};

/// Access to power-related controls and inquiries.
#[derive(Debug)]
pub struct PowerAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> PowerAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the current power state.
    pub fn state(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.power_state()
    }

    /// Turn the camera on.
    pub fn on(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PowerControl<Mode = M>,
    {
        self.camera.power_on()
    }

    /// Turn the camera off (standby).
    pub fn off(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PowerControl<Mode = M>,
    {
        self.camera.power_off()
    }

    /// Set power state (true = on, false = off).
    pub fn set(&self, on: bool) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PowerControl<Mode = M>,
    {
        if on {
            self.camera.power_on()
        } else {
            self.camera.power_off()
        }
    }
}

/// Access to zoom-related controls and inquiries.
#[derive(Debug)]
pub struct ZoomAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> ZoomAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the current zoom position.
    pub fn position(&self) -> M::Fut<'_, Result<crate::types::ZoomPosition, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.zoom_position()
    }

    /// Zoom to telephoto (zoom in).
    pub fn tele(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
    {
        self.camera.zoom_tele(None)
    }

    /// Zoom to wide (zoom out).
    pub fn wide(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
    {
        self.camera.zoom_wide(None)
    }

    /// Stop zoom movement.
    pub fn stop(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
    {
        self.camera.zoom_stop()
    }

    /// Set zoom position directly.
    ///
    /// This method accepts any type that can be converted to `ZoomPosition`.
    pub fn set_position<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
        T: TryInto<crate::types::ZoomPosition>,
        T::Error: Into<Error>,
    {
        self.camera.set_zoom(position)
    }

    /// Zoom to telephoto (zoom in) with variable speed.
    ///
    /// Accepts either `SpeedLevel` or `ZoomSpeed` for speed control.
    pub fn tele_variable<S>(&self, speed: S) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
        S: Into<crate::ZoomSpeed>,
    {
        self.camera.zoom_tele(Some(speed.into()))
    }

    /// Zoom to wide (zoom out) with variable speed.
    ///
    /// Accepts either `SpeedLevel` or `ZoomSpeed` for speed control.
    pub fn wide_variable<S>(&self, speed: S) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
        S: Into<crate::ZoomSpeed>,
    {
        self.camera.zoom_wide(Some(speed.into()))
    }

    /// Set zoom to absolute position.
    ///
    /// This method accepts any type that can be converted to `ZoomPosition`.
    pub fn absolute<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
        T: TryInto<crate::types::ZoomPosition>,
        T::Error: Into<Error>,
    {
        self.camera.set_zoom(position)
    }
}

/// Access to system-related controls and inquiries.
#[derive(Debug)]
pub struct SystemAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> SystemAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get camera version information.
    pub fn version(&self) -> M::Fut<'_, Result<crate::command::typed::VersionInfo, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.version()
    }

    /// Clear interface (reset communication).
    pub fn interface_clear(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: SystemControl<Mode = M>,
    {
        self.camera.interface_clear()
    }

    /// Cancel command on specific socket.
    pub fn cancel_command(&self, socket: crate::ViscaSocket) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: SystemControl<Mode = M>,
    {
        self.camera.cancel_command(socket)
    }
}

/// Access to pan/tilt-related controls and inquiries.
#[derive(Debug)]
pub struct PanTiltAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> PanTiltAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the current pan and tilt position.
    pub fn position(&self) -> M::Fut<'_, Result<crate::camera::PanTiltPosition, Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltInquiryControl<Mode = M>,
    {
        self.camera.pan_tilt_position()
    }

    /// Move in a specific direction.
    ///
    /// Accepts high-level `command::pan_tilt::PanTiltDirection` for direction.
    pub fn move_direction(
        &self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.camera.pan_tilt_move(direction, pan_speed, tilt_speed)
    }

    /// Move up.
    pub fn up(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.move_direction(
            crate::command::pan_tilt::PanTiltDirection::Up,
            pan_speed,
            tilt_speed,
        )
    }

    /// Move down.
    pub fn down(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.move_direction(
            crate::command::pan_tilt::PanTiltDirection::Down,
            pan_speed,
            tilt_speed,
        )
    }

    /// Move left.
    pub fn left(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.move_direction(
            crate::command::pan_tilt::PanTiltDirection::Left,
            pan_speed,
            tilt_speed,
        )
    }

    /// Move right.
    pub fn right(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.move_direction(
            crate::command::pan_tilt::PanTiltDirection::Right,
            pan_speed,
            tilt_speed,
        )
    }

    /// Stop pan/tilt movement.
    pub fn stop(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.camera.pan_tilt_stop()
    }

    /// Move to home position.
    pub fn home(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.camera.pan_tilt_home()
    }

    /// Move to absolute position.
    pub fn absolute(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.camera.pan_tilt_absolute(pan, tilt, speed)
    }

    /// Move relative to current position.
    pub fn relative(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    {
        self.camera.pan_tilt_relative(pan, tilt, speed)
    }
}

/// Access to focus-related controls and inquiries.
#[derive(Debug)]
pub struct FocusAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> FocusAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the current focus position.
    pub fn position(&self) -> M::Fut<'_, Result<crate::types::FocusPosition, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.focus_position()
    }

    /// Get the current focus mode.
    pub fn mode(&self) -> M::Fut<'_, Result<crate::command::FocusMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.focus_mode()
    }

    /// Set to auto focus mode.
    pub fn auto(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.focus_auto()
    }

    /// Set to manual focus mode.
    pub fn manual(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.focus_manual()
    }

    /// Focus near.
    pub fn near(&self, speed: crate::types::SpeedLevel) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.focus_near(speed)
    }

    /// Focus far.
    pub fn far(&self, speed: crate::types::SpeedLevel) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.focus_far(speed)
    }

    /// Stop focus movement.
    pub fn stop(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.focus_stop()
    }

    /// Set focus position directly.
    ///
    /// This method accepts any type that can be converted to `FocusPosition`.
    pub fn set_position<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
        T: TryInto<crate::types::FocusPosition>,
        T::Error: Into<Error>,
    {
        self.camera.set_focus(position)
    }

    /// Get focus near limit.
    pub fn near_limit(&self) -> M::Fut<'_, Result<crate::types::FocusPosition, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.focus_near_limit()
    }

    /// Get focus zone.
    pub fn zone(&self) -> M::Fut<'_, Result<crate::command::FocusZone, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.focus_zone()
    }

    /// Enable focus lock to prevent changes.
    pub fn lock(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.enable_focus_lock()
    }

    /// Disable focus lock.
    pub fn unlock(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.disable_focus_lock()
    }

    /// Press Push AF button (temporary auto focus).
    pub fn push_af_press(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.push_af_press()
    }

    /// Release Push AF button.
    pub fn push_af_release(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.push_af_release()
    }

    /// Set the focus zone.
    pub fn set_zone(&self, zone: crate::command::FocusZone) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.set_focus_zone(zone)
    }

    /// Set auto focus sensitivity.
    pub fn set_sensitivity(
        &self,
        sensitivity: crate::command::AutoFocusSensitivity,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.set_auto_focus_sensitivity(sensitivity)
    }

    /// Set the focus near limit.
    ///
    /// This method accepts any type that can be converted to `FocusPosition`.
    pub fn set_near_limit<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
        T: TryInto<crate::types::FocusPosition>,
        T::Error: Into<Error>,
    {
        self.camera.set_focus_near_limit(position)
    }

    /// Trigger one-push auto focus.
    ///
    /// Performs a single auto-focus operation then returns to the previous focus mode.
    pub fn one_push(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    {
        self.camera.focus_one_push()
    }
}

/// Access to exposure-related controls and inquiries.
#[derive(Debug)]
pub struct ExposureAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> ExposureAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the current exposure mode.
    pub fn mode(&self) -> M::Fut<'_, Result<crate::command::ExposureMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.exposure_mode()
    }

    /// Get exposure compensation value.
    pub fn compensation(&self) -> M::Fut<'_, Result<crate::types::ExposureCompensationLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.exposure_compensation()
    }

    /// Check if exposure compensation is enabled.
    pub fn compensation_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.exposure_compensation_enabled()
    }

    /// Get exposure compensation position.
    pub fn compensation_position(
        &self,
    ) -> M::Fut<'_, Result<crate::types::ExposureCompensationPosition, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.exposure_compensation_position()
    }

    /// Get iris value.
    pub fn iris(&self) -> M::Fut<'_, Result<crate::types::IrisLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.iris()
    }

    /// Get shutter speed.
    pub fn shutter(&self) -> M::Fut<'_, Result<crate::types::ShutterSpeed, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.shutter()
    }

    /// Get gain value.
    pub fn gain(&self) -> M::Fut<'_, Result<crate::types::GainLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.gain()
    }

    /// Get gain limit.
    pub fn gain_limit(&self) -> M::Fut<'_, Result<crate::types::GainLimit, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.gain_limit()
    }

    /// Set to auto exposure mode.
    pub fn auto(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ExposureControl<Mode = M>,
    {
        self.camera.exposure_auto()
    }

    /// Set to manual exposure mode.
    pub fn manual(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ExposureControl<Mode = M>,
    {
        self.camera.exposure_manual()
    }

    /// Set to shutter priority mode.
    pub fn shutter_priority(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ExposureControl<Mode = M>,
    {
        self.camera.exposure_shutter_priority()
    }

    /// Set to iris priority mode.
    pub fn iris_priority(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ExposureControl<Mode = M>,
    {
        self.camera.exposure_iris_priority()
    }
}

/// Access to white balance controls and inquiries.
#[derive(Debug)]
pub struct WhiteBalanceAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> WhiteBalanceAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get white balance mode.
    pub fn mode(&self) -> M::Fut<'_, Result<crate::command::WhiteBalanceMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.white_balance_mode()
    }

    /// Get red gain.
    pub fn red_gain(&self) -> M::Fut<'_, Result<crate::types::RedChannel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.red_gain()
    }

    /// Get blue gain.
    pub fn blue_gain(&self) -> M::Fut<'_, Result<crate::types::BlueChannel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.blue_gain()
    }

    /// Get red tuning.
    pub fn red_tuning(&self) -> M::Fut<'_, Result<crate::types::RedTuning, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.red_tuning()
    }

    /// Get blue tuning.
    pub fn blue_tuning(&self) -> M::Fut<'_, Result<crate::types::BlueTuning, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.blue_tuning()
    }

    /// Get color temperature.
    pub fn color_temperature(&self) -> M::Fut<'_, Result<crate::types::ColorTemp, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.color_temperature()
    }

    /// Set to auto white balance.
    pub fn auto(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: WhiteBalanceControl<Mode = M>,
    {
        self.camera.white_balance_auto()
    }

    /// Set to indoor white balance.
    pub fn indoor(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: WhiteBalanceControl<Mode = M>,
    {
        self.camera.white_balance_indoor()
    }

    /// Set to outdoor white balance.
    pub fn outdoor(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: WhiteBalanceControl<Mode = M>,
    {
        self.camera.white_balance_outdoor()
    }

    /// Set to manual white balance.
    pub fn manual(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: WhiteBalanceControl<Mode = M>,
    {
        self.camera.white_balance_manual()
    }

    /// Trigger one-push white balance.
    pub fn one_push_trigger(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: ColorControl<Mode = M>,
    {
        self.camera.one_push_trigger()
    }
}

/// Access to image processing controls and inquiries.
#[derive(Debug)]
pub struct ImageAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> ImageAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get brightness level.
    pub fn brightness(&self) -> M::Fut<'_, Result<crate::types::BrightnessLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.brightness()
    }

    /// Get saturation level.
    pub fn saturation(&self) -> M::Fut<'_, Result<crate::types::SaturationLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.saturation()
    }

    /// Get hue setting.
    pub fn hue(&self) -> M::Fut<'_, Result<crate::types::HueLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.hue()
    }

    /// Get gamma level.
    pub fn gamma(&self) -> M::Fut<'_, Result<crate::types::GammaLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.gamma()
    }

    /// Get sharpness mode.
    pub fn sharpness_mode(&self) -> M::Fut<'_, Result<crate::command::SharpnessMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.sharpness_mode()
    }

    /// Check if black and white mode is enabled.
    pub fn black_white(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.black_white()
    }

    /// Get black and white mode setting.
    pub fn black_white_mode(&self) -> M::Fut<'_, Result<crate::command::BlackWhiteMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.black_white_mode()
    }

    /// Get image flip settings.
    pub fn flip(&self) -> M::Fut<'_, Result<crate::command::FlipState, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.image_flip()
    }

    /// Get flip mode (combined horizontal/vertical).
    pub fn flip_mode(&self) -> M::Fut<'_, Result<crate::command::FlipState, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.flip_mode()
    }

    /// Get resolution mode.
    pub fn resolution(
        &self,
    ) -> M::Fut<'_, Result<crate::command::resolution::ResolutionMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.resolution()
    }

    /// Get picture effect mode.
    pub fn picture_effect(
        &self,
    ) -> M::Fut<'_, Result<crate::command::resolution::PictureEffectMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.picture_effect()
    }

    /// Check if backlight compensation is enabled.
    pub fn backlight_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.backlight_enabled()
    }

    /// Get defog level.
    pub fn defog_level(&self) -> M::Fut<'_, Result<crate::types::DefogLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.defog_level()
    }

    /// Get noise reduction level.
    pub fn noise_reduction_level(
        &self,
    ) -> M::Fut<'_, Result<crate::types::NoiseReductionLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.noise_reduction_level()
    }

    /// Get noise reduction 2D level.
    pub fn noise_reduction_2d(
        &self,
    ) -> M::Fut<'_, Result<crate::types::NoiseReduction2DLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.noise_reduction_2d()
    }

    /// Get noise reduction 3D level.
    pub fn noise_reduction_3d(
        &self,
    ) -> M::Fut<'_, Result<crate::types::NoiseReduction3DLevel, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.noise_reduction_3d()
    }

    /// Get noise reduction mode.
    pub fn noise_reduction_mode(
        &self,
    ) -> M::Fut<'_, Result<crate::command::NoiseReductionMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.noise_reduction_mode()
    }

    // Image processing control methods

    /// Enable image flip.
    pub fn enable_flip(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.enable_flip()
    }

    /// Disable image flip.
    pub fn disable_flip(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.disable_flip()
    }

    /// Enable horizontal flip (mirror).
    pub fn enable_horizontal_flip(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.enable_horizontal_flip()
    }

    /// Disable horizontal flip (mirror).
    pub fn disable_horizontal_flip(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.disable_horizontal_flip()
    }

    /// Set image flip mode (combined horizontal and vertical).
    pub fn set_flip_mode(
        &self,
        mode: crate::command::ImageFlipMode,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_image_flip(mode)
    }

    /// Set contrast level.
    pub fn set_contrast(&self, level: crate::types::ContrastLevel) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_contrast(level)
    }

    /// Set sharpness level.
    pub fn set_sharpness(
        &self,
        level: crate::types::SharpnessLevel,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_sharpness(level)
    }

    /// Set saturation level.
    pub fn set_saturation(
        &self,
        level: crate::types::SaturationLevel,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_saturation(level)
    }

    /// Set hue level.
    pub fn set_hue(&self, level: crate::types::HueLevel) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_hue(level)
    }

    /// Set luminance (brightness) level.
    pub fn set_luminance(
        &self,
        level: crate::types::LuminanceLevel,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_luminance(level)
    }

    /// Enable image freeze.
    pub fn freeze(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.enable_freeze()
    }

    /// Disable image freeze.
    pub fn unfreeze(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.disable_freeze()
    }

    /// Enable black and white mode.
    pub fn enable_black_white(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.enable_black_white()
    }

    /// Disable black and white mode.
    pub fn disable_black_white(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.disable_black_white()
    }

    /// Set picture effect mode.
    pub fn set_picture_effect(
        &self,
        mode: crate::command::resolution::PictureEffectMode,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_picture_effect(mode)
    }

    /// Set noise reduction 2D level.
    pub fn set_noise_reduction_2d(
        &self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_noise_reduction_2d(level)
    }

    /// Disable noise reduction 2D.
    pub fn disable_noise_reduction_2d(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.disable_noise_reduction_2d()
    }

    /// Set noise reduction 3D level.
    pub fn set_noise_reduction_3d(
        &self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.set_noise_reduction_3d(level)
    }

    /// Disable noise reduction 3D.
    pub fn disable_noise_reduction_3d(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>:
            crate::camera::controls::image_processing::ImageProcessingControl<Mode = M>,
    {
        use crate::camera::controls::image_processing::ImageProcessingControl;
        self.camera.disable_noise_reduction_3d()
    }
}

/// Access to preset-related controls.
#[derive(Debug)]
pub struct PresetsAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> PresetsAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Recall a preset.
    pub fn recall(&self, preset: u8) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PresetsControl<Mode = M> + ViscaClient<M>,
    {
        use crate::command::preset::PresetNumber;
        match PresetNumber::new(preset) {
            Ok(preset_number) => self.camera.preset_recall(preset_number),
            Err(_) => self.camera.error(Error::InvalidParameter {
                parameter: "preset",
                value: preset.to_string().into(),
                reason: "Preset number must be between 0 and 255".into(),
            }),
        }
    }

    /// Set (save) a preset.
    pub fn set(&self, preset: u8) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PresetsControl<Mode = M> + ViscaClient<M>,
    {
        use crate::command::preset::PresetNumber;
        match PresetNumber::new(preset) {
            Ok(preset_number) => self.camera.preset_set(preset_number),
            Err(_) => self.camera.error(Error::InvalidParameter {
                parameter: "preset",
                value: preset.to_string().into(),
                reason: "Preset number must be between 0 and 255".into(),
            }),
        }
    }

    /// Reset (clear) a preset.
    pub fn reset(&self, preset: u8) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: PresetsControl<Mode = M> + ViscaClient<M>,
    {
        use crate::command::preset::PresetNumber;
        match PresetNumber::new(preset) {
            Ok(preset_number) => self.camera.preset_reset(preset_number),
            Err(_) => self.camera.error(Error::InvalidParameter {
                parameter: "preset",
                value: preset.to_string().into(),
                reason: "Preset number must be between 0 and 255".into(),
            }),
        }
    }
}

/// Access to tally light controls and inquiries.
#[derive(Debug)]
pub struct TallyAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> TallyAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get tally light status.
    pub fn status(&self) -> M::Fut<'_, Result<crate::command::typed::TallyStatusState, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.tally_light_status()
    }

    /// Check if tally auto adjust is enabled.
    pub fn auto_adjust_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.tally_auto_adjust_enabled()
    }

    /// Turn on red tally light.
    pub fn red_on(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: TallyControl<Mode = M>,
    {
        self.camera.tally_red_on()
    }

    /// Turn off red tally light.
    pub fn red_off(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: TallyControl<Mode = M>,
    {
        self.camera.tally_red_off()
    }

    /// Turn on green tally light.
    pub fn green_on(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: TallyControl<Mode = M>,
    {
        self.camera.tally_green_on()
    }

    /// Turn off green tally light.
    pub fn green_off(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: TallyControl<Mode = M>,
    {
        self.camera.tally_green_off()
    }
}

/// Access to ND filter controls and inquiries.
#[derive(Debug)]
pub struct NdFilterAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> NdFilterAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    #[cfg(feature = "mode-async")]
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the current ND filter position.
    pub fn position(
        &self,
    ) -> M::Fut<'_, Result<crate::command::resolution::NdFilterPosition, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.nd_filter_position()
    }

    /// Get the ND filter preset setting.
    pub fn preset(&self) -> M::Fut<'_, Result<crate::types::NdFilterPreset, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.nd_filter_preset()
    }

    /// Set the ND filter mode.
    pub fn set_mode(&self, mode: crate::command::NdFilterMode) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: NdFilterControl<Mode = M>,
    {
        self.camera.set_nd_filter_mode(mode)
    }
}

/// Access to motion sync controls and inquiries.
#[derive(Debug)]
pub struct MotionSyncAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> MotionSyncAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    #[cfg(feature = "mode-async")]
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the motion sync mode setting.
    pub fn mode(&self) -> M::Fut<'_, Result<crate::command::MotionSyncMode, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.motion_sync_mode()
    }
}

/// Access to menu controls and inquiries.
#[derive(Debug)]
pub struct MenuAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> MenuAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Get the menu open/close status.
    pub fn is_open(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.menu_status()
    }

    /// Open the menu.
    pub fn open(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        self.camera.set_menu_display(true)
    }

    /// Close the menu.
    pub fn close(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        self.camera.set_menu_display(false)
    }

    /// Navigate up in the menu.
    pub fn up(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        use crate::command::MenuDirection;
        self.camera.menu_navigate(MenuDirection::Up)
    }

    /// Navigate down in the menu.
    pub fn down(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        use crate::command::MenuDirection;
        self.camera.menu_navigate(MenuDirection::Down)
    }

    /// Navigate left in the menu.
    pub fn left(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        use crate::command::MenuDirection;
        self.camera.menu_navigate(MenuDirection::Left)
    }

    /// Navigate right in the menu.
    pub fn right(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        use crate::command::MenuDirection;
        self.camera.menu_navigate(MenuDirection::Right)
    }

    /// Confirm menu selection.
    pub fn enter(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        use crate::command::MenuAction;
        self.camera.menu_action(MenuAction::Select)
    }

    /// Return from current menu level.
    pub fn return_menu(&self) -> M::Fut<'_, Result<(), Error>>
    where
        Camera<M, P, Tr, Exec>: MenuControl<Mode = M>,
    {
        use crate::command::MenuAction;
        self.camera.menu_action(MenuAction::Cancel)
    }
}

/// Access to advanced settings inquiries.
#[derive(Debug)]
pub struct AdvancedAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
}

impl<'a, M, P, Tr, Exec> AdvancedAccessor<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    #[cfg(feature = "mode-async")]
    pub(crate) fn new(camera: &'a Camera<M, P, Tr, Exec>) -> Self {
        Self { camera }
    }

    /// Check if night/day mode is enabled.
    pub fn night_day_mode(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.night_day_mode()
    }

    /// Check if standby mode is enabled.
    pub fn standby_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.standby_enabled()
    }

    /// Check iris control status.
    pub fn iris_control(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.iris_control()
    }

    /// Check if digital PTZ is enabled.
    pub fn digital_ptz_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.digital_ptz_enabled()
    }

    /// Check if auto trace is enabled.
    pub fn auto_trace_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.auto_trace_enabled()
    }

    /// Get focus unlock state.
    pub fn focus_unlock(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.focus_unlock()
    }

    /// Get broadcast domain setting.
    pub fn broadcast_domain(&self) -> M::Fut<'_, Result<crate::types::BroadcastDomain, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.broadcast_domain()
    }

    /// Check if USB audio is enabled.
    pub fn usb_audio_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.usb_audio_enabled()
    }

    /// Check if two tone mode is enabled.
    pub fn two_tone_mode_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.two_tone_mode_enabled()
    }

    /// Check if digital mode is enabled.
    pub fn digital_mode_enabled(&self) -> M::Fut<'_, Result<bool, Error>>
    where
        Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
    {
        self.camera.digital_mode_enabled()
    }
}
