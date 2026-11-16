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
    inner: builtin::ZoomDrive,
    speed: Speed<ZoomSpeed>,
}
impl TeleVariable {
    /// Resolves a coarse or exact speed without sending a command.
    pub fn for_target(
        target: &TargetRegistration,
        speed: impl Into<Speed<ZoomSpeed>>,
    ) -> crate::Result<Self> {
        let speed = speed.into();
        Ok(Self {
            inner: builtin::ZoomDrive::TeleVariable(lower_speed(target, speed)?),
            speed,
        })
    }
    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<ZoomSpeed> {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    TeleVariable,
    builtin::ZoomDrive,
    AppliedOnly,
    rows = [ZoomTeleVariable]
);

/// Drives toward wide angle at a speed resolved against the target's zoom range.
#[derive(Debug, Clone, Copy)]
pub struct WideVariable {
    inner: builtin::ZoomDrive,
    speed: Speed<ZoomSpeed>,
}
impl WideVariable {
    /// Resolves a coarse or exact speed without sending a command.
    pub fn for_target(
        target: &TargetRegistration,
        speed: impl Into<Speed<ZoomSpeed>>,
    ) -> crate::Result<Self> {
        let speed = speed.into();
        Ok(Self {
            inner: builtin::ZoomDrive::WideVariable(lower_speed(target, speed)?),
            speed,
        })
    }
    /// Returns the requested coarse or exact speed.
    #[must_use]
    pub const fn speed(self) -> Speed<ZoomSpeed> {
        self.speed
    }
}
crate::command::nouns::forward_request!(
    WideVariable,
    builtin::ZoomDrive,
    AppliedOnly,
    rows = [ZoomWideVariable]
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
    inner: builtin::ZoomTarget,
    position: UnitInterval,
    domain: ZoomDomain,
}
impl SetNormalized {
    /// Resolves `0` as wide and `1` as the selected domain's telephoto endpoint.
    ///
    /// The combined optical/digital domain requires a documented digital maximum.
    pub fn for_target(
        target: &TargetRegistration,
        position: UnitInterval,
        domain: ZoomDomain,
    ) -> crate::Result<Self> {
        Ok(Self {
            inner: builtin::ZoomTarget::from_normalized(position, domain, target.profile())?,
            position,
            domain,
        })
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
crate::command::nouns::forward_request!(
    SetNormalized,
    builtin::ZoomTarget,
    Targeted,
    rows = [ZoomPosition]
);

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
    inner: crate::command::ZoomPositionInquiry,
    domain: ZoomDomain,
    maximum: u16,
}
impl NormalizedInquiry {
    /// Selects the target's normalization domain without allocating or sending a query.
    pub fn for_target(target: &TargetRegistration, domain: ZoomDomain) -> crate::Result<Self> {
        let inquiry = Self {
            inner: crate::command::ZoomPositionInquiry,
            domain,
            maximum: normalization_maximum(target, domain)?,
        };
        crate::Request::validate_for_target(&inquiry, target)?;
        Ok(inquiry)
    }

    /// Returns the selected normalization domain.
    #[must_use]
    pub const fn domain(self) -> ZoomDomain {
        self.domain
    }

    fn validate_normalization(&self, target: &TargetRegistration) -> crate::Result<()> {
        if normalization_maximum(target, self.domain)? == self.maximum {
            Ok(())
        } else {
            Err(Error::InvalidRequest {
                reason: "zoom normalization belongs to a different profile range".into(),
            })
        }
    }
}
crate::command::nouns::forward_inquiry!(
    @request NormalizedInquiry, crate::command::ZoomPositionInquiry,
    validate = |request, target| request.validate_normalization(target)
);

impl crate::Inquiry for NormalizedInquiry {
    type Response = UnitInterval;

