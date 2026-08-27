//! Inert values crossing the deterministic protocol-engine boundary.

use std::{num::NonZeroU64, sync::Arc, time::Duration};

use smallvec::SmallVec;

use crate::{
    command::semantics::WriteOnlyState, raw::INLINE_BYTES, raw::MAX_BYTES, CameraId, Error,
    ViscaSocket,
};

/// Maximum number of Sony sequences retained for one request across retries.
pub(crate) const MAX_SEQUENCE_HISTORY: usize = 8;

/// Prepared wire data. It has no target, routing, or scheduling authority.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncodedMessage {
    bytes: SmallVec<[u8; INLINE_BYTES]>,
}

impl EncodedMessage {
    /// Copies validated, terminated VISCA bytes into reusable inert storage.
    pub(crate) fn new(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::ResponseTooLarge {
                max_size: MAX_BYTES,
            });
        }
        if bytes.last().copied() != Some(0xff) {
            return Err(Error::InvalidRequest(
                "encoded VISCA message must end in 0xff".into(),
            ));
        }
        Ok(Self {
            bytes: SmallVec::from_slice(bytes),
        })
    }

    /// Returns the inert wire bytes.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Whether the message uses only its inline allocation.
    #[cfg(test)]
    pub(crate) fn is_inline(&self) -> bool {
        !self.bytes.spilled()
    }
}

impl std::fmt::Debug for EncodedMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("EncodedMessage")
            .field(&self.bytes.as_slice())
            .finish()
    }
}

/// Private identity allocated only when a request becomes authoritative state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RequestId(NonZeroU64);

impl RequestId {
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }

    pub(super) const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// Identity of one exact request or cancellation transport write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TransmissionId(NonZeroU64);

impl TransmissionId {
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }

    pub(super) const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// Owner-local admission reply token. It never authorizes lifecycle mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AdmissionTicket(pub(crate) u64);

/// Immutable lifetime generation used by every derived engine index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GenerationTicket(pub(crate) u64);

/// Lazy-deletion ticket for one exact ready-queue insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct QueueTicket {
    pub(crate) request: RequestId,
    pub(crate) generation: GenerationTicket,
    pub(crate) queue_generation: u64,
}

/// Classifies an inquiry for parsed-content routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct InquiryRoute(pub(crate) u16);

impl InquiryRoute {
    /// A parser could not classify the response content.
    pub(crate) const UNKNOWN: Self = Self(0);
}

/// Public request semantics lower to this private scheduling classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlClass {
    Background,
    Normal,
    User,
    Urgent,
}

impl ControlClass {
    pub(super) const fn priority_index(self) -> usize {
        match self {
            Self::Background => 0,
            Self::Normal => 1,
            Self::User => 2,
            Self::Urgent => 3,
        }
    }
}

/// Explicit request-specific transport pacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControlPolicy {
    pub(crate) class: ControlClass,
    pub(crate) minimum_spacing: Duration,
}

impl Default for ControlPolicy {
    fn default() -> Self {
        Self {
            class: ControlClass::Normal,
            minimum_spacing: Duration::ZERO,
        }
    }
}

/// Whether an already-transmitted socket command supports protocol cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancellationPolicy {
    Supported,
    Unsupported,
}

/// All scheduler deadlines for one request class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimeoutPolicy {
    pub(crate) ack: Duration,
    pub(crate) completion: Duration,
    pub(crate) inquiry: Duration,
    pub(crate) cancellation: Duration,
    pub(crate) ambiguity: Duration,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            ack: Duration::from_millis(500),
            completion: Duration::from_secs(5),
            inquiry: Duration::from_secs(1),
            cancellation: Duration::from_secs(1),
            ambiguity: Duration::from_secs(1),
        }
    }
}

/// Context-sensitive bounded retry rules, completely decided before admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetryPolicy {
    pub(crate) max_retries: u32,
    pub(crate) initial_backoff: Duration,
    pub(crate) maximum_backoff: Duration,
    pub(crate) total_budget: Duration,
    pub(crate) ack_timeout: bool,
    pub(crate) completion_timeout: bool,
    pub(crate) inquiry_timeout: bool,
    pub(crate) buffer_full: bool,
    pub(crate) movement_not_executable: bool,
    pub(crate) builtin_inquiry_syntax: bool,
}

