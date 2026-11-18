//! Typed request contracts and immutable request semantics.

use std::{
    fmt,
    num::{NonZeroU8, NonZeroUsize},
    sync::Arc,
};

use crate::{completion, request, CameraId, Error, Result};

/// Crate-only authority passed to the hidden applied-state request hook.
///
/// The containing module is private and the token is not constructible by a
/// downstream implementation of [`crate::request::Request`]. This keeps the hook available to
/// typed built-ins while preserving the ordinary custom/raw `Request` default.
#[cfg(any(feature = "blocking", feature = "async"))]
#[doc(hidden)]
pub(crate) struct AppliedStateAuthority(private::SealedToken);

/// Crate-only authority passed to hidden owner-admission request hooks.
///
/// This type is deliberately not nameable by downstream `Request`
/// implementations.  The default hooks therefore preserve the public custom
/// request contract while reserving the urgent lane and dynamic size projection
/// for crate-controlled lowering.
#[doc(hidden)]
pub(crate) struct RequestContractAuthority(private::SealedToken);

/// Crate-only authority passed to the hidden built-in inquiry provenance hook.
///
/// The containing module is private and this token is not constructible by a
/// downstream [`crate::request::Inquiry`] implementation. Generated built-in inquiries use the
/// hook to authorize the narrow `0x02` retry; custom inquiries retain its
/// default `false` result.
#[doc(hidden)]
pub(crate) struct BuiltinInquiryAuthority(private::SealedToken);

mod private {
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct SealedToken;
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl AppliedStateAuthority {
    pub(crate) const fn new() -> Self {
        Self(private::SealedToken)
    }
}

impl RequestContractAuthority {
    pub(crate) const fn new() -> Self {
        Self(private::SealedToken)
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinInquiryAuthority {
    pub(crate) const fn new() -> Self {
        Self(private::SealedToken)
    }
}

/// Retry families meaningful for a command or physical operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum CommandRetryClass {
    /// Never replay the request automatically.
    Never,
    /// Conservative retries for an ordinary idempotent command.
    Standard,
    /// Movement-specific retries for transient camera state.
    Movement,
    /// Preset-specific retries for transient camera state.
    Preset,
}

/// Retry families meaningful for an inquiry's reply-only lifecycle.
///
/// Every inquiry policy accepts only these families. The sealed inquiry class
/// selects [`crate::request::InquiryPolicy`], so custom and raw requests share
/// the same guarantee before submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum InquiryRetryClass {
    /// Never replay an inquiry automatically.
    Never,
    /// Retry transient inquiry failures, including bounded reply loss.
    Inquiry,
}

/// Caller-selected quality-of-service class for ordinary submissions.
///
/// The owner dispatches eligible queued work from the highest occupied class
/// first and in admission order within that class. Typed STOPs and owner-issued
/// cancellation use the urgent safety lane independently of this setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum SubmissionClass {
    /// Opportunistic telemetry or bulk work.
    Background,
    /// Ordinary control traffic.
    Normal,
    /// Direct user interaction.
    User,
}

/// A set of physical camera axes affected by an operation.
///
/// Every value is non-empty, including values created by constants, safe
/// constructors, deserialization, and set operations. Code that needs an
/// optional selection uses `Option<AffectedAxes>` rather than a sentinel empty
/// value.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct AffectedAxes(NonZeroU8);

#[cfg(feature = "serde")]
impl serde::Serialize for AffectedAxes {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for AffectedAxes {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = <u8 as serde::Deserialize>::deserialize(deserializer)?;
        Self::from_bits(bits).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for AffectedAxes {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AffectedAxes".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut schema = <u8 as schemars::JsonSchema>::json_schema(generator);
        schema.insert("minimum".to_owned(), 1.into());
        schema.insert("maximum".to_owned(), 31.into());
        schema
    }
}

/// One physical axis represented by [`crate::types::AffectedAxes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffectedAxis {
    /// Pan and tilt mechanism (treated as one selectable axis).
    PanTilt,
    /// Optical/digital zoom mechanism.
    Zoom,
    /// Focus mechanism.
    Focus,
    /// Iris/aperture mechanism.
    Iris,
    /// Neutral-density filter mechanism.
    NdFilter,
}

/// Iterator over the individual axes in an [`crate::types::AffectedAxes`] set.
#[derive(Debug, Clone, Copy)]
pub struct AffectedAxisIter {
    bits: u8,
}

impl Iterator for AffectedAxisIter {
    type Item = AffectedAxis;

    fn next(&mut self) -> Option<Self::Item> {
        let (bit, axis) = if self.bits & AffectedAxes::PAN_TILT_BIT != 0 {
            (AffectedAxes::PAN_TILT_BIT, AffectedAxis::PanTilt)
        } else if self.bits & AffectedAxes::ZOOM_BIT != 0 {
            (AffectedAxes::ZOOM_BIT, AffectedAxis::Zoom)
        } else if self.bits & AffectedAxes::FOCUS_BIT != 0 {
            (AffectedAxes::FOCUS_BIT, AffectedAxis::Focus)
        } else if self.bits & AffectedAxes::IRIS_BIT != 0 {
            (AffectedAxes::IRIS_BIT, AffectedAxis::Iris)
        } else if self.bits & AffectedAxes::ND_FILTER_BIT != 0 {
            (AffectedAxes::ND_FILTER_BIT, AffectedAxis::NdFilter)
        } else {
            return None;
        };
        self.bits &= !bit;
        Some(axis)
    }
}

impl ExactSizeIterator for AffectedAxisIter {
    fn len(&self) -> usize {
        self.bits.count_ones() as usize
    }
}

impl std::iter::FusedIterator for AffectedAxisIter {}

impl AffectedAxes {
    const PAN_TILT_BIT: u8 = 1 << 0;
    const ZOOM_BIT: u8 = 1 << 1;
    const FOCUS_BIT: u8 = 1 << 2;
    const IRIS_BIT: u8 = 1 << 3;
    const ND_FILTER_BIT: u8 = 1 << 4;
    const VALID_BITS: u8 = Self::PAN_TILT_BIT
        | Self::ZOOM_BIT
        | Self::FOCUS_BIT
        | Self::IRIS_BIT
        | Self::ND_FILTER_BIT;

