//! Static, owner-backed noun accessors for the async camera facade.
//!
//! Each accessor is a thin view over [`crate::async_session::AsyncCameraCore`]
//! through its typed [`crate::Camera`] owner.  Request preparation, profile
//! validation, admission, and operation completion therefore remain in the
//! same path as [`crate::Camera::execute`], [`crate::Camera::inquire`], and
//! [`crate::Camera::submit`].

#![cfg(feature = "async")]

use crate::{
    async_session::Camera,
    camera::{IdleWait, MotionQuery},
    capabilities::{
        HasAutoFocusSensitivity, HasAutoTrackingWhiteBalance, HasAutoWhiteBalanceSensitivity,
        HasBacklightCompensation, HasBrightnessControl, HasColorTemperature, HasCombinedImageFlip,
        HasContrastControl, HasDigitalZoomRange, HasDigitalZoomToggle, HasDirectZoom, HasExposure,
        HasExposureCompensation, HasFocus, HasFocusLock, HasFocusNearLimitInquiry, HasFocusZone,
        HasGammaControl, HasHueControl, HasImageFlip, HasImageMirror, HasImageProcessing,
        HasIrisControl, HasLuminanceControl, HasMenuControl, HasMotionSync, HasNdFilter,
        HasNoiseReduction, HasNoiseReduction2D, HasNoiseReduction3D, HasOnePushFocus,
        HasOnePushWhiteBalance, HasPanTilt, HasPictureEffect, HasPower, HasPresets,
        HasPushAutoFocus, HasRgbGain, HasRgbTuning, HasSaturationControl, HasSharpnessControl,
        HasTally, HasVariableSpeed, HasWhiteBalance, HasWideDynamicRange, HasZoom,
    },
    command,
    completion::{AppliedOnly, Targeted},
    operation::Operation,
    profile::CompileTimeProfile,
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    Result, ZoomDomain,
};

macro_rules! accessor_method {
    ($(#[$meta:meta])* $name:ident, $method:ident) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, P> {
            $name::new(self)
        }
    };
    ($(#[$meta:meta])* $name:ident, $method:ident, $bound:path) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, P>
        where
            P: $bound,
        {
            $name::new(self)
        }
    };
}

macro_rules! accessor {
    ($(#[$meta:meta])* $name:ident, $method:ident $(, $bound:path)? ) => {
        $(#[$meta])*
        #[must_use]
        pub struct $name<'a, P: CompileTimeProfile> {
            camera: &'a Camera<P>,
        }

        impl<'a, P: CompileTimeProfile> std::fmt::Debug for $name<'a, P> {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }

        impl<'a, P: CompileTimeProfile> $name<'a, P> {
            fn new(camera: &'a Camera<P>) -> Self {
                Self { camera }
            }
        }

        impl<P: CompileTimeProfile> Camera<P> {
            accessor_method!($(#[$meta])* $name, $method $(, $bound)?);
        }
    };
}

accessor!(
    /// Power commands and inquiries for one camera target.
    PowerAccessor,
    power,
    HasPower
);
accessor!(
    /// Optical and digital zoom commands and inquiries.
    ZoomAccessor,
    zoom,
    HasZoom
);
accessor!(
    /// Camera system persistence and version inquiries.
    SystemAccessor,
    system
);
accessor!(
    /// Pan/tilt movement, limits, and position inquiries.
    PanTiltAccessor,
    pan_tilt,
    HasPanTilt
);
accessor!(
    /// Focus movement, modes, and focus inquiries.
    FocusAccessor,
    focus,
    HasFocus
);
accessor!(
    /// Exposure, iris, shutter, brightness, and gain controls.
    ExposureAccessor,
    exposure,
    HasExposure
);
accessor!(
    /// White-balance and channel controls.
    WhiteBalanceAccessor,
    white_balance,
    HasWhiteBalance
);
accessor!(
    /// Image-processing and orientation controls.
    ImageAccessor,
    image,
    HasImageProcessing
);
accessor!(
    /// Camera preset commands and preset inquiries.
    PresetsAccessor,
    presets,
    HasPresets
);

/// Tally-light commands and inquiries for a profile with tally support.
#[must_use]
pub struct TallyAccessor<'a, P: CompileTimeProfile + HasTally> {
    camera: &'a Camera<P>,
}

impl<'a, P: CompileTimeProfile + HasTally> std::fmt::Debug for TallyAccessor<'a, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TallyAccessor")
            .finish_non_exhaustive()
    }
}

/// ND-filter commands and inquiries for a profile with ND support.
#[must_use]
pub struct NdFilterAccessor<'a, P: CompileTimeProfile + HasNdFilter> {
    camera: &'a Camera<P>,
}

impl<'a, P: CompileTimeProfile + HasNdFilter> std::fmt::Debug for NdFilterAccessor<'a, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NdFilterAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion-sync commands and inquiries for a profile with motion-sync support.
#[must_use]
pub struct MotionSyncAccessor<'a, P: CompileTimeProfile + HasMotionSync> {
    camera: &'a Camera<P>,
}

impl<'a, P: CompileTimeProfile + HasMotionSync> std::fmt::Debug for MotionSyncAccessor<'a, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MotionSyncAccessor")
            .finish_non_exhaustive()
    }
}

accessor!(
    /// On-screen menu display and navigation commands and inquiries.
    MenuAccessor,
    menu,
    HasMenuControl
);
accessor!(
    /// Streaming, vendor, and variable-speed controls.
    AdvancedAccessor,
    advanced
);

/// Direct motion safety and observation methods.
#[must_use]
pub struct MotionAccessor<'a, P: CompileTimeProfile> {
    camera: &'a Camera<P>,
}

impl<'a, P: CompileTimeProfile> std::fmt::Debug for MotionAccessor<'a, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MotionAccessor")
            .finish_non_exhaustive()
    }
}

impl<'a, P: CompileTimeProfile> MotionAccessor<'a, P> {
    fn new(camera: &'a Camera<P>) -> Self {
        Self { camera }
    }

    /// Stops all supported pan/tilt, zoom, and focus movement.
    pub async fn stop_all_motion(&self) -> Result<()> {
        self.camera.core().stop_all_motion().await
    }

    /// Reports whether any mechanical movement axis is moving.
    ///
    /// This samples [`AffectedAxes::MOVEMENT`] with the default tolerance; use
    /// [`Self::is_moving_axes`] to pick the axes or the tolerance.
    ///
    /// [`AffectedAxes::MOVEMENT`]: crate::AffectedAxes::MOVEMENT
    pub async fn is_moving(&self) -> Result<bool> {
        self.camera.core().is_moving(MotionQuery::default()).await
    }

    /// Reports whether the selected physical axes are moving.
    pub async fn is_moving_axes(&self, query: MotionQuery) -> Result<bool> {
        self.camera.core().is_moving(query).await
    }

