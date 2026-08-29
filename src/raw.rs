//! Explicit, typed escape hatches for custom VISCA wire messages.
//!
//! The types in this module are the only raw-request surface in the typed
//! owner API. They are deliberately separate from the request-class markers
//! in [`crate::request`] and from the low-level [`crate::command`] extension
//! surface. A raw value is still admitted only through the async camera's
//! `execute`, `inquire`, or `submit` methods (and their blocking projections);
//! it cannot select a lifecycle ID, target, or completion kind at submission
//! time. Its [`crate::ControlClass`] is chosen in its [`crate::raw::Policy`]
//! from the ordinary lanes (`Background`, `Normal`, `User`); the urgent safety
//! lane is reserved for the owner's stops and protocol cancellation and is not
//! selectable here. The camera handle's `*_with_submission_class` methods and
//! `set_submission_class` default apply to it on the same terms.
//!
//! Raw constructors validate and own the complete VISCA frame. The first byte
//! must be a valid VISCA camera address and the final byte must be `0xff`.
//! Owner-only wire primitives are refused at construction: a socket cancel
//! (`8x 2y ff`) or a per-camera interface clear (`8x 01 00 01 ff`) would let a
//! target-scoped `execute` cancel another operation's socket or reset the
//! shared command buffer, so both are rejected. The broadcast address-set form
//! is refused by the target check during preparation.
//! The camera view supplies the authoritative target during preparation, so a
//! frame addressed to another target is rejected rather than being silently
//! rewritten or inspected to infer target identity. Frames up to
//! [`crate::raw::MAX_BYTES`] are accepted. The first `INLINE_BYTES` fit in inline storage;
//! larger custom frames use one bounded heap-backed `SmallVec` allocation and
//! are copied once into the prepared message. Standard built-in request
//! encoding remains inline and allocation-free.
//!
//! Raw policy is explicit and owned by each value. Timeout, retry, and control
//! classes are required constructor arguments (or can be supplied as a
//! [`crate::raw::Policy`] or [`crate::raw::Spec`]); no default, optional metadata, byte-based
//! inference, or caller-selected operation class exists.
//!
//! A raw command also declares its [`crate::raw::RawReplyShape`]: the default
//! [`AckThenCompletion`](crate::raw::RawReplyShape::AckThenCompletion) is the
//! ordinary ACK-then-completion protocol, while
//! [`CompletionOnly`](crate::raw::RawReplyShape::CompletionOnly) and
//! [`NoReply`](crate::raw::RawReplyShape::NoReply) describe legitimate vendor
//! frames that answer with a completion and no ACK, or with nothing at all. The
//! shape is set on the [`crate::raw::Policy`] with
//! [`Policy::with_reply_shape`](crate::raw::Policy::with_reply_shape) and lowered into the
//! owner's correlation so a completion-only frame is not held waiting for an ACK
//! it will never receive. It is a command axis only: [`Inquiry`] rejects any
//! non-default shape. Reply shape never re-admits the owner-only wire primitives
//! rejected above — a socket cancel or interface clear stays refused at
//! construction whatever shape is declared, because those act on another
//! operation's socket or the shared command buffer.

use std::{borrow::Cow, fmt};

use smallvec::SmallVec;

/// Policy and validation types used by the raw constructors.
pub use crate::{
    AffectedAxes, ControlClass, InquiryRoute, ResponseDecoder, RetryClass, TimeoutClass,
};

use crate::{
    completion, request, CameraId, Error, Inquiry as InquiryRequest, OperationCommand, Request,
    Result,
};

/// Number of raw frame bytes retained inline without a heap allocation.
///
/// This matches the protocol engine's standard inline message capacity.
pub(crate) const INLINE_BYTES: usize = 32;

/// Maximum raw frame length accepted by the typed escape hatch.
///
/// The bound is shared with the protocol engine. It is intentionally larger
/// than ordinary VISCA messages so vendor extensions remain possible while
/// still making construction and admission memory-bounded.
pub const MAX_BYTES: usize = 1_024;

const VISCA_TERMINATOR: u8 = 0xff;

/// The reply protocol a raw command declares the camera will use.
///
/// Every VISCA command is, by default, acknowledged, assigned a socket, then
/// completed. Some legitimate vendor extensions instead answer with a
/// completion and no acknowledgement, or expect no reply at all. A raw caller
/// declares which shape applies so the owner's correlation and quarantine honor
/// it; the engine never infers the shape from the wire bytes.
///
/// This axis is for *legitimate* custom completion-only or fire-and-forget
/// commands. It does not, and must not, re-admit the owner-only wire primitives
/// (a socket cancel or a per-camera interface clear) that [`crate::raw`]
/// rejects at construction regardless of reply shape: those remain owner-only
/// because they act on another operation's socket or the shared command buffer.
///
/// The shape is meaningful only for commands
/// ([`Plain`], [`Targeted`], [`AppliedOnly`]). An [`Inquiry`] always awaits its
/// reply, so its constructors reject any non-default reply shape rather than
/// silently ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RawReplyShape {
    /// The camera acknowledges the command, the owner assigns it a socket, and
    /// the command completes on the completion frame. This is the default and
    /// the behavior of every built-in command; existing raw callers are
    /// unchanged.
    #[default]
    AckThenCompletion,
    /// The camera replies with a completion (or terminal) frame but no
    /// acknowledgement. The command is never assigned a socket and never enters
    /// the unacknowledged-command gate expecting an ACK, so a missing ACK cannot
    /// fail or poison it. It terminates on the completion frame or, failing
    /// that, on a bounded completion deadline.
    CompletionOnly,
    /// The command expects nothing back. It terminates successfully the instant
    /// its transport write succeeds; the owner never holds it waiting for a
    /// frame, and any later reply the camera nonetheless sends is ignored.
    NoReply,
}