impl RetryPolicy {
    pub(crate) const NEVER: Self = Self {
        max_retries: 0,
        initial_backoff: Duration::ZERO,
        maximum_backoff: Duration::ZERO,
        total_budget: Duration::ZERO,
        ack_timeout: false,
        completion_timeout: false,
        inquiry_timeout: false,
        buffer_full: false,
        movement_not_executable: false,
        builtin_inquiry_syntax: false,
    };
}

/// Every immutable protocol policy carried by an admitted request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestContext {
    pub(crate) target: CameraId,
    pub(crate) timeout: TimeoutPolicy,
    pub(crate) retry: RetryPolicy,
    pub(crate) control: ControlPolicy,
    pub(crate) cancellation: CancellationPolicy,
}

/// Maximum number of scalar values carried by one applied-state effect.
pub(crate) const MAX_APPLIED_STATE_VALUES: usize = 4;

/// Bounded, inline value storage for a known state replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedStateValue {
    pub(crate) values: [i64; MAX_APPLIED_STATE_VALUES],
    pub(crate) value_count: u8,
}

/// Compatibility name for the inline value carried by a `Set` action.
pub(crate) type InlineStateValue = AppliedStateValue;

impl AppliedStateValue {
    pub(crate) fn new(values: &[i64]) -> Result<Self, Error> {
        if values.len() > MAX_APPLIED_STATE_VALUES {
            return Err(Error::InvalidRequest(
                "applied-state projection accepts at most four values".into(),
            ));
        }
        let mut stored = [0; MAX_APPLIED_STATE_VALUES];
        stored[..values.len()].copy_from_slice(values);
        Ok(Self {
            values: stored,
            value_count: values.len() as u8,
        })
    }

    pub(crate) fn as_slice(&self) -> &[i64] {
        &self.values[..usize::from(self.value_count)]
    }
}

/// Closed action model for one write-only state key.
///
/// The engine carries this inert value across the lifecycle boundary and
/// emits it only when the exact request reaches [`RuntimeOutcome::Applied`].
/// `Clear` is deliberately distinct from `Invalidate`: the former records a
/// known absence while the latter records that the state can no longer be
/// trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppliedStateProjection {
    /// Replace the key with a bounded known value.
    Set {
        key: WriteOnlyState,
        value: AppliedStateValue,
    },
    /// Record a known absence for the key.
    Clear {
        key: WriteOnlyState,
        /// Optional bounded discriminator for a key-local clear operation.
        /// Pan/tilt limit clears carry the corner byte here; a legacy whole-key
        /// clear can still use an empty value.
        value: AppliedStateValue,
    },
    /// Record that the key's value is unknown.
    Invalidate { key: WriteOnlyState },
}

/// Alias emphasizing that projections are closed state actions.
pub(crate) type AppliedStateAction = AppliedStateProjection;

impl AppliedStateProjection {
    /// Constructs a known replacement value. This alias keeps construction
    /// concise at crate-internal preparation/test seams.
    pub(crate) fn new(key: WriteOnlyState, values: &[i64]) -> Result<Self, Error> {
        Self::set(key, values)
    }

    pub(crate) fn set(key: WriteOnlyState, values: &[i64]) -> Result<Self, Error> {
        Ok(Self::Set {
            key,
            value: AppliedStateValue::new(values)?,
        })
    }

    pub(crate) const fn clear(key: WriteOnlyState) -> Self {
        Self::Clear {
            key,
            value: AppliedStateValue {
                values: [0; MAX_APPLIED_STATE_VALUES],
                value_count: 0,
            },
        }
    }

    pub(crate) fn clear_with_values(key: WriteOnlyState, values: &[i64]) -> Result<Self, Error> {
        Ok(Self::Clear {
            key,
            value: AppliedStateValue::new(values)?,
        })
    }

    pub(crate) const fn invalidate(key: WriteOnlyState) -> Self {
        Self::Invalidate { key }
    }

    pub(crate) const fn state(self) -> WriteOnlyState {
        match self {
            Self::Set { key, .. } | Self::Clear { key, .. } | Self::Invalidate { key } => key,
        }
    }

    /// Alias for the state key carried by this effect.
    pub(crate) const fn key(self) -> WriteOnlyState {
        self.state()
    }

    pub(crate) const fn is_known(self) -> bool {
        matches!(self, Self::Set { .. } | Self::Clear { .. })
    }

    pub(crate) const fn kind(self) -> crate::command::semantics::AppliedStateEffectKind {
        match self {
            Self::Set { .. } => crate::command::semantics::AppliedStateEffectKind::Set,
            Self::Clear { .. } => crate::command::semantics::AppliedStateEffectKind::Clear,
            Self::Invalidate { .. } => {
                crate::command::semantics::AppliedStateEffectKind::Invalidate
            }
        }
    }
}