    /// Waits until the selected physical axes become idle.
    pub async fn wait_until_idle(&self, wait: IdleWait) -> Result<()> {
        self.camera.core().wait_until_idle(wait).await
    }
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the direct motion safety and observation surface.
    pub fn motion(&self) -> MotionAccessor<'_, P> {
        MotionAccessor::new(self)
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod inventory_tests {
    use std::collections::BTreeMap;

    use crate::command::semantics::BuiltinCommand;
    use crate::command::surface::{surface_entry, StaticSurfaceDisposition};

    #[test]
    fn every_ledger_method_has_one_async_definition_per_row() {
        let source = include_str!("async_nouns.rs");
        for accessor in [
            "PowerAccessor",
            "ZoomAccessor",
            "SystemAccessor",
            "PanTiltAccessor",
            "FocusAccessor",
            "ExposureAccessor",
            "WhiteBalanceAccessor",
            "ImageAccessor",
            "PresetsAccessor",
            "TallyAccessor",
            "NdFilterAccessor",
            "MotionSyncAccessor",
            "MenuAccessor",
            "AdvancedAccessor",
        ] {
            assert!(
                source.contains(&format!("{accessor}<'")),
                "missing canonical async noun accessor {accessor}",
            );
        }
        assert!(source.contains("pub struct MotionAccessor"));

        let mut expected = BTreeMap::<&str, usize>::new();

        for command in BuiltinCommand::ALL {
            if let StaticSurfaceDisposition::Noun { method, .. } =
                surface_entry(*command).disposition
            {
                *expected.entry(method).or_default() += 1;
            }
        }

        for (method, rows) in expected {
            let needle = format!("pub async fn {method}(");
            assert_eq!(
                source.matches(&needle).count(),
                rows,
                "async noun method {method} must represent exactly its ledger rows",
            );
        }

        for forbidden in [
            "address_set",
            "interface_clear",
            "cancel_command",
            "pan_tilt_home",
            "zoom_stop",
            "defog_mode",
            "nr_speed",
        ] {
            let declaration = format!("pub async fn {forbidden}(");
            assert!(!source.contains(&declaration));
        }
    }
}

impl<'a, P: CompileTimeProfile> PowerAccessor<'a, P> {
    /// Inquires the camera's current power state.
    pub async fn state(&self) -> Result<bool> {
        self.camera.inquire(&command::PowerInquiry).await
    }

    /// Powers the camera on.
    pub async fn on(&self) -> Result<()> {
        self.camera.execute(&command::PowerOn::new()).await
    }

    /// Places the camera in standby.
    pub async fn off(&self) -> Result<()> {
        self.camera.execute(&command::PowerStandby::new()).await
    }
}

impl<'a, P: CompileTimeProfile> ZoomAccessor<'a, P> {
    /// Inquires the current optical/digital zoom position.
    pub async fn position(&self) -> Result<types::ZoomPosition> {
        self.camera.inquire(&command::ZoomPositionInquiry).await
    }

    /// Drives toward telephoto at standard speed.
    pub async fn tele(&self) -> Result<Operation<AppliedOnly>> {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::ZoomDrive::Tele)
            .await
    }

    /// Drives toward wide angle at standard speed.
    pub async fn wide(&self) -> Result<Operation<AppliedOnly>> {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::ZoomDrive::Wide)
            .await
    }

    /// Stops zoom movement.
    pub async fn stop(&self) -> Result<Operation<AppliedOnly>> {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::ZoomStop)
            .await
    }

    /// Drives toward telephoto at a validated variable speed.
    pub async fn tele_variable(&self, speed: types::ZoomSpeed) -> Result<Operation<AppliedOnly>> {
        let command = builtin::ZoomDrive::TeleVariable(speed);
        self.camera.submit::<AppliedOnly, _>(&command).await
    }

    /// Drives toward wide angle at a validated variable speed.
    pub async fn wide_variable(&self, speed: types::ZoomSpeed) -> Result<Operation<AppliedOnly>> {
        let command = builtin::ZoomDrive::WideVariable(speed);
        self.camera.submit::<AppliedOnly, _>(&command).await
    }

    /// Moves to an absolute zoom position.
    pub async fn set_position(&self, position: types::ZoomPosition) -> Result<Operation<Targeted>>
    where
        P: HasDirectZoom,
    {
        let command = builtin::ZoomTarget::new(position);
        self.camera.submit::<Targeted, _>(&command).await
    }

    /// Moves to a normalized position across the optical zoom range.
    ///
    /// `0.0` is the wide end and `1.0` the telephoto end of the profile's
    /// documented optical range.
    pub async fn set_normalized(&self, position: UnitInterval) -> Result<Operation<Targeted>>
    where
        P: HasDirectZoom,
    {
        let command = builtin::ZoomTarget::from_normalized(
            position,
            ZoomDomain::Optical,
            self.camera.profile(),
        )?;
        self.camera.submit::<Targeted, _>(&command).await
    }

    /// Moves to a normalized position across a documented zoom domain.
    ///
    /// [`ZoomDomain::OpticalPlusDigital`] requires the profile to document a
    /// digital maximum and never falls back to the optical range.
    pub async fn set_normalized_in_domain(
        &self,
        position: UnitInterval,
        domain: ZoomDomain,
    ) -> Result<Operation<Targeted>>
    where
        P: HasDirectZoom + HasDigitalZoomRange,
    {
        let command =
            builtin::ZoomTarget::from_normalized(position, domain, self.camera.profile())?;
        self.camera.submit::<Targeted, _>(&command).await
    }

    /// Enables or disables digital zoom.
    pub async fn set_digital_zoom(&self, enabled: bool) -> Result<()>
    where
        P: HasDigitalZoomToggle,
    {
        self.camera
            .execute(&command::DigitalZoom::new(enabled))
            .await
    }
}

impl<'a, P: CompileTimeProfile> SystemAccessor<'a, P> {
    /// Inquires the camera firmware/version information.
    pub async fn version(&self) -> Result<command::VersionInfo> {
        self.camera.inquire(&command::VersionInquiry).await
    }

    /// Saves the camera's current settings to non-volatile storage.
    pub async fn save_settings(&self) -> Result<()> {
        self.camera
            .execute(&command::SettingsSaveCommand::new())
            .await
    }
}

impl<'a, P: CompileTimeProfile> PanTiltAccessor<'a, P> {
    /// Inquires the current pan/tilt position.
    pub async fn position(&self) -> Result<crate::camera::PanTiltPosition> {
        self.camera.inquire(&command::PanTiltPositionInquiry).await
    }

    /// Moves the pan/tilt mechanism to its home position.
    pub async fn home(&self) -> Result<Operation<Targeted>> {
        self.camera
            .submit::<Targeted, _>(&builtin::PanTiltHome)
            .await
    }

    /// Resets the pan/tilt mechanism.
    pub async fn reset(&self) -> Result<Operation<Targeted>> {
        self.camera
            .submit::<Targeted, _>(&builtin::PanTiltReset)
            .await
    }

