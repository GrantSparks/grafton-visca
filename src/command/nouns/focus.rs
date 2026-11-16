//! Focus movement, mode, lock and inquiry requests.
//!
//! Drive commands move the lens until completion or an explicit stop.

use crate::{
    request::builtin,
    types::{AutoFocusSensitivity, FocusMode, FocusPosition, FocusSpeed, FocusZone, Speed},
    Error, TargetRegistration,
};

/// Reads the current focus position.
#[derive(Debug, Clone, Copy, Default)]
pub struct PositionInquiry;
crate::command::nouns::forward_inquiry!(PositionInquiry, crate::command::FocusPositionInquiry; crate::command::FocusPositionInquiry);

/// Reads the active automatic or manual focus mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModeInquiry;
crate::command::nouns::forward_inquiry!(ModeInquiry, crate::command::FocusModeInquiry; crate::command::FocusModeInquiry);

/// Drives focus farther at standard speed; dropping the handle does not stop motion.
#[derive(Debug, Clone, Copy, Default)]
pub struct Far;
crate::command::nouns::forward_request!(Far, builtin::FocusDrive, AppliedOnly, rows = [FocusFar]; builtin::FocusDrive::Far);

/// Drives focus nearer at standard speed; dropping the handle does not stop motion.
#[derive(Debug, Clone, Copy, Default)]
pub struct Near;
crate::command::nouns::forward_request!(Near, builtin::FocusDrive, AppliedOnly, rows = [FocusNear]; builtin::FocusDrive::Near);

/// Stops focus movement.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stop;
crate::command::nouns::forward_request!(Stop, builtin::FocusStop, AppliedOnly, rows = [FocusStop]; builtin::FocusStop);

/// Triggers one-push autofocus.
#[derive(Debug, Clone, Copy, Default)]
pub struct OnePush;
crate::command::nouns::forward_request!(OnePush, builtin::FocusTrigger, AppliedOnly, rows = [FocusOnePush]; builtin::FocusTrigger::OnePush);

/// Moves focus to infinity.
#[derive(Debug, Clone, Copy, Default)]
pub struct Infinity;
crate::command::nouns::forward_request!(Infinity, builtin::FocusInfinity, Targeted, rows = [FocusInfinity]; builtin::FocusInfinity);

/// Toggles automatic and manual focus modes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleMode;
crate::command::nouns::forward_request!(ToggleMode, builtin::FocusModeCommand, Plain, rows = [FocusToggle]; builtin::FocusModeCommand::Toggle);

/// Triggers a supported snap-focus operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct Snap;
crate::command::nouns::forward_request!(Snap, builtin::FocusTrigger, AppliedOnly, rows = [FocusSnap]; builtin::FocusTrigger::Snap);

/// Presses the Push Auto Focus control.
#[derive(Debug, Clone, Copy, Default)]
pub struct PushAutoFocusPress;
crate::command::nouns::forward_request!(PushAutoFocusPress, builtin::PushAfPress, AppliedOnly, rows = [PushAfPress]; builtin::PushAfPress::new());

/// Releases the Push Auto Focus control.
#[derive(Debug, Clone, Copy, Default)]
pub struct PushAutoFocusRelease;
crate::command::nouns::forward_request!(PushAutoFocusRelease, builtin::PushAfRelease, AppliedOnly, rows = [PushAfRelease]; builtin::PushAfRelease::new());

/// Reads the configured minimum focus distance.
#[derive(Debug, Clone, Copy, Default)]
pub struct NearLimitInquiry;
crate::command::nouns::forward_inquiry!(NearLimitInquiry, crate::command::FocusNearLimitInquiry; crate::command::FocusNearLimitInquiry);

/// Reads the configured autofocus zone.
#[derive(Debug, Clone, Copy, Default)]
pub struct ZoneInquiry;
crate::command::nouns::forward_inquiry!(ZoneInquiry, crate::command::FocusZoneInquiry; crate::command::FocusZoneInquiry);

/// Reads the configured autofocus sensitivity.
#[derive(Debug, Clone, Copy, Default)]
pub struct SensitivityInquiry;
crate::command::nouns::forward_inquiry!(SensitivityInquiry, crate::command::AutoFocusSensitivityInquiry; crate::command::AutoFocusSensitivityInquiry);

