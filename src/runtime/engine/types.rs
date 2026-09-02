//! Inert values crossing the deterministic protocol-engine boundary.

use std::{num::NonZeroU64, sync::Arc, time::Duration};

use smallvec::SmallVec;

use crate::{
    command::semantics::WriteOnlyState, protocol::framer::RawIncompletePrefix, raw::INLINE_BYTES,
    raw::MAX_BYTES, CameraId, Error, ViscaSocket,
};

/// Maximum number of Sony sequences retained for one request across retries.
pub(crate) const MAX_SEQUENCE_HISTORY: usize = 8;

/// Scheduler work permitted when an external-input turn is completed.
///
/// Both owner shells use these same three boundaries. A complete turn runs
/// deadlines, pending cancellation work, and one ordinary dispatch. A
/// deadline-only turn withholds that final dispatch while an exact request is
/// reconsidered. An input-only turn preserves retained wire
/// evidence ahead of every deadline at the same sampled instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EngineTurn {
    run_due: bool,
    dispatch: bool,
}

impl EngineTurn {
    pub(crate) const COMPLETE: Self = Self {
        run_due: true,
        dispatch: true,
    };
    #[allow(dead_code)] // Used by blocking owner turns; async-only builds omit them.
    pub(crate) const DEADLINES_ONLY: Self = Self {
        run_due: true,
        dispatch: false,
    };
    pub(crate) const INPUT_ONLY: Self = Self {
        run_due: false,
        dispatch: false,
    };

    pub(super) const fn runs_due(self) -> bool {
        self.run_due
    }

    pub(super) const fn allows_dispatch(self) -> bool {
        self.dispatch
    }
}

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
    // Read back by `runtime::engine::tests` when it pins allocator wraparound; no
    // production caller yet (#636).
    #[allow(dead_code)]
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

/// The reply protocol one admitted command declares it will receive.
///
/// This is the private lowering of the public [`crate::raw::RawReplyShape`]. The
/// engine never infers it from wire bytes: preparation lowers the caller's
/// explicit declaration into this fact, exactly as it does the envelope and
/// control class. Built-in commands and inquiries always lower to
/// [`Self::AckThenCompletion`]; only a custom (raw or downstream) command can
/// declare another shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ReplyShape {
    /// The camera acknowledges, is assigned a socket, then completes. The
    /// default and the shape of every built-in command.
    #[default]
    AckThenCompletion,
    /// The camera replies with a completion/terminal but no acknowledgement, so
    /// the command owns no socket and never enters the unacknowledged-ACK gate.
    CompletionOnly,
    /// The command expects nothing back and reaches its terminal on a successful
    /// local transport write. Public raw operations reject this shape because
    /// that write is not protocol application; it is reserved for plain
    /// fire-and-forget execution.
    NoReply,
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
    // Consumed by `runtime::owner::blocking_transport` and the engine tests; the
    // async legs compile neither, so it reads as dead there (#636).
    #[allow(dead_code)]
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
    /// The reply protocol this command declared. Inquiry preparation admits
    /// only the default shape because inquiries always await a reply; commands
    /// honor their declared shape in the lifecycle state machine.
    pub(crate) reply_shape: ReplyShape,
}

/// Maximum number of scalar values carried by one applied-state effect.
pub(crate) const MAX_APPLIED_STATE_VALUES: usize = 4;

/// Bounded, inline value storage for a known state replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedStateValue {
    pub(crate) values: [i64; MAX_APPLIED_STATE_VALUES],
    pub(crate) value_count: u8,
}

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

impl AppliedStateProjection {
    pub(crate) fn set(key: WriteOnlyState, values: &[i64]) -> Result<Self, Error> {
        Ok(Self::Set {
            key,
            value: AppliedStateValue::new(values)?,
        })
    }

    // Value-less clear used by the engine and owner tests; production preparation
    // builds clears through `clear_with_values` (#636).
    #[allow(dead_code)]
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