    /// Starts a directional pan/tilt drive.
    pub async fn move_direction(
        &self,
        direction: command::PanTiltDirection,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> Result<Operation<AppliedOnly>> {
        let command = builtin::PanTiltDrive::new(direction, pan_speed, tilt_speed)?;
        self.camera.submit::<AppliedOnly, _>(&command).await
    }

    /// Starts an upward pan/tilt drive.
    pub async fn up(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> Result<Operation<AppliedOnly>> {
        self.move_direction(command::PanTiltDirection::Up, pan_speed, tilt_speed)
            .await
    }

    /// Starts a downward pan/tilt drive.
    pub async fn down(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> Result<Operation<AppliedOnly>> {
        self.move_direction(command::PanTiltDirection::Down, pan_speed, tilt_speed)
            .await
    }

    /// Starts a leftward pan/tilt drive.
    pub async fn left(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> Result<Operation<AppliedOnly>> {
        self.move_direction(command::PanTiltDirection::Left, pan_speed, tilt_speed)
            .await
    }

    /// Starts a rightward pan/tilt drive.
    pub async fn right(
        &self,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> Result<Operation<AppliedOnly>> {
        self.move_direction(command::PanTiltDirection::Right, pan_speed, tilt_speed)
            .await
    }

    /// Stops pan/tilt movement using profile-safe stop speeds.
    pub async fn stop(&self) -> Result<Operation<AppliedOnly>> {
        let command = self.camera.core().pan_tilt_stop_request()?;
        self.camera.submit::<AppliedOnly, _>(&command).await
    }

    /// Moves to an absolute degree position at the selected speed.
    pub async fn absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> Result<Operation<Targeted>> {
        let pan_speed = types::PanSpeed::from(speed);
        let tilt_speed = types::TiltSpeed::from(speed);
        let command = builtin::PanTiltAbsolute::for_profile(
            pan,
            tilt,
            pan_speed,
            tilt_speed,
            self.camera.profile(),
        )?;
        self.camera.submit::<Targeted, _>(&command).await
    }

    /// Moves by a relative degree offset at the selected speed.
    pub async fn relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> Result<Operation<Targeted>> {
        let pan_speed = types::PanSpeed::from(speed);
        let tilt_speed = types::TiltSpeed::from(speed);
        let command = builtin::PanTiltRelative::for_profile(
            pan,
            tilt,
            pan_speed,
            tilt_speed,
            self.camera.profile(),
        )?;
        self.camera.submit::<Targeted, _>(&command).await
    }

    /// Sets one pan/tilt movement-limit corner.
    pub async fn limit_set(
        &self,
        corner: command::PanTiltLimitCorner,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
    ) -> Result<()> {
        let command =
            builtin::PanTiltLimitSet::for_profile(corner, pan, tilt, self.camera.profile())?;
        self.camera.execute(&command).await
    }

    /// Clears one pan/tilt movement-limit corner.
    pub async fn limit_clear(&self, corner: command::PanTiltLimitCorner) -> Result<()> {
        self.camera
            .execute(&builtin::PanTiltLimitClear::new(corner))
            .await
    }
}

impl<'a, P: CompileTimeProfile> FocusAccessor<'a, P> {
    /// Inquires the current focus position.
    pub async fn position(&self) -> Result<types::FocusPosition> {
        self.camera.inquire(&command::FocusPositionInquiry).await
    }

    /// Inquires the current focus mode.
    pub async fn mode(&self) -> Result<command::FocusMode> {
        self.camera.inquire(&command::FocusModeInquiry).await
    }

    /// Drives focus farther at standard speed.
    pub async fn far(&self) -> Result<Operation<AppliedOnly>> {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::FocusDrive::Far)
            .await
    }

    /// Drives focus nearer at standard speed.
    pub async fn near(&self) -> Result<Operation<AppliedOnly>> {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::FocusDrive::Near)
            .await
    }

    /// Drives focus farther at a variable speed.
    pub async fn far_variable(&self, speed: command::FocusSpeed) -> Result<Operation<AppliedOnly>> {
        let command = builtin::FocusDrive::FarVariable(speed);
        self.camera.submit::<AppliedOnly, _>(&command).await
    }

    /// Drives focus nearer at a variable speed.
    pub async fn near_variable(
        &self,
        speed: command::FocusSpeed,
    ) -> Result<Operation<AppliedOnly>> {
        let command = builtin::FocusDrive::NearVariable(speed);
        self.camera.submit::<AppliedOnly, _>(&command).await
    }

    /// Stops focus movement.
    pub async fn stop(&self) -> Result<Operation<AppliedOnly>> {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::FocusStop)
            .await
    }

    /// Moves focus to an absolute position.
    pub async fn set_position(
        &self,
        position: types::FocusPosition,
    ) -> Result<Operation<Targeted>> {
        let command = builtin::FocusTarget::new(position);
        self.camera.submit::<Targeted, _>(&command).await
    }

    /// Enables automatic focus mode.
    pub async fn auto(&self) -> Result<()> {
        self.camera.execute(&builtin::FocusModeCommand::Auto).await
    }

    /// Enables manual focus mode.
    pub async fn manual(&self) -> Result<()> {
        self.camera
            .execute(&builtin::FocusModeCommand::Manual)
            .await
    }

    /// Triggers one-push autofocus.
    pub async fn one_push(&self) -> Result<Operation<AppliedOnly>>
    where
        P: HasOnePushFocus,
    {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::FocusTrigger::OnePush)
            .await
    }

    /// Moves focus to infinity.
    pub async fn infinity(&self) -> Result<Operation<Targeted>> {
        self.camera
            .submit::<Targeted, _>(&builtin::FocusInfinity)
            .await
    }

    /// Toggles automatic/manual focus mode.
    pub async fn toggle(&self) -> Result<()> {
        self.camera
            .execute(&builtin::FocusModeCommand::Toggle)
            .await
    }

    /// Triggers vendor snap focus.
    pub async fn snap(&self) -> Result<Operation<AppliedOnly>>
    where
        P: crate::capabilities::HasPtzOpticsSnapFocus,
    {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::FocusTrigger::Snap)
            .await
    }

    /// Selects a focus zone.
    pub async fn set_zone(&self, zone: command::FocusZone) -> Result<()>
    where
        P: HasFocusZone,
    {
        self.camera
            .execute(&command::FocusZoneCommand::new(zone))
            .await
    }

    /// Sets the autofocus sensitivity.
    pub async fn set_sensitivity(&self, sensitivity: command::AutoFocusSensitivity) -> Result<()>
    where
        P: HasAutoFocusSensitivity,
    {
        self.camera
            .execute(&command::AutoFocusSensitivityCommand::new(sensitivity))
            .await
    }

    /// Sets the minimum focus distance.
    pub async fn set_near_limit(&self, position: types::FocusPosition) -> Result<()>
    where
        P: HasFocusNearLimitInquiry,
    {
        self.camera
            .execute(&command::FocusNearLimitCommand::new(position))
            .await
    }

    /// Sets the focus-lock mode.
    pub async fn set_lock(&self, mode: command::FocusLock) -> Result<()>
    where
        P: HasFocusLock,
    {
        self.camera.execute(&mode).await
    }

    /// Presses the vendor Push-AF control.
    pub async fn push_af_press(&self) -> Result<Operation<AppliedOnly>>
    where
        P: HasPushAutoFocus,
    {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::PushAfPress::new())
            .await
    }

    /// Releases the vendor Push-AF control.
    pub async fn push_af_release(&self) -> Result<Operation<AppliedOnly>>
    where
        P: HasPushAutoFocus,
    {
        self.camera
            .submit::<AppliedOnly, _>(&builtin::PushAfRelease::new())
            .await
    }

    /// Inquires the configured focus near limit.
    pub async fn near_limit(&self) -> Result<types::FocusPosition>
    where
        P: HasFocusNearLimitInquiry,
    {
        self.camera.inquire(&command::FocusNearLimitInquiry).await
    }

    /// Inquires the configured focus zone.
    pub async fn zone(&self) -> Result<command::FocusZone>
    where
        P: HasFocusZone,
    {
        self.camera.inquire(&command::FocusZoneInquiry).await
    }

    /// Inquires the autofocus sensitivity.
    pub async fn sensitivity(&self) -> Result<command::AutoFocusSensitivity>
    where
        P: HasAutoFocusSensitivity,
    {
        self.camera
            .inquire(&command::AutoFocusSensitivityInquiry)
            .await
    }

    /// Inquires the configured focus range.
    pub async fn range(&self) -> Result<command::FocusRange> {
        self.camera.inquire(&command::FocusRangeInquiry).await
    }
}

