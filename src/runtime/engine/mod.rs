//! Deterministic, synchronous, runtime-neutral VISCA protocol engine.
//!
//! This is the sole authority for admitted request lifecycle, dispatch, timing,
//! retry, correlation, and protocol cancellation. It performs no I/O and knows
//! nothing about channels, executors, facade cameras, or observers.

mod correlation;
mod holds;
mod inquiry_shape;
mod lifecycle;
mod occupancy;
mod stream;
mod stream_ledger;
pub(crate) use lifecycle::*;
mod types;

use correlation::{CorrelationMode, CorrelationRef, CorrelationState};
use holds::StreamScope;
use inquiry_shape::InquiryShapes;
pub(crate) use inquiry_shape::MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET;
use occupancy::Occupancy;
use stream::{DatagramContext, StreamContext};

use stream_ledger::{
    Answer, Answered, Evidence, LaneState, NamedFrame, Outstanding, Owes, Resolved, Retro,
    Standing, StreamLedger,
};
pub(crate) use types::*;

use std::{
    cmp::Ordering,
    num::{NonZeroU32, NonZeroU64},
    sync::Arc,
    time::{Duration, Instant},
};

use smallvec::SmallVec;

use crate::protocol::framer::RawIncompletePrefix;
use crate::target::{PerTarget, TargetSlot};
use crate::{
    Certainty, CommandSocketCount, Error, FailureContext, FailureStage, PayloadShape, ViscaSocket,
};

mod decision;
mod entry_table;
use decision::Decision;
use entry_table::{AdmissionFacts, EntryChange, EntryPlacement, EntryTable, SequenceSlots};
mod framing;
pub(crate) use framing::{ProtocolEncoder, WireProtocol, WriteFraming};

#[cfg(test)]
mod tests;

/// The response scope retained when a raw command becomes unconfirmable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnconfirmedScope {
    Lifecycle,
    /// Camera evidence supersedes the command's socket ownership.
    Displaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorrelationKind {
    Request,
    Cancellation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CorrelationOwner {
    request: RequestId,
    kind: CorrelationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceRecord {
    sequence: u32,
    kind: CorrelationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TransmissionOwner {
    request: RequestId,
    write: ActiveWrite,
}

/// One raw-VISCA correlation scope retained after its live owner can no longer
/// safely consume unsequenced input.
///
/// Every bounded quarantine is keyed through this vocabulary. `Socket` is an
/// exact command-completion identity, `PreAck` is the target-local positional
/// command slot, `Inquiry(shape)` retains the exact inquiry declaration, and
/// `AllResponses` is the broad hold left by a command that never earned a wire
/// identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawHoldScope {
    Socket(ViscaSocket),
    PreAck,
    Inquiry(PayloadShape),
    AllResponses,
}

impl RawHoldScope {
    const fn order(self) -> (u8, usize) {
        match self {
            Self::Socket(ViscaSocket::S1) => (0, 0),
            Self::Socket(ViscaSocket::S2) => (1, 0),
            Self::PreAck => (2, 0),
            Self::Inquiry(PayloadShape::Unknown) => (3, 0),
            Self::Inquiry(PayloadShape::Bytes(length)) => (4, length),
            Self::Inquiry(PayloadShape::Nibbles(length)) => (5, length),
            Self::AllResponses => (6, 0),
        }
    }
}

/// Stable private ordering key without imposing ordering traits on the public
/// `TargetSlot` or `ViscaSocket` API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawHoldKey {
    target: TargetSlot,
    scope: RawHoldScope,
}

impl RawHoldKey {
    const fn new(target: TargetSlot, scope: RawHoldScope) -> Self {
        Self { target, scope }
    }
}

impl Ord for RawHoldKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.target.camera().value(), self.scope.order())
            .cmp(&(other.target.camera().value(), other.scope.order()))
    }
}

impl PartialOrd for RawHoldKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// One time-bounded entry in the engine's authoritative raw quarantine table.
///
/// `owner` is retained when exactly one request supplied the hold. Overlapping
/// holds from different requests deliberately collapse it to `None`: the scope
/// remains unsafe until the later deadline, but no request identity may be
/// guessed from that overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawHold {
    until: Instant,
    owner: Option<RequestId>,
}

/// What a raw stream camera's completions show about socket nibbles (#795).
/// A camera whose completions name their sockets (`z0 5y FF`) completes a
/// socketed command that way, so its socketless completion is a
/// `CompletionOnly` command's; one that omits the nibble could send either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionSockets {
    /// Nothing learned yet: socketless completions take the conservative
    /// path.
    Unknown,
    /// A completion named its socket.
    Named,
    /// A socketless completion arrived with no `CompletionOnly` command
    /// outstanding. Permanent for the session.
    Omitted,
}

/// Time-bounded retained-input decision for one due raw release (#713).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawReleaseGate {
    releases: RawCorrelationReleaseSet,
    await_until: Instant,
}

/// Socketless completions that could belong to a held predecessor or a live
/// holder. Identity snapshots prevent an old frame from completing a successor
/// after socket reuse; only a complete count proves that every candidate ended.
#[derive(Debug, Clone, Copy)]
struct PendingSocketlessCompletions {
    candidates: [Option<RequestId>; 2],
    count: u8,
}

/// Whole-request history retained after a written attempt enters retry waiting.
/// Its nonzero count and cause exist together; only `EntryChange::Retry` writes it.
#[derive(Debug)]
struct PriorAttempts {
    count: NonZeroU32,
    cause: Error,
    possibly_applied: bool,
}

/// Evidence supplied by the boundary ending the current attempt. Physical
/// writes and pre-write refusals do not mutate whole-request retry history.
#[derive(Debug, Clone, Copy)]
enum AttemptEvidence {
    Lifecycle,
    Unwritten,
    Written,
    Rejected,
}

/// Only the actual camera-error ingress can classify an attempt as rejected.
/// A transport error with the same public `Error` variant proves no rejection.
#[derive(Debug)]
enum RetryCause {
    CameraRejection(Error),
    Unconfirmed(Error),
}

/// The one authoritative lifecycle record for an admitted request.
#[derive(Debug)]
pub(crate) struct Entry {
    request: RuntimeRequest,
    lifecycle: EntryPlacement,
    submitted_at: Instant,
    prior: Option<PriorAttempts>,
    sequences: SequenceSlots,
    slot: AdmissionSlot,
}

impl Entry {
    const fn lifecycle(&self) -> Lifecycle {
        self.lifecycle.logical()
    }

    fn reply_family(&self) -> SendingReply {
        self.lifecycle()
            .reply_family()
            .unwrap_or_else(|| SendingReply::new(&self.request))
    }

    fn retry_count(&self) -> u32 {
        self.prior.as_ref().map_or(0, |prior| prior.count.get())
    }

    fn retry_cause(&self) -> Option<&Error> {
        self.prior.as_ref().map(|prior| &prior.cause)
    }

    /// A later cause cannot erase an earlier possible camera effect.
    fn terminal_error(&self, cause: Error, evidence: AttemptEvidence) -> Error {
        if self.certainty(evidence) == Certainty::Unconfirmed
            && cause.failure_context()
                != Some(FailureContext::new(
                    FailureStage::Terminal,
                    Certainty::Unconfirmed,
                ))
        {
            Error::request_unconfirmed(cause)
        } else {
            cause
        }
    }

    /// Every retry replays the request's authoritative Sony sequence. A
    /// cancellation occupies its own slot and cannot replace that sequence.
    fn current_sequence(&self) -> Option<u32> {
        self.sequences[CorrelationKind::Request]
    }

    /// An exact Sony sequence proves a previous written attempt while the
    /// request waits off the wire. ACKs require the ACK-bearing reply family;
    /// completions additionally accept `CompletionOnly` (which has no ACK).
    fn previous_written_attempt(&self) -> Option<AttemptFacts> {
        self.prior.as_ref()?;
        self.current_sequence()?;
        self.lifecycle().previous_attempt()
    }

    /// The one derivation of a request's terminal certainty (D20/D21/D28).
    ///
    /// Inquiries have no physical effect and fail conclusively. A command may
    /// have applied if a prior attempt remains unconfirmed, if this boundary
    /// reports a physical write, or if its current lifecycle is on the wire.
    /// A pre-write refusal or an unwritten halt STOP cannot contribute a new
    /// effect; neither can a conclusive pre-ACK camera rejection. A confirmed
    /// ACK has already proved possible effect, even when a later error ends it.
    /// Queued and backed-off requests therefore use only their prior history.
    fn certainty(&self, evidence: AttemptEvidence) -> Certainty {
        if self.reply_family() == SendingReply::Inquiry {
            return Certainty::FailedConclusively;
        }
        let current_possible = match evidence {
            AttemptEvidence::Unwritten | AttemptEvidence::Rejected => {
                self.lifecycle().is_accepted()
            }
            AttemptEvidence::Written => true,
            AttemptEvidence::Lifecycle => self.lifecycle().attempt().is_some(),
        };
        if current_possible
            || self
                .prior
                .as_ref()
                .is_some_and(|prior| prior.possibly_applied)
        {
            Certainty::Unconfirmed
        } else {
            Certainty::NotAccepted
        }
    }

    fn may_have_effect(&self, evidence: AttemptEvidence) -> bool {
        self.certainty(evidence) != Certainty::NotAccepted
    }

    fn retry_budget_deadline(&self) -> Option<Instant> {
        self.lifecycle()
            .retry_budget_deadline(self.submitted_at, self.request.context().retry.total_budget)
    }
    fn correlated_response_deadline(&self) -> Option<Instant> {
        self.lifecycle()
            .correlated_response_deadline(self.retry_budget_deadline())
    }
    fn dispatched_socket_capacity(&self) -> Option<CommandSocketCount> {
        self.lifecycle()
            .attempt()
            .and_then(|attempt| attempt.socket_capacity)
    }
    fn stream_order(&self) -> Option<u64> {
        self.lifecycle()
            .attempt()
            .and_then(|attempt| attempt.stream_order)
    }
    fn cancel_attempted_socket(&self) -> Option<ViscaSocket> {
        self.lifecycle().attempted_socket()
    }
    fn cancellation_observation_open(&self) -> bool {
        self.lifecycle().observer_open()
    }

    fn due(&self, id: RequestId) -> Option<DueWork> {
        let (at, action) = self.lifecycle.active()?.due_action(
            self.retry_budget_deadline(),
            self.certainty(AttemptEvidence::Lifecycle),
        )?;
        Some(DueWork {
            at,
            action,
            request: id,
        })
    }

    /// Test projection of the entry's phase, read by `runtime::engine::tests`
    /// and `OwnerState::request_state`.
    #[cfg(test)]
    pub(crate) fn phase(&self) -> Phase {
        self.lifecycle().phase()
    }

    /// Test projection of the entry's cancellation state.
    #[cfg(test)]
    pub(crate) const fn cancellation(&self) -> CancelState {
        self.lifecycle().cancellation()
    }
}

#[derive(Debug)]
struct IdAllocator {
    next: u64,
}

impl IdAllocator {
    const fn new() -> Self {
        Self { next: 1 }
    }

    #[cfg(test)]
    const fn seeded(next: u64) -> Self {
        Self { next }
    }

    fn candidate(&mut self) -> Option<NonZeroU64> {
        // Zero is the exhausted sentinel.  Once MAX has been issued, this
        // allocator must never wrap and hand a stale owner input a reusable
        // identity.
        let value = NonZeroU64::new(self.next)?;
        self.next = if value.get() == u64::MAX {
            0
        } else {
            value.get() + 1
        };
        Some(value)
    }
}

#[derive(Debug, Clone, Copy)]
struct DispatchSelection {
    request: RequestId,
}

#[derive(Debug, Clone, Copy)]
struct DueWork {
    at: Instant,
    action: DueAction,
    request: RequestId,
}

/// One owner-sampled instant shared by an ordered external-input turn.
///
/// A turn lets the serialized owner apply every decoded frame from one wire
/// read, and recursively apply identified transmission results produced while
/// draining each frame's effects, before scheduler deadlines or ordinary
/// dispatch run. The token deliberately owns the timestamp so every input in
/// the turn is evaluated at exactly the same monotonic instant.
#[derive(Debug)]
pub(crate) struct InputTurn {
    now: Instant,
}

impl DueWork {
    fn key(self) -> (Instant, RequestId, u8) {
        (self.at, self.request, self.action.priority())
    }
}

#[derive(Debug, Default)]
struct TargetState {
    policy: Option<TargetPolicy>,
    motion_fences: [u64; 5],
}

/// Pure state machine implementing the issue #542 protocol boundary.
#[derive(Debug)]
pub(crate) struct ProtocolEngine {
    core: EngineCore,
    correlation: CorrelationMode,
}

#[derive(Debug)]
pub(crate) struct EngineCore {
    policy: ProtocolPolicy,
    targets: PerTarget<TargetState>,
    entries: EntryTable,
    next_request_id: IdAllocator,
    next_transmission_id: IdAllocator,
    jitter: Jitter,
    pacing: Pacing,
    session: EngineSession,
}

/// Lane timestamps are authoritative; the session-wide floor is derived.
#[derive(Debug, Default)]
struct Pacing {
    last_command: Option<Instant>,
    last_inquiry: Option<Instant>,
    inquiry_cooldown_until: Option<Instant>,
}

impl Pacing {
    fn record(&mut self, lane: Lane, now: Instant) {
        *match lane {
            Lane::Command => &mut self.last_command,
            Lane::Inquiry => &mut self.last_inquiry,
        } = Some(now);
    }

    fn last_request(&self) -> Option<Instant> {
        self.last_command.max(self.last_inquiry)
    }
}

impl std::ops::Deref for ProtocolEngine {
    type Target = EngineCore;
    fn deref(&self) -> &EngineCore {
        &self.core
    }
}
impl std::ops::DerefMut for ProtocolEngine {
    fn deref_mut(&mut self) -> &mut EngineCore {
        &mut self.core
    }
}

trait EngineAccess: std::ops::Deref<Target = EngineCore> + std::ops::DerefMut {
    fn mode(&self) -> CorrelationRef<'_>;
    fn split(&mut self) -> (&mut EngineCore, CorrelationState<'_>);
}
impl EngineAccess for ProtocolEngine {
    fn mode(&self) -> CorrelationRef<'_> {
        self.correlation.as_ref()
    }
    fn split(&mut self) -> (&mut EngineCore, CorrelationState<'_>) {
        (&mut self.core, self.correlation.as_mut())
    }
}

impl ProtocolEngine {
    /// Constructs an empty session engine. Targets are registered separately.
    pub(crate) fn new(policy: ProtocolPolicy, now: Instant) -> Result<Self, Error> {
        if policy.capacity == 0 || policy.inquiry_capacity == 0 {
            return Err(Error::InvalidRequest {
                reason: "engine and inquiry capacity must be non-zero".into(),
            });
        }
        policy.validate_at(now)?;
        let correlation = CorrelationMode::new(policy.wire.kind(), policy.transport);
        Ok(Self {
            correlation,
            core: EngineCore {
                policy,
                targets: PerTarget::default(),
                entries: EntryTable::new(),
                next_request_id: IdAllocator::new(),
                next_transmission_id: IdAllocator::new(),
                jitter: Jitter::new(),
                pacing: Pacing::default(),
                session: EngineSession::Running,
            },
        })
    }

    pub(crate) fn register_target(
        &mut self,
        target: TargetSlot,
        policy: TargetPolicy,
    ) -> Result<(), Error> {
        EngineMethods::register_target(self, target, policy)
    }

    pub(crate) fn retune(
        &mut self,
        command_spacing: Duration,
        inquiry_spacing: Duration,
        command_sockets: PerTarget<Option<CommandSocketCount>>,
        now: Instant,
    ) -> Result<(), Error> {
        EngineMethods::retune(self, command_spacing, inquiry_spacing, command_sockets, now)
    }

    #[cfg(feature = "test-utils")]
    pub(crate) const fn command_spacing(&self) -> Duration {
        self.core.policy.command_spacing
    }

    #[cfg(feature = "test-utils")]
    pub(crate) fn envelope_kind(&self) -> EnvelopeKind {
        self.core.policy.wire.kind()
    }

    #[cfg(test)]
    pub(crate) fn operational_limits_for_test(
        &self,
    ) -> (Duration, Duration, PerTarget<Option<CommandSocketCount>>) {
        (
            self.core.policy.command_spacing,
            self.core.policy.inquiry_spacing,
            PerTarget::from_fn(|target| {
                self.core.targets[target]
                    .policy
                    .map(|policy| policy.command_sockets)
            }),
        )
    }

    pub(crate) const fn state(&self) -> SessionState {
        self.core.session.state()
    }

    /// Project the retained cause from the engine's complete terminal verdict.
    /// Terminal effects carry that same verdict directly to their consumers.
    pub(crate) const fn terminal_error(&self) -> Option<&Error> {
        self.core.session.terminal_error()
    }

    #[cfg(test)]
    pub(crate) fn entry(&self, id: RequestId) -> Option<&Entry> {
        EngineMethods::entry(self, id)
    }

    #[cfg(test)]
    pub(crate) fn active_len(&self) -> usize {
        EngineMethods::active_len(self)
    }

    #[cfg(test)]
    pub(crate) fn assert_input_boundary(&self) -> Result<(), Box<str>> {
        EngineMethods::assert_input_boundary(self)
    }

    // Test drivers collect effects; production always appends to owner-owned storage.
    #[cfg(test)]
    pub(crate) fn handle(&mut self, input: Input, now: Instant) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.handle_into(input, now, &mut effects);
        effects
    }

    pub(crate) fn handle_into(&mut self, input: Input, now: Instant, effects: &mut Vec<Effect>) {
        EngineMethods::handle_into(self, input, now, effects);
    }

    #[cfg(test)]
    pub(crate) fn handle_turn(
        &mut self,
        input: Input,
        now: Instant,
        engine_turn: EngineTurn,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.handle_turn_into(input, now, engine_turn, &mut effects);
        effects
    }

    pub(crate) fn handle_turn_into(
        &mut self,
        input: Input,
        now: Instant,
        engine_turn: EngineTurn,
        effects: &mut Vec<Effect>,
    ) {
        EngineMethods::handle_turn_into(self, input, now, engine_turn, effects);
    }

    pub(crate) fn begin_input_turn(&self, now: Instant) -> InputTurn {
        EngineMethods::begin_input_turn(self, now)
    }

    #[cfg(test)]
    pub(crate) fn handle_in_turn(&mut self, turn: &InputTurn, input: Input) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.handle_in_turn_into(turn, input, &mut effects);
        effects
    }

    pub(crate) fn handle_in_turn_into(
        &mut self,
        turn: &InputTurn,
        input: Input,
        effects: &mut Vec<Effect>,
    ) {
        EngineMethods::handle_in_turn_into(self, turn, input, effects);
    }

    pub(crate) fn admit_in_turn(
        &mut self,
        turn: &InputTurn,
        request: RuntimeRequest,
        slot: AdmissionSlot,
    ) -> Result<RequestId, Error> {
        EngineMethods::admit_in_turn(self, turn, request, slot)
    }

    pub(crate) fn cancel_in_turn_into(
        &mut self,
        turn: &InputTurn,
        id: RequestId,
        effects: &mut Vec<Effect>,
    ) -> Result<(), Error> {
        EngineMethods::cancel_in_turn_into(self, turn, id, effects)
    }

    #[cfg(test)]
    pub(crate) fn finish_input_turn(
        &mut self,
        input_turn: InputTurn,
        engine_turn: EngineTurn,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.finish_input_turn_into(input_turn, engine_turn, &mut effects);
        effects
    }

    pub(crate) fn finish_input_turn_into(
        &mut self,
        input_turn: InputTurn,
        engine_turn: EngineTurn,
        effects: &mut Vec<Effect>,
    ) {
        EngineMethods::finish_input_turn_into(self, input_turn, engine_turn, effects);
    }

    #[cfg(test)]
    pub(crate) fn advance(&mut self, now: Instant) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.advance_into(now, &mut effects);
        effects
    }

    pub(crate) fn advance_into(&mut self, now: Instant, effects: &mut Vec<Effect>) {
        EngineMethods::advance_into(self, now, effects);
    }

    #[cfg(test)]
    pub(crate) fn advance_turn(&mut self, now: Instant, turn: EngineTurn) -> Vec<Effect> {
        let mut effects = Vec::new();
        EngineMethods::advance_turn_into(self, now, turn, &mut effects);
        effects
    }

    #[cfg(test)]
    pub(crate) fn halt_in_turn(
        &mut self,
        turn: &InputTurn,
        target: TargetSlot,
        axes: crate::AffectedAxes,
        cutoff: u64,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.halt_in_turn_into(turn, target, axes, cutoff, &mut effects);
        effects
    }

    pub(crate) fn halt_in_turn_into(
        &mut self,
        turn: &InputTurn,
        target: TargetSlot,
        axes: crate::AffectedAxes,
        cutoff: u64,
        effects: &mut Vec<Effect>,
    ) {
        EngineMethods::halt_in_turn_into(self, turn, target, axes, cutoff, effects);
    }

    #[cfg(test)]
    pub(crate) fn expire_unwritten_halt(
        &mut self,
        request: RequestId,
        transmission: TransmissionId,
        now: Instant,
    ) -> Option<Vec<Effect>> {
        let mut effects = Vec::new();
        self.expire_unwritten_halt_into(request, transmission, now, &mut effects)
            .then_some(effects)
    }

    pub(crate) fn expire_unwritten_halt_into(
        &mut self,
        request: RequestId,
        transmission: TransmissionId,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) -> bool {
        EngineMethods::expire_unwritten_halt_into(self, request, transmission, now, effects)
    }

    pub(crate) fn raw_correlation_releases_due(&self, now: Instant) -> RawCorrelationReleaseSet {
        EngineMethods::raw_correlation_releases_due(self, now)
    }

    #[cfg(test)]
    pub(crate) fn raw_prefix_disposition(
        &self,
        releases: RawCorrelationReleaseSet,
        evidence: RawPrefixEvidence,
    ) -> RawPrefixDisposition {
        EngineMethods::raw_prefix_disposition(self, releases, evidence)
    }

    pub(crate) fn resolve_raw_release_gate(
        &mut self,
        now: Instant,
        evidence: Option<RawPrefixEvidence>,
    ) -> RawReleaseGateAction {
        EngineMethods::resolve_raw_release_gate(self, now, evidence)
    }

    pub(crate) fn next_wake(&self, now: Instant) -> Option<Instant> {
        EngineMethods::next_wake(self, now)
    }

    #[cfg(test)]
    pub(crate) fn assert_invariants(&self) -> Result<(), Box<str>> {
        EngineMethods::assert_invariants(self)
    }
}