/// Explicit timeout, retry, and scheduler-control classes for one raw value.
///
/// The owner lowers these semantic classes to its private runtime policy. The
/// public value never exposes a queue position or a lifecycle identifier.
///
/// The [`ControlClass`] here is the raw request's *own* classification, exactly
/// as an ordinary built-in's associated constant is. It may name the
/// `Background`, `Normal`, or `User` lane; it may not name
/// [`ControlClass::Urgent`], which [`Policy::new`] rejects, because the urgent
/// lane is the owner's stop and protocol-cancel reserve. A camera handle's
/// default class and a per-submission class replace ordinary classifications,
/// but neither can replace or demote an urgent request.
///
/// The [`RawReplyShape`] here is the command's declared reply protocol. It
/// defaults to [`RawReplyShape::AckThenCompletion`], so a policy built by
/// [`Policy::new`] is unchanged for existing callers; a completion-only or
/// no-reply command sets it with [`Policy::with_reply_shape`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Policy {
    timeout: TimeoutClass,
    retry: RetryClass,
    control: ControlClass,
    reply_shape: RawReplyShape,
}

/// The caller-facing policy specification used to build a raw request.
///
/// `Spec` is intentionally distinct from [`Policy`]: a specification is the
/// value supplied at a construction boundary, while `Policy` is the compact
/// policy retained by a constructed request. Both carry the same three
/// semantic classes and neither exposes a queue position or lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spec {
    policy: Policy,
}

/// The error every raw policy/spec constructor returns for an urgent request.
///
/// Shared so [`Policy::new`] and [`Spec::new`] cannot drift in wording. It is a
/// free `const fn` returning the [`Error`] by value: a `const fn` cannot drop an
/// intermediate `Result`, so each constructor keeps its own one-line `matches!`
/// guard (pinned together by `raw_policy_rejects_urgent_control_class`) rather
/// than delegating through a fallible call.
const fn urgent_control_class_error() -> Error {
    Error::InvalidRequest(Cow::Borrowed(
        "raw policy cannot select ControlClass::Urgent; the urgent lane is reserved for owner-issued stops and protocol cancellation (issue pan_tilt().stop() for preemption)",
    ))
}

impl Spec {
    /// Creates an explicit raw-request specification.
    ///
    /// Rejects [`ControlClass::Urgent`] on the same grounds as [`Policy::new`].
    pub const fn new(
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        if matches!(control, ControlClass::Urgent) {
            return Err(urgent_control_class_error());
        }
        Ok(Self {
            policy: Policy {
                timeout,
                retry,
                control,
                reply_shape: RawReplyShape::AckThenCompletion,
            },
        })
    }

    /// Returns this specification with the given reply shape.
    ///
    /// The default is [`RawReplyShape::AckThenCompletion`]; see
    /// [`Policy::with_reply_shape`].
    #[must_use]
    pub const fn with_reply_shape(self, reply_shape: RawReplyShape) -> Self {
        Self {
            policy: self.policy.with_reply_shape(reply_shape),
        }
    }

    /// Returns the declared reply shape.
    #[must_use]
    pub const fn reply_shape(self) -> RawReplyShape {
        self.policy.reply_shape()
    }

    /// Returns the normalized policy represented by this specification.
    #[must_use]
    pub const fn policy(self) -> Policy {
        self.policy
    }
}

impl From<Policy> for Spec {
    fn from(policy: Policy) -> Self {
        Self { policy }
    }
}

impl From<Spec> for Policy {
    fn from(spec: Spec) -> Self {
        spec.policy
    }
}

impl Policy {
    /// Creates an explicit raw-request policy.
    ///
    /// Returns [`Error::InvalidRequest`] when `control` is
    /// [`ControlClass::Urgent`]. The urgent lane is FIFO within its class and
    /// bypasses admission backlog so owner-issued stops and protocol
    /// cancellation always preempt; letting ordinary raw traffic enter it would
    /// dilute that reserve and delay a genuine `PanTiltStop`. A raw caller who
    /// needs preemption issues the typed stop (`pan_tilt().stop()`, and the
    /// like), which the crate classifies urgent on the caller's behalf.
    pub const fn new(
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        if matches!(control, ControlClass::Urgent) {
            return Err(urgent_control_class_error());
        }
        Ok(Self {
            timeout,
            retry,
            control,
            reply_shape: RawReplyShape::AckThenCompletion,
        })
    }

