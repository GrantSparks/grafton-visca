//! Inert values crossing the deterministic protocol-engine boundary.
use crate::target::{PerTarget, TargetSlot};

use std::{
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant},
};

use smallvec::SmallVec;

use crate::{
    protocol::framer::RawIncompletePrefix, raw::INLINE_BYTES, raw::MAX_BYTES, Error, ViscaSocket,
};

/// Maximum number of Sony sequences retained for one request across retries.
pub(crate) const MAX_SEQUENCE_HISTORY: usize = 8;

/// Scheduler work permitted when an external-input turn is completed.
///
/// Both owner shells use these two boundaries. A complete turn runs deadlines,
/// pending cancellation, and one ordinary dispatch. An input-only turn retains
/// wire evidence ahead of deadlines at the same sampled instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EngineTurn {
    Complete,
    InputOnly,
}

impl EngineTurn {
    pub(super) const fn runs_due(self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Prepared wire data. It has no target, routing, or scheduling authority.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncodedMessage {
    bytes: SmallVec<[u8; INLINE_BYTES]>,
}

impl EncodedMessage {
    /// Copies validated, terminated VISCA bytes into reusable inert storage.
    #[cfg(test)]
    pub(crate) fn new(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::ResponseTooLarge {
                max_size: MAX_BYTES,
            });
        }
        if bytes.last().copied() != Some(0xff) {
            return Err(Error::InvalidRequest {
                reason: "encoded VISCA message must end in 0xff".into(),
            });
        }
        Ok(Self {
            bytes: SmallVec::from_slice(bytes),
        })
    }

    /// Writes directly into bounded inert storage, retaining a large frame's
    /// one allocation instead of copying it into a second buffer.
    pub(crate) fn encode(
        capacity: usize,
        write: impl FnOnce(&mut [u8]) -> Result<usize, Error>,
    ) -> Result<Self, Error> {
        if capacity > MAX_BYTES {
            return Err(Error::InvalidRequest {
                reason: "encoded VISCA storage exceeds raw::MAX_BYTES".into(),
            });
        }
        let mut bytes = SmallVec::from_elem(0, capacity);
        let written = write(&mut bytes)?;
        if written > capacity {
            return Err(Error::InvalidRequest {
                reason: "encoder returned a length beyond its supplied buffer".into(),
            });
        }
        bytes.truncate(written);
        if bytes.last().copied() != Some(0xff) {
            return Err(Error::InvalidRequest {
                reason: "encoded VISCA message must end in 0xff".into(),
            });
        }
        Ok(Self { bytes })
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
/// Never reused within a session: its ordering is admission ordering, and an
/// entry lookup by this identity also proves retained evidence's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RequestId(NonZeroU64);

impl RequestId {
    pub(crate) const fn as_nonzero(self) -> NonZeroU64 {
        self.0
    }

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
    /// The raw identity, read back by `runtime::engine::tests` when it pins
    /// allocator wraparound.
    #[cfg(test)]
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }

    pub(super) const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// Test-only admission reply token for the input/effect compatibility shim.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AdmissionTicket(pub(crate) u64);

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

/// A strictly positive interval. Absolute representability is checked against
/// the owner's sampled clock when a policy or request crosses the engine boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NonZeroDuration(Duration);

impl NonZeroDuration {
    pub(crate) fn new(duration: Duration) -> Result<Self, Error> {
        if duration.is_zero() {
            return Err(Error::InvalidRequest {
                reason: "engine timeout durations must be non-zero".into(),
            });
        }
        Ok(Self(duration))
    }

    #[cfg(test)]
    pub(crate) const fn from_millis(milliseconds: NonZeroU64) -> Self {
        Self(Duration::from_millis(milliseconds.get()))
    }

    pub(crate) const fn get(self) -> Duration {
        self.0
    }
}

/// All scheduler deadlines for one request class. Zero ambiguity, including
/// when a virtual clock lands exactly on the ACK deadline, is unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimeoutPolicy {
    pub(crate) ack: NonZeroDuration,
    pub(crate) completion: NonZeroDuration,
    pub(crate) inquiry: NonZeroDuration,
    pub(crate) cancellation: NonZeroDuration,
    pub(crate) ambiguity: NonZeroDuration,
}

impl TimeoutPolicy {
    pub(crate) fn new(
        ack: Duration,
        completion: Duration,
        inquiry: Duration,
        cancellation: Duration,
        ambiguity: Duration,
    ) -> Result<Self, Error> {
        Ok(Self {
            ack: NonZeroDuration::new(ack)?,
            completion: NonZeroDuration::new(completion)?,
            inquiry: NonZeroDuration::new(inquiry)?,
            cancellation: NonZeroDuration::new(cancellation)?,
            ambiguity: NonZeroDuration::new(ambiguity)?,
        })
    }