/// Drives focus farther at a speed resolved against the target's range.
///
/// Dropping the operation handle does not stop lens motion.
#[derive(Debug, Clone, Copy)]
pub struct FarVariable {
    inner: builtin::FocusDrive,
    speed: Speed<FocusSpeed>,
}
impl FarVariable {
    /// Resolves a coarse or exact speed without sending a command.
    pub fn for_target(
        target: &TargetRegistration,
        speed: impl Into<Speed<FocusSpeed>>,
    ) -> crate::Result<Self> {
        let speed = speed.into();
        Ok(Self {
            inner: builtin::FocusDrive::FarVariable(lower_speed(target, speed)?),
            speed,
        })
    }
    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<FocusSpeed> {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    FarVariable,
    builtin::FocusDrive,
    AppliedOnly,
    rows = [FocusFarVariable]
);

/// Drives focus nearer at a speed resolved against the target's range.
///
/// Dropping the operation handle does not stop lens motion.
#[derive(Debug, Clone, Copy)]
pub struct NearVariable {
    inner: builtin::FocusDrive,
    speed: Speed<FocusSpeed>,
}
impl NearVariable {
    /// Resolves a coarse or exact speed without sending a command.
    pub fn for_target(
        target: &TargetRegistration,
        speed: impl Into<Speed<FocusSpeed>>,
    ) -> crate::Result<Self> {
        let speed = speed.into();
        Ok(Self {
            inner: builtin::FocusDrive::NearVariable(lower_speed(target, speed)?),
            speed,
        })
    }
    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<FocusSpeed> {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    NearVariable,
    builtin::FocusDrive,
    AppliedOnly,
    rows = [FocusNearVariable]
);

/// Moves focus to an absolute position.
#[derive(Debug, Clone, Copy)]
pub struct SetPosition {
    inner: builtin::FocusTarget,
    position: FocusPosition,
}
impl SetPosition {
    /// Creates an absolute target; preparation checks the profile's focus range.
    #[must_use]
    pub const fn new(position: FocusPosition) -> Self {
        Self {
            inner: builtin::FocusTarget::new(position),
            position,
        }
    }
    /// Returns the requested absolute focus position.
    #[must_use]
    pub const fn position(self) -> FocusPosition {
        self.position
    }
}
crate::command::nouns::forward_request!(
    SetPosition,
    builtin::FocusTarget,
    Targeted,
    rows = [FocusPosition]
);

/// Selects automatic or manual focus mode.
#[derive(Debug, Clone, Copy)]
pub struct SetMode {
    inner: builtin::FocusModeCommand,
    mode: FocusMode,
}
impl SetMode {
    /// Creates a focus-mode request; preparation checks target support.
    #[must_use]
    pub const fn new(mode: FocusMode) -> Self {
        Self {
            inner: match mode {
                FocusMode::Auto => builtin::FocusModeCommand::Auto,
                FocusMode::Manual => builtin::FocusModeCommand::Manual,
            },
            mode,
        }
    }
    /// Returns the requested focus mode.
    #[must_use]
    pub const fn mode(self) -> FocusMode {
        self.mode
    }
}
crate::command::nouns::forward_request!(
    SetMode,
    builtin::FocusModeCommand,
    Plain,
    rows = [FocusAuto, FocusManual]
);

/// Selects an autofocus zone.
#[derive(Debug, Clone, Copy)]
pub struct SetZone {
    inner: crate::command::focus_wire::FocusZoneCommand,
    zone: FocusZone,
}
impl SetZone {
    /// Creates the setting; preparation checks target support and allowed values.
    #[must_use]
    pub fn new(zone: FocusZone) -> Self {
        Self {
            inner: crate::command::focus_wire::FocusZoneCommand::new(zone),
            zone,
        }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn zone(self) -> FocusZone {
        self.zone
    }
}
crate::command::nouns::forward_request!(
    SetZone,
    crate::command::focus_wire::FocusZoneCommand,
    Plain,
    rows = [FocusZone]
);

/// Sets autofocus sensitivity.
#[derive(Debug, Clone, Copy)]
pub struct SetSensitivity {
    inner: crate::command::focus_wire::AutoFocusSensitivityCommand,
    sensitivity: AutoFocusSensitivity,
}
impl SetSensitivity {
    /// Creates the setting; preparation checks target support and allowed values.
    #[must_use]
    pub fn new(sensitivity: AutoFocusSensitivity) -> Self {
        Self {
            inner: crate::command::focus_wire::AutoFocusSensitivityCommand::new(sensitivity),
            sensitivity,
        }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn sensitivity(self) -> AutoFocusSensitivity {
        self.sensitivity
    }
}
crate::command::nouns::forward_request!(
    SetSensitivity,
    crate::command::focus_wire::AutoFocusSensitivityCommand,
    Plain,
    rows = [FocusAutoSensitivity]
);

/// Sets the minimum focus distance.
#[derive(Debug, Clone, Copy)]
pub struct SetNearLimit {
    inner: crate::command::focus_wire::FocusNearLimitCommand,
    position: FocusPosition,
}
impl SetNearLimit {
    /// Creates the setting; preparation checks target support and allowed values.
    #[must_use]
    pub fn new(position: FocusPosition) -> Self {
        Self {
            inner: crate::command::focus_wire::FocusNearLimitCommand::new(position),
            position,
        }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn position(self) -> FocusPosition {
        self.position
    }
}
crate::command::nouns::forward_request!(
    SetNearLimit,
    crate::command::focus_wire::FocusNearLimitCommand,
    Plain,
    rows = [FocusNearLimit]
);

/// Enables or disables focus lock.
#[derive(Debug, Clone, Copy)]
pub struct SetLock {
    inner: crate::command::focus_wire::FocusLockCommand,
    enabled: bool,
}
impl SetLock {
    /// Creates the setting; preparation checks target support and allowed values.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            inner: crate::command::focus_wire::FocusLockCommand::new(enabled),
            enabled,
        }
    }
    /// Returns the requested setting.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}
crate::command::nouns::forward_request!(
    SetLock,
    crate::command::focus_wire::FocusLockCommand,
    Plain,
    rows = [FocusLock]
);

fn lower_speed(target: &TargetRegistration, speed: Speed<FocusSpeed>) -> crate::Result<FocusSpeed> {
    let facts = target
        .profile()
        .capabilities()
        .focus
        .as_ref()
        .ok_or(Error::FeatureNotSupported { feature: "focus" })?;
    let resolved = match speed {
        Speed::Exact(speed) => speed,
        Speed::Level(level) => FocusSpeed::new(
            level
                .baseline_speeds()
                .3
                .clamp(facts.speed.min(), facts.speed.max()),
        )?,
    };
    super::pan_tilt::require_speed("focus speed", resolved.value(), facts.speed)?;
    Ok(resolved)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        capabilities::CapabilityRange,
        profiles::{ProfileId, ProfileSpec},
        types::SpeedLevel,
        CameraId, Request,
    };