    /// Returns this policy with the given reply shape.
    ///
    /// [`Policy::new`] defaults the reply shape to
    /// [`RawReplyShape::AckThenCompletion`], keeping existing raw code
    /// unchanged. A completion-only or no-reply command sets its shape here:
    ///
    /// ```
    /// use grafton_visca::raw::{Policy, RawReplyShape};
    /// use grafton_visca::{ControlClass, RetryClass, TimeoutClass};
    ///
    /// let policy = Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)?
    ///     .with_reply_shape(RawReplyShape::CompletionOnly);
    /// assert_eq!(policy.reply_shape(), RawReplyShape::CompletionOnly);
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    #[must_use]
    pub const fn with_reply_shape(self, reply_shape: RawReplyShape) -> Self {
        Self {
            timeout: self.timeout,
            retry: self.retry,
            control: self.control,
            reply_shape,
        }
    }

    /// Returns the selected timeout class.
    #[must_use]
    pub const fn timeout_class(self) -> TimeoutClass {
        self.timeout
    }

    /// Returns the declared reply shape.
    #[must_use]
    pub const fn reply_shape(self) -> RawReplyShape {
        self.reply_shape
    }

    /// Returns the selected retry class.
    #[must_use]
    pub const fn retry_class(self) -> RetryClass {
        self.retry
    }

    /// Returns the selected scheduler-control class.
    #[must_use]
    pub const fn control_class(self) -> ControlClass {
        self.control
    }
}

/// A bounded owned raw VISCA frame.
///
/// This helper is intentionally private to the public raw request types. It
/// keeps the four request classes structurally distinct, so a plain frame,
/// inquiry, targeted operation, and applied-only operation cannot be
/// interchanged by changing a runtime enum.
#[derive(Clone, PartialEq, Eq)]
struct Wire(SmallVec<[u8; INLINE_BYTES]>);

impl Wire {
    fn new(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let bytes = bytes.as_ref();
        validate_wire(bytes)?;
        Ok(Self(SmallVec::from_slice(bytes)))
    }

    #[must_use]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn write_into(&self, buffer: &mut [u8]) -> Result<usize> {
        if buffer.len() < self.len() {
            return Err(Error::BufferTooSmall {
                required: self.len(),
                actual: buffer.len(),
            });
        }
        buffer[..self.len()].copy_from_slice(self.as_bytes());
        Ok(self.len())
    }
}

impl fmt::Debug for Wire {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Wire")
            .field(&self.as_bytes())
            .finish()
    }
}

fn validate_wire(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::ResponseTooLarge {
            max_size: MAX_BYTES,
        });
    }
    if bytes.len() < 2 {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must contain an address and terminator".into(),
        ));
    }
    if !(0x81..=0x88).contains(&bytes[0]) {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must begin with an address byte in 0x81..=0x88".into(),
        ));
    }
    if bytes.last().copied() != Some(VISCA_TERMINATOR) {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must end with the 0xff terminator".into(),
        ));
    }
    reject_owner_only_primitive(bytes)?;
    Ok(())
}

/// Rejects the owner-only wire primitives a target-scoped `execute` must never
/// be able to emit.
///
/// A socket cancel (`8x 2y ff`) could cancel an *unrelated* operation's socket,
/// and a per-camera interface clear (`8x 01 00 01 ff`) resets the shared
/// command buffer; both are legitimate only from the owner, which correlates
/// the exact camera-assigned socket first. Address assignment (`88 30 0y ff`)
/// is a broadcast primitive whose `0x88` address is refused by the target
/// check during preparation, so it is not re-checked here.
///
/// The socket-cancel guard is deliberately length-bounded to the exact 3-byte
/// cancel frame: a longer frame whose command byte falls in `0x20..=0x2f` (for
/// example USB audio, `81 2a 02 a0 04 02 ff`) is an ordinary command and stays
/// admissible.
fn reject_owner_only_primitive(bytes: &[u8]) -> Result<()> {
    // Caller guarantees `bytes.len() >= 2` and `bytes.last() == Some(0xff)`.
    if bytes.len() == 3 && (bytes[1] & 0xf0) == 0x20 {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must not be a socket cancel (8x 2y ff); the owner cancels a correlated operation through its handle, not through execute()".into(),
        ));
    }
    if bytes.len() == 5 && bytes[1] == 0x01 && bytes[2] == 0x00 && bytes[3] == 0x01 {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must not be an interface clear (8x 01 00 01 ff); it resets the shared command buffer and is owner-only".into(),
        ));
    }
    Ok(())
}

/// A raw plain command with no operation lifecycle handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plain {
    wire: Wire,
    policy: Policy,
}

