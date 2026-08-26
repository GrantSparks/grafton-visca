//! Typed request contracts and immutable request semantics.

use std::{fmt, sync::Arc};

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

/// Scheduling policy class selected by a request implementation.
///
/// This is semantic input to profile lowering, not a caller-controlled engine
/// priority. The owner retains sole authority over its private ready queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum ControlClass {
    /// Opportunistic background work.
    Background,
    /// Ordinary camera control traffic.
    Normal,
    /// Direct user interaction.
    User,
    /// Time-sensitive control work.
    Urgent,
}

/// A non-empty set of physical camera axes affected by an operation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "u8", into = "u8"))]
#[cfg_attr(
    feature = "schemars",
    derive(schemars::JsonSchema),
    schemars(with = "u8")
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct AffectedAxes(u8);

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

    /// The pan and tilt axes.
    pub const PAN_TILT: Self = Self(Self::PAN_TILT_BIT);
    /// The zoom axis.
    pub const ZOOM: Self = Self(Self::ZOOM_BIT);
    /// The focus axis.
    pub const FOCUS: Self = Self(Self::FOCUS_BIT);
    /// The iris/aperture axis.
    pub const IRIS: Self = Self(Self::IRIS_BIT);
    /// The neutral-density filter axis.
    pub const ND_FILTER: Self = Self(Self::ND_FILTER_BIT);

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
        Ok(Self(bits))
    }

    /// Returns the stable bit representation.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Returns whether no axis is selected.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether this set contains every axis in `other`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns the number of selected physical axes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Returns whether exactly one physical axis is selected.
    #[must_use]
    pub const fn is_single(self) -> bool {
        self.0.count_ones() == 1
    }

    /// Returns an iterator over selected axes in canonical bit order.
    #[must_use]
    pub const fn iter(self) -> AffectedAxisIter {
        AffectedAxisIter { bits: self.0 }
    }

    /// Combines two non-empty sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
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
    /// The owner still lowers this semantic class to its private scheduler
    /// priority; callers cannot provide a priority or change it at admission.
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