    pub(crate) fn validate_at(self, now: Instant) -> Result<(), Error> {
        for duration in [
            self.ack,
            self.completion,
            self.inquiry,
            self.cancellation,
            self.ambiguity,
        ] {
            crate::timeout::validate_duration_at(now, duration.get())?;
        }
        for duration in [
            duration_sum(self.ack.get(), self.completion.get())?,
            duration_sum(self.cancellation.get(), self.ambiguity.get())?,
        ] {
            crate::timeout::validate_duration_at(now, duration)?;
        }
        Ok(())
    }
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            ack: NonZeroDuration(Duration::from_millis(500)),
            completion: NonZeroDuration(Duration::from_secs(5)),
            inquiry: NonZeroDuration(Duration::from_secs(1)),
            cancellation: NonZeroDuration(Duration::from_secs(1)),
            ambiguity: NonZeroDuration(Duration::from_secs(1)),
        }
    }
}

fn duration_sum(left: Duration, right: Duration) -> Result<Duration, Error> {
    left.checked_add(right)
        .ok_or_else(|| Error::InvalidRequest {
            reason: "engine deadline interval sum is unrepresentable".into(),
        })
}

/// Context-sensitive bounded retry rules, completely decided before admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetryPolicy {
    pub(crate) max_retries: u32,
    pub(crate) initial_backoff: Duration,
    pub(crate) maximum_backoff: Duration,
    pub(crate) total_budget: Option<NonZeroDuration>,
    pub(crate) ack_timeout: bool,
    pub(crate) completion_timeout: bool,
    pub(crate) inquiry_timeout: bool,
    pub(crate) buffer_full: bool,
    pub(crate) movement_not_executable: bool,
    pub(crate) builtin_inquiry_syntax: bool,
}

impl RetryPolicy {
    pub(crate) fn validate_at(self, now: Instant) -> Result<(), Error> {
        if self.initial_backoff > self.maximum_backoff {
            return Err(Error::InvalidRequest {
                reason: "engine retry backoffs must be ordered".into(),
            });
        }
        for duration in [self.initial_backoff, self.maximum_backoff] {
            crate::timeout::validate_duration_at(now, duration)?;
        }
        if let Some(budget) = self.total_budget {
            crate::timeout::validate_duration_at(now, budget.get())?;
        }
        Ok(())
    }

    // A fixed no-retry policy for the engine and owner test fixtures;
    // production requests always carry a prepared retry policy (#636).
    #[cfg(test)]
    pub(crate) const NEVER: Self = Self {
        max_retries: 0,
        initial_backoff: Duration::ZERO,
        maximum_backoff: Duration::ZERO,
        total_budget: None,
        ack_timeout: false,
        completion_timeout: false,
        inquiry_timeout: false,
        buffer_full: false,
        movement_not_executable: false,
        builtin_inquiry_syntax: false,
    };
}

/// Immutable physical axes affected by a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MotionEffect {
    pub(crate) axes: crate::AffectedAxes,
}

