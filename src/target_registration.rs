//! Validated target identity and installation facts.

use std::{ops::RangeInclusive, sync::Arc};

#[cfg(test)]
use crate::CameraId;
use crate::{target::TargetSlot, Error, ProfileSpec, Result};

/// Camera mounting position selected in the FR7's installation menu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(
    feature = "schemars",
    derive(schemars::JsonSchema),
    schemars(deny_unknown_fields)
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct TargetRegistration {
    #[cfg_attr(feature = "schemars", schemars(with = "crate::IndividualCameraId"))]
    #[cfg_attr(feature = "ts-rs", ts(as = "crate::IndividualCameraId"))]
    target: TargetSlot,
    profile: Arc<ProfileSpec>,
    mount: Option<MountPosition>,
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetRegistrationData {
    target: crate::IndividualCameraId,
    profile: ProfileSpec,
    mount: Option<MountPosition>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for TargetRegistration {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut fields = serializer.serialize_struct("TargetRegistration", 3)?;
        fields.serialize_field("target", &self.target())?;
        fields.serialize_field("profile", self.profile())?;
        fields.serialize_field("mount", &self.mount())?;
        fields.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TargetRegistration {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let data = <TargetRegistrationData as serde::Deserialize>::deserialize(deserializer)?;
        let registration = Self::new(data.target, data.profile);
        match data.mount {
            Some(mount) => registration
                .with_mount(mount)
                .map_err(serde::de::Error::custom),
            None => Ok(registration),
        }
    }
}

impl TargetRegistration {
    /// Registers an individual camera with its default installation.
    #[must_use]
    pub fn new(target: crate::IndividualCameraId, profile: ProfileSpec) -> Self {
        Self::from_profile(TargetSlot::from(target), profile)
    }

    pub(crate) fn from_profile(target: TargetSlot, profile: ProfileSpec) -> Self {
        let mount = profile
            .pan_tilt_geometry()
            .is_some_and(|geometry| geometry.tilt_ranges.supports_mount_selection())
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
        if !self
            .profile
            .pan_tilt_geometry()
            .is_some_and(|geometry| geometry.tilt_ranges.supports_mount_selection())
        {
            return Err(Error::FeatureNotSupported {
                feature: "mount-dependent tilt range",
            });
        }
        self.mount = Some(mount);
        Ok(self)
    }

    /// Returns the registered individual camera address.
    #[must_use]
    pub const fn target(&self) -> crate::IndividualCameraId {
        self.target.individual_camera()
    }

    /// Returns the unchanged canonical profile facts.
    #[must_use]
    pub fn profile(&self) -> &ProfileSpec {
        &self.profile
    }

    /// Writes the VISCA frame for this target into the supplied buffer.
    ///
    /// Uses the receiving profile and installation to validate and lower the
    /// request, with the same frame checks as session preparation. Returns the
    /// encoded byte count without adding a transport envelope or sending bytes.
    pub fn encode(
        &self,
        request: &impl crate::request::Request,
        buffer: &mut [u8],
    ) -> Result<usize> {
        let (lowered, ()) = crate::requests::encode_for_target(request, self, buffer, || Ok(()))?;
        Ok(lowered.written)
    }

    /// Returns the selected mount, or `None` for a fixed-range profile.
    #[must_use]
    pub const fn mount(&self) -> Option<MountPosition> {
        self.mount
    }

    /// Returns effective tilt bounds when the profile declares degree positioning.
    #[must_use]
    pub fn tilt_range(&self) -> Option<crate::capabilities::CapabilityRange<i32>> {
        self.profile
            .pan_tilt_geometry()
            .map(|geometry| geometry.tilt_ranges.range(self.mount))
    }

    /// Returns the effective nominal tilt bounds in degrees.
    #[must_use]
    pub fn tilt_range_degrees(&self) -> Option<RangeInclusive<f32>> {
        self.profile
            .pan_tilt_geometry()
            .map(|geometry| geometry.tilt_range_degrees(self.mount))
    }

    pub(crate) fn pan_tilt_coordinates(&self) -> Result<crate::profiles::TargetPanTiltCoordinates> {
        let geometry = self
            .profile
            .pan_tilt_geometry()
            .ok_or(Error::FeatureNotSupported {
                feature: "degree-based pan/tilt positioning",
            })?;
        Ok(crate::profiles::TargetPanTiltCoordinates {
            conversion: geometry.coordinates,
            pan_units: geometry.pan_range,
            tilt_units: geometry.tilt_ranges.range(self.mount),
            endpoint_quantization: geometry.endpoint_quantization,
        })
    }

    fn convert_coordinates(
        &self,
        pan: f32,
        tilt: f32,
        kind: crate::profiles::CoordinateKind,
    ) -> Result<(i32, i32)> {
        let position = self.pan_tilt_coordinates()?.checked_units(
            crate::types::PanTiltPosition::from_degrees(pan, tilt)?,
            kind,
        )?;
        Ok((position.pan, position.tilt))
    }

    pub(crate) fn convert_pan_tilt_degrees(&self, pan: f32, tilt: f32) -> Result<(i32, i32)> {
        self.convert_coordinates(pan, tilt, crate::profiles::CoordinateKind::Position)
    }

    pub(crate) fn convert_pan_tilt_offset_degrees(
        &self,
        pan: f32,
        tilt: f32,
    ) -> Result<(i32, i32)> {
        self.convert_coordinates(pan, tilt, crate::profiles::CoordinateKind::Offset)
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
        profiles::{GenericVisca, SonyFr7},
        request::builtin::PanTiltAbsolute,
        types::{PanSpeed, TiltSpeed},
        units::Degrees,
        Request,
    };

    fn absolute(target: &TargetRegistration, pan: f32, tilt: f32) -> Result<PanTiltAbsolute> {
        PanTiltAbsolute::for_target(
            Degrees::new(pan)?,
            Degrees::new(tilt)?,
            PanSpeed::new(9).expect("pan speed"),
            TiltSpeed::new(5).expect("tilt speed"),
            target,
        )
    }

    #[test]
    fn fr7_registration_selects_mount_without_changing_profile_identity() {
        let profile = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7");
        let config = crate::SessionConfig::new(profile.clone())
            .with_mount(crate::IndividualCameraId::CAMERA_1, MountPosition::Ceiling)
            .expect("ceiling registration");
        assert_eq!(
            config.profile(crate::IndividualCameraId::CAMERA_1),
            Some(&profile)
        );
        let target = config
            .registration(crate::IndividualCameraId::CAMERA_1)
            .expect("registered target");
        assert_eq!(
            target.tilt_range(),
            Some(
                crate::capabilities::CapabilityRange::<i32>::new(-49539, 3538)
                    .expect("valid capability bounds")
            )
        );
        assert_eq!(target.mount(), Some(MountPosition::Ceiling));
        assert!(absolute(target, 0.0, 15.0).is_ok());
        assert!(absolute(target, 0.0, -210.0).is_ok());
        assert!(absolute(target, 0.0, 80.0).is_err());
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic");
        assert!(
            TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, generic)
                .with_mount(MountPosition::Desktop)
                .is_err()
        );
    }

    #[test]
    fn fr7_source_endpoints_snap_only_the_half_unit_rounding_cell() {
        let target = TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7"),
        );
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
            crate::IndividualCameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7"),
        );
        let ceiling = desktop
            .clone()
            .with_mount(MountPosition::Ceiling)
            .expect("ceiling");
        let request = crate::command::pan_tilt::Absolute::new(
            crate::types::PanTiltPosition::from_degrees(0.0, -210.0).expect("ceiling target"),
            crate::types::PanTiltSpeed::new(
                PanSpeed::new(9).expect("pan"),
                TiltSpeed::new(9).expect("tilt"),
            ),
        );
        assert!(ceiling.encode(&request, &mut [0; 17]).is_ok());
        assert!(desktop.encode(&request, &mut [0; 17]).is_err());
        let relative = crate::command::pan_tilt::Relative::new(
            crate::types::PanTiltPosition::from_degrees(200.0, -35.0).expect("relative span"),
            crate::types::PanTiltSpeed::new(
                PanSpeed::new(9).expect("pan"),
                TiltSpeed::new(9).expect("tilt"),
            ),
        );
        assert!(ceiling.encode(&relative, &mut [0; 17]).is_ok());
        assert!(desktop.encode(&relative, &mut [0; 17]).is_ok());
    }
}