/// Shared operations are reusable by a processor borrowing an exact mode.
trait EngineMethods: EngineAccess {
    fn stop_wait_bound_in(&self, entry: &Entry, occupancy: &Occupancy) -> Option<Instant> {
        self.mode().stream().and_then(|stream| {
            stream.stop_wait_bound(entry, || {
                occupancy.targets[entry.request.context().target].uncorrelated_write
            })
        })
    }
    fn pre_ack_bypass_in(&self, entry: &Entry, target: TargetSlot, occupancy: &Occupancy) -> bool {
        entry.reply_family() == SendingReply::AckThenCompletion
            && entry.request.context().is_stop()
            && (!self.mode().is_raw() || occupancy.targets[target].unacknowledged == 0)
    }
    fn used_capacity_in(&self, target: TargetSlot, occupancy: &Occupancy) -> usize {
        occupancy.targets[target]
            .commands
            .saturating_add(if self.mode().is_stream() {
                0
            } else {
                self.raw_socket_hold_count(target)
            })
    }
    fn hold_blocks_in(&self, entry: &Entry, target: TargetSlot, occupancy: &Occupancy) -> bool {
        let stop = urgent_stop(entry);
        let response_bearing = entry.reply_family() != SendingReply::NoReply;
        (response_bearing && !stop && self.raw_hold(target, RawHoldScope::AllResponses).is_some())
            || entry
                .request
                .payload_shape()
                .is_some_and(|shape| self.raw_inquiry_hold_deadline(target, shape).is_some())
            || (entry.reply_family() != SendingReply::Inquiry
                && !self.pre_ack_bypass_in(entry, target, occupancy)
                && self.raw_hold(target, RawHoldScope::PreAck).is_some())
            || (entry.reply_family() != SendingReply::Inquiry
                && !self.mode().is_stream()
                && !stop
                && occupancy.targets[target].commands < self.command_sockets(target).get()
                && self.used_capacity_in(target, occupancy) >= self.command_sockets(target).get())
    }
    fn command_gate_in(&self, entry: &Entry, target: TargetSlot, occupancy: &Occupancy) -> bool {
        let raw = self.mode().is_raw();
        let state = &occupancy.targets[target];
        let urgent_bypass = raw
            && entry.reply_family() != SendingReply::Inquiry
            && entry.request.context().is_stop()
            && !(self.mode().is_stream() && self.mode().ledger().forbids_crossing(target))
            && state.urgent_bypass(entry, self.mode().is_stream());
        (raw && state.unacknowledged > 0 && !urgent_bypass)
            || ((self.raw_hold(target, RawHoldScope::PreAck).is_some()
                || self.mode().ledger().lane_state(target, Lane::Command) != LaneState::Clear)
                && !self.pre_ack_bypass_in(entry, target, occupancy))
            || self.latched_raw_lane_error(&entry.request).is_some()
    }
    fn uncorrelated_shape_blocked_in(
        &self,
        entry: &Entry,
        target: TargetSlot,
        now: Instant,
        occupancy: &Occupancy,
    ) -> bool {
        let state = &occupancy.targets[target];
        match (&self.mode(), entry.reply_family()) {
            (CorrelationRef::Sequenced, _) => false,
            (_, SendingReply::Inquiry) => state.uncorrelated_command,
            (_, SendingReply::NoReply) => {
                self.used_capacity_in(target, occupancy) > 0 || state.inquiries > 0
            }
            (mode, SendingReply::CompletionOnly) => {
                self.used_capacity_in(target, occupancy) > 0
                    || state.inquiries > 0
                    || self.raw_target_hold_active(target)
                    || mode.ledger().owes_rejection(target)
                    || (self.raw_socket_hold_running(target, now)
                        && !matches!(mode, CorrelationRef::RawStream(stream) if stream.learning[target].completion_sockets == CompletionSockets::Named))
            }
            (_, SendingReply::AckThenCompletion) => false,
        }
    }
    fn dispatch_blocked_in(&self, entry: &Entry, now: Instant, occupancy: &Occupancy) -> bool {
        let target = entry.request.context().target;
        let state = &occupancy.targets[target];
        match entry.request.lane() {
            Lane::Inquiry => {
                self.hold_blocks_in(entry, target, occupancy)
                    || (if self.mode().is_raw() {
                        state.inquiries
                    } else {
                        occupancy.total_inquiries
                    }) >= self.policy.inquiry_capacity
                    || self.uncorrelated_shape_blocked_in(entry, target, now, occupancy)
                    || (self.mode().is_raw()
                        && entry.request.payload_shape().is_some_and(|shape| {
                            state.overlaps(shape)
                                || self.mode().ledger().inquiry_state(target, shape)
                                    != LaneState::Clear
                        }))
                    || self.mode().ledger().forbids_crossing(target)
            }
            Lane::Command => {
                let Some(policy) = self.targets[target].policy else {
                    return true;
                };
                !self
                    .stop_wait_bound_in(entry, occupancy)
                    .is_some_and(|bound| now >= bound)
                    && (self.hold_blocks_in(entry, target, occupancy)
                        || self.command_gate_in(entry, target, occupancy)
                        || (if urgent_stop(entry) {
                            state.commands
                        } else {
                            self.used_capacity_in(target, occupancy)
                        }) >= policy.command_sockets.get()
                        || self.uncorrelated_shape_blocked_in(entry, target, now, occupancy))
            }
        }
    }

    /// Registers one target's socket and cancellation policy.
    fn register_target(&mut self, target: TargetSlot, policy: TargetPolicy) -> Result<(), Error> {
        let slot = &mut self.targets[target].policy;
        match *slot {
            Some(existing) if existing != policy => Err(Error::InvalidState {
                reason: "target already has conflicting protocol policy".into(),
            }),
            Some(_) => Ok(()),
            None => {
                *slot = Some(policy);
                self.debug_assert_invariants();
                Ok(())
            }
        }
    }

    /// Replaces the session-wide pacing floor and per-target socket capacity.
    ///
    /// Target *registration* is still immutable: a slot that was never
    /// registered stays unregistered, and `command_sockets` is only replaced
    /// where a target policy already exists. Nothing else about an admitted
    /// request changes, so this cannot reorder, cancel, or re-time work that is
    /// already awaiting a protocol deadline; it only changes when the scheduler
    /// is next willing to put a frame on the wire. Lowering socket capacity
    /// below the number of commands currently in flight is therefore safe: the
    /// excess drains normally and no further dispatch happens until it has.
    fn retune(
        &mut self,
        command_spacing: Duration,
        inquiry_spacing: Duration,
        command_sockets: PerTarget<Option<CommandSocketCount>>,
        now: Instant,
    ) -> Result<(), Error> {
        let mut policy = self.policy.clone();
        policy.command_spacing = command_spacing;
        policy.inquiry_spacing = inquiry_spacing;
        policy.validate_at(now)?;
        self.policy.command_spacing = command_spacing;
        self.policy.inquiry_spacing = inquiry_spacing;
        for (target, slot) in self.targets.iter_mut() {
            if let (Some(policy), Some(sockets)) = (slot.policy.as_mut(), command_sockets[target]) {
                policy.command_sockets = sockets;
            }
        }
        self.debug_assert_invariants();
        Ok(())
    }

    // Read-only inspection seams for engine and owner tests.
    #[cfg(test)]
    fn entry(&self, id: RequestId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    #[cfg(test)]
    fn active_len(&self) -> usize {
        self.entries.len()
    }

    /// Audits every derived index against the authoritative entries.
    ///
    /// Issue #636: [`Self::assert_invariants`] had no production call site at
    /// all — it was a test auditor, so an index corruption on a path no test
    /// happened to exercise could ship. This hook closes each mutation entry
    /// point over it. `debug_assert!` is compiled out of a release build, so a
    /// shipped binary pays nothing; a debug build and every `cargo test` run
    /// audit the full index set after every single input.
    #[inline]
    fn debug_assert_invariants(&self) {
        debug_assert!(
            self.assert_invariants().is_ok(),
            "engine invariant violated: {}",
            self.assert_invariants()
                .err()
                .unwrap_or_else(|| Box::from("unknown"))
        );
    }

    /// A returned transmit is performed immediately. Its identified result is
    /// the only permitted input until that write has finished.
    fn assert_input_boundary(&self) -> Result<(), Box<str>> {
        if self
            .entries
            .values()
            .any(|entry| matches!(entry.lifecycle(), Lifecycle::Sending { .. }))
        {
            return Err("no entry may be Sending at an input boundary".into());
        }
        Ok(())
    }

    fn debug_assert_input_boundary(&self) {
        debug_assert!(
            self.assert_input_boundary().is_ok(),
            "no entry may be Sending at an input boundary"
        );
    }

    /// Applies one ordered external input first, then all work due at `now`.
    fn handle_into(&mut self, input: Input, now: Instant, effects: &mut Vec<Effect>) {
        self.handle_turn_into(input, now, EngineTurn::Complete, effects);
    }

    /// Apply one external input and append its complete turn to the owner's sink.
    /// Every emitted transmit must be written and completed before another input.
    fn handle_turn_into(
        &mut self,
        input: Input,
        now: Instant,
        engine_turn: EngineTurn,
        effects: &mut Vec<Effect>,
    ) {
        let input_turn = self.begin_input_turn(now);
        self.handle_in_turn_into(&input_turn, input, effects);
        self.finish_input_turn_into(input_turn, engine_turn, effects);
    }

    /// Starts an ordered external-input turn at one owner-sampled instant.
    ///
    /// The owner must fully drain the effects returned for one input before it
    /// applies the next input in the turn. Any identified transmission result
    /// produced by that draining is applied with [`Self::handle_in_turn_into`] and
    /// the same token. Once every frame in wire order is applied, the owner
    /// calls [`Self::finish_input_turn_into`] exactly once with the selected
    /// [`EngineTurn`] boundary.
    fn begin_input_turn(&self, now: Instant) -> InputTurn {
        InputTurn { now }
    }

    /// Applies one input without running due deadlines or ordinary dispatch.
    ///
    /// This is the recursive input seam for an active [`InputTurn`]. Effects
    /// remain in the exact order produced by this input; the owner drains them
    /// before passing the next wire frame.
    fn handle_in_turn_into(&mut self, turn: &InputTurn, input: Input, sink: &mut Vec<Effect>) {
        if !matches!(input, Input::TransmissionFinished { .. }) {
            self.debug_assert_input_boundary();
        }
        #[cfg(test)]
        if let Input::Cancel { id } = input {
            let _ = self.cancel_in_turn_into(turn, id, sink);
            return;
        }
        let mut effects = Decision::new(sink);
        self.apply_input(input, turn.now, &mut effects);
        self.finish_decision(turn.now, effects);
    }

    /// Returns the admission verdict inside the caller's sampled input turn.
    /// Admission produces no effects, deadlines or dispatch work.
    fn admit_in_turn(
        &mut self,
        turn: &InputTurn,
        request: RuntimeRequest,
        slot: AdmissionSlot,
    ) -> Result<RequestId, Error> {
        self.debug_assert_input_boundary();
        let verdict = self.admit(request, slot, turn.now);
        self.debug_assert_invariants();
        verdict
    }

    /// Applies cancellation in an input turn and returns synchronous refusal
    /// directly, so the owner does not need a parallel refusal latch.
    fn cancel_in_turn_into(
        &mut self,
        turn: &InputTurn,
        id: RequestId,
        sink: &mut Vec<Effect>,
    ) -> Result<(), Error> {
        self.debug_assert_input_boundary();
        let mut effects = Decision::new(sink);
        let result = self.cancel(id, turn.now, &mut effects);
        self.finish_decision(turn.now, effects);
        result
    }

    /// Ends an ordered input turn, then runs due work, pending cancellation,
    /// and one ordinary dispatch.
    ///
    /// External inputs at the turn timestamp therefore win over deadlines at
    /// that same timestamp, while frame and recursively produced effect order
    /// remains owner-controlled and deterministic.
    fn finish_input_turn_into(
        &mut self,
        input_turn: InputTurn,
        engine_turn: EngineTurn,
        sink: &mut Vec<Effect>,
    ) {
        let mut effects = Decision::new(sink);
        if engine_turn.runs_due() {
            self.run_due(input_turn.now, &mut effects);
            self.drain_pending_cancellations(input_turn.now, &mut effects);
            self.dispatch_one(input_turn.now, &mut effects);
        }
        self.finish_decision(input_turn.now, effects)
    }

    fn finish_decision(&mut self, now: Instant, mut decision: Decision<'_>) {
        while decision.has_deferred() {
            self.apply_retro(now, &mut decision);
        }
        self.debug_assert_invariants();
    }

    fn apply_input(&mut self, input: Input, now: Instant, effects: &mut Decision) {
        match input {
            #[cfg(test)]
            Input::Admit {
                ticket,
                request,
                slot,
            } => {
                let turn = self.begin_input_turn(now);
                effects.push(match self.admit_in_turn(&turn, request, slot) {
                    Ok(id) => Effect::Admitted { ticket, id },
                    Err(error) => Effect::AdmissionRejected { ticket, error },
                });
            }
            Input::TransmissionFinished {
                request,
                transmission,
                result,
            } => self.transmission_finished(request, transmission, result, now, effects),
            Input::Frame(frame) => self.frame(frame, now, effects),
            #[cfg(test)]
            Input::Cancel { id } => {
                let _ = self.cancel(id, now, effects);
            }
            Input::ReceiveFault { error } => self.receive_fault(&error, now, effects),
            Input::Shutdown(reason) => match reason {
                ShutdownReason::Explicit => self.terminate_session(
                    EndStatus::Shutdown,
                    Error::RuntimeShutdown,
                    now,
                    effects,
                ),
                ShutdownReason::TransportClosed { cause } => self.terminate_session(
                    EndStatus::Closed,
                    Error::connection_closed(cause),
                    now,
                    effects,
                ),
                ShutdownReason::FramingFailure { cause } => self.terminate_session(
                    EndStatus::Poisoned,
                    Error::stream_poisoned(cause),
                    now,
                    effects,
                ),
            },
            #[cfg(test)]
            Input::Wake => {}
        }
        #[cfg(test)]
        tests::assert_occupancy_agrees(self, now);
    }

    /// Runs due internal work, pending cancellation, and ordinary dispatch.
    fn advance_into(&mut self, now: Instant, sink: &mut Vec<Effect>) {
        self.advance_turn_into(now, EngineTurn::Complete, sink);
    }

    /// Runs the scheduler work selected by `turn` at `now`.
    fn advance_turn_into(&mut self, now: Instant, turn: EngineTurn, sink: &mut Vec<Effect>) {
        let mut effects = Decision::new(sink);
        if turn.runs_due() {
            self.run_due(now, &mut effects);
            self.drain_pending_cancellations(now, &mut effects);
            self.dispatch_one(now, &mut effects);
        }
        self.finish_decision(now, effects)
    }

    /// Entries holding `slot`, for `target` when the slot is a control
    /// reserve.
    fn entries_holding(&self, slot: AdmissionSlot, target: TargetSlot) -> usize {
        self.entries
            .values()
            .filter(|entry| {
                entry.slot == slot
                    && (slot == AdmissionSlot::Ordinary || entry.request.context().target == target)
            })
            .count()
    }

    /// Whether `request` may take `slot` (D26, #778). Ordinary slots are
    /// bounded by `capacity`; a target's reserved slots by its
    /// `control_reserve`, and only an urgent request may hold one.
    fn admission_slot_available(
        &self,
        request: &RuntimeRequest,
        slot: AdmissionSlot,
    ) -> Result<(), Error> {
        let target = request.context().target;
        match slot {
            AdmissionSlot::Ordinary => {
                if self.entries_holding(slot, target) >= self.policy.capacity {
                    return Err(Error::RuntimeQueueFull {
                        capacity: self.policy.capacity,
                    });
                }
            }
            AdmissionSlot::ControlReserve => {
                if !request.context().is_stop() {
                    return Err(Error::InvalidState {
                        reason: "only an urgent request may hold a control-reserve slot".into(),
                    });
                }
                let reserve = self.targets[target]
                    .policy
                    .map_or(0, |policy| policy.control_reserve);
                if self.entries_holding(slot, target) >= usize::from(reserve) {
                    return Err(Error::ControlReserveExhausted {
                        target: target.camera(),
                        reserve: usize::from(reserve),
                    });
                }
            }
        }
        Ok(())
    }

    /// Establish a one-time owner halt fence without dispatching or discarding
    /// correlation for writes already in flight. Ready/retry work is terminal;
    /// future retry decisions consult the same bounded fence table.
    fn halt_in_turn_into(
        &mut self,
        turn: &InputTurn,
        target: TargetSlot,
        axes: crate::AffectedAxes,
        cutoff: u64,
        sink: &mut Vec<Effect>,
    ) {
        self.halt_inner(target, axes, cutoff, turn.now, sink);
        self.debug_assert_invariants();
    }

    fn halt_inner(
        &mut self,
        target: TargetSlot,
        axes: crate::AffectedAxes,
        cutoff: u64,
        now: Instant,
        sink: &mut Vec<Effect>,
    ) {
        let fences = &mut self.targets[target].motion_fences;
        for (index, fence) in fences.iter_mut().enumerate() {
            if axes.bits() & (1 << index) != 0 {
                *fence = (*fence).max(cutoff);
            }
        }
        let ids: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(id, entry)| {
                (entry.lifecycle().is_off_wire()
                    && self.motion_superseded(entry.request.context()).is_some())
                .then_some((*id, entry.certainty(AttemptEvidence::Unwritten)))
            })
            .collect();
        let mut effects = Decision::new(sink);
        for (id, certainty) in ids {
            // A queued or backed-off request is not on the wire. It can have
            // affected the camera only through an earlier attempt that the
            // camera did not conclusively reject.
            self.finish(
                id,
                Error::MotionSuperseded {
                    axes,
                    context: superseded_context(certainty),
                },
                now,
                &mut effects,
            );
        }
        self.finish_decision(now, effects)
    }

    /// A staged write is still unsent. Expire an overdue halt STOP here,
    /// without fabricating a transport failure or poisoning a healthy stream.
    fn expire_unwritten_halt_into(
        &mut self,
        request: RequestId,
        transmission: TransmissionId,
        now: Instant,
        sink: &mut Vec<Effect>,
    ) -> bool {
        let expired = self.expire_unwritten_halt_inner(request, transmission, now, sink);
        self.debug_assert_invariants();
        expired
    }

    fn expire_unwritten_halt_inner(
        &mut self,
        request: RequestId,
        transmission: TransmissionId,
        now: Instant,
        sink: &mut Vec<Effect>,
    ) -> bool {
        let Some(owner) = self.transmission_owner(request, transmission) else {
            sink.push(Effect::Ignored(IgnoreReason::StaleTransmission));
            return true;
        };
        if owner.write.kind() != CorrelationKind::Request {
            return false;
        }
        let Some(entry) = self.entries.get(&owner.request) else {
            sink.push(Effect::Ignored(IgnoreReason::StaleTransmission));
            return true;
        };
        if entry
            .request
            .context()
            .dispatch_deadline
            .is_none_or(|deadline| now < deadline)
        {
            return false;
        }
        let certainty = entry.certainty(AttemptEvidence::Unwritten);
        let mut effects = Decision::new(sink);
        self.finish_observed(
            owner.request,
            Error::timeout(FailureStage::Terminal, certainty),
            AttemptEvidence::Unwritten,
            now,
            &mut effects,
        );
        // A STOP that never left may decide a dispute naming it.
        self.apply_retro(now, &mut effects);
        self.finish_decision(now, effects);
        true
    }

    fn motion_superseded(&self, context: &RequestContext) -> Option<crate::AffectedAxes> {
        let motion = context.motion.filter(|_| !context.is_stop())?;
        let fences = self.targets[context.target].motion_fences;
        fences
            .iter()
            .enumerate()
            .any(|(index, cutoff)| {
                motion.axes.bits() & (1 << index) != 0
                    && *cutoff != 0
                    && context.submission_order <= *cutoff
            })
            .then_some(motion.axes)
    }

    fn admit(
        &mut self,
        request: RuntimeRequest,
        slot: AdmissionSlot,
        now: Instant,
    ) -> Result<RequestId, Error> {
        if let EngineSession::Ended(end) = &self.session {
            return Err(end.cause.clone());
        }
        let context = *request.context();
        context.validate_at(now)?;
        if let Some(axes) = self.motion_superseded(&context) {
            return Err(Error::MotionSuperseded {
                axes,
                context: FailureContext::new(FailureStage::PreAdmission, Certainty::NotAccepted),
            });
        }
        self.admission_slot_available(&request, slot)?;
        let Some(target_policy) = self.targets[context.target].policy else {
            return Err(Error::InvalidState {
                reason: "target is not registered in this session".into(),
            });
        };
        if let Some(shape) = request.payload_shape().filter(|_| self.mode().is_raw()) {
            let available = self
                .tracked_inquiry_shapes(context.target)
                .is_ok_and(|tracked| {
                    tracked.contains(shape)
                        || tracked.len() < MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET
                });
            if !available {
                return Err(Error::inquiry_shape_capacity_exhausted(
                    context.target.camera(),
                    MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET,
                ));
            }
        }
        if let Some(error) = self.latched_raw_lane_error(&request) {
            return Err(error);
        }
        if target_policy.cancellation != context.cancellation {
            return Err(Error::InvalidState {
                reason: "request cancellation policy conflicts with target registration".into(),
            });
        }
        let Some(id) = self.allocate_request_id() else {
            return Err(Error::RuntimeIdentityExhausted);
        };
        let admitted = self.entries.admit(
            id,
            request,
            AdmissionFacts {
                submitted_at: now,
                slot,
            },
        );
        if !admitted {
            return Err(Error::RuntimeIdentityExhausted);
        }
        Ok(id)
    }

    fn allocate_request_id(&mut self) -> Option<RequestId> {
        self.next_request_id
            .candidate()
            .map(RequestId::from_nonzero)
    }

    fn allocate_transmission_id(&mut self) -> Option<TransmissionId> {
        self.next_transmission_id
            .candidate()
            .map(TransmissionId::from_nonzero)
    }

    fn eligible_request(
        &self,
        lane: Lane,
        priority: usize,
        now: Instant,
        occupancy: &Occupancy,
    ) -> Option<RequestId> {
        self.entries
            .ready(lane, priority)
            .iter()
            .copied()
            .find(|id| self.dispatch_eligible(*id, now, occupancy))
    }

    fn select_dispatch(&self, now: Instant) -> Option<DispatchSelection> {
        let occupancy = Occupancy::build(&self.entries);
        for priority in (0..4).rev() {
            let inquiry = self.eligible_request(Lane::Inquiry, priority, now, &occupancy);
            let command = self.eligible_request(Lane::Command, priority, now, &occupancy);
            let request = match (inquiry, command) {
                (Some(inquiry), Some(command)) => {
                    let inquiry_first = inquiry <= command;
                    if inquiry_first {
                        inquiry
                    } else {
                        command
                    }
                }
                (Some(id), None) | (None, Some(id)) => id,
                (None, None) => continue,
            };
            return Some(DispatchSelection { request });
        }
        None
    }

    fn dispatch_one(&mut self, now: Instant, effects: &mut Decision) {
        if !matches!(self.session, EngineSession::Running) {
            return;
        }
        if let Some(selected) = self.select_dispatch(now) {
            self.dispatch_selected(selected, now, effects);
        }
    }

    /// Sends every pacing-eligible cancellation whose ACK has already
    /// established a command socket.  Cancellation is urgent with respect to
    /// ordinary queued work, but its wire write still advances the shared
    /// command spacing deadline.  Re-scan after each write so zero-spacing
    /// profiles drain the complete urgent set; a positive spacing naturally
    /// stops the scan after the first write advances [`Pacing::last_request`].
    fn drain_pending_cancellations(&mut self, now: Instant, effects: &mut Decision) {
        if !matches!(self.session, EngineSession::Running) {
            return;
        }
        loop {
            let pending = self
                .entries
                .iter()
                .filter_map(|(id, entry)| {
                    let pending = entry.lifecycle().pending_cancellation()?;
                    if self.cancellation_send_at(entry) > now {
                        return None;
                    }
                    Some((*id, pending))
                })
                .min_by_key(|(id, _)| *id);
            let Some((id, pending)) = pending else {
                return;
            };
            let before = self.entries.get(&id).map(|entry| entry.lifecycle());
            let last_sent = self.pacing.last_request();
            self.emit_cancel(id, pending, now, effects);
            let after = self.entries.get(&id).map(|entry| entry.lifecycle());
            if before == after && self.pacing.last_request() == last_sent {
                return;
            }
        }
    }