/// Every immutable protocol policy carried by an admitted request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestContext {
    /// FR7 speeds above 24 require session-confirmed Extended step mode at dispatch.
    pub(crate) requires_extended_pan_tilt_speed: bool,
    pub(crate) motion: Option<MotionEffect>,
    /// Assigned under the owner ingress lock, independently of engine admission.
    pub(crate) submission_order: u64,
    /// Absolute last instant before which an owner-halt STOP may begin a write.
    pub(crate) dispatch_deadline: Option<Instant>,
    pub(crate) target: TargetSlot,
    pub(crate) timeout: TimeoutPolicy,
    pub(crate) retry: RetryPolicy,
    pub(crate) control: ControlPolicy,
    pub(crate) cancellation: CancellationPolicy,
}

impl RequestContext {
    /// Urgent admission authority belongs only to reviewed typed STOPs.
    pub(crate) const fn is_stop(self) -> bool {
        matches!(self.control.class, ControlClass::Urgent)
    }

    /// Checks the completed per-request policy at its actual admission instant,
    /// without reading another clock or revalidating the session's tuning.
    pub(crate) fn validate_at(self, now: Instant) -> Result<(), Error> {
        self.timeout.validate_at(now)?;
        self.retry.validate_at(now)?;
        crate::timeout::validate_duration_at(now, self.control.minimum_spacing)?;
        let attempt = duration_sum(self.timeout.ack.get(), self.timeout.completion.get())?;
        let cancellation = duration_sum(
            self.timeout.cancellation.get(),
            self.timeout.ambiguity.get(),
        )?;
        let window = attempt
            .max(cancellation)
            .max(self.timeout.inquiry.get())
            .max(self.retry.maximum_backoff);
        if let Some(budget) = self.retry.total_budget {
            crate::timeout::validate_duration_at(now, duration_sum(budget.get(), window)?)?;
        }
        Ok(())
    }
}

pub(crate) use crate::request::builtin_contract::AppliedStateProjection;

/// The two kinds of protocol work a camera serves independently.
///
/// Commands and inquiries are admitted, queued, paced and reported on
/// separate lanes, and the raw stream ledger latches each lane of a camera
/// separately (a `CompletionOnly` command's debt holds both).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lane {
    /// Commands: ACK-bearing, `CompletionOnly` and `NoReply` alike.
    Command,
    /// Inquiries.
    Inquiry,
}

/// The engine's only two protocol execution classes.
#[derive(Debug, Clone)]
pub(crate) enum RuntimeRequest {
    Command {
        wire: Arc<EncodedMessage>,
        context: RequestContext,
        reply_shape: ReplyShape,
        applied_state: Option<AppliedStateProjection>,
    },
    Inquiry {
        wire: Arc<EncodedMessage>,
        context: RequestContext,
        payload_shape: crate::PayloadShape,
    },
}

impl RuntimeRequest {
    pub(crate) fn context_mut(&mut self) -> &mut RequestContext {
        match self {
            Self::Command { context, .. } | Self::Inquiry { context, .. } => context,
        }
    }

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

    pub(crate) const fn payload_shape(&self) -> Option<crate::PayloadShape> {
        match self {
            Self::Inquiry { payload_shape, .. } => Some(*payload_shape),
            Self::Command { .. } => None,
        }
    }

    pub(crate) const fn is_inquiry(&self) -> bool {
        matches!(self, Self::Inquiry { .. })
    }