/// The engine's only two protocol execution classes.
#[derive(Debug, Clone)]
pub(crate) enum RuntimeRequest {
    Command {
        wire: Arc<EncodedMessage>,
        context: RequestContext,
        applied_state: Option<AppliedStateProjection>,
    },
    Inquiry {
        wire: Arc<EncodedMessage>,
        context: RequestContext,
        route: InquiryRoute,
    },
}

impl RuntimeRequest {
    pub(crate) const fn context(&self) -> &RequestContext {
        match self {
            Self::Command { context, .. } | Self::Inquiry { context, .. } => context,
        }
    }

    pub(crate) fn wire(&self) -> &Arc<EncodedMessage> {
        match self {
            Self::Command { wire, .. } | Self::Inquiry { wire, .. } => wire,
        }
    }

    pub(crate) const fn is_inquiry(&self) -> bool {
        matches!(self, Self::Inquiry { .. })
    }

    pub(crate) const fn inquiry_route(&self) -> Option<InquiryRoute> {
        match self {
            Self::Inquiry { route, .. } => Some(*route),
            Self::Command { .. } => None,
        }
    }

    pub(crate) const fn applied_state(&self) -> Option<AppliedStateProjection> {
        match self {
            Self::Command { applied_state, .. } => *applied_state,
            Self::Inquiry { .. } => None,
        }
    }
}

/// Transport framing/correlation capability for one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnvelopeKind {
    Raw,
    Sony,
}

/// Whether one failed write can corrupt following frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransportKind {
    Datagram,
    Stream,
}

/// Immutable policy registered for one camera target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TargetPolicy {
    pub(crate) command_sockets: u8,
    pub(crate) cancellation: CancellationPolicy,
}

/// Session-wide scheduling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProtocolPolicy {
    pub(crate) capacity: usize,
    pub(crate) envelope: EnvelopeKind,
    pub(crate) transport: TransportKind,
    pub(crate) inquiry_capacity: usize,
    pub(crate) command_spacing: Duration,
    pub(crate) inquiry_spacing: Duration,
    pub(crate) inquiry_cooldown: Duration,
}

/// Identifies full-width and known-truncated Sony envelope correlation data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EnvelopeSequence {
    pub(crate) value: u32,
    pub(crate) width: SequenceWidth,
}

/// Width supplied by scheduler-independent envelope parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceWidth {
    Full32,
    Lower16,
}

/// Owned scheduler-independent decoded response data.
///
/// `Ack` and `Completion` carry an *optional* socket because the socket nibble
/// is genuinely optional on the wire: a camera may answer `90 40 FF` /
/// `90 50 FF` with no socket. The scheduler, not the transport adapter, decides
/// what an absent socket means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodedResponse {
    Ack {
        socket: Option<ViscaSocket>,
    },
    Completion {
        socket: Option<ViscaSocket>,
    },
    InquiryReply {
        route: Option<InquiryRoute>,
        payload: SmallVec<[u8; INLINE_BYTES]>,
    },
    Error {
        socket: Option<ViscaSocket>,
        code: u8,
    },
    NetworkChange,
    Unknown,
}

/// A decoded frame owns its target and parsed data; it borrows no scheduler state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedFrame {
    pub(crate) target: CameraId,
    pub(crate) sequence: Option<EnvelopeSequence>,
    pub(crate) response: DecodedResponse,
}

/// Metadata returned by a successful exact transport write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransmissionMeta {
    pub(crate) sequence: Option<u32>,
}

/// One exact write requested by the engine.
#[derive(Debug, Clone)]
pub(crate) enum Transmission {
    Request {
        target: CameraId,
        wire: Arc<EncodedMessage>,
    },
    Cancel {
        target: CameraId,
        socket: ViscaSocket,
    },
}

/// Why an otherwise valid input was deliberately inert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IgnoreReason {
    UnknownRequest,
    StaleQueueTicket,
    StaleTransmission,
    IncompatibleTransmissionResult,
    MalformedFrame,
    UnmatchedFrame,
    UnmatchedSequencedFrame,
    TargetIncompatibleSequence,
    AmbiguousLower16Sequence,
    SocketConflict,
    DuplicateCancellation,
    SessionNotRunning,
}

/// Scheduler timeout source retained in terminal diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeadlineKind {
    CancellationAmbiguity,
    Ack,
    Completion,
    InquiryReply,
    CancellationResolution,
}

