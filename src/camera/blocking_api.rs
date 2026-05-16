//! Blocking-specific camera client with direct `Result<T, E>` returns.
//!
//! This module provides a thin wrapper around `Camera<Blocking, _, _, _>` that
//! converts the `Ready<T>` futures to direct `Result<T, E>` values, restoring
//! traditional blocking ergonomics while maintaining the unified Mode-generic design.

use std::ops::Deref;

use crate::{
    camera::{
        controls::{
            color::{
                ColorTemperatureControl, OnePushWhiteBalanceControl, RgbGainControl,
                RgbTuningControl,
            },
            exposure::{
                BacklightCompensationControl, BrightnessControl, ExposureCompensationControl,
                ExposureControl, IrisControl, WideDynamicRangeControl,
            },
            focus::{
                AutoFocusSensitivityControl, FocusControl, FocusLockControl, FocusZoneControl,
                OnePushFocusControl, PushAFControl,
            },
            image_processing::{
                ContrastControl, GammaControl, HueControl, ImageFlipControl, ImageFlipModeControl,
                ImageMirrorControl, LuminanceControl, NoiseReduction2DControl,
                NoiseReduction3DControl, PictureEffectControl, SaturationControl, SharpnessControl,
            },
            inquiry::{
                BacklightCompensationInquiryControl, BrightnessInquiryControl,
                ColorTemperatureInquiryControl, ContrastInquiryControl,
                ExposureCompensationInquiryControl, FocusNearLimitInquiryControl,
                FocusZoneInquiryControl, GammaInquiryControl, HueInquiryControl,
                ImageFlipInquiryControl, InquiryControl, IrisInquiryControl,
                LuminanceInquiryControl, NdFilterInquiryControl, NoiseReduction2DInquiryControl,
                NoiseReduction3DInquiryControl, NoiseReductionInquiryControl,
                PanTiltInquiryControl, PictureEffectInquiryControl, RgbGainInquiryControl,
                RgbTuningInquiryControl, SaturationInquiryControl, SharpnessInquiryControl,
                WideDynamicRangeInquiryControl,
            },
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
            white_balance::{
                AutoTrackingWhiteBalanceControl, AutoWhiteBalanceSensitivityControl,
                WhiteBalanceControl,
            },
            zoom::{DigitalZoomControl, DigitalZoomRangeControl, DirectZoomControl, ZoomControl},
        },
        Camera,
    },
    mode::{Blocking, BlockingFutureExt},
    Error,
};

/// Blocking camera client returned by [`Connect`](crate::camera::Connect) and
/// blocking [`CameraConfig`](crate::camera::CameraConfig) flows.
///
/// Most users should construct this type with
/// [`Connect::open_tcp_blocking`](crate::camera::Connect::open_tcp_blocking) or
/// [`Connect::open_udp_blocking`](crate::camera::Connect::open_udp_blocking).
/// Use [`BlockingClient::new`] only for advanced integrations that already own
/// a configured blocking transport.
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{
///     Error,
///     camera::{Connect, profiles::PtzOpticsG2},
///     types::ZoomPosition,
/// };
///
/// fn main() -> Result<(), Error> {
///     let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
///
///     // Direct Result<T, Error> returns - no .block() needed.
///     camera.power().on()?;
///     camera.zoom().stop()?;
///     camera.zoom().set_position(ZoomPosition::new(0x2000)?)?;
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

    /// Execute an arbitrary VISCA command and require a successful completion.
    ///
    /// This is the blocking raw-command escape hatch for custom command types
    /// that are not yet represented by a typed control method.
    pub fn execute<C>(&self, command: C) -> Result<(), Error>
    where
        P: Default,
        Tr: crate::transport::BlockingTransport
            + crate::transport::HasTransportConfig
            + Send
            + 'static,
        C: crate::command::ViscaCommand,
    {
        self.inner.execute(command).block()
    }

    /// Send an arbitrary VISCA command and return the raw response.
    ///
    /// Prefer [`execute`](Self::execute) for command-only operations and typed
    /// control methods for built-in commands.
    pub fn send_command<C>(&self, command: &C) -> Result<crate::command::Response, Error>
    where
        P: Default,
        Tr: crate::transport::BlockingTransport
            + crate::transport::HasTransportConfig
            + Send
            + 'static,
        C: crate::command::ViscaCommand,
    {
        self.inner.send_command(command).block()
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
    /// use grafton_visca::camera::{Connect, profiles::PtzOpticsG2};
    /// use std::time::Duration;
    ///
    /// let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
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

    /// Wait for movement completion with configurable options.
    ///
    /// This method provides fine-grained control over movement detection,
    /// including which axes to monitor, timeout, and debug logging.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::camera::{profiles::PtzOpticsG2, AwaitConfig, Connect};
    ///
    /// let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
    ///
    /// // After preset recall, wait for all axes with generous timeout
    /// camera.presets().recall(1)?;
    /// camera.await_with_config(&AwaitConfig::for_preset_recall())?;
    /// ```
    pub fn await_with_config(&mut self, config: &super::AwaitConfig) -> Result<(), Error>
    where
        P: crate::capabilities::ProfileMetadata + Default,
        Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        self.inner.await_with_config(config)
    }

    /// Wait for specific axes to become idle.
    ///
    /// This is a convenience method for selective axis monitoring. Use when
    /// you know which axes were affected by your command.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::{
    ///     camera::{profiles::PtzOpticsG2, Axes, Connect},
    ///     units::Degrees,
    ///     SpeedLevel,
    /// };
    /// use std::time::Duration;
    ///
    /// let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
    ///
    /// // After pan/tilt command, only wait for pan/tilt (not zoom/focus)
    /// camera.pan_tilt().absolute(Degrees(45.0), Degrees(10.0), SpeedLevel::Fast)?;
    /// camera.await_axes_idle(Axes::PAN_TILT, Duration::from_secs(20))?;
    /// ```
    pub fn await_axes_idle(
        &mut self,
        axes: super::Axes,
        timeout: std::time::Duration,
    ) -> Result<(), Error>
    where
        P: crate::capabilities::ProfileMetadata + Default,
        Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        self.inner.await_axes_idle(axes, timeout)
    }

    /// Close the camera connection.
    ///
    /// This method consumes the camera so no further operations can be started.
    /// The underlying blocking transport is closed when it is dropped.
    /// The camera object is consumed and cannot be used after this call.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::camera::{Connect, profiles::PtzOpticsG2};
    ///
    /// let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
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

    /// Access power-related controls and inquiries.
    pub fn power(&self) -> BlockingPowerAccessor<'_, P, Tr> {
        BlockingPowerAccessor::new(self)
    }

    /// Access zoom-related controls and inquiries.
    pub fn zoom(&self) -> BlockingZoomAccessor<'_, P, Tr> {
        BlockingZoomAccessor::new(self)
    }

    /// Access pan/tilt-related controls and inquiries.
    pub fn pan_tilt(&self) -> BlockingPanTiltAccessor<'_, P, Tr> {
        BlockingPanTiltAccessor::new(self)
    }

    /// Access focus-related controls and inquiries.
    pub fn focus(&self) -> BlockingFocusAccessor<'_, P, Tr> {
        BlockingFocusAccessor::new(self)
    }

    /// Access exposure-related controls and inquiries.
    pub fn exposure(&self) -> BlockingExposureAccessor<'_, P, Tr> {
        BlockingExposureAccessor::new(self)
    }

    /// Access white balance controls and inquiries.
    pub fn white_balance(&self) -> BlockingWhiteBalanceAccessor<'_, P, Tr> {
        BlockingWhiteBalanceAccessor::new(self)
    }

    /// Access image processing controls and inquiries.
    pub fn image(&self) -> BlockingImageAccessor<'_, P, Tr> {
        BlockingImageAccessor::new(self)
    }

    /// Access preset-related controls.
    pub fn presets(&self) -> BlockingPresetsAccessor<'_, P, Tr> {
        BlockingPresetsAccessor::new(self)
    }

    /// Access tally light controls and inquiries.
    pub fn tally(&self) -> BlockingTallyAccessor<'_, P, Tr>
    where
        P: crate::capabilities::HasTally,
    {
        BlockingTallyAccessor::new(self)
    }

    /// Access system-related controls and inquiries.
    pub fn system(&self) -> BlockingSystemAccessor<'_, P, Tr> {
        BlockingSystemAccessor::new(self)
    }

    /// Access menu controls and inquiries.
    pub fn menu(&self) -> BlockingMenuAccessor<'_, P, Tr> {
        BlockingMenuAccessor::new(self)
    }

    /// Access ND filter controls and inquiries.
    pub fn nd_filter(&self) -> BlockingNdFilterAccessor<'_, P, Tr>
    where
        P: crate::capabilities::HasNdFilter,
    {
        BlockingNdFilterAccessor::new(self)
    }

    /// Access motion sync controls and inquiries.
    pub fn motion_sync(&self) -> BlockingMotionSyncAccessor<'_, P, Tr>
    where
        P: crate::capabilities::HasMotionSync,
    {
        BlockingMotionSyncAccessor::new(self)
    }

    /// Access advanced settings inquiries.
    pub fn advanced(&self) -> BlockingAdvancedAccessor<'_, P, Tr> {
        BlockingAdvancedAccessor::new(self)
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

