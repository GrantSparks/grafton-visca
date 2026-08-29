//! Typed request contracts and immutable request semantics.

use std::{fmt, num::NonZeroU8, sync::Arc};

use crate::{completion, request, CameraId, Error, Result};

/// Crate-only authority passed to the hidden applied-state request hook.
///
/// The containing module is private and the token is not constructible by a
/// downstream implementation of [`Request`]. This keeps the hook available to
/// typed built-ins while preserving the ordinary custom/raw `Request` default.
#[doc(hidden)]
pub(crate) struct AppliedStateAuthority(private::SealedToken);

mod private {
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct SealedToken;
}

impl AppliedStateAuthority {
    pub(crate) const fn new() -> Self {
        Self(private::SealedToken)
    }
}

/// Encoding error reported before a request is admitted to a camera owner.
pub type EncodeError = Error;

/// Deadline family selected by a request implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum TimeoutClass {
    /// A short command with no expected mechanical movement.
    Quick,
    /// A mechanical movement command.
    Movement,
    /// A preset operation, which commonly has a longer deadline.
    Preset,
    /// An unusually long-running command.
    LongRunning,
    /// A transport or network-management command.
    Network,
    /// An inquiry response.
    Inquiry,
}

/// Retry family selected by a request implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum RetryClass {
    /// Never replay the request automatically.
    Never,
    /// Conservative retries for an ordinary idempotent command.
    Standard,
    /// Inquiry-specific retries, including bounded response loss recovery.
    Inquiry,
    /// Movement-specific retries for transient camera state.
    Movement,
    /// Preset-specific retries for transient camera state.
    Preset,
}

/// Intrinsic scheduling class of one request.
///
/// Every request classifies itself through [`Request::control_class`]. The
/// urgent class is reserved for safety and protocol-control work and cannot be
/// selected or demoted by a submission override. Request implementations and
/// raw policies classify their own semantics; caller QoS uses
/// [`SubmissionClass`].
///
/// # Lane semantics
///
/// The owner keeps one ready queue per class per lane (commands and inquiries
/// are separate lanes). Dispatch scans the classes from [`Urgent`](Self::Urgent)
/// down to [`Background`](Self::Background) and takes the first request that is
/// eligible, preferring an inquiry over a command *within* the same class.
/// Within one class the order is the order of admission.
///
/// Three consequences follow, and they are the whole contract:
///
/// - The class decides only which **queued** request is written next. Once a
///   request has been written to the transport nothing reorders, interrupts, or
///   cancels it, so raising a class cannot preempt work already in flight; use
///   a typed stop or a cancellation for that.
/// - Classes are strictly ordered rather than weighted: while urgent work is
///   ready and eligible, nothing below it is dispatched.
///
/// The owner retains sole authority over the queues themselves: a class selects
/// a lane, never a position, a deadline, a retry budget, or a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum ControlClass {
    /// Opportunistic background work: telemetry polling, warm-up reads, and
    /// anything that should yield the transport to everything else.
    ///
    /// This is 1.x's `Priority::Low`.
    Background,
    /// Ordinary camera control traffic, and the class of every built-in
    /// inquiry.
    ///
    /// This is 1.x's `Priority::Normal`, and the class a request gets when it
    /// says nothing else.
    Normal,
    /// Direct user interaction: drives, absolute moves, preset recalls, and the
    /// other commands a person is waiting on.
    ///
    /// This is 1.x's `Priority::High`.
    User,
    /// Time-sensitive control work that must reach the camera ahead of queued
    /// traffic: the typed stops and socket cancellation.
    ///
    /// This is 1.x's `Priority::Critical`. It is an immutable safety floor:
    /// caller-selected submission QoS can neither create nor demote it.
    Urgent,
}

/// Caller-selected quality-of-service class for ordinary submissions.
///
/// This preserves 1.x's useful background/normal/interactive scheduling
/// controls without exposing the owner's safety lane. When a request's
/// intrinsic [`ControlClass`] is [`ControlClass::Urgent`], this value is
/// ignored and the request remains urgent. For every other request it selects
/// the ready lane used at admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl SubmissionClass {
    /// Returns the corresponding ordinary intrinsic class.
    #[must_use]
    pub const fn control_class(self) -> ControlClass {
        match self {
            Self::Background => ControlClass::Background,
            Self::Normal => ControlClass::Normal,
            Self::User => ControlClass::User,
        }
    }
}

impl From<SubmissionClass> for ControlClass {
    fn from(class: SubmissionClass) -> Self {
        class.control_class()
    }
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

/// One physical axis represented by [`AffectedAxes`].
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

/// Iterator over the individual axes in an [`AffectedAxes`] set.
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
    /// [`Self::MOVEMENT`] for the three mechanical movement axes that every
    /// profile with motion support declares.
    pub const ALL: Self = Self::from_nonzero_bits(Self::VALID_BITS);

    /// The three mechanical movement axes: pan/tilt, zoom, and focus.
    ///
    /// This is the "wait for everything that moves" selection used by
    /// [`crate::camera::IdleWait::default`] and the named wait presets.
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
            return Err(Error::InvalidRequest(
                "affected axes must be non-empty and contain only known axes".into(),
            ));
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

/// A response-correlation route chosen before inquiry admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InquiryRoute(u16);

impl InquiryRoute {
    /// The unclassified route used for raw custom inquiries.
    pub const RAW: Self = Self(0);

    /// Constructs an application-defined route.
    #[must_use]
    pub const fn custom(identifier: u16) -> Self {
        Self(identifier)
    }