/// Public-observer-independent terminal engine result.
#[derive(Debug, Clone)]
pub(crate) enum RuntimeOutcome {
    Applied,
    Reply {
        route: Option<InquiryRoute>,
        payload: SmallVec<[u8; INLINE_BYTES]>,
    },
    Cancelled,
    Failed(Error),
}

/// Exact result observed by the cancellation token layer.
#[derive(Debug, Clone)]
pub(crate) enum CancellationObservation {
    Recorded,
    Cancelled,
    Completed,
    Failed(Error),
}

/// Explicit session termination source.
#[derive(Debug, Clone)]
pub(crate) enum ShutdownReason {
    Explicit,
    TransportClosed { reason: Option<Box<str>> },
    FramingFailure { reason: Box<str> },
}

/// Session state is terminal except for `Running`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionState {
    Running,
    Closed,
    Shutdown,
    Poisoned,
}

/// Ordered boundary inputs to the pure state machine.
#[derive(Debug)]
pub(crate) enum Input {
    Admit {
        ticket: AdmissionTicket,
        request: RuntimeRequest,
    },
    TransmissionFinished {
        transmission: TransmissionId,
        result: Result<TransmissionMeta, Error>,
    },
    Frame(DecodedFrame),
    Cancel {
        id: RequestId,
    },
    /// One receive-side transport failure the owner has already classified as
    /// transient: the session survives it and every command still waiting for
    /// its ACK is retried under its own bounded retry policy.
    ///
    /// A receive that proves the session is finished never reaches the engine
    /// this way; it arrives as [`Input::Close`], [`Input::Poison`], or
    /// [`Input::Shutdown`] instead.
    ReceiveFault {
        error: Error,
    },
    Close {
        reason: Option<Box<str>>,
    },
    Poison {
        reason: Box<str>,
    },
    Shutdown(ShutdownReason),
    Wake,
}

/// Explicit authoritative phase. No driver owns a parallel phase copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Ready {
        ticket: QueueTicket,
    },
    Sending {
        transmission: TransmissionId,
        started_at: std::time::Instant,
    },
    AwaitingAck {
        sent_at: std::time::Instant,
        deadline: std::time::Instant,
    },
    Executing {
        socket: ViscaSocket,
        started_at: std::time::Instant,
        deadline: std::time::Instant,
    },
    AwaitingReply {
        sent_at: std::time::Instant,
        deadline: std::time::Instant,
    },
    Backoff {
        ready_at: std::time::Instant,
        queue_generation: u64,
    },
    AwaitingCancellationResolution {
        socket: ViscaSocket,
        deadline: std::time::Instant,
    },
    AwaitingLateAck {
        deadline: std::time::Instant,
    },
}

/// Cancellation is a substate of the same authoritative request entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancelState {
    None,
    Requested {
        ambiguity_deadline: std::time::Instant,
    },
    Sending {
        transmission: TransmissionId,
        socket: ViscaSocket,
        ambiguity_deadline: std::time::Instant,
    },
    AwaitingTerminal {
        ambiguity_deadline: std::time::Instant,
        observation_deadline: std::time::Instant,
    },
    ObservationFailed {
        socket: ViscaSocket,
        ambiguity_deadline: std::time::Instant,
    },
}

/// Ordered, inert outputs. The owner performs I/O and observer/cache delivery.
#[derive(Debug, Clone)]
pub(crate) enum Effect {
    Admitted {
        ticket: AdmissionTicket,
        id: RequestId,
    },
    AdmissionRejected {
        ticket: AdmissionTicket,
        error: Error,
    },
    Transition {
        id: RequestId,
        from: Phase,
        to: Phase,
        cancellation: CancelState,
    },
    Transmit {
        transmission: TransmissionId,
        request: RequestId,
        kind: Transmission,
    },
    RetryScheduled {
        id: RequestId,
        attempt: u32,
        ready_at: std::time::Instant,
    },
    CancellationRecorded {
        id: RequestId,
    },
    CancellationObservation {
        id: RequestId,
        observation: CancellationObservation,
    },
    AppliedState {
        effect: AppliedStateEffect,
    },
    Terminal {
        id: RequestId,
        outcome: RuntimeOutcome,
    },
    SessionChanged {
        from: SessionState,
        to: SessionState,
    },
    Ignored(IgnoreReason),
}

/// Deterministic cache projection; applying it remains outside the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedStateEffect {
    pub(crate) request: RequestId,
    pub(crate) target: CameraId,
    pub(crate) projection: AppliedStateProjection,
}
