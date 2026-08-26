//! Canonical static noun views for the blocking owner-backed camera.
//!
//! The accessors in this module are deliberately small borrowed views.  They
//! do not own a transport, create a runtime, or introduce another command
//! path: every operation goes through [`super::BlockingCameraCore`].  The
//! method spelling follows the closed ledger in `command::surface` and the
//! return class is visible in the operation handle (`AppliedOnly` or
//! `Targeted`).

use std::fmt;

use crate::{
    camera::{IdleWait, MotionQuery, PanTiltPosition},
    capabilities::{
        HasAutoFocusSensitivity, HasAutoTrackingWhiteBalance, HasAutoWhiteBalanceSensitivity,
        HasBacklightCompensation, HasBrightnessControl, HasColorTemperature, HasCombinedImageFlip,
        HasContrastControl, HasDigitalZoomToggle, HasDirectMenuControl, HasDirectZoom, HasExposure,
        HasExposureCompensation, HasFocus, HasFocusLock, HasFocusNearLimitInquiry, HasFocusZone,
        HasGammaControl, HasHueControl, HasImageFlip, HasImageMirror, HasImageProcessing,
        HasIrisControl, HasLuminanceControl, HasMenuControl, HasMotionSync, HasNdFilter,
        HasNoiseReduction, HasNoiseReduction2D, HasNoiseReduction3D, HasOnePushFocus,
        HasOnePushWhiteBalance, HasPanTilt, HasPictureEffect, HasPower, HasPresets,
        HasPushAutoFocus, HasRgbGain, HasRgbTuning, HasSaturationControl, HasSharpnessControl,
        HasTally, HasVariableSpeed, HasWhiteBalance, HasWideDynamicRange, HasZoom,
    },
    command,
    completion::{self, AppliedOnly, Targeted},
    request::builtin,
    types,
    units::Degrees,
    CompileTimeProfile, Inquiry, OperationCommand, PlainCommand, Result,
};

use super::{Camera, Operation};

macro_rules! accessor_method {
    ($(#[$meta:meta])* $name:ident, $method:ident) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, 'session, P> {
            $name::new(self)
        }
    };
    ($(#[$meta:meta])* $name:ident, $method:ident, $bound:path) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, 'session, P>
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
        pub struct $name<'view, 'session, P: CompileTimeProfile> {
            camera: &'view Camera<'session, P>,
        }

        impl<P: CompileTimeProfile> fmt::Debug for $name<'_, '_, P> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }

        impl<'view, 'session, P: CompileTimeProfile> $name<'view, 'session, P> {
            fn new(camera: &'view Camera<'session, P>) -> Self {
                Self { camera }
            }
        }

        impl<'session, P: CompileTimeProfile> Camera<'session, P> {
            accessor_method!($(#[$meta])* $name, $method $(, $bound)?);
        }
    };
}

accessor!(
    /// Power controls and the power-state inquiry.
    PowerAccessor,
    power,
    HasPower
);
accessor!(
    /// Optical and digital zoom controls.
    ZoomAccessor,
    zoom,
    HasZoom
);
accessor!(
    /// Firmware/version and persistence controls.
    SystemAccessor,
    system
);
accessor!(
    /// Pan/tilt movement, limits, and position inquiry.
    PanTiltAccessor,
    pan_tilt,
    HasPanTilt
);
accessor!(
    /// Focus movement, modes, and typed focus inquiries.
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
    /// Image-processing, noise-reduction, and orientation controls.
    ImageAccessor,
    image,
    HasImageProcessing
);
accessor!(
    /// Preset commands and recall-speed control.
    PresetsAccessor,
    presets,
    HasPresets
);
accessor!(
    /// Menu display and navigation controls.
    MenuAccessor,
    menu,
    HasMenuControl
);
accessor!(
    /// Streaming, vendor, and variable-speed controls.
    AdvancedAccessor,
    advanced
);

/// Tally controls for a profile which declares tally support.
#[must_use]
pub struct TallyAccessor<'view, 'session, P: CompileTimeProfile + HasTally> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile + HasTally> fmt::Debug for TallyAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TallyAccessor")
            .finish_non_exhaustive()
    }
}

/// ND-filter controls for a profile which declares ND support.
#[must_use]
pub struct NdFilterAccessor<'view, 'session, P: CompileTimeProfile + HasNdFilter> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile + HasNdFilter> fmt::Debug for NdFilterAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NdFilterAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion-sync controls for a profile which declares motion-sync support.
#[must_use]
pub struct MotionSyncAccessor<'view, 'session, P: CompileTimeProfile + HasMotionSync> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile + HasMotionSync> fmt::Debug for MotionSyncAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MotionSyncAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion safety and observation methods kept separate from the pan/tilt noun.
#[must_use]
pub struct MotionAccessor<'view, 'session, P: CompileTimeProfile> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile> fmt::Debug for MotionAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MotionAccessor")
            .finish_non_exhaustive()
    }
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns the separate motion safety and observation view.
    pub fn motion(&self) -> MotionAccessor<'_, 'session, P> {
        MotionAccessor { camera: self }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod inventory_tests {
    use std::collections::BTreeMap;

    use crate::command::semantics::BuiltinCommand;
    use crate::command::surface::{surface_entry, StaticSurfaceDisposition};

    #[test]
    fn every_ledger_method_has_one_blocking_definition_per_row() {
        let source = include_str!("blocking_nouns.rs");
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
                "missing canonical blocking noun accessor {accessor}",
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
            let needle = format!("pub fn {method}(");
            assert_eq!(
                source.matches(&needle).count(),
                rows,
                "blocking noun method {method} must represent exactly its ledger rows",
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
            let declaration = format!("pub fn {forbidden}(");
            assert!(!source.contains(&declaration));
        }
    }
}

impl<'view, 'session, P: CompileTimeProfile + HasMotionSync>
    MotionSyncAccessor<'view, 'session, P>
{
    fn new(camera: &'view Camera<'session, P>) -> Self {
        Self { camera }
    }

    /// Returns the motion-sync mode.
    pub fn mode(&self) -> Result<command::MotionSyncMode> {
        inquire(self.camera, &command::MotionSyncModeInquiry)
    }

    /// Returns the motion-sync preset speed.
    pub fn preset(&self) -> Result<command::MotionSyncPreset> {
        inquire(self.camera, &command::MotionSyncPresetInquiry)
    }

    /// Enables or disables motion synchronization.
    pub fn set_mode(&self, mode: command::MotionSyncMode) -> Result<()> {
        execute(self.camera, &command::SetMotionSyncMode::new(mode))
    }

    /// Sets the motion-sync speed preset.
    pub fn set_preset(&self, speed: u8) -> Result<()> {
        execute(self.camera, &command::SetMotionSyncPreset::new(speed)?)
    }
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns motion-sync controls for a profile that declares support.
    pub fn motion_sync(&self) -> MotionSyncAccessor<'_, 'session, P>
    where
        P: HasMotionSync,
    {
        MotionSyncAccessor::new(self)
    }
}

impl<'view, 'session, P: CompileTimeProfile> MotionAccessor<'view, 'session, P> {
    /// Stops pan/tilt, zoom, and focus through the camera's single owner.
    pub fn stop_all_motion(&self) -> Result<()> {
        self.camera.core().stop_all_motion()
    }

    /// Reports whether the selected physical axes are moving.
    pub fn is_moving(&self, query: MotionQuery) -> Result<bool> {
        self.camera.core().is_moving(query)
    }

    /// Waits for the selected physical axes to become idle.
    pub fn wait_until_idle(&self, wait: IdleWait) -> Result<()> {
        self.camera.core().wait_until_idle(wait)
    }
}

fn execute<'session, P, C>(camera: &Camera<'session, P>, command: &C) -> Result<()>
where
    P: CompileTimeProfile,
    C: PlainCommand + ?Sized,
{
    camera.core().execute(command)
}