    // Every caller either supplies one of the non-zero bit constants, combines
    // a non-empty set with OR, or has already rejected zero in `from_bits`.
    // Keep the proof at this single private construction boundary.
    #[allow(clippy::panic, reason = "private callers prove bits is non-zero")]
    const fn from_nonzero_bits(bits: u8) -> Self {
        match NonZeroU8::new(bits) {
            Some(bits) => Self(bits),
            None => panic!("AffectedAxes requires at least one axis"),
        }
    }

    /// The pan and tilt axes.
    pub const PAN_TILT: Self = Self::from_nonzero_bits(Self::PAN_TILT_BIT);
    /// The zoom axis.
    pub const ZOOM: Self = Self::from_nonzero_bits(Self::ZOOM_BIT);
    /// The focus axis.
    pub const FOCUS: Self = Self::from_nonzero_bits(Self::FOCUS_BIT);
    /// The iris/aperture axis.
    pub const IRIS: Self = Self::from_nonzero_bits(Self::IRIS_BIT);
    /// The neutral-density filter axis.
    pub const ND_FILTER: Self = Self::from_nonzero_bits(Self::ND_FILTER_BIT);

    /// Every physical axis this type can represent.
    ///
    /// A motion observation over this set requires the profile to declare
    /// pan/tilt, zoom, focus, iris, *and* ND-filter position inquiries. Use
    /// [`Self::MOVEMENT`] to select the three mechanical axes on profiles that
    /// provide position inquiries for them.
    pub const ALL: Self = Self::from_nonzero_bits(Self::VALID_BITS);

    /// The three mechanical movement axes: pan/tilt, zoom, and focus.
    ///
    /// [`crate::types::IdleWait::default`] and the named wait presets select
    /// these axes. For a profile with fewer position inquiries, select the
    /// axes it can observe.
    pub const MOVEMENT: Self =
        Self::from_nonzero_bits(Self::PAN_TILT_BIT | Self::ZOOM_BIT | Self::FOCUS_BIT);

    /// Constructs an explicit non-empty set of affected axes.
    ///
    /// # Errors
    ///
    /// Returns an error when all arguments are `false`.
    pub fn new(pan_tilt: bool, zoom: bool, focus: bool) -> Result<Self> {
        let bits = (u8::from(pan_tilt) * Self::PAN_TILT_BIT)
            | (u8::from(zoom) * Self::ZOOM_BIT)
            | (u8::from(focus) * Self::FOCUS_BIT);
        Self::from_bits(bits)
    }

    /// Constructs an explicit non-empty set including iris and ND-filter
    /// selection.
    ///
    /// The original three-argument [`Self::new`] remains available for the
    /// pan/tilt, zoom, and focus subset. This extended constructor keeps the
    /// additional physical axes explicit without introducing an empty or
    /// implicit all-axis value.
    pub fn new_with_iris_nd(
        pan_tilt: bool,
        zoom: bool,
        focus: bool,
        iris: bool,
        nd_filter: bool,
    ) -> Result<Self> {
        let bits = (u8::from(pan_tilt) * Self::PAN_TILT_BIT)
            | (u8::from(zoom) * Self::ZOOM_BIT)
            | (u8::from(focus) * Self::FOCUS_BIT)
            | (u8::from(iris) * Self::IRIS_BIT)
            | (u8::from(nd_filter) * Self::ND_FILTER_BIT);
        Self::from_bits(bits)
    }

    /// Reconstructs an axis set from its stable bit representation.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty set or for unknown bits.
    pub fn from_bits(bits: u8) -> Result<Self> {
        if bits == 0 || bits & !Self::VALID_BITS != 0 {
            return Err(Error::InvalidRequest {
                reason: "affected axes must be non-empty and contain only known axes".into(),
            });
        }
        Ok(Self::from_nonzero_bits(bits))
    }

    /// Returns the stable bit representation.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0.get()
    }

    /// Returns whether this set contains every axis in `other`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.bits() & other.bits() == other.bits()
    }

    /// Returns the number of selected physical axes.
    #[must_use]
    pub const fn axis_count(self) -> usize {
        self.bits().count_ones() as usize
    }

    /// Returns whether exactly one physical axis is selected.
    #[must_use]
    pub const fn is_single(self) -> bool {
        self.bits().count_ones() == 1
    }

    /// Returns an iterator over selected axes in canonical bit order.
    #[must_use]
    pub const fn iter(self) -> AffectedAxisIter {
        AffectedAxisIter { bits: self.bits() }
    }

    /// Combines two non-empty sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self::from_nonzero_bits(self.bits() | other.bits())
    }

    /// Returns the non-empty intersection, or `None` when the sets are
    /// disjoint.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Option<Self> {
        let bits = self.bits() & other.bits();
        match NonZeroU8::new(bits) {
            Some(bits) => Some(Self(bits)),
            None => None,
        }
    }

    /// Combines `other` only when `condition` is true.
    #[must_use]
    pub const fn union_if(self, condition: bool, other: Self) -> Self {
        if condition {
            self.union(other)
        } else {
            self
        }
    }
}

impl std::ops::BitOr for AffectedAxes {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

impl std::ops::BitOrAssign for AffectedAxes {
    fn bitor_assign(&mut self, other: Self) {
        *self = self.union(other);
    }
}

impl fmt::Debug for AffectedAxes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AffectedAxes")
            .field("pan_tilt", &self.contains(Self::PAN_TILT))
            .field("zoom", &self.contains(Self::ZOOM))
            .field("focus", &self.contains(Self::FOCUS))
            .field("iris", &self.contains(Self::IRIS))
            .field("nd_filter", &self.contains(Self::ND_FILTER))
            .finish()
    }
}

impl TryFrom<u8> for AffectedAxes {
    type Error = Error;

    fn try_from(bits: u8) -> Result<Self> {
        Self::from_bits(bits)
    }
}