impl<'a, P: CompileTimeProfile> PresetsAccessor<'a, P> {
    /// Recalls a stored preset as a targeted operation.
    pub async fn recall(&self, preset: command::PresetNumber) -> Result<Operation<Targeted>> {
        let command = builtin::PresetRecall::for_profile(preset, self.camera.profile())?;
        self.camera.submit::<Targeted, _>(&command).await
    }

    /// Sets the preset-recall speed.
    pub async fn set_recall_speed(&self, speed: command::PresetRecallSpeed) -> Result<()> {
        self.camera
            .execute(&command::PresetRecallSpeedCommand::new(speed))
            .await
    }

    /// Stores the current camera state in a preset.
    pub async fn set(&self, preset: command::PresetNumber) -> Result<()> {
        self.camera.execute(&builtin::PresetSet::new(preset)).await
    }

    /// Clears a stored preset.
    pub async fn reset(&self, preset: command::PresetNumber) -> Result<()> {
        self.camera
            .execute(&builtin::PresetReset::new(preset))
            .await
    }
}

impl<'a, P: CompileTimeProfile> ExposureAccessor<'a, P> {
    /// Inquires the active exposure mode.
    pub async fn mode(&self) -> Result<command::ExposureMode> {
        self.camera.inquire(&command::ExposureModeInquiry).await
    }

    /// Sets the exposure mode.
    pub async fn set_mode(&self, mode: command::ExposureMode) -> Result<()> {
        self.camera
            .execute(&command::ExposureCommand::new(mode))
            .await
    }

    /// Inquires the shutter speed.
    pub async fn shutter(&self) -> Result<types::ShutterSpeed> {
        self.camera.inquire(&command::ShutterInquiry).await
    }

    /// Restores the camera's shutter default.
    pub async fn shutter_reset(&self) -> Result<()> {
        self.camera.execute(&command::Shutter::Reset).await
    }

    /// Increases shutter speed by one camera-defined step.
    pub async fn shutter_up(&self) -> Result<()> {
        self.camera.execute(&command::Shutter::Up).await
    }

    /// Decreases shutter speed by one camera-defined step.
    pub async fn shutter_down(&self) -> Result<()> {
        self.camera.execute(&command::Shutter::Down).await
    }

    /// Sets an explicit shutter speed.
    pub async fn shutter_direct(&self, speed: types::ShutterSpeed) -> Result<()> {
        self.camera
            .execute(&command::Shutter::SetSpeed(speed))
            .await
    }

    /// Inquires exposure compensation value.
    pub async fn compensation(&self) -> Result<types::ExposureCompensationLevel>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .inquire(&command::ExposureCompensationInquiry)
            .await
    }

    /// Inquires whether exposure compensation is enabled.
    pub async fn compensation_enabled(&self) -> Result<bool>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .inquire(&command::ExposureCompensationModeInquiry)
            .await
    }

    /// Inquires the camera's exposure compensation position.
    pub async fn compensation_position(&self) -> Result<types::ExposureCompensationPosition>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .inquire(&command::ExposureCompensationPositionInquiry)
            .await
    }

    /// Enables exposure compensation.
    pub async fn compensation_on(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .execute(&command::ExposureCompensation::On)
            .await
    }

    /// Disables exposure compensation.
    pub async fn compensation_off(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .execute(&command::ExposureCompensation::Off)
            .await
    }

    /// Resets exposure compensation.
    pub async fn compensation_reset(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .execute(&command::ExposureCompensation::Reset)
            .await
    }

    /// Increases exposure compensation by one step.
    pub async fn compensation_up(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .execute(&command::ExposureCompensation::Up)
            .await
    }

    /// Decreases exposure compensation by one step.
    pub async fn compensation_down(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .execute(&command::ExposureCompensation::Down)
            .await
    }

    /// Sets direct exposure compensation.
    pub async fn compensation_direct(&self, level: types::ExposureCompensationLevel) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        self.camera
            .execute(&command::ExposureCompensation::SetLevel(level))
            .await
    }

    /// Inquires the wide-dynamic-range level.
    pub async fn dynamic_range(&self) -> Result<types::DynamicRangeLevel>
    where
        P: HasWideDynamicRange,
    {
        self.camera.inquire(&command::DynamicRangeInquiry).await
    }

    /// Sets the wide-dynamic-range level.
    pub async fn set_dynamic_range(&self, level: types::DynamicRangeLevel) -> Result<()>
    where
        P: HasWideDynamicRange,
    {
        self.camera
            .execute(&command::DynamicRange::new(level))
            .await
    }

    /// Inquires whether iris control is automatic.
    pub async fn iris_control(&self) -> Result<bool>
    where
        P: HasIrisControl,
    {
        self.camera.inquire(&command::IrisControlInquiry).await
    }

    /// Inquires the iris level.
    pub async fn iris(&self) -> Result<types::IrisLevel>
    where
        P: HasIrisControl,
    {
        self.camera.inquire(&command::IrisInquiry).await
    }

    /// Resets the iris.
    pub async fn iris_reset(&self) -> Result<Operation<Targeted>>
    where
        P: HasIrisControl,
    {
        self.camera
            .submit::<Targeted, _>(&builtin::IrisReset::new())
            .await
    }

    /// Increases the iris by one step.
    pub async fn iris_up(&self) -> Result<Operation<Targeted>>
    where
        P: HasIrisControl,
    {
        self.camera
            .submit::<Targeted, _>(&builtin::IrisUp::new())
            .await
    }

    /// Decreases the iris by one step.
    pub async fn iris_down(&self) -> Result<Operation<Targeted>>
    where
        P: HasIrisControl,
    {
        self.camera
            .submit::<Targeted, _>(&builtin::IrisDown::new())
            .await
    }

    /// Sets a direct iris level.
    pub async fn iris_direct(&self, level: types::IrisLevel) -> Result<Operation<Targeted>>
    where
        P: HasIrisControl,
    {
        self.camera
            .submit::<Targeted, _>(&builtin::IrisDirect::new(level))
            .await
    }

    /// Inquires exposure brightness.
    pub async fn brightness(&self) -> Result<types::BrightnessLevel>
    where
        P: HasBrightnessControl,
    {
        self.camera.inquire(&command::BrightnessInquiry).await
    }

    /// Resets exposure brightness.
    pub async fn brightness_reset(&self) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        self.camera.execute(&command::Brightness::Reset).await
    }

    /// Increases exposure brightness.
    pub async fn brightness_up(&self) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        self.camera.execute(&command::Brightness::Up).await
    }

    /// Decreases exposure brightness.
    pub async fn brightness_down(&self) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        self.camera.execute(&command::Brightness::Down).await
    }

    /// Sets exposure brightness through the bright-direct command.
    pub async fn brightness_set(&self, level: types::BrightnessLevel) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        self.camera
            .execute(&command::Brightness::SetLevel(level))
            .await
    }

    /// Sets the camera's direct brightness value.
    pub async fn brightness_direct(&self, level: types::BrightnessLevel) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        self.camera
            .execute(&command::Brightness::Direct(level))
            .await
    }

    /// Inquires gain.
    pub async fn gain(&self) -> Result<types::GainLevel> {
        self.camera.inquire(&command::GainInquiry).await
    }

    /// Resets gain.
    pub async fn gain_reset(&self) -> Result<()> {
        self.camera.execute(&command::Gain::Reset).await
    }

    /// Increases gain.
    pub async fn gain_up(&self) -> Result<()> {
        self.camera.execute(&command::Gain::Up).await
    }

    /// Decreases gain.
    pub async fn gain_down(&self) -> Result<()> {
        self.camera.execute(&command::Gain::Down).await
    }

    /// Sets direct gain.
    pub async fn gain_direct(&self, level: types::GainLevel) -> Result<()> {
        self.camera.execute(&command::Gain::SetValue(level)).await
    }

    /// Inquires the configured gain limit.
    pub async fn gain_limit(&self) -> Result<types::GainLimit> {
        self.camera.inquire(&command::GainLimitInquiry).await
    }

    /// Sets the configured gain limit.
    pub async fn set_gain_limit(&self, limit: types::GainLimit) -> Result<()> {
        self.camera
            .execute(&command::GainLimitCommand::new(limit))
            .await
    }

    /// Sets anti-flicker mode.
    pub async fn set_anti_flicker(&self, mode: command::AntiFlickerMode) -> Result<()> {
        self.camera
            .execute(&command::AntiFlickerCommand::new(mode))
            .await
    }

    /// Inquires the configured anti-flicker mode.
    pub async fn flicker_mode(&self) -> Result<command::AntiFlickerMode> {
        self.camera.inquire(&command::FlickerModeInquiry).await
    }

    /// Enables spotlight mode.
    pub async fn spotlight_on(&self) -> Result<()> {
        self.camera.execute(&command::SpotlightOn::new()).await
    }

    /// Disables spotlight mode.
    pub async fn spotlight_off(&self) -> Result<()> {
        self.camera.execute(&command::SpotlightOff::new()).await
    }

    /// Enables automatic slow shutter.
    pub async fn auto_slow_shutter_on(&self) -> Result<()> {
        self.camera
            .execute(&command::AutoSlowShutterOn::new())
            .await
    }

    /// Disables automatic slow shutter.
    pub async fn auto_slow_shutter_off(&self) -> Result<()> {
        self.camera
            .execute(&command::AutoSlowShutterOff::new())
            .await
    }
}