    fn dispatch_selected(
        &mut self,
        selected: DispatchSelection,
        now: Instant,
        effects: &mut Decision,
    ) {
        let request = selected.request;
        let Some(transmission) = self.allocate_transmission_id() else {
            self.finish(request, Error::RuntimeIdentityExhausted, now, effects);
            return;
        };
        let Some(entry) = self.entries.get(&request) else {
            return;
        };
        let lane = entry.request.lane();
        let target = entry.request.context().target;
        let dispatched_socket_capacity =
            (entry.reply_family() != SendingReply::Inquiry).then(|| {
                self.targets[target]
                    .policy
                    .map_or(CommandSocketCount::ONE, |policy| policy.command_sockets)
            });
        let wire = Arc::clone(entry.request.wire());
        let owes = self.ledger_owes(entry);
        let framing = match self.policy.wire.encoder() {
            ProtocolEncoder::Raw(encoder) => encoder.plan(),
            ProtocolEncoder::Sony(encoder) => encoder.plan(entry.current_sequence().map_or(
                SonyRequestSequence::AllocateFresh,
                SonyRequestSequence::Replay,
            )),
        };
        let requires_extended_pan_tilt_speed =
            entry.request.context().requires_extended_pan_tilt_speed;
        let reply = SendingReply::new(&entry.request);
        let stream_order = match self.split().1 {
            CorrelationState::RawStream(stream) => {
                owes.map(|owes| stream.ledger.push(target, request, owes))
            }
            CorrelationState::Sequenced | CorrelationState::RawDatagram(_) => None,
        };
        let lifecycle = Lifecycle::Sending {
            transmission,
            framing: framing.expectation(),
            started_at: now,
            attempt: AttemptFacts {
                socket_capacity: dispatched_socket_capacity,
                stream_order,
            },
            reply,
        };
        self.transition(request, lifecycle, now, effects);
        self.pacing.record(lane, now);
        effects.push(Effect::Transmit {
            transmission,
            request,
            kind: Transmission::Request {
                target,
                wire,
                framing,
                lane,
                requires_extended_pan_tilt_speed,
            },
        });
    }

    /// The per-lane dispatch gate: whether protocol state, rather than
    /// pacing, keeps ready `entry` from being written. Raw holds, protocol
    /// capacity, the raw command gate, the stream ledger and uncorrelated raw
    /// shapes each block; a byte-stream STOP that has waited out its
    /// [`Self::stop_wait_bound_in`] by `now` bypasses the command gates. Pacing
    /// (spacing and the inquiry cooldown) is [`Self::candidate_send_at`]'s
    /// alone.
    #[cfg(test)]
    fn dispatch_blocked(&self, entry: &Entry, now: Instant) -> bool {
        let target = entry.request.context().target;
        match entry.request.lane() {
            Lane::Inquiry => {
                self.raw_hold_blocks_dispatch(entry, target)
                    || self.inquiries_inflight_for(target) >= self.policy.inquiry_capacity
                    || self.uncorrelated_raw_shape_blocked(entry, target, now)
                    || (self.mode().is_raw()
                        && entry.request.payload_shape().is_some_and(|shape| {
                            self.overlapping_raw_inquiry_inflight(target, shape)
                                || self.mode().ledger().inquiry_state(target, shape)
                                    != LaneState::Clear
                        }))
                    || self.mode().ledger().forbids_crossing(target)
            }
            Lane::Command => {
                let Some(policy) = self.targets[target].policy else {
                    return true;
                };
                !self.stop_wait_ended(entry, now)
                    && (self.raw_hold_blocks_dispatch(entry, target)
                        || self.raw_command_gate_blocks(entry, target)
                        || self.command_capacity_used_by(entry, target)
                            >= policy.command_sockets.get()
                        || self.uncorrelated_raw_shape_blocked(entry, target, now))
            }
        }
    }

    fn dispatch_eligible(&self, id: RequestId, now: Instant, occupancy: &Occupancy) -> bool {
        self.entries.get(&id).is_some_and(|entry| {
            self.candidate_send_at(entry) <= now && !self.dispatch_blocked_in(entry, now, occupancy)
        })
    }

    fn candidate_send_at(&self, entry: &Entry) -> Instant {
        let request_spacing = self
            .policy
            .command_spacing
            .max(entry.request.context().control.minimum_spacing);
        // Urgent controls retain command-to-command physical pacing, but an
        // inquiry does not start their safety-lane clock (#712). Ordinary work
        // keeps the established session-wide pacing floor.
        let last_spacing_write =
            if entry.reply_family() != SendingReply::Inquiry && entry.request.context().is_stop() {
                self.pacing.last_command
            } else {
                self.pacing.last_request()
            };
        let mut at = last_spacing_write.map_or(entry.submitted_at, |last| {
            add_duration(last, request_spacing)
        });
        if entry.reply_family() == SendingReply::Inquiry {
            if let Some(last) = self.pacing.last_inquiry {
                at = at.max(add_duration(last, self.policy.inquiry_spacing));
            }
            if let Some(cooldown) = self.pacing.inquiry_cooldown_until {
                at = at.max(cooldown);
            }
        }
        at
    }

    /// Earliest wire time for an owner-issued cancellation.  Unlike ordinary
    /// requests, cancellation does not inherit the request's control-class
    /// minimum spacing; only the shared profile command-spacing floor applies.
    fn cancellation_send_at(&self, entry: &Entry) -> Instant {
        self.pacing.last_command.map_or(entry.submitted_at, |last| {
            add_duration(last, self.policy.command_spacing)
        })
    }

    /// Inquiry capacity is session-wide for Sony envelopes because sequence
    /// correlation shares one transport namespace. Raw VISCA has no sequence
    /// field, but its response source identifies the camera, so independent
    /// targets have independent positional inquiry lanes.
    #[cfg(test)]
    fn inquiries_inflight_for(&self, target: TargetSlot) -> usize {
        self.entries
            .values()
            .filter(|entry| {
                (!self.mode().is_raw() || entry.request.context().target == target)
                    && entry.lifecycle().is_live_inquiry()
            })
            .count()
    }

    #[cfg(test)]
    fn commands_inflight(&self, target: TargetSlot) -> usize {
        self.entries
            .values()
            .filter(|entry| {
                entry.reply_family() != SendingReply::Inquiry
                    && entry.request.context().target == target
                    && entry.lifecycle().is_command_on_wire()
            })
            .count()
    }

    /// Command sockets in use on `target`: live work, and on a datagram
    /// transport its exact socket holds. A byte stream's socket hold only
    /// identifies a late answer; if the camera is still busy there it
    /// allocates another socket or answers buffer-full, so the hold takes no
    /// capacity.
    #[cfg(test)]
    fn command_capacity_used(&self, target: TargetSlot) -> usize {
        let held = if self.mode().is_stream() {
            0
        } else {
            self.raw_socket_hold_count(target)
        };
        self.commands_inflight(target).saturating_add(held)
    }

    /// The command capacity `entry` finds used: a STOP is never held back by
    /// a socket hold (the camera allocates a free socket or answers
    /// buffer-full).
    #[cfg(test)]
    fn command_capacity_used_by(&self, entry: &Entry, target: TargetSlot) -> usize {
        if urgent_stop(entry) {
            self.commands_inflight(target)
        } else {
            self.command_capacity_used(target)
        }
    }

    /// When a STOP queued on a raw byte stream stops waiting for the rules
    /// that keep its answers decisive (#714 crossing, sibling STOPs, a
    /// dispute, socket capacity) and is written anyway: one ACK interval
    /// plus one ambiguity interval after admission, and never later than
    /// half way to its dispatch deadline. The stream ledger still binds
    /// every frame by write order, so the cost is only decisiveness: a frame
    /// it makes ambiguous binds to neither request, and the STOP may end
    /// unconfirmed (#795). Only the local write of a `NoReply` or
    /// `CompletionOnly` command, which has no write order yet, still holds
    /// it back (`None` until that write has left).
    #[cfg(test)]
    fn stop_wait_bound(&self, entry: &Entry) -> Option<Instant> {
        self.mode().stream().and_then(|stream| {
            stream.stop_wait_bound(entry, || {
                self.uncorrelated_write_in_flight(entry.request.context().target)
            })
        })
    }

    /// Whether a `NoReply` or `CompletionOnly` command's local write to
    /// `target` is in flight.
    #[cfg(test)]
    fn uncorrelated_write_in_flight(&self, target: TargetSlot) -> bool {
        self.entries.values().any(|entry| {
            entry.request.context().target == target
                && matches!(
                    entry.lifecycle(),
                    Lifecycle::Sending {
                        reply: SendingReply::CompletionOnly | SendingReply::NoReply,
                        ..
                    }
                )
        })
    }

    /// Whether `entry`, a STOP on a raw byte stream, has waited out
    /// [`Self::stop_wait_bound_in`] by `now` and is written whatever else waits.
    #[cfg(test)]
    fn stop_wait_ended(&self, entry: &Entry, now: Instant) -> bool {
        self.stop_wait_bound(entry)
            .is_some_and(|bound| now >= bound)
    }

    fn raw_hold(&self, target: TargetSlot, scope: RawHoldScope) -> Option<RawHold> {
        self.mode()
            .holds()
            .get(&RawHoldKey::new(target, scope))
            .copied()
    }