impl From<AffectedAxes> for u8 {
    fn from(axes: AffectedAxes) -> Self {
        axes.bits()
    }
}

type SharedDecoder<R> = dyn Fn(&[u8]) -> Result<R> + Send + Sync + 'static;

enum DecoderInner<R> {
    Function(fn(&[u8]) -> Result<R>),
    NormalizedZoom {
        range: crate::capabilities::CapabilityRange<u16>,
        lift: fn(crate::types::UnitInterval) -> Result<R>,
    },
    Builtin {
        kind: crate::command::InquiryKind,
        framing: crate::command::pan_tilt_wire::PanTiltFraming,
        lift: fn(crate::command::Response) -> Result<R>,
    },
    #[cfg(any(feature = "blocking", feature = "async"))]
    PanTiltPosition {
        coordinates: crate::profiles::TargetPanTiltCoordinates,
        lift: fn(crate::types::PanTiltPosition) -> Result<R>,
    },
    ColorTemperature {
        steps: Arc<[u16]>,
        lift: fn(crate::types::Kelvin) -> Result<R>,
    },
    Shared(Arc<SharedDecoder<R>>),
}

/// An owned typed inquiry decoder.
///
/// Function decoders and built-in profile decoding facts are stored inline.
/// Custom context-bearing decoders use shared immutable storage so a prepared
/// request remains movable.
pub struct ResponseDecoder<R> {
    inner: DecoderInner<R>,
}

impl<R> ResponseDecoder<R> {
    /// Creates an allocation-free decoder from a function pointer.
    #[must_use]
    pub const fn from_fn(decode: fn(&[u8]) -> Result<R>) -> Self {
        Self {
            inner: DecoderInner::Function(decode),
        }
    }

    /// Stores the canonical raw response decoder and its profile framing inline.
    pub(crate) fn for_builtin_profile(
        kind: crate::command::InquiryKind,
        profile: &crate::ProfileSpec,
        lift: fn(crate::command::Response) -> Result<R>,
    ) -> Result<Self> {
        use crate::command::{pan_tilt_wire::PanTiltFraming, InquiryKind};
        let framing = if kind == InquiryKind::PanTiltPosition {
            PanTiltFraming::for_conversion(require_pan_tilt_coordinates(profile)?)
        } else {
            PanTiltFraming::STANDARD
        };
        Ok(Self {
            inner: DecoderInner::Builtin {
                kind,
                framing,
                lift,
            },
        })
    }

    /// Shares the profile's existing table and returns typed Kelvin directly.
    pub(crate) fn for_color_temperature(
        profile: &crate::ProfileSpec,
        lift: fn(crate::types::Kelvin) -> Result<R>,
    ) -> Result<Self> {
        let steps = profile
            .capabilities()
            .white_balance
            .as_ref()
            .filter(|domain| !domain.color_temperature_steps.is_empty())
            .map(|domain| Arc::clone(&domain.color_temperature_steps))
            .ok_or(Error::FeatureNotSupported {
                feature: "color-temperature inquiry",
            })?;
        Ok(Self {
            inner: DecoderInner::ColorTemperature { steps, lift },
        })
    }

    /// Creates a decoder that owns immutable decoding context.
    #[must_use]
    pub fn with_context<C>(context: C, decode: fn(&C, &[u8]) -> Result<R>) -> Self
    where
        C: Send + Sync + 'static,
        R: 'static,
    {
        Self {
            inner: DecoderInner::Shared(Arc::new(move |payload| decode(&context, payload))),
        }
    }

    /// Decodes one response payload.
    pub fn decode(&self, payload: &[u8]) -> Result<R> {
        match &self.inner {
            DecoderInner::Function(decode) => decode(payload),
            DecoderInner::NormalizedZoom { range, lift } => {
                let position =
                    Inquiry::decoder(&crate::command::ZoomPositionInquiry).decode(payload)?;
                range
                    .normalize(position.value())
                    .and_then(lift)
                    .map_err(|error| {
                        crate::command::inquiry_structs::reply_value_error(error, payload)
                    })
            }
            DecoderInner::Builtin {
                kind,
                framing,
                lift,
            } => {
                use crate::command::{inquiry_structs, Payload};
                inquiry_structs::dispatch_with_framing(*kind, Payload::new(payload), *framing)
                    .and_then(lift)
                    .map_err(|error| inquiry_structs::reply_value_error(error, payload))
            }
            #[cfg(any(feature = "blocking", feature = "async"))]
            DecoderInner::PanTiltPosition { coordinates, lift } => {
                use crate::command::{
                    inquiry_structs, pan_tilt_wire::PanTiltFraming, InquiryData, InquiryKind,
                    Payload, Response,
                };
                inquiry_structs::dispatch_with_framing(
                    InquiryKind::PanTiltPosition,
                    Payload::new(payload),
                    PanTiltFraming::for_conversion(coordinates.conversion),
                )
                .and_then(|response| match response {
                    Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt }) => coordinates
                        .position_degrees(crate::types::PanTiltUnits::new(pan, tilt))
                        .and_then(lift),
                    _ => Err(Error::UnexpectedResponseType),
                })
                .map_err(|error| inquiry_structs::reply_value_error(error, payload))
            }
            DecoderInner::ColorTemperature { steps, lift } => {
                let response = crate::command::inquiry_structs::dispatch(
                    crate::command::InquiryKind::ColorTemperature,
                    crate::command::Payload::new(payload),
                );
                response
                    .and_then(|response| match response {
                        crate::command::Response::Inquiry(
                            crate::command::InquiryData::ColorTemperature { temperature },
                        ) => crate::capabilities::WhiteBalanceCaps::temperature_from_step(
                            steps,
                            temperature,
                        )
                        .and_then(lift),
                        _ => Err(Error::UnexpectedResponseType),
                    })
                    .map_err(|error| {
                        crate::command::inquiry_structs::reply_value_error(error, payload)
                    })
            }
            DecoderInner::Shared(decode) => decode(payload),
        }
    }
}

impl ResponseDecoder<crate::types::UnitInterval> {
    pub(crate) const fn normalized_zoom(range: crate::capabilities::CapabilityRange<u16>) -> Self {
        Self {
            inner: DecoderInner::NormalizedZoom { range, lift: Ok },
        }
    }
}

