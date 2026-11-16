//! Pan/tilt requests, expressed in physical degrees and profile-resolved speeds.
//!
//! Movement commands operate real hardware. Keep the camera's movement area clear.

use crate::{
    request::builtin,
    types::{
        PanTiltDirection, PanTiltLimitCorner, PanTiltPosition, PanTiltSpeed, PanTiltSpeedMode,
        Speed,
    },
    Error, TargetRegistration,
};

/// Reads the raw, profile-specific pan/tilt position in VISCA units.
///
/// Use the registered profile's coordinate conversion to obtain physical degrees.
#[derive(Debug, Clone, Copy, Default)]
pub struct PositionInquiry;
crate::command::nouns::forward_inquiry!(PositionInquiry, crate::command::PanTiltPositionInquiry; crate::command::PanTiltPositionInquiry);

/// Moves the pan/tilt mechanism to its home position. Keep its movement area clear.
#[derive(Debug, Clone, Copy, Default)]
pub struct Home;
crate::command::nouns::forward_request!(Home, builtin::PanTiltHome, Targeted, rows = [PanTiltHome]; builtin::PanTiltHome);

/// Resets the pan/tilt mechanism. Keep its movement area clear.
#[derive(Debug, Clone, Copy, Default)]
pub struct Reset;
crate::command::nouns::forward_request!(Reset, builtin::PanTiltReset, Targeted, rows = [PanTiltReset]; builtin::PanTiltReset);

/// Starts a directional drive at a speed resolved against one registered target.
///
/// Keep the movement area clear; dropping the operation handle does not stop motion.
#[derive(Debug, Clone, Copy)]
pub struct Drive {
    inner: builtin::PanTiltDrive,
    direction: PanTiltDirection,
    speed: Speed<PanTiltSpeed>,
}
impl Drive {
    /// Resolves a coarse or exact speed using the target's independent axis ranges.
    pub fn for_target(
        target: &TargetRegistration,
        direction: PanTiltDirection,
        speed: impl Into<Speed<PanTiltSpeed>>,
    ) -> crate::Result<Self> {
        let speed = speed.into();
        let resolved = lower_speed(target, speed, false)?;
        Ok(Self {
            inner: builtin::PanTiltDrive::new(direction, resolved.pan, resolved.tilt)?,
            direction,
            speed,
        })
    }