fn inquire<'session, P, Q>(camera: &Camera<'session, P>, inquiry: &Q) -> Result<Q::Response>
where
    P: CompileTimeProfile,
    Q: Inquiry + ?Sized,
{
    camera.core().inquire(inquiry)
}

fn submit<'session, P, K, O>(
    camera: &Camera<'session, P>,
    operation: &O,
) -> Result<Operation<'session, K>>
where
    P: CompileTimeProfile,
    K: completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    camera.core().submit(operation)
}

impl<'view, 'session, P: CompileTimeProfile> PowerAccessor<'view, 'session, P> {
    /// Returns the current power state.
    pub fn state(&self) -> Result<bool> {
        inquire(self.camera, &command::PowerInquiry)
    }

    /// Powers the camera on.
    pub fn on(&self) -> Result<()> {
        execute(self.camera, &command::PowerOn::new())
    }

    /// Places the camera in standby.
    pub fn off(&self) -> Result<()> {
        execute(self.camera, &command::PowerStandby::new())
    }
}

impl<'view, 'session, P: CompileTimeProfile> ZoomAccessor<'view, 'session, P> {
    /// Returns the current zoom position.
    pub fn position(&self) -> Result<types::ZoomPosition> {
        inquire(self.camera, &command::ZoomPositionInquiry)
    }

    /// Drives toward telephoto at standard speed.
    pub fn tele(&self) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::ZoomDrive::Tele)
    }

    /// Drives toward wide angle at standard speed.
    pub fn wide(&self) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::ZoomDrive::Wide)
    }

    /// Stops zoom movement.
    pub fn stop(&self) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::ZoomStop)
    }

    /// Drives toward telephoto at a variable speed.
    pub fn tele_variable(
        &self,
        speed: types::ZoomSpeed,
    ) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::ZoomDrive::TeleVariable(speed))
    }

    /// Drives toward wide angle at a variable speed.
    pub fn wide_variable(
        &self,
        speed: types::ZoomSpeed,
    ) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::ZoomDrive::WideVariable(speed))
    }

    /// Moves to an absolute zoom position.
    pub fn set_position(
        &self,
        position: types::ZoomPosition,
    ) -> Result<Operation<'session, Targeted>>
    where
        P: HasDirectZoom,
    {
        submit(self.camera, &builtin::ZoomTarget::new(position))
    }

    /// Enables or disables digital zoom.
    pub fn set_digital_zoom(&self, enabled: bool) -> Result<()>
    where
        P: HasDigitalZoomToggle,
    {
        execute(self.camera, &command::DigitalZoom::new(enabled))
    }
}

impl<'view, 'session, P: CompileTimeProfile> SystemAccessor<'view, 'session, P> {
    /// Returns firmware/version information.
    pub fn version(&self) -> Result<command::VersionInfo> {
        inquire(self.camera, &command::VersionInquiry)
    }

    /// Saves camera settings to non-volatile storage.
    pub fn save_settings(&self) -> Result<()> {
        execute(self.camera, &command::SettingsSaveCommand::new())
    }
}

impl<'view, 'session, P: CompileTimeProfile> PanTiltAccessor<'view, 'session, P> {
    /// Returns the current pan/tilt position.
    pub fn position(&self) -> Result<PanTiltPosition> {
        inquire(self.camera, &command::PanTiltPositionInquiry)
    }

    /// Moves to the home position.
    pub fn home(&self) -> Result<Operation<'session, Targeted>> {
        submit(self.camera, &builtin::PanTiltHome)
    }

    /// Resets the pan/tilt mechanism.
    pub fn reset(&self) -> Result<Operation<'session, Targeted>> {
        submit(self.camera, &builtin::PanTiltReset)
    }

    /// Starts a directional pan/tilt drive.
    pub fn move_direction(
        &self,
        direction: command::PanTiltDirection,
        pan_speed: types::PanSpeed,
        tilt_speed: types::TiltSpeed,
    ) -> Result<Operation<'session, AppliedOnly>> {
        let command = builtin::PanTiltDrive::new(direction, pan_speed, tilt_speed)?;
        submit(self.camera, &command)
    }

    /// Stops pan/tilt movement with profile-safe stop speeds.
    pub fn stop(&self) -> Result<Operation<'session, AppliedOnly>> {
        let command = self.camera.core().pan_tilt_stop_request()?;
        submit(self.camera, &command)
    }

    /// Moves to an absolute degree position.
    pub fn absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> Result<Operation<'session, Targeted>> {
        let command = builtin::PanTiltAbsolute::for_profile(
            pan,
            tilt,
            types::PanSpeed::from(speed),
            types::TiltSpeed::from(speed),
            self.camera.profile(),
        )?;
        submit(self.camera, &command)
    }

    /// Moves by a relative degree offset.
    pub fn relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: types::SpeedLevel,
    ) -> Result<Operation<'session, Targeted>> {
        let command = builtin::PanTiltRelative::for_profile(
            pan,
            tilt,
            types::PanSpeed::from(speed),
            types::TiltSpeed::from(speed),
            self.camera.profile(),
        )?;
        submit(self.camera, &command)
    }

    /// Sets one movement-limit corner.
    pub fn limit_set(
        &self,
        corner: command::PanTiltLimitCorner,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
    ) -> Result<()> {
        let command =
            builtin::PanTiltLimitSet::for_profile(corner, pan, tilt, self.camera.profile())?;
        execute(self.camera, &command)
    }

    /// Clears one movement-limit corner.
    pub fn limit_clear(&self, corner: command::PanTiltLimitCorner) -> Result<()> {
        execute(self.camera, &builtin::PanTiltLimitClear::new(corner))
    }
}