fn require_pan_tilt_coordinates(
    profile: &crate::ProfileSpec,
) -> Result<crate::profiles::PanTiltCoordinateConversion> {
    profile
        .pan_tilt_coordinates()
        .ok_or(Error::FeatureNotSupported {
            feature: "pan/tilt coordinate conversion",
        })
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl ResponseDecoder<crate::types::PanTiltPosition> {
    pub(crate) fn for_pan_tilt_position(target: &crate::TargetRegistration) -> Result<Self> {
        Ok(Self {
            inner: DecoderInner::PanTiltPosition {
                coordinates: target.pan_tilt_coordinates()?,
                lift: Ok,
            },
        })
    }
}

impl<R> Clone for ResponseDecoder<R> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            DecoderInner::Function(decode) => DecoderInner::Function(*decode),
            DecoderInner::NormalizedZoom { range, lift } => DecoderInner::NormalizedZoom {
                range: *range,
                lift: *lift,
            },
            DecoderInner::Builtin {
                kind,
                framing,
                lift,
            } => DecoderInner::Builtin {
                kind: *kind,
                framing: *framing,
                lift: *lift,
            },
            #[cfg(any(feature = "blocking", feature = "async"))]
            DecoderInner::PanTiltPosition { coordinates, lift } => DecoderInner::PanTiltPosition {
                coordinates: *coordinates,
                lift: *lift,
            },
            DecoderInner::ColorTemperature { steps, lift } => DecoderInner::ColorTemperature {
                steps: Arc::clone(steps),
                lift: *lift,
            },
            DecoderInner::Shared(decode) => DecoderInner::Shared(Arc::clone(decode)),
        };
        Self { inner }
    }
}

impl<R> fmt::Debug for ResponseDecoder<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let storage = match self.inner {
            DecoderInner::Function(_) => "function",
            DecoderInner::Builtin { .. }
            | DecoderInner::NormalizedZoom { .. }
            | DecoderInner::ColorTemperature { .. } => "inline-builtin",
            #[cfg(any(feature = "blocking", feature = "async"))]
            DecoderInner::PanTiltPosition { .. } => "inline-builtin",
            DecoderInner::Shared(_) => "shared-context",
        };
        formatter
            .debug_struct("ResponseDecoder")
            .field("storage", &storage)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod bound_temperature_decoder_tests {
    use super::{DecoderInner, Inquiry, ResponseDecoder};
    use crate::{
        command::{ColorTemperatureInquiry, InquiryData, InquiryKind, Payload, Response},
        profiles::{GenericVisca, PtzOpticsG2},
        types::Kelvin,
        Error, ProfileSpec, Result,
    };
    use std::sync::Arc;

    #[derive(crate::ViscaInquiry)]
    #[visca(opcode = 0, response = ColorTemperature)]
    struct RawTemperatureInquiry;

    fn custom_profile(steps: Arc<[u16]>) -> Result<ProfileSpec> {
        let baseline = ProfileSpec::from_compile_time::<PtzOpticsG2>()?;
        let mut capabilities = baseline.capabilities().clone();
        capabilities.profile_id = None;
        let white_balance =
            capabilities
                .white_balance
                .as_mut()
                .ok_or(Error::FeatureNotSupported {
                    feature: "test white balance",
                })?;
        white_balance.color_temperature_steps = steps;
        let mut builder = ProfileSpec::builder(capabilities)
            .transports(baseline.transports())
            .envelope(baseline.envelope())
            .timing(baseline.timing())
            .maximum_command_sockets(baseline.maximum_command_sockets())
            .position_inquiries(baseline.position_inquiries())
            .supports_operation_complete(baseline.supports_operation_complete())
            .supports_command_cancel(baseline.supports_command_cancel());
        for selection in baseline.encodings().iter() {
            builder = builder.encoding(selection);
        }
        builder.build()
    }

    fn shares_steps(decoder: &ResponseDecoder<Kelvin>, steps: &Arc<[u16]>) -> bool {
        matches!(
            &decoder.inner,
            DecoderInner::ColorTemperature {
                steps: stored,
                ..
            } if Arc::ptr_eq(stored, steps)
        )
    }

    #[test]
    fn bound_temperature_decoder_uses_owned_profile_steps_without_double_conversion() -> Result<()>
    {
        let steps: Arc<[u16]> = Arc::from([3100, 4200, 6300]);
        let profile = custom_profile(Arc::clone(&steps))?;
        let decoder = ColorTemperatureInquiry.decoder_for_profile(&profile)?;
        let cloned = decoder.clone();

        assert!(shares_steps(&decoder, &steps));
        assert!(shares_steps(&cloned, &steps));
        assert_eq!(decoder.decode(&[1])?.value(), 4200);
        assert_eq!(
            ColorTemperatureInquiry.decoder().decode(&[1])?.value(),
            2600
        );
        assert!(matches!(
            crate::command::inquiry_structs::dispatch(
                InquiryKind::ColorTemperature,
                Payload::new(&[1]),
            )?,
            Response::Inquiry(InquiryData::ColorTemperature { temperature: 1 })
        ));
        assert!(matches!(
            RawTemperatureInquiry
                .decoder_for_profile(&profile)?
                .decode(&[1])?,
            Response::Inquiry(InquiryData::ColorTemperature { temperature: 1 })
        ));

        drop(profile);
        drop(steps);
        assert_eq!(cloned.decode(&[2])?.value(), 6300);
        Ok(())
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn custom_temperature_table_encodes_and_decodes_the_same_steps() -> Result<()> {
        let profile = custom_profile(Arc::from([3100, 4200, 6300]))?;
        let decoder = ColorTemperatureInquiry.decoder_for_profile(&profile)?;
        let target = crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile);
        for (kelvin, wire) in [(3100, 0), (4200, 1), (6300, 2)] {
            let temperature = Kelvin::new(kelvin)?;
            let command = crate::command::white_balance::SetColorTemperature::new(temperature);
            crate::Request::validate_for_target(&command, &target)?;
            let mut buffer = [0; 7];
            let length = super::write_for_target(&command, &target, &mut buffer)?;
            assert_eq!(&buffer[..length], &[0x81, 1, 4, 0x20, 0, wire, 0xff]);
            assert_eq!(decoder.decode(&[wire])?, temperature);
        }
        Ok(())
    }

    #[test]
    fn bound_temperature_decoder_requires_a_declared_table() -> Result<()> {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>()?;
        assert!(matches!(
            ColorTemperatureInquiry.decoder_for_profile(&profile),
            Err(Error::FeatureNotSupported { .. })
        ));
        assert!(matches!(
            RawTemperatureInquiry
                .decoder_for_profile(&profile)?
                .decode(&[0])?,
            Response::Inquiry(InquiryData::ColorTemperature { temperature: 0 })
        ));
        Ok(())
    }

    #[test]
    fn bound_temperature_decoder_rejects_unknown_steps_with_original_payload() -> Result<()> {
        let profile = custom_profile(Arc::from([3100, 4200, 6300]))?;
        let decoder = ColorTemperatureInquiry.decoder_for_profile(&profile)?;
        assert!(matches!(
            decoder.decode(&[3]),
            Err(Error::InvalidResponse { actual, .. }) if actual == [3]
        ));
        assert!(decoder.decode(&[]).is_err());
        assert!(decoder.decode(&[0, 1]).is_err());
        Ok(())
    }
}