    /// Returns the requested movement direction.
    #[must_use]
    pub const fn direction(self) -> PanTiltDirection {
        self.direction
    }
    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<PanTiltSpeed> {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    Drive,
    builtin::PanTiltDrive,
    AppliedOnly,
    rows = [PanTiltDrive]
);

/// Stops pan/tilt movement using speed fields accepted by the registered target.
#[derive(Debug, Clone, Copy)]
pub struct Stop {
    inner: builtin::PanTiltStop,
}
impl Stop {
    /// Selects profile-safe stop speed fields without sending a command.
    pub fn for_target(target: &TargetRegistration) -> crate::Result<Self> {
        Ok(Self {
            inner: crate::stop_request::pan_tilt_stop_request(target.profile())?,
        })
    }
}
crate::command::nouns::forward_request!(
    Stop,
    builtin::PanTiltStop,
    AppliedOnly,
    rows = [PanTiltStop]
);

/// Moves to an absolute position in degrees. Keep the movement area clear.
#[derive(Debug, Clone, Copy)]
pub struct Absolute {
    inner: builtin::PanTiltAbsolute,
    position: PanTiltPosition,
    speed: Speed<PanTiltSpeed>,
}
impl Absolute {
    /// Converts degrees and resolves speed through the target's position grammar.
    pub fn for_target(
        target: &TargetRegistration,
        position: PanTiltPosition,
        speed: impl Into<Speed<PanTiltSpeed>>,
    ) -> crate::Result<Self> {
        let speed = speed.into();
        let resolved = lower_speed(target, speed, true)?;
        Ok(Self {
            inner: builtin::PanTiltAbsolute::for_target(
                position.pan,
                position.tilt,
                resolved.pan,
                resolved.tilt,
                target,
            )?,
            position,
            speed,
        })
    }
    /// Returns the requested absolute position in degrees.
    #[must_use]
    pub const fn position(self) -> PanTiltPosition {
        self.position
    }
    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<PanTiltSpeed> {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    Absolute,
    builtin::PanTiltAbsolute,
    Targeted,
    rows = [PanTiltAbsolute]
);

/// Moves by a relative offset in degrees. Keep the movement area clear.
#[derive(Debug, Clone, Copy)]
pub struct Relative {
    inner: builtin::PanTiltRelative,
    offset: PanTiltPosition,
    speed: Speed<PanTiltSpeed>,
}
impl Relative {
    /// Converts an offset and resolves speed through the target's position grammar.
    pub fn for_target(
        target: &TargetRegistration,
        offset: PanTiltPosition,
        speed: impl Into<Speed<PanTiltSpeed>>,
    ) -> crate::Result<Self> {
        let speed = speed.into();
        let resolved = lower_speed(target, speed, true)?;
        Ok(Self {
            inner: builtin::PanTiltRelative::for_target(
                offset.pan,
                offset.tilt,
                resolved.pan,
                resolved.tilt,
                target,
            )?,
            offset,
            speed,
        })
    }
    /// Returns the requested relative offset in degrees.
    #[must_use]
    pub const fn offset(self) -> PanTiltPosition {
        self.offset
    }
    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<PanTiltSpeed> {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    Relative,
    builtin::PanTiltRelative,
    Targeted,
    rows = [PanTiltRelative]
);

/// Sets one movement-limit corner from an absolute position in degrees.
#[derive(Debug, Clone, Copy)]
pub struct SetLimit {
    inner: builtin::PanTiltLimitSet,
    corner: PanTiltLimitCorner,
    position: PanTiltPosition,
}
impl SetLimit {
    /// Converts and validates a corner using the registered target's mount and limits.
    pub fn for_target(
        target: &TargetRegistration,
        corner: PanTiltLimitCorner,
        position: PanTiltPosition,
    ) -> crate::Result<Self> {
        Ok(Self {
            inner: builtin::PanTiltLimitSet::for_target(
                corner,
                position.pan,
                position.tilt,
                target,
            )?,
            corner,
            position,
        })
    }
    /// Returns the selected movement-limit corner.
    #[must_use]
    pub const fn corner(self) -> PanTiltLimitCorner {
        self.corner
    }
    /// Returns the requested limit position in degrees.
    #[must_use]
    pub const fn position(self) -> PanTiltPosition {
        self.position
    }
}
crate::command::nouns::forward_request!(
    SetLimit,
    builtin::PanTiltLimitSet,
    Plain,
    rows = [PanTiltLimitSet]
);

/// Clears one movement-limit corner using the registered target's wire format.
#[derive(Debug, Clone, Copy)]
pub struct ClearLimit {
    inner: builtin::PanTiltLimitClear,
    corner: PanTiltLimitCorner,
}
impl ClearLimit {
    /// Selects the target's limit-clear encoding without sending a command.
    pub fn for_target(
        target: &TargetRegistration,
        corner: PanTiltLimitCorner,
    ) -> crate::Result<Self> {
        Ok(Self {
            inner: builtin::PanTiltLimitClear::for_profile(corner, target.profile())?,
            corner,
        })
    }
    /// Returns the selected movement-limit corner.
    #[must_use]
    pub const fn corner(self) -> PanTiltLimitCorner {
        self.corner
    }
}
crate::command::nouns::forward_request!(
    ClearLimit,
    builtin::PanTiltLimitClear,
    Plain,
    rows = [PanTiltLimitClear]
);

/// Selects the documented pan/tilt speed-step mode.
#[derive(Debug, Clone, Copy)]
pub struct SetSpeedMode {
    inner: crate::command::SetVariableSpeedMode,
}
impl SetSpeedMode {
    /// Creates a speed-step mode request; preparation checks target support.
    #[must_use]
    pub const fn new(mode: PanTiltSpeedMode) -> Self {
        Self {
            inner: crate::command::SetVariableSpeedMode::new(mode),
        }
    }
    /// Returns the requested speed-step mode.
    #[must_use]
    pub const fn mode(self) -> PanTiltSpeedMode {
        self.inner.mode
    }
}
crate::command::nouns::forward_request!(
    SetSpeedMode,
    crate::command::SetVariableSpeedMode,
    Plain,
    rows = [VariableSpeedMode]
);

fn lower_speed(
    target: &TargetRegistration,
    speed: Speed<PanTiltSpeed>,
    position: bool,
) -> crate::Result<PanTiltSpeed> {
    use crate::{
        capabilities::PanTiltWireCodec,
        types::{PanSpeed, TiltSpeed},
    };
    let facts =
        target
            .profile()
            .capabilities()
            .pan_tilt
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt",
            })?;
    let resolved = match speed {
        Speed::Exact(speed) => speed,
        Speed::Level(level) => {
            let (pan, tilt, _, _) = level.baseline_speeds();
            if position && facts.coordinates.wire_codec() == PanTiltWireCodec::SonyBrc300 {
                let speed = pan.clamp(
                    facts.pan_speed.min().max(facts.tilt_speed.min()),
                    facts.pan_speed.max().min(facts.tilt_speed.max()),
                );
                PanTiltSpeed::new(PanSpeed::new(speed)?, TiltSpeed::new(speed)?)
            } else {
                PanTiltSpeed::new(
                    PanSpeed::new(pan.clamp(facts.pan_speed.min(), facts.pan_speed.max()))?,
                    TiltSpeed::new(tilt.clamp(facts.tilt_speed.min(), facts.tilt_speed.max()))?,
                )
            }
        }
    };
    require_speed("pan speed", resolved.pan.value(), facts.pan_speed)?;
    require_speed("tilt speed", resolved.tilt.value(), facts.tilt_speed)?;
    Ok(resolved)
}

pub(super) fn require_speed(
    parameter: &'static str,
    value: u8,
    range: crate::capabilities::CapabilityRange<u8>,
) -> crate::Result<()> {
    if range.contains(value) {
        Ok(())
    } else {
        Err(Error::parameter_out_of_range(
            parameter,
            i32::from(value),
            i32::from(range.min()),
            i32::from(range.max()),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        profiles::ProfileId,
        types::{PanSpeed, SpeedLevel, TiltSpeed},
        CameraId, Request,
    };

    fn target(id: ProfileId) -> TargetRegistration {
        TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            id.spec().expect("built-in profile"),
        )
    }

    fn wire(request: &impl Request) -> Vec<u8> {
        let mut buffer = [0; 32];
        let len = request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("encoded request");
        buffer[..len].to_vec()
    }

    #[test]
    fn coarse_position_speeds_preserve_standard_and_shared_wire_grammars() {
        let position = PanTiltPosition::from_degrees(45.0, 15.0).expect("finite position");
        let brc = target(ProfileId::SonyBrc300);
        assert_eq!(
            wire(&Absolute::for_target(&brc, position, SpeedLevel::Fastest).expect("BRC absolute")),
            [0x81, 1, 6, 2, 0x18, 0, 0x0f, 0x0d, 0x0b, 7, 0, 0, 0x0c, 3, 0, 0xff]
        );
        assert_eq!(
            wire(&Relative::for_target(&brc, position, SpeedLevel::Fastest).expect("BRC relative")),
            [0x81, 1, 6, 3, 0x18, 0, 0x0f, 0x0d, 0x0b, 7, 0, 0, 0x0c, 3, 0, 0xff]
        );
        // BRC directional drive still has separate speed fields.
        assert_eq!(
            wire(
                &Drive::for_target(&brc, PanTiltDirection::Up, SpeedLevel::Medium)
                    .expect("BRC drive")
            ),
            [0x81, 1, 6, 1, 12, 10, 3, 1, 0xff]
        );
        let g2 = target(ProfileId::PtzOpticsG2);
        let position = PanTiltPosition::from_degrees(10.0, -5.0).expect("finite position");
        assert_eq!(
            wire(
                &Absolute::for_target(&g2, position, SpeedLevel::Medium)
                    .expect("standard absolute")
            ),
            [0x81, 1, 6, 2, 12, 10, 0, 0, 9, 0, 0x0f, 0x0f, 0x0b, 8, 0xff]
        );
    }

    #[test]
    fn exact_speeds_are_checked_and_every_profile_lowers_coarse_speeds() {
        let zero = PanTiltPosition::from_degrees(0.0, 0.0).expect("finite position");
        for id in [
            ProfileId::PtzOpticsG2,
            ProfileId::PtzOpticsG3,
            ProfileId::PtzOptics30x,
            ProfileId::SonyFr7,
            ProfileId::SonyBrcH900,
            ProfileId::SonyEviH100,
            ProfileId::SonyBrc300,
            ProfileId::NearusBrc300,
            ProfileId::GenericVisca,
        ] {
            let target = target(id);
            Absolute::for_target(&target, zero, SpeedLevel::Fastest).expect("coarse absolute");
            Relative::for_target(&target, zero, SpeedLevel::Fastest).expect("coarse relative");
            Drive::for_target(&target, PanTiltDirection::Up, SpeedLevel::Fastest)
                .expect("coarse drive");
        }
        let evi = target(ProfileId::SonyEviH100);
        assert!(matches!(
            Absolute::for_target(&evi, zero, PanTiltSpeed::new(PanSpeed::MAX, TiltSpeed::MAX)),
            Err(Error::ParameterOutOfRange { .. })
        ));
        let brc = target(ProfileId::SonyBrc300);
        let unequal = PanTiltSpeed::new(
            PanSpeed::new(12).expect("pan"),
            TiltSpeed::new(10).expect("tilt"),
        );
        assert!(Absolute::for_target(&brc, zero, unequal).is_err());
        Drive::for_target(&brc, PanTiltDirection::Up, unequal)
            .expect("drive permits independent speeds");
        let stop = Stop::for_target(&brc).expect("safe stop");
        assert_eq!(stop.control_class(), crate::ControlClass::Urgent);
        assert!(Drive::for_target(&brc, PanTiltDirection::Stop, SpeedLevel::Medium).is_err());
    }
}