impl<'a, P: CompileTimeProfile> WhiteBalanceAccessor<'a, P> {
    /// Inquires the active white-balance mode.
    pub async fn mode(&self) -> Result<command::WhiteBalanceMode> {
        self.camera.inquire(&command::WhiteBalanceModeInquiry).await
    }

    /// Selects automatic white balance.
    pub async fn auto(&self) -> Result<()> {
        self.camera
            .execute(&command::WhiteBalanceCommand::new(
                command::WhiteBalanceMode::Auto,
            ))
            .await
    }

    /// Selects the indoor white-balance preset.
    pub async fn indoor(&self) -> Result<()> {
        self.camera
            .execute(&command::WhiteBalanceCommand::new(
                command::WhiteBalanceMode::Indoor,
            ))
            .await
    }

    /// Selects the outdoor white-balance preset.
    pub async fn outdoor(&self) -> Result<()> {
        self.camera
            .execute(&command::WhiteBalanceCommand::new(
                command::WhiteBalanceMode::Outdoor,
            ))
            .await
    }

    /// Selects one-push white balance.
    pub async fn one_push(&self) -> Result<()>
    where
        P: HasOnePushWhiteBalance,
    {
        self.camera
            .execute(&command::WhiteBalanceCommand::new(
                command::WhiteBalanceMode::OnePush,
            ))
            .await
    }

    /// Selects auto-tracking white balance.
    pub async fn atw(&self) -> Result<()>
    where
        P: HasAutoTrackingWhiteBalance,
    {
        self.camera
            .execute(&command::WhiteBalanceCommand::new(
                command::WhiteBalanceMode::ATW,
            ))
            .await
    }

    /// Selects manual white balance.
    pub async fn manual(&self) -> Result<()> {
        self.camera
            .execute(&command::WhiteBalanceCommand::new(
                command::WhiteBalanceMode::Manual,
            ))
            .await
    }

    /// Selects color-temperature white balance mode.
    pub async fn color_temperature_mode(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        self.camera
            .execute(&command::WhiteBalanceCommand::new(
                command::WhiteBalanceMode::ColorTemperature,
            ))
            .await
    }

    /// Sets automatic white-balance sensitivity.
    pub async fn set_sensitivity(
        &self,
        sensitivity: command::AutoWhiteBalanceSensitivity,
    ) -> Result<()>
    where
        P: HasAutoWhiteBalanceSensitivity,
    {
        self.camera
            .execute(&command::AWBSensitivityCommand::new(sensitivity))
            .await
    }

    /// Inquires automatic white-balance sensitivity.
    pub async fn sensitivity(&self) -> Result<command::AutoWhiteBalanceSensitivity>
    where
        P: HasAutoWhiteBalanceSensitivity,
    {
        self.camera
            .inquire(&command::AutoWhiteBalanceSensitivityInquiry)
            .await
    }

    /// Triggers one-push white-balance calibration.
    pub async fn one_push_trigger(&self) -> Result<()>
    where
        P: HasOnePushWhiteBalance,
    {
        self.camera
            .execute(&command::OnePushTriggerCommand::new())
            .await
    }

    /// Sets red-channel white-balance tuning.
    pub async fn set_red_tuning(&self, level: types::RedTuning) -> Result<()>
    where
        P: HasRgbTuning,
    {
        self.camera
            .execute(&command::RedTuningCommand::new(level))
            .await
    }

    /// Sets blue-channel white-balance tuning.
    pub async fn set_blue_tuning(&self, level: types::BlueTuning) -> Result<()>
    where
        P: HasRgbTuning,
    {
        self.camera
            .execute(&command::BlueTuningCommand::new(level))
            .await
    }

    /// Inquires the color temperature.
    pub async fn color_temperature(&self) -> Result<types::ColorTemp>
    where
        P: HasColorTemperature,
    {
        self.camera.inquire(&command::ColorTemperatureInquiry).await
    }

    /// Resets color temperature.
    pub async fn reset_color_temperature(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        self.camera.execute(&command::ColorTemperature::Reset).await
    }

    /// Increases color temperature.
    pub async fn increase_color_temperature(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        self.camera.execute(&command::ColorTemperature::Up).await
    }

    /// Decreases color temperature.
    pub async fn decrease_color_temperature(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        self.camera.execute(&command::ColorTemperature::Down).await
    }

    /// Sets a direct color-temperature value.
    pub async fn set_color_temperature(&self, temperature: types::ColorTemp) -> Result<()>
    where
        P: HasColorTemperature,
    {
        self.camera
            .execute(&command::ColorTemperature::SetTemperature(temperature))
            .await
    }

    /// Inquires the red-channel gain.
    pub async fn red_gain(&self) -> Result<types::RedChannel>
    where
        P: HasRgbGain,
    {
        self.camera.inquire(&command::RedGainInquiry).await
    }