impl<'view, 'session, P: CompileTimeProfile> FocusAccessor<'view, 'session, P> {
    /// Returns the current focus position.
    pub fn position(&self) -> Result<types::FocusPosition> {
        inquire(self.camera, &command::FocusPositionInquiry)
    }

    /// Returns the current focus mode.
    pub fn mode(&self) -> Result<command::FocusMode> {
        inquire(self.camera, &command::FocusModeInquiry)
    }

    /// Drives focus farther at standard speed.
    pub fn far(&self) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::FocusDrive::Far)
    }

    /// Drives focus nearer at standard speed.
    pub fn near(&self) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::FocusDrive::Near)
    }

    /// Drives focus farther at a variable speed.
    pub fn far_variable(
        &self,
        speed: command::FocusSpeed,
    ) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::FocusDrive::FarVariable(speed))
    }

    /// Drives focus nearer at a variable speed.
    pub fn near_variable(
        &self,
        speed: command::FocusSpeed,
    ) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::FocusDrive::NearVariable(speed))
    }

    /// Stops focus movement.
    pub fn stop(&self) -> Result<Operation<'session, AppliedOnly>> {
        submit(self.camera, &builtin::FocusStop)
    }

    /// Moves focus to an absolute position.
    pub fn set_position(
        &self,
        position: types::FocusPosition,
    ) -> Result<Operation<'session, Targeted>> {
        submit(self.camera, &builtin::FocusTarget::new(position))
    }

    /// Enables automatic focus mode.
    pub fn auto(&self) -> Result<()> {
        execute(self.camera, &builtin::FocusModeCommand::Auto)
    }

    /// Enables manual focus mode.
    pub fn manual(&self) -> Result<()> {
        execute(self.camera, &builtin::FocusModeCommand::Manual)
    }

    /// Triggers one-push autofocus.
    pub fn one_push(&self) -> Result<Operation<'session, AppliedOnly>>
    where
        P: HasOnePushFocus,
    {
        submit(self.camera, &builtin::FocusTrigger::OnePush)
    }

    /// Moves focus to infinity.
    pub fn infinity(&self) -> Result<Operation<'session, Targeted>> {
        submit(self.camera, &builtin::FocusInfinity)
    }

    /// Toggles automatic/manual focus mode.
    pub fn toggle(&self) -> Result<()> {
        execute(self.camera, &builtin::FocusModeCommand::Toggle)
    }

    /// Triggers vendor snap focus.
    pub fn snap(&self) -> Result<Operation<'session, AppliedOnly>>
    where
        P: crate::capabilities::HasPtzOpticsSnapFocus,
    {
        submit(self.camera, &builtin::FocusTrigger::Snap)
    }

    /// Selects a focus zone.
    pub fn set_zone(&self, zone: command::FocusZone) -> Result<()>
    where
        P: HasFocusZone,
    {
        execute(self.camera, &command::FocusZoneCommand::new(zone))
    }

    /// Sets autofocus sensitivity.
    pub fn set_sensitivity(&self, sensitivity: command::AutoFocusSensitivity) -> Result<()>
    where
        P: HasAutoFocusSensitivity,
    {
        execute(
            self.camera,
            &command::AutoFocusSensitivityCommand::new(sensitivity),
        )
    }

    /// Sets the minimum focus distance.
    pub fn set_near_limit(&self, position: types::FocusPosition) -> Result<()>
    where
        P: HasFocusNearLimitInquiry,
    {
        execute(self.camera, &command::FocusNearLimitCommand::new(position))
    }

    /// Sets the focus-lock mode.
    pub fn set_lock(&self, mode: command::FocusLock) -> Result<()>
    where
        P: HasFocusLock,
    {
        execute(self.camera, &mode)
    }

    /// Presses vendor Push-AF.
    pub fn push_af_press(&self) -> Result<Operation<'session, AppliedOnly>>
    where
        P: HasPushAutoFocus,
    {
        submit(self.camera, &builtin::PushAfPress::new())
    }

    /// Releases vendor Push-AF.
    pub fn push_af_release(&self) -> Result<Operation<'session, AppliedOnly>>
    where
        P: HasPushAutoFocus,
    {
        submit(self.camera, &builtin::PushAfRelease::new())
    }

    /// Returns the configured focus near limit.
    pub fn near_limit(&self) -> Result<types::FocusPosition>
    where
        P: HasFocusNearLimitInquiry,
    {
        inquire(self.camera, &command::FocusNearLimitInquiry)
    }

    /// Returns the configured focus zone.
    pub fn zone(&self) -> Result<command::FocusZone>
    where
        P: HasFocusZone,
    {
        inquire(self.camera, &command::FocusZoneInquiry)
    }

    /// Returns autofocus sensitivity.
    pub fn sensitivity(&self) -> Result<command::AutoFocusSensitivity>
    where
        P: HasAutoFocusSensitivity,
    {
        inquire(self.camera, &command::AutoFocusSensitivityInquiry)
    }

    /// Returns the configured focus range.
    pub fn range(&self) -> Result<command::FocusRange> {
        inquire(self.camera, &command::FocusRangeInquiry)
    }
}

impl<'view, 'session, P: CompileTimeProfile> PresetsAccessor<'view, 'session, P> {
    /// Recalls a stored preset.
    pub fn recall(&self, preset: command::PresetNumber) -> Result<Operation<'session, Targeted>> {
        let command = builtin::PresetRecall::for_profile(preset, self.camera.profile())?;
        submit(self.camera, &command)
    }

    /// Sets the preset-recall speed.
    pub fn set_recall_speed(&self, speed: command::PresetRecallSpeed) -> Result<()> {
        execute(self.camera, &command::PresetRecallSpeedCommand::new(speed))
    }

    /// Stores the current camera state in a preset.
    pub fn set(&self, preset: command::PresetNumber) -> Result<()> {
        execute(self.camera, &builtin::PresetSet::new(preset))
    }

    /// Clears a stored preset.
    pub fn reset(&self, preset: command::PresetNumber) -> Result<()> {
        execute(self.camera, &builtin::PresetReset::new(preset))
    }
}