impl Plain {
    /// Creates a bounded raw plain command with explicit protocol policy.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, Policy::new(timeout, retry, control)?)
    }

    /// Creates a bounded raw plain command from an explicit policy value.
    pub fn with_policy<P>(bytes: impl AsRef<[u8]>, policy: P) -> Result<Self>
    where
        P: Into<Policy>,
    {
        Ok(Self {
            wire: Wire::new(bytes)?,
            policy: policy.into(),
        })
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this value's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this value's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this value's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this value's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    /// Returns this value's declared reply shape.
    #[must_use]
    pub const fn reply_shape(&self) -> RawReplyShape {
        self.policy.reply_shape()
    }
}

impl Request for Plain {
    type Class = request::Plain;

    // The trait retains associated constants for fixed built-in/downstream
    // request types. Raw values override the instance accessors below because
    // their policy is intentionally selected at construction.
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn reply_shape(&self) -> RawReplyShape {
        self.policy.reply_shape()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }
}

/// A raw inquiry with an explicit response route and typed decoder.
pub struct Inquiry<R> {
    wire: Wire,
    route: InquiryRoute,
    decoder: ResponseDecoder<R>,
    policy: Policy,
}

impl<R> Inquiry<R> {
    /// Creates a bounded raw inquiry with explicit route, decoder, and policy.
    ///
    /// `ResponseDecoder::with_context` can be used when the decoder needs
    /// immutable per-instance context; the context is moved into this inquiry
    /// and remains outside the protocol engine.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        decoder: ResponseDecoder<R>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, route, decoder, Policy::new(timeout, retry, control)?)
    }

    /// Creates a raw inquiry from explicit route, decoder, and policy values.
    ///
    /// The policy's reply shape must be the default
    /// [`RawReplyShape::AckThenCompletion`]: an inquiry always awaits its reply,
    /// so a completion-only or no-reply shape is meaningless for it and is
    /// rejected here rather than silently ignored.
    pub fn with_policy<P>(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        decoder: ResponseDecoder<R>,
        policy: P,
    ) -> Result<Self>
    where
        P: Into<Policy>,
    {
        let policy = policy.into();
        validate_inquiry_reply_shape(policy.reply_shape())?;
        validate_route(route)?;
        Ok(Self {
            wire: Wire::new(bytes)?,
            route,
            decoder,
            policy,
        })
    }

    /// Creates a raw inquiry using an allocation-free function decoder.
    pub fn from_fn(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        decoder: fn(&[u8]) -> Result<R>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::new(
            bytes,
            route,
            ResponseDecoder::from_fn(decoder),
            timeout,
            retry,
            control,
        )
    }

    /// Creates a raw inquiry whose decoder owns immutable context.
    pub fn with_context<C>(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        context: C,
        decoder: fn(&C, &[u8]) -> Result<R>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self>
    where
        C: Send + Sync + 'static,
        R: 'static,
    {
        Self::new(
            bytes,
            route,
            ResponseDecoder::with_context(context, decoder),
            timeout,
            retry,
            control,
        )
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this inquiry's explicit response-correlation route.
    #[must_use]
    pub const fn route(&self) -> InquiryRoute {
        self.route
    }

    /// Returns this inquiry's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this inquiry's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this inquiry's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this inquiry's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }
}

impl<R> fmt::Debug for Inquiry<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Inquiry")
            .field("wire", &self.wire)
            .field("route", &self.route)
            .field("decoder", &self.decoder)
            .field("policy", &self.policy)
            .finish()
    }
}

impl<R> Clone for Inquiry<R> {
    fn clone(&self) -> Self {
        Self {
            wire: self.wire.clone(),
            route: self.route,
            decoder: self.decoder.clone(),
            policy: self.policy,
        }
    }
}

impl<R> Request for Inquiry<R>
where
    R: Send + 'static,
{
    type Class = request::Inquiry;
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
    const RETRY_CLASS: RetryClass = RetryClass::Inquiry;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }
}

impl<R> InquiryRequest for Inquiry<R>
where
    R: Send + 'static,
{
    type Response = R;

    fn route(&self) -> InquiryRoute {
        self.route
    }

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        self.decoder.clone()
    }
}

/// A raw operation with a meaningful physical target and settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targeted {
    wire: Wire,
    axes: AffectedAxes,
    policy: Policy,
}

impl Targeted {
    /// Creates a bounded raw targeted operation with explicit non-empty axes
    /// and protocol policy.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        axes: AffectedAxes,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, axes, Policy::new(timeout, retry, control)?)
    }

    /// Creates a raw targeted operation from an explicit policy value.
    pub fn with_policy<P>(bytes: impl AsRef<[u8]>, axes: AffectedAxes, policy: P) -> Result<Self>
    where
        P: Into<Policy>,
    {
        Ok(Self {
            wire: Wire::new(bytes)?,
            axes,
            policy: policy.into(),
        })
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this operation's explicit affected axes.
    #[must_use]
    pub const fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }

    /// Returns this operation's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this operation's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this operation's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this operation's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    /// Returns this operation's declared reply shape.
    #[must_use]
    pub const fn reply_shape(&self) -> RawReplyShape {
        self.policy.reply_shape()
    }
}

