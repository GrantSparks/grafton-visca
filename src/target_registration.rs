//! Validated target identity and installation facts.

use std::{ops::RangeInclusive, sync::Arc};

use crate::{target::TargetSlot, CameraId, Error, ProfileSpec, Result};

/// Camera mounting position selected in the FR7's installation menu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum MountPosition {
    /// Upright desktop installation, the default registration.
    #[default]
    Desktop,
    /// Inverted ceiling installation.
    Ceiling,
}

/// One validated camera target, canonical profile and installation selection.
///
/// A registration keeps installation data separate from the source-backed
/// profile identity. Its range accessors derive the effective target limits.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetRegistration {
    target: TargetSlot,
    profile: Arc<ProfileSpec>,
    mount: Option<MountPosition>,
}

impl TargetRegistration {
    /// Registers an individual camera with its default installation.
    pub fn new(target: CameraId, profile: ProfileSpec) -> Result<Self> {
        Ok(Self::from_profile(TargetSlot::try_from(target)?, profile))
    }

    pub(crate) fn from_profile(target: TargetSlot, profile: ProfileSpec) -> Self {
        let mount = profile
            .supports_mount_selection()
            .then_some(MountPosition::Desktop);
        Self {
            target,
            profile: Arc::new(profile),
            mount,
        }
    }

    /// Selects the mounting position already configured on the camera.
    ///
    /// Profiles with one fixed tilt range refuse this selection.
    pub fn with_mount(mut self, mount: MountPosition) -> Result<Self> {
        if !self.profile.supports_mount_selection() {
            return Err(Error::FeatureNotSupported {
                feature: "mount-dependent tilt range",
            });
        }
        self.mount = Some(mount);
        Ok(self)
    }