/// The frame length and owner facts produced by one target-specific lowering.
pub(crate) struct LoweredRequest {
    pub written: usize,
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub requires_extended_pan_tilt_speed: bool,
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub applied_state: Option<crate::runtime::engine::AppliedStateProjection>,
}

pub(crate) fn write_lowered<R: Request + ?Sized>(
    request: &R,
    target: &crate::TargetRegistration,
    buffer: &mut [u8],
) -> Result<LoweredRequest> {
    let written = request.write_for_target(target, buffer, RequestContractAuthority::new())?;
    Ok(LoweredRequest {
        written,
        #[cfg(any(feature = "blocking", feature = "async"))]
        requires_extended_pan_tilt_speed: target
            .profile()
            .capabilities()
            .pan_tilt
            .as_ref()
            .is_some_and(|pan_tilt| pan_tilt.variable_speed)
            && request
                .pan_tilt_max_speed(RequestContractAuthority::new())
                .is_some_and(|speed| speed > crate::command::PanTiltSpeedMode::STANDARD_MAXIMUM),
        #[cfg(any(feature = "blocking", feature = "async"))]
        applied_state: request.applied_state_projection(AppliedStateAuthority::new()),
    })
}

pub(crate) fn encode_for_target<R: Request + ?Sized, T>(
    request: &R,
    target: &crate::TargetRegistration,
    buffer: &mut [u8],
    before_write: impl FnOnce() -> Result<T>,
) -> Result<(LoweredRequest, T)> {
    let declared_max = declared_max_size(request);
    let maximum = declared_max.min(crate::raw::MAX_BYTES);
    let available = maximum.min(buffer.len());
    let (lowered, result) = match request.lower_for_target(
        target,
        &mut buffer[..available],
        before_write,
        RequestContractAuthority::new(),
    ) {
        Err(Error::BufferTooSmall { required, .. }) if required > maximum => {
            return Err(Error::InvalidRequest {
                reason: format!(
                    "encoded request needs {required} bytes, exceeding MAX_SIZE ({declared_max}) or raw::MAX_BYTES ({})",
                    crate::raw::MAX_BYTES
                ).into(),
            });
        }
        other => other?,
    };
    let written = lowered.written;
    if written > maximum {
        return Err(Error::InvalidRequest {
            reason: format!(
                "write_into reported {written} bytes, exceeding MAX_SIZE ({declared_max}) or raw::MAX_BYTES ({})",
                crate::raw::MAX_BYTES
            ).into(),
        });
    }
    if written > available {
        return Err(Error::InvalidRequest {
            reason: "encoder returned a length beyond its supplied buffer".into(),
        });
    }
    let bytes = &buffer[..written];
    crate::raw::validate_wire(bytes)?;
    let camera = CameraId::from(target.target());
    let address = camera.to_address_byte();
    if bytes[0] != address {
        return Err(Error::InvalidRequest {
            reason: format!(
                "request targets VISCA address 0x{:02x}, but the selected camera target is {} (0x{address:02x})",
                bytes[0],
                camera,
            ).into(),
        });
    }
    let maximum = target.profile().maximum_request_bytes();
    if written > maximum {
        return Err(Error::InvalidRequest {
            reason: format!(
                "encoded request has {written} bytes, exceeding this profile's supported envelope payload bound ({maximum})"
            ).into(),
        });
    }
    Ok((lowered, result))
}

/// Refuses profile-free encoding for requests that must lower against a target.
pub(crate) fn target_required_for_encoding() -> Error {
    Error::InvalidRequest {
        reason: "request encoding requires a target profile".into(),
    }
}

/// A request that can be fully encoded before owner admission.
pub trait Request: Send + Sync {
    /// Closed semantic request class.
    type Class: request::Class;

    /// Maximum encoded byte length for this request type, including the target
    /// address and final terminator. The encoder may write a shorter frame.
    const MAX_SIZE: usize;
    /// Returns the policy selected by this request's lifecycle class.
    ///
    /// Plain commands return [`crate::request::CommandPolicy`], operations
    /// return [`crate::request::OperationPolicy`], and inquiries return
    /// [`crate::request::InquiryPolicy`]. Invalid lifecycle combinations cannot
    /// be declared, including an urgent ordinary submission class.
    fn policy(&self) -> <Self::Class as request::Class>::Policy;

    /// Declares axes moved by a plain custom command for owner halt fencing.
    ///
    /// Undeclared plain/raw commands are never suppressed by a halt. Operation
    /// commands instead use their mandatory [`crate::request::OperationCommand::affected_axes`].
    /// This grants no reserved STOP or cancellation authority.
    fn motion_axes(&self) -> Option<AffectedAxes> {
        None
    }