    fn narrow_speed_target() -> TargetRegistration {
        let base = ProfileId::GenericVisca.spec().expect("base profile");
        let mut facts = base.capabilities().clone();
        facts.profile_id = None;
        facts.focus.as_mut().expect("focus").speed = CapabilityRange::<u8>::new(3, 5);
        let mut builder = ProfileSpec::builder(facts)
            .transports(base.transports())
            .envelope(base.envelope())
            .timing(base.timing())
            .maximum_command_sockets(base.maximum_command_sockets())
            .supports_operation_complete(base.supports_operation_complete())
            .supports_command_cancel(base.supports_command_cancel())
            .position_inquiries(base.position_inquiries());
        for encoding in base.encodings().iter() {
            builder = builder.encoding(encoding);
        }
        let profile = builder.build().expect("custom profile");
        TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile)
    }
    #[test]
    fn variable_speeds_lower_coarse_levels_and_reject_unsupported_exact_values() {
        let target = narrow_speed_target();
        let far =
            FarVariable::for_target(&target, SpeedLevel::Slowest).expect("slowest supported speed");
        let near = NearVariable::for_target(&target, SpeedLevel::Fastest)
            .expect("fastest supported speed");
        let mut buffer = [0; 6];
        assert_eq!(
            far.write_into(CameraId::CAMERA_1, &mut buffer)
                .expect("far encoding"),
            6
        );
        assert_eq!(buffer, [0x81, 1, 4, 8, 0x23, 0xff]);
        near.write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("near encoding");
        assert_eq!(buffer, [0x81, 1, 4, 8, 0x35, 0xff]);
        assert!(matches!(
            FarVariable::for_target(&target, FocusSpeed::MAX),
            Err(Error::ParameterOutOfRange { .. })
        ));
    }
}
