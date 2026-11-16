//! Explicit, typed escape hatches for custom VISCA wire messages.
//!
//! The types in this module are the only raw-request surface in the typed
//! owner API. They are deliberately separate from the request-class markers
//! in [`crate::request`] and from the low-level [`crate::command`] extension
//! surface. A raw value is still admitted only through the async camera's
//! `execute`, `inquire`, or `submit` methods (and their blocking projections);
//! it cannot select a lifecycle ID, target, or completion kind at submission
//! time. Its [`crate::request::SubmissionClass`] is chosen in its kind-specific policy
//! from the ordinary lanes (`Background`, `Normal`, `User`); the urgent safety
//! lane is reserved for the owner's stops and protocol cancellation and is not
//! selectable here. A camera handle's `with_submission_class` derived view or
//! `set_submission_class` default applies to it on the same terms.
//!
//! Raw constructors validate and own the complete VISCA frame. The first byte
//! must be a valid VISCA camera address and the final byte must be `0xff`.
//! Owner-only wire primitives are refused at construction: a socket cancel
//! (`8x 2y ff`) or a per-camera interface clear (`8x 01 00 01 ff`) would let a
//! target-scoped `execute` cancel another operation's socket or reset the
//! shared command buffer, so both are rejected. The broadcast address-set form
//! is likewise owner-only and is rejected by raw validation; preparation still
//! checks the first address against its target as a separate defense.
//! The camera view supplies the authoritative target during preparation, so a
//! frame addressed to another target is rejected rather than being silently
//! rewritten or inspected to infer target identity. Frames up to
//! [`crate::raw::MAX_BYTES`] are accepted. The first `INLINE_BYTES` fit in inline storage;
//! larger custom frames allocate once when the raw value owns the input and
//! once when preparation writes directly into its final inert storage. No
//! intermediate frame buffer is copied. Standard built-in request
//! encoding remains inline and allocation-free.
//!
//! Raw policy is explicit and owned by each value. [`CommandPolicy`](crate::raw::CommandPolicy) supplies
//! a [`PlainRequest`](crate::raw::PlainRequest) command's [`crate::profiles::CommandTimeoutClass`], retry and ordinary submission
//! class; [`OperationPolicy`](crate::raw::OperationPolicy) supplies the same facts for [`TargetedRequest`](crate::raw::TargetedRequest) and
//! [`AppliedOnlyRequest`](crate::raw::AppliedOnlyRequest). [`InquiryPolicy`](crate::raw::InquiryPolicy) carries only retry and submission class:
//! its timeout is always [`crate::request::TimeoutClass::Inquiry`] and its reply protocol is
//! fixed. Policy constructors are infallible; raw request constructors validate
//! only the wire bytes. No byte-based lifecycle inference or caller-selected
//! operation class exists.
//!
//! A plain command declares its [`RawReplyShape`](crate::raw::RawReplyShape) with
//! [`CommandPolicy::with_reply_shape`](crate::raw::CommandPolicy::with_reply_shape). Operations instead select
//! [`OperationReplyShape`](crate::raw::OperationReplyShape), whose variants are `AckThenCompletion` and
//! `CompletionOnly`. `NoReply` is plain-command-only: a successful
//! fire-and-forget write proves local transport submission rather than the
//! protocol application an operation handle promises. Reply shape never
//! re-admits the owner-only wire primitives rejected above.

use std::fmt;

use smallvec::SmallVec;

/// Policy and validation types used by the raw constructors.
use crate::{
    AffectedAxes, CommandTimeoutClass, ControlClass, ResponseDecoder, RetryClass, SubmissionClass,
    TimeoutClass,
};

use crate::{
    command::bytes::{constants::system, VISCA_TERMINATOR},
    completion, request, CameraId, Error, Inquiry as InquiryContract, OperationCommand, Request,
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
/// `AckThenCompletion` and `CompletionOnly` are meaningful for all command
/// classes ([`PlainRequest`], [`TargetedRequest`], [`AppliedOnlyRequest`]). [`NoReply`](Self::NoReply)
/// is plain-command-only: it proves only that the local transport write
/// succeeded, whereas an operation's `applied()` promise requires a terminal
/// camera response. An [`InquiryRequest`] always awaits its reply, so its policy has
/// no command reply-shape field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
    ///
    /// Its answer carries no identity, so it is exclusive on its camera: it
    /// starts only when no other command or inquiry to that camera is live,
    /// and while it is unanswered no inquiry and no ordinary command to that
    /// camera is written — they queue, under their own deadlines. On a raw
    /// byte stream (TCP, serial) that lasts until its completion or
    /// rejection arrives, even after its own deadline ended it unconfirmed;
    /// if that answer is still missing when its ambiguity window ends, they
    /// fail unwritten with [`crate::Error::InquiryCorrelationLost`] or
    /// [`crate::Error::CommandCorrelationLost`] until it arrives. STOPs are
    /// never held back by it (#795).
    CompletionOnly,
    /// A plain fire-and-forget command expects nothing back. Its `execute()`
    /// call succeeds once the local transport write succeeds; that result does
    /// not claim camera acceptance or protocol application. The owner never
    /// holds it waiting for a frame. To keep its later raw-correlation
    /// quarantine attributable, the owner starts it only when the target has
    /// no live command or inquiry and excludes same-target work while the
    /// write is in flight; ordinary command and inquiry work then waits out
    /// the bounded quarantine, through which its late response is ignored.
    /// STOPs are never held back by it (#795): a STOP is written at once,
    /// and its ACK (which this command never sends) and the completion or
    /// error naming its socket still reach it. A socketless error in that
    /// window could be this command's rejection or the STOP's, and binds to
    /// neither: on a raw byte stream the STOP's own ACK proves it was this
    /// command's; failing such proof the STOP ends
    /// [`crate::Error::UnsequencedCommandUnconfirmed`]. On a raw byte stream
    /// its possible rejection stays owed until a later answer from the camera
    /// settles it (a stall can delay it past any window), so a
    /// `CompletionOnly` command to that camera waits for that, failing with
    /// [`crate::Error::CommandCorrelationLost`] once this command's ambiguity
    /// window has ended; and it is
    /// itself held back while an earlier command's answer is owed, failing
    /// with [`crate::Error::CommandCorrelationLost`] once that lane latches.
    /// Raw operations use [`OperationReplyShape`], which has no such variant.
    NoReply,
}