    fn payload_shape(&self) -> crate::PayloadShape {
        crate::Inquiry::payload_shape(&self.inner)
    }
    fn payload_shape_for_profile(
        &self,
        profile: &crate::profiles::ProfileSpec,
    ) -> crate::PayloadShape {
        crate::Inquiry::payload_shape_for_profile(&self.inner, profile)
    }
    fn decoder(&self) -> crate::ResponseDecoder<Self::Response> {
        crate::ResponseDecoder::with_inline_context([u64::from(self.maximum), 0], decode_normalized)
    }
    fn decoder_for_profile(
        &self,
        _profile: &crate::profiles::ProfileSpec,
    ) -> crate::ResponseDecoder<Self::Response> {
        // Preparation validated that the registered target has these exact bounds.
        self.decoder()
    }
    #[allow(private_interfaces)]
    fn builtin_inquiry_syntax_retry(
        &self,
        authority: crate::requests::BuiltinInquiryAuthority,
    ) -> bool {
        crate::Inquiry::builtin_inquiry_syntax_retry(&self.inner, authority)
    }
}

fn normalization_maximum(target: &TargetRegistration, domain: ZoomDomain) -> crate::Result<u16> {
    let capabilities = target.profile().capabilities();
    let zoom = capabilities
        .zoom
        .as_ref()
        .ok_or(Error::FeatureNotSupported { feature: "zoom" })?;
    if domain == ZoomDomain::OpticalPlusDigital
        && !capabilities.supports(crate::capabilities::TypedSupportSurface::DigitalZoomRange)
    {
        return Err(Error::FeatureNotSupported {
            feature: "optical-plus-digital zoom range",
        });
    }
    Ok(zoom.position_at(UnitInterval::MAX, domain)?.value())
}

fn decode_normalized(context: [u64; 2], payload: &[u8]) -> crate::Result<UnitInterval> {
    let position = crate::Inquiry::decoder(&crate::command::ZoomPositionInquiry).decode(payload)?;
    // Only this module constructs the inline context, from a checked u16 maximum.
    #[allow(clippy::cast_possible_truncation)]
    let maximum = context[0] as u16;
    crate::capabilities::CapabilityRange::<u16>::new(0, maximum)
        .normalize(position.value())
        .map_err(|error| crate::command::inquiry_structs::reply_value_error(error, payload))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        capabilities::{CapabilityRange, TypedSupportSet, TypedSupportSurface},
        profiles::{ProfileId, ProfileSpec},
        types::SpeedLevel,
        CameraId, Inquiry, Request,
    };

    fn target(digital_permission: bool) -> TargetRegistration {
        let base = ProfileId::PtzOpticsG2.spec().expect("base profile");
        let mut facts = base.capabilities().clone();
        facts.profile_id = None;
        let zoom = facts.zoom.as_mut().expect("zoom");
        zoom.digital_max = Some(0x7000);
        zoom.speed = CapabilityRange::<u8>::new(2, 5);
        facts.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
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
            .expect("registered target")
    }

    #[test]
    fn normalized_readback_uses_the_setters_domain_and_preserves_query_policy() {
        let target = target(true);
        let midpoint = UnitInterval::new(0.5).expect("midpoint");
        let setter = SetNormalized::for_target(&target, midpoint, ZoomDomain::OpticalPlusDigital)
            .expect("normalized setter");
        let mut buffer = [0; 9];
        setter
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("target frame");
        assert_eq!(buffer, [0x81, 1, 4, 0x47, 3, 8, 0, 0, 0xff]);
        let query = NormalizedInquiry::for_target(&target, ZoomDomain::OpticalPlusDigital)
            .expect("normalized getter");
        assert_eq!(
            query
                .decoder()
                .decode(&[3, 8, 0, 0])
                .expect("midpoint reply"),
            midpoint
        );
        assert_eq!(
            query
                .decoder()
                .clone()
                .decode(&[7, 0, 0, 0])
                .expect("maximum reply"),
            UnitInterval::MAX
        );
        assert_eq!(query.timeout_class(), crate::TimeoutClass::Inquiry);
        assert_eq!(query.retry_class(), crate::RetryClass::Inquiry);
        let optical =
            NormalizedInquiry::for_target(&target, ZoomDomain::Optical).expect("optical getter");
        assert!(matches!(
            optical.decoder().decode(&[7, 0, 0, 0]),
            Err(Error::InvalidResponse { .. })
        ));
        assert_eq!(
            optical.decoder().decode(&[0, 0, 0, 0]).expect("wide reply"),
            UnitInterval::MIN
        );
    }

    #[test]
    fn normalization_permission_and_profile_ranges_are_checked_before_encoding() {
        assert!(matches!(
            NormalizedInquiry::for_target(&target(false), ZoomDomain::OpticalPlusDigital),
            Err(Error::FeatureNotSupported { .. })
        ));
        let target = target(true);
        let slow =
            TeleVariable::for_target(&target, SpeedLevel::Slowest).expect("minimum coarse speed");
        let fast =
            WideVariable::for_target(&target, SpeedLevel::Fastest).expect("maximum coarse speed");
        let mut buffer = [0; 6];
        slow.write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("tele encoding");
        assert_eq!(buffer, [0x81, 1, 4, 7, 0x22, 0xff]);
        fast.write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("wide encoding");
        assert_eq!(buffer, [0x81, 1, 4, 7, 0x35, 0xff]);
        assert!(matches!(
            TeleVariable::for_target(&target, ZoomSpeed::MAX),
            Err(Error::ParameterOutOfRange { .. })
        ));
    }
}