impl<'view, 'session, P: CompileTimeProfile> ExposureAccessor<'view, 'session, P> {
    /// Returns the active exposure mode.
    pub fn mode(&self) -> Result<command::ExposureMode> {
        inquire(self.camera, &command::ExposureModeInquiry)
    }

    /// Sets the exposure mode.
    pub fn set_mode(&self, mode: command::ExposureMode) -> Result<()> {
        execute(self.camera, &command::ExposureCommand::new(mode))
    }

    /// Returns the shutter speed.
    pub fn shutter(&self) -> Result<types::ShutterSpeed> {
        inquire(self.camera, &command::ShutterInquiry)
    }

    /// Restores the shutter default.
    pub fn shutter_reset(&self) -> Result<()> {
        execute(self.camera, &command::Shutter::Reset)
    }

    /// Increases shutter speed by one camera-defined step.
    pub fn shutter_up(&self) -> Result<()> {
        execute(self.camera, &command::Shutter::Up)
    }

    /// Decreases shutter speed by one camera-defined step.
    pub fn shutter_down(&self) -> Result<()> {
        execute(self.camera, &command::Shutter::Down)
    }

    /// Sets an explicit shutter speed.
    pub fn shutter_direct(&self, speed: types::ShutterSpeed) -> Result<()> {
        execute(self.camera, &command::Shutter::SetSpeed(speed))
    }

    /// Returns exposure compensation.
    pub fn compensation(&self) -> Result<types::ExposureCompensationLevel>
    where
        P: HasExposureCompensation,
    {
        inquire(self.camera, &command::ExposureCompensationInquiry)
    }

    /// Returns whether exposure compensation is enabled.
    pub fn compensation_enabled(&self) -> Result<bool>
    where
        P: HasExposureCompensation,
    {
        inquire(self.camera, &command::ExposureCompensationModeInquiry)
    }

    /// Returns the exposure compensation position.
    pub fn compensation_position(&self) -> Result<types::ExposureCompensationPosition>
    where
        P: HasExposureCompensation,
    {
        inquire(self.camera, &command::ExposureCompensationPositionInquiry)
    }

    /// Enables exposure compensation.
    pub fn compensation_on(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        execute(self.camera, &command::ExposureCompensation::On)
    }

    /// Disables exposure compensation.
    pub fn compensation_off(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        execute(self.camera, &command::ExposureCompensation::Off)
    }

    /// Resets exposure compensation.
    pub fn compensation_reset(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        execute(self.camera, &command::ExposureCompensation::Reset)
    }

    /// Increases exposure compensation by one step.
    pub fn compensation_up(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        execute(self.camera, &command::ExposureCompensation::Up)
    }

    /// Decreases exposure compensation by one step.
    pub fn compensation_down(&self) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        execute(self.camera, &command::ExposureCompensation::Down)
    }

    /// Sets direct exposure compensation.
    pub fn compensation_direct(&self, level: types::ExposureCompensationLevel) -> Result<()>
    where
        P: HasExposureCompensation,
    {
        execute(self.camera, &command::ExposureCompensation::SetLevel(level))
    }

    /// Returns the wide-dynamic-range level.
    pub fn dynamic_range(&self) -> Result<types::DynamicRangeLevel>
    where
        P: HasWideDynamicRange,
    {
        inquire(self.camera, &command::DynamicRangeInquiry)
    }

    /// Sets the wide-dynamic-range level.
    pub fn set_dynamic_range(&self, level: types::DynamicRangeLevel) -> Result<()>
    where
        P: HasWideDynamicRange,
    {
        execute(self.camera, &command::DynamicRange::new(level))
    }

    /// Returns whether iris control is automatic.
    pub fn iris_control(&self) -> Result<bool>
    where
        P: HasIrisControl,
    {
        inquire(self.camera, &command::IrisControlInquiry)
    }

    /// Returns the iris level.
    pub fn iris(&self) -> Result<types::IrisLevel>
    where
        P: HasIrisControl,
    {
        inquire(self.camera, &command::IrisInquiry)
    }

    /// Resets iris.
    pub fn iris_reset(&self) -> Result<Operation<'session, Targeted>>
    where
        P: HasIrisControl,
    {
        submit(self.camera, &builtin::IrisReset::new())
    }

    /// Increases iris by one step.
    pub fn iris_up(&self) -> Result<Operation<'session, Targeted>>
    where
        P: HasIrisControl,
    {
        submit(self.camera, &builtin::IrisUp::new())
    }

    /// Decreases iris by one step.
    pub fn iris_down(&self) -> Result<Operation<'session, Targeted>>
    where
        P: HasIrisControl,
    {
        submit(self.camera, &builtin::IrisDown::new())
    }

    /// Sets a direct iris level.
    pub fn iris_direct(&self, level: types::IrisLevel) -> Result<Operation<'session, Targeted>>
    where
        P: HasIrisControl,
    {
        submit(self.camera, &builtin::IrisDirect::new(level))
    }

    /// Returns exposure brightness.
    pub fn brightness(&self) -> Result<types::BrightnessLevel>
    where
        P: HasBrightnessControl,
    {
        inquire(self.camera, &command::BrightnessInquiry)
    }

    /// Resets brightness.
    pub fn brightness_reset(&self) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        execute(self.camera, &command::Brightness::Reset)
    }

    /// Increases brightness.
    pub fn brightness_up(&self) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        execute(self.camera, &command::Brightness::Up)
    }

    /// Decreases brightness.
    pub fn brightness_down(&self) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        execute(self.camera, &command::Brightness::Down)
    }

    /// Sets the bright-direct value.
    pub fn brightness_set(&self, level: types::BrightnessLevel) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        execute(self.camera, &command::Brightness::SetLevel(level))
    }

    /// Sets the direct brightness value.
    pub fn brightness_direct(&self, level: types::BrightnessLevel) -> Result<()>
    where
        P: HasBrightnessControl,
    {
        execute(self.camera, &command::Brightness::Direct(level))
    }

    /// Returns gain.
    pub fn gain(&self) -> Result<types::GainLevel> {
        inquire(self.camera, &command::GainInquiry)
    }

    /// Resets gain.
    pub fn gain_reset(&self) -> Result<()> {
        execute(self.camera, &command::Gain::Reset)
    }

    /// Increases gain.
    pub fn gain_up(&self) -> Result<()> {
        execute(self.camera, &command::Gain::Up)
    }

    /// Decreases gain.
    pub fn gain_down(&self) -> Result<()> {
        execute(self.camera, &command::Gain::Down)
    }

    /// Sets direct gain.
    pub fn gain_direct(&self, level: types::GainLevel) -> Result<()> {
        execute(self.camera, &command::Gain::SetValue(level))
    }

    /// Returns the configured gain limit.
    pub fn gain_limit(&self) -> Result<types::GainLimit> {
        inquire(self.camera, &command::GainLimitInquiry)
    }

    /// Sets the configured gain limit.
    pub fn set_gain_limit(&self, limit: types::GainLimit) -> Result<()> {
        execute(self.camera, &command::GainLimitCommand::new(limit))
    }

    /// Sets anti-flicker mode.
    pub fn set_anti_flicker(&self, mode: command::AntiFlickerMode) -> Result<()> {
        execute(self.camera, &command::AntiFlickerCommand::new(mode))
    }

    /// Returns anti-flicker mode.
    pub fn flicker_mode(&self) -> Result<command::AntiFlickerMode> {
        inquire(self.camera, &command::FlickerModeInquiry)
    }

    /// Enables spotlight mode.
    pub fn spotlight_on(&self) -> Result<()> {
        execute(self.camera, &command::SpotlightOn::new())
    }

    /// Disables spotlight mode.
    pub fn spotlight_off(&self) -> Result<()> {
        execute(self.camera, &command::SpotlightOff::new())
    }

    /// Enables automatic slow shutter.
    pub fn auto_slow_shutter_on(&self) -> Result<()> {
        execute(self.camera, &command::AutoSlowShutterOn::new())
    }

    /// Disables automatic slow shutter.
    pub fn auto_slow_shutter_off(&self) -> Result<()> {
        execute(self.camera, &command::AutoSlowShutterOff::new())
    }
}

