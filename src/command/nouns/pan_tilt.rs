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
    preview: Option<builtin::PanTiltDrive>,
    direction: PanTiltDirection,
    speed: Speed<PanTiltSpeed>,
}
impl Drive {
    /// Captures a movement intent to resolve against the receiving camera.
    pub fn new(direction: PanTiltDirection, speed: impl Into<Speed<PanTiltSpeed>>) -> Self {
        Self {
            preview: None,
            direction,
            speed: speed.into(),
        }
    }

    /// Creates a movement intent with a preview for direct encoding.
    pub fn for_target(
        target: &TargetRegistration,
        direction: PanTiltDirection,
        speed: impl Into<Speed<PanTiltSpeed>>,
    ) -> crate::Result<Self> {
        let mut request = Self::new(direction, speed);
        request.preview = Some(request.lower(target)?);
        Ok(request)
    }

    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::PanTiltDrive> {
        #[cfg(test)]
        builtin::record_request_value_validation();
        let resolved = lower_drive_speed(target, self.speed)?;
        Ok(builtin::PanTiltDrive::new(
            self.direction,
            resolved.pan,
            resolved.tilt,
        ))
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
crate::command::nouns::movement_intent!(Drive, builtin::PanTiltDrive, AppliedOnly, PanTiltDrive);

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
    preview: Option<builtin::PanTiltAbsolute>,
    position: PanTiltPosition,
    speed: Speed<PanTiltSpeed>,
}
impl Absolute {
    /// Captures a movement intent to resolve against the receiving camera.
    pub fn new(position: PanTiltPosition, speed: impl Into<Speed<PanTiltSpeed>>) -> Self {
        Self {
            preview: None,
            position,
            speed: speed.into(),
        }
    }

    /// Creates a movement intent with a preview for direct encoding.
    pub fn for_target(
        target: &TargetRegistration,
        position: PanTiltPosition,
        speed: impl Into<Speed<PanTiltSpeed>>,
    ) -> crate::Result<Self> {
        let mut request = Self::new(position, speed);
        request.preview = Some(request.lower(target)?);
        Ok(request)
    }

    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::PanTiltAbsolute> {
        let speed = match self.speed {
            Speed::Exact(speed) => speed,
            Speed::Level(level) => target.profile().position_speeds()?.lower(level),
        };
        builtin::PanTiltAbsolute::for_target(
            self.position.pan,
            self.position.tilt,
            speed.pan,
            speed.tilt,
            target,
        )
    }

    /// Returns the requested movement position.
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
crate::command::nouns::movement_intent!(
    Absolute,
    builtin::PanTiltAbsolute,
    Targeted,
    PanTiltAbsolute
);

/// Moves by a relative offset in degrees. Keep the movement area clear.
#[derive(Debug, Clone, Copy)]
pub struct Relative {
    preview: Option<builtin::PanTiltRelative>,
    offset: PanTiltPosition,
    speed: Speed<PanTiltSpeed>,
}
impl Relative {
    /// Captures a movement intent to resolve against the receiving camera.
    pub fn new(offset: PanTiltPosition, speed: impl Into<Speed<PanTiltSpeed>>) -> Self {
        Self {
            preview: None,
            offset,
            speed: speed.into(),
        }
    }

    /// Creates a movement intent with a preview for direct encoding.
    pub fn for_target(
        target: &TargetRegistration,
        offset: PanTiltPosition,
        speed: impl Into<Speed<PanTiltSpeed>>,
    ) -> crate::Result<Self> {
        let mut request = Self::new(offset, speed);
        request.preview = Some(request.lower(target)?);
        Ok(request)
    }

    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::PanTiltRelative> {
        let speed = match self.speed {
            Speed::Exact(speed) => speed,
            Speed::Level(level) => target.profile().position_speeds()?.lower(level),
        };
        builtin::PanTiltRelative::for_target(
            self.offset.pan,
            self.offset.tilt,
            speed.pan,
            speed.tilt,
            target,
        )
    }

    /// Returns the requested movement offset.
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
crate::command::nouns::movement_intent!(
    Relative,
    builtin::PanTiltRelative,
    Targeted,
    PanTiltRelative
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

fn lower_drive_speed(
    target: &TargetRegistration,
    speed: Speed<PanTiltSpeed>,
) -> crate::Result<PanTiltSpeed> {
    use crate::types::{PanSpeed, TiltSpeed};
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
            PanTiltSpeed::new(
                PanSpeed::new(pan.clamp(facts.pan_speed.min(), facts.pan_speed.max()))?,
                TiltSpeed::new(tilt.clamp(facts.tilt_speed.min(), facts.tilt_speed.max()))?,
            )
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

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn position_intent_uses_the_receiver_even_with_a_different_wire_preview() {
        let preview_target = target(ProfileId::SonyBrc300);
        let receiving_target = TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_2,
            ProfileId::SonyEviH100.spec().expect("EVI profile"),
        );
        let origin = PanTiltPosition::from_degrees(0.0, 0.0).expect("zero degrees");
        let request = Absolute::for_target(&preview_target, origin, SpeedLevel::Fastest)
            .expect("BRC one-speed preview");
        assert_eq!(&wire(&request)[..6], &[0x81, 1, 6, 2, 0x18, 0]);
        let mut frame = [0; Absolute::MAX_SIZE];
        let len = crate::requests::write_for_target(&request, &receiving_target, &mut frame)
            .expect("EVI paired-speed frame");
        assert_eq!(
            &frame[..len],
            &[0x82, 1, 6, 2, 0x18, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0xff]
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
            if target.profile().pan_tilt_coordinates().is_some() {
                Absolute::for_target(&target, zero, SpeedLevel::Fastest).expect("coarse absolute");
                Relative::for_target(&target, zero, SpeedLevel::Fastest).expect("coarse relative");
            }
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
        assert_eq!(
            stop.admission_control_class(crate::requests::RequestContractAuthority::new()),
            crate::ControlClass::Urgent
        );
    }
}