/// The reply protocols supported by a raw operation.
///
/// Operations require terminal camera evidence, so they cannot select the
/// plain command's [`RawReplyShape::NoReply`] protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum OperationReplyShape {
    /// The camera acknowledges the command before completing it.
    #[default]
    AckThenCompletion,
    /// The camera completes the command without an acknowledgement.
    CompletionOnly,
}

impl OperationReplyShape {
    const fn raw(self) -> RawReplyShape {
        match self {
            Self::AckThenCompletion => RawReplyShape::AckThenCompletion,
            Self::CompletionOnly => RawReplyShape::CompletionOnly,
        }
    }
}

impl From<OperationReplyShape> for RawReplyShape {
    fn from(shape: OperationReplyShape) -> Self {
        shape.raw()
    }
}

/// Explicit command timeout, retry and ordinary submission classes for [`PlainRequest`].
///
/// A command timeout cannot be an inquiry timeout, and [`crate::request::SubmissionClass`]
/// cannot select the owner's urgent safety lane (#679).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "CommandPolicyData", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema), schemars(!try_from))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct CommandPolicy {
    timeout: CommandTimeoutClass,
    retry: RetryClass,
    class: SubmissionClass,
    reply_shape: RawReplyShape,
}

/// Explicit command timeout, retry and ordinary submission classes for raw operations.
///
/// Its reply shape requires terminal camera evidence and cannot be `NoReply`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "OperationPolicyData", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema), schemars(!try_from))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct OperationPolicy {
    timeout: CommandTimeoutClass,
    retry: RetryClass,
    class: SubmissionClass,
    reply_shape: OperationReplyShape,
}

/// Explicit retry and ordinary submission classes for [`InquiryRequest`].
///
/// InquiryRequest timeout and reply protocols are fixed by its request kind, so this
/// policy stores neither a timeout class nor a command reply shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "InquiryPolicyData", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema), schemars(!try_from))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct InquiryPolicy {
    retry: RetryClass,
    class: SubmissionClass,
}

// Persistence re-enters the public constructors rather than writing private
// policy fields. Typed fields retain the closed command-timeout, ordinary-lane
// and operation-reply subsets. These representations are never schema or TS
// exports: those describe the public policy's actual serialized fields.
#[cfg(feature = "serde")]
macro_rules! command_policy_data {
    ($policy:ident, $data:ident, $reply:ty) => {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $data {
            timeout: CommandTimeoutClass,
            retry: RetryClass,
            class: SubmissionClass,
            reply_shape: $reply,
        }

        impl TryFrom<$data> for $policy {
            type Error = Error;

            fn try_from(value: $data) -> Result<Self> {
                Ok(Self::new(value.timeout, value.retry, value.class)
                    .with_reply_shape(value.reply_shape))
            }
        }
    };
}

#[cfg(feature = "serde")]
command_policy_data!(CommandPolicy, CommandPolicyData, RawReplyShape);
#[cfg(feature = "serde")]
command_policy_data!(OperationPolicy, OperationPolicyData, OperationReplyShape);

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InquiryPolicyData {
    retry: RetryClass,
    class: SubmissionClass,
}

#[cfg(feature = "serde")]
impl TryFrom<InquiryPolicyData> for InquiryPolicy {
    type Error = Error;

    fn try_from(value: InquiryPolicyData) -> Result<Self> {
        Ok(Self::new(value.retry, value.class))
    }
}

macro_rules! ordinary_policy_accessors {
    () => {
        /// Returns the selected retry class.
        #[must_use]
        pub const fn retry_class(self) -> RetryClass {
            self.retry
        }

        /// Returns the selected ordinary submission class.
        #[must_use]
        pub const fn submission_class(self) -> SubmissionClass {
            self.class
        }

        /// Returns the corresponding scheduler-control class.
        #[must_use]
        pub const fn control_class(self) -> ControlClass {
            self.class.control_class()
        }
    };
}

macro_rules! command_policy {
    ($policy:ident, $reply:ident) => {
        impl $policy {
            /// Creates an explicit policy in an ordinary submission lane.
            ///
            /// `timeout_class` selects the registered profile's command deadline.
            #[must_use]
            pub const fn new(
                timeout_class: CommandTimeoutClass,
                retry: RetryClass,
                class: SubmissionClass,
            ) -> Self {
                Self {
                    timeout: timeout_class,
                    retry,
                    class,
                    reply_shape: $reply::AckThenCompletion,
                }
            }

            /// Returns this policy with the declared reply protocol.
            #[must_use]
            pub const fn with_reply_shape(mut self, reply_shape: $reply) -> Self {
                self.reply_shape = reply_shape;
                self
            }

            /// Returns the selected command timeout class.
            #[must_use]
            pub const fn timeout_class(self) -> TimeoutClass {
                self.timeout.timeout_class()
            }

            ordinary_policy_accessors!();
        }
    };
}

command_policy!(CommandPolicy, RawReplyShape);
command_policy!(OperationPolicy, OperationReplyShape);

impl CommandPolicy {
    /// Returns the declared command reply protocol.
    #[must_use]
    pub const fn reply_shape(self) -> RawReplyShape {
        self.reply_shape
    }
}

impl OperationPolicy {
    /// Returns the declared operation reply protocol as its raw wire shape.
    #[must_use]
    pub const fn reply_shape(self) -> RawReplyShape {
        self.reply_shape.raw()
    }
}

impl InquiryPolicy {
    /// Creates an inquiry policy in an ordinary submission lane.
    #[must_use]
    pub const fn new(retry: RetryClass, class: SubmissionClass) -> Self {
        Self { retry, class }
    }