impl<'view, 'session, P: CompileTimeProfile> WhiteBalanceAccessor<'view, 'session, P> {
    /// Returns the active white-balance mode.
    pub fn mode(&self) -> Result<command::WhiteBalanceMode> {
        inquire(self.camera, &command::WhiteBalanceModeInquiry)
    }

    /// Selects automatic white balance.
    pub fn auto(&self) -> Result<()> {
        execute(
            self.camera,
            &command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Auto),
        )
    }

    /// Selects the indoor white-balance preset.
    pub fn indoor(&self) -> Result<()> {
        execute(
            self.camera,
            &command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Indoor),
        )
    }

    /// Selects the outdoor white-balance preset.
    pub fn outdoor(&self) -> Result<()> {
        execute(
            self.camera,
            &command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Outdoor),
        )
    }

    /// Selects one-push white balance.
    pub fn one_push(&self) -> Result<()>
    where
        P: HasOnePushWhiteBalance,
    {
        execute(
            self.camera,
            &command::WhiteBalanceCommand::new(command::WhiteBalanceMode::OnePush),
        )
    }

    /// Selects auto-tracking white balance.
    pub fn atw(&self) -> Result<()>
    where
        P: HasAutoTrackingWhiteBalance,
    {
        execute(
            self.camera,
            &command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ATW),
        )
    }

    /// Selects manual white balance.
    pub fn manual(&self) -> Result<()> {
        execute(
            self.camera,
            &command::WhiteBalanceCommand::new(command::WhiteBalanceMode::Manual),
        )
    }

    /// Selects color-temperature white-balance mode.
    pub fn color_temperature_mode(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        execute(
            self.camera,
            &command::WhiteBalanceCommand::new(command::WhiteBalanceMode::ColorTemperature),
        )
    }

    /// Sets automatic white-balance sensitivity.
    pub fn set_sensitivity(&self, sensitivity: command::AutoWhiteBalanceSensitivity) -> Result<()>
    where
        P: HasAutoWhiteBalanceSensitivity,
    {
        execute(
            self.camera,
            &command::AWBSensitivityCommand::new(sensitivity),
        )
    }

    /// Returns automatic white-balance sensitivity.
    pub fn sensitivity(&self) -> Result<command::AutoWhiteBalanceSensitivity>
    where
        P: HasAutoWhiteBalanceSensitivity,
    {
        inquire(self.camera, &command::AutoWhiteBalanceSensitivityInquiry)
    }

    /// Triggers one-push white-balance calibration.
    pub fn one_push_trigger(&self) -> Result<()>
    where
        P: HasOnePushWhiteBalance,
    {
        execute(self.camera, &command::OnePushTriggerCommand::new())
    }

    /// Sets red-channel white-balance tuning.
    pub fn set_red_tuning(&self, level: types::RedTuning) -> Result<()>
    where
        P: HasRgbTuning,
    {
        execute(self.camera, &command::RedTuningCommand::new(level))
    }

    /// Sets blue-channel white-balance tuning.
    pub fn set_blue_tuning(&self, level: types::BlueTuning) -> Result<()>
    where
        P: HasRgbTuning,
    {
        execute(self.camera, &command::BlueTuningCommand::new(level))
    }

    /// Returns the color temperature.
    pub fn color_temperature(&self) -> Result<types::ColorTemp>
    where
        P: HasColorTemperature,
    {
        inquire(self.camera, &command::ColorTemperatureInquiry)
    }

    /// Resets color temperature.
    pub fn reset_color_temperature(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        execute(self.camera, &command::ColorTemperature::Reset)
    }

    /// Increases color temperature.
    pub fn increase_color_temperature(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        execute(self.camera, &command::ColorTemperature::Up)
    }

    /// Decreases color temperature.
    pub fn decrease_color_temperature(&self) -> Result<()>
    where
        P: HasColorTemperature,
    {
        execute(self.camera, &command::ColorTemperature::Down)
    }

    /// Sets a direct color-temperature value.
    pub fn set_color_temperature(&self, temperature: types::ColorTemp) -> Result<()>
    where
        P: HasColorTemperature,
    {
        execute(
            self.camera,
            &command::ColorTemperature::SetTemperature(temperature),
        )
    }

    /// Returns red-channel gain.
    pub fn red_gain(&self) -> Result<types::RedChannel>
    where
        P: HasRgbGain,
    {
        inquire(self.camera, &command::RedGainInquiry)
    }

    /// Resets red-channel gain.
    pub fn reset_red_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::RedGain::Reset)
    }

    /// Increases red-channel gain.
    pub fn increase_red_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::RedGain::Up)
    }

    /// Decreases red-channel gain.
    pub fn decrease_red_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::RedGain::Down)
    }

    /// Sets direct red-channel gain.
    pub fn set_red_gain(&self, value: types::RedChannel) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::RedGain::SetValue(value))
    }

    /// Returns blue-channel gain.
    pub fn blue_gain(&self) -> Result<types::BlueChannel>
    where
        P: HasRgbGain,
    {
        inquire(self.camera, &command::BlueGainInquiry)
    }

    /// Resets blue-channel gain.
    pub fn reset_blue_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::BlueGain::Reset)
    }

    /// Increases blue-channel gain.
    pub fn increase_blue_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::BlueGain::Up)
    }

    /// Decreases blue-channel gain.
    pub fn decrease_blue_gain(&self) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::BlueGain::Down)
    }

    /// Sets direct blue-channel gain.
    pub fn set_blue_gain(&self, value: types::BlueChannel) -> Result<()>
    where
        P: HasRgbGain,
    {
        execute(self.camera, &command::BlueGain::SetValue(value))
    }

    /// Returns red-channel tuning.
    pub fn red_tuning(&self) -> Result<types::RedTuning>
    where
        P: HasRgbTuning,
    {
        inquire(self.camera, &command::RedTuningInquiry)
    }

    /// Returns blue-channel tuning.
    pub fn blue_tuning(&self) -> Result<types::BlueTuning>
    where
        P: HasRgbTuning,
    {
        inquire(self.camera, &command::BlueTuningInquiry)
    }
}