    /// Returns the registered individual camera address.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.target.camera()
    }

    /// Returns the unchanged canonical profile facts.
    #[must_use]
    pub fn profile(&self) -> &ProfileSpec {
        &self.profile
    }

    /// Returns the selected mount, or `None` for a fixed-range profile.
    #[must_use]
    pub const fn mount(&self) -> Option<MountPosition> {
        self.mount
    }

    /// Returns the target's effective raw tilt bounds.
    #[must_use]
    pub fn tilt_range(&self) -> RangeInclusive<i32> {
        self.profile.tilt_range_for_mount(self.mount)
    }

    /// Returns the target's effective logical tilt bounds in degrees.
    #[must_use]
    pub fn tilt_range_degrees(&self) -> Option<RangeInclusive<f32>> {
        self.profile
            .pan_tilt_coordinates()
            .map(|conversion| conversion.tilt_degree_range(&self.tilt_range()))
    }

    pub(crate) fn convert_pan_tilt_degrees(&self, pan: f32, tilt: f32) -> Result<(i32, i32)> {
        let conversion = self.profile.pan_tilt_coordinates().ok_or_else(|| {
            Error::InvalidRequest("profile has no pan/tilt coordinate conversion".into())
        })?;
        if !self.profile.supports_mount_selection() {
            let capabilities = self.profile.capabilities();
            let position = conversion.checked_units(
                crate::inquiry_conversions::PanTiltPositionDeg::new(
                    crate::units::Degrees(pan),
                    crate::units::Degrees(tilt),
                ),
                crate::profile::PanTiltRanges {
                    pan_degrees: &capabilities.pan_range_degrees,
                    tilt_degrees: &capabilities.tilt_range_degrees,
                    pan_units: &capabilities.pan_range,
                    tilt_units: &capabilities.tilt_range,
                },
            )?;
            return Ok((position.pan, position.tilt));
        }
        let convert =
            |axis: &'static str, degrees: f32, scale: f32, range: &RangeInclusive<i32>| {
                let raw = degrees * scale;
                // R7's integer endpoints are rounded representations of nominal
                // degree limits (195*235.9=46000.5). Snap only the documented
                // half-unit endpoint cell; interior values retain nearest-away
                // rounding. No extrapolation beyond a source endpoint is allowed.
                let endpoint_cell = if self.profile.supports_mount_selection() {
                    0.5
                } else {
                    0.0
                };
                if !raw.is_finite()
                    || raw < *range.start() as f32 - endpoint_cell
                    || raw > *range.end() as f32 + endpoint_cell
                {
                    return Err(Error::InvalidParameter {
                        parameter: axis,
                        value: degrees.to_string().into(),
                        reason: format!("position is outside raw limits {range:?}").into(),
                    });
                }
                Ok((raw.round() as i32).clamp(*range.start(), *range.end()))
            };
        Ok((
            convert(
                "pan",
                pan,
                conversion.pan_degrees_to_units(),
                &self.profile.capabilities().pan_range,
            )?,
            convert(
                "tilt",
                tilt,
                conversion.tilt_degrees_to_units(),
                &self.tilt_range(),
            )?,
        ))
    }

    pub(crate) fn validate_pan_tilt_offset(&self, pan: i32, tilt: i32) -> Result<()> {
        for (parameter, value, range) in [
            (
                "pan offset",
                pan,
                self.profile.capabilities().pan_range.clone(),
            ),
            ("tilt offset", tilt, self.tilt_range()),
        ] {
            let span = i64::from(*range.end()) - i64::from(*range.start());
            if i64::from(value).abs() > span {
                return Err(Error::InvalidParameter {
                    parameter,
                    value: value.to_string().into(),
                    reason: format!("offset must lie within the relative span -{span}..={span}")
                        .into(),
                });
            }
        }
        Ok(())
    }

    pub(crate) const fn slot(&self) -> TargetSlot {
        self.target
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        profiles::{GenericVisca, SonyFR7},
        request::builtin::{PanTiltAbsolute, PanTiltRelative},
        types::{PanSpeed, TiltSpeed},
        units::Degrees,
        Request,
    };

    fn absolute(target: &TargetRegistration, pan: f32, tilt: f32) -> Result<PanTiltAbsolute> {
        PanTiltAbsolute::for_target(
            Degrees(pan),
            Degrees(tilt),
            PanSpeed::new(9).expect("pan speed"),
            TiltSpeed::new(5).expect("tilt speed"),
            target,
        )
    }

    #[test]
    fn fr7_registration_selects_mount_without_changing_profile_identity() {
        let profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7");
        let mut config = crate::SessionConfig::new(profile.clone());
        config
            .set_mount(CameraId::CAMERA_1, MountPosition::Ceiling)
            .expect("ceiling registration");
        assert_eq!(config.profile(CameraId::CAMERA_1), Some(&profile));
        let target = config
            .registration(CameraId::CAMERA_1)
            .expect("registered target");
        assert_eq!(target.tilt_range(), -49539..=3538);
        assert_eq!(target.mount(), Some(MountPosition::Ceiling));
        assert!(absolute(target, 0.0, 15.0).is_ok());
        assert!(absolute(target, 0.0, -210.0).is_ok());
        assert!(absolute(target, 0.0, 80.0).is_err());
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic");
        assert!(TargetRegistration::new(CameraId::CAMERA_1, generic)
            .expect("target")
            .with_mount(MountPosition::Desktop)
            .is_err());
    }

    #[test]
    fn fr7_source_endpoints_snap_only_the_half_unit_rounding_cell() {
        let target = TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7"),
        )
        .expect("target");
        let mut buffer = [0; 17];
        let request = absolute(&target, -170.0, 195.0).expect("left/up endpoints");
        assert_eq!(
            request
                .write_into(CameraId::CAMERA_1, &mut buffer)
                .expect("FR7 frame"),
            17
        );
        assert_eq!(&buffer[6..16], [0, 9, 12, 10, 7, 0, 11, 3, 11, 0]);
        let request = absolute(&target, 170.0, -30.0).expect("right/down endpoints");
        request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("FR7 frame");
        assert_eq!(&buffer[6..16], [15, 6, 3, 5, 9, 15, 14, 4, 5, 11]);
        assert!(absolute(&target, 0.0, 195.01).is_err());
        let ceiling = target.with_mount(MountPosition::Ceiling).expect("ceiling");
        let request = absolute(&ceiling, 0.0, 15.0).expect("ceiling endpoint");
        request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("ceiling frame");
        assert_eq!(&buffer[11..16], [0, 0, 13, 13, 2]);
        assert!(absolute(&ceiling, 0.0, 15.01).is_err());
    }

    #[test]
    fn fr7_target_validation_rejects_cross_mount_positions_before_encoding() {
        let desktop = TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7"),
        )
        .expect("desktop");
        let ceiling = desktop
            .clone()
            .with_mount(MountPosition::Ceiling)
            .expect("ceiling");
        let request = absolute(&ceiling, 0.0, -210.0).expect("ceiling target");
        assert!(request.validate_for_target(&ceiling).is_ok());
        assert!(request.validate_for_target(&desktop).is_err());
        let relative = PanTiltRelative::for_target(
            Degrees(200.0),
            Degrees(-35.0),
            PanSpeed::new(9).expect("pan"),
            TiltSpeed::new(9).expect("tilt"),
            &desktop,
        )
        .expect("relative span");
        assert!(relative.validate_for_target(&desktop).is_ok());
        assert!(relative.validate_for_target(&ceiling).is_ok());
    }
}