    // Consumed by `runtime::owner::tests`; the applied-state observer facade is
    // its production caller (#636).
    #[allow(dead_code)]
    pub(crate) const fn is_known(self) -> bool {
        matches!(self, Self::Set { .. } | Self::Clear { .. })
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
    /// Profile-derived skew window retained only after an unkeyed raw inquiry
    /// can still produce a late reply (#712).
    pub(crate) raw_inquiry_release_hold: Duration,
    /// Maximum time retained raw stream evidence may wait for a completing
    /// tail after a correlation hold becomes releasable (#713).
    pub(crate) raw_release_grace: Duration,
    /// Opt-in strict recovery mode for the raw envelope.
    ///
    /// When `false` (the default), a raw command whose ACK or completion can no
    /// longer be confirmed — a lost ACK/completion datagram, a spent retry
    /// budget, or an expired cancellation-ambiguity window — fails on its own
    /// with [`Error::UnsequencedCommandUnconfirmed`] while its socket or
    /// unacknowledged-command slot is quarantined for the ambiguity window so a
    /// late reply cannot misbind; the session and every unrelated request keep
    /// running. When `true`, the same events instead poison the whole session
    /// (the pre-fix behavior), for deployments that would rather hard-fail than
    /// risk a subtle correlation error. This flag is meaningless for the Sony
    /// envelope, whose sequence correlation never needs the quarantine.
    pub(crate) strict_unconfirmed_poison: bool,
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
    SonyControl {
        code: u16,
    },
    NetworkChange,
    Unknown,
}

/// Target-local correlation evidence whose ambiguity window becomes due in an
/// owner turn.
///
/// This deliberately retains the identity which is being released.  A bool per
/// target loses the distinction between a released socket-one request and a
/// still-live request on the other socket, which is exactly the distinction a
/// byte-stream owner needs before it advances due work and dispatches a
/// successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RawCorrelationRelease {
    terminal_all: bool,
    inquiry_unkeyed: bool,
    pre_ack_unkeyed: bool,
    exact_sockets: [bool; 2],
}

impl RawCorrelationRelease {
    pub(crate) const fn is_empty(self) -> bool {
        !self.terminal_all
            && !self.inquiry_unkeyed
            && !self.pre_ack_unkeyed
            && !self.exact_sockets[0]
            && !self.exact_sockets[1]
    }

    pub(crate) const fn terminal_all(self) -> bool {
        self.terminal_all
    }

    // Queried by async/blocking retained-prefix integrations; engine-only
    // feature combinations construct the scope but do not inspect it.
    #[allow(dead_code)]
    pub(crate) const fn inquiry_unkeyed(self) -> bool {
        self.inquiry_unkeyed
    }

    // See `inquiry_unkeyed`: this remains part of the crate-private owner API.
    #[allow(dead_code)]
    pub(crate) const fn pre_ack_unkeyed(self) -> bool {
        self.pre_ack_unkeyed
    }

    pub(crate) fn exact_socket(self, socket: ViscaSocket) -> bool {
        self.exact_sockets[socket.as_index()]
    }

    pub(super) fn release_terminal_all(&mut self) {
        self.terminal_all = true;
    }

    pub(super) fn release_inquiry_unkeyed(&mut self) {
        self.inquiry_unkeyed = true;
    }

    pub(super) fn release_pre_ack_unkeyed(&mut self) {
        self.pre_ack_unkeyed = true;
    }

    pub(super) fn release_exact_socket(&mut self, socket: ViscaSocket) {
        self.exact_sockets[socket.as_index()] = true;
    }
}

/// The complete set of raw correlation releases due in one owner turn.
///
/// Index zero is intentionally retained as an inert slot, matching the engine
/// target table.  Real camera identifiers occupy one through seven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RawCorrelationReleaseSet {
    targets: [RawCorrelationRelease; 9],
}

impl RawCorrelationReleaseSet {
    pub(crate) const fn is_empty(self) -> bool {
        let mut index = 0;
        while index < self.targets.len() {
            if !self.targets[index].is_empty() {
                return false;
            }
            index += 1;
        }
        true
    }

    pub(crate) fn for_target(self, target: CameraId) -> RawCorrelationRelease {
        self.targets[target.id() as usize]
    }

    pub(super) fn for_target_mut(&mut self, target: CameraId) -> &mut RawCorrelationRelease {
        &mut self.targets[target.id() as usize]
    }
}

/// What a byte-stream framer can prove about bytes it retained at a raw
/// correlation-release boundary.
///
/// It deliberately does not describe a complete frame: complete frames always
/// enter the engine before due work, preserving the input-first boundary rule.
/// The socket nibble of an ACK is not included as identity because it expresses
/// an assignment preference, not ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawPrefixEvidence {
    Complete,
    Incomplete {
        target: CameraId,
        kind: RawIncompletePrefix,
    },
}