impl<'view, 'session, P: CompileTimeProfile> ImageAccessor<'view, 'session, P> {
    /// Returns the camera resolution mode.
    pub fn resolution(&self) -> Result<command::ResolutionMode> {
        inquire(self.camera, &command::ResolutionInquiry)
    }

    /// Returns image saturation.
    pub fn saturation(&self) -> Result<types::SaturationLevel>
    where
        P: HasSaturationControl,
    {
        inquire(self.camera, &command::SaturationInquiry)
    }

    /// Sets image saturation.
    pub fn set_saturation(&self, level: types::SaturationLevel) -> Result<()>
    where
        P: HasSaturationControl,
    {
        execute(self.camera, &command::SaturationCommand::new(level))
    }

    /// Returns image hue.
    pub fn hue(&self) -> Result<types::HueLevel>
    where
        P: HasHueControl,
    {
        inquire(self.camera, &command::HueInquiry)
    }

    /// Sets image hue.
    pub fn set_hue(&self, level: types::HueLevel) -> Result<()>
    where
        P: HasHueControl,
    {
        execute(self.camera, &command::HueCommand::new(level))
    }

    /// Returns image luminance.
    pub fn luminance(&self) -> Result<types::LuminanceLevel>
    where
        P: HasLuminanceControl,
    {
        inquire(self.camera, &command::LuminanceInquiry)
    }

    /// Sets image luminance.
    pub fn set_luminance(&self, level: types::LuminanceLevel) -> Result<()>
    where
        P: HasLuminanceControl,
    {
        execute(self.camera, &command::Luminance::new(level))
    }

    /// Returns image contrast.
    pub fn contrast(&self) -> Result<types::ContrastLevel>
    where
        P: HasContrastControl,
    {
        inquire(self.camera, &command::ContrastInquiry)
    }

    /// Sets image contrast.
    pub fn set_contrast(&self, level: types::ContrastLevel) -> Result<()>
    where
        P: HasContrastControl,
    {
        execute(self.camera, &command::Contrast::new(level))
    }

    /// Returns the gamma curve.
    pub fn gamma(&self) -> Result<types::GammaLevel>
    where
        P: HasGammaControl,
    {
        inquire(self.camera, &command::GammaInquiry)
    }

    /// Sets the gamma curve.
    pub fn set_gamma(&self, level: types::GammaLevel) -> Result<()>
    where
        P: HasGammaControl,
    {
        execute(self.camera, &command::GammaCommand::new(level))
    }

    /// Returns the sharpness mode.
    pub fn sharpness_mode(&self) -> Result<command::SharpnessMode>
    where
        P: HasSharpnessControl,
    {
        inquire(self.camera, &command::SharpnessModeInquiry)
    }

    /// Returns the sharpness level.
    pub fn sharpness_level(&self) -> Result<types::SharpnessLevel>
    where
        P: HasSharpnessControl,
    {
        inquire(self.camera, &command::SharpnessPositionInquiry)
    }