    /// Constructs an application-defined route, rejecting the reserved raw
    /// FIFO identifier.
    pub fn try_custom(identifier: u16) -> Result<Self> {
        if identifier == 0 {
            return Err(Error::InvalidRequest(
                "inquiry route identifier 0 is reserved for InquiryRoute::RAW".into(),
            ));
        }
        Ok(Self(identifier))
    }

    /// Returns whether this route requests the explicit raw FIFO path.
    #[must_use]
    pub const fn is_raw(self) -> bool {
        self.0 == 0
    }

    /// Returns the stable route identifier.
    #[must_use]
    pub const fn identifier(self) -> u16 {
        self.0
    }
}

type SharedDecoder<R> = dyn Fn(&[u8]) -> Result<R> + Send + Sync + 'static;

enum DecoderInner<R> {
    Function(fn(&[u8]) -> Result<R>),
    Shared(Arc<SharedDecoder<R>>),
}

/// An owned typed inquiry decoder.
///
/// Function decoders are stored without allocation. Context-bearing decoders
/// use shared immutable storage so a prepared request remains movable.
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
            DecoderInner::Shared(decode) => decode(payload),
        }
    }
}

impl<R> Clone for ResponseDecoder<R> {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            DecoderInner::Function(decode) => DecoderInner::Function(*decode),
            DecoderInner::Shared(decode) => DecoderInner::Shared(Arc::clone(decode)),
        };
        Self { inner }
    }
}

impl<R> fmt::Debug for ResponseDecoder<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let storage = match self.inner {
            DecoderInner::Function(_) => "function",
            DecoderInner::Shared(_) => "shared-context",
        };
        formatter
            .debug_struct("ResponseDecoder")
            .field("storage", &storage)
            .finish_non_exhaustive()
    }
}

/// A request that can be fully encoded before owner admission.
pub trait Request: Send + Sync {
    /// Closed semantic request class.
    type Class: request::Class;

    /// Maximum encoded byte length for this request type.
    const MAX_SIZE: usize;
    /// Deadline family for this request.
    const TIMEOUT_CLASS: TimeoutClass;
    /// Retry family for this request.
    const RETRY_CLASS: RetryClass;
    /// Scheduling class for this request.
    const CONTROL_CLASS: ControlClass;

    /// Returns this value's timeout class.
    ///
    /// Built-in and ordinary downstream requests use the associated constant.
    /// Explicit raw requests override this accessor with the policy selected
    /// during construction; preparation therefore never has to infer policy
    /// from inert wire bytes.
    #[must_use]
    fn timeout_class(&self) -> TimeoutClass {
        Self::TIMEOUT_CLASS
    }

    /// Returns this value's retry class.
    ///
    /// The default keeps the zero-cost associated-constant path for fixed
    /// request types while allowing bounded raw values to carry an explicit
    /// per-instance class.
    #[must_use]
    fn retry_class(&self) -> RetryClass {
        Self::RETRY_CLASS
    }

    /// Returns this value's control class.
    ///
    /// This is the request's own classification. A caller may select ordinary
    /// submission QoS through [`SubmissionClass`], but an urgent intrinsic
    /// class is immutable. The owner lowers the resolved class to private ready
    /// queues; callers never select the safety lane or a queue position.
    #[must_use]
    fn control_class(&self) -> ControlClass {
        Self::CONTROL_CLASS
    }

    /// Returns the exact encoded byte length for this value.
    #[must_use]
    fn encoded_size(&self) -> usize {
        Self::MAX_SIZE
    }

    /// Writes the complete terminated VISCA request into `buffer`.
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError>;

    /// Validates value-specific capability and range facts before encoding.
    ///
    /// Custom requests with no profile-specific constraints may use the default.
    /// Built-ins and derives that carry constrained values override this hook.
    fn validate_for_profile(&self, _profile: &crate::ProfileSpec) -> Result<()> {
        Ok(())
    }

    /// Returns a crate-selected state effect for exact application.
    ///
    /// The authority token is intentionally hidden and non-forgeable outside
    /// this crate. Built-ins use it to carry their closed effect through the
    /// same generic preparation path as raw and downstream requests; those
    /// other request types retain the default `None`.
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn applied_state_projection(
        &self,
        _authority: AppliedStateAuthority,
    ) -> Option<crate::runtime::engine::AppliedStateProjection> {
        None
    }
}

/// Reads the hidden request effect hook at the preparation boundary.
pub(crate) fn applied_state_projection<R: Request + ?Sized>(
    request: &R,
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    request.applied_state_projection(AppliedStateAuthority::new())
}

/// A command with no physical-operation handle semantics.
pub trait PlainCommand: Request<Class = request::Plain> {}

impl<T> PlainCommand for T where T: Request<Class = request::Plain> + ?Sized {}

/// A typed inquiry request.
pub trait Inquiry: Request<Class = request::Inquiry> {
    /// Successfully decoded response value.
    type Response: Send + 'static;

    /// Returns the response-correlation route.
    fn route(&self) -> InquiryRoute;

    /// Returns the owned response decoder.
    fn decoder(&self) -> ResponseDecoder<Self::Response>;

    /// Returns a decoder owning any immutable runtime-profile context it needs.
    ///
    /// Most inquiries are profile-independent and use [`Self::decoder`].
    /// Profile-sensitive built-ins override this during pure preparation.
    #[doc(hidden)]
    fn decoder_for_profile(
        &self,
        _profile: &crate::ProfileSpec,
    ) -> ResponseDecoder<Self::Response> {
        self.decoder()
    }
}

/// A command with explicit physical-operation semantics.
pub trait OperationCommand<K>: Request<Class = request::Operation<K>>
where
    K: completion::Kind,
{
    /// Returns the non-empty set of axes affected by this operation.
    fn affected_axes(&self) -> AffectedAxes;
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