/// The safe action for retained raw bytes at a correlation-release boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawPrefixDisposition {
    /// No raw correlation release is due; leave normal owner processing alone.
    NoRelease,
    /// Input must be consumed before due work, or the prefix is too ambiguous
    /// for release and the owner must fail closed rather than dispatch.
    Defer,
    /// The retained prefix belongs to the released correlation and must be
    /// dropped before advancing due work.
    Discard,
    /// Due work may run while the retained bytes remain available for the next
    /// receive turn; they cannot bind the released correlation to a successor.
    ReleasePreserving,
}

/// Engine-owned decision for retained input at a raw-correlation release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawReleaseGateAction {
    /// The release may advance and ordinary scheduling may resume.
    Advance,
    /// Keep the old correlation scope alive and await input until this instant.
    AwaitInputUntil(std::time::Instant),
    /// Discard the first retained raw fragment, record it, and classify again.
    DiscardFirst,
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
        /// `None` asks the envelope to allocate a sequence for the first
        /// logical transmission (or for a request whose first write never
        /// succeeded).  A retry carries the request's last successful Sony
        /// sequence so the exact logical message is replayed byte-for-byte.
        requested_sequence: Option<u32>,
    },
    Cancel {
        target: CameraId,
        socket: ViscaSocket,
        /// Cancellation is a separate logical message and therefore always
        /// asks Sony framing for a fresh sequence.
        requested_sequence: Option<u32>,
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

/// Which of a request's own protocol deadlines expired.
///
/// These are exactly the three deadlines 1.x counted as timeouts. Cancellation
/// deadlines are deliberately not part of this vocabulary: they resolve a
/// quarantine rather than the request's own protocol progress, and they are
/// already reported through [`Effect::CancellationObservation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeadlineKind {
    /// The acknowledgement deadline for a sent command expired.
    Ack,
    /// The completion deadline for an acknowledged command expired.
    Completion,
    /// The reply deadline for a sent inquiry expired.
    InquiryReply,
}

/// Public-observer-independent terminal engine result.
#[derive(Debug, Clone)]
pub(crate) enum RuntimeOutcome {
    /// A raw plain `NoReply` command was accepted by the local transport.
    ///
    /// This deliberately differs from [`Self::Applied`]: the camera did not
    /// supply protocol evidence that it accepted or applied the command.
    Written,
    Applied,
    Reply {
        // Reported to the owner's inquiry decoder; the blocking-only legs never read
        // the route back out of the outcome (#636).
        #[allow(dead_code)]
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
    /// transient at the transport layer. The engine still applies the
    /// envelope-specific evidence rule: a Sony request may retry with its exact
    /// sequence, while a successfully sent raw command awaiting ACK is, by
    /// default, left untouched to ride to its own ACK deadline — a transient
    /// fault consumes nothing and a raw command has no sequence to safely replay
    /// (issue #671). The strict `strict_unconfirmed_poison` opt-in instead
    /// poisons the session on such a fault.
    ///
    /// A receive that proves the session is finished never reaches the engine
    /// this way; it arrives as [`Input::Shutdown`] instead.
    ReceiveFault {
        error: Error,
    },
    Shutdown(ShutdownReason),
    // A pure "re-evaluate deadlines now" input. The owners call `advance`
    // directly; only `runtime::engine::tests` drives it as an input (#636).
    #[allow(dead_code)]
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
    /// A completion-only raw command that was sent but never acknowledged, so it
    /// owns no socket. It awaits its completion frame under the completion
    /// deadline and, unlike [`Self::AwaitingAck`], has no ACK-timeout path.
    AwaitingCompletion {
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
    /// One of a request's own protocol deadlines expired.
    ///
    /// `will_retry` is the engine's actual decision for this expiry rather than
    /// the policy that motivated it: it is true exactly when the expiry produced
    /// a [`Effect::RetryScheduled`] for the same request. A subscriber therefore
    /// never has to infer the decision from a [`Effect::Transition`] plus the
    /// absence of a retry, which is what 1.x's
    /// `SchedulerAction::Timeout { will_retry }` carried directly.
    DeadlineExpired {
        id: RequestId,
        deadline: DeadlineKind,
        will_retry: bool,
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