    /// Whether a target terminal tombstone prevents this ready request from
    /// dispatching.
    ///
    /// A broad uncorrelatable-command tombstone blocks response-bearing work;
    /// the narrow inquiry skew blocks only another inquiry. A second `NoReply`
    /// consumes no response at all, so it can safely write and extend a broad
    /// hold.
    fn raw_inquiry_holds(
        &self,
        target: TargetSlot,
    ) -> impl Iterator<Item = (PayloadShape, RawHold)> + '_ {
        self.mode()
            .holds()
            .iter()
            .filter_map(move |(key, hold)| match key.scope {
                RawHoldScope::Inquiry(shape) if key.target == target => Some((shape, *hold)),
                _ => None,
            })
    }

    fn raw_inquiry_hold_deadline(
        &self,
        target: TargetSlot,
        shape: PayloadShape,
    ) -> Option<Instant> {
        self.raw_inquiry_holds(target)
            .filter(|(held, _)| held.overlaps(shape))
            .map(|(_, hold)| hold.until)
            .max()
    }

    /// Derives the exact reservation union at this instant. Queued inquiries
    /// reserve their declaration without excluding disjoint wire scheduling;
    /// retired requests retain it through their debt or reply-skew hold.
    fn tracked_inquiry_shapes(&self, target: TargetSlot) -> Result<InquiryShapes, Box<str>> {
        let mut shapes = InquiryShapes::default();
        let admitted = self
            .entries
            .values()
            .filter(|entry| entry.request.context().target == target)
            .filter_map(|entry| entry.request.payload_shape());
        let owed = self.mode().ledger().inquiry_shapes(target);
        let held = self.raw_inquiry_holds(target).map(|(shape, _)| shape);
        for shape in admitted.chain(owed).chain(held) {
            if !shapes.try_insert(shape) {
                return Err("raw inquiry declaration reservation exceeds its bound".into());
            }
        }
        Ok(shapes)
    }

    #[cfg(test)]
    fn overlapping_raw_inquiry_inflight(&self, target: TargetSlot, shape: PayloadShape) -> bool {
        self.entries.values().any(|entry| {
            entry.request.context().target == target
                && entry
                    .request
                    .payload_shape()
                    .is_some_and(|live| live.overlaps(shape))
                && entry.lifecycle().is_live_inquiry()
        })
    }

    #[cfg(test)]
    fn raw_hold_blocks_dispatch(&self, entry: &Entry, target: TargetSlot) -> bool {
        self.raw_hold_dispatch_deadline_for(entry, target).is_some()
    }

    #[cfg(test)]
    fn raw_hold_dispatch_deadline_for(&self, entry: &Entry, target: TargetSlot) -> Option<Instant> {
        let response_bearing = entry.reply_family() != SendingReply::NoReply;
        // A STOP is never held back by a hold (#795): a frame it makes
        // ambiguous binds to neither request.
        let stop = urgent_stop(entry);
        let all_responses = (response_bearing && !stop)
            .then(|| self.raw_hold(target, RawHoldScope::AllResponses))
            .flatten()
            .map(|hold| hold.until);
        // A late inquiry reply has only target/data correlation. It can bind
        // to a later inquiry, but it cannot be mistaken for the ACK of an
        // ordinary or urgent command (#712).
        let inquiry_unkeyed = entry
            .request
            .payload_shape()
            .and_then(|shape| self.raw_inquiry_hold_deadline(target, shape));
        let pre_ack = (entry.reply_family() != SendingReply::Inquiry
            && !self.raw_preack_hold_bypass_available(entry, target))
        .then(|| self.raw_hold(target, RawHoldScope::PreAck))
        .flatten()
        .map(|hold| hold.until);
        // Exact socket holds consume physical command capacity. If live work
        // alone still leaves room, the earliest held socket release is the
        // deterministic point at which this request can be reconsidered.
        let socket_capacity =
            if entry.reply_family() == SendingReply::Inquiry || self.mode().is_stream() || stop {
                None
            } else {
                let capacity = self.command_sockets(target).get();
                let live = self.commands_inflight(target);
                let held = self.raw_socket_hold_count(target);
                (live < capacity && live.saturating_add(held) >= capacity)
                    .then(|| self.raw_socket_hold_wake(target))
                    .flatten()
            };
        all_responses
            .into_iter()
            .chain(inquiry_unkeyed)
            .chain(pre_ack)
            .chain(socket_capacity)
            .max()
    }

    #[cfg(test)]
    fn raw_preack_hold_bypass_available(&self, entry: &Entry, target: TargetSlot) -> bool {
        entry.reply_family() == SendingReply::AckThenCompletion
            && entry.request.context().is_stop()
            && self.raw_unacknowledged_command_count(target) == 0
    }

    fn raw_socket_hold_count(&self, target: TargetSlot) -> usize {
        [ViscaSocket::S1, ViscaSocket::S2]
            .into_iter()
            .filter(|socket| {
                self.raw_hold(target, RawHoldScope::Socket(*socket))
                    .is_some()
            })
            .count()
    }

    /// Whether a socket hold on `target` is still within its deadline.
    fn raw_socket_hold_running(&self, target: TargetSlot, now: Instant) -> bool {
        [ViscaSocket::S1, ViscaSocket::S2]
            .into_iter()
            .any(|socket| {
                self.raw_hold(target, RawHoldScope::Socket(socket))
                    .is_some_and(|hold| hold.until > now)
            })
    }

    #[cfg(test)]
    fn raw_socket_hold_wake(&self, target: TargetSlot) -> Option<Instant> {
        [ViscaSocket::S1, ViscaSocket::S2]
            .into_iter()
            .filter_map(|socket| {
                self.raw_hold(target, RawHoldScope::Socket(socket))
                    .map(|hold| hold.until)
            })
            .min()
    }

    /// Whether a raw completion-only command can be made the target's sole
    /// response-bearing command.  Besides live commands/inquiries, a terminal
    /// tombstone is incompatible: a socketless completion from the older
    /// command would otherwise be indistinguishable from this new command's
    /// terminal frame.
    fn raw_target_hold_active(&self, target: TargetSlot) -> bool {
        self.raw_hold(target, RawHoldScope::AllResponses).is_some()
            || self.raw_inquiry_holds(target).next().is_some()
    }

    /// Raw VISCA has no request identity before the camera assigns a socket.
    /// Keep one command per target in that unacknowledged window so a later
    /// ACK can never require temporal guessing between multiple candidates.
    ///
    /// A completion-only command (issue #700) is uncorrelated for its whole
    /// lifetime — it never earns a socket — so its [`Phase::AwaitingCompletion`]
    /// is included here: while it is in flight no other command may be
    /// dispatched to the target, which is what keeps its completion (and any
    /// socketless error) attributable to it alone.
    fn raw_command_unacknowledged(&self, target: TargetSlot) -> bool {
        self.mode().is_raw()
            && self.entries.values().any(|entry| {
                entry.request.context().target == target
                    && raw_unacknowledged_command_candidate(entry)
            })
    }

    #[cfg(test)]
    fn raw_unacknowledged_command_count(&self, target: TargetSlot) -> usize {
        match &self.mode() {
            CorrelationRef::Sequenced => 0,
            CorrelationRef::RawDatagram(_) | CorrelationRef::RawStream(_) => self
                .entries
                .values()
                .filter(|entry| {
                    entry.request.context().target == target
                        && raw_unacknowledged_command_candidate(entry)
                })
                .count(),
        }
    }

    /// Whether this request may create the one audited two-candidate raw state.
    ///
    /// An urgent safety command may cross one existing positional command
    /// candidate (#714). While both are open, the exact raw ACK/error resolvers
    /// see multiple candidates and bind to neither; no temporal guess is made.
    /// A third candidate is never admitted through this exception.
    #[cfg(test)]
    fn raw_urgent_gate_bypass_available(&self, entry: &Entry, target: TargetSlot) -> bool {
        self.mode().is_raw()
            && entry.reply_family() != SendingReply::Inquiry
            && entry.request.context().is_stop()
            && self.raw_unacknowledged_command_count(target) == 1
            // A disputed STOP's own answer must stay decisive, and behind a
            // `CompletionOnly` command that may still be rejected any dispute
            // must involve one STOP (#795).
            && !(self.mode().is_stream() && self.mode().ledger().forbids_crossing(target))
            // The #714 safety lane can cross only an ordinary ACK-bearing
            // positional candidate, or on a byte stream a completion-only one
            // (the ledger keeps the two apart, #795). Otherwise completion-only
            // and no-reply commands have no response identity and retain the
            // #700 exclusive target lane.
            && self.entries.values().all(|candidate| {
                candidate.request.context().target != target
                    || !raw_unacknowledged_command_candidate(candidate)
                    || (self.mode().is_stream()
                        && candidate.reply_family() == SendingReply::CompletionOnly)
                    || (candidate.reply_family() == SendingReply::AckThenCompletion
                        // Sibling STOPs in one halt may dispatch after ACK,
                        // without awaiting completion. Crossing each other's
                        // pre-ACK window would destroy raw attribution.
                        && !(entry.request.context().dispatch_deadline.is_some()
                            && candidate.request.context().dispatch_deadline.is_some()
                            && entry.request.context().submission_order == candidate.request.context().submission_order))
            })
    }

    /// Whether `target`'s raw command gate keeps `entry` from dispatching. A
    /// command lane that owes an answer blocks a command whether it waits
    /// (`Window`) or has latched: a latched lane's queued work fails in the
    /// due pass, and must never be written in between. A `NoReply` command
    /// is no exception on a byte stream: its possible rejection is owed too,
    /// and could not be told from the owed answer.
    #[cfg(test)]
    fn raw_command_gate_blocks(&self, entry: &Entry, target: TargetSlot) -> bool {
        let owed = self.mode().ledger().lane_state(target, Lane::Command) != LaneState::Clear;
        (self.raw_command_unacknowledged(target)
            && !self.raw_urgent_gate_bypass_available(entry, target))
            || ((self.raw_hold(target, RawHoldScope::PreAck).is_some() || owed)
                && !self.raw_preack_hold_bypass_available(entry, target))
            || self.latched_raw_lane_error(&entry.request).is_some()
    }

    /// Whether an uncorrelatable raw command is still occupying `target`.
    ///
    /// `CompletionOnly` has no socket through its whole lifecycle; `NoReply`
    /// has none during its local write. Both must keep same-target work from
    /// starting: a socketless error received during either phase otherwise has
    /// two possible owners.
    #[cfg(test)]
    fn raw_uncorrelated_command_inflight(&self, target: TargetSlot) -> bool {
        self.mode().is_raw()
            && self.entries.values().any(|entry| {
                entry.reply_family() != SendingReply::Inquiry
                    && entry.request.context().target == target
                    && match entry.reply_family() {
                        SendingReply::NoReply => entry.lifecycle().is_sending(),
                        SendingReply::CompletionOnly => {
                            entry.lifecycle().is_sending()
                                || entry.lifecycle().is_awaiting_completion()
                        }
                        SendingReply::Inquiry | SendingReply::AckThenCompletion => false,
                    }
            })
    }

    /// Whether `entry` is barred by raw uncorrelatable-shape exclusivity
    /// (issue #700).
    ///
    /// `CompletionOnly` earns no socket, and `NoReply` has no response identity
    /// during its write. Their completion/error traffic can be attributed only
    /// while they are the sole in-flight request on the target. They may start
    /// only when no command or inquiry is live, and same-target work may not
    /// start while either occupies the target. Other commands are already
    /// stopped by [`Self::raw_command_unacknowledged`]. A `CompletionOnly`
    /// command also waits while a `NoReply` command's possible rejection is
    /// owed on a byte stream (until a later first answer settles it: no
    /// window proves it will not come), and while a socket hold's deadline
    /// runs: the held
    /// command may still complete, and a camera that omits the socket nibble
    /// would make the socketless completion ambiguous. (A byte stream keeps
    /// the hold past its deadline for late frames naming the socket; it no
    /// longer delays work.) The rule is raw-only: Sony correlates by
    /// sequence, so these shapes need no exclusivity there.
    #[cfg(test)]
    fn uncorrelated_raw_shape_blocked(
        &self,
        entry: &Entry,
        target: TargetSlot,
        now: Instant,
    ) -> bool {
        match (&self.mode(), entry.reply_family()) {
            (CorrelationRef::Sequenced, _) => false,
            (_, SendingReply::Inquiry) => self.raw_uncorrelated_command_inflight(target),
            (_, SendingReply::NoReply) => {
                self.command_capacity_used(target) > 0 || self.raw_inquiry_inflight(target)
            }
            (mode, SendingReply::CompletionOnly) => {
                self.command_capacity_used(target) > 0
                    || self.raw_inquiry_inflight(target)
                    || self.raw_target_hold_active(target)
                    || mode.ledger().owes_rejection(target)
                    || (self.raw_socket_hold_running(target, now)
                        && !matches!(mode, CorrelationRef::RawStream(stream) if stream.learning[target].completion_sockets == CompletionSockets::Named))
            }
            (_, SendingReply::AckThenCompletion) => false,
        }
    }

    fn transition(&mut self, id: RequestId, to: Lifecycle, now: Instant, effects: &mut Decision) {
        match self.entries.transition(id, EntryChange::Lifecycle(to)) {
            Ok(Some(changed)) => effects.push(Effect::Transition {
                id,
                from: changed.from,
                to: changed.to,
            }),
            Ok(None) => {}
            Err(conflict) => self.finish_after_ack(
                id,
                Error::InvalidState {
                    reason: format!(
                        "socket {} is already owned by {:?}",
                        conflict.socket, conflict.owner
                    )
                    .into(),
                },
                false,
                now,
                effects,
            ),
        }
    }

    /// A write receipt is owned by the named entry's active sending phase.
    /// Transmission identities never repeat, so a retired phase cannot accept
    /// a late receipt and needs no second ownership map.
    fn transmission_owner(
        &self,
        request: RequestId,
        transmission: TransmissionId,
    ) -> Option<TransmissionOwner> {
        let lifecycle = self.entries.get(&request)?.lifecycle.active()?;
        Some(TransmissionOwner {
            request,
            write: lifecycle.active_write(transmission)?,
        })
    }

    fn transmission_finished(
        &mut self,
        request: RequestId,
        transmission: TransmissionId,
        result: WriteOutcome,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(owner) = self.transmission_owner(request, transmission) else {
            let reason = if self
                .entries
                .active_entries()
                .any(|(id, _)| self.transmission_owner(*id, transmission).is_some())
            {
                IgnoreReason::IncompatibleTransmissionResult
            } else {
                IgnoreReason::StaleTransmission
            };
            effects.push(Effect::Ignored(reason));
            return;
        };
        match result {
            WriteOutcome::NotSent(error) => self.transmission_not_sent(owner, error, now, effects),
            // Failure means bytes may have left. Its certainty and correlation
            // treatment cannot depend on when the write result is delivered.
            WriteOutcome::Failed(error) => self.failed_transmission(owner, error, now, effects),
            WriteOutcome::Sent(meta) => {
                if !self.write_result_after_total_budget(owner, now, effects) {
                    self.successful_transmission(owner, meta, now, effects);
                }
            }
        }
    }

    fn transmission_not_sent(
        &mut self,
        owner: TransmissionOwner,
        source: Error,
        now: Instant,
        effects: &mut Decision,
    ) {
        if let ActiveWrite::Cancellation(write) = owner.write {
            self.failed_cancellation_transmission(owner.request, write, source, now, effects);
            return;
        }
        let Some(entry) = self.entries.get(&owner.request) else {
            return;
        };
        let certainty = entry.certainty(AttemptEvidence::Unwritten);
        let possible = certainty == Certainty::Unconfirmed;
        let error = Error::transmission_not_sent(source, certainty);
        // No current bytes left, but an earlier attempt's owed answers and
        // raw ambiguity must survive this logical request's terminal.
        if possible && self.mode().is_raw() {
            self.quarantine_unconfirmed_raw(owner.request, now);
        }
        self.finish_observed(
            owner.request,
            error,
            AttemptEvidence::Unwritten,
            now,
            effects,
        );
    }

    /// Applies the admission-to-terminal budget before an active request write
    /// result may mutate protocol state.
    ///
    /// An input sampled exactly at the deadline remains input-first, but a
    /// result sampled strictly later is outside the request's total budget.
    /// Once a successful result is late, a raw command write can no longer
    /// prove whether the camera observed the frame, while a Sony write must
    /// not register a sequence. Failed writes always use the same evidence-led
    /// [`Self::failed_transmission`] path, independent of this budget.
    /// Cancellation writes deliberately
    /// bypass this guard because their ambiguity window, rather than the
    /// request retry budget, owns the active cancellation lifecycle.
    ///
    /// Keeping this at the engine's one transmission-result ingress also makes
    /// an [`EngineTurn::InputOnly`] write-result turn obey the same boundary
    /// as an ordinary [`Self::handle_into`] turn.
    fn write_result_after_total_budget(
        &mut self,
        owner: TransmissionOwner,
        now: Instant,
        effects: &mut Decision,
    ) -> bool {
        let Some((deadline, raw_active_command)) =
            self.entries.get(&owner.request).and_then(|entry| {
                entry.retry_budget_deadline().map(|deadline| {
                    (
                        deadline,
                        self.mode().is_raw()
                            && entry.reply_family() != SendingReply::Inquiry
                            && entry.lifecycle().is_sending(),
                    )
                })
            })
        else {
            return false;
        };
        if owner.write.kind() != CorrelationKind::Request || deadline >= now {
            return false;
        }
        let Some(entry) = self.entries.get(&owner.request) else {
            return false;
        };
        let error = entry.terminal_error(
            entry
                .retry_cause()
                .cloned()
                .unwrap_or_else(|| terminal_timeout(entry)),
            AttemptEvidence::Written,
        );
        self.mark_stream_answer_written(owner.request, false);

        if raw_active_command {
            // Remove the active transmission and retain raw correlation through its ambiguity window (or poisons in strict
            // recovery mode).
            self.terminate_unconfirmed_raw(
                owner.request,
                UnconfirmedScope::Lifecycle,
                now,
                effects,
            );
        } else {
            // A raw inquiry can be physically uncertain while
            // its write result is still pending too. Retain the same bounded
            // shape hold before normal terminal cleanup so a delayed reply
            // cannot bind to an overlapping successor. A successful stream
            // write proves the bytes entered the stream, so there the reply
            // is owed instead (`retire_stream_answers`).
            if !self.mode().is_stream() {
                self.quarantine_raw_inquiry_correlation(owner.request, now);
            }
            // A late successful Sony result cannot
            // extend the budget or mutate correlation. Preserve the prior
            // retry cause when there is one, matching ordinary budget expiry.
            self.finish_observed(owner.request, error, AttemptEvidence::Written, now, effects);
        }
        true
    }

    fn successful_transmission(
        &mut self,
        owner: TransmissionOwner,
        meta: TransmissionMeta,
        now: Instant,
        effects: &mut Decision,
    ) {
        let metadata_valid = match (owner.write.framing(), meta) {
            (RequestFraming::Raw, TransmissionMeta::Raw) => true,
            (
                RequestFraming::Sony {
                    sequence: SonyRequestSequence::AllocateFresh,
                },
                TransmissionMeta::Sony { .. },
            ) => true,
            (
                RequestFraming::Sony {
                    sequence: SonyRequestSequence::Replay(requested),
                },
                TransmissionMeta::Sony { sequence },
            ) => requested == sequence,
            (RequestFraming::Raw, TransmissionMeta::Sony { .. })
            | (RequestFraming::Sony { .. }, TransmissionMeta::Raw) => false,
        };
        if !metadata_valid {
            self.finish_observed(
                owner.request,
                Error::InvalidState {
                    reason: "transmission metadata conflicts with requested framing".into(),
                },
                AttemptEvidence::Written,
                now,
                effects,
            );
            return;
        }
        let _ = self.entries.transition(
            owner.request,
            EntryChange::Written {
                receipt: meta,
                kind: owner.write.kind(),
            },
        );
        self.mark_stream_answer_written(
            owner.request,
            owner.write.kind() == CorrelationKind::Cancellation,
        );
        let Some(entry) = self.entries.get(&owner.request) else {
            return;
        };
        match owner.write {
            ActiveWrite::Request { attempt, reply, .. } => {
                let context = *entry.request.context();
                match reply {
                    SendingReply::Inquiry => {
                        self.transition(
                            owner.request,
                            Lifecycle::AwaitingReply {
                                sent_at: now,
                                deadline: add_duration(now, context.timeout.inquiry.get()),
                                attempt,
                            },
                            now,
                            effects,
                        );
                    }
                    SendingReply::AckThenCompletion => {
                        self.transition(
                            owner.request,
                            Lifecycle::AwaitingAck {
                                sent_at: now,
                                deadline: add_duration(now, context.timeout.ack.get()),
                                attempt,
                                cancellation: None,
                            },
                            now,
                            effects,
                        );
                    }
                    SendingReply::CompletionOnly => {
                        self.transition(
                            owner.request,
                            Lifecycle::AwaitingCompletion {
                                sent_at: now,
                                deadline: add_duration(now, context.timeout.completion.get()),
                                attempt,
                            },
                            now,
                            effects,
                        );
                    }
                    SendingReply::NoReply => {
                        self.quarantine_raw_terminal(owner.request, now);
                        self.finish_observed(
                            owner.request,
                            CommandOutcome::Written,
                            AttemptEvidence::Written,
                            now,
                            effects,
                        );
                    }
                }
            }
            ActiveWrite::Cancellation(write) => {
                let observation_deadline =
                    add_duration(now, entry.request.context().timeout.cancellation.get());
                self.transition(
                    owner.request,
                    write.written(observation_deadline),
                    now,
                    effects,
                );
            }
        }
    }

    /// Applies one failed transport write.
    ///
    /// **Datagram**: exactly one request fails and the session keeps running.
    /// The write may have reached the camera. Raw commands therefore retain
    /// an inert correlation hold and finish unconfirmed (or poison under the
    /// strict opt-in); raw inquiries retain their reply-skew hold. Sony command
    /// errors retain their transport cause under an unconfirmed terminal.
    ///
    /// **Stream**: the session is poisoned. This is a deliberate, explicit
    /// policy rather than an accident. Public transport sends return
    /// `Result<(), Error>`, so a physical `WriteOutcome::Failed` cannot prove
    /// that no byte of the frame reached the wire;
    /// a partially written frame desynchronizes the camera's parser for every
    /// frame that follows, which makes the byte-stream position unknowable.
    /// Fail-one-command would therefore leave the caller believing a session
    /// that is already unusable is healthy, and [`Error::StreamPoisoned`] is
    /// the one terminal error that answers
    /// [`Error::requires_new_session`](crate::Error::requires_new_session)
    /// correctly (issue #564). Refusal before transport I/O has its own
    /// `WriteOutcome::NotSent` path and never enters this handler.
    ///
    /// The exact transport cause is never lost: it is carried in the poison
    /// source chain. The failed command is failed individually and the stream
    /// session is poisoned, because after an unconfirmed write the byte stream
    /// can no longer be trusted to be frame-aligned.
    fn failed_transmission(
        &mut self,
        owner: TransmissionOwner,
        error: Error,
        now: Instant,
        effects: &mut Decision,
    ) {
        if self.policy.transport == TransportKind::Stream {
            self.terminate_session(
                EndStatus::Poisoned,
                Error::stream_poisoned(error),
                now,
                effects,
            );
            return;
        }
        match owner.write {
            ActiveWrite::Request { .. } => {
                let raw_command = self.mode().is_raw()
                    && self
                        .entries
                        .get(&owner.request)
                        .is_some_and(|entry| entry.reply_family() != SendingReply::Inquiry);
                if raw_command {
                    self.terminate_unconfirmed_raw(
                        owner.request,
                        UnconfirmedScope::Lifecycle,
                        now,
                        effects,
                    );
                } else {
                    self.quarantine_raw_inquiry_correlation(owner.request, now);
                    self.finish_terminal_error(
                        owner.request,
                        error,
                        AttemptEvidence::Written,
                        now,
                        effects,
                    );
                }
            }
            ActiveWrite::Cancellation(write) => {
                self.failed_cancellation_transmission(owner.request, write, error, now, effects);
            }
        }
    }

    fn failed_cancellation_transmission(
        &mut self,
        request: RequestId,
        write: CancellationWrite,
        error: Error,
        now: Instant,
        effects: &mut Decision,
    ) {
        // Retire only the staged cancellation answer. The proof preserves the
        // acknowledged operation's original socket and completion deadline.
        self.retire_answers(request, now, effects);
        self.transition(request, write.refused(), now, effects);
        effects.push(Effect::CancellationObservation {
            id: request,
            observation: CancellationObservation::Failed(error),
        });
    }

    /// Applies one transient receive-side transport failure.
    ///
    /// The engine acts on the fault only for requests whose envelope supplies
    /// safe evidence. A sequenced Sony command
    /// still waiting for its ACK is retried under its own bounded retry policy.
    ///
    /// A raw command awaiting its ACK has no sequence key to replay, but a
    /// transient receive fault consumes nothing and cannot desynchronize raw
    /// framing (the recovery doc downgrades a fatal read to a close on exactly
    /// that fact). So by default (issue #671) the fault does **not** terminalize
    /// the in-flight raw command: it is neither retried (replay is unsafe) nor
    /// failed here, but left to ride to its own ACK deadline, where — if no ACK
    /// arrives — it quarantines per-request rather than poisoning the session.
    /// The read-side pause/escalation the owner already performs handles the
    /// transport itself. The strict opt-in mode instead poisons the whole
    /// session with [`Error::StreamPoisoned`] only when no cancellation intent
    /// is recorded. A cancellation already in flight follows its own live
    /// pre-ACK or socket-resolution deadline and poisons under strict mode only
    /// if that deadline remains unconfirmed. The classic case
    /// is a UDP `recv` returning ECONNREFUSED because an earlier datagram drew
    /// an ICMP port-unreachable.
    ///
    /// The scan is deliberately conservative: inquiries are untouched (only commands are examined),
    /// and a request with cancellation in flight is left to its ambiguity deadline, because
    /// retrying it would abandon the quarantine that owns its socket.
    fn receive_fault(&mut self, error: &Error, now: Instant, effects: &mut Decision) {
        if !matches!(self.session, EngineSession::Running) {
            effects.push(Effect::Ignored(IgnoreReason::SessionNotRunning));
            return;
        }
        let mut affected: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(id, entry)| {
                if entry.reply_family() == SendingReply::Inquiry
                    || !entry.lifecycle().is_awaiting_ack()
                    || entry.lifecycle().cancellation_ambiguity().is_some()
                {
                    return None;
                }
                entry
                    .lifecycle()
                    .retry_eligibility(false)
                    .map(|retry| (*id, retry))
            })
            .collect();
        affected.sort_unstable_by_key(|(id, _)| *id);
        if self.raw_unconfirmed_poison() && !affected.is_empty() {
            self.poison_strict_unconfirmed(now, effects);
            return;
        }
        if self.mode().is_raw() {
            // Default: leave any unacknowledged raw command to its own ACK
            // deadline; never replay an unconfirmed raw command.
            return;
        }
        for (id, retry) in affected {
            self.schedule_retry(
                id,
                retry,
                now,
                RetryCause::Unconfirmed(error.clone()),
                Backoff::Uncapped,
                effects,
            );
        }
    }

    fn resolve_sequence(
        &self,
        target: TargetSlot,
        sequence: EnvelopeSequence,
    ) -> Result<(RequestId, CorrelationKind), IgnoreReason> {
        let exact_candidates = self.entries.exact_owners(sequence.value);
        if sequence.width == SequenceWidth::Full32 {
            if exact_candidates.is_empty() {
                return Err(IgnoreReason::UnmatchedSequencedFrame);
            }
            let compatible = unique_requests(
                exact_candidates
                    .iter()
                    .copied()
                    .filter(|owner| self.owner_target(*owner) == Some(target)),
            );
            return match compatible.as_slice() {
                [owner] => Ok((owner.request, owner.kind)),
                [] => Err(IgnoreReason::TargetIncompatibleSequence),
                _ => Err(IgnoreReason::UnmatchedSequencedFrame),
            };
        }
        let unique = unique_requests(
            self.entries
                .lower_owners(sequence.value as u16)
                .iter()
                .copied()
                .filter(|owner| self.owner_target(*owner) == Some(target)),
        );
        match unique.as_slice() {
            [owner] => Ok((owner.request, owner.kind)),
            [] => Err(IgnoreReason::UnmatchedSequencedFrame),
            _ => Err(IgnoreReason::AmbiguousLower16Sequence),
        }
    }

    fn owner_target(&self, owner: CorrelationOwner) -> Option<TargetSlot> {
        self.entries
            .get(&owner.request)
            .map(|entry| entry.request.context().target)
    }

    fn frame(&mut self, frame: DecodedFrame, now: Instant, effects: &mut Decision) {
        if !matches!(self.session, EngineSession::Running) {
            effects.push(Effect::Ignored(IgnoreReason::SessionNotRunning));
            return;
        }
        let unmatched = if self.raw_reply_shape_mismatch(&frame) {
            IgnoreReason::ReplyShapeMismatch
        } else {
            IgnoreReason::UnmatchedFrame
        };
        // Sony transport-control replies are observable protocol diagnostics,
        // never VISCA request-correlation evidence.
        if matches!(frame.response, DecodedResponse::SonyControl { .. }) {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        // On a raw byte stream the correlation ledger is the single rule for
        // every unsequenced frame (#795). On a raw datagram transport, an
        // unsequenced terminal response cannot identify which released
        // correlation it answers: check the fixed-size tombstones before the
        // ordinary raw resolver, which could otherwise advance a successor
        // awaiting its ACK, finish a successor inquiry with stale payload, or fail/retry
        // either on a socketless error.
        let resolved = if let Some(sequence) = frame.sequence {
            match self.resolve_sequence(frame.target, sequence) {
                Ok(owner) => {
                    self.retire_stale_correlated_request(owner.0, now, effects);
                    Some(owner)
                }
                Err(reason) => {
                    effects.push(Effect::Ignored(reason));
                    return;
                }
            }
        } else {
            let (core, mode) = self.split();
            match mode {
                CorrelationState::Sequenced => {
                    effects.push(Effect::Ignored(IgnoreReason::MalformedFrame));
                    return;
                }
                CorrelationState::RawStream(stream) => {
                    let mut context = StreamContext { core, stream };
                    context.retire_stale_stream_candidates(&frame, now, effects);
                    if !matches!(context.session, EngineSession::Running) {
                        return;
                    }
                    context
                        .resolve_raw_stream(&frame, now, effects)
                        .map(|id| (id, CorrelationKind::Request))
                }
                CorrelationState::RawDatagram(holds) => {
                    let mut context = DatagramContext { core, holds };
                    if context.raw_held_response(&frame) {
                        effects.push(Effect::Ignored(unmatched));
                        return;
                    }
                    context.resolve_raw(&frame).and_then(|id| {
                        context.retire_stale_correlated_request(id, now, effects);
                        (!context.raw_held_response(&frame))
                            .then_some((id, CorrelationKind::Request))
                    })
                }
            }
        };
        self.apply_retro(now, effects);
        if !matches!(self.session, EngineSession::Running) {
            return;
        }
        let Some((id, correlation_kind)) = resolved else {
            effects.push(Effect::Ignored(unmatched));
            return;
        };
        let compatible_target = self
            .entries
            .get(&id)
            .is_some_and(|entry| entry.request.context().target == frame.target);
        if !compatible_target {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        match frame.response {
            DecodedResponse::Ack { socket } => self.ack(id, socket, now, effects),
            DecodedResponse::Completion { socket } => {
                if correlation_kind == CorrelationKind::Cancellation {
                    self.confirm_cancelled(id, now, effects);
                } else {
                    self.completion(id, socket, now, effects);
                }
            }
            DecodedResponse::InquiryReply { payload } => {
                if correlation_kind == CorrelationKind::Cancellation {
                    effects.push(Effect::Ignored(IgnoreReason::MalformedFrame));
                } else {
                    let reply = InquiryOutcome::Reply { payload };
                    self.inquiry_reply(id, reply, now, effects);
                }
            }
            DecodedResponse::Error { socket, code } => {
                self.camera_error(id, correlation_kind, socket, code, now, effects);
            }
            DecodedResponse::SonyControl { .. }
            | DecodedResponse::NetworkChange
            | DecodedResponse::Unknown => {
                effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            }
        }
    }

    /// Whether an unsequenced raw response must be ignored because a released
    /// terminal predecessor still owns the target's available correlation
    /// evidence.
    ///
    /// A `NoReply`/`CompletionOnly` hold is broad because it has no response
    /// identity at all.  An inquiry hold is intentionally narrower: inquiry
    /// data and socketless errors are unkeyed, but an exact live socket remains
    /// authoritative for a concurrently executing command.  This matters for
    /// raw two-socket cameras, where an inquiry can legitimately finish while
    /// another command is executing on S1 or S2.
    fn raw_held_response(&self, frame: &DecodedFrame) -> bool {
        let target = frame.target;
        // The commands behind this hold (`NoReply`, `CompletionOnly`) never
        // ACK and earn no socket, so a STOP written inside it (#795) still
        // receives its own ACK, and the completion or execution error naming
        // the socket it executes on (a rejection names a free socket).
        // Anything else stays inert: a socketless error could be either's.
        if self.raw_hold(target, RawHoldScope::AllResponses).is_some() {
            return match &frame.response {
                DecodedResponse::Completion {
                    socket: Some(socket),
                }
                | DecodedResponse::Error {
                    socket: Some(socket),
                    ..
                } => self.socket_owner(target, *socket).is_none(),
                DecodedResponse::Completion { .. }
                | DecodedResponse::InquiryReply { .. }
                | DecodedResponse::Error { .. } => true,
                _ => false,
            };
        }
        let inquiry_unkeyed = self.raw_inquiry_holds(target).next().is_some();
        let pre_ack = self.raw_hold(target, RawHoldScope::PreAck).is_some();
        let any_socket = self.raw_socket_hold_count(target) != 0;
        match &frame.response {
            // A live unique pre-ACK request was admitted before this inquiry
            // hold. Its complete ACK is still attributable; the hold prevents
            // a successor from becoming a competing candidate.
            DecodedResponse::Ack { .. } => {
                (pre_ack || inquiry_unkeyed) && self.unique_raw_command_candidate(target).is_none()
            }
            // An inquiry reply is target/data-correlated and a socketless error
            // has no command/inquiry discriminator, so both remain inert.
            DecodedResponse::InquiryReply { payload } => self
                .raw_inquiry_holds(target)
                .any(|(shape, _)| shape.admits(payload)),
            DecodedResponse::Error { socket: None, .. } => inquiry_unkeyed || pre_ack || any_socket,
            // A named terminal is safe only when the exact socket is still
            // owned. An unowned named frame must not fall through to any
            // successor inquiry while this hold is active.
            DecodedResponse::Completion {
                socket: Some(socket),
            }
            | DecodedResponse::Error {
                socket: Some(socket),
                ..
            } => {
                self.raw_hold(target, RawHoldScope::Socket(*socket))
                    .is_some()
                    || ((inquiry_unkeyed || pre_ack)
                        && self.socket_owner(target, *socket).is_none())
            }
            // Socketless completion is safe solely when an already-live socket
            // owner makes it uniquely attributable. It cannot be stale inquiry
            // data and no successor can dispatch under this hold.
            DecodedResponse::Completion { socket: None } => {
                pre_ack
                    || any_socket
                    || (inquiry_unkeyed && self.sole_socket_holder(target).is_none())
            }
            DecodedResponse::SonyControl { .. }
            | DecodedResponse::NetworkChange
            | DecodedResponse::Unknown => false,
        }
    }

    fn resolve_raw(&self, frame: &DecodedFrame) -> Option<RequestId> {
        let target = frame.target;
        match &frame.response {
            DecodedResponse::Ack { .. } => self.unique_raw_command_candidate(target),
            DecodedResponse::Completion { socket } => match socket {
                // A completion-only command owns no socket, so socket ownership
                // resolves nothing for it; because it holds the target's command
                // channel alone (issue #700), a completion that resolves to no
                // socket owner falls back to it. When a socket *is* owned, that
                // ownership is authoritative and wins.
                Some(socket) => self
                    .socket_owner(target, *socket)
                    .or_else(|| self.completion_only_candidate(target)),
                // A camera that answers `90 50 FF` sends no socket nibble, so
                // the frame is attributable to the sole socket holder or, failing
                // that, the sole completion-only command on the target.
                None => self
                    .sole_socket_holder(target)
                    .or_else(|| self.completion_only_candidate(target)),
            },
            DecodedResponse::InquiryReply { payload } => self.raw_inquiry_matching(target, payload),
            DecodedResponse::Error { socket, code } => {
                if let Some(socket) = socket {
                    return self.resolve_raw_named_error(target, *socket, *code);
                }
                let inquiry_owner = self.sole_raw_inquiry(target);
                let inquiry_live = inquiry_owner.is_some() || self.raw_inquiry_inflight(target);
                if self.raw_command_unacknowledged(target) {
                    // A socketless error can identify a command only when it
                    // is the sole possible command owner and no inquiry is
                    // competing for the same frame.  In particular, never
                    // temporally attribute it to an Executing command.
                    if !inquiry_live {
                        return self.unique_raw_error_candidate(target);
                    }
                    return None;
                }
                inquiry_owner
            }
            DecodedResponse::SonyControl { .. }
            | DecodedResponse::NetworkChange
            | DecodedResponse::Unknown => None,
        }
    }

    /// Resolves a raw error that names a socket, on a datagram transport.
    ///
    /// A camera rejects a command it cannot start with `z0 6y ...`, where `y`
    /// is the socket it allocated for that command, and that socket is free
    /// in our index because the command never earned an ACK (a PTZOptics G2
    /// focus STOP in auto-focus mode names its next free rotation socket).
    /// The named socket is therefore exact evidence, in the same way an ACK
    /// naming a free socket is. An inquiry is never allocated a socket: it is
    /// answered on socket 0 (`docs/visca_reference.md` §6.2), so its error is
    /// socketless (§6.3), and a named error is never an inquiry's.
    ///
    /// - `0x04`/`0x05` answer only a cancellation packet, so they route only
    ///   to the request whose emitted cancellation targets that socket.
    /// - A socket owned by a live request routes to that owner (its execution
    ///   error), unless another command on the target is unacknowledged: its
    ///   rejection could name it too. That is the #714 ambiguity: bind to
    ///   neither, never guess by recency.
    /// - A rejection code naming a free socket with no exact-socket hold routes
    ///   to the target's unique unacknowledged ACK-bearing command.
    ///   Target-wide holds have already discarded the frame in
    ///   [`Self::raw_held_response`].
    /// - Anything else identifies no request.
    ///
    /// On a byte stream [`Self::resolve_raw_stream`] decides instead (#795).
    fn resolve_raw_named_error(
        &self,
        target: TargetSlot,
        socket: ViscaSocket,
        code: u8,
    ) -> Option<RequestId> {
        // A `0x04`/`0x05` naming a socket answers only the cancellation
        // packet emitted for it (H1, #795).
        if is_cancel_reply(code) {
            return self.socket_owner(target, socket).filter(|owner| {
                self.entries
                    .get(owner)
                    .is_some_and(|entry| entry.cancel_attempted_socket() == Some(socket))
            });
        }
        if let Some(owner) = self.socket_owner(target, socket) {
            return (!self.raw_command_unacknowledged(target)).then_some(owner);
        }
        if !is_rejection(code)
            || self
                .raw_hold(target, RawHoldScope::Socket(socket))
                .is_some()
        {
            return None;
        }
        self.unique_raw_command_candidate(target)
    }

    fn sole_raw_inquiry(&self, target: TargetSlot) -> Option<RequestId> {
        let mut candidates = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry.request.context().target == target
                    && entry.request.is_inquiry()
                    && entry.lifecycle().is_awaiting_reply()
            })
            .map(|(id, _)| *id);
        let candidate = candidates.next()?;
        candidates.next().is_none().then_some(candidate)
    }

    fn raw_inquiry_matching(&self, target: TargetSlot, payload: &[u8]) -> Option<RequestId> {
        let mut candidates = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry.request.context().target == target
                    && entry
                        .request
                        .payload_shape()
                        .is_some_and(|shape| shape.admits(payload))
                    && entry.lifecycle().is_awaiting_reply()
            })
            .map(|(id, _)| *id);
        let candidate = candidates.next()?;
        // Overlap exclusion guarantees uniqueness. Retaining this closed check
        // also makes an invalid private state fail without guessing an owner.
        candidates.next().is_none().then_some(candidate)
    }

    fn raw_reply_shape_mismatch(&self, frame: &DecodedFrame) -> bool {
        let DecodedResponse::InquiryReply { payload } = &frame.response else {
            return false;
        };
        if !self.mode().is_raw() || frame.sequence.is_some() {
            return false;
        }
        let target = frame.target;
        let live = self
            .entries
            .values()
            .filter(|entry| {
                entry.request.context().target == target && entry.lifecycle().is_live_inquiry()
            })
            .filter_map(|entry| entry.request.payload_shape());
        let mut expected = false;
        for shape in live
            .chain(self.mode().ledger().inquiry_shapes(target))
            .chain(self.raw_inquiry_holds(target).map(|(shape, _)| shape))
        {
            expected = true;
            if shape.admits(payload) {
                return false;
            }
        }
        expected
    }

    /// Whether an inquiry has reached the raw wire on this target. Inquiry
    /// data has structural evidence; a socketless error has none and remains
    /// ambiguous with every outstanding inquiry and pre-ACK command.
    fn raw_inquiry_inflight(&self, target: TargetSlot) -> bool {
        self.entries.values().any(|entry| {
            entry.request.context().target == target && entry.lifecycle().is_live_inquiry()
        })
    }

    fn socket_owner(&self, target: TargetSlot, socket: ViscaSocket) -> Option<RequestId> {
        self.entries.socket_owner(target, socket)
    }

    /// The single request currently holding a command socket on `target`, if
    /// exactly one does.
    fn sole_socket_holder(&self, target: TargetSlot) -> Option<RequestId> {
        match (
            self.socket_owner(target, ViscaSocket::S1),
            self.socket_owner(target, ViscaSocket::S2),
        ) {
            (Some(id), None) | (None, Some(id)) => Some(id),
            (Some(first), Some(second)) if first == second => Some(first),
            _ => None,
        }
    }

    /// The unique raw command on `target` whose ACK-bearing reply has not been
    /// established.
    ///
    /// Raw dispatch normally admits one unacknowledged command per target.
    /// Issue #714 permits one intrinsically Urgent command to cross one open
    /// positional candidate. This resolver is the safety boundary for that
    /// explicit two-candidate state: it returns `None` and never turns
    /// admission order or recency into an ACK/error guess. `AwaitingAck` is
    /// the only phase eligible for a new socket assignment.
    fn sole_raw_candidate(
        &self,
        target: TargetSlot,
        predicate: impl Fn(&Entry) -> bool,
    ) -> Option<RequestId> {
        match self.mode() {
            CorrelationRef::Sequenced => None,
            CorrelationRef::RawDatagram(_) | CorrelationRef::RawStream(_) => {
                let mut candidates = self
                    .entries
                    .iter()
                    .filter(|(_, entry)| {
                        entry.request.context().target == target && predicate(entry)
                    })
                    .map(|(id, _)| *id);
                let first = candidates.next()?;
                candidates.next().is_none().then_some(first)
            }
        }
    }

    fn unique_raw_command_candidate(&self, target: TargetSlot) -> Option<RequestId> {
        self.sole_raw_candidate(target, |entry| entry.lifecycle().is_awaiting_ack())
    }

    /// The unique completion-only raw command on `target` awaiting (or about to
    /// await) its completion, if exactly one exists (issue #700).
    ///
    /// A completion-only command owns no socket, so a completion for it cannot be
    /// resolved by [`Self::socket_owner`]/[`Self::sole_socket_holder`]. It is the
    /// sole in-flight command on its target (the dispatch gate guarantees it), so
    /// its completion is unambiguous. It fails closed if a second candidate
    /// exists; `AwaitingCompletion` itself proves the command's reply family.
    fn completion_only_candidate(&self, target: TargetSlot) -> Option<RequestId> {
        self.sole_raw_candidate(target, |entry| entry.lifecycle().is_awaiting_completion())
    }

    /// The unique raw command that may own a socketless camera error on
    /// `target`, if exactly one exists.
    ///
    /// ACK routing deliberately uses [`Self::unique_raw_command_candidate`].
    /// `NoReply` must never be an error candidate because its local-write
    /// terminal ignores every camera response. Error routing has one additional
    /// valid candidate — a completion-only command in
    /// [`Phase::AwaitingCompletion`].  Keep this extension local to errors so
    /// the ordinary ACK candidate and its exclusivity rules remain unchanged.
    /// As with the ACK candidate, a second possible owner fails closed rather
    /// than allowing temporal recency to choose one.
    fn unique_raw_error_candidate(&self, target: TargetSlot) -> Option<RequestId> {
        self.sole_raw_candidate(target, |entry| {
            entry.reply_family() != SendingReply::Inquiry
                && (entry.lifecycle().is_awaiting_ack()
                    || entry.lifecycle().is_awaiting_completion())
        })
    }

    fn command_sockets(&self, target: TargetSlot) -> CommandSocketCount {
        self.targets[target]
            .policy
            .map_or(CommandSocketCount::ONE, |policy| policy.command_sockets)
    }

    /// Socket assignment may need the capacity that was in force when this
    /// exact command attempt was dispatched. A live retune lowers admission
    /// capacity immediately, but it does not revoke the second physical socket
    /// from a pre-retune attempt already draining under the old capacity.
    fn assignment_socket_count(&self, target: TargetSlot, id: RequestId) -> CommandSocketCount {
        let current = self.command_sockets(target);
        self.entries
            .get(&id)
            .and_then(|entry| {
                entry.dispatched_socket_capacity().or_else(|| {
                    entry
                        .lifecycle()
                        .previous_attempt()
                        .and_then(|attempt| attempt.socket_capacity)
                })
            })
            .map_or(current, |dispatched| current.max(dispatched))
    }

    fn socket_available(&self, target: TargetSlot, socket: ViscaSocket, id: RequestId) -> bool {
        self.raw_hold(target, RawHoldScope::Socket(socket))
            .is_none()
            && self
                .socket_owner(target, socket)
                .is_none_or(|owner| owner == id)
    }

    /// Chooses the camera's named socket, or the first free physical socket
    /// for an ACK that does not name one. Named ownership is reconciled by
    /// `assign_ack_socket` before this helper; inventing a different socket
    /// would send cancellation to the wrong command.
    fn assign_socket(
        &self,
        target: TargetSlot,
        requested: Option<ViscaSocket>,
        id: RequestId,
    ) -> Option<ViscaSocket> {
        let assignment_socket_count = self.assignment_socket_count(target, id);
        if let Some(socket) = requested {
            if self.socket_available(target, socket, id) {
                return Some(socket);
            }
            return None;
        }
        assignment_socket_count
            .sockets()
            .find(|socket| self.socket_available(target, *socket, id))
    }

    /// A compatibility ACK cannot reconstruct physical socket occupancy after
    /// a completion was retained ambiguously, or while inert socket evidence
    /// may already have completed unseen. In particular, the camera can reuse
    /// a socket that the host still indexes to a predecessor.
    fn uncertain_socketless_stream_ack(&self, target: TargetSlot) -> bool {
        let Some(stream) = self.mode().stream() else {
            return false;
        };
        let learning = &stream.learning[target];
        self.raw_socket_hold_count(target) != 0
            || learning.pending_socketless_completions.is_some()
            || learning.unlearned_completions != 0
            || self
                .command_sockets(target)
                .sockets()
                .all(|socket| self.socket_owner(target, socket).is_some())
    }

    /// Applies the camera's authoritative named socket assignment.
    ///
    /// Raw predecessors become ambiguity holds. Sony predecessors lose their
    /// socket claim but retain their exact-sequence terminal and deadline.
    fn assign_ack_socket(
        &mut self,
        target: TargetSlot,
        requested: Option<ViscaSocket>,
        id: RequestId,
        now: Instant,
        effects: &mut Decision,
    ) -> Option<ViscaSocket> {
        if let Some(socket) = requested {
            if let Some(stale) = self
                .socket_owner(target, socket)
                .filter(|owner| *owner != id)
            {
                if self.mode().is_raw() {
                    self.terminate_unconfirmed_raw(
                        stale,
                        UnconfirmedScope::Displaced,
                        now,
                        effects,
                    );
                    if !matches!(self.session, EngineSession::Running) {
                        return None;
                    }
                } else {
                    self.displace_sony_socket(stale, now, effects);
                }
            }
            let (core, mode) = self.split();
            match mode {
                CorrelationState::RawDatagram(holds) => {
                    DatagramContext { core, holds }.quarantine_displaced_socket_hold(target, socket)
                }
                CorrelationState::RawStream(stream) => {
                    StreamContext { core, stream }.quarantine_displaced_socket_hold(target, socket)
                }
                CorrelationState::Sequenced => {}
            }
        }
        self.assign_socket(target, requested, id)
    }

    fn displace_sony_socket(&mut self, id: RequestId, now: Instant, effects: &mut Decision) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let next = match entry.lifecycle() {
            Lifecycle::Executing {
                started_at,
                deadline,
                attempt,
                ..
            } => Lifecycle::ExecutingDisplaced {
                started_at,
                deadline,
                attempt,
            },
            Lifecycle::AwaitingCancellationResolution {
                deadline, attempt, ..
            } => Lifecycle::ExecutingDisplaced {
                started_at: now,
                deadline,
                attempt,
            },
            _ => return,
        };
        if entry.cancellation_observation_open() {
            effects.push(Effect::CancellationObservation {
                id,
                observation: CancellationObservation::Failed(Error::CancellationUnconfirmed),
            });
        }
        self.transition(id, next, now, effects);
    }

    fn ack(
        &mut self,
        id: RequestId,
        socket: Option<ViscaSocket>,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        let (attempt, cancellation) = match (entry.lifecycle(), entry.previous_written_attempt()) {
            (
                Lifecycle::AwaitingAck {
                    attempt,
                    cancellation,
                    ..
                },
                _,
            ) => (attempt, cancellation),
            (Lifecycle::Ready { .. } | Lifecycle::Backoff { .. }, Some(attempt))
                if !self.mode().is_raw()
                    && entry.reply_family() == SendingReply::AckThenCompletion =>
            {
                (attempt, None)
            }
            _ => {
                effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
                return;
            }
        };
        let target = entry.request.context().target;
        let completion = entry.request.context().timeout.completion.get();
        let deadline = add_duration(now, completion);
        if socket.is_none() && self.uncertain_socketless_stream_ack(target) {
            let (core, mode) = self.split();
            if let CorrelationState::RawStream(stream) = mode {
                StreamContext { core, stream }
                    .hold_uncertain_socketless_execution(target, completion, now, effects);
            }
            self.terminate_unconfirmed_raw(id, UnconfirmedScope::Lifecycle, now, effects);
            return;
        }
        let Some(socket) = self.assign_ack_socket(target, socket, id, now, effects) else {
            if !matches!(self.session, EngineSession::Running) {
                return;
            }
            effects.push(Effect::Ignored(IgnoreReason::SocketConflict));
            if !self.mode().is_raw() {
                // The exact sequence still proves acceptance. A conflicting
                // local socket index cannot make that command safe to replay.
                self.finish_after_ack(
                    id,
                    Error::InvalidState {
                        reason: "accepted command has no uncontested socket".into(),
                    },
                    false,
                    now,
                    effects,
                );
            }
            return;
        };
        let next = Lifecycle::Executing {
            socket,
            started_at: now,
            deadline,
            attempt,
            cancellation: cancellation.map_or(
                ExecutingCancellation::None,
                ExecutingCancellation::Requested,
            ),
        };
        self.transition(id, next, now, effects);
        if let Some(pending) = next.pending_cancellation() {
            self.emit_cancel(id, pending, now, effects);
        }
    }

    fn completion(
        &mut self,
        id: RequestId,
        socket: Option<ViscaSocket>,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        let accept = match entry.lifecycle() {
            Lifecycle::Executing { socket: owned, .. }
            | Lifecycle::AwaitingCancellationResolution { socket: owned, .. } => {
                socket.is_none_or(|socket| socket == owned)
            }
            Lifecycle::AwaitingAck { .. } => !self.mode().is_raw(),
            Lifecycle::Ready { .. } | Lifecycle::Backoff { .. }
                if !self.mode().is_raw()
                    && entry.reply_family() != SendingReply::Inquiry
                    && entry.previous_written_attempt().is_some() =>
            {
                true
            }
            Lifecycle::AwaitingCompletion { .. } | Lifecycle::ExecutingDisplaced { .. } => true,
            Lifecycle::Sending { .. }
            | Lifecycle::Ready { .. }
            | Lifecycle::Backoff { .. }
            | Lifecycle::AwaitingReply { .. } => false,
        };
        if !accept {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        self.quarantine_raw_terminal(id, now);
        self.finish(id, CommandOutcome::Applied, now, effects);
    }

    /// Retains bounded raw-correlation evidence after an uncorrelatable
    /// successful command terminal.
    ///
    /// `NoReply` and `CompletionOnly` earned no response identity, so a later
    /// response-bearing command on that target must wait through the ambiguity
    /// window. Ordinary ACK-then-completion commands deliberately do not add a
    /// terminal hold: the camera may immediately reuse their released socket,
    /// and this protocol surface has no frame identity with which to distinguish
    /// a duplicate from that legitimate next response. On a byte stream a
    /// `CompletionOnly` command's single answer is accounted by the ledger,
    /// so it leaves no hold (#795).
    fn quarantine_raw_terminal(&mut self, id: RequestId, now: Instant) {
        let Some((target, ambiguity)) = self.entries.get(&id).and_then(|entry| {
            let uncorrelatable_command = match entry.reply_family() {
                SendingReply::NoReply => true,
                SendingReply::CompletionOnly => !self.mode().is_stream(),
                SendingReply::Inquiry | SendingReply::AckThenCompletion => false,
            };
            (self.mode().is_raw() && uncorrelatable_command).then_some((
                entry.request.context().target,
                entry.request.context().timeout.ambiguity.get(),
            ))
        }) else {
            return;
        };
        self.split().1.extend_hold(
            target,
            StreamScope::AllResponses,
            add_duration(now, ambiguity),
            Some(id),
        );
    }

    /// Retains the exact declaration's reply skew after an uncertain raw
    /// datagram release. A matched reply exhausts its response immediately;
    /// unrelated declarations never inherit this quarantine.
    fn quarantine_raw_inquiry_correlation(&mut self, id: RequestId, now: Instant) {
        let Some((target, shape)) = self.entries.get(&id).and_then(|entry| {
            let shape = entry.request.payload_shape()?;
            (self.mode().is_raw() && entry.lifecycle().is_live_inquiry())
                .then_some((entry.request.context().target, shape))
        }) else {
            return;
        };
        let until = add_duration(now, self.policy.raw_inquiry_release_hold);
        self.split()
            .1
            .extend_hold(target, StreamScope::Inquiry(shape), until, Some(id));
    }

    // ---------------------------------------------------------------------
    // The raw byte-stream correlation ledger (#795). See `stream_ledger`.
    // ---------------------------------------------------------------------

    /// A written inquiry on a lossless raw stream is never resent for a
    /// missing reply: its first answer remains owed in this shape class.
    fn raw_stream_inquiry(&self, entry: &Entry) -> bool {
        self.mode().is_stream() && entry.request.is_inquiry()
    }

    /// The first answer `entry`'s write will be owed, when it is written on a
    /// raw byte stream. A `NoReply` command is owed nothing, but the camera
    /// may still reject it.
    fn ledger_owes(&self, entry: &Entry) -> Option<Owes> {
        self.mode().stream().and_then(|stream| stream.owes(entry))
    }

    /// Keeps the socket of a raw request that ends while its emitted
    /// cancellation is unanswered held, so its late `0x04`/`0x05` cannot meet
    /// a new owner of that socket, and the command's own late completion or
    /// execution error is not taken for a later command's. On a datagram
    /// transport the hold lasts through the cancellation's ambiguity deadline.
    /// On a byte stream the ledger also owes the cancellation's answer, and
    /// the hold lasts until evidence releases it: that answer, or a frame
    /// naming the socket — all of which precede any rejection of a new
    /// command the camera allocates that socket (#795).
    fn hold_socket_of_unanswered_cancel(&mut self, id: RequestId) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let Some((socket, until)) = entry.lifecycle().unanswered_cancellation() else {
            return;
        };
        let target = entry.request.context().target;
        self.split()
            .1
            .extend_hold(target, StreamScope::Socket(socket), until, Some(id));
    }

    /// Records that a request's (or its cancellation's) write left: from now
    /// on its first answer is owed even if the request ends first.
    fn retire_answers(&mut self, id: RequestId, now: Instant, effects: &mut Decision) {
        let (core, mode) = self.split();
        if let CorrelationState::RawStream(stream) = mode {
            StreamContext { core, stream }.retire_stream_answers(id, now, effects);
        }
    }

    fn mark_stream_answer_written(&mut self, id: RequestId, cancel: bool) {
        if let Some((target, capacity)) = self.entries.get(&id).map(|entry| {
            (
                entry.request.context().target,
                entry.dispatched_socket_capacity(),
            )
        }) {
            if let CorrelationState::RawStream(stream) = self.split().1 {
                if let Some(capacity) = capacity {
                    let seen = &mut stream.learning[target].socket_capacity_seen;
                    *seen = (*seen).max(Some(capacity));
                }
                stream.ledger.mark_written(target, id, cancel);
            }
        }
    }

    /// Retains raw inquiry correlation for an inquiry that ends unanswered
    /// after its write. On a stream its reply is owed through the ledger
    /// (`retire_stream_answers`); elsewhere the bounded reply skew applies.
    fn retain_unanswered_inquiry(&mut self, id: RequestId, now: Instant) {
        if !self.mode().is_stream() {
            self.quarantine_raw_inquiry_correlation(id, now);
        }
    }

    /// Preserve evidence even when its owner expired: retire the lifecycle
    /// first, then resolve the frame against the retained identity or debt.
    /// Equality belongs to input, and unrelated requests never advance here.
    fn retire_stale_correlated_request(
        &mut self,
        id: RequestId,
        now: Instant,
        effects: &mut Decision,
    ) {
        if !self.entries.get(&id).is_some_and(|entry| {
            [
                entry.correlated_response_deadline(),
                entry.retry_budget_deadline(),
            ]
            .into_iter()
            .flatten()
            .min()
            .is_some_and(|deadline| deadline < now)
        }) {
            return;
        }
        while let Some(due) = self
            .entries
            .get(&id)
            .and_then(|entry| entry.due(id))
            .filter(|due| due.at < now)
        {
            self.apply_due(due, now, effects);
        }
    }

    /// Applies the frames a proven dispute assigned to their requests (their
    /// arrival preceded the frame that proved it), to each request that still
    /// awaits it.
    ///
    /// It runs where a decision can occur: after a frame resolves, after due
    /// work (a learned completion, a request ending unwritten), and after an
    /// unwritten halt STOP expires. Applying a frame never decides another
    /// dispute (the dispute it came from is already settled, and a request it
    /// ends was written), so one pass leaves nothing pending.
    fn apply_retro(&mut self, now: Instant, effects: &mut Decision) {
        let deferred = effects.take_deferred();
        for id in deferred.displaced {
            self.terminate_unconfirmed_raw(id, UnconfirmedScope::Displaced, now, effects);
            if !matches!(self.session, EngineSession::Running) {
                return;
            }
        }
        for retro in deferred.retro {
            let awaiting = self
                .entries
                .get(&retro.request)
                .is_some_and(|entry| !entry.lifecycle().is_off_wire());
            if !awaiting {
                continue;
            }
            match retro.evidence {
                Evidence::Ack(socket) => self.ack(retro.request, socket, now, effects),
                Evidence::Error(code) => self.camera_error(
                    retro.request,
                    CorrelationKind::Request,
                    None,
                    code,
                    now,
                    effects,
                ),
                Evidence::Named(NamedFrame::Completion(socket)) => {
                    self.completion(retro.request, Some(socket), now, effects);
                }
                Evidence::Named(NamedFrame::Error(socket, code)) => self.camera_error(
                    retro.request,
                    CorrelationKind::Request,
                    Some(socket),
                    code,
                    now,
                    effects,
                ),
                Evidence::Other => {}
            }
            if !matches!(self.session, EngineSession::Running) {
                return;
            }
        }
    }

    /// The error a latched lane gives `request` instead of writing it, if its
    /// target's lane is latched for that kind of request.
    ///
    /// A latched inquiry lane fails inquiries and `CompletionOnly` commands,
    /// whose unkeyed completion the retained inquiry hold also filters
    /// ([`Error::InquiryCorrelationLost`]). A latched command lane fails
    /// ordinary ACK-bearing and `CompletionOnly` commands
    /// and `NoReply` commands ([`Error::CommandCorrelationLost`]). Urgent
    /// STOPs and other targets are never blocked.
    fn latched_raw_lane_error(&self, request: &RuntimeRequest) -> Option<Error> {
        let context = request.context();
        let target = context.target;
        let family = SendingReply::new(request);
        let completion_only = family == SendingReply::CompletionOnly;
        if request.payload_shape().is_some_and(|shape| {
            self.mode().ledger().inquiry_state(target, shape) == LaneState::Latched
        }) || (completion_only
            && self.mode().ledger().lane_state(target, Lane::Inquiry) == LaneState::Latched)
        {
            return Some(Error::inquiry_correlation_lost(target.camera()));
        }
        // A `NoReply` command is refused too: the ledger owes its possible
        // rejection until write order settles it, and that could not be told
        // from the owed answer (nor, at the debt cap, be owed at all).
        let command_lane = !request.is_inquiry()
            && (completion_only || family == SendingReply::NoReply || !context.is_stop());
        // A `CompletionOnly` command's socketless rejection could not be told
        // from a `NoReply` command's still owed past its window.
        let rejection_latched = completion_only && self.mode().ledger().rejection_latched(target);
        ((command_lane
            && self.mode().ledger().lane_state(target, Lane::Command) == LaneState::Latched)
            || rejection_latched)
            .then(|| Error::command_correlation_lost(target.camera()))
    }

    /// Fails every queued (`Ready`/`Backoff`) request that a latched target
    /// lane blocks ([`Self::latched_raw_lane_error`]). Such a request was
    /// never written.
    fn fail_latched_raw_lanes(&mut self, now: Instant, effects: &mut Decision) {
        let failed: SmallVec<[(RequestId, Error); 4]> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.lifecycle().is_off_wire())
            .filter_map(|(id, entry)| {
                self.latched_raw_lane_error(&entry.request)
                    .map(|error| (*id, error))
            })
            .collect();
        for (id, error) in failed {
            self.finish(id, error, now, effects);
        }
    }

    fn inquiry_reply(
        &mut self,
        id: RequestId,
        reply: InquiryOutcome,
        now: Instant,
        effects: &mut Decision,
    ) {
        let compatible = self.entries.get(&id).is_some_and(|entry| {
            entry.reply_family() == SendingReply::Inquiry && entry.lifecycle().is_awaiting_reply()
        });
        if !compatible {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        self.finish(id, reply, now, effects);
    }

    fn camera_error(
        &mut self,
        id: RequestId,
        correlation_kind: CorrelationKind,
        socket: Option<ViscaSocket>,
        code: u8,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        if let Some(socket) = socket {
            if entry
                .lifecycle()
                .socket()
                .is_some_and(|owned| owned != socket)
            {
                effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
                return;
            }
        }
        // Sony deliberately keeps the request's prior sequence owners through
        // Backoff, Ready, and Sending so late completions can still be routed
        // without guessing.  A camera error is different: applying one is a
        // state mutation (and a retryable error can spend the retry budget),
        // so it must belong to a response-bearing phase for this correlation
        // kind.  In particular, a delayed duplicate from the attempt that
        // caused Backoff must not schedule another retry while the request is
        // waiting or being written again.
        if !camera_error_phase_compatible(entry, correlation_kind) {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        let cancellation_active = entry.lifecycle().cancellation_ambiguity().is_some();
        if code == 0x04
            && (cancellation_active || correlation_kind == CorrelationKind::Cancellation)
        {
            self.confirm_cancelled(id, now, effects);
            return;
        }
        // A rejection proves that a command did not start only before its ACK.
        // Once the camera has acknowledged it, the command may already be
        // moving; writing it again could perform a relative move or preset
        // twice. Any later error therefore ends the request unconfirmed
        // (#795). On the raw envelope a named-socket `0x05` while an emitted
        // cancellation awaits its answer is that cancellation's answer: the
        // camera had no command on the socket to cancel. Raw frames carry no
        // independent cancellation correlation, so a merely recorded,
        // socketless cancel intent instead leaves `0x05` as the original
        // request's retryable rejection below.
        let acknowledged = entry.lifecycle().is_accepted();
        if acknowledged {
            let cancellation_answered =
                self.mode().is_raw() && code == 0x05 && raw_cancellation_no_socket_response(entry);
            self.finish_after_ack(
                id,
                Error::from_code(code),
                cancellation_answered,
                now,
                effects,
            );
            return;
        }
        let retry = (correlation_kind == CorrelationKind::Request
            && self.camera_error_retryable(entry, code))
        .then(|| entry.lifecycle().retry_eligibility(true))
        .flatten();
        let error = Error::from_code(code);
        let evidence = if is_rejection(code) || code == 0x05 {
            AttemptEvidence::Rejected
        } else {
            AttemptEvidence::Lifecycle
        };
        let possibly_applied = entry.certainty(evidence) == Certainty::Unconfirmed;
        // A raw inquiry error can race a late data reply. Retain the bounded
        // inquiry-only skew before a retry or terminal failure exposes another
        // inquiry to that stale frame class (#712).
        self.quarantine_raw_inquiry_correlation(id, now);
        if let Some(retry) = retry {
            if cancellation_active && possibly_applied {
                self.finish_observed(id, Error::CancellationUnconfirmed, evidence, now, effects);
            } else if cancellation_active {
                self.finish_observed(
                    id,
                    CommandOutcome::Cancelled,
                    AttemptEvidence::Rejected,
                    now,
                    effects,
                );
            } else {
                self.schedule_retry(
                    id,
                    retry,
                    now,
                    RetryCause::CameraRejection(error),
                    Backoff::Uncapped,
                    effects,
                );
            }
        } else {
            self.finish_terminal_error(id, error, evidence, now, effects);
        }
    }

    /// Ends an acknowledged request with the camera's error. The request may
    /// have started, so its outcome is [`Error::CommandFailedAfterAck`]; an
    /// open cancellation observation keeps the camera's exact error.
    fn finish_after_ack(
        &mut self,
        id: RequestId,
        error: Error,
        cancellation_answered: bool,
        now: Instant,
        effects: &mut Decision,
    ) {
        if self
            .entries
            .get(&id)
            .is_some_and(Entry::cancellation_observation_open)
        {
            let _ = self.entries.transition(id, EntryChange::CloseObserver);
            effects.push(Effect::CancellationObservation {
                id,
                observation: CancellationObservation::Failed(error.clone()),
            });
        }
        self.finish_with_cancel_hold(
            id,
            Error::command_failed_after_ack(error),
            !cancellation_answered,
            now,
            effects,
        );
    }

    fn camera_error_retryable(&self, entry: &Entry, code: u8) -> bool {
        let retry = entry.request.context().retry;
        match code {
            0x03 | 0x05 => retry.buffer_full,
            0x41 => retry.movement_not_executable,
            0x02 => entry.reply_family() == SendingReply::Inquiry && retry.builtin_inquiry_syntax,
            _ => false,
        }
    }

    fn confirm_cancelled(&mut self, id: RequestId, now: Instant, effects: &mut Decision) {
        let active = self
            .entries
            .get(&id)
            .is_some_and(|entry| entry.lifecycle().cancellation_ambiguity().is_some());
        if !active {
            self.finish(id, Error::CommandCancelled, now, effects);
            return;
        }
        self.finish_with_cancel_hold(id, CommandOutcome::Cancelled, false, now, effects);
    }

    fn cancel(&mut self, id: RequestId, now: Instant, effects: &mut Decision) -> Result<(), Error> {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return Ok(());
        };
        // Inquiry refusal is request-kind authority; a queued inquiry must not
        // take the local command-cancellation path.
        let queued_inquiry =
            entry.reply_family() == SendingReply::Inquiry && entry.lifecycle().is_off_wire();
        let decision = if queued_inquiry {
            CancellationDecision::Refused(Error::InquiryNotCancellable)
        } else {
            entry.lifecycle().cancellation_decision(
                entry.request.context().cancellation == CancellationPolicy::Supported,
                add_duration(now, entry.request.context().timeout.ambiguity.get()),
            )
        };
        match decision {
            CancellationDecision::Local => {
                effects.push(Effect::CancellationRecorded { id });
                if entry.certainty(AttemptEvidence::Unwritten) == Certainty::Unconfirmed {
                    effects.push(Effect::CancellationObservation {
                        id,
                        observation: CancellationObservation::Failed(
                            Error::CancellationUnconfirmed,
                        ),
                    });
                    self.finish(id, Error::CancellationUnconfirmed, now, effects);
                } else {
                    effects.push(Effect::CancellationObservation {
                        id,
                        observation: CancellationObservation::Cancelled,
                    });
                    self.finish(id, CommandOutcome::Cancelled, now, effects);
                }
            }
            CancellationDecision::Refused(error) => {
                effects.push(Effect::CancellationObservation {
                    id,
                    observation: CancellationObservation::Failed(error.clone()),
                });
                return Err(error);
            }
            CancellationDecision::Duplicate => {
                effects.push(Effect::Ignored(IgnoreReason::DuplicateCancellation))
            }
            CancellationDecision::Record(next) => {
                self.transition(id, next, now, effects);
                effects.push(Effect::CancellationRecorded { id });
                effects.push(Effect::CancellationObservation {
                    id,
                    observation: CancellationObservation::Recorded,
                });
                if let Some(pending) = next.pending_cancellation() {
                    self.emit_cancel(id, pending, now, effects);
                }
            }
        }
        Ok(())
    }

    fn emit_cancel(
        &mut self,
        id: RequestId,
        pending: PendingCancellation,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        if self.cancellation_send_at(entry) > now {
            return;
        }
        let ambiguity_deadline = pending.ambiguity_deadline().max(add_duration(
            now,
            entry.request.context().timeout.ambiguity.get(),
        ));
        let target = entry.request.context().target;
        let socket = pending.socket();
        let framing = match self.policy.wire.encoder() {
            ProtocolEncoder::Raw(encoder) => encoder.plan(),
            ProtocolEncoder::Sony(encoder) => encoder.plan(SonyRequestSequence::AllocateFresh),
        };
        let Some(transmission) = self.allocate_transmission_id() else {
            self.finish(id, Error::RuntimeIdentityExhausted, now, effects);
            return;
        };
        self.transition(
            id,
            pending.emitted(transmission, ambiguity_deadline, framing.expectation()),
            now,
            effects,
        );
        if let CorrelationState::RawStream(stream) = self.split().1 {
            stream.ledger.push(target, id, Owes::Cancel(socket));
        }
        effects.push(Effect::Transmit {
            transmission,
            request: id,
            kind: Transmission::Cancel {
                target,
                socket,
                framing,
            },
        });
        self.pacing.record(Lane::Command, now);
    }

    fn schedule_retry(
        &mut self,
        id: RequestId,
        retry: RetryEligibility,
        now: Instant,
        cause: RetryCause,
        backoff: Backoff,
        effects: &mut Decision,
    ) {
        let (error, evidence) = match cause {
            RetryCause::CameraRejection(error) => (error, AttemptEvidence::Rejected),
            RetryCause::Unconfirmed(error) => (error, AttemptEvidence::Written),
        };
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let certainty = entry.certainty(evidence);
        if let Some(axes) = self
            .entries
            .get(&id)
            .and_then(|entry| self.motion_superseded(entry.request.context()))
        {
            self.finish_observed(
                id,
                Error::MotionSuperseded {
                    axes,
                    context: superseded_context(certainty),
                },
                evidence,
                now,
                effects,
            );
            return;
        }
        let Some((cancellation, policy, submitted_at, retry_count)) =
            self.entries.get(&id).map(|entry| {
                (
                    entry.lifecycle().cancellation_ambiguity().is_some(),
                    entry.request.context().retry,
                    entry.submitted_at,
                    entry.retry_count(),
                )
            })
        else {
            return;
        };
        if cancellation {
            self.finish_terminal_error(id, error, evidence, now, effects);
            return;
        }
        // Releasing a raw inquiry for a retry could let a delayed attempt-N
        // reply bind to the requeued attempt or an overlapping successor.
        // The exact declaration's skew hold is the bounded policy; it
        // intentionally cannot make unidentifiable raw traffic safe forever.
        // A stream never reaches this path on a reply timeout: its reply is
        // owed instead (`retire_stream_answers`).
        self.quarantine_raw_inquiry_correlation(id, now);
        let next_retry_count = retry_count.saturating_add(1);
        let elapsed = now.saturating_duration_since(submitted_at);
        let within_duration = policy
            .total_budget
            .is_none_or(|budget| elapsed < budget.get());
        if next_retry_count > policy.max_retries || !within_duration {
            self.finish_terminal_error(id, error, evidence, now, effects);
            return;
        }
        let delay = retry_delay(
            policy,
            next_retry_count,
            backoff,
            self.jitter.fraction(id, next_retry_count),
        );
        let ready_at = add_duration(now, delay);
        if policy
            .total_budget
            .is_some_and(|budget| ready_at >= add_duration(submitted_at, budget.get()))
        {
            self.finish_terminal_error(id, error, evidence, now, effects);
            return;
        }
        self.retire_answers(id, now, effects);
        self.release_attempt_ownership(id);
        let to = Lifecycle::Backoff {
            ready_at,
            previous_attempt: retry.attempt(),
        };
        if let Ok(Some(changed)) = self.entries.transition(
            id,
            EntryChange::Retry {
                to,
                prior: PriorAttempts {
                    count: NonZeroU32::MIN.saturating_add(retry_count),
                    cause: error,
                    possibly_applied: certainty == Certainty::Unconfirmed,
                },
            },
        ) {
            effects.push(Effect::Transition {
                id,
                from: changed.from,
                to: changed.to,
            });
        }
        effects.push(Effect::RetryScheduled {
            id,
            attempt: next_retry_count,
            #[cfg(test)]
            ready_at,
        });
        if self.entries.get(&id).is_some_and(|entry| {
            entry.reply_family() == SendingReply::Inquiry
                && matches!(entry.retry_cause(), Some(Error::SyntaxError))
        }) {
            let cooldown = add_duration(now, self.policy.inquiry_cooldown.max(delay));
            self.pacing.inquiry_cooldown_until = Some(
                self.pacing
                    .inquiry_cooldown_until
                    .map_or(cooldown, |existing| existing.max(cooldown)),
            );
        }
        if ready_at <= now {
            self.promote_retry(id, retry.attempt(), now, effects);
        }
    }

    fn release_attempt_ownership(&mut self, id: RequestId) {
        let _ = self.entries.transition(id, EntryChange::DisplaceSocket);
    }

    fn run_due(&mut self, now: Instant, effects: &mut Decision) {
        self.debug_assert_input_boundary();
        // A frame delivered at the exact deadline is still processed before
        // this function by an input turn, so it is conservatively ignored by
        // the tombstone. Once the turn reaches due work, release every expired
        // fixed slot before dispatching a queued successor.
        let (core, mode) = self.split();
        if let CorrelationState::RawStream(stream) = mode {
            stream.release_gate = None;
            stream.ledger.latch_due(now);
            StreamContext { core, stream }.pay_unlearned_completions(now, effects);
        }
        self.expire_raw_holds(now);
        #[cfg(debug_assertions)]
        let iteration_bound = self.entries.len().saturating_add(1);
        #[cfg(debug_assertions)]
        let mut iterations = 0;
        while let Some(due) = self.next_due().filter(|due| due.at <= now) {
            #[cfg(debug_assertions)]
            {
                iterations += 1;
                debug_assert!(
                    iterations <= iteration_bound,
                    "due work exceeded its entry progress bound"
                );
            }
            self.apply_due(due, now, effects);
        }
        self.fail_latched_raw_lanes(now, effects);
        // Due work (a learned completion, a request ending unwritten) may
        // have decided a dispute.
        self.apply_retro(now, effects);
        if self
            .pacing
            .inquiry_cooldown_until
            .is_some_and(|deadline| deadline <= now)
        {
            self.pacing.inquiry_cooldown_until = None;
        }
    }

    fn next_due(&self) -> Option<DueWork> {
        self.entries
            .iter()
            .filter_map(|(id, entry)| entry.due(*id))
            .min_by_key(|due| due.key())
    }

    /// Whether an unconfirmable raw command must poison the whole session
    /// rather than fail on its own.
    ///
    /// True only in the strict opt-in mode on the raw envelope (issue #671).
    /// Sony correlation never needs the quarantine, so the flag is inert there
    /// even if a caller set it.
    fn raw_unconfirmed_poison(&self) -> bool {
        self.mode().is_raw() && self.policy.strict_unconfirmed_poison
    }

    /// Poisons the whole session for the strict opt-in mode (issue #671).
    ///
    /// A poisoned session is terminal, so the error it hands to every request —
    /// and to any later operation through the owner's boundary verdict — must
    /// answer [`Error::requires_new_session`](crate::Error::requires_new_session)
    /// with `true` so a caller rebuilds it. [`Error::UnsequencedCommandUnconfirmed`]
    /// is the typed cause, while the outer [`Error::StreamPoisoned`] verdict
    /// distinguishes session death from that survivable per-request outcome.
    /// Its recovery guidance covers a raw command whose completion is uncertain.
    fn poison_strict_unconfirmed(&mut self, now: Instant, effects: &mut Decision) {
        self.terminate_session(
            EndStatus::Poisoned,
            Error::stream_poisoned(Error::UnsequencedCommandUnconfirmed),
            now,
            effects,
        );
    }

    /// Retains the attempt's raw response scope without choosing its terminal
    /// error or treating a refused current write as a transport failure.
    fn quarantine_unconfirmed_raw(&mut self, id: RequestId, now: Instant) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let target = entry.request.context().target;
        // A cancelled attempt retains its complete response window and a
        // fresh ambiguity interval, even if due work was delayed. Ordinary
        // retirement (for example, a retry budget) starts its hold at `now`.
        let hold_base = if entry.lifecycle().cancellation_ambiguity().is_some() {
            now.max(entry.lifecycle().response_window_end().unwrap_or(now))
        } else {
            now
        };
        let ambiguity_deadline =
            add_duration(hold_base, entry.request.context().timeout.ambiguity.get());
        let lifecycle = entry.lifecycle();
        match self.split().1 {
            CorrelationState::RawDatagram(holds) => {
                correlation::quarantine_datagram(holds, target, id, lifecycle, ambiguity_deadline)
            }
            CorrelationState::RawStream(stream) => {
                stream.quarantine(target, id, lifecycle, ambiguity_deadline)
            }
            CorrelationState::Sequenced => {}
        }
    }

    /// Terminates one raw command whose ACK or completion can no longer be
    /// confirmed (issue #671).
    ///
    /// Strict opt-in mode poisons the whole session (the pre-fix behavior). The
    /// default fails exactly this request with
    /// [`Error::UnsequencedCommandUnconfirmed`] while holding whatever
    /// correlation it still owns — its command socket, or its place as the sole
    /// unacknowledged raw command on the target — quarantined until the
    /// ambiguity deadline. The terminal outcome is emitted immediately; the
    /// inert keyed hold, rather than a hidden still-live request phase, prevents
    /// late input from binding to a successor. The session and every unrelated
    /// request keep running.
    fn terminate_unconfirmed_raw(
        &mut self,
        id: RequestId,
        scope: UnconfirmedScope,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let target = entry.request.context().target;
        let ambiguity = entry.request.context().timeout.ambiguity.get();
        if self.raw_unconfirmed_poison() {
            self.poison_strict_unconfirmed(now, effects);
            return;
        }
        debug_assert!(!self.raw_unconfirmed_poison());
        let keep_cancel_hold = match scope {
            UnconfirmedScope::Lifecycle => {
                self.quarantine_unconfirmed_raw(id, now);
                true
            }
            UnconfirmedScope::Displaced => {
                // The camera has superseded this socket claim. Datagrams
                // retain only an unkeyed ambiguity hold; a byte stream owes
                // unanswered frames through its ledger and needs no hold.
                debug_assert!(self.mode().is_raw());
                self.release_attempt_ownership(id);
                if let CorrelationState::RawDatagram(holds) = self.split().1 {
                    holds.extend(
                        target,
                        RawHoldScope::PreAck,
                        add_duration(now, ambiguity),
                        Some(id),
                    );
                }
                false
            }
        };
        self.finish_with_cancel_hold(
            id,
            Error::UnsequencedCommandUnconfirmed,
            keep_cancel_hold,
            now,
            effects,
        );
    }

    fn finish_cancellation_unconfirmed(
        &mut self,
        id: RequestId,
        now: Instant,
        effects: &mut Decision,
    ) {
        match self.mode() {
            CorrelationRef::RawDatagram(_) | CorrelationRef::RawStream(_) => {
                self.terminate_unconfirmed_raw(id, UnconfirmedScope::Lifecycle, now, effects)
            }
            CorrelationRef::Sequenced => {
                self.finish(id, Error::CancellationUnconfirmed, now, effects)
            }
        }
    }

    fn close_cancellation_observer(&mut self, id: RequestId, effects: &mut Decision) {
        let _ = self.entries.transition(id, EntryChange::CloseObserver);
        effects.push(Effect::CancellationObservation {
            id,
            observation: CancellationObservation::Failed(Error::timeout(
                FailureStage::CancellationAttempt,
                Certainty::StillLive,
            )),
        });
    }

    fn apply_due(&mut self, due: DueWork, now: Instant, effects: &mut Decision) {
        let id = due.request;
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let context = *entry.request.context();
        let current_certainty = entry.certainty(AttemptEvidence::Lifecycle);
        match due.action {
            DueAction::CancellationObservation { terminal_at } => {
                self.close_cancellation_observer(id, effects);
                if terminal_at <= now {
                    self.finish_cancellation_unconfirmed(id, now, effects);
                }
            }
            DueAction::CancellationResolution {
                observation_deadline,
            } => {
                if observation_deadline.is_some_and(|deadline| deadline <= now) {
                    self.close_cancellation_observer(id, effects);
                }
                self.finish_cancellation_unconfirmed(id, now, effects);
            }
            DueAction::RetryBudget {
                retirement,
                certainty,
            } => {
                let error = entry.terminal_error(
                    entry
                        .retry_cause()
                        .cloned()
                        .unwrap_or_else(|| Error::timeout(FailureStage::Terminal, certainty)),
                    AttemptEvidence::Lifecycle,
                );
                match retirement {
                    BudgetRetirement::Command if self.mode().is_raw() => self
                        .terminate_unconfirmed_raw(id, UnconfirmedScope::Lifecycle, now, effects),
                    BudgetRetirement::Inquiry => {
                        self.retain_unanswered_inquiry(id, now);
                        self.finish(id, error, now, effects);
                    }
                    BudgetRetirement::Queued | BudgetRetirement::Command => {
                        self.finish(id, error, now, effects)
                    }
                }
            }
            DueAction::Ack {
                sent_at,
                deadline,
                budget_at,
                retry,
                cancellation,
            } => {
                let mark = effects.len();
                if cancellation.is_some() {
                    self.finish_cancellation_unconfirmed(id, now, effects);
                } else if self.mode().is_stream()
                    && self.mode().ledger().awaits_dispute(context.target, id)
                    && deadline.saturating_duration_since(sent_at) <= context.timeout.ack.get()
                    && budget_at.is_none_or(|at| at > now)
                {
                    // A tracked first answer may still be proven. A positive
                    // ambiguity interval buys one later deadline; a saturated
                    // clock with no later instant terminalizes instead.
                    let extended = add_duration(now, context.timeout.ambiguity.get());
                    if extended > now {
                        self.transition(
                            id,
                            Lifecycle::AwaitingAck {
                                sent_at,
                                deadline: extended,
                                attempt: retry.attempt(),
                                cancellation: None,
                            },
                            now,
                            effects,
                        );
                    } else {
                        self.terminate_unconfirmed_raw(
                            id,
                            UnconfirmedScope::Lifecycle,
                            now,
                            effects,
                        );
                    }
                } else if self.mode().is_raw() {
                    self.terminate_unconfirmed_raw(id, UnconfirmedScope::Lifecycle, now, effects);
                } else if context.retry.ack_timeout {
                    self.schedule_retry(
                        id,
                        retry,
                        now,
                        RetryCause::Unconfirmed(Error::timeout(
                            FailureStage::Terminal,
                            current_certainty,
                        )),
                        Backoff::AckCapped,
                        effects,
                    );
                } else {
                    self.finish(
                        id,
                        Error::timeout(FailureStage::Terminal, current_certainty),
                        now,
                        effects,
                    );
                }
                record_deadline_expiry(id, DeadlineKind::Ack, mark, effects);
            }
            DueAction::Completion(completion) => {
                let mark = effects.len();
                match completion {
                    CompletionDeadline::Unacknowledged { retry } => {
                        if self.mode().is_raw() {
                            self.terminate_unconfirmed_raw(
                                id,
                                UnconfirmedScope::Lifecycle,
                                now,
                                effects,
                            );
                        } else if context.retry.completion_timeout {
                            self.schedule_retry(
                                id,
                                retry,
                                now,
                                RetryCause::Unconfirmed(Error::timeout(
                                    FailureStage::Terminal,
                                    current_certainty,
                                )),
                                Backoff::Uncapped,
                                effects,
                            );
                        } else {
                            self.finish(
                                id,
                                Error::timeout(FailureStage::Terminal, current_certainty),
                                now,
                                effects,
                            );
                        }
                    }
                    CompletionDeadline::Accepted => {
                        if self.mode().is_raw() {
                            self.terminate_unconfirmed_raw(
                                id,
                                UnconfirmedScope::Lifecycle,
                                now,
                                effects,
                            );
                        } else {
                            // Acceptance permanently forbids replay, including
                            // same-sequence Sony retransmission.
                            self.finish(
                                id,
                                Error::timeout(FailureStage::Terminal, current_certainty),
                                now,
                                effects,
                            );
                        }
                    }
                    CompletionDeadline::Cancelling {
                        socket,
                        original_deadline,
                        attempt,
                        cancellation,
                    } => {
                        if cancellation.ambiguity_deadline() > now {
                            self.transition(
                                id,
                                Lifecycle::AwaitingCancellationResolution {
                                    socket,
                                    deadline: original_deadline,
                                    attempt,
                                    cancellation,
                                },
                                now,
                                effects,
                            );
                        } else {
                            self.finish_cancellation_unconfirmed(id, now, effects);
                        }
                    }
                }
                record_deadline_expiry(id, DeadlineKind::Completion, mark, effects);
            }
            DueAction::InquiryReply { retry } => {
                let mark = effects.len();
                let retry_timeout =
                    context.retry.inquiry_timeout && !self.raw_stream_inquiry(entry);
                let error = Error::timeout(FailureStage::Terminal, current_certainty);
                self.retain_unanswered_inquiry(id, now);
                if retry_timeout {
                    self.schedule_retry(
                        id,
                        retry,
                        now,
                        RetryCause::Unconfirmed(error),
                        Backoff::Uncapped,
                        effects,
                    );
                } else {
                    self.finish(id, error, now, effects);
                }
                record_deadline_expiry(id, DeadlineKind::InquiryReply, mark, effects);
            }
            DueAction::RetryReady {
                previous_attempt,
                budget_at,
                certainty,
            } => {
                if budget_at.is_some_and(|at| at <= now) {
                    let error = entry.terminal_error(
                        entry
                            .retry_cause()
                            .cloned()
                            .unwrap_or_else(|| Error::timeout(FailureStage::Terminal, certainty)),
                        AttemptEvidence::Lifecycle,
                    );
                    self.finish(id, error, now, effects);
                } else {
                    self.promote_retry(id, previous_attempt, now, effects);
                }
            }
        }
        debug_assert!(
            self.entries
                .get(&id)
                .and_then(|entry| entry.due(id))
                .is_none_or(|next| next.at > now),
            "due action left its request due at the sampled instant"
        );
    }

    fn promote_retry(
        &mut self,
        id: RequestId,
        previous_attempt: AttemptFacts,
        now: Instant,
        effects: &mut Decision,
    ) {
        self.transition(
            id,
            Lifecycle::Ready {
                previous_attempt: Some(previous_attempt),
            },
            now,
            effects,
        );
    }

    /// Raw correlation scopes which can be released by advancing at `now`.
    ///
    /// Byte-stream owners use this typed projection before a due pass.  It
    /// keeps an exact socket release distinct from target-only ambiguity, so a
    /// split S1 completion survives an S2 release on the same camera.  The
    /// matching [`Self::raw_prefix_disposition`] is the sole policy authority
    /// for the owner-side retained-prefix decision.
    fn raw_correlation_releases_due(&self, now: Instant) -> RawCorrelationReleaseSet {
        let mut releases = RawCorrelationReleaseSet::default();
        for (key, hold) in self.mode().holds() {
            // An owed reply latches rather than releases at its window end,
            // and a stream's socket hold waits for evidence.
            if hold.until > now || self.raw_hold_released_by_evidence(*key) {
                continue;
            }
            let release = releases.for_target_mut(key.target);
            match key.scope {
                RawHoldScope::Socket(socket) => release.release_exact_socket(socket),
                RawHoldScope::PreAck => release.release_pre_ack_unkeyed(),
                RawHoldScope::Inquiry(_) => release.release_inquiry_unkeyed(),
                RawHoldScope::AllResponses => release.release_terminal_all(),
            }
        }
        for entry in self.entries.values().filter(|_| self.mode().is_raw()) {
            // Ordinary response deadlines can either extend a live cancellation
            // phase or install a new keyed hold in the due pass. Neither case
            // releases correlation before that input-first turn, so project
            // only the deadline which can actually remove this live entry.
            let Some(cancellation) = entry.lifecycle().raw_cancellation_release() else {
                continue;
            };
            if cancellation.at > now {
                continue;
            }
            let release = releases.for_target_mut(entry.request.context().target);
            match cancellation.scope {
                ReleaseScope::ExactSocket(socket) => release.release_exact_socket(socket),
                ReleaseScope::PreAckUnkeyed => release.release_pre_ack_unkeyed(),
            }
        }
        releases
    }

    /// Classifies retained raw stream bytes before the owner advances a due
    /// turn.  This is intentionally conservative: an incomplete source, ACK,
    /// socketless completion, or socketless error cannot establish ownership,
    /// so it never permits release plus successor dispatch.
    fn raw_prefix_disposition(
        &self,
        releases: RawCorrelationReleaseSet,
        evidence: RawPrefixEvidence,
    ) -> RawPrefixDisposition {
        if releases.is_empty() {
            return RawPrefixDisposition::NoRelease;
        }
        if matches!(evidence, RawPrefixEvidence::Malformed) {
            // A source byte the framer already rejected cannot gain a valid
            // owner from a later tail. Drop it immediately, using the same
            // observable malformed-frame path as a complete invalid frame.
            return RawPrefixDisposition::Discard;
        }
        let RawPrefixEvidence::Incomplete { target, kind } = evidence else {
            // Complete frames are always input-first, even exactly at a due
            // boundary. The owner feeds this into `handle` before advancing.
            return RawPrefixDisposition::Defer;
        };
        let release = releases.for_target(target);
        if release.is_empty() {
            // A retained frame for another target cannot be claimed by this
            // target's correlation release.
            return RawPrefixDisposition::ReleasePreserving;
        }
        if release.terminal_all() {
            // The broad NoReply/CompletionOnly hold makes every response shape
            // ambiguous. Retain its established conservative behavior.
            return match kind {
                RawIncompletePrefix::Noncorrelating => RawPrefixDisposition::ReleasePreserving,
                RawIncompletePrefix::SourceOnly
                | RawIncompletePrefix::Ack
                | RawIncompletePrefix::SocketlessCompletion
                | RawIncompletePrefix::SocketlessError
                | RawIncompletePrefix::NamedCompletionOrError(_) => RawPrefixDisposition::Discard,
            };
        }
        match kind {
            RawIncompletePrefix::SourceOnly
            | RawIncompletePrefix::Ack
            | RawIncompletePrefix::SocketlessCompletion
            | RawIncompletePrefix::SocketlessError => RawPrefixDisposition::Defer,
            RawIncompletePrefix::NamedCompletionOrError(socket) => {
                if release.exact_socket(socket) {
                    RawPrefixDisposition::Discard
                } else if self.socket_owner(target, socket).is_some() {
                    // An explicit still-live socket owner is authoritative,
                    // including when a different exact or unkeyed scope is
                    // released on this target.
                    RawPrefixDisposition::ReleasePreserving
                } else {
                    // A pre-ACK/inquiry hold has no named successor that could
                    // safely receive an unowned terminal frame.
                    RawPrefixDisposition::Discard
                }
            }
            RawIncompletePrefix::Noncorrelating => RawPrefixDisposition::ReleasePreserving,
        }
    }

    /// Resolves retained stream evidence against a due raw-correlation release
    /// using one engine-owned time budget (#713).
    ///
    /// An ambiguous prefix keeps the old scope alive until the returned
    /// deadline. Both owner shells map that deadline onto their native wait.
    /// Once the grace expires the orphan is discarded and recorded rather than
    /// turning a fast poll counter into a poisoned session.
    fn resolve_raw_release_gate(
        &mut self,
        now: Instant,
        evidence: Option<RawPrefixEvidence>,
    ) -> RawReleaseGateAction {
        let action = self.resolve_raw_release_gate_inner(now, evidence);
        self.debug_assert_invariants();
        action
    }

    fn resolve_raw_release_gate_inner(
        &mut self,
        now: Instant,
        evidence: Option<RawPrefixEvidence>,
    ) -> RawReleaseGateAction {
        let (core, mode) = self.split();
        match mode {
            CorrelationState::RawStream(stream) => {
                StreamContext { core, stream }.resolve_release_gate(now, evidence)
            }
            CorrelationState::Sequenced | CorrelationState::RawDatagram(_) => {
                RawReleaseGateAction::Advance
            }
        }
    }

    /// Earliest scheduler deadline, retry eligibility, pacing release, or cooldown.
    fn next_wake(&self, now: Instant) -> Option<Instant> {
        if !matches!(self.session, EngineSession::Running) {
            return None;
        }
        let occupancy = Occupancy::build(&self.entries);
        let mut wake = occupancy
            .due
            .map(|due| due.at)
            .into_iter()
            .chain(self.raw_hold_wake(now))
            .chain(self.mode().ledger().wake())
            .min();
        for entry in self.entries.values() {
            let candidate = if entry.lifecycle().is_ready()
                && !self.dispatch_blocked_in(entry, now, &occupancy)
            {
                Some(self.candidate_send_at(entry))
            } else if entry.lifecycle().pending_cancellation_socket().is_some() {
                Some(self.cancellation_send_at(entry))
            } else if entry.lifecycle().is_ready() {
                // A blocked STOP on a byte stream is written at its bound.
                self.stop_wait_bound_in(entry, &occupancy)
                    .map(|bound| bound.max(self.candidate_send_at(entry)))
            } else {
                None
            };
            if let Some(candidate) = candidate {
                wake = Some(wake.map_or(candidate, |current| current.min(candidate)));
            }
        }
        wake
    }

    /// Earliest expiry across the keyed raw quarantine table.
    fn raw_hold_wake(&self, now: Instant) -> Option<Instant> {
        // A hold released by evidence has no deadline of its own: an owed
        // reply's window wakes the owner through the ledger, and once latched
        // only input settles it.
        // A stream's socket hold still wakes the owner at its deadline, when
        // a waiting `CompletionOnly` command may start.
        self.mode()
            .holds()
            .iter()
            .filter(|(key, hold)| {
                !self.raw_hold_released_by_evidence(**key)
                    || (matches!(key.scope, RawHoldScope::Socket(_)) && hold.until > now)
            })
            .map(|(_, hold)| hold.until)
            .min()
    }

    /// Whether `key` never expires by time: an inquiry-shape hold whose
    /// target still owes an inquiry reply, or a byte stream's `Socket` hold,
    /// whose late answer is delayed, never lost (#795).
    fn raw_hold_released_by_evidence(&self, key: RawHoldKey) -> bool {
        self.mode().released_by_evidence(key)
    }

    /// Expiry is mode-specific: stream socket and inquiry debts await evidence.
    fn expire_raw_holds(&mut self, now: Instant) {
        match self.split().1 {
            CorrelationState::Sequenced => {}
            CorrelationState::RawDatagram(holds) => holds.expire(now),
            CorrelationState::RawStream(stream) => stream.holds.expire(now, &stream.ledger),
        }
    }

    fn finish(
        &mut self,
        id: RequestId,
        outcome: impl Into<FinishOutcome>,
        now: Instant,
        effects: &mut Decision,
    ) {
        self.finish_with_cancel_hold(id, outcome, true, now, effects);
    }

    fn finish_with_cancel_hold(
        &mut self,
        id: RequestId,
        outcome: impl Into<FinishOutcome>,
        keep_cancel_hold: bool,
        now: Instant,
        effects: &mut Decision,
    ) {
        self.finish_with_evidence(
            id,
            outcome,
            keep_cancel_hold,
            AttemptEvidence::Lifecycle,
            now,
            effects,
        );
    }

    fn finish_observed(
        &mut self,
        id: RequestId,
        outcome: impl Into<FinishOutcome>,
        evidence: AttemptEvidence,
        now: Instant,
        effects: &mut Decision,
    ) {
        self.finish_with_evidence(id, outcome, true, evidence, now, effects);
    }

    fn finish_terminal_error(
        &mut self,
        id: RequestId,
        cause: Error,
        evidence: AttemptEvidence,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let error = entry.terminal_error(cause, evidence);
        self.finish_observed(id, error, evidence, now, effects);
    }

    fn finish_with_evidence(
        &mut self,
        id: RequestId,
        outcome: impl Into<FinishOutcome>,
        keep_cancel_hold: bool,
        evidence: AttemptEvidence,
        now: Instant,
        effects: &mut Decision,
    ) {
        self.retire_answers(id, now, effects);
        if keep_cancel_hold {
            self.hold_socket_of_unanswered_cancel(id);
        }
        let Some(entry) = self.entries.remove(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        let outcome = outcome.into().for_request(&entry.request);
        let target = entry.request.context().target;
        if let Some(projection) = entry.request.applied_state() {
            let projection = if matches!(outcome, RuntimeOutcome::Command(CommandOutcome::Applied))
            {
                Some(projection)
            } else if entry.may_have_effect(evidence) {
                Some(AppliedStateProjection::invalidate(projection.key()))
            } else {
                None
            };
            if let Some(projection) = projection {
                effects.push(Effect::AppliedState {
                    effect: AppliedStateEffect {
                        request: id,
                        target,
                        projection,
                    },
                });
            }
        }
        if entry.cancellation_observation_open() {
            let observation = match &outcome {
                RuntimeOutcome::Command(CommandOutcome::Applied) => {
                    Some(CancellationObservation::Completed)
                }
                RuntimeOutcome::Command(CommandOutcome::Cancelled) => {
                    Some(CancellationObservation::Cancelled)
                }
                RuntimeOutcome::Command(CommandOutcome::Failed(error))
                | RuntimeOutcome::Inquiry(InquiryOutcome::Failed(error)) => {
                    Some(CancellationObservation::Failed(error.clone()))
                }
                RuntimeOutcome::Command(CommandOutcome::Written)
                | RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }) => None,
            };
            if let Some(observation) = observation {
                effects.push(Effect::CancellationObservation { id, observation });
            }
        }
        effects.push(Effect::Terminal { id, outcome });
    }

    fn terminate_session(
        &mut self,
        status: EndStatus,
        error: Error,
        now: Instant,
        effects: &mut Decision,
    ) {
        match &self.session {
            EngineSession::Ended(_) => return,
            EngineSession::Running => {
                let end = SessionEnd {
                    status,
                    cause: error.clone(),
                };
                self.session = EngineSession::Ended(end.clone());
                effects.push(Effect::SessionEnded(end));
            }
        }
        let active: Vec<_> = self.entries.iter().map(|(id, _)| *id).collect();
        for id in active {
            self.finish(id, error.clone(), now, effects);
        }
        self.targets = PerTarget::from_fn(|target| TargetState {
            policy: self.targets[target].policy,
            ..TargetState::default()
        });
        match self.split().1 {
            CorrelationState::Sequenced => {}
            CorrelationState::RawDatagram(holds) => holds.clear(),
            CorrelationState::RawStream(stream) => {
                stream.holds.clear();
                stream.ledger.clear();
                stream.learning = PerTarget::default();
                stream.release_gate = None;
            }
        }
        effects.clear_deferred();
    }

    /// The declaration cap and overlap exclusion are separate facts: Ready
    /// entries reserve exact declarations, while only staged/written entries
    /// can owe a reply. Pairwise exclusion also proves every inquiry debt is a
    /// single member and charges at most one row per reserved declaration in
    /// the stream ledger's storage bound.
    fn check_inquiry_shape_invariants(&self) -> Result<(), Box<str>> {
        for target in TargetSlot::ALL.into_iter().filter(|_| self.mode().is_raw()) {
            self.tracked_inquiry_shapes(target)?;
            let mut previous = InquiryShapes::default();
            if self.mode().is_stream() {
                for (_, row) in self
                    .mode()
                    .ledger()
                    .entries()
                    .filter(|(owner, _)| *owner == target)
                {
                    let Owes::Reply(shape) = row.owes else {
                        continue;
                    };
                    if row.members != 1 || previous.iter().any(|older| older.overlaps(shape)) {
                        return Err("overlapping staged, live or owed raw stream inquiries".into());
                    }
                    if !previous.try_insert(shape) {
                        return Err("raw stream inquiry rows exceed declaration bound".into());
                    }
                }
            } else {
                for entry in self.entries.values().filter(|entry| {
                    entry.request.context().target == target && entry.lifecycle().is_live_inquiry()
                }) {
                    let Some(shape) = entry.request.payload_shape() else {
                        continue;
                    };
                    if previous.iter().any(|older| older.overlaps(shape)) {
                        return Err("overlapping staged or live raw datagram inquiries".into());
                    }
                    if !previous.try_insert(shape) {
                        return Err("raw datagram inquiries exceed declaration bound".into());
                    }
                }
            }
        }
        Ok(())
    }

    /// Audits cross-domain safety rules that construction alone cannot prove.
    fn assert_invariants(&self) -> Result<(), Box<str>> {
        self.check_inquiry_shape_invariants()?;
        if !matches!(self.session, EngineSession::Running)
            && (!self.entries.is_empty()
                || !self.mode().holds().is_empty()
                || !self.mode().ledger().is_empty())
        {
            return Err("ended session retains lifecycle or correlation work".into());
        }
        let ordinary = self
            .entries
            .values()
            .filter(|entry| entry.slot == AdmissionSlot::Ordinary)
            .count();
        if ordinary > self.policy.capacity {
            return Err("ordinary entry capacity exceeded".into());
        }
        for (index, state) in self.targets.iter() {
            let policy = &state.policy;
            let reserved = self
                .entries
                .values()
                .filter(|entry| {
                    entry.slot == AdmissionSlot::ControlReserve
                        && entry.request.context().target == index
                })
                .count();
            if reserved > policy.map_or(0, |policy| usize::from(policy.control_reserve)) {
                return Err("control reserve exceeded".into());
            }
        }
        if self.entries.values().any(|entry| {
            entry.slot == AdmissionSlot::ControlReserve && !entry.request.context().is_stop()
        }) {
            return Err("a non-urgent entry holds a control-reserve slot".into());
        }
        for entry in self.entries.values() {
            if matches!(entry.lifecycle(), Lifecycle::ExecutingDisplaced { .. })
                && (self.mode().is_raw() || entry.current_sequence().is_none())
            {
                return Err("displaced execution requires a Sony request sequence".into());
            }
            if matches!(
                entry.lifecycle(),
                Lifecycle::Ready { .. } | Lifecycle::Backoff { .. }
            ) && entry.lifecycle().previous_attempt().is_some() != entry.prior.is_some()
            {
                return Err("queued previous attempt disagrees with retry history".into());
            }
        }
        self.mode().ledger().check_disputes()?;
        for (index, state) in self.targets.iter() {
            let policy = &state.policy;
            let Some(policy) = policy else {
                continue;
            };
            let target = index;
            let admissible = self
                .policy
                .capacity
                .saturating_add(usize::from(policy.control_reserve));
            if self.mode().ledger().len(target) > stream_ledger::max_entries_per_target(admissible)
            {
                return Err("stream correlation ledger exceeds its bound".into());
            }
        }
        for (target, outstanding) in self.mode().ledger().entries() {
            if outstanding.standing != Standing::Live {
                continue;
            }
            if outstanding.members != 1 {
                return Err("live stream ledger entry with several members".into());
            }
            let awaiting = self.entries.get(&outstanding.request).is_some_and(|entry| {
                entry.request.context().target == target
                    && match outstanding.owes {
                        Owes::Ack => {
                            entry.lifecycle().is_sending() || entry.lifecycle().is_awaiting_ack()
                        }
                        Owes::Reply(_) => {
                            entry.lifecycle().is_sending() || entry.lifecycle().is_awaiting_reply()
                        }
                        Owes::Rejection => entry.lifecycle().is_sending(),
                        Owes::Completion | Owes::AcceptedCompletion => {
                            entry.lifecycle().is_sending()
                                || entry.lifecycle().is_awaiting_completion()
                        }
                        Owes::Cancel(socket) => {
                            entry.cancel_attempted_socket() == Some(socket)
                                && entry.lifecycle().awaiting_cancellation_answer()
                        }
                    }
            });
            if !awaiting {
                return Err("live stream ledger entry no longer awaits its answer".into());
            }
        }
        if self
            .mode()
            .holds()
            .keys()
            .any(|key| self.targets[key.target].policy.is_none())
        {
            return Err("raw correlation hold has an unregistered target".into());
        }
        // Issue #671 / D8: audit the raw positional-candidate rule that
        // correlation safety depends on. Raw dispatch normally keeps one
        // command per target in the window where a reply is attributed
        // positionally rather than by an owned socket. Issue #714 admits one
        // explicit safety-lane exception: at most two candidates, with at least
        // one intrinsically Urgent. The resolver then binds an ACK/error to
        // neither. Keep this audit on the same predicate as dispatch:
        // completion-only `AwaitingCompletion` is uncorrelated for its entire
        // lifetime, while exact socket holds live in the keyed quarantine table
        // rather than masquerading as active positional candidates. See
        // GitHub issue #542,
        // “Decisions from this review”, item 7, and architecture_2_0.md,
        // “Correlation before ACK is envelope-specific” and “Operational
        // invariants”.
        if self.mode().is_raw() {
            let mut unacknowledged = PerTarget::<u8>::default();
            let mut urgent = PerTarget::<u8>::default();
            let mut uncorrelatable = PerTarget::<u8>::default();
            let mut no_reply = PerTarget::<u8>::default();
            for entry in self.entries.values() {
                if !raw_unacknowledged_command_candidate(entry) {
                    continue;
                }
                let target = entry.request.context().target;
                unacknowledged[target] = unacknowledged[target].saturating_add(1);
                if entry.request.context().is_stop() {
                    urgent[target] = urgent[target].saturating_add(1);
                }
                if entry.reply_family() != SendingReply::AckThenCompletion {
                    uncorrelatable[target] = uncorrelatable[target].saturating_add(1);
                }
                if entry.reply_family() == SendingReply::NoReply {
                    no_reply[target] = no_reply[target].saturating_add(1);
                }
                // On a byte stream the ledger binds every frame by write
                // order, and a STOP that waited out its bound is written
                // whatever else waits (#795): only STOPs add candidates there.
                let excess = if self.mode().is_stream() {
                    unacknowledged[target] > urgent[target].saturating_add(1)
                } else {
                    unacknowledged[target] > 2
                        || (unacknowledged[target] > 1 && urgent[target] == 0)
                };
                if excess {
                    return Err("more than one raw command is unacknowledged on a target".into());
                }
            }
            // On a byte stream urgent STOPs may be written behind a
            // `CompletionOnly` command (one, or more once they waited out
            // their bound); the ledger keeps their answers apart.
            let stream = self.mode().is_stream();
            if TargetSlot::ALL.into_iter().any(|target| {
                let crossing = stream
                    && no_reply[target] == 0
                    && uncorrelatable[target] == 1
                    && unacknowledged[target] >= 2
                    && urgent[target] == unacknowledged[target] - 1;
                uncorrelatable[target] != 0 && unacknowledged[target] != 1 && !crossing
            }) {
                return Err(
                    "uncorrelatable raw command does not own its target exclusively".into(),
                );
            }
        }
        Ok(())
    }

    /// Seeds the deterministic retry jitter for engine tests.
    #[cfg(test)]
    fn seed_jitter(&mut self, seed: u64) {
        self.jitter = Jitter { seed };
    }

    #[cfg(test)]
    fn seed_allocators(&mut self, request: u64, transmission: u64) {
        self.next_request_id = IdAllocator::seeded(request);
        self.next_transmission_id = IdAllocator::seeded(transmission);
    }
}
impl<T: EngineAccess> EngineMethods for T {}