    /// Resolves the request's class for owner admission.
    ///
    /// The private authority reserves STOP urgency for reviewed built-ins.
    /// The default derives an ordinary class from the typed policy.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn admission(
        &self,
        _authority: RequestContractAuthority,
    ) -> request::policy::AdmissionOf<Self::Class> {
        request::policy::Sealed::ordinary_admission(self.policy())
    }

    /// Returns the fixed encoded-size declaration used for owner admission.
    ///
    /// The default is exactly [`Self::MAX_SIZE`]. The hidden authority lets an
    /// internal erased adapter project the original concrete request's bound,
    /// or a crate-owned raw value tighten it to its already validated frame
    /// length, without allowing a downstream implementation to relax its
    /// static declaration.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn declared_max_size(&self, _authority: RequestContractAuthority) -> usize {
        Self::MAX_SIZE
    }

    /// Writes the complete terminated VISCA request into `buffer` and returns
    /// the number of bytes written, at most [`Self::MAX_SIZE`].
    ///
    /// This is only the profile-free encoding hook: it receives the explicit
    /// target address, but not a [`crate::profiles::ProfileSpec`]. Do not use it for
    /// capability or profile-range validation. Use
    /// [`crate::config::TargetRegistration::encode`] for target-bound encoding.
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize>;

    /// Encodes a reviewed built-in request against the selected target.
    ///
    /// The private authority reserves profile-dependent packet selection for
    /// crate-owned requests. Downstream requests retain their explicit bytes.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn write_for_target(
        &self,
        target: &crate::TargetRegistration,
        buffer: &mut [u8],
        _authority: RequestContractAuthority,
    ) -> Result<usize> {
        self.write_into(target.target().into(), buffer)
    }

    /// Validates target-specific capability, range and installation facts before encoding.
    ///
    /// Shared preparation validates the request before encoding, owner admission,
    /// or transport I/O. Built-in intents can combine validation with lowering
    /// and return [`Error::FeatureNotSupported`] for an unavailable feature.
    ///
    /// Raw and other downstream requests are explicit low-level extensions:
    /// they may override this hook with their own profile validation, while a
    /// custom request with no profile-specific constraints may use the default.
    /// The direct generic APIs intentionally do not replace the noun/accessor
    /// `Has*` marker bounds with request-specific compile-time `P` bounds.
    fn validate_for_target(&self, _target: &crate::TargetRegistration) -> Result<()> {
        Ok(())
    }

    /// Validates and lowers the request against its receiving target.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn lower_for_target<T>(
        &self,
        target: &crate::TargetRegistration,
        buffer: &mut [u8],
        before_write: impl FnOnce() -> Result<T>,
        _authority: RequestContractAuthority,
    ) -> Result<(LoweredRequest, T)> {
        self.validate_for_target(target)?;
        let result = before_write()?;
        Ok((write_lowered(self, target, buffer)?, result))
    }

    /// Carries the largest reviewed built-in pan/tilt speed to owner admission.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn pan_tilt_max_speed(&self, _authority: RequestContractAuthority) -> Option<u8> {
        None
    }

    /// Returns a crate-selected state effect for exact application.
    ///
    /// The authority token is intentionally hidden and non-forgeable outside
    /// this crate. Built-ins use it to carry their closed effect through the
    /// same generic preparation path as raw and downstream requests; those
    /// other request types retain the default `None`.
    #[cfg(any(feature = "blocking", feature = "async"))]
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn applied_state_projection(
        &self,
        _authority: AppliedStateAuthority,
    ) -> Option<crate::runtime::engine::AppliedStateProjection> {
        None
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
/// Reads a request's authorized scheduling class at the owner boundary.
pub(crate) fn admission<R: Request + ?Sized>(
    request: &R,
) -> request::policy::AdmissionOf<R::Class> {
    request.admission(RequestContractAuthority::new())
}

/// Reads a request's fixed size declaration at the owner boundary.
pub(crate) fn declared_max_size<R: Request + ?Sized>(request: &R) -> usize {
    request.declared_max_size(RequestContractAuthority::new())
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) fn applied_state_projection<R: Request + ?Sized>(
    request: &R,
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    request.applied_state_projection(AppliedStateAuthority::new())
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) fn write_for_target<R: Request + ?Sized>(
    request: &R,
    target: &crate::TargetRegistration,
    buffer: &mut [u8],
) -> Result<usize> {
    request.write_for_target(target, buffer, RequestContractAuthority::new())
}

/// A command with no physical-operation handle semantics.
pub trait PlainCommand: Request<Class = request::PlainClass> {}

impl<T> PlainCommand for T where T: Request<Class = request::PlainClass> + ?Sized {}

/// Structural evidence describing an inquiry's decoded data payload.
///
/// Exact lengths count one or more data bytes, excluding the VISCA address,
/// reply header and terminator. Shape constrains framing and nibble width.
/// The response decoder checks documented values, padding and profile ranges.
///
/// # Examples
///
/// ```
/// use grafton_visca::request::PayloadShape;
/// use std::num::NonZeroUsize;
///
/// let one_nibble = PayloadShape::Nibbles(NonZeroUsize::MIN);
/// assert!(one_nibble.admits(&[0x0F]));
/// assert!(!one_nibble.admits(&[]));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PayloadShape {
    /// No declared structure. Admits every payload and overlaps every shape.
    #[default]
    Unknown,
    /// Exactly this many data bytes, with unrestricted byte values.
    Bytes(NonZeroUsize),
    /// Exactly this many data bytes, each in `0x00..=0x0F`.
    Nibbles(NonZeroUsize),
}

impl PayloadShape {
    /// Returns whether decoded data satisfies this structural declaration.
    #[must_use]
    pub fn admits(self, payload: &[u8]) -> bool {
        match self {
            Self::Unknown => true,
            Self::Bytes(length) => payload.len() == length.get(),
            Self::Nibbles(length) => {
                payload.len() == length.get() && payload.iter().all(|byte| *byte <= 0x0F)
            }
        }
    }