impl Request for Targeted {
    type Class = request::Operation<completion::Targeted>;
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
    const RETRY_CLASS: RetryClass = RetryClass::Movement;
    const CONTROL_CLASS: ControlClass = ControlClass::User;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn reply_shape(&self) -> RawReplyShape {
        self.policy.reply_shape()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }

    fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<()> {
        if profile.supports_axes(self.axes) {
            Ok(())
        } else {
            Err(Error::FeatureNotSupported {
                feature: "raw targeted operation axes",
            })
        }
    }
}

impl OperationCommand<completion::Targeted> for Targeted {
    fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }
}

/// A raw operation whose terminal protocol application is its only lifecycle
/// observation. It intentionally has no `settled` method or target inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedOnly {
    wire: Wire,
    axes: AffectedAxes,
    policy: Policy,
}

impl AppliedOnly {
    /// Creates a bounded raw applied-only operation with explicit non-empty
    /// axes and protocol policy.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        axes: AffectedAxes,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, axes, Policy::new(timeout, retry, control)?)
    }

    /// Creates a raw applied-only operation from an explicit policy value.
    pub fn with_policy<P>(bytes: impl AsRef<[u8]>, axes: AffectedAxes, policy: P) -> Result<Self>
    where
        P: Into<Policy>,
    {
        Ok(Self {
            wire: Wire::new(bytes)?,
            axes,
            policy: policy.into(),
        })
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this operation's explicit affected axes.
    #[must_use]
    pub const fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }

    /// Returns this operation's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this operation's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this operation's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this operation's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    /// Returns this operation's declared reply shape.
    #[must_use]
    pub const fn reply_shape(&self) -> RawReplyShape {
        self.policy.reply_shape()
    }
}

impl Request for AppliedOnly {
    type Class = request::Operation<completion::AppliedOnly>;
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::User;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn reply_shape(&self) -> RawReplyShape {
        self.policy.reply_shape()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }

    fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<()> {
        if profile.supports_axes(self.axes) {
            Ok(())
        } else {
            Err(Error::FeatureNotSupported {
                feature: "raw applied-only operation axes",
            })
        }
    }
}

impl OperationCommand<completion::AppliedOnly> for AppliedOnly {
    fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }
}

fn validate_inquiry_reply_shape(reply_shape: RawReplyShape) -> Result<()> {
    // An inquiry's lifecycle is fixed: it awaits its reply and never an ACK or a
    // command completion, so the completion-only and no-reply shapes cannot
    // describe it. Reject them at construction so a caller cannot set a shape the
    // owner would silently ignore.
    if matches!(reply_shape, RawReplyShape::AckThenCompletion) {
        Ok(())
    } else {
        Err(Error::InvalidRequest(Cow::Borrowed(
            "raw inquiry reply shape must be RawReplyShape::AckThenCompletion; an inquiry always awaits its reply",
        )))
    }
}