    ordinary_policy_accessors!();
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
            return Err(Error::buffer_too_small(self.len(), buffer.len()));
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
        return Err(Error::InvalidRequest {
            reason: format!(
                "raw VISCA frame has {} bytes, exceeding raw::MAX_BYTES ({MAX_BYTES})",
                bytes.len()
            )
            .into(),
        });
    }
    if bytes.len() < 2 {
        return Err(Error::InvalidRequest {
            reason: "raw VISCA frame must contain an address and terminator".into(),
        });
    }
    if !(0x81..=0x88).contains(&bytes[0]) {
        return Err(Error::InvalidRequest {
            reason: "raw VISCA frame must begin with an address byte in 0x81..=0x88".into(),
        });
    }
    if bytes.last().copied() != Some(VISCA_TERMINATOR) {
        return Err(Error::InvalidRequest {
            reason: "raw VISCA frame must end with the 0xff terminator".into(),
        });
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
/// is a broadcast primitive and is owner-only as well; raw validation rejects it
/// at the start of a frame, while preparation still checks the first address
/// against its target.
///
/// The socket-cancel guard is deliberately length-bounded to the exact 3-byte
/// cancel subframe: a longer frame whose command byte falls in `0x20..=0x2f`
/// (for example USB audio, `81 2a 02 a0 04 02 ff`) is an ordinary command and
/// stays admissible.  A raw value can still contain a vendor payload byte equal
/// to `0xff`; only a valid VISCA address immediately following a terminator is
/// treated as an apparent second subframe, which raw requests do not model.
pub(crate) fn reject_owner_only_primitive(bytes: &[u8]) -> Result<()> {
    // Caller guarantees `bytes.len() >= 2`, `bytes[0]` is a VISCA address, and
    // `bytes.last() == Some(0xff)`.
    // The first primitive prefix is consumed by the camera's framer at its
    // first terminator even when arbitrary bytes trail it, so owner-only
    // prefixes cannot be hidden behind a custom suffix.
    reject_owner_only_at_boundary(bytes, 0)?;

    // VISCA's stream boundary is the terminator.  Do not reject every second
    // `0xff`: values such as a preset number of 0xff are legal payload data.
    // A valid VISCA address immediately following a terminator is instead an
    // apparent second subframe. Raw requests model one complete frame and one
    // lifecycle, so accepting that sequence would send multiple commands while
    // tracking only the first one.
    if bytes
        .windows(2)
        .any(|pair| pair[0] == VISCA_TERMINATOR && is_visca_address(pair[1]))
    {
        return Err(Error::InvalidRequest { reason: "raw VISCA frame must contain exactly one subframe; a terminator followed by another VISCA address is not supported".into() });
    }
    Ok(())
}

/// Reject an owner-only primitive whose first byte is at `start`.
///
/// The bounds checks intentionally inspect only the primitive prefix. A longer
/// ordinary command beginning with a `0x2y` command byte is not mistaken for a
/// socket cancel unless its third byte is the terminator.
fn reject_owner_only_at_boundary(bytes: &[u8], start: usize) -> Result<()> {
    let remaining = &bytes[start..];
    if remaining.len() >= 3
        && (remaining[0] & 0xf0) == 0x80
        && (remaining[1] & 0xf0) == system::COMMAND_CANCEL
        && remaining[2] == VISCA_TERMINATOR
    {
        return Err(Error::InvalidRequest { reason: "raw VISCA frame must not be a socket cancel (8x 2y ff); the owner cancels a correlated operation through its handle, not through execute()".into() });
    }
    if remaining.len() >= 5
        && (remaining[0] & 0xf0) == 0x80
        && remaining[1..4] == system::INTERFACE_CLEAR
        && remaining[4] == VISCA_TERMINATOR
    {
        return Err(Error::InvalidRequest { reason: "raw VISCA frame must not be an interface clear (8x 01 00 01 ff); it resets the shared command buffer and is owner-only".into() });
    }
    // Address assignment is also owner-only. The first address is still
    // checked against the prepared target, and this rejects the broadcast
    // setup primitive before a target-scoped request can emit it.
    if remaining.len() >= 4
        && remaining[0] == CameraId::BROADCAST.to_address_byte()
        && remaining[1] == system::ADDRESS_SET_REGISTER
        && (remaining[2] & 0xf0) == 0x00
        && remaining[3] == VISCA_TERMINATOR
    {
        return Err(Error::InvalidRequest { reason: "raw VISCA frame must not contain an address-set subframe (88 30 0y ff); serial setup is owner-only".into() });
    }
    Ok(())
}

#[inline]
fn is_visca_address(byte: u8) -> bool {
    (0x81..=0x88).contains(&byte)
}

/// The policy accessors every raw request value exposes, written once for
/// the four raw types (#817). `$subject` names the value in the docs; the
/// optional `reply_shape` arm adds the reply-shape accessor the command and
/// operation types carry.
macro_rules! raw_policy_accessors {
    (@common $policy:ident, $subject:literal) => {
        #[doc = concat!("Returns ", $subject, "'s explicit policy.")]
        #[must_use]
        pub const fn policy(&self) -> $policy {
            self.policy
        }

        #[doc = concat!("Returns ", $subject, "'s explicit retry class.")]
        #[must_use]
        pub const fn retry_class(&self) -> RetryClass {
            self.policy.retry_class()
        }

        #[doc = concat!("Returns ", $subject, "'s explicit control class.")]
        #[must_use]
        pub const fn control_class(&self) -> ControlClass {
            self.policy.control_class()
        }
    };
    ($policy:ident, $subject:literal) => {
        raw_policy_accessors!(@common $policy, $subject);
    };
    ($policy:ident, $subject:literal, reply_shape) => {
        raw_policy_accessors!(@common $policy, $subject);

        #[doc = concat!("Returns ", $subject, "'s explicit timeout class.")]
        #[must_use]
        pub const fn timeout_class(&self) -> TimeoutClass {
            self.policy.timeout_class()
        }

        #[doc = concat!("Returns ", $subject, "'s declared reply shape.")]
        #[must_use]
        pub const fn reply_shape(&self) -> RawReplyShape {
            self.policy.reply_shape()
        }
    };
}

/// The [`crate::request::Request`] items every raw request value forwards to its policy and
/// wire, written once for the four raw types (#817). The policy constants are
/// the given unread fallbacks, documented as such.
macro_rules! raw_request_forwarding {
    (@timeout Inquiry, $value:ident) => { TimeoutClass::Inquiry };
    (@timeout $timeout:ident, $value:ident) => { $value.policy.timeout_class() };
    ($timeout:ident, $retry:ident, $control:ident) => {
        const MAX_SIZE: usize = MAX_BYTES;

        /// Unread fallback. A raw value's policy is selected at construction
        /// and returned by [`crate::request::Request::timeout_class`], which this type
        /// overrides; this constant serves only fixed request types.
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::$timeout;

        /// Unread fallback. A raw value's policy is selected at construction
        /// and returned by [`crate::request::Request::retry_class`], which this type
        /// overrides; this constant serves only fixed request types.
        const RETRY_CLASS: RetryClass = RetryClass::$retry;

        /// Unread fallback. A raw value's policy is selected at construction
        /// and returned by [`crate::request::Request::control_class`], which this type
        /// overrides; this constant serves only fixed request types.
        const CONTROL_CLASS: ControlClass = ControlClass::$control;

        fn timeout_class(&self) -> TimeoutClass {
            raw_request_forwarding!(@timeout $timeout, self)
        }

        fn retry_class(&self) -> RetryClass {
            self.policy.retry_class()
        }

        fn control_class(&self) -> ControlClass {
            self.policy.control_class()
        }

        #[allow(private_interfaces)]
        fn declared_max_size(
            &self,
            _authority: crate::requests::RequestContractAuthority,
        ) -> usize {
            self.wire.len()
        }

        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
            self.wire.write_into(buffer)
        }
    };
}