    /// Resets red-channel gain.
    pub async fn reset_red_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera.execute(&command::RedGain::Reset).await
    }

    /// Increases red-channel gain.
    pub async fn increase_red_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera.execute(&command::RedGain::Up).await
    }

    /// Decreases red-channel gain.
    pub async fn decrease_red_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera.execute(&command::RedGain::Down).await
    }

    /// Sets direct red-channel gain.
    pub async fn set_red_gain(&self, value: types::RedChannel) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera
            .execute(&command::RedGain::SetValue(value))
            .await
    }

    /// Inquires the blue-channel gain.
    pub async fn blue_gain(&self) -> Result<types::BlueChannel>
    where
        P: HasRgbGain,
    {
        self.camera.inquire(&command::BlueGainInquiry).await
    }

    /// Resets blue-channel gain.
    pub async fn reset_blue_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera.execute(&command::BlueGain::Reset).await
    }

    /// Increases blue-channel gain.
    pub async fn increase_blue_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera.execute(&command::BlueGain::Up).await
    }

    /// Decreases blue-channel gain.
    pub async fn decrease_blue_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera.execute(&command::BlueGain::Down).await
    }

    /// Sets direct blue-channel gain.
    pub async fn set_blue_gain(&self, value: types::BlueChannel) -> Result<()>
    where
        P: HasRgbGain,
    {
        self.camera
            .execute(&command::BlueGain::SetValue(value))
            .await
    }

    /// Inquires red-channel tuning.
    pub async fn red_tuning(&self) -> Result<types::RedTuning>
    where
        P: HasRgbTuning,
    {
        self.camera.inquire(&command::RedTuningInquiry).await
    }

    /// Inquires blue-channel tuning.
    pub async fn blue_tuning(&self) -> Result<types::BlueTuning>
    where
        P: HasRgbTuning,
    {
        self.camera.inquire(&command::BlueTuningInquiry).await
    }
}

impl<'a, P: CompileTimeProfile> ImageAccessor<'a, P> {
    /// Inquires the camera resolution mode.
    pub async fn resolution(&self) -> Result<command::ResolutionMode> {
        self.camera.inquire(&command::ResolutionInquiry).await
    }

    /// Inquires image saturation.
    pub async fn saturation(&self) -> Result<types::SaturationLevel>
    where
        P: HasSaturationControl,
    {
        self.camera.inquire(&command::SaturationInquiry).await
    }

    /// Sets image saturation.
    pub async fn set_saturation(&self, level: types::SaturationLevel) -> Result<()>
    where
        P: HasSaturationControl,
    {
        self.camera
            .execute(&command::SaturationCommand::new(level))
            .await
    }

    /// Inquires image hue.
    pub async fn hue(&self) -> Result<types::HueLevel>
    where
        P: HasHueControl,
    {
        self.camera.inquire(&command::HueInquiry).await
    }

    /// Sets image hue.
    pub async fn set_hue(&self, level: types::HueLevel) -> Result<()>
    where
        P: HasHueControl,
    {
        self.camera.execute(&command::HueCommand::new(level)).await
    }

    /// Inquires image luminance.
    pub async fn luminance(&self) -> Result<types::LuminanceLevel>
    where
        P: HasLuminanceControl,
    {
        self.camera.inquire(&command::LuminanceInquiry).await
    }

    /// Sets image luminance.
    pub async fn set_luminance(&self, level: types::LuminanceLevel) -> Result<()>
    where
        P: HasLuminanceControl,
    {
        self.camera.execute(&command::Luminance::new(level)).await
    }

    /// Inquires image contrast.
    pub async fn contrast(&self) -> Result<types::ContrastLevel>
    where
        P: HasContrastControl,
    {
        self.camera.inquire(&command::ContrastInquiry).await
    }

    /// Sets image contrast.
    pub async fn set_contrast(&self, level: types::ContrastLevel) -> Result<()>
    where
        P: HasContrastControl,
    {
        self.camera.execute(&command::Contrast::new(level)).await
    }

    /// Inquires the gamma curve.
    pub async fn gamma(&self) -> Result<types::GammaLevel>
    where
        P: HasGammaControl,
    {
        self.camera.inquire(&command::GammaInquiry).await
    }

    /// Sets the gamma curve.
    pub async fn set_gamma(&self, level: types::GammaLevel) -> Result<()>
    where
        P: HasGammaControl,
    {
        self.camera
            .execute(&command::GammaCommand::new(level))
            .await
    }

    /// Inquires the sharpness mode.
    pub async fn sharpness_mode(&self) -> Result<command::SharpnessMode>
    where
        P: HasSharpnessControl,
    {
        self.camera.inquire(&command::SharpnessModeInquiry).await
    }

    /// Inquires the sharpness level.
    pub async fn sharpness_level(&self) -> Result<types::SharpnessLevel>
    where
        P: HasSharpnessControl,
    {
        self.camera
            .inquire(&command::SharpnessPositionInquiry)
            .await
    }