fn validate_route(route: InquiryRoute) -> Result<()> {
    // `InquiryRoute::RAW` (identifier zero) is the explicit raw FIFO route;
    // non-zero values are application/built-in route identifiers. Rebuilding
    // non-zero values through the checked constructor keeps the reserved
    // identifier rule in one place and makes the validation visible at the
    // raw boundary.
    if route.is_raw() {
        Ok(())
    } else {
        InquiryRoute::try_custom(route.identifier()).map(|_| ())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{completion, Inquiry as InquiryTrait, OperationCommand, PlainCommand, Request};

    fn assert_plain<T: PlainCommand>() {}
    fn assert_inquiry<T: InquiryTrait>() {}
    fn assert_targeted<T: OperationCommand<completion::Targeted>>() {}
    fn assert_applied<T: OperationCommand<completion::AppliedOnly>>() {}

    fn decode_first(payload: &[u8]) -> Result<u8> {
        payload
            .first()
            .copied()
            .ok_or_else(|| Error::InvalidRequest("empty test payload".into()))
    }

    #[test]
    fn classes_are_structurally_distinct_and_encoding_is_exact() {
        assert_plain::<Plain>();
        assert_inquiry::<Inquiry<u8>>();
        assert_targeted::<Targeted>();
        assert_applied::<AppliedOnly>();

        let command = Plain::new(
            [0x81, 0x01, 0x02, 0xff],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid raw command");
        assert_eq!(command.bytes(), &[0x81, 0x01, 0x02, 0xff]);
        assert_eq!(command.encoded_size(), 4);
        assert_eq!(command.timeout_class(), TimeoutClass::Quick);
        let mut encoded = [0_u8; 4];
        assert_eq!(
            command
                .write_into(CameraId::CAMERA_1, &mut encoded)
                .expect("exact encoding"),
            4
        );
        assert_eq!(encoded, [0x81, 0x01, 0x02, 0xff]);

        let inquiry = Inquiry::new(
            [0x81, 0x09, 0x04, 0xff],
            InquiryRoute::try_custom(7).expect("non-zero route"),
            ResponseDecoder::from_fn(decode_first),
            TimeoutClass::Inquiry,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid raw inquiry");
        assert_eq!(inquiry.route(), InquiryRoute::custom(7));
        assert_eq!(inquiry.decoder().decode(&[0x42]).expect("decode"), 0x42);

        let context_inquiry = Inquiry::<u8>::with_context(
            [0x81, 0x09, 0x05, 0xff],
            InquiryRoute::RAW,
            0x10_u8,
            |context, payload| {
                payload
                    .first()
                    .map(|value| value.saturating_add(*context))
                    .ok_or_else(|| Error::InvalidRequest("empty test payload".into()))
            },
            TimeoutClass::Inquiry,
            RetryClass::Inquiry,
            ControlClass::Normal,
        )
        .expect("context decoder");
        assert_eq!(
            context_inquiry
                .decoder()
                .decode(&[0x02])
                .expect("context decode"),
            0x12
        );

        let axes = AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM);
        let targeted = Targeted::new(
            [0x81, 0x01, 0x06, 0xff],
            axes,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        )
        .expect("valid targeted operation");
        assert_eq!(targeted.affected_axes(), axes);

        let applied = AppliedOnly::with_policy(
            [0x81, 0x01, 0x07, 0xff],
            AffectedAxes::ZOOM,
            Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::User)
                .expect("valid raw policy"),
        )
        .expect("valid applied-only operation");
        assert_eq!(applied.affected_axes(), AffectedAxes::ZOOM);
        assert_eq!(applied.control_class(), ControlClass::User);
    }

    #[test]
    fn construction_rejects_invalid_wire_axes_and_route_specs() {
        assert!(Plain::new(
            [],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(Plain::new(
            [0x80, 0x01, 0xff],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(Plain::new(
            [0x81, 0x01, 0x00],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(Plain::new(
            vec![0x81; MAX_BYTES + 1],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(InquiryRoute::try_custom(0).is_err());
        assert!(AffectedAxes::from_bits(0).is_err());
    }

    /// An otherwise-valid frame one byte over the bound is rejected *by the
    /// size branch*.
    ///
    /// The fixture carries a legal address byte and a legal terminator, so no
    /// other rule in `validate_wire` can satisfy the assertion on its behalf;
    /// deleting the `MAX_BYTES` branch fails this test.
    #[test]
    fn oversize_but_otherwise_valid_frame_is_rejected_by_the_size_bound() {
        fn frame(len: usize) -> Vec<u8> {
            let mut bytes = vec![0x00; len];
            bytes[0] = 0x81;
            bytes[len - 1] = VISCA_TERMINATOR;
            bytes
        }

        let oversize = frame(MAX_BYTES + 1);
        assert_eq!(oversize[0], 0x81, "fixture must carry a legal address byte");
        assert_eq!(
            oversize.last().copied(),
            Some(VISCA_TERMINATOR),
            "fixture must carry a legal terminator"
        );

        let error = Plain::new(
            oversize,
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect_err("a frame longer than MAX_BYTES must be rejected");
        assert!(
            matches!(
                error,
                Error::ResponseTooLarge {
                    max_size: MAX_BYTES
                }
            ),
            "expected the frame-size bound to reject the frame, got {error:?}"
        );

        // The same fixture at exactly the bound is accepted, so the assertion
        // above cannot pass by way of an unrelated tightening.
        let at_bound = frame(MAX_BYTES);
        let accepted = Plain::new(
            at_bound.clone(),
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("a frame of exactly MAX_BYTES must be accepted");
        assert_eq!(accepted.bytes(), at_bound.as_slice());

        // Every constructor shares `validate_wire`, so the bound holds for the
        // axis- and route-carrying values too.
        assert!(matches!(
            Targeted::new(
                frame(MAX_BYTES + 1),
                AffectedAxes::PAN_TILT,
                TimeoutClass::Movement,
                RetryClass::Movement,
                ControlClass::User,
            )
            .expect_err("oversize targeted frame"),
            Error::ResponseTooLarge {
                max_size: MAX_BYTES
            }
        ));
        assert!(matches!(
            AppliedOnly::new(
                frame(MAX_BYTES + 1),
                AffectedAxes::ZOOM,
                TimeoutClass::Movement,
                RetryClass::Movement,
                ControlClass::User,
            )
            .expect_err("oversize applied-only frame"),
            Error::ResponseTooLarge {
                max_size: MAX_BYTES
            }
        ));
    }

    /// Raw operation constructors accept the structurally non-empty axis type.
    #[test]
    fn axis_carrying_constructors_preserve_the_explicit_axis_set() {
        const VALID: [u8; 4] = [0x81, 0x01, 0x06, 0xff];
        let targeted = Targeted::new(
            VALID,
            AffectedAxes::PAN_TILT,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        )
        .expect("non-empty targeted axes");
        assert_eq!(targeted.affected_axes(), AffectedAxes::PAN_TILT);

        let applied = AppliedOnly::with_policy(
            VALID,
            AffectedAxes::ZOOM,
            Policy::new(
                TimeoutClass::Movement,
                RetryClass::Movement,
                ControlClass::User,
            )
            .expect("valid raw policy"),
        )
        .expect("non-empty applied-only axes");
        assert_eq!(applied.affected_axes(), AffectedAxes::ZOOM);
    }

    #[test]
    fn owner_target_is_not_inferred_or_rewritten_from_raw_bytes() {
        let command = Plain::new(
            [0x82, 0x01, 0xff],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid frame for another explicit target");
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("profile");
        let error = crate::prepared::prepare_command(
            &command,
            CameraId::CAMERA_1,
            &profile,
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .expect_err("the owner target must reject a mismatched wire address");
        assert!(matches!(error, Error::InvalidRequest(_)));
        assert_eq!(command.bytes(), &[0x82, 0x01, 0xff]);
    }

    /// #679: raw policy cannot manufacture the urgent safety lane.
    ///
    /// `Policy::new`, `Spec::new`, and every raw constructor that funnels
    /// through them reject [`ControlClass::Urgent`], so ordinary raw traffic can
    /// never enter the FIFO safety lane and dilute a genuine stop. The three
    /// ordinary lanes stay admissible, so the guard rejects only urgent and
    /// nothing wider.
    #[test]
    fn raw_policy_rejects_urgent_control_class() {
        const FRAME: [u8; 4] = [0x81, 0x01, 0x02, 0xff];

        assert!(
            matches!(
                Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Urgent),
                Err(Error::InvalidRequest(_))
            ),
            "Policy::new must reject the urgent safety lane"
        );
        assert!(
            matches!(
                Spec::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Urgent),
                Err(Error::InvalidRequest(_))
            ),
            "Spec::new must reject the urgent safety lane"
        );

        // Every raw request constructor funnels its class through `Policy::new`,
        // so the rejection reaches each of the four request classes.
        assert!(Plain::new(
            FRAME,
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Urgent
        )
        .is_err());
        assert!(Inquiry::from_fn(
            [0x81, 0x09, 0x04, 0xff],
            InquiryRoute::RAW,
            decode_first,
            TimeoutClass::Inquiry,
            RetryClass::Never,
            ControlClass::Urgent,
        )
        .is_err());
        assert!(Targeted::new(
            [0x81, 0x01, 0x06, 0xff],
            AffectedAxes::PAN_TILT,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::Urgent,
        )
        .is_err());
        assert!(AppliedOnly::new(
            [0x81, 0x01, 0x07, 0xff],
            AffectedAxes::ZOOM,
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Urgent,
        )
        .is_err());

        // The three ordinary lanes remain selectable, so the guard is exactly
        // the urgent lane and not a blanket refusal.
        for control in [
            ControlClass::Background,
            ControlClass::Normal,
            ControlClass::User,
        ] {
            let policy = Policy::new(TimeoutClass::Quick, RetryClass::Never, control)
                .expect("ordinary lanes are admissible");
            assert_eq!(policy.control_class(), control);
            assert!(Plain::new(FRAME, TimeoutClass::Quick, RetryClass::Never, control).is_ok());
        }
    }

    /// #678: the raw hatch refuses owner-only wire primitives.
    ///
    /// A socket cancel (`8x 2y ff`) or a per-camera interface clear
    /// (`8x 01 00 01 ff`) submitted through `execute()` could cancel an
    /// unrelated operation's socket or reset the shared command buffer. Because
    /// every raw constructor validates through `validate_wire`, these frames
    /// cannot be built at all, so they can never reach the owner or hijack a
    /// victim operation — the reported repro is prevented at construction.
    #[test]
    fn construction_rejects_owner_only_wire_primitives() {
        // Socket cancel, every socket nibble, for every camera address.
        for address in 0x81_u8..=0x88 {
            for socket in 0x0_u8..=0xf {
                let cancel: [u8; 3] = [address, 0x20 | socket, 0xff];
                assert!(
                    Plain::new(
                        cancel,
                        TimeoutClass::Quick,
                        RetryClass::Never,
                        ControlClass::Normal
                    )
                    .is_err(),
                    "socket cancel {cancel:x?} must be rejected"
                );
            }
        }

        // Per-camera and broadcast interface clear.
        for address in [0x81_u8, 0x82, 0x88] {
            let clear: [u8; 5] = [address, 0x01, 0x00, 0x01, 0xff];
            assert!(
                Plain::new(
                    clear,
                    TimeoutClass::Quick,
                    RetryClass::Never,
                    ControlClass::Normal
                )
                .is_err(),
                "interface clear {clear:x?} must be rejected"
            );
        }

        // The guard is shared by every raw request class, not just `Plain`.
        assert!(Inquiry::from_fn(
            [0x81, 0x21, 0xff],
            InquiryRoute::RAW,
            decode_first,
            TimeoutClass::Inquiry,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(Targeted::new(
            [0x81, 0x21, 0xff],
            AffectedAxes::PAN_TILT,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        )
        .is_err());
        assert!(AppliedOnly::new(
            [0x81, 0x01, 0x00, 0x01, 0xff],
            AffectedAxes::ZOOM,
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::User,
        )
        .is_err());

        // Legitimate custom frames stay admissible — including the two shapes
        // the guard sits closest to, proving it rejects exactly the owner-only
        // primitives and nothing wider:
        //   * a 3-byte non-cancel command (`8x 01 ff`) is not caught by the
        //     length-3 cancel guard, which is gated on the `0x2y` command byte;
        //   * USB audio (`81 2a 02 a0 04 02 ff`) carries command byte `0x2a`
        //     inside the cancel nibble range but is not the 3-byte cancel frame;
        //   * `8x 01 00 02 ff` shares the interface-clear prefix but is not the
        //     exact `01 00 01` clear payload.
        for frame in [
            &[0x81, 0x01, 0xff][..],
            &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff][..],
            &[0x81, 0x2a, 0x02, 0xa0, 0x04, 0x02, 0xff][..],
            &[0x81, 0x01, 0x00, 0x02, 0xff][..],
        ] {
            assert!(
                Plain::new(
                    frame,
                    TimeoutClass::Quick,
                    RetryClass::Never,
                    ControlClass::Normal
                )
                .is_ok(),
                "legitimate custom frame {frame:x?} must stay admissible"
            );
        }
    }

    /// #700: the reply-shape axis defaults to `AckThenCompletion` and threads
    /// through the policy, the spec, and every raw command's `Request` hook.
    #[test]
    fn reply_shape_defaults_and_threads_through_policy_and_commands() {
        // The default keeps existing raw code unchanged.
        let default_policy =
            Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
                .expect("valid policy");
        assert_eq!(
            default_policy.reply_shape(),
            RawReplyShape::AckThenCompletion
        );
        assert_eq!(RawReplyShape::default(), RawReplyShape::AckThenCompletion);
        assert_eq!(
            Plain::new(
                [0x81, 0x01, 0x02, 0xff],
                TimeoutClass::Quick,
                RetryClass::Never,
                ControlClass::Normal,
            )
            .expect("plain")
            .reply_shape(),
            RawReplyShape::AckThenCompletion,
        );

        for shape in [
            RawReplyShape::AckThenCompletion,
            RawReplyShape::CompletionOnly,
            RawReplyShape::NoReply,
        ] {
            // The builder sets the shape without disturbing the other classes.
            let policy = default_policy.with_reply_shape(shape);
            assert_eq!(policy.reply_shape(), shape);
            assert_eq!(policy.timeout_class(), TimeoutClass::Quick);
            assert_eq!(policy.retry_class(), RetryClass::Never);
            assert_eq!(policy.control_class(), ControlClass::Normal);

            // The spec carries the same axis and lowers to the same policy.
            let spec = Spec::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
                .expect("valid spec")
                .with_reply_shape(shape);
            assert_eq!(spec.reply_shape(), shape);
            assert_eq!(spec.policy().reply_shape(), shape);

            // Each command reports the shape through its inherent accessor and
            // the `Request::reply_shape` hook preparation reads.
            let plain = Plain::with_policy([0x81, 0x01, 0x02, 0xff], policy).expect("plain");
            assert_eq!(plain.reply_shape(), shape);
            assert_eq!(Request::reply_shape(&plain), shape);

            let targeted =
                Targeted::with_policy([0x81, 0x01, 0x06, 0xff], AffectedAxes::PAN_TILT, policy)
                    .expect("targeted");
            assert_eq!(targeted.reply_shape(), shape);
            assert_eq!(Request::reply_shape(&targeted), shape);

            let applied =
                AppliedOnly::with_policy([0x81, 0x01, 0x07, 0xff], AffectedAxes::ZOOM, policy)
                    .expect("applied");
            assert_eq!(applied.reply_shape(), shape);
            assert_eq!(Request::reply_shape(&applied), shape);
        }
    }

    /// #700: an inquiry always awaits its reply, so a raw inquiry rejects any
    /// non-default reply shape at construction rather than silently ignore it.
    #[test]
    fn raw_inquiry_rejects_non_default_reply_shape() {
        let ackthen = Policy::new(
            TimeoutClass::Inquiry,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid policy");

        // The default shape is accepted through every inquiry constructor.
        assert!(Inquiry::<u8>::with_policy(
            [0x81, 0x09, 0x04, 0xff],
            InquiryRoute::RAW,
            ResponseDecoder::from_fn(decode_first),
            ackthen,
        )
        .is_ok());
        assert!(Inquiry::from_fn(
            [0x81, 0x09, 0x04, 0xff],
            InquiryRoute::RAW,
            decode_first,
            TimeoutClass::Inquiry,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_ok());

        // Completion-only and no-reply are rejected — the guard is exactly the
        // non-default shapes and nothing wider.
        for shape in [RawReplyShape::CompletionOnly, RawReplyShape::NoReply] {
            let policy = ackthen.with_reply_shape(shape);
            assert!(
                matches!(
                    Inquiry::<u8>::with_policy(
                        [0x81, 0x09, 0x04, 0xff],
                        InquiryRoute::RAW,
                        ResponseDecoder::from_fn(decode_first),
                        policy,
                    ),
                    Err(Error::InvalidRequest(_))
                ),
                "raw inquiry must reject reply shape {shape:?}"
            );
        }
    }
}