/// A raw operation type of one completion kind, written once for
/// [`TargetedRequest`] and [`AppliedOnlyRequest`] (#817).
macro_rules! raw_operation {
    (
        $(#[$type_doc:meta])*
        $name:ident: $kind:ty, $noun:literal, $lifecycle:literal
    ) => {
        $(#[$type_doc])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            wire: Wire,
            axes: AffectedAxes,
            policy: OperationPolicy,
        }

        impl $name {
            #[doc = concat!(
                "Creates a bounded raw ", $noun,
                " operation that moves `axes`, with an explicit protocol policy. ",
                "`timeout_class` selects the registered profile's command deadline."
            )]
            pub fn new(
                bytes: impl AsRef<[u8]>,
                axes: AffectedAxes,
                timeout_class: CommandTimeoutClass,
                retry: RetryClass,
                class: SubmissionClass,
            ) -> Result<Self> {
                Self::with_policy(bytes, axes, OperationPolicy::new(timeout_class, retry, class))
            }

            /// [`Self::new`] with an explicit [`OperationPolicy`] value.
            pub fn with_policy(
                bytes: impl AsRef<[u8]>,
                axes: AffectedAxes,
                policy: OperationPolicy,
            ) -> Result<Self> {
                Ok(Self {
                    wire: Wire::new(bytes)?,
                    axes,
                    policy,
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

            raw_policy_accessors!(OperationPolicy, "this operation", reply_shape);
        }

        impl Request for $name {
            type Class = request::OperationClass<$kind>;
            raw_request_forwarding!(Quick, Never, User);

            fn reply_shape(&self) -> RawReplyShape {
                self.policy.reply_shape()
            }

            fn validate_for_target(&self, target: &crate::TargetRegistration) -> Result<()> {
                let profile = target.profile();
                if profile.supports_axes(self.axes) {
                    Ok(())
                } else {
                    Err(Error::FeatureNotSupported {
                        feature: concat!("raw ", $noun, " operation axes"),
                    })
                }
            }
        }

        impl OperationCommand<$kind> for $name {
            fn affected_axes(&self) -> AffectedAxes {
                self.axes
            }
        }
    };
}

/// A raw plain command with no operation lifecycle handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainRequest {
    wire: Wire,
    policy: CommandPolicy,
    motion_axes: Option<AffectedAxes>,
}

impl PlainRequest {
    /// Creates a bounded raw plain command with explicit protocol policy.
    ///
    /// `timeout_class` selects the registered profile's command deadline.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        timeout_class: CommandTimeoutClass,
        retry: RetryClass,
        class: SubmissionClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, CommandPolicy::new(timeout_class, retry, class))
    }

    /// Creates a bounded raw plain command from an explicit policy value.
    pub fn with_policy(bytes: impl AsRef<[u8]>, policy: CommandPolicy) -> Result<Self> {
        Ok(Self {
            wire: Wire::new(bytes)?,
            policy,
            motion_axes: None,
        })
    }

    /// Declares axes moved by this command so an owner halt can suppress it.
    /// Undeclared raw commands are never suppressed; this grants no STOP priority.
    #[must_use]
    pub const fn with_motion_axes(mut self, axes: AffectedAxes) -> Self {
        self.motion_axes = Some(axes);
        self
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    raw_policy_accessors!(CommandPolicy, "this value", reply_shape);
}

impl Request for PlainRequest {
    type Class = request::PlainClass;
    raw_request_forwarding!(Quick, Never, Normal);

    fn reply_shape(&self) -> RawReplyShape {
        self.policy.reply_shape()
    }

    fn motion_axes(&self) -> Option<AffectedAxes> {
        self.motion_axes
    }
}

/// A raw inquiry with an owned typed response decoder.
pub struct InquiryRequest<R> {
    wire: Wire,
    decoder: ResponseDecoder<R>,
    policy: InquiryPolicy,
    payload_shape: crate::PayloadShape,
}