    /// The lane this request is admitted, queued and reported on.
    pub(crate) const fn lane(&self) -> Lane {
        match self {
            Self::Command { .. } => Lane::Command,
            Self::Inquiry { .. } => Lane::Inquiry,
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
    /// Admission slots reserved for this target's urgent typed STOPs, one
    /// per STOP path its profile supports (D26, #778).
    pub(crate) control_reserve: u8,
}

#[cfg(test)]
impl TargetPolicy {
    /// Shared test fixture: two command sockets, supported cancellation and
    /// no control reserve. Tests override fields with `..TargetPolicy::test_default()`.
    pub(crate) const fn test_default() -> Self {
        Self {
            command_sockets: 2,
            cancellation: CancellationPolicy::Supported,
            control_reserve: 0,
        }
    }
}

/// Which admission budget an entry holds (D26, #778).
///
/// Ordinary requests hold one of the session's `capacity` slots. An urgent
/// typed STOP may instead hold one of its target's reserved slots, which
/// ordinary work can never use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmissionSlot {
    Ordinary,
    ControlReserve,
}

/// Session-wide scheduling policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtocolPolicy {
    pub(crate) capacity: usize,
    pub(crate) wire: super::WireProtocol,
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
    /// When `false` (the default), an uncancelled raw command whose ACK or
    /// completion can no longer be confirmed — including at a spent retry
    /// budget — immediately fails on its own with
    /// [`Error::UnsequencedCommandUnconfirmed`] and leaves only the applicable
    /// keyed correlation hold through the ambiguity interval. An active
    /// cancellation remains live until its own resolution bound and fails with
    /// the same error if that bound expires. The session and every unrelated
    /// request keep running. When `true`, the same events instead poison the
    /// whole session (the pre-fix behavior), for deployments that would rather
    /// hard-fail than risk a subtle correlation error. This flag is meaningless
    /// for the Sony envelope, whose sequence correlation needs no raw hold.
    pub(crate) strict_unconfirmed_poison: bool,
}

impl ProtocolPolicy {
    pub(crate) fn validate_at(&self, now: Instant) -> Result<(), Error> {
        for duration in [
            self.command_spacing,
            self.inquiry_spacing,
            self.inquiry_cooldown,
            self.raw_inquiry_release_hold,
            self.raw_release_grace,
        ] {
            crate::timeout::validate_duration_at(now, duration)?;
        }
        crate::timeout::validate_duration_at(
            now,
            duration_sum(self.raw_inquiry_release_hold, self.raw_release_grace)?,
        )
    }
}

#[cfg(test)]
impl ProtocolPolicy {
    /// Shared test fixture: a raw datagram policy with capacity 16, inquiry
    /// capacity 8, no command or inquiry spacing, a 25 ms inquiry cooldown and
    /// non-strict recovery. Tests override fields
    /// with `..ProtocolPolicy::test_default()`.
    pub(crate) fn test_default() -> Self {
        Self {
            capacity: 16,
            wire: super::WireProtocol::new(EnvelopeKind::Raw, crate::transport::AddressingMode::Ip),
            transport: TransportKind::Datagram,
            inquiry_capacity: 8,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::from_millis(25),
            raw_inquiry_release_hold: Duration::from_millis(50),
            raw_release_grace: Duration::from_millis(100),
            strict_unconfirmed_poison: false,
        }
    }
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

    /// Test projection of the inquiry-unkeyed release.
    #[cfg(test)]
    pub(crate) const fn inquiry_unkeyed(self) -> bool {
        self.inquiry_unkeyed
    }

    /// Test projection of the pre-ACK-unkeyed release.
    #[cfg(test)]
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
/// Only validated individual targets can enter this fixed, seven-slot table;
/// controller and broadcast wire addresses have no correlation-release slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RawCorrelationReleaseSet {
    targets: PerTarget<RawCorrelationRelease>,
}

impl RawCorrelationReleaseSet {
    pub(crate) fn is_empty(self) -> bool {
        self.targets.iter().all(|(_, release)| release.is_empty())
    }

    pub(crate) fn for_target(self, target: TargetSlot) -> RawCorrelationRelease {
        self.targets[target]
    }

    pub(super) fn for_target_mut(&mut self, target: TargetSlot) -> &mut RawCorrelationRelease {
        &mut self.targets[target]
    }
}