    /// Sets the sharpness mode.
    pub fn set_sharpness_mode(&self, mode: command::SharpnessMode) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        execute(self.camera, &command::Sharpness::Mode(mode))
    }

    /// Resets sharpness.
    pub fn reset_sharpness(&self) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        execute(self.camera, &command::Sharpness::Reset)
    }

    /// Increases sharpness by one step.
    pub fn increase_sharpness(&self) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        execute(self.camera, &command::Sharpness::Up)
    }

    /// Decreases sharpness by one step.
    pub fn decrease_sharpness(&self) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        execute(self.camera, &command::Sharpness::Down)
    }

    /// Sets a direct sharpness level.
    pub fn set_sharpness(&self, level: types::SharpnessLevel) -> Result<()>
    where
        P: HasSharpnessControl,
    {
        execute(
            self.camera,
            &command::Sharpness::SetLevel {
                value: level.value(),
            },
        )
    }

    /// Returns backlight-compensation state.
    pub fn backlight(&self) -> Result<bool>
    where
        P: HasBacklightCompensation,
    {
        inquire(self.camera, &command::BacklightInquiry)
    }

    /// Enables or disables backlight compensation.
    pub fn set_backlight(&self, enabled: bool) -> Result<()>
    where
        P: HasBacklightCompensation,
    {
        execute(self.camera, &command::BacklightCommand::new(enabled))
    }

    /// Returns 2D noise-reduction level.
    pub fn noise_reduction_2d(&self) -> Result<types::NoiseReduction2DLevel>
    where
        P: HasNoiseReduction2D,
    {
        inquire(self.camera, &command::NoiseReduction2DInquiry)
    }

    /// Sets 2D noise-reduction level.
    pub fn set_noise_reduction_2d(&self, level: types::NoiseReduction2DLevel) -> Result<()>
    where
        P: HasNoiseReduction2D,
    {
        execute(self.camera, &command::NoiseReduction2D::with_level(level))
    }

    /// Returns 3D noise-reduction level.
    pub fn noise_reduction_3d(&self) -> Result<types::NoiseReduction3DLevel>
    where
        P: HasNoiseReduction3D,
    {
        inquire(self.camera, &command::NoiseReduction3DInquiry)
    }

    /// Sets 3D noise-reduction level.
    pub fn set_noise_reduction_3d(&self, level: types::NoiseReduction3DLevel) -> Result<()>
    where
        P: HasNoiseReduction3D,
    {
        execute(self.camera, &command::NoiseReduction3D::with_level(level))
    }

    /// Returns the aggregate noise-reduction level.
    pub fn noise_reduction_level(&self) -> Result<types::NoiseReductionLevel>
    where
        P: HasNoiseReduction,
    {
        inquire(self.camera, &command::NrLevelInquiry)
    }

    /// Returns the aggregate noise-reduction mode.
    pub fn noise_reduction_mode(&self) -> Result<command::NoiseReductionMode>
    where
        P: HasNoiseReduction,
    {
        inquire(self.camera, &command::NrModeInquiry)
    }

    /// Disables vertical image flip.
    pub fn disable_flip(&self) -> Result<()>
    where
        P: HasImageFlip,
    {
        execute(
            self.camera,
            &builtin::ImageFlipCommand::new(command::Flip::Off),
        )
    }

    /// Enables vertical image flip.
    pub fn enable_flip(&self) -> Result<()>
    where
        P: HasImageFlip,
    {
        execute(
            self.camera,
            &builtin::ImageFlipCommand::new(command::Flip::On),
        )
    }

    /// Enables horizontal image mirroring.
    pub fn enable_horizontal_flip(&self) -> Result<()>
    where
        P: HasImageMirror,
    {
        execute(self.camera, &builtin::ImageMirrorCommand::new(true))
    }

    /// Sets both image-flip axes.
    pub fn set_flip_both(&self) -> Result<()>
    where
        P: HasImageFlip,
    {
        execute(
            self.camera,
            &command::ImageFlipCombinedCommand::new(command::ImageFlipMode::Both),
        )
    }

    /// Sets the combined image-flip mode.
    pub fn set_flip_mode(&self, mode: command::ImageFlipMode) -> Result<()>
    where
        P: HasCombinedImageFlip,
    {
        execute(self.camera, &command::ImageFlipCombinedCommand::new(mode))
    }

    /// Freezes image output.
    pub fn freeze_on(&self) -> Result<()> {
        execute(self.camera, &command::ImageFreeze::on())
    }

    /// Resumes live image output.
    pub fn freeze_off(&self) -> Result<()> {
        execute(self.camera, &command::ImageFreeze::off())
    }

    /// Returns the canonical image-flip state.
    pub fn flip(&self) -> Result<command::FlipState>
    where
        P: HasImageFlip,
    {
        inquire(self.camera, &command::ImageFlipInquiry)
    }

    /// Returns the combined image-flip mode.
    pub fn flip_mode(&self) -> Result<command::FlipState>
    where
        P: HasImageFlip,
    {
        inquire(self.camera, &command::FlipStateInquiry)
    }

    /// Returns whether black-and-white mode is active.
    pub fn black_white(&self) -> Result<bool>
    where
        P: HasPictureEffect,
    {
        inquire(self.camera, &command::BlackWhiteInquiry)
    }

    /// Returns black-and-white mode.
    pub fn black_white_mode(&self) -> Result<command::BlackWhiteMode>
    where
        P: HasPictureEffect,
    {
        inquire(self.camera, &command::BlackWhiteModeInquiry)
    }

    /// Returns picture-effect mode.
    pub fn picture_effect(&self) -> Result<command::PictureEffectMode>
    where
        P: HasPictureEffect,
    {
        inquire(self.camera, &command::PictureEffectInquiry)
    }

    /// Sets picture-effect mode.
    pub fn set_picture_effect(&self, mode: command::PictureEffectMode) -> Result<()>
    where
        P: HasPictureEffect,
    {
        execute(self.camera, &command::PictureEffectCommand::new(mode))
    }

    /// Returns the camera's defog level.
    pub fn defog_level(&self) -> Result<types::DefogLevel> {
        inquire(self.camera, &command::DefogLevelInquiry)
    }
}

impl<'view, 'session, P: CompileTimeProfile> MenuAccessor<'view, 'session, P> {
    /// Returns whether the on-screen menu is open.
    pub fn status(&self) -> Result<bool> {
        inquire(self.camera, &command::MenuOpenCloseInquiry)
    }

    /// Displays or hides the on-screen menu.
    pub fn display(&self, on: bool) -> Result<()> {
        execute(self.camera, &command::SetMenuDisplay::new(on))
    }

    /// Moves the menu cursor.
    pub fn navigate(&self, direction: command::MenuDirection) -> Result<()> {
        execute(self.camera, &command::MenuNavigate::new(direction))
    }

    /// Selects the current menu item.
    pub fn select(&self) -> Result<()> {
        execute(
            self.camera,
            &command::PerformMenuAction::new(command::MenuAction::Select),
        )
    }

    /// Cancels or returns from the current menu item.
    pub fn cancel(&self) -> Result<()> {
        execute(
            self.camera,
            &command::PerformMenuAction::new(command::MenuAction::Cancel),
        )
    }

    /// Sends a direct menu control.
    pub fn direct(&self, control1: u8, control2: u8) -> Result<()>
    where
        P: HasDirectMenuControl,
    {
        execute(
            self.camera,
            &command::DirectMenuControl::new(control1, control2),
        )
    }
}

impl<'view, 'session, P: CompileTimeProfile> AdvancedAccessor<'view, 'session, P> {
    /// Returns night/day mode.
    pub fn night_day_mode(&self) -> Result<bool> {
        inquire(self.camera, &command::NightDayModeInquiry)
    }

    /// Returns standby state.
    pub fn standby_enabled(&self) -> Result<bool> {
        inquire(self.camera, &command::StandbyInquiry)
    }

    /// Returns digital-PTZ state.
    pub fn digital_ptz_enabled(&self) -> Result<bool> {
        inquire(self.camera, &command::DigitalPtzInquiry)
    }

    /// Returns auto-trace state.
    pub fn auto_trace_enabled(&self) -> Result<bool> {
        inquire(self.camera, &command::AutoTraceInquiry)
    }

