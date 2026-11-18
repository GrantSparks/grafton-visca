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
    direction: PanTiltDirection,
    speed: Speed<PanTiltSpeed>,
}
impl Drive {
    /// Captures a movement intent to resolve against the receiving camera.
    pub fn new(direction: PanTiltDirection, speed: impl Into<Speed<PanTiltSpeed>>) -> Self {
        Self {
            direction,
            speed: speed.into(),
        }
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

/// Stops pan/tilt movement with speed fields selected by the receiving target.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stop;

impl Stop {
    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::PanTiltStop> {
        #[cfg(test)]
        builtin::record_request_value_validation();
        crate::stop_request::pan_tilt_stop_request(target.profile())
    }
}

crate::command::nouns::movement_intent!(Stop, builtin::PanTiltStop, AppliedOnly, PanTiltStop);

/// Moves to an absolute position in degrees. Keep the movement area clear.
#[derive(Debug, Clone, Copy)]
pub struct Absolute {
    position: PanTiltPosition,
    speed: Speed<PanTiltSpeed>,
}
impl Absolute {
    /// Captures a movement intent to resolve against the receiving camera.
    pub fn new(position: PanTiltPosition, speed: impl Into<Speed<PanTiltSpeed>>) -> Self {
        Self {
            position,
            speed: speed.into(),
        }
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
    offset: PanTiltPosition,
    speed: Speed<PanTiltSpeed>,
}
impl Relative {
    /// Captures a movement intent to resolve against the receiving camera.
    pub fn new(offset: PanTiltPosition, speed: impl Into<Speed<PanTiltSpeed>>) -> Self {
        Self {
            offset,
            speed: speed.into(),
        }
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
    corner: PanTiltLimitCorner,
    position: PanTiltPosition,
}
impl SetLimit {
    /// Captures a movement limit in degrees for the receiving target.
    #[must_use]
    pub const fn new(corner: PanTiltLimitCorner, position: PanTiltPosition) -> Self {
        Self { corner, position }
    }

    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::PanTiltLimitSet> {
        builtin::PanTiltLimitSet::for_target(
            self.corner,
            self.position.pan,
            self.position.tilt,
            target,
        )
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
crate::command::nouns::movement_intent!(SetLimit, builtin::PanTiltLimitSet, Plain, PanTiltLimitSet);

/// Clears one movement-limit corner using the registered target's wire format.
#[derive(Debug, Clone, Copy)]
pub struct ClearLimit {
    corner: PanTiltLimitCorner,
}
impl ClearLimit {
    /// Captures the corner to clear on the receiving target.
    #[must_use]
    pub const fn new(corner: PanTiltLimitCorner) -> Self {
        Self { corner }
    }

    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::PanTiltLimitClear> {
        builtin::PanTiltLimitClear::for_profile(self.corner, target.profile())
    }

    /// Returns the selected movement-limit corner.
    #[must_use]
    pub const fn corner(self) -> PanTiltLimitCorner {
        self.corner
    }
}
crate::command::nouns::movement_intent!(
    ClearLimit,
    builtin::PanTiltLimitClear,
    Plain,
    PanTiltLimitClear
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
        Request,
    };

    fn target(id: ProfileId) -> TargetRegistration {
        TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            id.spec().expect("built-in profile"),
        )
    }

    fn wire(request: &impl Request, target: &TargetRegistration) -> crate::Result<Vec<u8>> {
        let mut buffer = [0; 32];
        let len = target.encode(request, &mut buffer)?;
        Ok(buffer[..len].to_vec())
    }

    #[test]
    fn coarse_position_speeds_preserve_standard_and_shared_wire_grammars() {
        let position = PanTiltPosition::from_degrees(45.0, 15.0).expect("finite position");
        let brc = target(ProfileId::SonyBrc300);
        assert_eq!(
            wire(&Absolute::new(position, SpeedLevel::Fastest), &brc).expect("BRC absolute"),
            [0x81, 1, 6, 2, 0x18, 0, 0x0f, 0x0d, 0x0b, 7, 0, 0, 0x0c, 3, 0, 0xff]
        );
        assert_eq!(
            wire(&Relative::new(position, SpeedLevel::Fastest), &brc).expect("BRC relative"),
            [0x81, 1, 6, 3, 0x18, 0, 0x0f, 0x0d, 0x0b, 7, 0, 0, 0x0c, 3, 0, 0xff]
        );
        assert_eq!(
            wire(&Drive::new(PanTiltDirection::Up, SpeedLevel::Medium), &brc)
                .expect("BRC independent drive speeds"),
            [0x81, 1, 6, 1, 12, 10, 3, 1, 0xff]
        );
        let g2 = target(ProfileId::PtzOpticsG2);
        let position = PanTiltPosition::from_degrees(10.0, -5.0).expect("finite position");
        assert_eq!(
            wire(&Absolute::new(position, SpeedLevel::Medium), &g2).expect("standard absolute"),
            [0x81, 1, 6, 2, 12, 10, 0, 0, 9, 0, 0x0f, 0x0f, 0x0b, 8, 0xff]
        );
    }

    #[test]
    fn position_intent_uses_each_receiving_targets_wire_grammar() {
        let shared_speed_target = target(ProfileId::SonyBrc300);
        let paired_speed_target = TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_2,
            ProfileId::SonyEviH100.spec().expect("EVI profile"),
        );
        let origin = PanTiltPosition::from_degrees(0.0, 0.0).expect("zero degrees");
        let request = Absolute::new(origin, SpeedLevel::Fastest);
        assert_eq!(
            &wire(&request, &shared_speed_target).expect("one-speed frame")[..6],
            &[0x81, 1, 6, 2, 0x18, 0]
        );
        assert_eq!(
            wire(&request, &paired_speed_target).expect("paired-speed frame"),
            &[0x82, 1, 6, 2, 0x18, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0xff]
        );
    }

    #[test]
    fn limit_intents_lower_against_each_receiving_mount_and_codec() {
        let desktop = target(ProfileId::SonyFr7);
        let ceiling = desktop
            .clone()
            .with_mount(crate::MountPosition::Ceiling)
            .expect("ceiling");
        let position = PanTiltPosition::from_degrees(10.0, 5.0).expect("checked coordinates");
        let set = SetLimit::new(PanTiltLimitCorner::UpRight, position);
        for registration in [&desktop, &ceiling] {
            assert_eq!(
                wire(&set, registration).expect("FR7 positioned limit"),
                [
                    0x81, 0x01, 0x06, 0x07, 0x00, 0x01, 0x0f, 0x0f, 0x06, 0x0c, 0x09, 0x00, 0x00,
                    0x04, 0x09, 0x0c, 0xff
                ]
            );
        }
        let clear = ClearLimit::new(PanTiltLimitCorner::UpRight);
        assert_eq!(
            wire(&clear, &target(ProfileId::PtzOpticsG2)).expect("G2 clear"),
            [
                0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0f, 0x0f, 0x0f, 0x07, 0x0f, 0x0f, 0x0f,
                0xff
            ]
        );
        assert_eq!(
            wire(&clear, &target(ProfileId::SonyBrc300)).expect("BRC clear"),
            [
                0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0f, 0x0f, 0x0f, 0x0f, 0x07, 0x0f, 0x0f,
                0x0f, 0xff
            ]
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
                wire(&Absolute::new(zero, SpeedLevel::Fastest), &target).expect("coarse absolute");
                wire(&Relative::new(zero, SpeedLevel::Fastest), &target).expect("coarse relative");
            }
            wire(
                &Drive::new(PanTiltDirection::Up, SpeedLevel::Fastest),
                &target,
            )
            .expect("coarse drive");
        }
        let evi = target(ProfileId::SonyEviH100);
        assert!(matches!(
            wire(
                &Absolute::new(zero, PanTiltSpeed::new(PanSpeed::MAX, TiltSpeed::MAX)),
                &evi
            ),
            Err(Error::ParameterOutOfRange { .. })
        ));
        let brc = target(ProfileId::SonyBrc300);
        let unequal = PanTiltSpeed::new(
            PanSpeed::new(12).expect("pan"),
            TiltSpeed::new(10).expect("tilt"),
        );
        assert!(wire(&Absolute::new(zero, unequal), &brc).is_err());
        wire(&Drive::new(PanTiltDirection::Up, unequal), &brc)
            .expect("drive permits independent speeds");
        assert_eq!(
            Stop.admission(crate::requests::RequestContractAuthority::new()),
            crate::request::policy::CommandAdmission::Stop
        );
    }
}