#[cfg(all(test, any(feature = "async", feature = "blocking")))]
impl RawCorrelationReleaseSet {
    /// Add `target`'s broad terminal release, for owner arbitration tests that
    /// need distinct non-empty sets without a running engine.
    pub(crate) fn with_terminal_for_test(mut self, target: TargetSlot) -> Self {
        self.for_target_mut(target).release_terminal_all();
        self
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
    /// The framer rejected the first byte using the same strict source rule as
    /// complete response decoding. It is discardable noise, not a correlation
    /// candidate and never a framing failure (#745).
    Malformed,
    Incomplete {
        target: TargetSlot,
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
    AwaitInputUntil(Instant),
    /// Discard the first retained raw fragment, record it, and classify again.
    DiscardFirst,
}

/// A decoded frame owns its target and parsed data; it borrows no scheduler state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedFrame {
    pub(crate) target: TargetSlot,
    pub(crate) sequence: Option<EnvelopeSequence>,
    pub(crate) response: DecodedResponse,
}

/// Metadata returned by a successful exact transport write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransmissionMeta {
    Raw,
    Sony { sequence: u32 },
}

impl TransmissionMeta {
    #[cfg(test)]
    pub(crate) const fn for_sequence(sequence: Option<u32>) -> Self {
        match sequence {
            None => Self::Raw,
            Some(sequence) => Self::Sony { sequence },
        }
    }
}

pub(crate) use crate::transport::envelope::SonyRequestSequence;

/// Correlation expectation derived from one bound write plan.
/// Raw VISCA has no Sony sequence axis; Sony preserves its exact retry intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestFraming {
    Raw,
    Sony { sequence: SonyRequestSequence },
}

/// The transport boundary classifies refusal before I/O separately from a
/// send whose bytes may have reached the camera.
#[derive(Debug)]
pub(crate) enum WriteOutcome {
    NotSent(Error),
    Sent(TransmissionMeta),
    Failed(Error),
}

/// One exact write requested by the engine.
#[derive(Debug, Clone)]
pub(crate) enum Transmission {
    Request {
        target: TargetSlot,
        wire: Arc<EncodedMessage>,
        framing: super::WriteFraming,
        lane: Lane,
        requires_extended_pan_tilt_speed: bool,
    },
    Cancel {
        target: TargetSlot,
        socket: ViscaSocket,
        framing: super::WriteFraming,
    },
}

/// Why an otherwise valid input was deliberately inert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IgnoreReason {
    UnknownRequest,
    StaleTransmission,
    IncompatibleTransmissionResult,
    MalformedFrame,
    ReplyShapeMismatch,
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
/// These are exactly the three deadlines that count as request timeouts.
/// Cancellation deadlines are deliberately not part of this vocabulary: they resolve the
/// separate cancellation lifecycle rather than the request's ordinary protocol
/// progress, and they are already reported through
/// [`Effect::CancellationObservation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeadlineKind {
    /// The acknowledgement deadline for a sent command expired.
    Ack,
    /// The completion deadline for an acknowledged command expired.
    Completion,
    /// The reply deadline for a sent inquiry expired.
    InquiryReply,
}

/// A command's terminal result. Inquiry-only payloads cannot enter this lane.
#[derive(Debug, Clone)]
pub(crate) enum CommandOutcome {
    /// A raw plain `NoReply` command reached the local transport only.
    Written,
    Applied,
    Cancelled,
    Failed(Error),
}

impl CommandOutcome {
    /// One projection shared by application and cancellation observers.
    pub(crate) fn verdict(&self) -> Result<crate::CancellationOutcome, Error> {
        match self {
            Self::Written | Self::Applied => Ok(crate::CancellationOutcome::Completed),
            Self::Cancelled => Ok(crate::CancellationOutcome::Cancelled),
            Self::Failed(error) => Err(error.clone()),
        }
    }
}