fn add_duration(at: Instant, duration: Duration) -> Instant {
    crate::timeout::saturating_deadline(at, duration)
}

/// A raw command which has no socket identity and therefore occupies the
/// target's positional-correlation gate.
///
/// The raw dispatch gate and its invariant audit intentionally share this
/// predicate, so extending one cannot leave the other with a divergent phase
/// list.
fn raw_unacknowledged_command_candidate(entry: &Entry) -> bool {
    entry.lifecycle().is_unacknowledged_command()
}

fn terminal_timeout(entry: &Entry) -> Error {
    Error::timeout(
        FailureStage::Terminal,
        entry.certainty(AttemptEvidence::Lifecycle),
    )
}

/// Whether a VISCA error code answers a socket cancellation packet (`0x04`
/// cancelled, `0x05` no socket). Named, it never rejects a new command.
const fn is_cancel_reply(code: u8) -> bool {
    matches!(code, 0x04 | 0x05)
}

/// Whether a VISCA error code is an immediate rejection of a request the
/// camera read: message length, syntax, buffer full, or not executable.
const fn is_rejection(code: u8) -> bool {
    matches!(code, 0x01 | 0x02 | 0x03 | 0x41)
}

/// The failure context of an admitted request a halt superseded (D21, #795).
///
/// Supersession happens only while the request is off the wire (queued, in
/// backoff, or between a failed attempt and its retry), so the request has
/// affected the camera only if an earlier written attempt may have.
const fn superseded_context(certainty: Certainty) -> FailureContext {
    FailureContext::new(FailureStage::Terminal, certainty)
}