#[cfg(all(test, feature = "serde"))]
#[allow(clippy::expect_used)]
mod portable_tests {
    use super::*;
    use crate::profiles::ProfileId;

    fn registration(id: ProfileId) -> TargetRegistration {
        TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            id.spec().expect("built-in spec"),
        )
    }

    #[test]
    fn registration_round_trips_only_its_public_data_fields() {
        for id in ProfileId::all() {
            let registered = registration(*id);
            let data = serde_json::to_value(&registered).expect("serialize registration");
            let keys: std::collections::BTreeSet<_> = data
                .as_object()
                .expect("record")
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(
                keys,
                std::collections::BTreeSet::from(["target", "profile", "mount"])
            );
            assert_eq!(data["target"], serde_json::json!(1));
            assert_eq!(
                data["profile"],
                serde_json::to_value(registered.profile()).expect("canonical spec")
            );
            assert_eq!(
                serde_json::from_value::<TargetRegistration>(data).expect("restore registration"),
                registered
            );
        }
        let ceiling = registration(ProfileId::SonyFr7)
            .with_mount(MountPosition::Ceiling)
            .expect("ceiling");
        let data = serde_json::to_value(&ceiling).expect("serialize ceiling");
        assert_eq!(data["mount"], serde_json::json!("ceiling"));
        assert_eq!(
            serde_json::from_value::<TargetRegistration>(data).expect("restore ceiling"),
            ceiling
        );
    }

    #[test]
    fn registration_deserialization_keeps_constructor_defaults_and_checks() {
        for id in [ProfileId::SonyFr7, ProfileId::GenericVisca] {
            let expected = registration(id);
            for omit_mount in [false, true] {
                let mut data = serde_json::to_value(&expected).expect("serialize");
                if omit_mount {
                    data.as_object_mut().expect("record").remove("mount");
                } else {
                    data["mount"] = serde_json::Value::Null;
                }
                let restored = serde_json::from_value::<TargetRegistration>(data)
                    .expect("default installation");
                assert_eq!(restored.mount(), expected.mount());
            }
        }
        let valid = serde_json::to_value(registration(ProfileId::SonyFr7)).expect("serialize");
        for target in [0, 8, 9] {
            let mut invalid = valid.clone();
            invalid["target"] = serde_json::json!(target);
            assert!(serde_json::from_value::<TargetRegistration>(invalid).is_err());
        }
        let mut unknown = valid.clone();
        unknown["extra"] = serde_json::json!(true);
        assert!(serde_json::from_value::<TargetRegistration>(unknown).is_err());
        let mut bad_mount = valid.clone();
        bad_mount["mount"] = serde_json::json!("wall");
        assert!(serde_json::from_value::<TargetRegistration>(bad_mount).is_err());
        let mut bad_profile = valid;
        bad_profile["profile"]["maximum_command_sockets"] = serde_json::json!(0);
        assert!(serde_json::from_value::<TargetRegistration>(bad_profile).is_err());
    }

    #[test]
    fn explicit_mount_cannot_bypass_fixed_or_absent_pan_tilt_domains() {
        let fixed = serde_json::to_value(registration(ProfileId::GenericVisca)).expect("serialize");
        let mut absent = fixed.clone();
        absent["profile"]["capabilities"]["profile_id"] = serde_json::Value::Null;
        absent["profile"]["capabilities"]["pan_tilt"] = serde_json::Value::Null;
        absent["profile"]["position_inquiries"] = serde_json::Value::Null;
        // This synthetic custom profile uses exact completion rather than
        // position polling; its missing pan/tilt domain is otherwise valid.
        absent["profile"]["supports_operation_complete"] = serde_json::json!(true);
        absent["profile"]["capabilities"]["presets"]["recall_axes"] = serde_json::to_value(
            crate::types::AffectedAxes::ZOOM.union(crate::types::AffectedAxes::FOCUS),
        )
        .expect("present preset axes");
        serde_json::from_value::<TargetRegistration>(absent.clone())
            .expect("valid profile without pan/tilt");
        for data in [fixed, absent] {
            for mount in ["desktop", "ceiling"] {
                let mut invalid = data.clone();
                invalid["mount"] = serde_json::json!(mount);
                assert!(serde_json::from_value::<TargetRegistration>(invalid).is_err());
            }
        }
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn portable_registration_exports_public_types_and_a_closed_schema() {
        let schema = schemars::schema_for!(TargetRegistration);
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&serde_json::json!(false))
        );
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("properties");
        assert_eq!(
            properties
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["target", "profile", "mount"])
        );
        let declaration = <TargetRegistration as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(
            declaration.contains("target: IndividualCameraId"),
            "{declaration}"
        );
        assert!(
            declaration.contains("profile: ProfileSpec"),
            "{declaration}"
        );
        assert!(
            declaration.contains("mount: MountPosition | null"),
            "{declaration}"
        );
        assert!(
            !declaration.contains("TargetSlot") && !declaration.contains("TargetRegistrationData")
        );
    }
}

#[cfg(test)]
mod coordinate_tests;