    /// Sets the sharpness mode.
    pub async fn set_sharpness_mode(&self, mode: command::SharpnessMode) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        self.camera.execute(&command::Sharpness::Mode(mode)).await
    }

    /// Resets sharpness.
    pub async fn reset_sharpness(&self) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        self.camera.execute(&command::Sharpness::Reset).await
    }

    /// Increases sharpness by one step.
    pub async fn increase_sharpness(&self) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        self.camera.execute(&command::Sharpness::Up).await
    }

    /// Decreases sharpness by one step.
    pub async fn decrease_sharpness(&self) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        self.camera.execute(&command::Sharpness::Down).await
    }

    /// Sets a direct sharpness level.
    pub async fn set_sharpness(&self, level: types::SharpnessLevel) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        self.camera
            .execute(&command::Sharpness::SetLevel {
                value: level.value(),
            })
            .await
    }

    /// Inquires backlight compensation state.
    pub async fn backlight(&self) -> Result<bool>
    where
        P: HasBacklightCompensation,
    {
        self.camera.inquire(&command::BacklightInquiry).await
    }

    /// Enables or disables backlight compensation.
    pub async fn set_backlight(&self, enabled: bool) -> Result<()>
    where
        P: HasBacklightCompensation,
    {
        self.camera
            .execute(&command::BacklightCommand::new(enabled))
            .await
    }

    /// Inquires 2D noise reduction level.
    pub async fn noise_reduction_2d(&self) -> Result<types::NoiseReduction2DLevel>
    where
        P: HasNoiseReduction2D,
    {
        self.camera.inquire(&command::NoiseReduction2DInquiry).await
    }

    /// Sets 2D noise reduction level.
    pub async fn set_noise_reduction_2d(&self, level: types::NoiseReduction2DLevel) -> Result<()>
    where
        P: HasNoiseReduction2D,
    {
        self.camera
            .execute(&command::NoiseReduction2D::with_level(level))
            .await
    }

    /// Inquires 3D noise reduction level.
    pub async fn noise_reduction_3d(&self) -> Result<types::NoiseReduction3DLevel>
    where
        P: HasNoiseReduction3D,
    {
        self.camera.inquire(&command::NoiseReduction3DInquiry).await
    }

    /// Sets 3D noise reduction level.
    pub async fn set_noise_reduction_3d(&self, level: types::NoiseReduction3DLevel) -> Result<()>
    where
        P: HasNoiseReduction3D,
    {
        self.camera
            .execute(&command::NoiseReduction3D::with_level(level))
            .await
    }

    /// Inquires the aggregate noise-reduction level.
    pub async fn noise_reduction_level(&self) -> Result<types::NoiseReductionLevel>
    where
        P: HasNoiseReduction,
    {
        self.camera.inquire(&command::NrLevelInquiry).await
    }

    /// Inquires the aggregate noise-reduction mode.
    pub async fn noise_reduction_mode(&self) -> Result<command::NoiseReductionMode>
    where
        P: HasNoiseReduction,
    {
        self.camera.inquire(&command::NrModeInquiry).await
    }

    /// Disables vertical image flip.
    pub async fn disable_flip(&self) -> Result<()>
    where
        P: HasImageFlip,
    {
        self.camera
            .execute(&builtin::ImageFlipCommand::new(command::Flip::Off))
            .await
    }

    /// Enables vertical image flip.
    pub async fn enable_flip(&self) -> Result<()>
    where
        P: HasImageFlip,
    {
        self.camera
            .execute(&builtin::ImageFlipCommand::new(command::Flip::On))
            .await
    }

    /// Enables horizontal image mirroring.
    pub async fn enable_horizontal_flip(&self) -> Result<()>
    where
        P: HasImageMirror,
    {
        self.camera
            .execute(&builtin::ImageMirrorCommand::new(true))
            .await
    }

    /// Disables horizontal image mirroring.
    pub async fn disable_horizontal_flip(&self) -> Result<()>
    where
        P: HasImageMirror,
    {
        self.camera
            .execute(&builtin::ImageMirrorCommand::new(false))
            .await
    }

    /// Sets the combined image-flip mode to both axes.
    pub async fn set_flip_both(&self) -> Result<()>
    where
        P: HasImageFlip,
    {
        self.camera
            .execute(&command::ImageFlipCombinedCommand::new(
                command::ImageFlipMode::Both,
            ))
            .await
    }

    /// Sets the combined image-flip mode.
    pub async fn set_flip_mode(&self, mode: command::ImageFlipMode) -> Result<()>
    where
        P: HasCombinedImageFlip,
    {
        self.camera
            .execute(&command::ImageFlipCombinedCommand::new(mode))
            .await
    }

    /// Freezes the image.
    pub async fn freeze_on(&self) -> Result<()> {
        self.camera.execute(&command::ImageFreeze::on()).await
    }

    /// Resumes live image output.
    pub async fn freeze_off(&self) -> Result<()> {
        self.camera.execute(&command::ImageFreeze::off()).await
    }

    /// Inquires the canonical image-flip state.
    pub async fn flip(&self) -> Result<command::FlipState>
    where
        P: HasImageFlip,
    {
        self.camera.inquire(&command::ImageFlipInquiry).await
    }

    /// Inquires the combined image-flip mode.
    pub async fn flip_mode(&self) -> Result<command::FlipState>
    where
        P: HasImageFlip,
    {
        self.camera.inquire(&command::FlipStateInquiry).await
    }

    /// Inquires whether black-and-white mode is active.
    pub async fn black_white(&self) -> Result<bool>
    where
        P: HasPictureEffect,
    {
        self.camera.inquire(&command::BlackWhiteInquiry).await
    }

    /// Inquires black-and-white mode.
    pub async fn black_white_mode(&self) -> Result<command::BlackWhiteMode>
    where
        P: HasPictureEffect,
    {
        self.camera.inquire(&command::BlackWhiteModeInquiry).await
    }

    /// Inquires picture-effect mode.
    pub async fn picture_effect(&self) -> Result<command::PictureEffectMode>
    where
        P: HasPictureEffect,
    {
        self.camera.inquire(&command::PictureEffectInquiry).await
    }

    /// Sets picture-effect mode.
    pub async fn set_picture_effect(&self, mode: command::PictureEffectMode) -> Result<()>
    where
        P: HasPictureEffect,
    {
        self.camera
            .execute(&command::PictureEffectCommand::new(mode))
            .await
    }

    /// Inquires the camera's defog level.
    pub async fn defog_level(&self) -> Result<types::DefogLevel> {
        self.camera.inquire(&command::DefogLevelInquiry).await
    }
}

impl<'a, P: CompileTimeProfile> MenuAccessor<'a, P> {
    /// Inquires whether the on-screen menu is open.
    pub async fn status(&self) -> Result<bool> {
        self.camera.inquire(&command::MenuOpenCloseInquiry).await
    }

    /// Displays or hides the on-screen menu.
    pub async fn display(&self, on: bool) -> Result<()> {
        self.camera.execute(&command::SetMenuDisplay::new(on)).await
    }

    /// Moves the menu cursor.
    pub async fn navigate(&self, direction: command::MenuDirection) -> Result<()> {
        self.camera
            .execute(&command::MenuNavigate::new(direction))
            .await
    }

    /// Selects the current menu item.
    pub async fn select(&self) -> Result<()> {
        self.camera
            .execute(&command::PerformMenuAction::new(
                command::MenuAction::Select,
            ))
            .await
    }

    /// Cancels or returns from the current menu item.
    pub async fn cancel(&self) -> Result<()> {
        self.camera
            .execute(&command::PerformMenuAction::new(
                command::MenuAction::Cancel,
            ))
            .await
    }

    /// Sends a vendor-specific direct menu control.
    pub async fn direct(&self, control1: u8, control2: u8) -> Result<()>
    where
        P: crate::capabilities::HasDirectMenuControl,
    {
        self.camera
            .execute(&command::DirectMenuControl::new(control1, control2))
            .await
    }

    /// Toggles the on-screen menu open or closed.
    ///
    /// This is the vendor open/close direct control, so it needs no prior
    /// [`Self::status`] round trip to decide which way to move.
    pub async fn toggle_display(&self) -> Result<()>
    where
        P: crate::capabilities::HasDirectMenuControl,
    {
        self.camera
            .execute(&command::DirectMenuControl::open_close())
            .await
    }
}

impl<'a, P: CompileTimeProfile> AdvancedAccessor<'a, P> {
    /// Inquires night/day mode.
    pub async fn night_day_mode(&self) -> Result<bool> {
        self.camera.inquire(&command::NightDayModeInquiry).await
    }

    /// Inquires standby state.
    pub async fn standby_enabled(&self) -> Result<bool> {
        self.camera.inquire(&command::StandbyInquiry).await
    }

    /// Inquires digital PTZ state.
    pub async fn digital_ptz_enabled(&self) -> Result<bool> {
        self.camera.inquire(&command::DigitalPtzInquiry).await
    }

    /// Inquires auto-trace state.
    pub async fn auto_trace_enabled(&self) -> Result<bool> {
        self.camera.inquire(&command::AutoTraceInquiry).await
    }

    /// Inquires focus-unlock state.
    pub async fn focus_unlock(&self) -> Result<bool> {
        self.camera.inquire(&command::FocusUnlockInquiry).await
    }

    /// Inquires the broadcast domain.
    pub async fn broadcast_domain(&self) -> Result<types::BroadcastDomain> {
        self.camera.inquire(&command::BroadcastDomainInquiry).await
    }

    /// Inquires USB-audio state.
    pub async fn usb_audio_enabled(&self) -> Result<bool> {
        self.camera.inquire(&command::UsbAudioInquiry).await
    }

    /// Inquires two-tone mode.
    pub async fn two_tone_mode_enabled(&self) -> Result<bool> {
        self.camera.inquire(&command::TwoToneModeInquiry).await
    }

    /// Inquires digital mode.
    pub async fn digital_mode_enabled(&self) -> Result<bool> {
        self.camera.inquire(&command::DigitalInquiry).await
    }

    /// Enables multicast streaming.
    pub async fn multicast_on(&self) -> Result<()> {
        self.camera.execute(&command::MulticastStreaming::On).await
    }

    /// Disables multicast streaming.
    pub async fn multicast_off(&self) -> Result<()> {
        self.camera.execute(&command::MulticastStreaming::Off).await
    }

    /// Sets NDI streaming quality.
    pub async fn set_ndi_quality(&self, quality: types::NdiQuality) -> Result<()> {
        self.camera
            .execute(&command::SetNdiQuality::new(quality))
            .await
    }