/// An inquiry's terminal result: a reply or a conclusive failure.
#[derive(Debug, Clone)]
pub(crate) enum InquiryOutcome {
    Reply {
        payload: SmallVec<[u8; INLINE_BYTES]>,
    },
    Failed(Error),
}

/// The heterogeneous effect stream retains each terminal result's lane.
#[derive(Debug, Clone)]
pub(crate) enum RuntimeOutcome {
    Command(CommandOutcome),
    Inquiry(InquiryOutcome),
}

#[cfg(test)]
impl RuntimeOutcome {
    pub(crate) const fn failure(&self) -> Option<&Error> {
        match self {
            Self::Command(CommandOutcome::Failed(error))
            | Self::Inquiry(InquiryOutcome::Failed(error)) => Some(error),
            _ => None,
        }
    }
}

/// A generic failure is lifted into the entry's lane at the terminal boundary.
/// Success constructors already carry their lane's closed outcome type.
pub(super) enum FinishOutcome {
    Command(CommandOutcome),
    Inquiry(InquiryOutcome),
    Failed(Error),
}
impl From<CommandOutcome> for FinishOutcome {
    fn from(value: CommandOutcome) -> Self {
        Self::Command(value)
    }
}
impl From<InquiryOutcome> for FinishOutcome {
    fn from(value: InquiryOutcome) -> Self {
        Self::Inquiry(value)
    }
}
impl From<Error> for FinishOutcome {
    fn from(value: Error) -> Self {
        Self::Failed(value)
    }
}
impl FinishOutcome {
    pub(super) fn for_request(self, request: &RuntimeRequest) -> RuntimeOutcome {
        match self {
            Self::Command(outcome) => RuntimeOutcome::Command(outcome),
            Self::Inquiry(outcome) => RuntimeOutcome::Inquiry(outcome),
            Self::Failed(error) => match request {
                RuntimeRequest::Command { .. } => {
                    RuntimeOutcome::Command(CommandOutcome::Failed(error))
                }
                RuntimeRequest::Inquiry { .. } => {
                    RuntimeOutcome::Inquiry(InquiryOutcome::Failed(Error::inquiry_failed(error)))
                }
            },
        }
    }
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
    TransportClosed { cause: Option<Arc<Error>> },
    FramingFailure { cause: Arc<Error> },
}

/// Session state is terminal except for `Running`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionState {
    Running,
    Closed,
    Shutdown,
    Poisoned,
}

/// A terminal session status cannot describe a running engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndStatus {
    Closed,
    Shutdown,
    Poisoned,
}

impl EndStatus {
    pub(crate) const fn state(self) -> SessionState {
        match self {
            Self::Closed => SessionState::Closed,
            Self::Shutdown => SessionState::Shutdown,
            Self::Poisoned => SessionState::Poisoned,
        }
    }
}

/// The engine's terminal verdict, retained and delivered as one complete value.
#[derive(Debug, Clone)]
pub(crate) struct SessionEnd {
    pub(crate) status: EndStatus,
    pub(crate) cause: Error,
}

/// A session either runs without a terminal cause or ends with one.
#[derive(Debug)]
pub(super) enum EngineSession {
    Running,
    Ended(SessionEnd),
}

impl EngineSession {
    pub(super) const fn state(&self) -> SessionState {
        match self {
            Self::Running => SessionState::Running,
            Self::Ended(end) => end.status.state(),
        }
    }

    pub(super) const fn terminal_error(&self) -> Option<&Error> {
        match self {
            Self::Running => None,
            Self::Ended(end) => Some(&end.cause),
        }
    }
}

/// Ordered boundary inputs to the pure state machine.
// Admission owns the complete bounded request. Keep this synchronous engine
// input inline instead of allocating every admitted request for enum size.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum Input {
    #[cfg(test)]
    Admit {
        ticket: AdmissionTicket,
        request: RuntimeRequest,
        slot: AdmissionSlot,
    },
    TransmissionFinished {
        request: RequestId,
        transmission: TransmissionId,
        result: WriteOutcome,
    },
    Frame(DecodedFrame),
    #[cfg(test)]
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
    /// A pure "re-evaluate deadlines now" input. The owners call `advance`
    /// directly; only `runtime::engine::tests` drives it as an input.
    #[cfg(test)]
    Wake,
}