    /// Returns focus-unlock state.
    pub fn focus_unlock(&self) -> Result<bool> {
        inquire(self.camera, &command::FocusUnlockInquiry)
    }

    /// Returns the broadcast domain.
    pub fn broadcast_domain(&self) -> Result<types::BroadcastDomain> {
        inquire(self.camera, &command::BroadcastDomainInquiry)
    }

    /// Returns USB-audio state.
    pub fn usb_audio_enabled(&self) -> Result<bool> {
        inquire(self.camera, &command::UsbAudioInquiry)
    }

    /// Returns two-tone mode.
    pub fn two_tone_mode_enabled(&self) -> Result<bool> {
        inquire(self.camera, &command::TwoToneModeInquiry)
    }

    /// Returns digital mode.
    pub fn digital_mode_enabled(&self) -> Result<bool> {
        inquire(self.camera, &command::DigitalInquiry)
    }

    /// Enables multicast streaming.
    pub fn multicast_on(&self) -> Result<()> {
        execute(self.camera, &command::MulticastStreaming::On)
    }

    /// Disables multicast streaming.
    pub fn multicast_off(&self) -> Result<()> {
        execute(self.camera, &command::MulticastStreaming::Off)
    }

    /// Sets NDI streaming quality.
    pub fn set_ndi_quality(&self, quality: types::NdiQuality) -> Result<()> {
        execute(self.camera, &command::SetNdiQuality::new(quality))
    }

    /// Enables USB audio.
    pub fn usb_audio_on(&self) -> Result<()> {
        execute(self.camera, &command::UsbAudio::On)
    }

    /// Disables USB audio.
    pub fn usb_audio_off(&self) -> Result<()> {
        execute(self.camera, &command::UsbAudio::Off)
    }

    /// Sets pan/tilt variable-speed mode.
    pub fn set_variable_speed_mode(&self, mode: command::VariableSpeedMode) -> Result<()>
    where
        P: HasVariableSpeed,
    {
        execute(self.camera, &command::SetVariableSpeedMode::new(mode))
    }
}

impl<'view, 'session, P: CompileTimeProfile + HasTally> TallyAccessor<'view, 'session, P> {
    fn new(camera: &'view Camera<'session, P>) -> Self {
        Self { camera }
    }

    /// Returns all tally-light state.
    pub fn status(&self) -> Result<command::TallyStatusState> {
        inquire(self.camera, &command::TallyStatusInquiry)
    }

    /// Turns the red tally on.
    pub fn red_on(&self) -> Result<()> {
        execute(self.camera, &command::TallyRedOn::new())
    }

    /// Turns the red tally off.
    pub fn red_off(&self) -> Result<()> {
        execute(self.camera, &command::TallyRedOff::new())
    }

    /// Sets low tally brightness.
    pub fn bright_lo(&self) -> Result<()> {
        execute(self.camera, &command::TallyBrightLo::new())
    }

    /// Sets high tally brightness.
    pub fn bright_hi(&self) -> Result<()> {
        execute(self.camera, &command::TallyBrightHi::new())
    }

    /// Turns the green tally on.
    pub fn green_on(&self) -> Result<()> {
        execute(self.camera, &command::TallyGreenOn::new())
    }

    /// Turns the green tally off.
    pub fn green_off(&self) -> Result<()> {
        execute(self.camera, &command::TallyGreenOff::new())
    }

    /// Sets tally flash mode.
    pub fn flash(&self) -> Result<()> {
        execute(self.camera, &command::TallyFlash::new())
    }

    /// Sets tally solid-on mode.
    pub fn on(&self) -> Result<()> {
        execute(self.camera, &command::TallyOn::new())
    }

    /// Turns tally output off.
    pub fn off(&self) -> Result<()> {
        execute(self.camera, &command::TallyOff::new())
    }

    /// Returns red tally state.
    pub fn red_status(&self) -> Result<bool> {
        inquire(self.camera, &command::TallyRedInquiry)
    }

    /// Returns green tally state.
    pub fn green_status(&self) -> Result<bool> {
        inquire(self.camera, &command::TallyGreenInquiry)
    }

    /// Returns automatic tally-adjustment state.
    pub fn auto_adjust_enabled(&self) -> Result<bool> {
        inquire(self.camera, &command::TallyAutoAdjustInquiry)
    }
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns tally controls for a profile that declares tally support.
    pub fn tally(&self) -> TallyAccessor<'_, 'session, P>
    where
        P: HasTally,
    {
        TallyAccessor::new(self)
    }
}

impl<'view, 'session, P: CompileTimeProfile + HasNdFilter> NdFilterAccessor<'view, 'session, P> {
    fn new(camera: &'view Camera<'session, P>) -> Self {
        Self { camera }
    }

    /// Returns the current ND-filter position.
    pub fn position(&self) -> Result<command::NdFilterPosition> {
        inquire(self.camera, &command::NdFilterInquiry)
    }

    /// Returns the current ND-filter preset.
    pub fn preset(&self) -> Result<types::NdFilterPreset> {
        inquire(self.camera, &command::NdFilterPresetInquiry)
    }

    /// Selects preset or variable ND-filter mode.
    pub fn set_mode(&self, mode: command::NdFilterMode) -> Result<()> {
        execute(self.camera, &command::NdFilterModeCommand::new(mode))
    }

    /// Sets a direct variable ND-filter value.
    pub fn set_value(&self, value: u16) -> Result<Operation<'session, Targeted>> {
        let value = command::NdFilterValue::new(value)?;
        submit(self.camera, &builtin::NdFilterDirect::new(value))
    }

    /// Increases ND-filter density by one step.
    pub fn step_up(&self) -> Result<Operation<'session, Targeted>> {
        submit(self.camera, &builtin::NdFilterStepUp::new())
    }

    /// Decreases ND-filter density by one step.
    pub fn step_down(&self) -> Result<Operation<'session, Targeted>> {
        submit(self.camera, &builtin::NdFilterStepDown::new())
    }

    /// Enables automatic ND filtering.
    pub fn auto_on(&self) -> Result<()> {
        execute(self.camera, &command::AutoNdCommand::new(true))
    }

    /// Disables automatic ND filtering.
    pub fn auto_off(&self) -> Result<()> {
        execute(self.camera, &command::AutoNdCommand::new(false))
    }
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns ND-filter controls for a profile that declares ND support.
    pub fn nd_filter(&self) -> NdFilterAccessor<'_, 'session, P>
    where
        P: HasNdFilter,
    {
        NdFilterAccessor::new(self)
    }
}