impl<R> InquiryRequest<R> {
    /// Creates a bounded raw inquiry with an explicit decoder and policy.
    /// Its timeout class is always [`crate::request::TimeoutClass::Inquiry`].
    ///
    /// `ResponseDecoder::with_context` can be used when the decoder needs
    /// immutable per-instance context; the context is moved into this inquiry
    /// and remains outside the protocol engine.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        decoder: ResponseDecoder<R>,
        retry: RetryClass,
        class: SubmissionClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, decoder, InquiryPolicy::new(retry, class))
    }

    /// Creates a raw inquiry from an explicit decoder and policy.
    pub fn with_policy(
        bytes: impl AsRef<[u8]>,
        decoder: ResponseDecoder<R>,
        policy: InquiryPolicy,
    ) -> Result<Self> {
        Ok(Self {
            wire: Wire::new(bytes)?,
            decoder,
            policy,
            payload_shape: crate::PayloadShape::Unknown,
        })
    }

    /// Creates a raw inquiry using an allocation-free function decoder.
    /// Its timeout class is always [`crate::request::TimeoutClass::Inquiry`].
    pub fn from_fn(
        bytes: impl AsRef<[u8]>,
        decoder: fn(&[u8]) -> Result<R>,
        retry: RetryClass,
        class: SubmissionClass,
    ) -> Result<Self> {
        Self::with_policy(
            bytes,
            ResponseDecoder::from_fn(decoder),
            InquiryPolicy::new(retry, class),
        )
    }

    /// Creates a raw inquiry whose decoder owns immutable context.
    /// Its timeout class is always [`crate::request::TimeoutClass::Inquiry`].
    pub fn with_context<C>(
        bytes: impl AsRef<[u8]>,
        context: C,
        decoder: fn(&C, &[u8]) -> Result<R>,
        retry: RetryClass,
        class: SubmissionClass,
    ) -> Result<Self>
    where
        C: Send + Sync + 'static,
        R: 'static,
    {
        Self::with_policy(
            bytes,
            ResponseDecoder::with_context(context, decoder),
            InquiryPolicy::new(retry, class),
        )
    }

    /// Declares the decoded data shape accepted by this inquiry's decoder.
    ///
    /// Lengths exclude the address, reply header and terminator. A declared
    /// shape supplies raw-reply correlation evidence; decoder value checks
    /// still apply. The default [`crate::request::PayloadShape::Unknown`] overlaps all
    /// shapes and preserves conservative positional correlation.
    /// Raw targets reserve at most 16 exact declarations across admitted
    /// inquiries, unpaid replies and skew holds; a new declaration at capacity
    /// fails with [`Error::InquiryShapeCapacityExhausted`].
    #[must_use]
    pub const fn with_payload_shape(mut self, shape: crate::PayloadShape) -> Self {
        self.payload_shape = shape;
        self
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    raw_policy_accessors!(InquiryPolicy, "this inquiry");
}

impl<R> fmt::Debug for InquiryRequest<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InquiryRequest")
            .field("wire", &self.wire)
            .field("decoder", &self.decoder)
            .field("policy", &self.policy)
            .field("payload_shape", &self.payload_shape)
            .finish()
    }
}

impl<R> Clone for InquiryRequest<R> {
    fn clone(&self) -> Self {
        Self {
            wire: self.wire.clone(),
            decoder: self.decoder.clone(),
            policy: self.policy,
            payload_shape: self.payload_shape,
        }
    }
}

impl<R> Request for InquiryRequest<R>
where
    R: Send + 'static,
{
    type Class = request::InquiryClass;
    raw_request_forwarding!(Inquiry, Inquiry, Normal);
}

impl<R> InquiryContract for InquiryRequest<R>
where
    R: Send + 'static,
{
    type Response = R;

    fn payload_shape(&self) -> crate::PayloadShape {
        self.payload_shape
    }

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        self.decoder.clone()
    }
}

raw_operation! {
    /// A raw operation with a meaningful target state and profile-selected
    /// protocol settlement.
    TargetedRequest: completion::Targeted, "targeted", "`applied()`/`settled()`"
}

