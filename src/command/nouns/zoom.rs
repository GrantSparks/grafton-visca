//! Zoom requests and profile-aware normalized-position readback.
//!
//! Drive requests move the lens until stopped; dropping a handle does not stop motion.

use crate::{
    request::builtin,
    types::{HighResolutionZoomSpeed, Speed, UnitInterval, ZoomDomain, ZoomPosition, ZoomSpeed},
    Error, TargetRegistration,
};

/// Reads the current raw zoom position.
#[derive(Debug, Clone, Copy, Default)]
pub struct PositionInquiry;
crate::command::nouns::forward_inquiry!(PositionInquiry, crate::command::ZoomPositionInquiry; crate::command::ZoomPositionInquiry);

/// Drives toward telephoto at standard speed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tele;
crate::command::nouns::forward_request!(Tele, builtin::ZoomDrive, AppliedOnly, rows = [ZoomTele]; builtin::ZoomDrive::Tele);

/// Drives toward wide angle at standard speed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Wide;
crate::command::nouns::forward_request!(Wide, builtin::ZoomDrive, AppliedOnly, rows = [ZoomWide]; builtin::ZoomDrive::Wide);

/// Stops zoom movement.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stop;
crate::command::nouns::forward_request!(Stop, builtin::ZoomStop, AppliedOnly, rows = [ZoomStop]; builtin::ZoomStop);

/// Stops high-resolution zoom movement (R7 p. 16).
#[derive(Debug, Clone, Copy, Default)]
pub struct StopHighResolution;
crate::command::nouns::forward_request!(StopHighResolution, crate::command::Fr7ZoomHighResolutionStop, AppliedOnly, rows = [Fr7ZoomHighResolutionStop]; crate::command::Fr7ZoomHighResolutionStop);

/// Reads the Clear Image Zoom position (R7 p. 22).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClearImagePositionInquiry;
crate::command::nouns::forward_inquiry!(ClearImagePositionInquiry, crate::command::Fr7ClearImageZoomPositionInquiry; crate::command::Fr7ClearImageZoomPositionInquiry);

/// Drives toward telephoto at a speed resolved against the target's zoom range.
#[derive(Debug, Clone, Copy)]
pub struct TeleVariable {
    speed: Speed<ZoomSpeed>,
}
impl TeleVariable {
    /// Captures a speed to resolve against the receiving camera during preparation.
    pub fn new(speed: impl Into<Speed<ZoomSpeed>>) -> Self {
        Self {
            speed: speed.into(),
        }
    }

    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::ZoomDrive> {
        #[cfg(test)]
        builtin::record_request_value_validation();
        Ok(builtin::ZoomDrive::TeleVariable(lower_speed(
            target, self.speed,
        )?))
    }

    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<ZoomSpeed> {
        self.speed
    }
}
crate::command::nouns::movement_intent!(
    TeleVariable,
    builtin::ZoomDrive,
    AppliedOnly,
    ZoomTeleVariable
);

/// Drives toward wide angle at a speed resolved against the target's zoom range.
#[derive(Debug, Clone, Copy)]
pub struct WideVariable {
    speed: Speed<ZoomSpeed>,
}
impl WideVariable {
    /// Captures a speed to resolve against the receiving camera during preparation.
    pub fn new(speed: impl Into<Speed<ZoomSpeed>>) -> Self {
        Self {
            speed: speed.into(),
        }
    }

    fn lower(&self, target: &TargetRegistration) -> crate::Result<builtin::ZoomDrive> {
        #[cfg(test)]
        builtin::record_request_value_validation();
        Ok(builtin::ZoomDrive::WideVariable(lower_speed(
            target, self.speed,
        )?))
    }

    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<ZoomSpeed> {
        self.speed
    }
}
crate::command::nouns::movement_intent!(
    WideVariable,
    builtin::ZoomDrive,
    AppliedOnly,
    ZoomWideVariable
);

/// Moves to an absolute zoom position.
#[derive(Debug, Clone, Copy)]
pub struct SetPosition {
    inner: builtin::ZoomTarget,
    position: ZoomPosition,
}
impl SetPosition {
    /// Creates a target; preparation checks direct-zoom support and profile limits.
    #[must_use]
    pub const fn new(position: ZoomPosition) -> Self {
        Self {
            inner: builtin::ZoomTarget::new(position),
            position,
        }
    }
    /// Returns the requested absolute zoom position.
    #[must_use]
    pub const fn position(self) -> ZoomPosition {
        self.position
    }
}
crate::command::nouns::forward_request!(
    SetPosition,
    builtin::ZoomTarget,
    Targeted,
    rows = [ZoomPosition]
);