macro_rules! define_blocking_accessor {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        pub struct $name<'a, P, Tr>
        where
            P: crate::capabilities::Profile,
        {
            camera: &'a BlockingClient<P, Tr>,
        }

        impl<'a, P, Tr> $name<'a, P, Tr>
        where
            P: crate::capabilities::Profile,
        {
            fn new(camera: &'a BlockingClient<P, Tr>) -> Self {
                Self { camera }
            }
        }

        impl<P, Tr> core::fmt::Debug for $name<'_, P, Tr>
        where
            P: crate::capabilities::Profile,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    };
}

define_blocking_accessor!(
    /// Blocking power controls and inquiries.
    BlockingPowerAccessor
);

impl<P, Tr> BlockingPowerAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::power::Power,
    Camera<Blocking, P, Tr, ()>: PowerControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Get the current power state.
    pub fn state(&self) -> Result<bool, Error> {
        self.camera.power_state()
    }

    /// Turn the camera on.
    pub fn on(&self) -> Result<(), Error> {
        self.camera.power_on()
    }

    /// Turn the camera off.
    pub fn off(&self) -> Result<(), Error> {
        self.camera.power_off()
    }

    /// Set power state.
    pub fn set(&self, on: bool) -> Result<(), Error> {
        if on {
            self.on()
        } else {
            self.off()
        }
    }
}

define_blocking_accessor!(
    /// Blocking zoom controls and inquiries.
    BlockingZoomAccessor
);

impl<P, Tr> BlockingZoomAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
    Camera<Blocking, P, Tr, ()>: ZoomControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Get the current zoom position.
    pub fn position(&self) -> Result<crate::types::ZoomPosition, Error> {
        self.camera.zoom_position()
    }

    /// Zoom toward telephoto at standard speed.
    pub fn tele(&self) -> Result<(), Error> {
        self.camera.zoom_tele(None)
    }

    /// Zoom toward wide angle at standard speed.
    pub fn wide(&self) -> Result<(), Error> {
        self.camera.zoom_wide(None)
    }

    /// Stop zoom movement.
    pub fn stop(&self) -> Result<(), Error> {
        self.camera.zoom_stop()
    }

    /// Set zoom position directly from raw VISCA units.
    pub fn set_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: DirectZoomControl<Mode = Blocking>,
    {
        self.camera.inner.set_zoom(position).block()
    }

    /// Set zoom to a normalized optical position.
    pub fn set_normalized(&self, position: crate::UnitInterval) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: DirectZoomControl<Mode = Blocking>,
    {
        self.camera.inner.set_zoom_normalized(position).block()
    }

    /// Set zoom to a normalized position in a documented zoom domain.
    pub fn set_normalized_in_domain(
        &self,
        position: crate::UnitInterval,
        domain: crate::ZoomDomain,
    ) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: DigitalZoomRangeControl<Mode = Blocking>,
    {
        self.camera
            .inner
            .set_zoom_normalized_in_domain(position, domain)
            .block()
    }

    /// Zoom toward telephoto with variable speed.
    pub fn tele_variable<S>(&self, speed: S) -> Result<(), Error>
    where
        S: Into<crate::ZoomSpeed>,
    {
        self.camera.zoom_tele(Some(speed.into()))
    }

    /// Zoom toward wide angle with variable speed.
    pub fn wide_variable<S>(&self, speed: S) -> Result<(), Error>
    where
        S: Into<crate::ZoomSpeed>,
    {
        self.camera.zoom_wide(Some(speed.into()))
    }
}

define_blocking_accessor!(
    /// Blocking pan/tilt controls and inquiries.
    BlockingPanTiltAccessor
);

impl<P, Tr> BlockingPanTiltAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::pan_tilt::PanTilt,
    Camera<Blocking, P, Tr, ()>:
        PanTiltControl<Mode = Blocking> + PanTiltInquiryControl<Mode = Blocking>,
{
    /// Get the current pan/tilt position.
    pub fn position(&self) -> Result<crate::camera::PanTiltPosition, Error> {
        self.camera.pan_tilt_position()
    }

    /// Move in a specific direction.
    pub fn move_direction(
        &self,
        direction: crate::command::PanTiltDirection,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        self.camera.pan_tilt_move(direction, pan_speed, tilt_speed)
    }

    /// Move up.
    pub fn up(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        self.move_direction(crate::command::PanTiltDirection::Up, pan_speed, tilt_speed)
    }

    /// Move down.
    pub fn down(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        self.move_direction(
            crate::command::PanTiltDirection::Down,
            pan_speed,
            tilt_speed,
        )
    }

    /// Move left.
    pub fn left(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        self.move_direction(
            crate::command::PanTiltDirection::Left,
            pan_speed,
            tilt_speed,
        )
    }

    /// Move right.
    pub fn right(
        &self,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        self.move_direction(
            crate::command::PanTiltDirection::Right,
            pan_speed,
            tilt_speed,
        )
    }

    /// Stop pan/tilt movement.
    pub fn stop(&self) -> Result<(), Error> {
        self.camera.pan_tilt_stop()
    }

    /// Move to home position.
    pub fn home(&self) -> Result<(), Error> {
        self.camera.pan_tilt_home()
    }

    /// Move to an absolute pan/tilt position.
    pub fn absolute(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> Result<(), Error> {
        self.camera.pan_tilt_absolute(pan, tilt, speed)
    }

    /// Move relative to the current pan/tilt position.
    pub fn relative(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> Result<(), Error> {
        self.camera.pan_tilt_relative(pan, tilt, speed)
    }
}

define_blocking_accessor!(
    /// Blocking focus controls and inquiries.
    BlockingFocusAccessor
);

impl<P, Tr> BlockingFocusAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::focus::Focus,
    Camera<Blocking, P, Tr, ()>: FocusControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Get the current focus position.
    pub fn position(&self) -> Result<crate::types::FocusPosition, Error> {
        self.camera.focus_position()
    }

    /// Get the current focus mode.
    pub fn mode(&self) -> Result<crate::command::FocusMode, Error> {
        self.camera.focus_mode()
    }

    /// Set auto focus mode.
    pub fn auto(&self) -> Result<(), Error> {
        self.camera.focus_auto()
    }

    /// Set manual focus mode.
    pub fn manual(&self) -> Result<(), Error> {
        self.camera.focus_manual()
    }

    /// Focus near.
    pub fn near(&self, speed: crate::types::SpeedLevel) -> Result<(), Error> {
        self.camera.focus_near(speed)
    }

    /// Focus far.
    pub fn far(&self, speed: crate::types::SpeedLevel) -> Result<(), Error> {
        self.camera.focus_far(speed)
    }

    /// Stop focus movement.
    pub fn stop(&self) -> Result<(), Error> {
        self.camera.focus_stop()
    }

    /// Set focus position directly.
    pub fn set_position<T>(&self, position: T) -> Result<(), Error>
    where
        T: TryInto<crate::types::FocusPosition>,
        T::Error: Into<Error>,
    {
        let position = position.try_into().map_err(Into::into)?;
        self.camera.set_focus(position)
    }

    /// Get the focus near limit.
    pub fn near_limit(&self) -> Result<crate::types::FocusPosition, Error>
    where
        Camera<Blocking, P, Tr, ()>: FocusNearLimitInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.focus_near_limit().block()
    }

    /// Get the focus zone.
    pub fn zone(&self) -> Result<crate::command::FocusZone, Error>
    where
        Camera<Blocking, P, Tr, ()>: FocusZoneInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.focus_zone().block()
    }

    /// Set the focus zone.
    pub fn set_zone(&self, zone: crate::command::FocusZone) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: FocusZoneControl<Mode = Blocking>,
    {
        self.camera.inner.set_focus_zone(zone).block()
    }

    /// Set auto focus sensitivity.
    pub fn set_sensitivity(
        &self,
        sensitivity: crate::command::AutoFocusSensitivity,
    ) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: AutoFocusSensitivityControl<Mode = Blocking>,
    {
        self.camera
            .inner
            .set_auto_focus_sensitivity(sensitivity)
            .block()
    }

    /// Set the focus near limit.
    pub fn set_near_limit<T>(&self, position: T) -> Result<(), Error>
    where
        T: TryInto<crate::types::FocusPosition>,
        T::Error: Into<Error>,
    {
        let position = position.try_into().map_err(Into::into)?;
        self.camera.set_focus_near_limit(position)
    }

    /// Trigger one-push auto focus when supported by the profile.
    pub fn one_push(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: OnePushFocusControl<Mode = Blocking>,
    {
        self.camera.inner.focus_one_push().block()
    }
}