/// Whether `entry` is an urgent ACK-bearing command (a STOP), which no hold
/// keeps from being written (#795).
fn urgent_stop(entry: &Entry) -> bool {
    entry.reply_family() != SendingReply::Inquiry
        && entry.request.context().is_stop()
        && entry.reply_family() == SendingReply::AckThenCompletion
}

/// Records one expired request deadline ahead of whatever the expiry produced.
///
/// `mark` is the effect-queue length captured immediately before the expiry was
/// handled, so everything from `mark` onwards is this expiry's consequence. The
/// retry decision is read back out of those effects rather than recomputed from
/// the retry policy: a policy that permits retrying this deadline still fails
/// the request when the attempt or duration budget is spent, and only the
/// emitted [`Effect::RetryScheduled`] knows which of the two happened. Reading
/// the decision from the emitted effect also keeps this correct for retry
/// reasons the engine grows later.
///
/// The effect is inserted at `mark` rather than appended so a subscriber reads
/// the cause before its consequences.
fn record_deadline_expiry(
    id: RequestId,
    deadline: DeadlineKind,
    mark: usize,
    effects: &mut Decision,
) {
    let will_retry = effects.as_slice()[mark..].iter().any(
        |effect| matches!(effect, Effect::RetryScheduled { id: retried, .. } if *retried == id),
    );
    effects.insert(
        mark,
        Effect::DeadlineExpired {
            id,
            deadline,
            will_retry,
        },
    );
}