/// Flattened lifecycle projection for diagnostics and independent test oracles.
/// The engine stores only `Lifecycle`; this is never a parallel authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Ready,
    Sending {
        transmission: TransmissionId,
        started_at: Instant,
    },
    AwaitingAck {
        sent_at: Instant,
        deadline: Instant,
    },
    /// A completion-only raw command that was sent but never acknowledged, so it
    /// owns no socket. It awaits its completion frame under the completion
    /// deadline and, unlike [`Self::AwaitingAck`], has no ACK-timeout path.
    AwaitingCompletion {
        sent_at: Instant,
        deadline: Instant,
    },
    Executing {
        socket: ViscaSocket,
        started_at: Instant,
        deadline: Instant,
    },
    AwaitingReply {
        sent_at: Instant,
        deadline: Instant,
    },
    Backoff {
        ready_at: Instant,
    },
    /// A socket-owned command whose emitted cancellation is awaiting a
    /// conclusive terminal response.
    AwaitingCancellationResolution {
        socket: ViscaSocket,
        deadline: Instant,
    },
}

/// Flattened cancellation projection; never independently stored in an entry.
/// Socket identity is read from the enclosing lifecycle only for this view.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancelState {
    None,
    Requested {
        ambiguity_deadline: Instant,
    },
    Sending {
        transmission: TransmissionId,
        socket: ViscaSocket,
        ambiguity_deadline: Instant,
    },
    AwaitingTerminal {
        ambiguity_deadline: Instant,
        observation_deadline: Instant,
    },
    ObservationFailed {
        socket: ViscaSocket,
        ambiguity_deadline: Instant,
    },
}

/// Ordered, inert outputs. The owner performs I/O and observer/cache delivery.
#[derive(Debug, Clone)]
pub(crate) enum Effect {
    #[cfg(test)]
    Admitted {
        ticket: AdmissionTicket,
        id: RequestId,
    },
    #[cfg(test)]
    AdmissionRejected {
        ticket: AdmissionTicket,
        error: Error,
    },
    Transition {
        id: RequestId,
        from: super::Lifecycle,
        to: super::Lifecycle,
    },
    Transmit {
        transmission: TransmissionId,
        request: RequestId,
        kind: Transmission,
    },
    RetryScheduled {
        id: RequestId,
        attempt: u32,
        #[cfg(test)]
        ready_at: Instant,
    },
    /// One of a request's own protocol deadlines expired.
    ///
    /// `will_retry` is the engine's actual decision for this expiry rather than
    /// the policy that motivated it: it is true exactly when the expiry produced
    /// a [`Effect::RetryScheduled`] for the same request. A subscriber therefore
    /// never has to infer the decision from a [`Effect::Transition`] plus the
    /// absence of a retry: the event carries the decision directly.
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
    SessionEnded(SessionEnd),
    Ignored(IgnoreReason),
}

/// Deterministic cache projection; applying it remains outside the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedStateEffect {
    pub(crate) request: RequestId,
    pub(crate) target: TargetSlot,
    pub(crate) projection: AppliedStateProjection,
}

/// Returns the stream ledger's fixed memory bound, or refuses arithmetic overflow.
pub(crate) const fn checked_stream_ledger_bound(admissible: usize) -> Option<usize> {
    super::stream_ledger::checked_max_entries_per_target(admissible)
}

#[cfg(test)]
mod duration_proof_tests {
    use super::*;

    fn context(timeout: TimeoutPolicy, retry: RetryPolicy) -> RequestContext {
        RequestContext {
            requires_extended_pan_tilt_speed: false,
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: TargetSlot::CAMERA_1,
            timeout,
            retry,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
        }
    }