    /// Returns whether one payload could satisfy both shapes.
    ///
    /// Known shapes overlap exactly when their lengths agree. `Unknown`
    /// overlaps every shape, so overlap is pairwise and is not transitive:
    /// two different lengths each overlap `Unknown` without overlapping each other.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => true,
            (
                Self::Bytes(left) | Self::Nibbles(left),
                Self::Bytes(right) | Self::Nibbles(right),
            ) => left.get() == right.get(),
        }
    }
}

/// A typed inquiry request.
pub trait Inquiry: Request<Class = request::InquiryClass> {
    /// Successfully decoded response value.
    type Response: Send + 'static;

    /// Returns the structural shape accepted by this inquiry's decoder.
    ///
    /// Custom inquiries may declare a shape to supply correlation evidence for
    /// raw VISCA replies. The default retains conservative positional behavior.
    /// On raw transports, an exact shape exceeding the configured reply frame
    /// limit after VISCA framing overhead fails with [`Error::InvalidParameter`]
    /// before engine admission.
    /// A raw target reserves at most 16 exact declarations across admitted
    /// inquiries, unpaid replies and skew holds. A new declaration at that
    /// limit fails with [`Error::InquiryShapeCapacityExhausted`]; releasing its
    /// retained evidence frees a reservation. Equal-length declarations overlap
    /// even when their byte/nibble restrictions differ.
    fn payload_shape(&self) -> PayloadShape {
        PayloadShape::Unknown
    }

    /// Selects profile-dependent reply framing during pure preparation.
    #[doc(hidden)]
    fn payload_shape_for_profile(&self, _profile: &crate::ProfileSpec) -> PayloadShape {
        self.payload_shape()
    }

    /// Returns the owned response decoder.
    fn decoder(&self) -> ResponseDecoder<Self::Response>;

    /// Returns a decoder owning any immutable runtime-profile context it needs.
    ///
    /// Most inquiries are profile-independent and use [`Self::decoder`].
    /// Profile-sensitive inquiries bind their decoding facts during pure
    /// preparation and may reject a profile that cannot decode their reply.
    #[doc(hidden)]
    fn decoder_for_profile(
        &self,
        _profile: &crate::ProfileSpec,
    ) -> Result<ResponseDecoder<Self::Response>> {
        Ok(self.decoder())
    }