/// Moves to a normalized position across one documented zoom domain.
#[derive(Debug, Clone, Copy)]
pub struct SetNormalized {
    position: UnitInterval,
    domain: ZoomDomain,
}
impl SetNormalized {
    /// Selects `0` as wide and `1` as the selected domain's telephoto endpoint.
    ///
    /// Preparation resolves the position against the receiving camera. A
    /// combined optical/digital domain requires a documented digital maximum.
    #[must_use]
    pub const fn new(position: UnitInterval, domain: ZoomDomain) -> Self {
        Self { position, domain }
    }

    /// Returns the requested normalized position.
    #[must_use]
    pub const fn position(self) -> UnitInterval {
        self.position
    }

    /// Returns the selected normalization domain.
    #[must_use]
    pub const fn domain(self) -> ZoomDomain {
        self.domain
    }
}

/// Enables or disables digital zoom.
#[derive(Debug, Clone, Copy)]
pub struct SetDigitalZoom {
    inner: crate::command::zoom_wire::DigitalZoom,
}
impl SetDigitalZoom {
    /// Creates a digital-zoom setting; preparation checks target support.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self {
            inner: crate::command::zoom_wire::DigitalZoom::new(enabled),
        }
    }
    /// Returns whether digital zoom is requested.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.inner.enabled()
    }
}
crate::command::nouns::forward_request!(
    SetDigitalZoom,
    crate::command::zoom_wire::DigitalZoom,
    Plain,
    rows = [DigitalZoom]
);

/// Drives toward telephoto at the requested high-resolution speed (R7 p. 16).
///
/// Zero is the slowest drive speed; use [`StopHighResolution`] to stop the lens.
#[derive(Debug, Clone, Copy)]
pub struct TeleHighResolution {
    inner: crate::command::Fr7ZoomHighResolution,
    speed: HighResolutionZoomSpeed,
}
impl TeleHighResolution {
    /// Creates a checked high-resolution drive; preparation checks target support.
    #[must_use]
    pub const fn new(speed: HighResolutionZoomSpeed) -> Self {
        Self {
            inner: crate::command::Fr7ZoomHighResolution::Tele(speed),
            speed,
        }
    }
    /// Returns the requested high-resolution drive speed.
    #[must_use]
    pub const fn speed(self) -> HighResolutionZoomSpeed {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    TeleHighResolution,
    crate::command::Fr7ZoomHighResolution,
    AppliedOnly,
    rows = [Fr7ZoomHighResolutionTele]
);

/// Drives toward wide angle at the requested high-resolution speed (R7 p. 16).
///
/// Zero is the slowest drive speed; use [`StopHighResolution`] to stop the lens.
#[derive(Debug, Clone, Copy)]
pub struct WideHighResolution {
    inner: crate::command::Fr7ZoomHighResolution,
    speed: HighResolutionZoomSpeed,
}
impl WideHighResolution {
    /// Creates a checked high-resolution drive; preparation checks target support.
    #[must_use]
    pub const fn new(speed: HighResolutionZoomSpeed) -> Self {
        Self {
            inner: crate::command::Fr7ZoomHighResolution::Wide(speed),
            speed,
        }
    }
    /// Returns the requested high-resolution drive speed.
    #[must_use]
    pub const fn speed(self) -> HighResolutionZoomSpeed {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    WideHighResolution,
    crate::command::Fr7ZoomHighResolution,
    AppliedOnly,
    rows = [Fr7ZoomHighResolutionWide]
);

fn lower_speed(target: &TargetRegistration, speed: Speed<ZoomSpeed>) -> crate::Result<ZoomSpeed> {
    let facts = target
        .profile()
        .capabilities()
        .zoom
        .as_ref()
        .ok_or(Error::FeatureNotSupported { feature: "zoom" })?;
    if !facts.variable {
        return Err(Error::FeatureNotSupported {
            feature: "variable zoom drive",
        });
    }
    let resolved = match speed {
        Speed::Exact(speed) => speed,
        Speed::Level(level) => ZoomSpeed::new(
            level
                .baseline_speeds()
                .2
                .clamp(facts.speed.min(), facts.speed.max()),
        )?,
    };
    super::pan_tilt::require_speed("zoom speed", resolved.value(), facts.speed)?;
    Ok(resolved)
}

/// Reads the current zoom position normalized to one documented target domain.
///
/// The response uses the same `0` wide and `1` telephoto endpoints as [`SetNormalized`].
#[derive(Debug, Clone, Copy)]
pub struct NormalizedInquiry {
    domain: ZoomDomain,
}
impl NormalizedInquiry {
    /// Creates a query that normalizes its reply against the receiving camera.
    #[must_use]
    pub const fn new(domain: ZoomDomain) -> Self {
        Self { domain }
    }