/// Backoff exponent ceiling for a retry triggered by a lost ACK.
///
/// The ACK backoff exponent is capped at 5 (32x the initial delay), while
/// completion, inquiry, protocol-error and transport-fault retries are
/// uncapped.
///
/// The cap bounds a lost-ACK retry at `initial_backoff << 5` regardless
/// of `maximum_backoff`, so a session configured to wait a long time for a
/// command the camera has already accepted does not inherit that same wait for
/// a frame the camera never acknowledged at all.
///
/// It therefore binds only when `maximum_backoff > initial_backoff << 5`, and
/// **no shipped profile reaches that**. Preparation derives `initial_backoff`
/// 50ms and `maximum_backoff` `max(500ms, busy_timeout)`, and the largest
/// `busy_timeout` in the profile registry is 240ms, so `maximum_backoff` is
/// 500ms and already clamps every delay from exponent 4 — one below this cap,
/// which consequently never changes a shipped delay. The cap becomes reachable
/// only through an explicit `OperationalTuning::with_retry_budget` override that
/// widens the ceiling past 32x the initial backoff; it is kept for that case
/// rather than deleted as unreachable.
const ACK_BACKOFF_EXPONENT_CAP: u32 = 5;

/// Which backoff ceiling a retry is subject to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backoff {
    /// A lost ACK: the exponent stops at [`ACK_BACKOFF_EXPONENT_CAP`].
    AckCapped,
    /// Everything else: the exponent runs up to `maximum_backoff`.
    Uncapped,
}