    /// Returns whether this inquiry is one of the crate-generated built-ins
    /// eligible for the narrow syntax-error replay policy.
    ///
    /// The private authority keeps this provenance closed: downstream/custom
    /// inquiry implementations use the default and cannot opt themselves in.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn builtin_inquiry_syntax_retry(&self, _authority: BuiltinInquiryAuthority) -> bool {
        false
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
/// Reads the closed built-in inquiry provenance during generic preparation.
pub(crate) fn builtin_inquiry_syntax_retry<Q>(inquiry: &Q) -> bool
where
    Q: Inquiry + ?Sized,
{
    inquiry.builtin_inquiry_syntax_retry(BuiltinInquiryAuthority::new())
}

/// A command with explicit physical-operation semantics.
pub trait OperationCommand<K>: Request<Class = request::OperationClass<K>>
where
    K: completion::Kind,
{
    /// Returns the affected axes. Profile-dependent built-ins report a
    /// conservative superset before preparation selects the exact axes.
    fn affected_axes(&self) -> AffectedAxes;

    /// Resolves the exact motion axes against the receiving target.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn affected_axes_for_target(
        &self,
        _target: &crate::TargetRegistration,
        _authority: RequestContractAuthority,
    ) -> Result<AffectedAxes> {
        Ok(self.affected_axes())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod affected_axes_tests {
    use std::mem::size_of;

    use super::AffectedAxes;

    #[test]
    fn all_contains_every_axis_and_movement_contains_only_the_mechanical_three() {
        for axis in [
            AffectedAxes::PAN_TILT,
            AffectedAxes::ZOOM,
            AffectedAxes::FOCUS,
            AffectedAxes::IRIS,
            AffectedAxes::ND_FILTER,
        ] {
            assert!(AffectedAxes::ALL.contains(axis));
        }
        assert_eq!(AffectedAxes::ALL.axis_count(), 5);

        assert_eq!(AffectedAxes::MOVEMENT.axis_count(), 3);
        assert!(AffectedAxes::MOVEMENT.contains(AffectedAxes::PAN_TILT));
        assert!(AffectedAxes::MOVEMENT.contains(AffectedAxes::ZOOM));
        assert!(AffectedAxes::MOVEMENT.contains(AffectedAxes::FOCUS));
        assert!(!AffectedAxes::MOVEMENT.contains(AffectedAxes::IRIS));
        assert!(!AffectedAxes::MOVEMENT.contains(AffectedAxes::ND_FILTER));
        assert!(AffectedAxes::ALL.contains(AffectedAxes::MOVEMENT));
    }

    #[test]
    fn empty_values_cannot_be_constructed() {
        assert!(AffectedAxes::from_bits(0).is_err());
        assert!(AffectedAxes::new(false, false, false).is_err());
        assert!(AffectedAxes::new_with_iris_nd(false, false, false, false, false).is_err());
        assert_eq!(
            size_of::<AffectedAxes>(),
            size_of::<u8>(),
            "the non-zero representation should retain the one-byte niche"
        );
        assert_eq!(
            size_of::<Option<AffectedAxes>>(),
            size_of::<u8>(),
            "Option should use the non-zero niche instead of adding a sentinel state"
        );
    }

    #[test]
    fn union_and_optional_intersection_preserve_non_empty_values() {
        let combined = AffectedAxes::PAN_TILT | AffectedAxes::ZOOM | AffectedAxes::FOCUS;
        assert_eq!(combined, AffectedAxes::MOVEMENT);
        assert_eq!(
            combined,
            AffectedAxes::PAN_TILT
                .union(AffectedAxes::ZOOM)
                .union(AffectedAxes::FOCUS)
        );

        let mut accumulated = AffectedAxes::PAN_TILT;
        accumulated |= AffectedAxes::ZOOM;
        accumulated |= AffectedAxes::FOCUS;
        assert_eq!(accumulated, AffectedAxes::MOVEMENT);

        assert_eq!(
            AffectedAxes::ALL.intersection(AffectedAxes::MOVEMENT),
            Some(combined)
        );
        assert_eq!(
            AffectedAxes::MOVEMENT.intersection(AffectedAxes::IRIS),
            None
        );
    }

    #[test]
    fn all_round_trips_through_its_bit_representation() {
        assert_eq!(
            AffectedAxes::from_bits(AffectedAxes::ALL.bits()).expect("ALL is non-empty"),
            AffectedAxes::ALL
        );
        assert_eq!(
            AffectedAxes::from_bits(AffectedAxes::MOVEMENT.bits()).expect("MOVEMENT is non-empty"),
            AffectedAxes::MOVEMENT
        );
    }
}

#[cfg(test)]
mod payload_shape_tests {
    use super::{Inquiry, NonZeroUsize, PayloadShape, Request, ResponseDecoder};

    #[test]
    fn payload_shape_admission_checks_exact_length_and_every_nibble() {
        assert_eq!(PayloadShape::default(), PayloadShape::Unknown);
        for payload in [vec![], vec![0], vec![0xFF; 32]] {
            assert!(PayloadShape::Unknown.admits(&payload));
        }
        for shape in [PayloadShape::Bytes, PayloadShape::Nibbles] {
            assert_eq!(NonZeroUsize::new(0).map(shape), None);
            assert!(shape(NonZeroUsize::MIN).admits(&[0x0F]));
            assert!(!shape(NonZeroUsize::MIN).admits(&[]));
            assert!(!shape(NonZeroUsize::MAX).admits(&[]));
        }
        let four = NonZeroUsize::MIN.saturating_add(3);
        let bytes = PayloadShape::Bytes(four);
        let nibbles = PayloadShape::Nibbles(four);
        assert!(bytes.admits(&[0xFF; 4]));
        assert!(nibbles.admits(&[0, 1, 14, 15]));
        for length in [0, 1, 3, 5, 10] {
            assert!(!bytes.admits(&vec![0; length]));
            assert!(!nibbles.admits(&vec![0; length]));
        }
        for index in 0..4 {
            for byte in 0x10..=u8::MAX {
                let mut payload = [0; 4];
                payload[index] = byte;
                assert!(!nibbles.admits(&payload));
                assert!(bytes.admits(&payload));
            }
        }
    }

    #[test]
    fn payload_shape_overlap_is_symmetric_and_pairwise_not_transitive() {
        use PayloadShape::{Bytes, Nibbles, Unknown};
        let four = NonZeroUsize::MIN.saturating_add(3);
        let shapes = [
            Unknown,
            Bytes(NonZeroUsize::MIN),
            Nibbles(NonZeroUsize::MIN),
            Bytes(four),
            Nibbles(four),
        ];
        let expected = [
            [true, true, true, true, true],
            [true, true, true, false, false],
            [true, true, true, false, false],
            [true, false, false, true, true],
            [true, false, false, true, true],
        ];
        for (left_index, left) in shapes.into_iter().enumerate() {
            for (right_index, right) in shapes.into_iter().enumerate() {
                assert_eq!(
                    left.overlaps(right),
                    expected[left_index][right_index],
                    "{left:?} against {right:?}"
                );
                assert_eq!(left.overlaps(right), right.overlaps(left));
            }
        }
        const UNKNOWN_OVERLAPS_BYTE: bool = Unknown.overlaps(Bytes(NonZeroUsize::MIN));
        assert_eq!(UNKNOWN_OVERLAPS_BYTE, shapes[0].overlaps(shapes[1]));
        assert!(Bytes(NonZeroUsize::MAX).overlaps(Nibbles(NonZeroUsize::MAX)));
        assert!(!Bytes(NonZeroUsize::MAX).overlaps(Bytes(NonZeroUsize::MIN)));
    }

    struct UndeclaredInquiry;

    impl Request for UndeclaredInquiry {
        type Class = crate::request::InquiryClass;
        const MAX_SIZE: usize = 4;
        fn policy(&self) -> crate::request::InquiryPolicy {
            crate::request::InquiryPolicy::new(
                crate::request::InquiryRetryClass::Never,
                crate::request::SubmissionClass::Normal,
            )
        }
        fn write_into(&self, target: crate::CameraId, buffer: &mut [u8]) -> crate::Result<usize> {
            let available = buffer.len();
            let frame = buffer
                .get_mut(..4)
                .ok_or_else(|| crate::Error::buffer_too_small(4, available))?;
            frame.copy_from_slice(&[target.to_address_byte(), 0x09, 0x04, 0xFF]);
            Ok(4)
        }
    }

    impl Inquiry for UndeclaredInquiry {
        type Response = usize;
        fn decoder(&self) -> ResponseDecoder<usize> {
            ResponseDecoder::from_fn(|payload| Ok(payload.len()))
        }
    }

    #[test]
    fn custom_inquiries_default_to_unknown_shape_with_and_without_a_profile() -> crate::Result<()> {
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()?;
        assert_eq!(UndeclaredInquiry.payload_shape(), PayloadShape::Unknown);
        assert_eq!(
            UndeclaredInquiry.payload_shape_for_profile(&profile),
            PayloadShape::Unknown
        );
        Ok(())
    }
}

#[cfg(all(test, feature = "serde"))]
mod inquiry_retry_tests {
    use super::InquiryRetryClass;

    #[test]
    fn inquiry_retry_names_refuse_command_families() -> Result<(), serde_json::Error> {
        for (class, name) in [
            (InquiryRetryClass::Never, "never"),
            (InquiryRetryClass::Inquiry, "inquiry"),
        ] {
            assert_eq!(serde_json::to_value(class)?, name);
            assert_eq!(
                serde_json::from_value::<InquiryRetryClass>(name.into())?,
                class
            );
        }
        for name in ["standard", "movement", "preset", "Never", "Inquiry"] {
            assert!(
                serde_json::from_value::<InquiryRetryClass>(name.into()).is_err(),
                "{name}"
            );
        }
        Ok(())
    }
}