    /// Enables USB audio.
    pub async fn usb_audio_on(&self) -> Result<()> {
        self.camera.execute(&command::UsbAudio::On).await
    }

    /// Disables USB audio.
    pub async fn usb_audio_off(&self) -> Result<()> {
        self.camera.execute(&command::UsbAudio::Off).await
    }

    /// Sets pan/tilt variable-speed mode.
    pub async fn set_variable_speed_mode(&self, mode: command::VariableSpeedMode) -> Result<()>
    where
        P: HasVariableSpeed,
    {
        self.camera
            .execute(&command::SetVariableSpeedMode::new(mode))
            .await
    }
}

impl<'a, P: CompileTimeProfile + HasTally> TallyAccessor<'a, P> {
    fn new(camera: &'a Camera<P>) -> Self {
        Self { camera }
    }

    /// Inquires all tally light state.
    pub async fn status(&self) -> Result<command::TallyStatusState> {
        self.camera.inquire(&command::TallyStatusInquiry).await
    }

    /// Turns the red tally on.
    pub async fn red_on(&self) -> Result<()> {
        self.camera.execute(&command::TallyRedOn::new()).await
    }

    /// Turns the red tally off.
    pub async fn red_off(&self) -> Result<()> {
        self.camera.execute(&command::TallyRedOff::new()).await
    }

    /// Sets low tally brightness.
    pub async fn bright_lo(&self) -> Result<()> {
        self.camera.execute(&command::TallyBrightLo::new()).await
    }

    /// Sets high tally brightness.
    pub async fn bright_hi(&self) -> Result<()> {
        self.camera.execute(&command::TallyBrightHi::new()).await
    }

    /// Turns the green tally on.
    pub async fn green_on(&self) -> Result<()> {
        self.camera.execute(&command::TallyGreenOn::new()).await
    }

    /// Turns the green tally off.
    pub async fn green_off(&self) -> Result<()> {
        self.camera.execute(&command::TallyGreenOff::new()).await
    }

    /// Sets tally flash mode.
    pub async fn flash(&self) -> Result<()> {
        self.camera.execute(&command::TallyFlash::new()).await
    }

    /// Sets tally solid-on mode.
    pub async fn on(&self) -> Result<()> {
        self.camera.execute(&command::TallyOn::new()).await
    }

    /// Turns tally output off.
    pub async fn off(&self) -> Result<()> {
        self.camera.execute(&command::TallyOff::new()).await
    }

    /// Inquires red tally state.
    pub async fn red_status(&self) -> Result<bool> {
        self.camera.inquire(&command::TallyRedInquiry).await
    }

    /// Inquires green tally state.
    pub async fn green_status(&self) -> Result<bool> {
        self.camera.inquire(&command::TallyGreenInquiry).await
    }

    /// Inquires automatic tally adjustment state.
    pub async fn auto_adjust_enabled(&self) -> Result<bool> {
        self.camera.inquire(&command::TallyAutoAdjustInquiry).await
    }
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the tally accessor for a profile that declares tally support.
    pub fn tally(&self) -> TallyAccessor<'_, P>
    where
        P: HasTally,
    {
        TallyAccessor::new(self)
    }
}

impl<'a, P: CompileTimeProfile + HasNdFilter> NdFilterAccessor<'a, P> {
    fn new(camera: &'a Camera<P>) -> Self {
        Self { camera }
    }

    /// Inquires the current ND-filter position.
    pub async fn position(&self) -> Result<command::NdFilterPosition> {
        self.camera.inquire(&command::NdFilterInquiry).await
    }

    /// Inquires the current ND-filter preset.
    pub async fn preset(&self) -> Result<types::NdFilterPreset> {
        self.camera.inquire(&command::NdFilterPresetInquiry).await
    }

    /// Selects preset or variable ND-filter mode.
    pub async fn set_mode(&self, mode: command::NdFilterMode) -> Result<()> {
        self.camera
            .execute(&command::NdFilterModeCommand::new(mode))
            .await
    }

    /// Sets a direct variable ND-filter value.
    pub async fn set_value(&self, value: u16) -> Result<Operation<Targeted>> {
        let value = command::NdFilterValue::new(value)?;
        let request = builtin::NdFilterDirect::new(value);
        self.camera.submit::<Targeted, _>(&request).await
    }

    /// Sets a direct variable ND-filter value in photographic stops.
    ///
    /// `stops` is the light reduction in stops and must lie in `2.0..=7.0`.
    /// Each raw unit is a quarter stop, so `2.0` maps to the minimum density
    /// and `7.0` to the maximum.
    pub async fn set_stops(&self, stops: f32) -> Result<Operation<Targeted>> {
        let value = command::NdFilterValue::from_stops(stops)?;
        let request = builtin::NdFilterDirect::new(value);
        self.camera.submit::<Targeted, _>(&request).await
    }

    /// Increases ND-filter density by one step.
    pub async fn step_up(&self) -> Result<Operation<Targeted>> {
        self.camera
            .submit::<Targeted, _>(&builtin::NdFilterStepUp::new())
            .await
    }

    /// Decreases ND-filter density by one step.
    pub async fn step_down(&self) -> Result<Operation<Targeted>> {
        self.camera
            .submit::<Targeted, _>(&builtin::NdFilterStepDown::new())
            .await
    }

    /// Enables automatic ND filtering.
    pub async fn auto_on(&self) -> Result<()> {
        self.camera
            .execute(&command::AutoNdCommand::new(true))
            .await
    }

    /// Disables automatic ND filtering.
    pub async fn auto_off(&self) -> Result<()> {
        self.camera
            .execute(&command::AutoNdCommand::new(false))
            .await
    }
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the ND-filter accessor for a profile that declares ND support.
    pub fn nd_filter(&self) -> NdFilterAccessor<'_, P>
    where
        P: HasNdFilter,
    {
        NdFilterAccessor::new(self)
    }
}

impl<'a, P: CompileTimeProfile + HasMotionSync> MotionSyncAccessor<'a, P> {
    fn new(camera: &'a Camera<P>) -> Self {
        Self { camera }
    }

    /// Inquires the motion-sync mode.
    pub async fn mode(&self) -> Result<command::MotionSyncMode> {
        self.camera.inquire(&command::MotionSyncModeInquiry).await
    }

    /// Inquires the motion-sync preset speed.
    pub async fn preset(&self) -> Result<command::MotionSyncPreset> {
        self.camera.inquire(&command::MotionSyncPresetInquiry).await
    }

    /// Enables or disables motion synchronization.
    pub async fn set_mode(&self, mode: command::MotionSyncMode) -> Result<()> {
        self.camera
            .execute(&command::SetMotionSyncMode::new(mode))
            .await
    }

    /// Sets the motion-sync speed preset.
    pub async fn set_preset(&self, speed: u8) -> Result<()> {
        self.camera
            .execute(&command::SetMotionSyncPreset::new(speed)?)
            .await
    }

    /// Sets the motion-sync speed from a range-checked speed value.
    ///
    /// This is [`Self::set_preset`] with the `1..=24` bound moved into the
    /// argument type, so an out-of-range speed cannot be constructed.
    pub async fn set_speed(&self, speed: types::MotionSyncSpeed) -> Result<()> {
        self.camera
            .execute(&command::SetMotionSyncPreset::new(speed.value())?)
            .await
    }
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the motion-sync accessor for a profile that declares support.
    pub fn motion_sync(&self) -> MotionSyncAccessor<'_, P>
    where
        P: HasMotionSync,
    {
        MotionSyncAccessor::new(self)
    }
}