    /// Returns the selected normalization domain.
    #[must_use]
    pub const fn domain(self) -> ZoomDomain {
        self.domain
    }
}
crate::command::nouns::forward_inquiry!(
    @request NormalizedInquiry, crate::command::ZoomPositionInquiry,
    validate = |_request, _target| {
        #[cfg(test)]
        builtin::record_request_validation();
        Ok::<(), Error>(())
    };
    crate::command::ZoomPositionInquiry
);

impl crate::Inquiry for NormalizedInquiry {
    type Response = UnitInterval;

    fn payload_shape(&self) -> crate::PayloadShape {
        crate::Inquiry::payload_shape(&crate::command::ZoomPositionInquiry)
    }
    fn payload_shape_for_profile(
        &self,
        profile: &crate::profiles::ProfileSpec,
    ) -> crate::PayloadShape {
        crate::Inquiry::payload_shape_for_profile(&crate::command::ZoomPositionInquiry, profile)
    }
    fn decoder(&self) -> crate::ResponseDecoder<Self::Response> {
        crate::ResponseDecoder::from_fn(|_| {
            Err(Error::InvalidRequest {
                reason: "normalized zoom decoding requires a target profile".into(),
            })
        })
    }
    fn decoder_for_profile(
        &self,
        profile: &crate::profiles::ProfileSpec,
    ) -> crate::Result<crate::ResponseDecoder<Self::Response>> {
        #[cfg(test)]
        builtin::record_request_value_validation();
        let capabilities = profile.capabilities();
        let zoom = capabilities
            .zoom
            .as_ref()
            .ok_or(Error::FeatureNotSupported { feature: "zoom" })?;
        if self.domain == ZoomDomain::OpticalPlusDigital
            && !capabilities.supports(crate::capabilities::TypedSupportSurface::DigitalZoomRange)
        {
            return Err(Error::FeatureNotSupported {
                feature: "optical-plus-digital zoom range",
            });
        }
        let maximum = crate::inquiry_conversions::zoom_domain_max(
            self.domain,
            zoom.optical_max,
            zoom.digital_max,
        )?;
        let range = crate::capabilities::CapabilityRange::<u16>::new(0, maximum)?;
        Ok(crate::ResponseDecoder::normalized_zoom(range))
    }
    #[allow(private_interfaces)]
    fn builtin_inquiry_syntax_retry(
        &self,
        authority: crate::requests::BuiltinInquiryAuthority,
    ) -> bool {
        crate::Inquiry::builtin_inquiry_syntax_retry(
            &crate::command::ZoomPositionInquiry,
            authority,
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        capabilities::{CapabilityRange, TypedSupportSet, TypedSupportSurface},
        profiles::{ProfileId, ProfileSpec},
        types::SpeedLevel,
        Inquiry, Request,
    };

    fn target(digital_permission: bool) -> TargetRegistration {
        target_with_optical_limit(digital_permission, 0x4000)
    }

    fn target_with_optical_limit(digital_permission: bool, optical_max: u16) -> TargetRegistration {
        let base = ProfileId::PtzOpticsG2.spec().expect("base profile");
        let mut facts = base.capabilities().clone();
        facts.profile_id = None;
        let zoom = facts.zoom.as_mut().expect("zoom");
        zoom.optical_max = optical_max;
        zoom.digital_max = Some(0x7000);
        zoom.speed = CapabilityRange::<u8>::new(2, 5).expect("valid capability bounds");
        facts.typed_support = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::PanTiltPositioning,
        ]);
        if digital_permission {
            facts.typed_support = facts.typed_support.union(TypedSupportSet::from_surface(
                TypedSupportSurface::DigitalZoomRange,
            ));
        }
        let profile = ProfileSpec::builder(facts)
            .transports(base.transports())
            .envelope(base.envelope())
            .timing(base.timing())
            .maximum_command_sockets(base.maximum_command_sockets())
            .supports_operation_complete(base.supports_operation_complete())
            .supports_command_cancel(base.supports_command_cancel())
            .position_inquiries(crate::AffectedAxes::ZOOM)
            .build()
            .expect("custom profile");
        TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile)
    }