raw_operation! {
    /// A raw operation whose terminal protocol application is its only
    /// lifecycle observation. It intentionally has no `settled` method or
    /// target inference.
    AppliedOnlyRequest: completion::AppliedOnly, "applied-only", "`applied()`"
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

    #[cfg(feature = "serde")]
    #[test]
    fn reply_shapes_serialize_as_exact_snake_case_tags() {
        for (shape, tag) in [
            (RawReplyShape::AckThenCompletion, "ack_then_completion"),
            (RawReplyShape::CompletionOnly, "completion_only"),
            (RawReplyShape::NoReply, "no_reply"),
        ] {
            assert_eq!(serde_json::to_value(shape).unwrap(), tag);
            assert_eq!(
                serde_json::from_value::<RawReplyShape>(tag.into()).unwrap(),
                shape
            );
        }
        for (shape, tag) in [
            (
                OperationReplyShape::AckThenCompletion,
                "ack_then_completion",
            ),
            (OperationReplyShape::CompletionOnly, "completion_only"),
        ] {
            assert_eq!(serde_json::to_value(shape).unwrap(), tag);
            assert_eq!(
                serde_json::from_value::<OperationReplyShape>(tag.into()).unwrap(),
                shape
            );
        }
        assert!(serde_json::from_value::<OperationReplyShape>("no_reply".into()).is_err());
        assert!(serde_json::from_value::<RawReplyShape>("AckThenCompletion".into()).is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn policy_json_round_trips_the_constructor_contract() {
        use serde_json::json;
        let command = CommandPolicy::new(
            CommandTimeoutClass::Quick,
            RetryClass::Standard,
            SubmissionClass::Background,
        )
        .with_reply_shape(RawReplyShape::NoReply);
        let operation = OperationPolicy::new(
            CommandTimeoutClass::Movement,
            RetryClass::Never,
            SubmissionClass::User,
        )
        .with_reply_shape(OperationReplyShape::CompletionOnly);
        let inquiry = InquiryPolicy::new(RetryClass::Inquiry, SubmissionClass::Normal);
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            json!({
                "timeout": "quick", "retry": "standard", "class": "background", "reply_shape": "no_reply"
            })
        );
        assert_eq!(
            serde_json::to_value(operation).unwrap(),
            json!({
                "timeout": "movement", "retry": "never", "class": "user", "reply_shape": "completion_only"
            })
        );
        assert_eq!(
            serde_json::to_value(inquiry).unwrap(),
            json!({
                "retry": "inquiry", "class": "normal"
            })
        );
        assert_eq!(
            serde_json::from_value::<CommandPolicy>(serde_json::to_value(command).unwrap())
                .unwrap(),
            command
        );
        assert_eq!(
            serde_json::from_value::<OperationPolicy>(serde_json::to_value(operation).unwrap())
                .unwrap(),
            operation
        );
        assert_eq!(
            serde_json::from_value::<InquiryPolicy>(serde_json::to_value(inquiry).unwrap())
                .unwrap(),
            inquiry
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn policy_json_cannot_bypass_restricted_classes_or_reply_shapes() {
        use serde_json::json;
        for forbidden in [
            json!({"timeout":"inquiry", "retry":"never", "class":"normal", "reply_shape":"ack_then_completion"}),
            json!({"timeout":"quick", "retry":"never", "class":"urgent", "reply_shape":"ack_then_completion"}),
            json!({"timeout":"Quick", "retry":"never", "class":"normal", "reply_shape":"ack_then_completion"}),
        ] {
            assert!(serde_json::from_value::<CommandPolicy>(forbidden.clone()).is_err());
            assert!(serde_json::from_value::<OperationPolicy>(forbidden).is_err());
        }
        assert!(serde_json::from_value::<OperationPolicy>(json!({
            "timeout":"quick", "retry":"never", "class":"normal", "reply_shape":"no_reply"
        }))
        .is_err());
        assert!(serde_json::from_value::<InquiryPolicy>(json!({
            "retry":"inquiry", "class":"urgent"
        }))
        .is_err());
        assert!(serde_json::from_value::<InquiryPolicy>(json!({
            "retry":"inquiry", "class":"normal", "timeout":"quick"
        }))
        .is_err());

        fn refuses_incomplete_or_unknown_fields<
            T: serde::de::DeserializeOwned + serde::Serialize,
        >(
            value: T,
        ) {
            let original = serde_json::to_value(value).unwrap();
            let mut unknown = original.clone();
            unknown
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), json!(true));
            assert!(serde_json::from_value::<T>(unknown).is_err());
            for field in original.as_object().unwrap().keys() {
                let mut missing = original.clone();
                missing.as_object_mut().unwrap().remove(field);
                assert!(serde_json::from_value::<T>(missing).is_err(), "{field}");
            }
        }
        refuses_incomplete_or_unknown_fields(CommandPolicy::new(
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        ));
        refuses_incomplete_or_unknown_fields(OperationPolicy::new(
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        ));
        refuses_incomplete_or_unknown_fields(InquiryPolicy::new(
            RetryClass::Inquiry,
            SubmissionClass::Normal,
        ));
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn policy_schemas_and_typescript_describe_only_public_closed_shapes() {
        use ts_rs::TS;
        fn check_policy<T: schemars::JsonSchema + TS>(fields: &[&str]) {
            let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
            let properties = schema["properties"].as_object().unwrap();
            let actual: std::collections::BTreeSet<_> =
                properties.keys().map(String::as_str).collect();
            let expected: std::collections::BTreeSet<_> = fields.iter().copied().collect();
            assert_eq!(actual, expected);
            assert_eq!(schema["additionalProperties"], false);
            let required: std::collections::BTreeSet<_> = schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|field| field.as_str().unwrap())
                .collect();
            assert_eq!(required, expected);
            assert!(!schema.to_string().contains("PolicyData"));
            let declaration = T::decl(&ts_rs::Config::default());
            for field in fields {
                assert!(declaration.contains(&format!("{field}:")), "{declaration}");
            }
            assert!(!declaration.contains("PolicyData"));
        }
        check_policy::<CommandPolicy>(&["timeout", "retry", "class", "reply_shape"]);
        check_policy::<OperationPolicy>(&["timeout", "retry", "class", "reply_shape"]);
        check_policy::<InquiryPolicy>(&["retry", "class"]);
        for tag in ["ack_then_completion", "completion_only", "no_reply"] {
            assert!(serde_json::to_value(schemars::schema_for!(RawReplyShape))
                .unwrap()
                .to_string()
                .contains(&format!("\"{tag}\"")));
            assert!(RawReplyShape::decl(&ts_rs::Config::default()).contains(&format!("\"{tag}\"")));
        }
        let operation_schema = serde_json::to_value(schemars::schema_for!(OperationReplyShape))
            .unwrap()
            .to_string();
        let operation_declaration = OperationReplyShape::decl(&ts_rs::Config::default());
        for tag in ["ack_then_completion", "completion_only"] {
            assert!(operation_schema.contains(&format!("\"{tag}\"")));
            assert!(operation_declaration.contains(&format!("\"{tag}\"")));
        }
        assert!(!operation_schema.contains("no_reply"));
        assert!(!operation_declaration.contains("no_reply"));
    }

    fn decode_first(payload: &[u8]) -> Result<u8> {
        payload
            .first()
            .copied()
            .ok_or_else(|| Error::InvalidRequest {
                reason: "empty test payload".into(),
            })
    }

    #[test]
    fn every_raw_inquiry_constructor_retains_its_optional_payload_shape() {
        use crate::PayloadShape;
        let bytes = [0x81, 0x09, 0x04, 0xFF];
        let policy = InquiryPolicy::new(RetryClass::Never, SubmissionClass::Normal);
        let queries = [
            InquiryRequest::new(
                bytes,
                ResponseDecoder::from_fn(decode_first),
                RetryClass::Never,
                SubmissionClass::Normal,
            )
            .expect("owned decoder"),
            InquiryRequest::with_policy(bytes, ResponseDecoder::from_fn(decode_first), policy)
                .expect("explicit policy"),
            InquiryRequest::from_fn(
                bytes,
                decode_first,
                RetryClass::Never,
                SubmissionClass::Normal,
            )
            .expect("function decoder"),
            InquiryRequest::with_context(
                bytes,
                0_u8,
                |context, payload| decode_first(payload).map(|byte| byte + *context),
                RetryClass::Never,
                SubmissionClass::Normal,
            )
            .expect("context decoder"),
        ];
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("profile");
        for inquiry in queries {
            assert_eq!(inquiry.payload_shape(), PayloadShape::Unknown);
            assert_eq!(
                inquiry.payload_shape_for_profile(&profile),
                PayloadShape::Unknown
            );
            let shaped = inquiry.with_payload_shape(PayloadShape::Nibbles(4));
            let cloned = shaped.clone();
            for query in [&shaped, &cloned] {
                assert_eq!(query.payload_shape(), PayloadShape::Nibbles(4));
                assert_eq!(
                    query.payload_shape_for_profile(&profile),
                    PayloadShape::Nibbles(4)
                );
                assert_eq!(query.bytes(), bytes);
                assert_eq!(
                    query
                        .decoder()
                        .decode(&[4, 3, 2, 1])
                        .expect("decode unchanged"),
                    4
                );
            }
            assert_eq!(
                cloned
                    .with_payload_shape(PayloadShape::Bytes(7))
                    .payload_shape(),
                PayloadShape::Bytes(7)
            );
        }
    }

    #[test]
    fn classes_are_structurally_distinct_and_encoding_is_exact() {
        assert_plain::<PlainRequest>();
        assert_inquiry::<InquiryRequest<u8>>();
        assert_targeted::<TargetedRequest>();
        assert_applied::<AppliedOnlyRequest>();

        let command = PlainRequest::new(
            [0x81, 0x01, 0x02, 0xff],
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .expect("valid raw command");
        assert_eq!(command.bytes(), &[0x81, 0x01, 0x02, 0xff]);
        assert_eq!(command.timeout_class(), TimeoutClass::Quick);
        let mut encoded = [0_u8; 4];
        assert_eq!(
            command
                .write_into(CameraId::CAMERA_1, &mut encoded)
                .expect("exact encoding"),
            4
        );
        assert_eq!(encoded, [0x81, 0x01, 0x02, 0xff]);

        let inquiry = InquiryRequest::new(
            [0x81, 0x09, 0x04, 0xff],
            ResponseDecoder::from_fn(decode_first),
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .expect("valid raw inquiry");
        assert_eq!(inquiry.decoder().decode(&[0x42]).expect("decode"), 0x42);

        let context_inquiry = InquiryRequest::<u8>::with_context(
            [0x81, 0x09, 0x05, 0xff],
            0x10_u8,
            |context, payload| {
                payload
                    .first()
                    .map(|value| value.saturating_add(*context))
                    .ok_or_else(|| Error::InvalidRequest {
                        reason: "empty test payload".into(),
                    })
            },
            RetryClass::Inquiry,
            SubmissionClass::Normal,
        )
        .expect("context decoder");
        assert_eq!(context_inquiry.timeout_class(), TimeoutClass::Inquiry);
        assert_eq!(
            context_inquiry
                .decoder()
                .decode(&[0x02])
                .expect("context decode"),
            0x12
        );

        let axes = AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM);
        let targeted = TargetedRequest::new(
            [0x81, 0x01, 0x06, 0xff],
            axes,
            CommandTimeoutClass::Movement,
            RetryClass::Movement,
            SubmissionClass::User,
        )
        .expect("valid targeted operation");
        assert_eq!(targeted.affected_axes(), axes);

        let applied = AppliedOnlyRequest::with_policy(
            [0x81, 0x01, 0x07, 0xff],
            AffectedAxes::ZOOM,
            OperationPolicy::new(
                CommandTimeoutClass::Quick,
                RetryClass::Never,
                SubmissionClass::User,
            ),
        )
        .expect("valid applied-only operation");
        assert_eq!(applied.affected_axes(), AffectedAxes::ZOOM);
        assert_eq!(applied.control_class(), ControlClass::User);
    }

    #[test]
    fn construction_rejects_invalid_wire_and_axes() {
        assert!(PlainRequest::new(
            [],
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .is_err());
        assert!(PlainRequest::new(
            [0x80, 0x01, 0xff],
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .is_err());
        assert!(PlainRequest::new(
            [0x81, 0x01, 0x00],
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .is_err());
        assert!(PlainRequest::new(
            vec![0x81; MAX_BYTES + 1],
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .is_err());
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

        let error = PlainRequest::new(
            oversize,
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .expect_err("a frame longer than MAX_BYTES must be rejected");
        assert_eq!(error.kind(), crate::ErrorKind::InvalidParameter);
        assert!(
            matches!(&error, Error::InvalidRequest { reason: message } if message.contains("raw::MAX_BYTES") && message.contains(&(MAX_BYTES + 1).to_string()))
        );

        // The same fixture at exactly the bound is accepted, so the assertion
        // above cannot pass by way of an unrelated tightening.
        let at_bound = frame(MAX_BYTES);
        let accepted = PlainRequest::new(
            at_bound.clone(),
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .expect("a frame of exactly MAX_BYTES must be accepted");
        assert_eq!(accepted.bytes(), at_bound.as_slice());

        // Every constructor shares `validate_wire`, so the bound holds for the
        // axis-carrying values too.
        let inquiry_error = InquiryRequest::new(
            frame(MAX_BYTES + 1),
            ResponseDecoder::from_fn(decode_first),
            RetryClass::Inquiry,
            SubmissionClass::Normal,
        )
        .expect_err("oversize inquiry frame");
        assert_eq!(inquiry_error.kind(), crate::ErrorKind::InvalidParameter);
        assert!(matches!(
            TargetedRequest::new(
                frame(MAX_BYTES + 1),
                AffectedAxes::PAN_TILT,
                CommandTimeoutClass::Movement,
                RetryClass::Movement,
                SubmissionClass::User,
            )
            .expect_err("oversize targeted frame"),
            Error::InvalidRequest { .. }
        ));
        assert!(matches!(
            AppliedOnlyRequest::new(
                frame(MAX_BYTES + 1),
                AffectedAxes::ZOOM,
                CommandTimeoutClass::Movement,
                RetryClass::Movement,
                SubmissionClass::User,
            )
            .expect_err("oversize applied-only frame"),
            Error::InvalidRequest { .. }
        ));
    }

    /// Raw operation constructors accept the structurally non-empty axis type.
    #[test]
    fn axis_carrying_constructors_preserve_the_explicit_axis_set() {
        const VALID: [u8; 4] = [0x81, 0x01, 0x06, 0xff];
        let targeted = TargetedRequest::new(
            VALID,
            AffectedAxes::PAN_TILT,
            CommandTimeoutClass::Movement,
            RetryClass::Movement,
            SubmissionClass::User,
        )
        .expect("non-empty targeted axes");
        assert_eq!(targeted.affected_axes(), AffectedAxes::PAN_TILT);

        let applied = AppliedOnlyRequest::with_policy(
            VALID,
            AffectedAxes::ZOOM,
            OperationPolicy::new(
                CommandTimeoutClass::Movement,
                RetryClass::Movement,
                SubmissionClass::User,
            ),
        )
        .expect("non-empty applied-only axes");
        assert_eq!(applied.affected_axes(), AffectedAxes::ZOOM);
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn owner_target_is_not_inferred_or_rewritten_from_raw_bytes() {
        let command = PlainRequest::new(
            [0x82, 0x01, 0xff],
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .expect("valid frame for another explicit target");
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("profile");
        let error = crate::prepared::test_support::prepare_command(
            &command,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).individual_camera(),
                profile.clone(),
            )
            .expect("valid target registration"),
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .expect_err("the owner target must reject a mismatched wire address");
        assert!(matches!(error, Error::InvalidRequest { .. }));
        let message = error.to_string();
        assert!(message.contains("selected camera target is Camera 1"));
        assert!(!message.contains("write_into"));
        assert_eq!(command.bytes(), &[0x82, 0x01, 0xff]);
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
                    PlainRequest::new(
                        cancel,
                        CommandTimeoutClass::Quick,
                        RetryClass::Never,
                        SubmissionClass::Normal
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
                PlainRequest::new(
                    clear,
                    CommandTimeoutClass::Quick,
                    RetryClass::Never,
                    SubmissionClass::Normal
                )
                .is_err(),
                "interface clear {clear:x?} must be rejected"
            );
        }

        // The guard is shared by every raw request class, not just `PlainRequest`.
        assert!(InquiryRequest::from_fn(
            [0x81, 0x21, 0xff],
            decode_first,
            RetryClass::Never,
            SubmissionClass::Normal
        )
        .is_err());
        assert!(TargetedRequest::new(
            [0x81, 0x21, 0xff],
            AffectedAxes::PAN_TILT,
            CommandTimeoutClass::Movement,
            RetryClass::Movement,
            SubmissionClass::User,
        )
        .is_err());
        assert!(AppliedOnlyRequest::new(
            [0x81, 0x01, 0x00, 0x01, 0xff],
            AffectedAxes::ZOOM,
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::User,
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
                PlainRequest::new(
                    frame,
                    CommandTimeoutClass::Quick,
                    RetryClass::Never,
                    SubmissionClass::Normal
                )
                .is_ok(),
                "legitimate custom frame {frame:x?} must stay admissible"
            );
        }
    }

    /// #810: raw admission and the Command Cancel encoder read the one
    /// `system::COMMAND_CANCEL` opcode, so every frame the owner's encoder
    /// emits for any camera and socket is exactly a frame admission refuses.
    #[test]
    fn every_encoded_command_cancel_is_refused_by_raw_admission() {
        use crate::{
            command::{encode::WireEncode, system_wire::CommandCancelCommand},
            ViscaSocket,
        };

        for id in 1..=7 {
            let camera = CameraId::new(id).expect("individual camera");
            for socket in [ViscaSocket::S1, ViscaSocket::S2] {
                let mut frame = [0_u8; 8];
                let len =
                    WireEncode::write_into(&CommandCancelCommand::new(socket), camera, &mut frame)
                        .expect("cancel frame fits");
                let frame = &frame[..len];
                assert!(
                    reject_owner_only_primitive(frame).is_err(),
                    "encoded cancel {frame:x?} must be refused by raw admission"
                );
            }
        }
    }

    #[test]
    fn construction_rejects_trailing_owner_primitives_and_compound_frames() {
        let rejects = [
            // The first terminator is consumed by the camera even when bytes
            // trail the owner-only primitive.
            &[0x81, 0x21, 0xff, 0x00, 0xff][..],
            &[0x81, 0x01, 0x00, 0x01, 0xff, 0x00, 0xff][..],
            &[0x88, 0x30, 0x01, 0xff, 0x00, 0xff][..],
            // A raw request has one frame/lifecycle, not two commands.
            &[
                0x81, 0x01, 0x04, 0x00, 0x02, 0xff, 0x81, 0x01, 0x04, 0x00, 0x03, 0xff,
            ][..],
            &[
                0x81, 0x01, 0x04, 0x00, 0x02, 0xff, 0x82, 0x01, 0x04, 0x00, 0x03, 0xff,
            ][..],
        ];
        for frame in rejects {
            assert!(
                PlainRequest::new(
                    frame,
                    CommandTimeoutClass::Quick,
                    RetryClass::Never,
                    SubmissionClass::Normal,
                )
                .is_err(),
                "raw frame {frame:x?} must be rejected",
            );
        }

        // A second 0xff is legal payload when it is not followed by an
        // apparent VISCA address.
        assert!(PlainRequest::new(
            [0x81, 0x01, 0x04, 0x3f, 0x01, 0xff, 0xff],
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        )
        .is_ok());
    }

    /// The reply-shape axis defaults to `AckThenCompletion` and threads through
    /// policy/spec plus every compatible raw command's `Request` hook. NoReply
    /// remains intentionally plain-command-only.
    #[test]
    fn reply_shape_defaults_and_threads_through_policy_and_commands() {
        // The default keeps existing raw code unchanged.
        let default_policy = CommandPolicy::new(
            CommandTimeoutClass::Quick,
            RetryClass::Never,
            SubmissionClass::Normal,
        );
        assert_eq!(
            default_policy.reply_shape(),
            RawReplyShape::AckThenCompletion
        );
        assert_eq!(RawReplyShape::default(), RawReplyShape::AckThenCompletion);
        assert_eq!(
            PlainRequest::new(
                [0x81, 0x01, 0x02, 0xff],
                CommandTimeoutClass::Quick,
                RetryClass::Never,
                SubmissionClass::Normal,
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

            // Each command reports the shape through its inherent accessor and
            // the `Request::reply_shape` hook preparation reads.
            let plain = PlainRequest::with_policy([0x81, 0x01, 0x02, 0xff], policy).expect("plain");
            assert_eq!(plain.reply_shape(), shape);
            assert_eq!(Request::reply_shape(&plain), shape);
        }
        for shape in [
            OperationReplyShape::AckThenCompletion,
            OperationReplyShape::CompletionOnly,
        ] {
            let policy = OperationPolicy::new(
                CommandTimeoutClass::Quick,
                RetryClass::Never,
                SubmissionClass::Normal,
            )
            .with_reply_shape(shape);
            let expected = RawReplyShape::from(shape);
            let targeted = TargetedRequest::with_policy(
                [0x81, 0x01, 0x06, 0xff],
                AffectedAxes::PAN_TILT,
                policy,
            )
            .expect("targeted");
            assert_eq!(targeted.reply_shape(), expected);
            assert_eq!(Request::reply_shape(&targeted), expected);
            let applied = AppliedOnlyRequest::with_policy(
                [0x81, 0x01, 0x07, 0xff],
                AffectedAxes::ZOOM,
                policy,
            )
            .expect("applied");
            assert_eq!(applied.reply_shape(), expected);
            assert_eq!(Request::reply_shape(&applied), expected);
        }
    }
}