impl<P, Tr> BlockingFocusAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::HasFocusLock,
    Camera<Blocking, P, Tr, ()>: FocusLockControl<Mode = Blocking>,
{
    /// Enable focus lock.
    pub fn lock(&self) -> Result<(), Error> {
        self.camera.enable_focus_lock()
    }

    /// Disable focus lock.
    pub fn unlock(&self) -> Result<(), Error> {
        self.camera.disable_focus_lock()
    }
}

impl<P, Tr> BlockingFocusAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::HasPushAutoFocus,
    Camera<Blocking, P, Tr, ()>: PushAFControl<Mode = Blocking>,
{
    /// Press Push AF.
    pub fn push_af_press(&self) -> Result<(), Error> {
        self.camera.push_af_press()
    }

    /// Release Push AF.
    pub fn push_af_release(&self) -> Result<(), Error> {
        self.camera.push_af_release()
    }
}

define_blocking_accessor!(
    /// Blocking exposure controls and inquiries.
    BlockingExposureAccessor
);

impl<P, Tr> BlockingExposureAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::exposure::Exposure,
    Camera<Blocking, P, Tr, ()>: ExposureControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Get the current exposure mode.
    pub fn mode(&self) -> Result<crate::command::ExposureMode, Error> {
        self.camera.exposure_mode()
    }

    /// Get exposure compensation.
    pub fn compensation(&self) -> Result<crate::types::ExposureCompensationLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: ExposureCompensationInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.exposure_compensation().block()
    }

    /// Check whether exposure compensation is enabled.
    pub fn compensation_enabled(&self) -> Result<bool, Error>
    where
        Camera<Blocking, P, Tr, ()>: ExposureCompensationInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.exposure_compensation_enabled().block()
    }

    /// Get exposure compensation position.
    pub fn compensation_position(&self) -> Result<crate::types::ExposureCompensationPosition, Error>
    where
        Camera<Blocking, P, Tr, ()>: ExposureCompensationInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.exposure_compensation_position().block()
    }

    /// Get iris level.
    pub fn iris(&self) -> Result<crate::types::IrisLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: IrisInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.iris().block()
    }

    /// Get shutter speed.
    pub fn shutter(&self) -> Result<crate::types::ShutterSpeed, Error> {
        self.camera.shutter()
    }

    /// Get gain value.
    pub fn gain(&self) -> Result<crate::types::GainLevel, Error> {
        self.camera.gain()
    }

    /// Get gain limit.
    pub fn gain_limit(&self) -> Result<crate::types::GainLimit, Error> {
        self.camera.gain_limit()
    }

    /// Get exposure brightness.
    pub fn brightness(&self) -> Result<crate::types::BrightnessLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: BrightnessInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.brightness().block()
    }

    /// Set auto exposure.
    pub fn auto(&self) -> Result<(), Error> {
        self.camera.exposure_auto()
    }

    /// Set manual exposure.
    pub fn manual(&self) -> Result<(), Error> {
        self.camera.exposure_manual()
    }

    /// Set shutter-priority exposure.
    pub fn shutter_priority(&self) -> Result<(), Error> {
        self.camera.exposure_shutter_priority()
    }

    /// Set brightness-priority exposure mode.
    pub fn bright_mode(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: BrightnessControl<Mode = Blocking>,
    {
        self.camera.inner.exposure_bright_mode().block()
    }

    /// Set exposure brightness level.
    pub fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: BrightnessControl<Mode = Blocking>,
    {
        self.camera.inner.set_brightness(level).block()
    }

    /// Reset exposure brightness.
    pub fn reset_brightness(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: BrightnessControl<Mode = Blocking>,
    {
        self.camera.inner.reset_brightness().block()
    }

    /// Increase exposure brightness.
    pub fn increase_brightness(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: BrightnessControl<Mode = Blocking>,
    {
        self.camera.inner.increase_brightness().block()
    }

    /// Decrease exposure brightness.
    pub fn decrease_brightness(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: BrightnessControl<Mode = Blocking>,
    {
        self.camera.inner.decrease_brightness().block()
    }

    /// Set exposure brightness using direct mode.
    pub fn set_brightness_direct(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: BrightnessControl<Mode = Blocking>,
    {
        self.camera.inner.set_brightness_direct(level).block()
    }

    /// Set iris-priority exposure.
    pub fn iris_priority(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: IrisControl<Mode = Blocking>,
    {
        self.camera.inner.exposure_iris_priority().block()
    }
}

define_blocking_accessor!(
    /// Blocking white balance controls and inquiries.
    BlockingWhiteBalanceAccessor
);

impl<P, Tr> BlockingWhiteBalanceAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::white_balance::WhiteBalance,
    Camera<Blocking, P, Tr, ()>:
        WhiteBalanceControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Get white balance mode.
    pub fn mode(&self) -> Result<crate::command::WhiteBalanceMode, Error> {
        self.camera.white_balance_mode()
    }

    /// Get red gain.
    pub fn red_gain(&self) -> Result<crate::types::RedChannel, Error>
    where
        Camera<Blocking, P, Tr, ()>: RgbGainInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.red_gain().block()
    }

    /// Get blue gain.
    pub fn blue_gain(&self) -> Result<crate::types::BlueChannel, Error>
    where
        Camera<Blocking, P, Tr, ()>: RgbGainInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.blue_gain().block()
    }

    /// Get red tuning.
    pub fn red_tuning(&self) -> Result<crate::types::RedTuning, Error>
    where
        Camera<Blocking, P, Tr, ()>: RgbTuningInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.red_tuning().block()
    }

    /// Get blue tuning.
    pub fn blue_tuning(&self) -> Result<crate::types::BlueTuning, Error>
    where
        Camera<Blocking, P, Tr, ()>: RgbTuningInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.blue_tuning().block()
    }

    /// Get color temperature.
    pub fn color_temperature(&self) -> Result<crate::types::ColorTemp, Error>
    where
        Camera<Blocking, P, Tr, ()>: ColorTemperatureInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.color_temperature().block()
    }

    /// Set auto white balance.
    pub fn auto(&self) -> Result<(), Error> {
        self.camera.white_balance_auto()
    }

    /// Set indoor white balance.
    pub fn indoor(&self) -> Result<(), Error> {
        self.camera.white_balance_indoor()
    }

    /// Set outdoor white balance.
    pub fn outdoor(&self) -> Result<(), Error> {
        self.camera.white_balance_outdoor()
    }

    /// Set one-push white balance mode.
    pub fn one_push(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: OnePushWhiteBalanceControl<Mode = Blocking>,
    {
        self.camera.inner.white_balance_one_push().block()
    }

    /// Set auto-tracing white balance mode.
    pub fn atw(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: AutoTrackingWhiteBalanceControl<Mode = Blocking>,
    {
        self.camera.inner.white_balance_atw().block()
    }

    /// Set manual white balance.
    pub fn manual(&self) -> Result<(), Error> {
        self.camera.white_balance_manual()
    }

    /// Set color temperature white balance mode.
    pub fn color_temperature_mode(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: ColorTemperatureControl<Mode = Blocking>,
    {
        self.camera.inner.white_balance_color_temperature().block()
    }
}

impl<P, Tr> BlockingWhiteBalanceAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Camera<Blocking, P, Tr, ()>: OnePushWhiteBalanceControl<Mode = Blocking>,
{
    /// Trigger one-push white balance.
    pub fn one_push_trigger(&self) -> Result<(), Error> {
        self.camera.inner.one_push_trigger().block()
    }
}

define_blocking_accessor!(
    /// Blocking image processing controls and inquiries.
    BlockingImageAccessor
);

impl<P, Tr> BlockingImageAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::image_processing::ImageProcessing,
    Camera<Blocking, P, Tr, ()>: InquiryControl<Mode = Blocking>,
{
    /// Get saturation level.
    pub fn saturation(&self) -> Result<crate::types::SaturationLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: SaturationInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.saturation().block()
    }

    /// Get hue level.
    pub fn hue(&self) -> Result<crate::types::HueLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: HueInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.hue().block()
    }

    /// Get contrast level.
    pub fn contrast(&self) -> Result<crate::types::ContrastLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: ContrastInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.contrast().block()
    }

    /// Get luminance level.
    pub fn luminance(&self) -> Result<crate::types::LuminanceLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: LuminanceInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.luminance().block()
    }

    /// Get gamma level.
    pub fn gamma(&self) -> Result<crate::types::GammaLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: GammaInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.gamma().block()
    }

    /// Get sharpness mode.
    pub fn sharpness_mode(&self) -> Result<crate::command::SharpnessMode, Error>
    where
        Camera<Blocking, P, Tr, ()>: SharpnessInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.sharpness_mode().block()
    }

    /// Get sharpness level.
    pub fn sharpness_level(&self) -> Result<crate::types::SharpnessLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: SharpnessInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.sharpness_level().block()
    }

    /// Check whether black-and-white mode is enabled.
    pub fn black_white(&self) -> Result<bool, Error>
    where
        Camera<Blocking, P, Tr, ()>: PictureEffectInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.black_white().block()
    }

    /// Get black-and-white mode.
    pub fn black_white_mode(&self) -> Result<crate::command::BlackWhiteMode, Error>
    where
        Camera<Blocking, P, Tr, ()>: PictureEffectInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.black_white_mode().block()
    }

    /// Get image flip state.
    pub fn flip(&self) -> Result<crate::command::FlipState, Error>
    where
        Camera<Blocking, P, Tr, ()>: ImageFlipInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.image_flip().block()
    }

    /// Get flip mode.
    pub fn flip_mode(&self) -> Result<crate::command::FlipState, Error>
    where
        Camera<Blocking, P, Tr, ()>: ImageFlipInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.flip_mode().block()
    }

    /// Get resolution mode.
    pub fn resolution(&self) -> Result<crate::command::ResolutionMode, Error> {
        self.camera.resolution()
    }

    /// Get picture effect mode.
    pub fn picture_effect(&self) -> Result<crate::command::PictureEffectMode, Error>
    where
        Camera<Blocking, P, Tr, ()>: PictureEffectInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.picture_effect().block()
    }

    /// Check whether backlight compensation is enabled.
    pub fn backlight_enabled(&self) -> Result<bool, Error>
    where
        Camera<Blocking, P, Tr, ()>: BacklightCompensationInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.backlight_enabled().block()
    }

    /// Get defog level.
    pub fn defog_level(&self) -> Result<crate::types::DefogLevel, Error> {
        self.camera.defog_level()
    }

    /// Get aggregate noise reduction level.
    pub fn noise_reduction_level(&self) -> Result<crate::types::NoiseReductionLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReductionInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.noise_reduction_level().block()
    }

    /// Get 2D noise reduction level.
    pub fn noise_reduction_2d(&self) -> Result<crate::types::NoiseReduction2DLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReduction2DInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.noise_reduction_2d().block()
    }

    /// Get 3D noise reduction level.
    pub fn noise_reduction_3d(&self) -> Result<crate::types::NoiseReduction3DLevel, Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReduction3DInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.noise_reduction_3d().block()
    }

    /// Get noise reduction mode.
    pub fn noise_reduction_mode(&self) -> Result<crate::command::NoiseReductionMode, Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReductionInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.noise_reduction_mode().block()
    }

    /// Enable vertical image flip.
    pub fn enable_flip(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: ImageFlipControl<Mode = Blocking>,
    {
        self.camera.inner.enable_flip().block()
    }

    /// Disable vertical image flip.
    pub fn disable_flip(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: ImageFlipControl<Mode = Blocking>,
    {
        self.camera.inner.disable_flip().block()
    }

    /// Enable horizontal image flip.
    pub fn enable_horizontal_flip(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: ImageMirrorControl<Mode = Blocking>,
    {
        self.camera.inner.enable_horizontal_flip().block()
    }

    /// Disable horizontal image flip.
    pub fn disable_horizontal_flip(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: ImageMirrorControl<Mode = Blocking>,
    {
        self.camera.inner.disable_horizontal_flip().block()
    }

    /// Set combined flip mode.
    pub fn set_flip_mode(&self, mode: crate::command::ImageFlipMode) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: ImageFlipModeControl<Mode = Blocking>,
    {
        self.camera.inner.set_image_flip(mode).block()
    }

    /// Set contrast level.
    pub fn set_contrast(&self, level: crate::types::ContrastLevel) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: ContrastControl<Mode = Blocking>,
    {
        self.camera.inner.set_contrast(level).block()
    }

    /// Set sharpness level.
    pub fn set_sharpness(&self, level: crate::types::SharpnessLevel) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: SharpnessControl<Mode = Blocking>,
    {
        self.camera.inner.set_sharpness(level).block()
    }

    /// Set saturation level.
    pub fn set_saturation(&self, level: crate::types::SaturationLevel) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: SaturationControl<Mode = Blocking>,
    {
        self.camera.inner.set_saturation(level).block()
    }

    /// Set hue level.
    pub fn set_hue(&self, level: crate::types::HueLevel) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: HueControl<Mode = Blocking>,
    {
        self.camera.inner.set_hue(level).block()
    }

    /// Set luminance level.
    pub fn set_luminance(&self, level: crate::types::LuminanceLevel) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: LuminanceControl<Mode = Blocking>,
    {
        self.camera.inner.set_luminance(level).block()
    }

    /// Enable black-and-white mode.
    pub fn enable_black_white(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: PictureEffectControl<Mode = Blocking>,
    {
        self.camera.inner.enable_black_white().block()
    }

    /// Disable black-and-white mode.
    pub fn disable_black_white(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: PictureEffectControl<Mode = Blocking>,
    {
        self.camera.inner.disable_black_white().block()
    }

    /// Set picture effect mode.
    pub fn set_picture_effect(&self, mode: crate::command::PictureEffectMode) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: PictureEffectControl<Mode = Blocking>,
    {
        self.camera.inner.set_picture_effect(mode).block()
    }

    /// Set 2D noise reduction level.
    pub fn set_noise_reduction_2d(
        &self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReduction2DControl<Mode = Blocking>,
    {
        self.camera.inner.set_noise_reduction_2d(level).block()
    }

    /// Disable 2D noise reduction.
    pub fn disable_noise_reduction_2d(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReduction2DControl<Mode = Blocking>,
    {
        self.camera.inner.disable_noise_reduction_2d().block()
    }

    /// Set 3D noise reduction level.
    pub fn set_noise_reduction_3d(
        &self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReduction3DControl<Mode = Blocking>,
    {
        self.camera.inner.set_noise_reduction_3d(level).block()
    }

    /// Disable 3D noise reduction.
    pub fn disable_noise_reduction_3d(&self) -> Result<(), Error>
    where
        Camera<Blocking, P, Tr, ()>: NoiseReduction3DControl<Mode = Blocking>,
    {
        self.camera.inner.disable_noise_reduction_3d().block()
    }
}

define_blocking_accessor!(
    /// Blocking preset controls.
    BlockingPresetsAccessor
);

impl<P, Tr> BlockingPresetsAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::presets::Presets,
    Camera<Blocking, P, Tr, ()>: PresetsControl<Mode = Blocking>,
{
    /// Recall a preset.
    pub fn recall(&self, preset: u8) -> Result<(), Error> {
        self.camera.preset_recall(crate::PresetNumber::new(preset)?)
    }

    /// Save the current position as a preset.
    pub fn set(&self, preset: u8) -> Result<(), Error> {
        self.camera.preset_set(crate::PresetNumber::new(preset)?)
    }

    /// Clear a preset.
    pub fn reset(&self, preset: u8) -> Result<(), Error> {
        self.camera.preset_reset(crate::PresetNumber::new(preset)?)
    }
}

define_blocking_accessor!(
    /// Blocking tally controls and inquiries.
    BlockingTallyAccessor
);

impl<P, Tr> BlockingTallyAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + crate::capabilities::HasTally + Default,
    Camera<Blocking, P, Tr, ()>: TallyControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Get tally light status.
    pub fn status(&self) -> Result<crate::command::TallyStatusState, Error> {
        self.camera.tally_status()
    }

    /// Check whether tally auto-adjust is enabled.
    pub fn auto_adjust_enabled(&self) -> Result<bool, Error> {
        self.camera.tally_auto_adjust_enabled()
    }

    /// Turn on red tally light.
    pub fn red_on(&self) -> Result<(), Error> {
        self.camera.tally_red_on()
    }

    /// Turn off red tally light.
    pub fn red_off(&self) -> Result<(), Error> {
        self.camera.tally_red_off()
    }

    /// Turn on green tally light.
    pub fn green_on(&self) -> Result<(), Error> {
        self.camera.tally_green_on()
    }

    /// Turn off green tally light.
    pub fn green_off(&self) -> Result<(), Error> {
        self.camera.tally_green_off()
    }

    /// Query green tally light state.
    pub fn green_status(&self) -> Result<bool, Error> {
        self.camera.green_tally_status()
    }

    /// Set tally brightness to low.
    pub fn bright_lo(&self) -> Result<(), Error> {
        self.camera.tally_bright_lo()
    }

    /// Set tally brightness to high.
    pub fn bright_hi(&self) -> Result<(), Error> {
        self.camera.tally_bright_hi()
    }

    /// Flash the tally light.
    pub fn flash(&self) -> Result<(), Error> {
        self.camera.tally_flash()
    }

    /// Turn the tally light system on.
    pub fn on(&self) -> Result<(), Error> {
        self.camera.tally_on()
    }

    /// Turn the tally light system off.
    pub fn off(&self) -> Result<(), Error> {
        self.camera.tally_off()
    }
}

define_blocking_accessor!(
    /// Blocking system controls and inquiries.
    BlockingSystemAccessor
);

impl<P, Tr> BlockingSystemAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Camera<Blocking, P, Tr, ()>: SystemControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Get camera version information.
    pub fn version(&self) -> Result<crate::command::VersionInfo, Error> {
        self.camera.version()
    }

    /// Clear the VISCA interface.
    pub fn interface_clear(&self) -> Result<(), Error> {
        self.camera.interface_clear()
    }

    /// Cancel a command on a VISCA socket.
    pub fn cancel_command(&self, socket: crate::ViscaSocket) -> Result<(), Error> {
        self.camera.cancel_command(socket)
    }
}

define_blocking_accessor!(
    /// Blocking menu controls and inquiries.
    BlockingMenuAccessor
);

impl<P, Tr> BlockingMenuAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::MenuCapability,
    Camera<Blocking, P, Tr, ()>: MenuControl<Mode = Blocking> + InquiryControl<Mode = Blocking>,
{
    /// Check whether the menu is open.
    pub fn is_open(&self) -> Result<bool, Error> {
        self.camera.menu_status()
    }

    /// Open the menu.
    pub fn open(&self) -> Result<(), Error> {
        self.camera.set_menu_display(true)
    }

    /// Close the menu.
    pub fn close(&self) -> Result<(), Error> {
        self.camera.set_menu_display(false)
    }

    /// Navigate up.
    pub fn up(&self) -> Result<(), Error> {
        self.camera
            .menu_navigate(crate::command::menu::MenuDirection::Up)
    }

    /// Navigate down.
    pub fn down(&self) -> Result<(), Error> {
        self.camera
            .menu_navigate(crate::command::menu::MenuDirection::Down)
    }

    /// Navigate left.
    pub fn left(&self) -> Result<(), Error> {
        self.camera
            .menu_navigate(crate::command::menu::MenuDirection::Left)
    }

    /// Navigate right.
    pub fn right(&self) -> Result<(), Error> {
        self.camera
            .menu_navigate(crate::command::menu::MenuDirection::Right)
    }

    /// Confirm menu selection.
    pub fn enter(&self) -> Result<(), Error> {
        self.camera
            .menu_action(crate::command::menu::MenuAction::Select)
    }

    /// Return from the current menu level.
    pub fn return_menu(&self) -> Result<(), Error> {
        self.camera
            .menu_action(crate::command::menu::MenuAction::Cancel)
    }
}

define_blocking_accessor!(
    /// Blocking ND filter controls and inquiries.
    BlockingNdFilterAccessor
);

impl<P, Tr> BlockingNdFilterAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNdFilter,
    Camera<Blocking, P, Tr, ()>:
        NdFilterControl<Mode = Blocking> + NdFilterInquiryControl<Mode = Blocking>,
{
    /// Get the current ND filter position.
    pub fn position(&self) -> Result<crate::command::NdFilterPosition, Error> {
        self.camera.nd_filter_position()
    }

    /// Get the ND filter preset setting.
    pub fn preset(&self) -> Result<crate::types::NdFilterPreset, Error> {
        self.camera.nd_filter_preset()
    }

    /// Set ND filter mode.
    pub fn set_mode(&self, mode: crate::command::NdFilterMode) -> Result<(), Error> {
        self.camera.set_nd_filter_mode(mode)
    }

    /// Set ND filter value directly.
    pub fn set_value(&self, value: u16) -> Result<(), Error> {
        self.camera.set_nd_filter_value(value)
    }

    /// Set ND filter by stop value.
    pub fn set_stops(&self, stops: f32) -> Result<(), Error> {
        self.camera.set_nd_filter_stops(stops)
    }

    /// Step ND filter up or down.
    pub fn step(&self, direction: crate::command::NdFilterStep) -> Result<(), Error> {
        self.camera.step_nd_filter(direction)
    }

    /// Enable or disable auto ND.
    pub fn set_auto(&self, enabled: bool) -> Result<(), Error> {
        self.camera.set_auto_nd(enabled)
    }
}

define_blocking_accessor!(
    /// Blocking motion sync controls and inquiries.
    BlockingMotionSyncAccessor
);

impl<P, Tr> BlockingMotionSyncAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::HasMotionSync,
    Camera<Blocking, P, Tr, ()>: MotionSyncControl<Mode = Blocking>,
{
    /// Get the motion sync mode.
    pub fn mode(&self) -> Result<crate::command::MotionSyncMode, Error> {
        self.camera.motion_sync_mode()
    }
}

define_blocking_accessor!(
    /// Blocking advanced settings inquiries.
    BlockingAdvancedAccessor
);

impl<P, Tr> BlockingAdvancedAccessor<'_, P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Camera<Blocking, P, Tr, ()>: InquiryControl<Mode = Blocking>,
{
    /// Check whether night/day mode is enabled.
    pub fn night_day_mode(&self) -> Result<bool, Error> {
        self.camera.night_day_mode()
    }

    /// Check whether standby is enabled.
    pub fn standby_enabled(&self) -> Result<bool, Error> {
        self.camera.standby_enabled()
    }

    /// Check iris control status.
    pub fn iris_control(&self) -> Result<bool, Error>
    where
        Camera<Blocking, P, Tr, ()>: IrisInquiryControl<Mode = Blocking>,
    {
        self.camera.inner.iris_control().block()
    }

    /// Check whether digital PTZ is enabled.
    pub fn digital_ptz_enabled(&self) -> Result<bool, Error> {
        self.camera.digital_ptz_enabled()
    }

    /// Check whether auto trace is enabled.
    pub fn auto_trace_enabled(&self) -> Result<bool, Error> {
        self.camera.auto_trace_enabled()
    }

    /// Get focus unlock state.
    pub fn focus_unlock(&self) -> Result<bool, Error> {
        self.camera.focus_unlock()
    }

    /// Get broadcast domain setting.
    pub fn broadcast_domain(&self) -> Result<crate::types::BroadcastDomain, Error> {
        self.camera.broadcast_domain()
    }

    /// Check whether USB audio is enabled.
    pub fn usb_audio_enabled(&self) -> Result<bool, Error> {
        self.camera.usb_audio_enabled()
    }

    /// Check whether two-tone mode is enabled.
    pub fn two_tone_mode_enabled(&self) -> Result<bool, Error> {
        self.camera.two_tone_mode_enabled()
    }

    /// Check whether digital mode is enabled.
    pub fn digital_mode_enabled(&self) -> Result<bool, Error> {
        self.camera.digital_mode_enabled()
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
            pub fn $name(&self $(, $arg: $ty)*) -> Result<$ret_ok, Error> {
                self.inner.$name($($arg),*).block()
            }
        )+
    };
}

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

        /// Start zooming in with optional speed control.
        /// When `speed` is `None`, uses standard zoom speed.
        fn zoom_tele(speed: Option<crate::ZoomSpeed>) -> ();

        /// Start zooming out with optional speed control.
        /// When `speed` is `None`, uses standard zoom speed.
        fn zoom_wide(speed: Option<crate::ZoomSpeed>) -> ();

    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: DirectZoomControl<Mode = Blocking>,
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDirectZoom,
{
    /// Set zoom to an absolute raw VISCA position.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::types::ZoomPosition;
    ///
    /// camera.set_zoom(ZoomPosition::new(0x4000)?)?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The raw position exceeds the selected profile's documented range
    /// - The command fails to send or receive a response
    pub fn set_zoom(&self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        self.inner.set_zoom(position).block()
    }

    /// Set zoom to a normalized optical position.
    pub fn set_zoom_normalized(&self, position: crate::UnitInterval) -> Result<(), Error> {
        self.inner.set_zoom_normalized(position).block()
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: DigitalZoomControl<Mode = Blocking>,
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDigitalZoomToggle,
{
    impl_blocking_methods! {
        /// Set digital zoom on or off.
        fn set_digital_zoom(enabled: bool) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: DigitalZoomRangeControl<Mode = Blocking>,
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDigitalZoomRange,
{
    /// Set zoom to a normalized position within a documented zoom domain.
    pub fn set_zoom_normalized_in_domain(
        &self,
        position: crate::UnitInterval,
        domain: crate::ZoomDomain,
    ) -> Result<(), Error> {
        self.inner
            .set_zoom_normalized_in_domain(position, domain)
            .block()
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

        /// Set focus position.
        fn set_focus(position: crate::types::FocusPosition) -> ();

        /// Set focus to infinity.
        fn focus_infinity() -> ();

        /// Set focus near limit.
        fn set_focus_near_limit(position: crate::types::FocusPosition) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: OnePushFocusControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasOnePushFocus,
{
    impl_blocking_methods! {
        /// Trigger one-push auto focus.
        fn focus_one_push() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: FocusZoneControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasFocusZone,
{
    impl_blocking_methods! {
        /// Set focus zone.
        fn set_focus_zone(zone: crate::command::focus::FocusZone) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: AutoFocusSensitivityControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasAutoFocusSensitivity,
{
    impl_blocking_methods! {
        /// Set auto focus sensitivity.
        fn set_auto_focus_sensitivity(sensitivity: crate::command::focus::AutoFocusSensitivity) -> ();
    }
}

// ============================================================================
// FocusLockControl implementation (PtzOptics vendor-specific)
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: FocusLockControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasFocusLock,
{
    impl_blocking_methods! {
        /// Enable focus lock.
        ///
        /// Focus lock prevents any focus changes while enabled, useful for
        /// maintaining consistent focus during recording. This is a vendor-specific
        /// feature primarily supported by PtzOptics cameras.
        fn enable_focus_lock() -> ();

        /// Disable focus lock.
        ///
        /// Disables focus lock, allowing focus changes again.
        fn disable_focus_lock() -> ();
    }
}

// ============================================================================
// PushAFControl implementation (Sony vendor-specific)
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: PushAFControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasPushAutoFocus,
{
    impl_blocking_methods! {
        /// Push AF press.
        ///
        /// Temporarily activates auto focus while the button is pressed.
        /// This is a vendor-specific feature primarily supported by Sony cameras.
        fn push_af_press() -> ();

        /// Push AF release.
        ///
        /// Releases the push AF button, returning to the previous focus mode.
        fn push_af_release() -> ();
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

    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: BrightnessControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasBrightnessControl,
{
    impl_blocking_methods! {
        /// Set exposure to bright mode.
        fn exposure_bright_mode() -> ();

        /// Set exposure brightness level.
        fn set_brightness(level: crate::types::BrightnessLevel) -> ();

        /// Reset exposure brightness to default.
        fn reset_brightness() -> ();

        /// Increase exposure brightness.
        fn increase_brightness() -> ();

        /// Decrease exposure brightness.
        fn decrease_brightness() -> ();

        /// Set exposure brightness directly.
        fn set_brightness_direct(level: crate::types::BrightnessLevel) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: BacklightCompensationControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasBacklightCompensation,
{
    impl_blocking_methods! {
        /// Enable/disable backlight.
        fn set_backlight(enabled: bool) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: WideDynamicRangeControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasWideDynamicRange,
{
    impl_blocking_methods! {
        /// Set dynamic range level.
        fn set_dynamic_range(level: crate::types::DynamicRangeLevel) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: IrisControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasIrisControl,
{
    impl_blocking_methods! {
        /// Set exposure to iris priority mode.
        fn exposure_iris_priority() -> ();

        /// Set iris level.
        fn set_iris(level: crate::types::IrisLevel) -> ();

        /// Reset iris to default.
        fn reset_iris() -> ();

        /// Increase iris (open).
        fn increase_iris() -> ();

        /// Decrease iris (close).
        fn decrease_iris() -> ();
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
        fn pan_tilt_move(direction: crate::command::PanTiltDirection, pan_speed: crate::types::PanSpeed, tilt_speed: crate::types::TiltSpeed) -> ();

        /// Reset pan/tilt to default position.
        fn pan_tilt_reset() -> ();

        /// Set pan/tilt limit at a specific corner.
        fn pan_tilt_limit_set(corner: crate::command::PanTiltLimitCorner, pan: crate::types::PanPosition, tilt: crate::types::TiltPosition) -> ();

        /// Clear pan/tilt limit for a specific corner.
        fn pan_tilt_limit_clear(corner: crate::command::PanTiltLimitCorner) -> ();
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
        fn preset_recall(preset: crate::command::PresetNumber) -> ();

        /// Set (save) current position as preset.
        fn preset_set(preset: crate::command::PresetNumber) -> ();

        /// Reset (clear) a preset.
        fn preset_reset(preset: crate::command::PresetNumber) -> ();
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

        /// Set manual white balance mode.
        fn white_balance_manual() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: OnePushWhiteBalanceControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasOnePushWhiteBalance,
{
    impl_blocking_methods! {
        /// Set one-push white balance mode.
        fn white_balance_one_push() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: AutoTrackingWhiteBalanceControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasAutoTrackingWhiteBalance,
{
    impl_blocking_methods! {
        /// Set auto tracking white balance.
        fn white_balance_atw() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ColorTemperatureControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasColorTemperature,
{
    impl_blocking_methods! {
        /// Set color temperature white balance mode.
        fn white_balance_color_temperature() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: AutoWhiteBalanceSensitivityControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasAutoWhiteBalanceSensitivity,
{
    impl_blocking_methods! {
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
    P: crate::capabilities::Profile + crate::capabilities::HasTally + Default,
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
        fn tally_status() -> crate::command::TallyStatusState;

        /// Get green tally status (FR7 specific).
        fn green_tally_status() -> bool;

        /// Check whether tally auto-adjust is enabled.
        fn tally_auto_adjust_enabled() -> bool;
    }
}

// ============================================================================
// Color subcontrol implementations
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: OnePushWhiteBalanceControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasOnePushWhiteBalance,
{
    impl_blocking_methods! {
        /// Trigger one-push white balance.
        fn one_push_trigger() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ColorTemperatureControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasColorTemperature,
{
    impl_blocking_methods! {
        /// Set color temperature.
        fn set_color_temperature(temp: crate::types::ColorTemp) -> ();

        /// Reset color temperature.
        fn reset_color_temperature() -> ();

        /// Increase color temperature.
        fn increase_color_temperature() -> ();

        /// Decrease color temperature.
        fn decrease_color_temperature() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: RgbGainControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbGain,
{
    impl_blocking_methods! {
        /// Set red gain.
        fn set_red_gain(gain: crate::types::RedChannel) -> ();

        /// Control red gain (set, reset, increase or decrease).
        fn control_red_gain(command: crate::command::color::RedGain) -> ();

        /// Set blue gain.
        fn set_blue_gain(gain: crate::types::BlueChannel) -> ();

        /// Control blue gain (set, reset, increase or decrease).
        fn control_blue_gain(command: crate::command::color::BlueGain) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: RgbTuningControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbTuning,
{
    impl_blocking_methods! {
        /// Set red tuning.
        fn set_red_tuning(tuning: crate::types::RedTuning) -> ();

        /// Set blue tuning.
        fn set_blue_tuning(tuning: crate::types::BlueTuning) -> ();
    }
}

// ============================================================================
// Image quality subcontrol implementations
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ContrastControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasContrastControl,
{
    impl_blocking_methods! {
        /// Set contrast level.
        fn set_contrast(level: crate::types::ContrastLevel) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: SharpnessControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasSharpnessControl,
{
    impl_blocking_methods! {
        /// Set sharpness level.
        fn set_sharpness(level: crate::types::SharpnessLevel) -> ();

        /// Reset sharpness.
        fn reset_sharpness() -> ();

        /// Increase sharpness.
        fn increase_sharpness() -> ();

        /// Decrease sharpness.
        fn decrease_sharpness() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ImageFlipControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasImageFlip,
{
    impl_blocking_methods! {
        /// Enable image flip.
        fn enable_flip() -> ();

        /// Disable image flip.
        fn disable_flip() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ImageMirrorControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasImageMirror,
{
    impl_blocking_methods! {
        /// Enable horizontal flip.
        fn enable_horizontal_flip() -> ();

        /// Disable horizontal flip.
        fn disable_horizontal_flip() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ImageFlipModeControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasCombinedImageFlip,
{
    impl_blocking_methods! {
        /// Set image flip mode.
        fn set_image_flip(mode: crate::command::ImageFlipMode) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: SaturationControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasSaturationControl,
{
    impl_blocking_methods! {
        /// Set saturation level.
        fn set_saturation(level: crate::types::SaturationLevel) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: HueControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasHueControl,
{
    impl_blocking_methods! {
        /// Set hue level.
        fn set_hue(level: crate::types::HueLevel) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: LuminanceControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasLuminanceControl,
{
    impl_blocking_methods! {
        /// Set luminance level.
        fn set_luminance(level: crate::types::LuminanceLevel) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: GammaControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasGammaControl,
{
    impl_blocking_methods! {
        /// Set gamma curve.
        fn set_gamma(level: crate::types::GammaLevel) -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NoiseReduction2DControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction2D,
{
    impl_blocking_methods! {
        /// Set 2D noise reduction level.
        fn set_noise_reduction_2d(level: crate::types::NoiseReduction2DLevel) -> ();

        /// Disable 2D noise reduction.
        fn disable_noise_reduction_2d() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NoiseReduction3DControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction3D,
{
    impl_blocking_methods! {
        /// Set 3D noise reduction level.
        fn set_noise_reduction_3d(level: crate::types::NoiseReduction3DLevel) -> ();

        /// Disable 3D noise reduction.
        fn disable_noise_reduction_3d() -> ();
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: PictureEffectControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasPictureEffect,
{
    impl_blocking_methods! {
        /// Enable black and white mode.
        fn enable_black_white() -> ();

        /// Disable black and white mode.
        fn disable_black_white() -> ();

        /// Set picture effect mode.
        fn set_picture_effect(mode: crate::command::PictureEffectMode) -> ();
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
    P: crate::capabilities::Profile + Default + crate::capabilities::HasVariableSpeed,
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
    P: crate::capabilities::Profile + Default + crate::capabilities::HasMotionSync,
{
    impl_blocking_methods! {
        /// Set motion sync mode.
        fn set_motion_sync_mode(mode: crate::command::system::MotionSyncMode) -> ();

        /// Set motion sync speed.
        fn set_motion_sync_speed(speed: crate::types::MotionSyncSpeed) -> ();

        /// Set motion sync preset speed.
        fn set_motion_sync_preset_speed(speed: crate::command::system::MotionSyncPreset) -> ();

        /// Get motion sync mode.
        fn motion_sync_mode() -> crate::command::system::MotionSyncMode;

        /// Get motion sync speed.
        fn motion_sync_speed() -> crate::command::system::MotionSyncPreset;
    }
}

// ============================================================================
// NdFilterControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NdFilterControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNdFilter,
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
        fn power_state() -> bool;

        /// Get zoom position.
        fn zoom_position() -> crate::types::ZoomPosition;

        /// Get focus position.
        fn focus_position() -> crate::types::FocusPosition;

        /// Get exposure mode.
        fn exposure_mode() -> crate::command::exposure::ExposureMode;

        /// Get shutter value.
        fn shutter() -> crate::types::ShutterSpeed;

        /// Get gain value.
        fn gain() -> crate::types::GainLevel;

        /// Get gain limit.
        fn gain_limit() -> crate::types::GainLimit;

        /// Get white balance mode.
        fn white_balance_mode() -> crate::command::white_balance::WhiteBalanceMode;

        /// Get resolution.
        fn resolution() -> crate::command::ResolutionMode;

        /// Get camera version information.
        fn version() -> crate::command::VersionInfo;

        /// Get focus mode.
        fn focus_mode() -> crate::command::focus::FocusMode;

        /// Get menu status.
        fn menu_status() -> bool;

        /// Get night/day mode.
        fn night_day_mode() -> bool;

        /// Get standby enabled status.
        fn standby_enabled() -> bool;

        /// Get defog level.
        fn defog_level() -> crate::types::DefogLevel;

        /// Get digital PTZ enabled status.
        fn digital_ptz_enabled() -> bool;

        /// Get auto trace enabled status.
        fn auto_trace_enabled() -> bool;

        /// Get focus unlock status.
        fn focus_unlock() -> bool;

        /// Get broadcast domain.
        fn broadcast_domain() -> crate::types::BroadcastDomain;

        /// Get USB audio enabled status.
        fn usb_audio_enabled() -> bool;

        /// Get two-tone mode enabled status.
        fn two_tone_mode_enabled() -> bool;

        /// Get digital mode enabled status.
        fn digital_mode_enabled() -> bool;

    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: BrightnessInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasBrightnessControl,
{
    impl_blocking_methods! {
        /// Get exposure brightness.
        fn brightness() -> crate::types::BrightnessLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ContrastInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasContrastControl,
{
    impl_blocking_methods! {
        /// Get contrast level.
        fn contrast() -> crate::types::ContrastLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: SharpnessInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasSharpnessControl,
{
    impl_blocking_methods! {
        /// Get sharpness mode.
        fn sharpness_mode() -> crate::command::SharpnessMode;

        /// Get sharpness level.
        fn sharpness_level() -> crate::types::SharpnessLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ExposureCompensationInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasExposureCompensation,
{
    impl_blocking_methods! {
        /// Get exposure compensation.
        fn exposure_compensation() -> crate::types::ExposureCompensationLevel;

        /// Get exposure compensation enabled status.
        fn exposure_compensation_enabled() -> bool;

        /// Get exposure compensation position.
        fn exposure_compensation_position() -> crate::types::ExposureCompensationPosition;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: BacklightCompensationInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasBacklightCompensation,
{
    impl_blocking_methods! {
        /// Get backlight enabled status.
        fn backlight_enabled() -> bool;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: WideDynamicRangeInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasWideDynamicRange,
{
    impl_blocking_methods! {
        /// Get dynamic range level.
        fn dynamic_range() -> crate::types::DynamicRangeLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ColorTemperatureInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasColorTemperature,
{
    impl_blocking_methods! {
        /// Get color temperature.
        fn color_temperature() -> crate::types::ColorTemp;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: RgbGainInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbGain,
{
    impl_blocking_methods! {
        /// Get red gain.
        fn red_gain() -> crate::types::RedChannel;

        /// Get blue gain.
        fn blue_gain() -> crate::types::BlueChannel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: RgbTuningInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbTuning,
{
    impl_blocking_methods! {
        /// Get red tuning.
        fn red_tuning() -> crate::types::RedTuning;

        /// Get blue tuning.
        fn blue_tuning() -> crate::types::BlueTuning;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: SaturationInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasSaturationControl,
{
    impl_blocking_methods! {
        /// Get saturation.
        fn saturation() -> crate::types::SaturationLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: HueInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasHueControl,
{
    impl_blocking_methods! {
        /// Get hue.
        fn hue() -> crate::types::HueLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: LuminanceInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasLuminanceControl,
{
    impl_blocking_methods! {
        /// Get luminance level.
        fn luminance() -> crate::types::LuminanceLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: GammaInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasGammaControl,
{
    impl_blocking_methods! {
        /// Get gamma value.
        fn gamma() -> crate::types::GammaLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: ImageFlipInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasImageFlip,
{
    impl_blocking_methods! {
        /// Get image flip state.
        fn image_flip() -> crate::command::FlipState;

        /// Get flip mode.
        fn flip_mode() -> crate::command::FlipState;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NoiseReductionInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction,
{
    impl_blocking_methods! {
        /// Get noise reduction level.
        fn noise_reduction_level() -> crate::types::NoiseReductionLevel;

        /// Get noise reduction mode.
        fn noise_reduction_mode() -> crate::command::NoiseReductionMode;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NoiseReduction2DInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction2D,
{
    impl_blocking_methods! {
        /// Get 2D noise reduction level.
        fn noise_reduction_2d() -> crate::types::NoiseReduction2DLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NoiseReduction3DInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction3D,
{
    impl_blocking_methods! {
        /// Get 3D noise reduction level.
        fn noise_reduction_3d() -> crate::types::NoiseReduction3DLevel;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: PictureEffectInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasPictureEffect,
{
    impl_blocking_methods! {
        /// Get black and white mode.
        fn black_white() -> bool;

        /// Get black and white mode details.
        fn black_white_mode() -> crate::command::BlackWhiteMode;

        /// Get picture effect.
        fn picture_effect() -> crate::command::PictureEffectMode;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: FocusNearLimitInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasFocusNearLimitInquiry,
{
    impl_blocking_methods! {
        /// Get focus near limit.
        fn focus_near_limit() -> crate::types::FocusPosition;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: FocusZoneInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasFocusZone,
{
    impl_blocking_methods! {
        /// Get focus zone.
        fn focus_zone() -> crate::command::focus::FocusZone;
    }
}

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: IrisInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasIrisControl,
{
    impl_blocking_methods! {
        /// Get iris control status.
        fn iris_control() -> bool;

        /// Get iris value.
        fn iris() -> crate::types::IrisLevel;
    }
}

// ============================================================================
// NdFilterInquiryControl implementation
// ============================================================================

impl<P, Tr> BlockingClient<P, Tr>
where
    Camera<Blocking, P, Tr, ()>: NdFilterInquiryControl<Mode = Blocking>,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNdFilter,
{
    impl_blocking_methods! {
        /// Get ND filter position.
        fn nd_filter_position() -> crate::command::NdFilterPosition;

        /// Get ND filter preset.
        fn nd_filter_preset() -> crate::types::NdFilterPreset;
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
        fn pan_tilt_position() -> crate::camera::PanTiltPosition;
    }
}