    #[test]
    fn every_protocol_timeout_is_nonzero_by_construction() {
        assert!(NonZeroDuration::new(Duration::ZERO).is_err());
        for index in 0..5 {
            let mut values = [Duration::from_secs(1); 5];
            values[index] = Duration::ZERO;
            assert!(
                TimeoutPolicy::new(values[0], values[1], values[2], values[3], values[4]).is_err()
            );
        }
        assert!(RetryPolicy::NEVER.total_budget.is_none());
    }

    #[test]
    fn derived_long_profile_timings_are_not_capped_like_configured_overrides() -> Result<(), Error>
    {
        let now = Instant::now();
        let hours = |value: u64| Duration::from_secs(value * 60 * 60);
        let timeout = TimeoutPolicy::new(
            Duration::from_secs(1),
            hours(2),
            hours(2),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )?;
        let retry = RetryPolicy {
            initial_backoff: hours(2),
            maximum_backoff: hours(4),
            total_budget: Some(NonZeroDuration::new(hours(4))?),
            ..RetryPolicy::NEVER
        };
        context(timeout, retry).validate_at(now)?;
        Ok(())
    }

    #[test]
    fn context_refuses_unrepresentable_individual_and_composed_windows() -> Result<(), Error> {
        let now = Instant::now();
        let timeout = TimeoutPolicy {
            ambiguity: NonZeroDuration::new(Duration::MAX)?,
            ..TimeoutPolicy::default()
        };
        assert!(timeout.validate_at(now).is_err());
        let mut request = context(TimeoutPolicy::default(), RetryPolicy::NEVER);
        request.control.minimum_spacing = Duration::MAX;
        assert!(request.validate_at(now).is_err());
        request.control.minimum_spacing = Duration::ZERO;
        request.retry.maximum_backoff = Duration::MAX;
        assert!(request.validate_at(now).is_err());
        request.retry.maximum_backoff = Duration::ZERO;
        request.retry.total_budget = Some(NonZeroDuration::new(Duration::MAX)?);
        assert!(request.validate_at(now).is_err());

        let span = crate::timeout::saturating_deadline(now, Duration::MAX).duration_since(now);
        let half = span / 2 + Duration::from_nanos(1);
        assert!(now.checked_add(half).is_some());
        let nanos = Duration::from_nanos(1);
        let timeout = TimeoutPolicy::new(half, half, nanos, nanos, nanos)?;
        assert!(
            timeout.validate_at(now).is_err(),
            "representable components can overflow as an attempt"
        );
        let timeout = TimeoutPolicy::new(nanos, nanos, nanos, nanos, nanos)?;
        timeout.validate_at(now)?;
        let retry = RetryPolicy {
            total_budget: Some(NonZeroDuration::new(span - nanos)?),
            ..RetryPolicy::NEVER
        };
        retry.validate_at(now)?;
        assert!(
            context(timeout, retry).validate_at(now).is_err(),
            "the complete queued-attempt window must fit"
        );
        Ok(())
    }

    #[test]
    fn session_intervals_are_validated_without_forbidding_zero_pacing() -> Result<(), Error> {
        let now = Instant::now();
        let setters: [fn(&mut ProtocolPolicy, Duration); 5] = [
            |policy, value| policy.command_spacing = value,
            |policy, value| policy.inquiry_spacing = value,
            |policy, value| policy.inquiry_cooldown = value,
            |policy, value| policy.raw_inquiry_release_hold = value,
            |policy, value| policy.raw_release_grace = value,
        ];
        for set in setters {
            let mut policy = ProtocolPolicy::test_default();
            set(&mut policy, Duration::MAX);
            assert!(policy.validate_at(now).is_err());
            set(&mut policy, Duration::ZERO);
            policy.validate_at(now)?;
        }
        Ok(())
    }
}