    #[test]
    fn normalized_readback_uses_the_setters_domain_and_preserves_query_policy() {
        let target = target(true);
        let midpoint = UnitInterval::new(0.5).expect("midpoint");
        let setter = SetNormalized::new(midpoint, ZoomDomain::OpticalPlusDigital);
        let mut buffer = [0; 9];
        setter
            .validate_for_target(&target)
            .expect("documented digital domain");
        setter
            .write_for_target(
                &target,
                &mut buffer,
                crate::requests::RequestContractAuthority::new(),
            )
            .expect("target frame");
        assert_eq!(buffer, [0x81, 1, 4, 0x47, 3, 8, 0, 0, 0xff]);
        let query = NormalizedInquiry::new(ZoomDomain::OpticalPlusDigital);
        let decoder = query
            .decoder_for_profile(target.profile())
            .expect("target decoder");
        assert_eq!(
            decoder.decode(&[3, 8, 0, 0]).expect("midpoint reply"),
            midpoint
        );
        assert_eq!(
            decoder
                .clone()
                .decode(&[7, 0, 0, 0])
                .expect("maximum reply"),
            UnitInterval::MAX
        );
        let _: crate::request::InquiryPolicy = query.policy();
        assert_eq!(
            query.policy().retry_class(),
            crate::request::InquiryRetryClass::Inquiry
        );
        let optical = NormalizedInquiry::new(ZoomDomain::Optical)
            .decoder_for_profile(target.profile())
            .expect("optical decoder");
        assert!(matches!(
            optical.decode(&[7, 0, 0, 0]),
            Err(Error::InvalidResponse { .. })
        ));
        assert_eq!(
            optical.decode(&[0, 0, 0, 0]).expect("wide reply"),
            UnitInterval::MIN
        );
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn normalized_zoom_intent_prepares_against_each_receiving_profile() {
        use crate::{completion::Targeted, prepared::test_support::prepare_builtin_operation};
        let half = UnitInterval::new(0.5).expect("midpoint");
        let combined = SetNormalized::new(half, ZoomDomain::OpticalPlusDigital);
        let query = NormalizedInquiry::new(ZoomDomain::OpticalPlusDigital);
        for (profile, expected) in [
            (ProfileId::SonyBrc300, [3, 0x0f, 8, 0]),
            (ProfileId::SonyEviH100, [3, 0x0d, 6, 0]),
        ] {
            let target = TargetRegistration::new(
                crate::IndividualCameraId::CAMERA_2,
                profile.spec().expect("built-in profile"),
            );
            prepare_builtin_operation::<Targeted, _>(
                &combined,
                &target,
                crate::OperationalTuning::new(),
            )
            .expect("combined midpoint")
            .admit_with(|request, _, _, _| {
                let [a, b, c, d] = expected;
                assert_eq!(
                    request.wire().as_bytes(),
                    &[0x82, 1, 4, 0x47, a, b, c, d, 0xff]
                );
            });
            let decoder = query
                .decoder_for_profile(target.profile())
                .expect("profile-bound query");
            assert_eq!(decoder.decode(&expected).expect("midpoint reply"), half);
        }

        let narrower = target_with_optical_limit(true, 0x3000);
        let optical = SetNormalized::new(UnitInterval::MAX, ZoomDomain::Optical);
        prepare_builtin_operation::<Targeted, _>(
            &optical,
            &narrower,
            crate::OperationalTuning::new(),
        )
        .expect("receiving optical endpoint")
        .admit_with(|request, _, _, _| {
            assert_eq!(
                request.wire().as_bytes(),
                &[0x81, 1, 4, 0x47, 3, 0, 0, 0, 0xff]
            );
        });
        let no_combined_range = target(false);
        assert!(matches!(
            crate::prepared::test_support::prepare_inquiry(
                &query,
                &no_combined_range,
                crate::OperationalTuning::new(),
                crate::prepared::ClassSelection::Request
            ),
            Err(Error::FeatureNotSupported { .. })
        ));
        assert!(matches!(
            prepare_builtin_operation::<Targeted, _>(
                &combined,
                &no_combined_range,
                crate::OperationalTuning::new()
            ),
            Err(Error::FeatureNotSupported { .. })
        ));
    }

    #[test]
    fn normalization_permission_and_profile_ranges_are_checked_before_encoding() {
        assert!(matches!(
            NormalizedInquiry::new(ZoomDomain::OpticalPlusDigital)
                .decoder_for_profile(target(false).profile()),
            Err(Error::FeatureNotSupported { .. })
        ));
        let target = target(true);
        let slow = TeleVariable::new(SpeedLevel::Slowest);
        let fast = WideVariable::new(SpeedLevel::Fastest);
        let mut buffer = [0; 6];
        target.encode(&slow, &mut buffer).expect("tele encoding");
        assert_eq!(buffer, [0x81, 1, 4, 7, 0x22, 0xff]);
        target.encode(&fast, &mut buffer).expect("wide encoding");
        assert_eq!(buffer, [0x81, 1, 4, 7, 0x35, 0xff]);
        assert!(matches!(
            target.encode(&TeleVariable::new(ZoomSpeed::MAX), &mut buffer),
            Err(Error::ParameterOutOfRange { .. })
        ));
    }
}