/// Deterministic backoff jitter.
///
/// The unjittered delay is exactly `initial << (attempt - 1)` with no entropy on the path. Jitter
/// exists because the `maximum_backoff` ceiling makes retries *converge*: every command that times
/// out together against one camera saturates the same ceiling and then retries on the same instant,
/// forever, which is precisely the collision a backoff is supposed to break up.
///
/// The spread is a pure function of the seed, the request identity and the
/// attempt number, never of wall-clock time or process entropy. The engine
/// stays what it is designed to be — a total function of its inputs — so a
/// replayed input sequence still produces byte-identical effects, and
/// `assert_invariants` and the ordered `BTreeMap` traversals are untouched. A
/// test pins the exact sequence by construction, and [`seed_jitter`] moves it
/// to prove the spread is really seed-derived.
///
/// [`seed_jitter`]: ProtocolEngine::seed_jitter
#[derive(Debug, Clone, Copy)]
struct Jitter {
    seed: u64,
}

impl Jitter {
    /// Arbitrary odd constant; only its bit spread matters.
    const DEFAULT_SEED: u64 = 0x2545_F491_4F6C_DD1D;

    const fn new() -> Self {
        Self {
            seed: Self::DEFAULT_SEED,
        }
    }

    /// Returns this attempt's spread as a 32-bit fraction of one.
    ///
    /// SplitMix64's finalizer over the seed mixed with the request identity
    /// and the attempt, so two requests retrying from the same instant land on
    /// different instants and one request's successive attempts do not repeat
    /// the same offset.
    const fn fraction(self, id: RequestId, attempt: u32) -> u64 {
        let mut z = self
            .seed
            .wrapping_add(id.get().wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add((attempt as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) >> 32
    }
}

/// Computes one attempt's backoff.
///
/// The ceiling is the exponential `initial << (attempt - 1)`, bounded by the
/// ACK exponent cap where it applies and by `maximum_backoff` always. The wait
/// is then the equal-jitter half-open band `[ceiling / 2, ceiling)`, so no
/// request ever waits longer than the unjittered exponential, the ceiling is
/// honored exactly, and concurrent requests separate.
fn retry_delay(policy: RetryPolicy, attempt: u32, backoff: Backoff, jitter: u64) -> Duration {
    if policy.initial_backoff == Duration::ZERO {
        return Duration::ZERO;
    }
    let cap = match backoff {
        Backoff::AckCapped => ACK_BACKOFF_EXPONENT_CAP,
        Backoff::Uncapped => u32::MAX,
    };
    let exponent = attempt.saturating_sub(1).min(cap).min(31);
    let multiplier = 1_u32 << exponent;
    let ceiling = policy
        .initial_backoff
        .checked_mul(multiplier)
        .unwrap_or(policy.maximum_backoff)
        .min(policy.maximum_backoff);
    let floor = ceiling / 2;
    let spread = ceiling
        .saturating_sub(floor)
        .as_nanos()
        .saturating_mul(u128::from(jitter))
        >> 32;
    floor.saturating_add(Duration::from_nanos(
        u64::try_from(spread).unwrap_or(u64::MAX),
    ))
}

fn unique_requests(
    owners: impl Iterator<Item = CorrelationOwner>,
) -> SmallVec<[CorrelationOwner; 4]> {
    let mut unique = SmallVec::new();
    for owner in owners {
        if !unique.iter().any(|existing: &CorrelationOwner| {
            existing.request == owner.request && existing.kind == owner.kind
        }) {
            unique.push(owner);
        }
    }
    unique
}

fn correlation_phase_compatible(entry: &Entry, kind: CorrelationKind) -> bool {
    match kind {
        // Sony sequences deliberately outlive an attempt across retry. A
        // queued/backoff owner therefore needs evidence of an earlier attempt;
        // the ordinary response phases must also agree with the reply shape.
        CorrelationKind::Request => {
            !entry.lifecycle().is_off_wire() || entry.previous_written_attempt().is_some()
        }
        CorrelationKind::Cancellation => entry.lifecycle().awaiting_cancellation_answer(),
    }
}

/// Whether an attributed camera error may mutate the request at its current
/// phase.
///
/// Sequence correlation intentionally outlives one attempt so a late reply
/// can be identified without falling back to an unrelated request. Errors
/// cannot use that broad routing window: retryable errors mutate retry state,
/// and all errors otherwise produce a terminal result. Keep those mutations
/// limited to phases that are actually waiting for the correlated protocol
/// response. Cancellation responses have their own owner and remain valid only
/// while its transmission/terminal window is open.
fn camera_error_phase_compatible(entry: &Entry, kind: CorrelationKind) -> bool {
    match kind {
        CorrelationKind::Request => matches!(
            entry.lifecycle(),
            Lifecycle::AwaitingReply { .. }
                | Lifecycle::AwaitingAck { .. }
                | Lifecycle::AwaitingCompletion { .. }
                | Lifecycle::Executing { .. }
                | Lifecycle::ExecutingDisplaced { .. }
                | Lifecycle::AwaitingCancellationResolution { .. }
        ),
        CorrelationKind::Cancellation => entry.lifecycle().awaiting_cancellation_answer(),
    }
}

/// Whether a raw request-correlated `0x05` is the failed terminal response to
/// an emitted cancellation packet rather than a rejection of the original
/// command.
///
/// Raw VISCA does not distinguish those messages in its response correlation.
/// A cancellation is only actually in that response window after its write was
/// emitted (`Sending`) or accepted for terminal observation
/// (`AwaitingTerminal`), and both states must retain the socket the packet
/// targeted. `Requested` has only intent, while `ObservationFailed` has already
/// resolved the cancellation write failure, so neither may steal the original
/// command's retryable `0x05` classification.
fn raw_cancellation_no_socket_response(entry: &Entry) -> bool {
    entry.lifecycle().awaiting_cancellation_answer()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod cancellation_regression_tests {
    use std::{sync::Arc, time::Duration};

    use super::*;

    fn camera() -> TargetSlot {
        TargetSlot::ALL[0]
    }

    fn policy(envelope: EnvelopeKind, command_spacing: Duration) -> ProtocolPolicy {
        ProtocolPolicy {
            wire: WireProtocol::new(envelope, crate::transport::AddressingMode::Ip),
            command_spacing,
            inquiry_cooldown: Duration::ZERO,
            ..ProtocolPolicy::test_default()
        }
    }

    fn context() -> RequestContext {
        RequestContext {
            requires_extended_pan_tilt_speed: false,
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: camera(),
            timeout: TimeoutPolicy::new(
                Duration::from_secs(1),
                Duration::from_millis(5),
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_millis(50),
            )
            .expect("valid fixture timing"),
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
        }
    }

    fn retrying_buffer_full() -> RetryPolicy {
        RetryPolicy {
            max_retries: 1,
            initial_backoff: Duration::from_millis(1),
            maximum_backoff: Duration::from_millis(1),
            total_budget: Some(
                NonZeroDuration::new(Duration::from_secs(1)).expect("valid fixture timing"),
            ),
            buffer_full: true,
            ..RetryPolicy::NEVER
        }
    }

    fn command_with(reply_shape: ReplyShape, retry: RetryPolicy) -> RuntimeRequest {
        let mut request_context = context();

        request_context.retry = retry;
        RuntimeRequest::Command {
            reply_shape,
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0xff]).expect("test wire")),
            context: request_context,
            applied_state: None,
        }
    }

    fn command() -> RuntimeRequest {
        command_with(ReplyShape::AckThenCompletion, RetryPolicy::NEVER)
    }

    fn engine(envelope: EnvelopeKind, command_spacing: Duration) -> ProtocolEngine {
        let mut engine =
            ProtocolEngine::new(policy(envelope, command_spacing), Instant::now()).expect("engine");
        engine
            .register_target(camera(), TargetPolicy::test_default())
            .expect("target");
        engine
    }

    fn admitted(effects: &[Effect]) -> RequestId {
        effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Admitted { id, .. } => Some(*id),
                _ => None,
            })
            .expect("admission effect")
    }

    fn request_transmission(effects: &[Effect]) -> TransmissionId {
        effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Transmit {
                    transmission,
                    kind: Transmission::Request { .. },
                    ..
                } => Some(*transmission),
                _ => None,
            })
            .expect("request transmission")
    }

    fn cancel_transmission(effects: &[Effect]) -> TransmissionId {
        effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Transmit {
                    transmission,
                    kind: Transmission::Cancel { .. },
                    ..
                } => Some(*transmission),
                _ => None,
            })
            .expect("cancel transmission")
    }

    fn terminal_unconfirmed(effects: &[Effect], id: RequestId) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::UnsequencedCommandUnconfirmed)),
                } if *terminal == id
            )
        })
    }

    fn start_executing(
        envelope: EnvelopeKind,
        command_spacing: Duration,
        now: Instant,
    ) -> (ProtocolEngine, RequestId) {
        let mut engine = engine(envelope, command_spacing);
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command(),
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        let id = admitted(&admitted_effects);
        let request_transmission = request_transmission(&admitted_effects);
        let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
        engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission: request_transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(sequence)),
            },
            now,
        );
        let ack = DecodedFrame {
            target: camera(),
            sequence: (envelope == EnvelopeKind::Sony).then_some(EnvelopeSequence {
                value: 0x1001,
                width: SequenceWidth::Full32,
            }),
            response: DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        };
        engine.handle(Input::Frame(ack), now);
        (engine, id)
    }

    fn quarantine() -> (ProtocolEngine, RequestId, Instant) {
        let now = Instant::now();
        let (mut engine, id) = start_executing(EnvelopeKind::Raw, Duration::ZERO, now);
        let completion_deadline = now + Duration::from_millis(5);
        let expired = engine.advance(completion_deadline);
        assert!(terminal_unconfirmed(&expired, id));
        assert!(engine.entry(id).is_none());
        let deadline = engine
            .raw_hold(camera(), RawHoldScope::Socket(ViscaSocket::S1))
            .expect("socket quarantine hold")
            .until;
        (engine, id, deadline)
    }

    #[test]
    fn pacing_delayed_cancel_extends_raw_and_sony_quarantine() {
        let start = Instant::now();
        for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
            let (mut engine, id) = start_executing(envelope, Duration::from_millis(40), start);
            // Record cancellation while the command is still executing. The
            // completion deadline then extends the executing response window
            // before pacing permits the cancellation write for both envelopes.
            let cancel_at = start + Duration::from_millis(1);
            let recorded = engine.handle(Input::Cancel { id }, cancel_at);
            assert!(recorded.iter().any(
                |effect| matches!(effect, Effect::CancellationRecorded { id: recorded_id } if *recorded_id == id)
            ));
            assert!(!recorded.iter().any(|effect| {
                matches!(
                    effect,
                    Effect::Transmit {
                        kind: Transmission::Cancel { .. },
                        ..
                    }
                )
            }));

            let send_at = start + Duration::from_millis(40);
            let sent = engine.advance(send_at);
            let cancel_transmission = cancel_transmission(&sent);
            let expected_deadline = send_at + Duration::from_millis(50);
            assert!(matches!(
                engine.entry(id).map(Entry::phase),
                Some(Phase::AwaitingCancellationResolution { deadline, .. })
                    if deadline == expected_deadline
            ));
            assert_eq!(engine.next_wake(start), Some(expected_deadline));

            let old_deadline = cancel_at + Duration::from_millis(50);
            let before_expiry = engine.advance(old_deadline);
            assert!(!terminal_unconfirmed(&before_expiry, id));
            assert!(engine.entry(id).is_some());
            assert!(matches!(
                engine.entry(id).map(Entry::cancellation),
                Some(CancelState::Sending { transmission, .. })
                    if transmission == cancel_transmission
            ));
            engine.assert_invariants().expect("engine invariants");
        }
    }

    #[test]
    fn quarantined_terminal_rejects_cancel_at_and_after_hold_boundary() {
        let (mut equal_engine, equal_id, equal_deadline) = quarantine();
        let equal = equal_engine.handle(Input::Cancel { id: equal_id }, equal_deadline);
        assert!(matches!(
            equal.as_slice(),
            [Effect::Ignored(IgnoreReason::UnknownRequest)]
        ));
        assert!(equal_engine.correlation.holds().is_empty());

        let (mut overdue_engine, overdue_id, overdue_deadline) = quarantine();
        let overdue = overdue_engine.handle(
            Input::Cancel { id: overdue_id },
            overdue_deadline + Duration::from_nanos(1),
        );
        assert!(matches!(
            overdue.as_slice(),
            [Effect::Ignored(IgnoreReason::UnknownRequest)]
        ));
        assert!(overdue_engine.entry(overdue_id).is_none());
        assert!(overdue_engine.correlation.holds().is_empty());
        overdue_engine
            .assert_invariants()
            .expect("engine invariants");
    }

    #[test]
    fn raw_requested_socketless_no_socket_is_cancelled() {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, Duration::ZERO);
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with(ReplyShape::AckThenCompletion, retrying_buffer_full()),
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        let id = admitted(&admitted_effects);
        engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission: request_transmission(&admitted_effects),
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            now,
        );
        assert!(matches!(
            engine.entry(id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ));

        let cancel = engine.handle(Input::Cancel { id }, now);
        assert!(cancel.iter().any(
            |effect| matches!(effect, Effect::CancellationRecorded { id: recorded } if *recorded == id)
        ));
        assert!(!cancel.iter().any(|effect| {
            matches!(
                effect,
                Effect::Transmit {
                    kind: Transmission::Cancel { .. },
                    ..
                }
            )
        }));
        assert!(matches!(
            engine.entry(id).map(Entry::cancellation),
            Some(CancelState::Requested { .. })
        ));

        let effects = engine.handle(
            Input::Frame(DecodedFrame {
                target: camera(),
                sequence: None,
                response: DecodedResponse::Error {
                    socket: None,
                    code: 0x05,
                },
            }),
            now,
        );
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Command(CommandOutcome::Cancelled),
                } if *terminal == id
            )
        }));
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::NoSocket)),
                } if *terminal == id
            ) || matches!(effect, Effect::RetryScheduled { id: scheduled, .. } if *scheduled == id)
        }));
        engine.assert_invariants().expect("engine invariants");
    }

    #[test]
    fn raw_completion_only_refused_cancel_keeps_no_socket_retryable() {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, Duration::ZERO);
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with(ReplyShape::CompletionOnly, retrying_buffer_full()),
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        let id = admitted(&admitted_effects);
        engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission: request_transmission(&admitted_effects),
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            now,
        );
        assert!(matches!(
            engine.entry(id).map(Entry::phase),
            Some(Phase::AwaitingCompletion { .. })
        ));

        let cancel = engine.handle(Input::Cancel { id }, now);
        assert!(!cancel.iter().any(|effect| {
            matches!(
                effect,
                Effect::Transmit {
                    kind: Transmission::Cancel { .. },
                    ..
                }
            )
        }));
        assert!(matches!(
            engine.entry(id).map(Entry::cancellation),
            Some(CancelState::None)
        ));

        let effects = engine.handle(
            Input::Frame(DecodedFrame {
                target: camera(),
                sequence: None,
                response: DecodedResponse::Error {
                    socket: None,
                    code: 0x05,
                },
            }),
            now,
        );
        assert!(cancel.iter().any(|effect| matches!(effect,
            Effect::CancellationObservation { id: observed, observation: CancellationObservation::Failed(Error::CancellationNotSupported) } if *observed == id)));
        assert!(effects.iter().any(|effect| matches!(effect,
            Effect::RetryScheduled { id: scheduled, .. } if *scheduled == id)));
        assert!(!effects.iter().any(|effect| matches!(effect,
            Effect::Terminal { id: terminal, .. } if *terminal == id)));
        engine.assert_invariants().expect("engine invariants");
    }

    #[test]
    fn raw_uncancelled_no_socket_still_retries() {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, Duration::ZERO);
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with(ReplyShape::AckThenCompletion, retrying_buffer_full()),
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        let id = admitted(&admitted_effects);
        engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission: request_transmission(&admitted_effects),
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            now,
        );

        let effects = engine.handle(
            Input::Frame(DecodedFrame {
                target: camera(),
                sequence: None,
                response: DecodedResponse::Error {
                    socket: None,
                    code: 0x05,
                },
            }),
            now,
        );
        assert!(effects.iter().any(
            |effect| matches!(effect, Effect::RetryScheduled { id: scheduled, .. } if *scheduled == id)
        ));
        assert!(!effects.iter().any(
            |effect| matches!(effect, Effect::Terminal { id: terminal, .. } if *terminal == id)
        ));
        assert!(matches!(
            engine.entry(id).map(Entry::phase),
            Some(Phase::Backoff { .. })
        ));
        engine.assert_invariants().expect("engine invariants");
    }

    #[test]
    fn raw_awaiting_terminal_cancel_no_socket_is_a_failed_observation() {
        let now = Instant::now();
        let (mut engine, id) = start_executing(EnvelopeKind::Raw, Duration::ZERO, now);
        let cancel = engine.handle(Input::Cancel { id }, now);
        let cancel_transmission = cancel_transmission(&cancel);
        engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission: cancel_transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            now,
        );
        assert!(matches!(
            (
                engine.entry(id).map(Entry::phase),
                engine.entry(id).map(Entry::cancellation),
            ),
            (
                Some(Phase::AwaitingCancellationResolution {
                    socket: ViscaSocket::S1,
                    ..
                }),
                Some(CancelState::AwaitingTerminal { .. }),
            )
        ));
        let effects = engine.handle(
            Input::Frame(DecodedFrame {
                target: camera(),
                sequence: None,
                response: DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x05,
                },
            }),
            now,
        );
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::CancellationObservation {
                    id: observed,
                    observation: CancellationObservation::Failed(Error::NoSocket),
                } if *observed == id
            )
        }));
        // The acknowledged command's own outcome is unconfirmed (#795); the
        // cancellation observation keeps the camera's exact answer.
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::CommandFailedAfterAck { source, .. })),
                } if *terminal == id && matches!(**source, Error::NoSocket)
            )
        }));
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Command(CommandOutcome::Cancelled),
                } if *terminal == id
            )
        }));
        engine.assert_invariants().expect("engine invariants");
    }
}
