//! Deterministic, synchronous, runtime-neutral VISCA protocol engine.
//!
//! This is the sole authority for admitted request lifecycle, dispatch, timing,
//! retry, correlation, and protocol cancellation. It performs no I/O and knows
//! nothing about channels, executors, facade cameras, or observers.

mod types;

pub(crate) use types::*;

use std::{
    array,
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant},
};

use smallvec::SmallVec;

#[cfg(any(feature = "async", feature = "blocking"))]
use crate::protocol::framer::RawIncompletePrefix;
use crate::{raw::INLINE_BYTES, CameraId, Error, ViscaSocket};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorrelationKind {
    Request,
    Cancellation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CorrelationOwner {
    request: RequestId,
    generation: GenerationTicket,
    kind: CorrelationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceRecord {
    sequence: u32,
    kind: CorrelationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SocketOwner {
    request: RequestId,
    generation: GenerationTicket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TransmissionOwner {
    request: RequestId,
    generation: GenerationTicket,
    attempt: u32,
    kind: CorrelationKind,
    requested_sequence: Option<u32>,
}

/// A camera ACK that reached the engine before the write result for the very
/// frame it answers.
///
/// Issue #297: an owner whose reader and writer are not strictly ordered can
/// deliver the ACK first. The engine latches it on the entry instead of
/// dropping it, and applies it verbatim as soon as the transmission is
/// confirmed, so the race cannot silently reopen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeferredAck {
    socket: Option<ViscaSocket>,
}

/// A camera completion that reached the engine before the write result for the
/// very frame it answers, for a completion-only raw command.
///
/// The mirror of [`DeferredAck`] for [`ReplyShape::CompletionOnly`]: a
/// completion-only command receives no ACK, so its terminal frame is the
/// completion. An owner whose reader is not strictly ordered behind its writer
/// can deliver that completion first; the engine latches it on the entry while
/// the write is still `Sending` and applies it as soon as the send is confirmed,
/// so the race cannot silently drop the completion and strand the command until
/// its completion deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeferredCompletion {
    socket: Option<ViscaSocket>,
}

/// A bounded raw-VISCA correlation hold left behind by a terminal response
/// whose wire frame has no request identity.
///
/// Raw frames carry no request identity. `NoReply`/`CompletionOnly` terminals
/// and inquiry terminals leave different evidence behind: the former can
/// imitate every response shape, while the latter can only imitate a raw
/// inquiry reply or socketless error.  Keep their deadlines independently so
/// one cannot accidentally broaden or shorten the other.
///
/// This is deliberately not an `Entry`: the caller has already received its
/// terminal outcome, and these holds have no observer, retry, or cancellation
/// lifecycle of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawTerminalTombstone {
    /// A terminal from an uncorrelatable command can imitate any raw response.
    terminal_deadline: Option<Instant>,
    /// An inquiry response has no request identity, but cannot imitate a
    /// socket-owned command terminal.
    inquiry_deadline: Option<Instant>,
}

/// Time-bounded retained-input decision for one due raw release (#713).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawReleaseGate {
    releases: RawCorrelationReleaseSet,
    await_until: Instant,
}

impl RawTerminalTombstone {
    const fn terminal(deadline: Instant) -> Self {
        Self {
            terminal_deadline: Some(deadline),
            inquiry_deadline: None,
        }
    }

    const fn inquiry(deadline: Instant) -> Self {
        Self {
            terminal_deadline: None,
            inquiry_deadline: Some(deadline),
        }
    }

    const fn is_active(self) -> bool {
        self.terminal_deadline.is_some() || self.inquiry_deadline.is_some()
    }

    /// A response-bearing successor must wait until every active scope has
    /// expired, not merely the earliest one.
    fn dispatch_deadline(self) -> Option<Instant> {
        self.terminal_deadline
            .into_iter()
            .chain(self.inquiry_deadline)
            .max()
    }

    fn next_deadline(self) -> Option<Instant> {
        self.terminal_deadline
            .into_iter()
            .chain(self.inquiry_deadline)
            .min()
    }

    fn expire_at(&mut self, now: Instant) {
        if self
            .terminal_deadline
            .is_some_and(|deadline| deadline <= now)
        {
            self.terminal_deadline = None;
        }
        if self
            .inquiry_deadline
            .is_some_and(|deadline| deadline <= now)
        {
            self.inquiry_deadline = None;
        }
    }
}

/// The one authoritative lifecycle record for an admitted request.
#[derive(Debug)]
pub(crate) struct Entry {
    request: RuntimeRequest,
    phase: Phase,
    cancellation: CancelState,
    admission_order: u64,
    submitted_at: Instant,
    attempt: u32,
    last_error: Option<Error>,
    generation: GenerationTicket,
    queue_generation: u64,
    transmission_order: Option<u64>,
    /// The most recently successful Sony sequence for this logical request.
    /// Retries reuse it; cancellation has its own sequence and never changes
    /// this value.
    current_sequence: Option<u32>,
    sequence_history: SmallVec<[SequenceRecord; MAX_SEQUENCE_HISTORY]>,
    /// Command-socket capacity in force when the current command attempt was
    /// dispatched. It lets that already-dispatched attempt finish after a live
    /// capacity reduction without relaxing the new limit for later work.
    dispatched_socket_capacity: Option<u8>,
    cancel_attempted_socket: Option<ViscaSocket>,
    cancellation_observation_open: bool,
    deferred_ack: Option<DeferredAck>,
    deferred_completion: Option<DeferredCompletion>,
}

impl Entry {
    // Read by `runtime::engine::tests` and by `OwnerState::request_state`, which
    // is itself `#[cfg(test)]`; nothing in a non-test build projects a phase out
    // of the engine yet (#636).
    #[allow(dead_code)]
    pub(crate) const fn phase(&self) -> Phase {
        self.phase
    }

    // Same test-only projection as `phase` (#636).
    #[allow(dead_code)]
    pub(crate) const fn cancellation(&self) -> CancelState {
        self.cancellation
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lane {
    Command,
    Inquiry,
}

#[derive(Debug, Clone, Copy)]
struct DispatchSelection {
    lane: Lane,
    priority: usize,
    index: usize,
    ticket: QueueTicket,
}

#[derive(Debug, Clone, Copy)]
struct DueWork {
    at: Instant,
    admission_order: u64,
    kind_order: u8,
    request: RequestId,
    generation: GenerationTicket,
    queue_generation: u64,
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

/// Why an otherwise-ready first dispatch needs an owner-side wait.
///
/// A raw correlation hold needs an ordered input turn at its boundary so a
/// buffered stale frame is made inert before the hold releases. Ordinary
/// pacing has no such input authority: consuming a peer frame while waiting
/// for it would violate the blocking first-write admission boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FirstDispatchWait {
    RawCorrelationTombstone,
    Pacing,
}

/// Result of attempting one exact first dispatch without running due work.
#[derive(Debug)]
pub(crate) enum FirstDispatch {
    // Both payloads are read by the blocking owner's caller-thread submission
    // (`runtime::owner::blocking`) and by the engine tests; the async owner never
    // takes this seam, so an async-only leg compiles neither reader (#636).
    #[allow(dead_code)]
    Effects(Vec<Effect>),
    #[allow(dead_code)]
    WaitUntil {
        deadline: Instant,
        reason: FirstDispatchWait,
    },
    /// The request stays queued: it is admitted and ready, but some other
    /// request currently owns the capacity it needs. It is never terminal.
    Blocked,
    Missing,
}

impl DueWork {
    fn key(self) -> (Instant, u64, u8) {
        (self.at, self.admission_order, self.kind_order)
    }
}

/// Pure state machine implementing the issue #542 protocol boundary.
#[derive(Debug)]
pub(crate) struct ProtocolEngine {
    policy: ProtocolPolicy,
    targets: [Option<TargetPolicy>; 9],
    entries: BTreeMap<RequestId, Entry>,
    command_queues: [VecDeque<QueueTicket>; 4],
    inquiry_queues: [VecDeque<QueueTicket>; 4],
    transmissions: BTreeMap<TransmissionId, TransmissionOwner>,
    sequences: BTreeMap<u32, SmallVec<[CorrelationOwner; 2]>>,
    lower_sequences: BTreeMap<u16, SmallVec<[CorrelationOwner; 2]>>,
    socket_owners: [[Option<SocketOwner>; 2]; 9],
    raw_inquiries: [VecDeque<CorrelationOwner>; 9],
    /// A terminal raw response correlation can have emitted a delayed frame.
    /// Hold later response-bearing work on that target until its ambiguity
    /// deadline.
    raw_target_tombstones: [Option<RawTerminalTombstone>; 9],
    raw_release_gate: Option<RawReleaseGate>,
    next_request_id: IdAllocator,
    next_transmission_id: IdAllocator,
    next_generation: IdAllocator,
    next_admission_order: u64,
    next_transmission_order: u64,
    jitter: Jitter,
    last_request_sent: Option<Instant>,
    last_inquiry_sent: Option<Instant>,
    inquiry_cooldown_until: Option<Instant>,
    state: SessionState,
    terminal_error: Option<Error>,
}

impl ProtocolEngine {
    /// Constructs an empty session engine. Targets are registered separately.
    pub(crate) fn new(policy: ProtocolPolicy) -> Result<Self, Error> {
        if policy.capacity == 0 || policy.inquiry_capacity == 0 {
            return Err(Error::InvalidRequest(
                "engine and inquiry capacity must be non-zero".into(),
            ));
        }
        Ok(Self {
            policy,
            targets: [None; 9],
            entries: BTreeMap::new(),
            command_queues: array::from_fn(|_| VecDeque::new()),
            inquiry_queues: array::from_fn(|_| VecDeque::new()),
            transmissions: BTreeMap::new(),
            sequences: BTreeMap::new(),
            lower_sequences: BTreeMap::new(),
            socket_owners: [[None; 2]; 9],
            raw_inquiries: array::from_fn(|_| VecDeque::new()),
            raw_target_tombstones: [None; 9],
            raw_release_gate: None,
            next_request_id: IdAllocator::new(),
            next_transmission_id: IdAllocator::new(),
            next_generation: IdAllocator::new(),
            next_admission_order: 0,
            next_transmission_order: 0,
            jitter: Jitter::new(),
            last_request_sent: None,
            last_inquiry_sent: None,
            inquiry_cooldown_until: None,
            state: SessionState::Running,
            terminal_error: None,
        })
    }

    /// Registers one target's socket and cancellation policy.
    pub(crate) fn register_target(
        &mut self,
        target: CameraId,
        policy: TargetPolicy,
    ) -> Result<(), Error> {
        if !(1..=7).contains(&target.id()) {
            return Err(Error::InvalidRequest(
                "engine target must be an individual camera (1 through 7)".into(),
            ));
        }
        if !(1..=2).contains(&policy.command_sockets) {
            return Err(Error::InvalidRequest(
                "VISCA command socket capacity must be one or two".into(),
            ));
        }
        let slot = &mut self.targets[target.id() as usize];
        match *slot {
            Some(existing) if existing != policy => Err(Error::InvalidState(
                "target already has conflicting protocol policy".into(),
            )),
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
    pub(crate) fn retune(
        &mut self,
        command_spacing: Duration,
        inquiry_spacing: Duration,
        command_sockets: [Option<u8>; 9],
    ) -> Result<(), Error> {
        for (slot, sockets) in self.targets.iter().zip(command_sockets.iter()) {
            if let (Some(_), Some(sockets)) = (slot, sockets) {
                if !(1..=2).contains(sockets) {
                    return Err(Error::InvalidRequest(
                        "VISCA command socket capacity must be one or two".into(),
                    ));
                }
            }
        }
        self.policy.command_spacing = command_spacing;
        self.policy.inquiry_spacing = inquiry_spacing;
        for (slot, sockets) in self.targets.iter_mut().zip(command_sockets.iter()) {
            if let (Some(policy), Some(sockets)) = (slot.as_mut(), sockets) {
                policy.command_sockets = *sockets;
            }
        }
        Ok(())
    }

    pub(crate) const fn state(&self) -> SessionState {
        self.state
    }

    /// The exact error the engine recorded when it left `Running`, if it has.
    ///
    /// Every terminal transition — an owner-supplied `Close`/`Poison`/`Shutdown`
    /// and every engine-initiated verdict alike (deadline expiry, the strict
    /// `strict_unconfirmed_poison` opt-in, a stream/framing self-poison) — is
    /// funneled through [`Self::terminate_session`], which stamps this field
    /// before emitting [`Effect::SessionChanged`]. The effect itself carries no
    /// payload, so the owner reads this to learn the true terminal cause and
    /// latch it into its own `session_error` (issue #680); without it an
    /// engine-initiated poison is invisible to `shutdown()`/`close()` and the
    /// documented recovery loop re-latches `RuntimeShutdown` instead.
    pub(crate) fn terminal_error(&self) -> Option<Error> {
        self.terminal_error.clone()
    }

    // Read-only inspection seams. `entry` and `active_len` are driven by
    // `runtime::engine::tests` and by `OwnerState`'s own test-gated projections;
    // `queued_dispatch_at` is projected by `OwnerState::dispatch_at`, which the
    // blocking submission path will consume once it distinguishes pacing from
    // socket backpressure (#636).
    #[allow(dead_code)]
    pub(crate) fn entry(&self, id: RequestId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    #[allow(dead_code)] // See `entry` (#636).
    pub(crate) fn active_len(&self) -> usize {
        self.entries.len()
    }

    /// Earliest time a specific ready request may dispatch without waiting for
    /// another request to release protocol capacity. Blocking submission uses
    /// this to distinguish pacing from socket/inquiry backpressure.
    #[allow(dead_code)] // See `entry` (#636).
    pub(crate) fn queued_dispatch_at(&self, id: RequestId) -> Option<Instant> {
        let entry = self.entries.get(&id)?;
        if !matches!(entry.phase, Phase::Ready { .. }) {
            return None;
        }
        if entry.request.is_inquiry() {
            let target = entry.request.context().target;
            if self.raw_tombstone_blocks_dispatch(entry, target)
                || self.inquiries_inflight() >= self.policy.inquiry_capacity
                || self.uncorrelated_raw_shape_blocked(entry, target)
            {
                return None;
            }
        } else {
            let target = entry.request.context().target;
            let policy = self.targets[target.id() as usize]?;
            if self.raw_tombstone_blocks_dispatch(entry, target)
                || self.raw_command_unacknowledged(target)
                || self.commands_inflight(target) >= usize::from(policy.command_sockets)
                || self.uncorrelated_raw_shape_blocked(entry, target)
            {
                return None;
            }
        }
        Some(self.candidate_send_at(entry))
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

    /// Applies one ordered external input first, then all work due at `now`.
    pub(crate) fn handle(&mut self, input: Input, now: Instant) -> Vec<Effect> {
        let turn = self.begin_input_turn(now);
        let mut effects = self.handle_in_turn(&turn, input);
        effects.extend(self.finish_input_turn(turn));
        effects
    }

    /// Applies one ordered external input and its due/cancellation consequences
    /// without ordinary dispatch. The blocking owner uses this for raw
    /// submission-side input waits (the pre-ACK gate and correlation
    /// tombstones): the frame must be applied, but a queued ordinary request
    /// must not consume a newly free socket before the exact submitting
    /// request is reconsidered (issue #673).
    #[cfg(feature = "blocking")]
    pub(crate) fn handle_without_dispatch(&mut self, input: Input, now: Instant) -> Vec<Effect> {
        let turn = self.begin_input_turn(now);
        let mut effects = self.handle_in_turn(&turn, input);
        effects.extend(self.finish_input_turn_without_dispatch(turn));
        effects
    }

    /// Starts an ordered external-input turn at one owner-sampled instant.
    ///
    /// The owner must fully drain the effects returned for one input before it
    /// applies the next input in the turn. Any identified transmission result
    /// produced by that draining is applied with [`Self::handle_in_turn`] and
    /// the same token. Once every frame in wire order is applied, the owner
    /// calls [`Self::finish_input_turn`] (or its dispatch-suppressed variant)
    /// exactly once.
    pub(crate) const fn begin_input_turn(&self, now: Instant) -> InputTurn {
        InputTurn { now }
    }

    /// Applies one input without running due deadlines or ordinary dispatch.
    ///
    /// This is the recursive input seam for an active [`InputTurn`]. Effects
    /// remain in the exact order produced by this input; the owner drains them
    /// before passing the next wire frame.
    pub(crate) fn handle_in_turn(&mut self, turn: &InputTurn, input: Input) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.apply_input(input, turn.now, &mut effects);
        self.debug_assert_invariants();
        effects
    }

    /// Ends an ordered input turn, then runs due work, pending cancellation,
    /// and one ordinary dispatch.
    ///
    /// External inputs at the turn timestamp therefore win over deadlines at
    /// that same timestamp, while frame and recursively produced effect order
    /// remains owner-controlled and deterministic.
    pub(crate) fn finish_input_turn(&mut self, turn: InputTurn) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.run_due(turn.now, &mut effects);
        self.drain_pending_cancellations(turn.now, &mut effects);
        self.dispatch_one(turn.now, &mut effects);
        self.debug_assert_invariants();
        effects
    }

    /// Ends an ordered input turn after running due work and pending
    /// cancellations, but leaves ordinary ready work queued. This covers the
    /// blocking pre-ACK and raw-correlation submission seams; the exact
    /// submitting request must be reconsidered before a freed socket is
    /// offered to the ordinary scheduler.
    #[cfg(feature = "blocking")]
    pub(crate) fn finish_input_turn_without_dispatch(&mut self, turn: InputTurn) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.run_due(turn.now, &mut effects);
        self.drain_pending_cancellations(turn.now, &mut effects);
        self.debug_assert_invariants();
        effects
    }

    /// Ends an ordered external-input turn without running due work, pending
    /// cancellation, or dispatch. The blocking raw-tombstone wait uses this
    /// only while it still has to inspect retained stream framing; it follows
    /// with one ordinary dispatch-suppressed due pass after that framing has
    /// completed or been discarded.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn finish_input_turn_without_due(&mut self, _turn: InputTurn) -> Vec<Effect> {
        self.debug_assert_invariants();
        Vec::new()
    }

    fn apply_input(&mut self, input: Input, now: Instant, effects: &mut Vec<Effect>) {
        match input {
            Input::Admit { ticket, request } => self.admit(ticket, request, now, effects),
            Input::TransmissionFinished {
                transmission,
                result,
            } => self.transmission_finished(transmission, result, now, effects),
            Input::Frame(frame) => self.frame(frame, now, effects),
            Input::Cancel { id } => self.cancel(id, now, effects),
            Input::ReceiveFault { error } => self.receive_fault(&error, now, effects),
            Input::Close { reason } => self.terminate_session(
                SessionState::Closed,
                Error::ConnectionClosed {
                    reason: reason.map(|value| Cow::Owned(value.into())),
                },
                effects,
            ),
            Input::Poison { reason } => self.terminate_session(
                SessionState::Poisoned,
                Error::StreamPoisoned {
                    reason: Cow::Owned(reason.into()),
                },
                effects,
            ),
            Input::Shutdown(reason) => match reason {
                ShutdownReason::Explicit => {
                    self.terminate_session(SessionState::Shutdown, Error::RuntimeShutdown, effects)
                }
                ShutdownReason::TransportClosed { reason } => self.terminate_session(
                    SessionState::Closed,
                    Error::ConnectionClosed {
                        reason: reason.map(|value| Cow::Owned(value.into())),
                    },
                    effects,
                ),
                ShutdownReason::FramingFailure { reason } => self.terminate_session(
                    SessionState::Poisoned,
                    Error::StreamPoisoned {
                        reason: Cow::Owned(reason.into()),
                    },
                    effects,
                ),
            },
            Input::Wake => {}
        }
    }

    /// Runs due internal work, pending cancellation, and ordinary dispatch.
    pub(crate) fn advance(&mut self, now: Instant) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.run_due(now, &mut effects);
        self.drain_pending_cancellations(now, &mut effects);
        self.dispatch_one(now, &mut effects);
        self.debug_assert_invariants();
        effects
    }

    /// Runs due work and pending cancellations without ordinary dispatch.
    ///
    /// Used by blocking submission-side waits: the raw pre-ACK drain and raw
    /// correlation tombstone hold need their ordered input turns, while an
    /// ordinary pacing wait must service an earlier deadline without consuming
    /// a peer reply. In all cases, only the exact submitting request may be
    /// reconsidered after this seam returns.
    #[cfg(feature = "blocking")]
    pub(crate) fn advance_without_dispatch(&mut self, now: Instant) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.run_due(now, &mut effects);
        self.drain_pending_cancellations(now, &mut effects);
        self.debug_assert_invariants();
        effects
    }

    /// Admits one request without running due work or ordinary dispatch.
    // The three `*_without_due` seams belong to the blocking owner's caller-thread
    // pump (`runtime::owner::blocking`) and to the engine tests; an async-only leg
    // compiles neither (#636).
    #[allow(dead_code)]
    pub(crate) fn admit_without_due(
        &mut self,
        ticket: AdmissionTicket,
        request: RuntimeRequest,
        now: Instant,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.admit(ticket, request, now, &mut effects);
        self.debug_assert_invariants();
        effects
    }

    /// Applies one identified write result without due work or dispatch.
    #[allow(dead_code)] // See `admit_without_due` (#636).
    pub(crate) fn finish_write_without_due(
        &mut self,
        transmission: TransmissionId,
        result: Result<TransmissionMeta, Error>,
        now: Instant,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.transmission_finished(transmission, result, now, &mut effects);
        self.debug_assert_invariants();
        effects
    }

    /// Dispatches `id` only when it is the normative global scheduler winner.
    /// No queue, peer request, deadline, or pacing state is mutated when a
    /// different request would win: [`FirstDispatch::Blocked`] leaves `id`
    /// queued so an ordinary later turn can dispatch it once capacity frees.
    #[allow(dead_code)] // See `admit_without_due` (#636).
    pub(crate) fn first_dispatch_without_due(
        &mut self,
        id: RequestId,
        now: Instant,
    ) -> FirstDispatch {
        let dispatch = self.first_dispatch_without_due_inner(id, now);
        self.debug_assert_invariants();
        dispatch
    }

    /// Rejects an admitted request that has not had its first write yet.
    ///
    /// This is the blocking operation admission boundary: a caller may ask
    /// for a lifecycle handle only after this request's initial write has
    /// succeeded.  The helper deliberately goes through the normal terminal
    /// transition so queue tickets, correlations, and any engine-owned
    /// admission state are cleaned up by one authority.  It does not run due
    /// work or dispatch another request.
    #[allow(dead_code)] // Consumed by the blocking owner (#542).
    pub(crate) fn reject_unwritten_without_due(
        &mut self,
        id: RequestId,
        error: Error,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        match self.entries.get(&id) {
            Some(entry) if matches!(entry.phase, Phase::Ready { .. }) => {
                self.finish(id, RuntimeOutcome::Failed(error), &mut effects);
            }
            Some(_) => effects.push(Effect::Ignored(
                IgnoreReason::IncompatibleTransmissionResult,
            )),
            None => effects.push(Effect::Ignored(IgnoreReason::UnknownRequest)),
        }
        self.debug_assert_invariants();
        effects
    }

    #[allow(dead_code)] // See `admit_without_due` (#636).
    fn first_dispatch_without_due_inner(&mut self, id: RequestId, now: Instant) -> FirstDispatch {
        if self.state != SessionState::Running {
            return FirstDispatch::Missing;
        }
        // Admission has already been applied at this sampled instant. This
        // exact-dispatch seam deliberately does *not* run any due work: a raw
        // target tombstone can only be released by the ordered owner turn
        // after that turn has given already-sampled frames at its boundary a
        // chance to be attributed. In particular, locally expiring it here
        // would let a same-target successor write before a stale reply at the
        // exact hold deadline is made inert, and would bypass an earlier total
        // retry budget on the ready successor.
        let Some(entry) = self.entries.get(&id) else {
            return FirstDispatch::Missing;
        };
        if !matches!(entry.phase, Phase::Ready { .. }) {
            return FirstDispatch::Missing;
        }
        if self.has_pending_cancellation() {
            return FirstDispatch::Blocked;
        }
        if let Some(deadline) = self.raw_tombstone_dispatch_deadline(entry) {
            return FirstDispatch::WaitUntil {
                deadline,
                reason: FirstDispatchWait::RawCorrelationTombstone,
            };
        }
        if !self.capacity_available_for(entry) {
            return FirstDispatch::Blocked;
        }
        let ready_at = self.candidate_send_at(entry);
        match self.select_dispatch(now) {
            Some(selected) if selected.ticket.request != id => FirstDispatch::Blocked,
            Some(selected) => {
                let mut effects = Vec::new();
                self.dispatch_selected(selected, now, &mut effects);
                FirstDispatch::Effects(effects)
            }
            None if ready_at > now => FirstDispatch::WaitUntil {
                deadline: ready_at,
                reason: FirstDispatchWait::Pacing,
            },
            None => FirstDispatch::Blocked,
        }
    }

    fn admit(
        &mut self,
        ticket: AdmissionTicket,
        request: RuntimeRequest,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        if self.state != SessionState::Running {
            effects.push(Effect::AdmissionRejected {
                ticket,
                error: self
                    .terminal_error
                    .clone()
                    .unwrap_or(Error::RuntimeShutdown),
            });
            return;
        }
        if self.entries.len() >= self.policy.capacity {
            effects.push(Effect::AdmissionRejected {
                ticket,
                error: Error::RuntimeQueueFull {
                    capacity: self.policy.capacity,
                },
            });
            return;
        }
        let context = *request.context();
        let Some(target_policy) = self.targets[context.target.id() as usize] else {
            effects.push(Effect::AdmissionRejected {
                ticket,
                error: Error::InvalidState("target is not registered in this session".into()),
            });
            return;
        };
        if target_policy.cancellation != context.cancellation {
            effects.push(Effect::AdmissionRejected {
                ticket,
                error: Error::InvalidState(
                    "request cancellation policy conflicts with target registration".into(),
                ),
            });
            return;
        }
        let Some(id) = self.allocate_request_id() else {
            effects.push(Effect::AdmissionRejected {
                ticket,
                error: Error::RuntimeIdentityExhausted,
            });
            return;
        };
        let Some(generation) = self.allocate_generation() else {
            effects.push(Effect::AdmissionRejected {
                ticket,
                error: Error::RuntimeIdentityExhausted,
            });
            return;
        };
        let admission_order = self.next_admission_order;
        self.next_admission_order = self.next_admission_order.wrapping_add(1);
        let queue_generation = 1;
        let queue_ticket = QueueTicket {
            request: id,
            generation,
            queue_generation,
        };
        let priority = request.context().control.class.priority_index();
        let inquiry = request.is_inquiry();
        self.entries.insert(
            id,
            Entry {
                request,
                phase: Phase::Ready {
                    ticket: queue_ticket,
                },
                cancellation: CancelState::None,
                admission_order,
                submitted_at: now,
                attempt: 0,
                last_error: None,
                generation,
                queue_generation,
                transmission_order: None,
                current_sequence: None,
                sequence_history: SmallVec::new(),
                dispatched_socket_capacity: None,
                cancel_attempted_socket: None,
                cancellation_observation_open: false,
                deferred_ack: None,
                deferred_completion: None,
            },
        );
        self.queue_mut(inquiry, priority).push_back(queue_ticket);
        effects.push(Effect::Admitted { ticket, id });
    }

    fn allocate_request_id(&mut self) -> Option<RequestId> {
        for _ in 0..=self.entries.len() {
            let candidate = RequestId::from_nonzero(self.next_request_id.candidate()?);
            if !self.entries.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn allocate_transmission_id(&mut self) -> Option<TransmissionId> {
        for _ in 0..=self.transmissions.len() {
            let candidate = TransmissionId::from_nonzero(self.next_transmission_id.candidate()?);
            if !self.transmissions.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn allocate_generation(&mut self) -> Option<GenerationTicket> {
        for _ in 0..=self.entries.len() {
            let candidate = GenerationTicket(self.next_generation.candidate()?.get());
            if self
                .entries
                .values()
                .all(|entry| entry.generation != candidate)
            {
                return Some(candidate);
            }
        }
        None
    }

    fn queue_mut(&mut self, inquiry: bool, priority: usize) -> &mut VecDeque<QueueTicket> {
        if inquiry {
            &mut self.inquiry_queues[priority]
        } else {
            &mut self.command_queues[priority]
        }
    }

    fn prune_queue(&mut self, lane: Lane, priority: usize) {
        let entries = &self.entries;
        let queue = match lane {
            Lane::Command => &mut self.command_queues[priority],
            Lane::Inquiry => &mut self.inquiry_queues[priority],
        };
        queue.retain(|ticket| {
            entries.get(&ticket.request).is_some_and(|entry| {
                entry.generation == ticket.generation
                    && entry.queue_generation == ticket.queue_generation
                    && matches!(entry.phase, Phase::Ready { ticket: active } if active == *ticket)
                    && (entry.request.is_inquiry() == (lane == Lane::Inquiry))
                    && entry.request.context().control.class.priority_index() == priority
            })
        });
    }

    fn eligible_ticket_readonly(
        &self,
        lane: Lane,
        priority: usize,
        now: Instant,
    ) -> Option<(usize, QueueTicket)> {
        let queue = match lane {
            Lane::Command => &self.command_queues[priority],
            Lane::Inquiry => &self.inquiry_queues[priority],
        };
        queue.iter().copied().enumerate().find(|(_, ticket)| {
            self.entries.get(&ticket.request).is_some_and(|entry| {
                entry.generation == ticket.generation
                    && entry.queue_generation == ticket.queue_generation
                    && matches!(entry.phase, Phase::Ready { ticket: active } if active == *ticket)
                    && (entry.request.is_inquiry() == (lane == Lane::Inquiry))
                    && entry.request.context().control.class.priority_index() == priority
            }) && self.dispatch_eligible(*ticket, now)
        })
    }

    fn select_dispatch(&self, now: Instant) -> Option<DispatchSelection> {
        for priority in (0..4).rev() {
            let inquiry = self.eligible_ticket_readonly(Lane::Inquiry, priority, now);
            let command = self.eligible_ticket_readonly(Lane::Command, priority, now);
            let selected = match (inquiry, command) {
                (Some((index, ticket)), None) => Some(DispatchSelection {
                    lane: Lane::Inquiry,
                    priority,
                    index,
                    ticket,
                }),
                (None, Some((index, ticket))) => Some(DispatchSelection {
                    lane: Lane::Command,
                    priority,
                    index,
                    ticket,
                }),
                (Some((inquiry_index, inquiry_ticket)), Some((command_index, command_ticket))) => {
                    let inquiry_first = self
                        .entries
                        .get(&inquiry_ticket.request)
                        .zip(self.entries.get(&command_ticket.request))
                        .is_some_and(|(inquiry, command)| {
                            inquiry.admission_order <= command.admission_order
                        });
                    let (lane, index, ticket) = if inquiry_first {
                        (Lane::Inquiry, inquiry_index, inquiry_ticket)
                    } else {
                        (Lane::Command, command_index, command_ticket)
                    };
                    Some(DispatchSelection {
                        lane,
                        priority,
                        index,
                        ticket,
                    })
                }
                (None, None) => None,
            };
            if selected.is_some() {
                return selected;
            }
        }
        None
    }

    fn remove_ticket(&mut self, lane: Lane, priority: usize, index: usize) -> Option<QueueTicket> {
        match lane {
            Lane::Command => self.command_queues[priority].remove(index),
            Lane::Inquiry => self.inquiry_queues[priority].remove(index),
        }
    }

    fn dispatch_one(&mut self, now: Instant, effects: &mut Vec<Effect>) {
        if self.state != SessionState::Running {
            return;
        }
        for priority in 0..4 {
            self.prune_queue(Lane::Command, priority);
            self.prune_queue(Lane::Inquiry, priority);
        }
        let Some(selected) = self.select_dispatch(now) else {
            return;
        };
        self.dispatch_selected(selected, now, effects);
    }

    /// Sends every pacing-eligible cancellation whose ACK has already
    /// established a command socket.  Cancellation is urgent with respect to
    /// ordinary queued work, but its wire write still advances the shared
    /// command spacing deadline.  Re-scan after each write so zero-spacing
    /// profiles drain the complete urgent set; a positive spacing naturally
    /// stops the scan after the first write advances [`Self::last_request_sent`].
    fn drain_pending_cancellations(&mut self, now: Instant, effects: &mut Vec<Effect>) {
        if self.state != SessionState::Running {
            return;
        }
        loop {
            let pending = self
                .entries
                .iter()
                .filter_map(|(id, entry)| {
                    let socket = pending_cancellation_socket(entry)?;
                    if self.cancellation_send_at(entry) > now {
                        return None;
                    }
                    Some((entry.admission_order, *id, socket))
                })
                .min_by_key(|(admission_order, id, _)| (*admission_order, *id));
            let Some((_, id, socket)) = pending else {
                return;
            };
            let before = self
                .entries
                .get(&id)
                .map(|entry| (entry.phase, entry.cancellation));
            let last_sent = self.last_request_sent;
            self.emit_cancel(id, socket, now, effects);
            let after = self
                .entries
                .get(&id)
                .map(|entry| (entry.phase, entry.cancellation));
            if before == after && self.last_request_sent == last_sent {
                return;
            }
        }
    }

    fn has_pending_cancellation(&self) -> bool {
        self.entries
            .values()
            .any(|entry| pending_cancellation_socket(entry).is_some())
    }

    fn dispatch_selected(
        &mut self,
        selected: DispatchSelection,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let DispatchSelection {
            lane,
            priority,
            index,
            ticket,
        } = selected;
        let _ = self.remove_ticket(lane, priority, index);
        let Some(transmission) = self.allocate_transmission_id() else {
            self.finish(
                ticket.request,
                RuntimeOutcome::Failed(Error::RuntimeIdentityExhausted),
                effects,
            );
            return;
        };
        let Some(entry) = self.entries.get(&ticket.request) else {
            effects.push(Effect::Ignored(IgnoreReason::StaleQueueTicket));
            return;
        };
        if entry.generation != ticket.generation
            || entry.queue_generation != ticket.queue_generation
            || !matches!(entry.phase, Phase::Ready { ticket: active } if active == ticket)
        {
            effects.push(Effect::Ignored(IgnoreReason::StaleQueueTicket));
            return;
        }
        let generation = entry.generation;
        let attempt = entry.attempt;
        let target = entry.request.context().target;
        let cancellation = entry.cancellation;
        let dispatched_socket_capacity = (!entry.request.is_inquiry())
            .then(|| self.targets[target.id() as usize].map_or(1, |policy| policy.command_sockets));
        let wire = Arc::clone(entry.request.wire());
        let requested_sequence = if self.policy.envelope == EnvelopeKind::Sony {
            entry.current_sequence
        } else {
            None
        };
        let phase = Phase::Sending {
            transmission,
            started_at: now,
        };
        if let Some(entry) = self.entries.get_mut(&ticket.request) {
            entry.dispatched_socket_capacity = dispatched_socket_capacity;
        }
        self.transition(ticket.request, phase, cancellation, effects);
        self.transmissions.insert(
            transmission,
            TransmissionOwner {
                request: ticket.request,
                generation,
                attempt,
                kind: CorrelationKind::Request,
                requested_sequence,
            },
        );
        self.last_request_sent = Some(now);
        if lane == Lane::Inquiry {
            self.last_inquiry_sent = Some(now);
        }
        effects.push(Effect::Transmit {
            transmission,
            request: ticket.request,
            kind: Transmission::Request {
                target,
                wire,
                requested_sequence,
            },
        });
    }

    fn dispatch_eligible(&self, ticket: QueueTicket, now: Instant) -> bool {
        let Some(entry) = self.entries.get(&ticket.request) else {
            return false;
        };
        if entry.request.is_inquiry() {
            let target = entry.request.context().target;
            if self.raw_tombstone_blocks_dispatch(entry, target)
                || self.inquiries_inflight() >= self.policy.inquiry_capacity
                || self.uncorrelated_raw_shape_blocked(entry, target)
            {
                return false;
            }
            if self.inquiry_cooldown_until.is_some_and(|until| until > now) {
                return false;
            }
        } else {
            let target = entry.request.context().target;
            let Some(policy) = self.targets[target.id() as usize] else {
                return false;
            };
            if self.raw_tombstone_blocks_dispatch(entry, target)
                || self.raw_command_unacknowledged(target)
                || self.commands_inflight(target) >= usize::from(policy.command_sockets)
                || self.uncorrelated_raw_shape_blocked(entry, target)
            {
                return false;
            }
        }
        self.candidate_send_at(entry) <= now
    }

    fn candidate_send_at(&self, entry: &Entry) -> Instant {
        let request_spacing = self
            .policy
            .command_spacing
            .max(entry.request.context().control.minimum_spacing);
        let mut at = self.last_request_sent.map_or(entry.submitted_at, |last| {
            add_duration(last, request_spacing)
        });
        if entry.request.is_inquiry() {
            if let Some(last) = self.last_inquiry_sent {
                at = at.max(add_duration(last, self.policy.inquiry_spacing));
            }
            if let Some(cooldown) = self.inquiry_cooldown_until {
                at = at.max(cooldown);
            }
        }
        at
    }

    /// Earliest wire time for an owner-issued cancellation.  Unlike ordinary
    /// requests, cancellation does not inherit the request's control-class
    /// minimum spacing; only the shared profile command-spacing floor applies.
    fn cancellation_send_at(&self, entry: &Entry) -> Instant {
        self.last_request_sent.map_or(entry.submitted_at, |last| {
            add_duration(last, self.policy.command_spacing)
        })
    }

    fn inquiries_inflight(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| {
                entry.request.is_inquiry()
                    && matches!(
                        entry.phase,
                        Phase::Sending { .. } | Phase::AwaitingReply { .. }
                    )
            })
            .count()
    }

    fn commands_inflight(&self, target: CameraId) -> usize {
        self.entries
            .values()
            .filter(|entry| {
                !entry.request.is_inquiry()
                    && entry.request.context().target == target
                    && matches!(
                        entry.phase,
                        Phase::Sending { .. }
                            | Phase::AwaitingAck { .. }
                            | Phase::AwaitingCompletion { .. }
                            | Phase::Executing { .. }
                            | Phase::AwaitingCancellationResolution { .. }
                            | Phase::AwaitingLateAck { .. }
                    )
            })
            .count()
    }

    /// Whether a released raw response correlation still prevents later
    /// response-bearing work from starting on `target`.
    ///
    /// `NoReply` and `CompletionOnly` never establish a socket identity, and
    /// raw inquiry replies carry no request identity. A delayed raw frame is
    /// therefore indistinguishable from the same frame for new work. The only
    /// evidence-safe action is to hold that target's correlation lane for the
    /// bounded tombstone interval.
    fn raw_target_correlation_quarantined(&self, target: CameraId) -> bool {
        self.policy.envelope == EnvelopeKind::Raw
            && self.raw_target_tombstones[target.id() as usize]
                .is_some_and(RawTerminalTombstone::is_active)
    }

    /// Whether a target terminal tombstone prevents this ready request from
    /// dispatching.
    ///
    /// A successor that expects a response would make delayed terminal traffic
    /// ambiguous, including an inquiry whose socketless error has no
    /// command/inquiry discriminator. A second `NoReply` consumes no response
    /// at all, so it can safely write and extend the same fixed hold.
    fn raw_tombstone_blocks_dispatch(&self, entry: &Entry, target: CameraId) -> bool {
        self.raw_target_correlation_quarantined(target)
            && (entry.request.is_inquiry()
                || entry.request.context().reply_shape != ReplyShape::NoReply)
    }

    /// The deterministic release time for a ready request blocked only by a
    /// fixed raw target tombstone. Blocking first-write submission uses this
    /// to classify a caller deadline as a time-bound wait rather than generic
    /// queue backpressure.
    fn raw_tombstone_dispatch_deadline(&self, entry: &Entry) -> Option<Instant> {
        let target = entry.request.context().target;
        self.raw_tombstone_blocks_dispatch(entry, target)
            .then(|| {
                self.raw_target_tombstones[target.id() as usize]
                    .and_then(RawTerminalTombstone::dispatch_deadline)
            })
            .flatten()
    }

    /// Whether a raw completion-only command can be made the target's sole
    /// response-bearing command.  Besides live commands/inquiries, a terminal
    /// tombstone is incompatible: a socketless completion from the older
    /// command would otherwise be indistinguishable from this new command's
    /// terminal frame.
    fn raw_terminal_tombstone_active(&self, target: CameraId) -> bool {
        if self.policy.envelope != EnvelopeKind::Raw {
            return false;
        }
        self.raw_target_tombstones[target.id() as usize]
            .is_some_and(RawTerminalTombstone::is_active)
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
    fn raw_command_unacknowledged(&self, target: CameraId) -> bool {
        self.policy.envelope == EnvelopeKind::Raw
            && self.entries.values().any(|entry| {
                entry.request.context().target == target
                    && raw_unacknowledged_command_candidate(entry)
            })
    }

    /// Whether an uncorrelatable raw command is still occupying `target`.
    ///
    /// `CompletionOnly` has no socket through its whole lifecycle; `NoReply`
    /// has none during its local write. Both must keep same-target work from
    /// starting: a socketless error received during either phase otherwise has
    /// two possible owners.
    fn raw_uncorrelated_command_inflight(&self, target: CameraId) -> bool {
        self.policy.envelope == EnvelopeKind::Raw
            && self.entries.values().any(|entry| {
                !entry.request.is_inquiry()
                    && entry.request.context().target == target
                    && match entry.request.context().reply_shape {
                        ReplyShape::NoReply => matches!(entry.phase, Phase::Sending { .. }),
                        ReplyShape::CompletionOnly => matches!(
                            entry.phase,
                            Phase::Sending { .. }
                                | Phase::AwaitingCompletion { .. }
                                | Phase::AwaitingLateAck { .. }
                        ),
                        ReplyShape::AckThenCompletion => false,
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
    /// stopped by [`Self::raw_command_unacknowledged`]. The rule is raw-only:
    /// Sony correlates by sequence, so these shapes need no exclusivity there.
    fn uncorrelated_raw_shape_blocked(&self, entry: &Entry, target: CameraId) -> bool {
        if self.policy.envelope != EnvelopeKind::Raw {
            return false;
        }
        if entry.request.is_inquiry() {
            return self.raw_uncorrelated_command_inflight(target);
        }
        match entry.request.context().reply_shape {
            ReplyShape::NoReply => {
                self.commands_inflight(target) > 0 || self.raw_inquiry_inflight(target)
            }
            ReplyShape::CompletionOnly => {
                self.commands_inflight(target) > 0
                    || self.raw_inquiry_inflight(target)
                    || self.raw_terminal_tombstone_active(target)
            }
            ReplyShape::AckThenCompletion => false,
        }
    }

    /// Whether the raw single-candidate pre-ACK gate — and not genuine
    /// socket-capacity exhaustion — is what currently blocks a *new* command on
    /// `target`, such that pumping the pending peer ACK would free a socket for
    /// it.
    ///
    /// This deliberately uses the sole *ACK-capable* predecessor rather than
    /// [`Self::raw_command_unacknowledged`]. The latter is the broader
    /// correlation/exclusivity predicate and must continue to count
    /// completion-only commands and #671 late-ACK quarantines. Neither can
    /// release a socket by accepting an ACK: `AwaitingCompletion` has no ACK
    /// phase, while an `AwaitingLateAck` entry with `CancelState::None` is a
    /// quarantine whose late frames are ignored. A cancellation-driven late-ACK
    /// entry remains eligible because its ACK is still accepted and may assign
    /// the socket needed to issue cancellation.
    ///
    /// When the sole ACK-capable predecessor is still in its unacknowledged
    /// window while a command socket remains free, its ACK clears the gate and
    /// the next command can use that socket. When every socket is already
    /// occupied this is `false`, because the pending ACK only moves a command
    /// from awaiting-ACK to executing without releasing a socket — that is real
    /// contention, and the caller's fail-fast rejection must stand. Consumed by
    /// the blocking operation-submit path (issue #673).
    #[cfg(feature = "blocking")]
    pub(crate) fn raw_preack_gate_frees_socket_on_ack(&self, target: CameraId) -> bool {
        !self.raw_target_correlation_quarantined(target)
            && self.raw_ack_capable_candidate(target).is_some()
            && self.targets[target.id() as usize].is_some_and(|policy| {
                self.commands_inflight(target) < usize::from(policy.command_sockets)
            })
    }

    /// Returns the sole raw command on `target` whose next accepted frame may
    /// be an ACK, or `None` when there is no such command or the state is
    /// ambiguous.
    ///
    /// The `Sending` phase is included for the deferred-ACK race. A late-ACK
    /// quarantine is included only when cancellation intent is present: the
    /// default #671 quarantine (`CancelState::None`) deliberately ignores late
    /// frames and must never cause a blocking submission to wait for one.
    #[cfg(feature = "blocking")]
    fn raw_ack_capable_candidate(&self, target: CameraId) -> Option<RequestId> {
        if self.policy.envelope != EnvelopeKind::Raw {
            return None;
        }
        let mut sole = None;
        for (id, entry) in &self.entries {
            if entry.request.is_inquiry()
                || entry.request.context().target != target
                || entry.request.context().reply_shape != ReplyShape::AckThenCompletion
                || !matches!(
                    entry.phase,
                    Phase::Sending { .. } | Phase::AwaitingAck { .. }
                ) && !matches!(
                    entry.phase,
                    Phase::AwaitingLateAck { .. }
                        if !matches!(entry.cancellation, CancelState::None)
                )
            {
                continue;
            }
            if sole.is_some() {
                return None;
            }
            sole = Some(*id);
        }
        sole
    }

    fn transition(
        &mut self,
        id: RequestId,
        to: Phase,
        cancellation: CancelState,
        effects: &mut Vec<Effect>,
    ) {
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        let from = entry.phase;
        entry.phase = to;
        entry.cancellation = cancellation;
        effects.push(Effect::Transition {
            id,
            from,
            to,
            cancellation,
        });
    }

    fn transmission_finished(
        &mut self,
        transmission: TransmissionId,
        result: Result<TransmissionMeta, Error>,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let Some(owner) = self.transmissions.get(&transmission).copied() else {
            effects.push(Effect::Ignored(IgnoreReason::StaleTransmission));
            return;
        };
        let compatible = self.entries.get(&owner.request).is_some_and(|entry| {
            entry.generation == owner.generation
                && entry.attempt == owner.attempt
                && match owner.kind {
                    CorrelationKind::Request => {
                        matches!(entry.phase, Phase::Sending { transmission: active, .. } if active == transmission)
                    }
                    CorrelationKind::Cancellation => {
                        matches!(entry.cancellation, CancelState::Sending { transmission: active, .. } if active == transmission)
                    }
                }
        });
        if !compatible {
            self.transmissions.remove(&transmission);
            effects.push(Effect::Ignored(
                IgnoreReason::IncompatibleTransmissionResult,
            ));
            return;
        }
        match result {
            // A stream write error is a session verdict even when the owner
            // samples it after this request's total budget. The transport seam
            // cannot prove that no byte reached the wire, so the authoritative
            // failed-transmission path must poison before any per-request
            // late-result policy can classify it.
            Err(error) if self.policy.transport == TransportKind::Stream => {
                self.failed_transmission(owner, error, effects);
            }
            result => {
                if self.write_result_after_total_budget(owner, now, effects) {
                    return;
                }
                self.transmissions.remove(&transmission);
                match result {
                    Ok(meta) => self.successful_transmission(owner, meta, now, effects),
                    Err(error) => self.failed_transmission(owner, error, effects),
                }
            }
        }
    }

    /// Applies the admission-to-terminal budget before an active request write
    /// result may mutate protocol state.
    ///
    /// An input sampled exactly at the deadline remains input-first, but a
    /// result sampled strictly later is outside the request's total budget.
    /// This ordering covers `Ok` and datagram `Err`: once the result is late, a
    /// raw command write can no longer prove whether the camera observed the
    /// frame, while a Sony write must not register a sequence or consume a
    /// deferred response. A compatible stream `Err` is handled first by
    /// [`Self::failed_transmission`], because no per-request boundary can make
    /// an unknowable stream position safe. Cancellation writes deliberately
    /// bypass this guard because their ambiguity window, rather than the
    /// request retry budget, owns the active cancellation lifecycle.
    ///
    /// Keeping this at the engine's one transmission-result ingress also makes
    /// [`Self::finish_write_without_due`] obey the same boundary as ordinary
    /// [`Self::handle`] turns.
    fn write_result_after_total_budget(
        &mut self,
        owner: TransmissionOwner,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) -> bool {
        let Some((deadline, raw_active_command, last_error)) =
            self.entries.get(&owner.request).and_then(|entry| {
                retry_budget_deadline(entry).map(|deadline| {
                    (
                        deadline,
                        self.policy.envelope == EnvelopeKind::Raw
                            && !entry.request.is_inquiry()
                            && matches!(entry.phase, Phase::Sending { .. }),
                        entry.last_error.clone(),
                    )
                })
            })
        else {
            return false;
        };
        if owner.kind != CorrelationKind::Request || deadline >= now {
            return false;
        }

        if raw_active_command {
            // This also removes the active transmission and clears any ACK or
            // completion latch that raced the write, then retains raw
            // correlation through its ambiguity window (or poisons in strict
            // recovery mode).
            self.terminate_unconfirmed_raw(owner.request, now, effects);
        } else {
            // A raw single-flight inquiry can be physically uncertain while
            // its write result is still pending too. Retain the same bounded
            // target hold before normal terminal cleanup so a delayed reply
            // cannot bind to its same-target successor.
            self.quarantine_raw_inquiry_correlation(owner.request, now);
            // A late Sony result — including a late transport error — cannot
            // extend the budget or mutate correlation. Preserve the prior
            // retry cause when there is one, matching ordinary budget expiry.
            self.finish(
                owner.request,
                RuntimeOutcome::Failed(last_error.unwrap_or(Error::Timeout)),
                effects,
            );
        }
        true
    }

    fn successful_transmission(
        &mut self,
        owner: TransmissionOwner,
        meta: TransmissionMeta,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let metadata_valid = match self.policy.envelope {
            EnvelopeKind::Raw => meta.sequence.is_none(),
            EnvelopeKind::Sony => meta.sequence.is_some(),
        };
        let requested_sequence_valid = owner
            .requested_sequence
            .is_none_or(|requested| meta.sequence == Some(requested));
        if !metadata_valid || !requested_sequence_valid {
            self.finish(
                owner.request,
                RuntimeOutcome::Failed(Error::InvalidState(
                    "transmission metadata conflicts with session envelope".into(),
                )),
                effects,
            );
            return;
        }
        if let Some(sequence) = meta.sequence {
            self.register_sequence(owner.request, sequence, owner.kind);
        }
        match owner.kind {
            CorrelationKind::Request => {
                let Some(entry) = self.entries.get_mut(&owner.request) else {
                    return;
                };
                // Only the exact, currently-authoritative request write may
                // establish the sequence reused by a later retry.  A
                // cancellation is a separate logical message and deliberately
                // does not enter this field.
                if self.policy.envelope == EnvelopeKind::Sony {
                    entry.current_sequence = meta.sequence;
                }
                entry.transmission_order = Some(self.next_transmission_order);
                self.next_transmission_order = self.next_transmission_order.wrapping_add(1);
                let request_is_inquiry = entry.request.is_inquiry();
                let context = *entry.request.context();
                let cancellation = entry.cancellation;
                let generation = entry.generation;
                if request_is_inquiry {
                    let deadline = add_duration(now, context.timeout.inquiry);
                    self.transition(
                        owner.request,
                        Phase::AwaitingReply {
                            sent_at: now,
                            deadline,
                        },
                        cancellation,
                        effects,
                    );
                    if self.policy.envelope == EnvelopeKind::Raw {
                        self.raw_inquiries[context.target.id() as usize].push_back(
                            CorrelationOwner {
                                request: owner.request,
                                generation,
                                kind: CorrelationKind::Request,
                            },
                        );
                    }
                } else {
                    match context.reply_shape {
                        ReplyShape::AckThenCompletion => {
                            let deadline = add_duration(now, context.timeout.ack);
                            self.transition(
                                owner.request,
                                Phase::AwaitingAck {
                                    sent_at: now,
                                    deadline,
                                },
                                cancellation,
                                effects,
                            );
                            // Issue #297: an owner whose reader is not strictly
                            // ordered behind its writer can hand the engine the
                            // camera's ACK before this write result. That ACK was
                            // latched rather than dropped, so apply it now that
                            // the request is authoritatively awaiting one.
                            let deferred = self
                                .entries
                                .get_mut(&owner.request)
                                .and_then(|entry| entry.deferred_ack.take());
                            if let Some(deferred) = deferred {
                                self.ack(owner.request, deferred.socket, now, effects);
                            }
                        }
                        ReplyShape::CompletionOnly => {
                            // Issue #700: a completion-only command receives no
                            // ACK and is never assigned a socket, so it skips the
                            // AwaitingAck phase (and its ACK-timeout/poison path)
                            // and waits for its completion under the completion
                            // deadline. Any ACK that raced the write is spurious
                            // for this shape and is discarded with the latch.
                            let deadline = add_duration(now, context.timeout.completion);
                            self.transition(
                                owner.request,
                                Phase::AwaitingCompletion {
                                    sent_at: now,
                                    deadline,
                                },
                                cancellation,
                                effects,
                            );
                            // The completion can race the write result the same
                            // way an ACK can; apply it now if it was latched, and
                            // drop any spurious raced ACK.
                            let deferred = self.entries.get_mut(&owner.request).and_then(|entry| {
                                entry.deferred_ack = None;
                                entry.deferred_completion.take()
                            });
                            if let Some(deferred) = deferred {
                                self.completion(owner.request, deferred.socket, now, effects);
                            }
                        }
                        ReplyShape::NoReply => {
                            // A fire-and-forget command reaches its local-write
                            // terminal here. It owns no response identity, so
                            // retain a bounded target tombstone before releasing
                            // the entry: a late command frame must not bind to a
                            // later response-bearing raw work.
                            self.quarantine_raw_terminal(owner.request, now);
                            self.finish(owner.request, RuntimeOutcome::Written, effects);
                        }
                    }
                }
            }
            CorrelationKind::Cancellation => {
                let Some(entry) = self.entries.get(&owner.request) else {
                    return;
                };
                let CancelState::Sending {
                    socket,
                    ambiguity_deadline,
                    ..
                } = entry.cancellation
                else {
                    effects.push(Effect::Ignored(
                        IgnoreReason::IncompatibleTransmissionResult,
                    ));
                    return;
                };
                let observation_deadline =
                    add_duration(now, entry.request.context().timeout.cancellation);
                self.transition(
                    owner.request,
                    Phase::AwaitingCancellationResolution {
                        socket,
                        // Capacity/correlation are conservatively retained through
                        // the full ambiguity quarantine even if an observer-facing
                        // cancellation response timeout is shorter.
                        deadline: ambiguity_deadline,
                    },
                    CancelState::AwaitingTerminal {
                        ambiguity_deadline,
                        observation_deadline,
                    },
                    effects,
                );
            }
        }
    }

    /// Applies one failed transport write.
    ///
    /// **Datagram**: exactly one request fails, with its own transport error,
    /// and the session keeps running. A datagram is framed by its own boundary,
    /// so a failed write cannot corrupt any other transmission.
    ///
    /// **Stream**: the session is poisoned. This is a deliberate, explicit
    /// policy rather than an accident. The write seam is `Result<(), Error>`
    /// for every transport implementation, public ones included, so a failed
    /// stream write cannot prove that no byte of the frame reached the wire;
    /// a partially written frame desynchronizes the camera's parser for every
    /// frame that follows, which makes the byte-stream position unknowable.
    /// Fail-one-command would therefore leave the caller believing a session
    /// that is already unusable is healthy, and [`Error::StreamPoisoned`] is
    /// the one terminal error that answers
    /// [`Error::requires_new_session`](crate::Error::requires_new_session)
    /// correctly (issue #564). Narrowing this to "poison only on a genuine
    /// partial write" needs a write seam that reports how many bytes reached
    /// the wire, which no transport trait in this crate offers.
    ///
    /// The exact transport cause is never lost: it is carried in the poison
    /// reason. 1.x drew the same line — `fail_after_send_error` failed the one
    /// command, and the runtime loops around it (`handle_send_failure!` in
    /// `loop_task.rs`, the `SendSemantics::Stream` arms in `blocking_runner.rs`)
    /// then poisoned every stream session anyway.
    fn failed_transmission(
        &mut self,
        owner: TransmissionOwner,
        error: Error,
        effects: &mut Vec<Effect>,
    ) {
        if self.policy.transport == TransportKind::Stream {
            self.terminate_session(
                SessionState::Poisoned,
                Error::StreamPoisoned {
                    reason: Cow::Owned(error.to_string()),
                },
                effects,
            );
            return;
        }
        match owner.kind {
            CorrelationKind::Request => {
                self.finish(owner.request, RuntimeOutcome::Failed(error), effects);
            }
            CorrelationKind::Cancellation => {
                self.failed_cancellation_transmission(owner, error, effects);
            }
        }
    }

    fn failed_cancellation_transmission(
        &mut self,
        owner: TransmissionOwner,
        error: Error,
        effects: &mut Vec<Effect>,
    ) {
        let Some(entry) = self.entries.get(&owner.request) else {
            return;
        };
        let CancelState::Sending {
            socket,
            ambiguity_deadline,
            ..
        } = entry.cancellation
        else {
            effects.push(Effect::Ignored(
                IgnoreReason::IncompatibleTransmissionResult,
            ));
            return;
        };
        let phase = entry.phase;
        self.transition(
            owner.request,
            phase,
            CancelState::ObservationFailed {
                socket,
                ambiguity_deadline,
            },
            effects,
        );
        if let Some(entry) = self.entries.get_mut(&owner.request) {
            entry.cancellation_observation_open = false;
        }
        effects.push(Effect::CancellationObservation {
            id: owner.request,
            observation: CancellationObservation::Failed(error),
        });
    }

    /// Applies one transient receive-side transport failure.
    ///
    /// This restores the 1.x `SchedulerEvent::NetworkError` contract only for
    /// requests whose envelope supplies safe evidence. A sequenced Sony command
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
    /// is recorded. A cancellation already in flight follows its own late-ACK
    /// ambiguity resolution and poisons under strict mode only if that deadline
    /// remains unconfirmed. The classic case
    /// is a UDP `recv` returning ECONNREFUSED because an earlier datagram drew
    /// an ICMP port-unreachable.
    ///
    /// Two deliberate narrowings of the 1.x scan, both conservative:
    /// inquiries are untouched (1.x scanned only its command table), and a
    /// request with cancellation in flight is left to its ambiguity deadline,
    /// because retrying it would abandon the quarantine that owns its socket.
    fn receive_fault(&mut self, error: &Error, now: Instant, effects: &mut Vec<Effect>) {
        if self.state != SessionState::Running {
            effects.push(Effect::Ignored(IgnoreReason::SessionNotRunning));
            return;
        }
        let mut affected: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                !entry.request.is_inquiry()
                    && matches!(entry.phase, Phase::AwaitingAck { .. })
                    && matches!(entry.cancellation, CancelState::None)
            })
            .map(|(id, entry)| (entry.admission_order, *id))
            .collect();
        affected.sort_unstable_by_key(|(order, _)| *order);
        if self.raw_unconfirmed_poison() && !affected.is_empty() {
            self.poison_strict_unconfirmed(effects);
            return;
        }
        if self.policy.envelope == EnvelopeKind::Raw {
            // Default: leave any unacknowledged raw command to its own ACK
            // deadline; never replay an unconfirmed raw command.
            return;
        }
        for (_, id) in affected {
            self.schedule_retry(id, now, error.clone(), Backoff::Uncapped, effects);
        }
    }

    fn register_sequence(&mut self, id: RequestId, sequence: u32, kind: CorrelationKind) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let owner = CorrelationOwner {
            request: id,
            generation: entry.generation,
            kind,
        };
        if entry
            .sequence_history
            .iter()
            .any(|record| record.sequence == sequence && record.kind == kind)
        {
            return;
        }
        let evicted = if entry.sequence_history.len() == MAX_SEQUENCE_HISTORY {
            entry.sequence_history.first().copied()
        } else {
            None
        };
        if let Some(evicted) = evicted {
            self.remove_correlation_owner(
                evicted.sequence,
                owner.request,
                owner.generation,
                evicted.kind,
            );
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.sequence_history.remove(0);
            }
        }
        if let Some(entry) = self.entries.get_mut(&id) {
            entry
                .sequence_history
                .push(SequenceRecord { sequence, kind });
        }
        add_unique_owner(self.sequences.entry(sequence).or_default(), owner);
        add_unique_owner(
            self.lower_sequences.entry(sequence as u16).or_default(),
            owner,
        );
    }

    fn remove_correlation_owner(
        &mut self,
        sequence: u32,
        request: RequestId,
        generation: GenerationTicket,
        kind: CorrelationKind,
    ) {
        let remove_exact = if let Some(owners) = self.sequences.get_mut(&sequence) {
            owners.retain(|owner| {
                !(owner.request == request && owner.generation == generation && owner.kind == kind)
            });
            owners.is_empty()
        } else {
            false
        };
        if remove_exact {
            self.sequences.remove(&sequence);
        }
        let lower = sequence as u16;
        let remove_lower = if let Some(owners) = self.lower_sequences.get_mut(&lower) {
            owners.retain(|owner| {
                !(owner.request == request && owner.generation == generation && owner.kind == kind)
            });
            owners.is_empty()
        } else {
            false
        };
        if remove_lower {
            self.lower_sequences.remove(&lower);
        }
    }

    fn resolve_sequence(
        &self,
        target: CameraId,
        sequence: EnvelopeSequence,
    ) -> Result<(RequestId, CorrelationKind), IgnoreReason> {
        let exact_candidates = self
            .sequences
            .get(&sequence.value)
            .map_or_else(SmallVec::new, |owners| self.valid_owners(owners, None));
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
        let lower_candidates = self
            .lower_sequences
            .get(&(sequence.value as u16))
            .map_or_else(SmallVec::new, |owners| {
                self.valid_owners(owners, Some(target))
            });
        let unique = unique_requests(lower_candidates.iter().copied());
        match unique.as_slice() {
            [owner] => Ok((owner.request, owner.kind)),
            [] => Err(IgnoreReason::UnmatchedSequencedFrame),
            _ => Err(IgnoreReason::AmbiguousLower16Sequence),
        }
    }

    fn valid_owners(
        &self,
        owners: &[CorrelationOwner],
        target: Option<CameraId>,
    ) -> SmallVec<[CorrelationOwner; 4]> {
        owners
            .iter()
            .copied()
            .filter(|owner| {
                self.entries.get(&owner.request).is_some_and(|entry| {
                    entry.generation == owner.generation
                        && target.is_none_or(|expected| entry.request.context().target == expected)
                        && correlation_phase_compatible(entry, owner.kind)
                })
            })
            .collect()
    }

    fn owner_target(&self, owner: CorrelationOwner) -> Option<CameraId> {
        self.entries
            .get(&owner.request)
            .filter(|entry| entry.generation == owner.generation)
            .map(|entry| entry.request.context().target)
    }

    fn frame(&mut self, frame: DecodedFrame, now: Instant, effects: &mut Vec<Effect>) {
        if self.state != SessionState::Running {
            effects.push(Effect::Ignored(IgnoreReason::SessionNotRunning));
            return;
        }
        // An unsequenced raw terminal response cannot identify which released
        // correlation it answers. Check the fixed-size tombstones before the
        // ordinary raw resolver: letting the resolver see it could otherwise
        // latch an ACK on a successor that is still Sending, advance a
        // successor awaiting its ACK, finish a successor inquiry with stale
        // payload, or fail/retry either on a socketless error.
        if self.policy.envelope == EnvelopeKind::Raw
            && frame.sequence.is_none()
            && self.raw_terminal_response_quarantined(&frame)
        {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        let resolved = if let Some(sequence) = frame.sequence {
            match self.resolve_sequence(frame.target, sequence) {
                Ok(owner) => Some(owner),
                Err(reason) => {
                    effects.push(Effect::Ignored(reason));
                    return;
                }
            }
        } else if self.policy.envelope == EnvelopeKind::Sony {
            effects.push(Effect::Ignored(IgnoreReason::MalformedFrame));
            return;
        } else {
            self.resolve_raw(&frame)
                .map(|id| (id, CorrelationKind::Request))
        };
        let Some((id, correlation_kind)) = resolved else {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
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
        // An external frame at the exact response deadline wins, but a frame
        // received strictly later must not revive or mutate the expired phase.
        // Resolve it first: a deadline on one request must never make an
        // unrelated frame inert. `finish_input_turn` will then apply the normal
        // due transition for this still-active entry.
        if self.entries.get(&id).is_some_and(|entry| {
            correlated_response_deadline(entry).is_some_and(|deadline| deadline < now)
        }) {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        // Issue #671: a raw command that has already failed
        // `UnsequencedCommandUnconfirmed` is only holding its correlation slot
        // (its socket, or its place as the sole unacknowledged command) until
        // the ambiguity deadline. Any late frame that resolves to it is ignored,
        // never applied: applying it could re-open a request the caller was told
        // is unconfirmed, and dropping it here keeps the slot reserved so the
        // late reply cannot be misattributed to a later command.
        if self.entries.get(&id).is_some_and(is_unconfirmed_quarantine) {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        match frame.response {
            DecodedResponse::Ack { socket } => self.ack(id, socket, now, effects),
            DecodedResponse::Completion { socket } => {
                if correlation_kind == CorrelationKind::Cancellation {
                    self.confirm_cancelled(id, effects);
                } else {
                    self.completion(id, socket, now, effects);
                }
            }
            DecodedResponse::InquiryReply { route, payload } => {
                if correlation_kind == CorrelationKind::Cancellation {
                    effects.push(Effect::Ignored(IgnoreReason::MalformedFrame));
                } else {
                    self.inquiry_reply(id, route, payload, now, effects);
                }
            }
            DecodedResponse::Error { socket, code } => {
                self.camera_error(id, correlation_kind, socket, code, now, effects);
            }
            DecodedResponse::NetworkChange | DecodedResponse::Unknown => {
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
    fn raw_terminal_response_quarantined(&self, frame: &DecodedFrame) -> bool {
        let target = frame.target;
        let target_index = target.id() as usize;
        let Some(tombstone) = self.raw_target_tombstones[target_index] else {
            return false;
        };
        if tombstone.terminal_deadline.is_some() {
            return matches!(
                &frame.response,
                DecodedResponse::Ack { .. }
                    | DecodedResponse::Completion { .. }
                    | DecodedResponse::InquiryReply { .. }
                    | DecodedResponse::Error { .. }
            );
        }
        if tombstone.inquiry_deadline.is_none() {
            return false;
        }
        match &frame.response {
            // A live unique pre-ACK request was admitted before this inquiry
            // hold. Its complete ACK is still attributable; the hold prevents
            // a successor from becoming a competing candidate.
            DecodedResponse::Ack { .. } => self.unique_raw_command_candidate(target).is_none(),
            // An inquiry reply is target/data-correlated and a socketless error
            // has no command/inquiry discriminator, so both remain inert.
            DecodedResponse::InquiryReply { .. } | DecodedResponse::Error { socket: None, .. } => {
                true
            }
            // A named terminal is safe only when the exact socket is still
            // owned. An unowned named frame must not fall through to any
            // successor or inquiry FIFO while this hold is active.
            DecodedResponse::Completion {
                socket: Some(socket),
            }
            | DecodedResponse::Error {
                socket: Some(socket),
                ..
            } => self.socket_owner(target, *socket).is_none(),
            // Socketless completion is safe solely when an already-live socket
            // owner makes it uniquely attributable. It cannot be stale inquiry
            // data and no successor can dispatch under this hold.
            DecodedResponse::Completion { socket: None } => {
                self.sole_socket_holder(target).is_none()
            }
            DecodedResponse::NetworkChange | DecodedResponse::Unknown => false,
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
            DecodedResponse::InquiryReply { route, .. } => {
                if let Some(route) = route.filter(|route| *route != InquiryRoute::UNKNOWN) {
                    let matches: SmallVec<[RequestId; 4]> = self
                        .entries
                        .iter()
                        .filter(|(_, entry)| {
                            entry.request.context().target == target
                                && entry.request.inquiry_route() == Some(route)
                                && matches!(entry.phase, Phase::AwaitingReply { .. })
                        })
                        .map(|(id, _)| *id)
                        .collect();
                    if let [id] = matches.as_slice() {
                        return Some(*id);
                    }
                }
                self.raw_inquiry_front(target)
            }
            DecodedResponse::Error { socket, .. } => {
                if let Some(socket) = socket {
                    // A named socket is authoritative.  An unowned socket is
                    // not evidence for any other request, so never fall back
                    // to inquiry FIFO or a command candidate.
                    return self.socket_owner(target, *socket);
                }
                let inquiry_owner = self.raw_inquiry_front(target);
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
            DecodedResponse::NetworkChange | DecodedResponse::Unknown => None,
        }
    }

    fn raw_inquiry_front(&self, target: CameraId) -> Option<RequestId> {
        self.raw_inquiries[target.id() as usize]
            .iter()
            .find(|owner| {
                self.entries.get(&owner.request).is_some_and(|entry| {
                    entry.generation == owner.generation
                        && entry.request.context().target == target
                        && matches!(entry.phase, Phase::AwaitingReply { .. })
                })
            })
            .map(|owner| owner.request)
    }

    /// Whether a raw inquiry has been sent or is awaiting its reply on
    /// `target`.  `raw_inquiry_front` is the authoritative FIFO owner for
    /// routing, but a Sending inquiry is still live and must make a concurrent
    /// socketless error ambiguous with an unacknowledged command.
    fn raw_inquiry_inflight(&self, target: CameraId) -> bool {
        self.entries.values().any(|entry| {
            entry.request.is_inquiry()
                && entry.request.context().target == target
                && matches!(
                    entry.phase,
                    Phase::Sending { .. } | Phase::AwaitingReply { .. }
                )
        })
    }

    fn socket_owner(&self, target: CameraId, socket: ViscaSocket) -> Option<RequestId> {
        let owner = self.socket_owners[target.id() as usize][socket.as_index()]?;
        self.entries
            .get(&owner.request)
            .filter(|entry| {
                entry.generation == owner.generation
                    && entry.request.context().target == target
                    && phase_owns_socket(entry.phase, socket)
            })
            .map(|_| owner.request)
    }

    /// The single request currently holding a command socket on `target`, if
    /// exactly one does.
    fn sole_socket_holder(&self, target: CameraId) -> Option<RequestId> {
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
    /// Raw dispatch admits only one unacknowledged command per target (decision
    /// D7), so a raw target can never hold two entries at once in
    /// `Sending`, `AwaitingAck`, or `AwaitingLateAck`. A multi-`AwaitingAck` raw
    /// state is therefore unreachable, which is what makes the ambiguous
    /// multi-candidate raw correlation raised in #669 impossible to reach in the
    /// first place. Keep this resolver exact regardless: if an invariant
    /// regression ever produced a second candidate it returns `None` (fails
    /// closed) rather than turning admission order into an ACK/error guess. The
    /// `Sending` phase is included for the deferred-ACK latch. Only an
    /// `AckThenCompletion` command is eligible: `NoReply` has no response
    /// lifecycle, and `CompletionOnly` deliberately has no ACK/socket
    /// lifecycle, so either shape must keep an ACK racing its local write inert.
    fn unique_raw_command_candidate(&self, target: CameraId) -> Option<RequestId> {
        let mut sole = None;
        for (id, entry) in &self.entries {
            if self.policy.envelope != EnvelopeKind::Raw
                || entry.request.is_inquiry()
                || entry.request.context().target != target
                || entry.request.context().reply_shape != ReplyShape::AckThenCompletion
                || !matches!(
                    entry.phase,
                    Phase::Sending { .. }
                        | Phase::AwaitingAck { .. }
                        | Phase::AwaitingLateAck { .. }
                )
            {
                continue;
            }
            if sole.is_some() {
                return None;
            }
            sole = Some(*id);
        }
        sole
    }

    /// The unique completion-only raw command on `target` awaiting (or about to
    /// await) its completion, if exactly one exists (issue #700).
    ///
    /// A completion-only command owns no socket, so a completion for it cannot be
    /// resolved by [`Self::socket_owner`]/[`Self::sole_socket_holder`]. It is the
    /// sole in-flight command on its target (the dispatch gate guarantees it), so
    /// its completion is unambiguous. [`Phase::Sending`] is included so a
    /// completion that races ahead of the write result can be latched and applied
    /// once the send is confirmed, mirroring the deferred-ACK latch. Fails closed
    /// (returns `None`) if a second candidate is ever present, so it never guesses.
    fn completion_only_candidate(&self, target: CameraId) -> Option<RequestId> {
        let mut sole = None;
        for (id, entry) in &self.entries {
            if self.policy.envelope != EnvelopeKind::Raw
                || entry.request.is_inquiry()
                || entry.request.context().target != target
                || entry.request.context().reply_shape != ReplyShape::CompletionOnly
                || !matches!(
                    entry.phase,
                    Phase::Sending { .. } | Phase::AwaitingCompletion { .. }
                )
            {
                continue;
            }
            if sole.is_some() {
                return None;
            }
            sole = Some(*id);
        }
        sole
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
    fn unique_raw_error_candidate(&self, target: CameraId) -> Option<RequestId> {
        let mut sole = None;
        for (id, entry) in &self.entries {
            if self.policy.envelope != EnvelopeKind::Raw
                || entry.request.is_inquiry()
                || entry.request.context().target != target
            {
                continue;
            }
            let is_candidate = entry.request.context().reply_shape != ReplyShape::NoReply
                && (matches!(
                    entry.phase,
                    Phase::Sending { .. }
                        | Phase::AwaitingAck { .. }
                        | Phase::AwaitingLateAck { .. }
                ) || (entry.request.context().reply_shape == ReplyShape::CompletionOnly
                    && matches!(entry.phase, Phase::AwaitingCompletion { .. })));
            if !is_candidate {
                continue;
            }
            if sole.is_some() {
                return None;
            }
            sole = Some(*id);
        }
        sole
    }

    fn command_sockets(&self, target: CameraId) -> usize {
        self.targets[target.id() as usize].map_or(1, |policy| usize::from(policy.command_sockets))
    }

    /// Socket assignment may need the capacity that was in force when this
    /// exact command attempt was dispatched. A live retune lowers admission
    /// capacity immediately, but it does not revoke the second physical socket
    /// from a pre-retune attempt already draining under the old capacity.
    fn assignment_socket_count(&self, target: CameraId, id: RequestId) -> usize {
        let current = self.command_sockets(target);
        self.entries
            .get(&id)
            .and_then(|entry| entry.dispatched_socket_capacity)
            .map_or(current, |dispatched| current.max(usize::from(dispatched)))
    }

    fn socket_available(&self, target: CameraId, socket: ViscaSocket, id: RequestId) -> bool {
        self.socket_owner(target, socket)
            .is_none_or(|owner| owner == id)
    }

    /// Chooses the socket an ACK assigns using only the evidence in that ACK.
    ///
    /// A camera that names a free socket is authoritative about it. When the
    /// named socket is instead held by another request — typically because a
    /// completion frame was lost and the camera reused the socket — the ACK
    /// falls back to the target's other socket if it is free (issue #620/#682).
    /// The candidate request was already uniquely identified before this runs
    /// (a raw ACK resolves to the sole unacknowledged command), so binding it to
    /// the free socket cannot mis-attribute the ACK; it only avoids wedging the
    /// command, which — before issue #671 — cascaded to a session poison at its
    /// ACK deadline. An ACK with no socket nibble takes the first free physical
    /// socket available to that dispatched attempt. `None` means every socket it
    /// may still use is already taken and the ACK stays inert.
    fn assign_socket(
        &self,
        target: CameraId,
        requested: Option<ViscaSocket>,
        id: RequestId,
    ) -> Option<ViscaSocket> {
        let assignment_socket_count = self.assignment_socket_count(target, id);
        if let Some(socket) = requested {
            if self.socket_available(target, socket, id) {
                return Some(socket);
            }
            let other = other_socket(socket);
            if assignment_socket_count > 1 && self.socket_available(target, other, id) {
                return Some(other);
            }
            return None;
        }
        [ViscaSocket::S1, ViscaSocket::S2]
            .into_iter()
            .take(assignment_socket_count)
            .find(|socket| self.socket_available(target, *socket, id))
    }

    fn ack(
        &mut self,
        id: RequestId,
        socket: Option<ViscaSocket>,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        if entry.request.is_inquiry()
            || entry.request.context().reply_shape != ReplyShape::AckThenCompletion
        {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        match entry.phase {
            Phase::AwaitingAck { .. } | Phase::AwaitingLateAck { .. } => {}
            // Issue #297: the camera answered before this owner applied the
            // write result for the frame being answered. Latch the ACK on the
            // entry; `successful_transmission` applies it the instant the
            // request is authoritatively awaiting one. Dropping it here is what
            // reopens the race.
            //
            // Issue #636: the latch holds the first ACK this attempt raced and
            // is never overwritten. A second ACK arriving while the same frame
            // is still being written is either a duplicate or answers a frame
            // this engine has not written yet; letting it replace the latch
            // would hand the first ACK's socket to the wrong request.
            Phase::Sending { .. } => {
                if entry.deferred_ack.is_some() {
                    effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
                } else if let Some(entry) = self.entries.get_mut(&id) {
                    entry.deferred_ack = Some(DeferredAck { socket });
                }
                return;
            }
            // Issue #700: a completion-only command receives no ACK. If the
            // camera nonetheless sends one it is spurious for this shape, so it
            // is ignored — it must never assign the command a socket.
            Phase::AwaitingCompletion { .. }
            | Phase::Ready { .. }
            | Phase::Executing { .. }
            | Phase::AwaitingReply { .. }
            | Phase::Backoff { .. }
            | Phase::AwaitingCancellationResolution { .. } => {
                effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
                return;
            }
        }
        let target = entry.request.context().target;
        let Some(socket) = self.assign_socket(target, socket, id) else {
            effects.push(Effect::Ignored(IgnoreReason::SocketConflict));
            return;
        };
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        let generation = entry.generation;
        let cancellation = entry.cancellation;
        let deadline = add_duration(now, entry.request.context().timeout.completion);
        self.socket_owners[target.id() as usize][socket.as_index()] = Some(SocketOwner {
            request: id,
            generation,
        });
        self.transition(
            id,
            Phase::Executing {
                socket,
                started_at: now,
                deadline,
            },
            cancellation,
            effects,
        );
        if matches!(
            cancellation,
            CancelState::Requested { .. } | CancelState::ObservationFailed { .. }
        ) {
            self.emit_cancel(id, socket, now, effects);
        }
    }

    fn completion(
        &mut self,
        id: RequestId,
        socket: Option<ViscaSocket>,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        if entry.request.is_inquiry() {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        let phase = entry.phase;
        let completion_only = entry.request.context().reply_shape == ReplyShape::CompletionOnly;
        let has_deferred = entry.deferred_completion.is_some();
        let accept = match phase {
            // A completion that carries no socket nibble was already
            // attributed by sequence (Sony) or by sole socket ownership
            // (raw), so it completes whichever socket this request owns.
            Phase::Executing { socket: owned, .. }
            | Phase::AwaitingCancellationResolution { socket: owned, .. } => {
                socket.is_none_or(|socket| socket == owned)
            }
            // A Sony exact completion can legitimately beat or replace an ACK.
            Phase::AwaitingAck { .. } | Phase::AwaitingLateAck { .. } => {
                self.policy.envelope == EnvelopeKind::Sony
            }
            // Issue #700: a completion-only command owns no socket. The resolver
            // established it as the sole completion-only candidate on the target,
            // so accept its completion regardless of any socket nibble the vendor
            // frame echoes.
            Phase::AwaitingCompletion { .. } => true,
            // Issue #700 / #297: the completion raced ahead of the write result
            // for a completion-only command. Latch it, as a deferred ACK is
            // latched, and apply it once `successful_transmission` confirms the
            // send. The first latch wins; a second is a duplicate.
            Phase::Sending { .. } if completion_only => {
                if has_deferred {
                    effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
                } else if let Some(entry) = self.entries.get_mut(&id) {
                    entry.deferred_completion = Some(DeferredCompletion { socket });
                }
                return;
            }
            _ => false,
        };
        if !accept {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        self.quarantine_raw_terminal(id, now);
        self.finish(id, RuntimeOutcome::Applied, effects);
    }

    /// Retains bounded raw-correlation evidence after an uncorrelatable
    /// successful command terminal.
    ///
    /// `NoReply` and `CompletionOnly` earned no response identity, so a later
    /// response-bearing command on that target must wait through the ambiguity
    /// window. Ordinary ACK-then-completion commands deliberately do not add a
    /// terminal hold: the camera may immediately reuse their released socket,
    /// and this protocol surface has no frame identity with which to distinguish
    /// a duplicate from that legitimate next response.
    fn quarantine_raw_terminal(&mut self, id: RequestId, now: Instant) {
        let Some((target, ambiguity)) = self.entries.get(&id).and_then(|entry| {
            let uncorrelatable_command = !entry.request.is_inquiry()
                && matches!(
                    entry.request.context().reply_shape,
                    ReplyShape::NoReply | ReplyShape::CompletionOnly
                );
            (self.policy.envelope == EnvelopeKind::Raw && uncorrelatable_command).then_some((
                entry.request.context().target,
                entry.request.context().timeout.ambiguity,
            ))
        }) else {
            return;
        };
        let tombstone = RawTerminalTombstone::terminal(add_duration(now, ambiguity));
        let target_index = target.id() as usize;
        extend_tombstone(&mut self.raw_target_tombstones[target_index], tombstone);
    }

    /// Retains one bounded raw inquiry hold before its target-only reply
    /// correlation is released or becomes uncertain while its write is still
    /// pending in the raw single-flight topology. A raw reply has no request
    /// identity, so the hold is deliberately a bounded ambiguity policy rather
    /// than a claim of permanent identity: stale or duplicate frames are inert
    /// until expiry, then a queued successor may acquire the lane. This
    /// helper's single-flight precondition is intentional: a wider raw FIFO is
    /// *not* protected by this hold, because it cannot distinguish a duplicate
    /// from an already-live next inquiry. Production raw owners enter this
    /// engine through the adapter with `inquiry_capacity == 1`.
    fn quarantine_raw_inquiry_correlation(&mut self, id: RequestId, now: Instant) {
        let Some((target, ambiguity)) = self.entries.get(&id).and_then(|entry| {
            (self.policy.envelope == EnvelopeKind::Raw
                && self.policy.inquiry_capacity == 1
                && entry.request.is_inquiry()
                && matches!(
                    entry.phase,
                    Phase::Sending { .. } | Phase::AwaitingReply { .. }
                ))
            .then_some((
                entry.request.context().target,
                entry.request.context().timeout.ambiguity,
            ))
        }) else {
            return;
        };
        extend_tombstone(
            &mut self.raw_target_tombstones[target.id() as usize],
            RawTerminalTombstone::inquiry(add_duration(now, ambiguity)),
        );
    }

    fn inquiry_reply(
        &mut self,
        id: RequestId,
        route: Option<InquiryRoute>,
        payload: SmallVec<[u8; INLINE_BYTES]>,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let compatible = self.entries.get(&id).is_some_and(|entry| {
            entry.request.is_inquiry() && matches!(entry.phase, Phase::AwaitingReply { .. })
        });
        if !compatible {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        self.quarantine_raw_inquiry_correlation(id, now);
        self.finish(id, RuntimeOutcome::Reply { route, payload }, effects);
    }

    fn camera_error(
        &mut self,
        id: RequestId,
        correlation_kind: CorrelationKind,
        socket: Option<ViscaSocket>,
        code: u8,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        if let Some(socket) = socket {
            if matches!(
                entry.phase,
                Phase::Executing { socket: owned, .. }
                    | Phase::AwaitingCancellationResolution { socket: owned, .. }
                    if owned != socket
            ) {
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
        let cancellation_active = !matches!(entry.cancellation, CancelState::None);
        if code == 0x04
            && (cancellation_active || correlation_kind == CorrelationKind::Cancellation)
        {
            self.confirm_cancelled(id, effects);
            return;
        }
        // On the raw envelope a named-socket 0x05 can mean that the camera did
        // not accept a cancellation packet for that socket. Raw frames do not
        // carry an independent cancellation correlation, so preserve that
        // failed-cancellation result only while an emitted cancellation is
        // awaiting its terminal response on the request's owned socket. A
        // merely recorded, socketless cancel intent instead leaves this as the
        // original request's retryable rejection; the cancellation intent then
        // suppresses the retry below and finishes `Cancelled`.
        if self.policy.envelope == EnvelopeKind::Raw
            && correlation_kind == CorrelationKind::Request
            && code == 0x05
            && raw_cancellation_no_socket_response(entry)
        {
            self.finish(id, RuntimeOutcome::Failed(Error::NoSocket), effects);
            return;
        }
        let retryable = correlation_kind == CorrelationKind::Request
            && self.camera_error_retryable(entry, code);
        let error = Error::from_code(code);
        // A raw inquiry error releases the same target-only reply evidence as
        // a successful reply. Retain the bounded hold before a retry or a
        // terminal failure can expose a successor to that stale frame class.
        self.quarantine_raw_inquiry_correlation(id, now);
        if retryable {
            if cancellation_active {
                self.finish(id, RuntimeOutcome::Cancelled, effects);
            } else {
                self.schedule_retry(id, now, error, Backoff::Uncapped, effects);
            }
        } else {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
        }
    }

    fn camera_error_retryable(&self, entry: &Entry, code: u8) -> bool {
        let retry = entry.request.context().retry;
        match code {
            0x03 | 0x05 => retry.buffer_full,
            0x41 => retry.movement_not_executable,
            0x02 => entry.request.is_inquiry() && retry.builtin_inquiry_syntax,
            _ => false,
        }
    }

    fn confirm_cancelled(&mut self, id: RequestId, effects: &mut Vec<Effect>) {
        let active = self
            .entries
            .get(&id)
            .is_some_and(|entry| !matches!(entry.cancellation, CancelState::None));
        if !active {
            self.finish(id, RuntimeOutcome::Failed(Error::CommandCanceled), effects);
            return;
        }
        self.finish(id, RuntimeOutcome::Cancelled, effects);
    }

    fn cancel(&mut self, id: RequestId, now: Instant, effects: &mut Vec<Effect>) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        if entry.request.is_inquiry() {
            effects.push(Effect::CancellationObservation {
                id,
                observation: CancellationObservation::Failed(Error::InquiryNotCancelable),
            });
            return;
        }
        if !matches!(entry.cancellation, CancelState::None) {
            effects.push(Effect::Ignored(IgnoreReason::DuplicateCancellation));
            return;
        }
        let phase = entry.phase;
        if matches!(
            phase,
            Phase::AwaitingCancellationResolution { deadline, .. }
                | Phase::AwaitingLateAck { deadline }
                if deadline < now
        ) {
            self.expire_quarantine(id, effects);
            return;
        }
        match phase {
            Phase::Ready { .. } | Phase::Backoff { .. } => {
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.cancellation_observation_open = true;
                }
                effects.push(Effect::CancellationRecorded { id });
                self.finish(id, RuntimeOutcome::Cancelled, effects);
            }
            Phase::Sending { .. }
            | Phase::AwaitingAck { .. }
            | Phase::AwaitingCompletion { .. }
            | Phase::AwaitingLateAck { .. }
            | Phase::Executing { .. }
            | Phase::AwaitingCancellationResolution { .. }
            | Phase::AwaitingReply { .. } => {
                let cancellation = entry.request.context().cancellation;
                if cancellation == CancellationPolicy::Unsupported {
                    // Unsupported sent cancellation is deliberately not intent: the
                    // original remains retryable and able to complete.
                    effects.push(Effect::CancellationObservation {
                        id,
                        observation: CancellationObservation::Failed(Error::NotSupported),
                    });
                    return;
                }
                let ambiguity_deadline =
                    add_duration(now, entry.request.context().timeout.ambiguity);
                // A raw ambiguity quarantine already owns either a socket or
                // the sole unacknowledged-command slot. If cancellation is
                // requested while that hold is active, keep the later of the
                // two deadlines so the cancellation-driven path stays eligible
                // until its own ambiguity window closes.
                let phase = match phase {
                    Phase::AwaitingCancellationResolution { socket, deadline } => {
                        Phase::AwaitingCancellationResolution {
                            socket,
                            deadline: deadline.max(ambiguity_deadline),
                        }
                    }
                    Phase::AwaitingLateAck { deadline } => Phase::AwaitingLateAck {
                        deadline: deadline.max(ambiguity_deadline),
                    },
                    phase => phase,
                };
                let ambiguity_deadline = match phase {
                    Phase::AwaitingCancellationResolution { deadline, .. }
                    | Phase::AwaitingLateAck { deadline } => deadline,
                    _ => ambiguity_deadline,
                };
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.cancellation_observation_open = true;
                }
                self.transition(
                    id,
                    phase,
                    CancelState::Requested { ambiguity_deadline },
                    effects,
                );
                effects.push(Effect::CancellationRecorded { id });
                effects.push(Effect::CancellationObservation {
                    id,
                    observation: CancellationObservation::Recorded,
                });
                if let Phase::Executing { socket, .. }
                | Phase::AwaitingCancellationResolution { socket, .. } = phase
                {
                    self.emit_cancel(id, socket, now, effects);
                }
            }
        }
    }

    fn emit_cancel(
        &mut self,
        id: RequestId,
        socket: ViscaSocket,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        if entry.cancel_attempted_socket == Some(socket) {
            return;
        }
        if self.cancellation_send_at(entry) > now {
            return;
        }
        let ambiguity_deadline = match entry.cancellation {
            CancelState::Requested { ambiguity_deadline } => {
                ambiguity_deadline.max(add_duration(now, entry.request.context().timeout.ambiguity))
            }
            CancelState::Sending {
                ambiguity_deadline, ..
            }
            | CancelState::AwaitingTerminal {
                ambiguity_deadline, ..
            } => ambiguity_deadline,
            CancelState::None | CancelState::ObservationFailed { .. } => return,
        };
        let target = entry.request.context().target;
        let generation = entry.generation;
        let attempt = entry.attempt;
        // A cancellation can wait behind the shared command-spacing floor
        // after a completion-timeout quarantine has already begun.  The
        // cancellation ambiguity window starts when the cancel write is
        // actually emitted, so keep the socket quarantine through that later
        // deadline rather than allowing the older phase deadline to release
        // it while the write is still unresolved.
        let phase = match entry.phase {
            Phase::AwaitingCancellationResolution {
                socket: phase_socket,
                deadline,
            } => Phase::AwaitingCancellationResolution {
                socket: phase_socket,
                deadline: deadline.max(ambiguity_deadline),
            },
            phase => phase,
        };
        let Some(transmission) = self.allocate_transmission_id() else {
            self.finish(
                id,
                RuntimeOutcome::Failed(Error::RuntimeIdentityExhausted),
                effects,
            );
            return;
        };
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.cancel_attempted_socket = Some(socket);
        }
        self.transition(
            id,
            phase,
            CancelState::Sending {
                transmission,
                socket,
                ambiguity_deadline,
            },
            effects,
        );
        self.transmissions.insert(
            transmission,
            TransmissionOwner {
                request: id,
                generation,
                attempt,
                kind: CorrelationKind::Cancellation,
                // A cancellation is not a retry of the command.  It must get
                // a fresh Sony identity even when the command already has a
                // successful sequence recorded.
                requested_sequence: None,
            },
        );
        effects.push(Effect::Transmit {
            transmission,
            request: id,
            kind: Transmission::Cancel {
                target,
                socket,
                requested_sequence: None,
            },
        });
        self.last_request_sent = Some(now);
    }

    fn schedule_retry(
        &mut self,
        id: RequestId,
        now: Instant,
        error: Error,
        backoff: Backoff,
        effects: &mut Vec<Effect>,
    ) {
        let Some((cancellation, policy, submitted_at, attempt)) =
            self.entries.get(&id).map(|entry| {
                (
                    entry.cancellation,
                    entry.request.context().retry,
                    entry.submitted_at,
                    entry.attempt,
                )
            })
        else {
            return;
        };
        if !matches!(cancellation, CancelState::None) {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
            return;
        }
        // Releasing a raw inquiry FIFO owner for a retry would otherwise let a
        // delayed attempt-N reply bind to the requeued attempt or its next
        // same-target inquiry. The target tombstone is the bounded policy; it
        // intentionally cannot make unidentifiable raw traffic safe forever.
        self.quarantine_raw_inquiry_correlation(id, now);
        let next_attempt = attempt.saturating_add(1);
        let elapsed = now.saturating_duration_since(submitted_at);
        let within_duration =
            policy.total_budget == Duration::ZERO || elapsed < policy.total_budget;
        if next_attempt > policy.max_retries || !within_duration {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
            return;
        }
        let delay = retry_delay(
            policy,
            next_attempt,
            backoff,
            self.jitter.fraction(id, next_attempt),
        );
        let ready_at = add_duration(now, delay);
        if policy.total_budget != Duration::ZERO
            && ready_at >= add_duration(submitted_at, policy.total_budget)
        {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
            return;
        }
        self.release_attempt_ownership(id);
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        entry.attempt = next_attempt;
        entry.last_error = Some(error);
        // A latch belongs to exactly one attempt's write.
        entry.deferred_ack = None;
        entry.deferred_completion = None;
        entry.queue_generation = entry.queue_generation.wrapping_add(1).max(1);
        let queue_generation = entry.queue_generation;
        let cancellation = entry.cancellation;
        self.transition(
            id,
            Phase::Backoff {
                ready_at,
                queue_generation,
            },
            cancellation,
            effects,
        );
        effects.push(Effect::RetryScheduled {
            id,
            attempt: next_attempt,
            ready_at,
        });
        if self.entries.get(&id).is_some_and(|entry| {
            entry.request.is_inquiry() && matches!(entry.last_error, Some(Error::SyntaxError))
        }) {
            let cooldown = add_duration(now, self.policy.inquiry_cooldown.max(delay));
            self.inquiry_cooldown_until = Some(
                self.inquiry_cooldown_until
                    .map_or(cooldown, |existing| existing.max(cooldown)),
            );
        }
    }

    fn release_attempt_ownership(&mut self, id: RequestId) {
        for target in 1..self.socket_owners.len() {
            for socket in 0..2 {
                if self.socket_owners[target][socket].is_some_and(|owner| owner.request == id) {
                    self.socket_owners[target][socket] = None;
                }
            }
            self.raw_inquiries[target].retain(|owner| owner.request != id);
        }
        self.transmissions.retain(|_, owner| owner.request != id);
    }

    fn run_due(&mut self, now: Instant, effects: &mut Vec<Effect>) {
        // A frame delivered at the exact deadline is still processed before
        // this function by an input turn, so it is conservatively ignored by
        // the tombstone. Once the turn reaches due work, release every expired
        // fixed slot before dispatching a queued successor.
        self.raw_release_gate = None;
        self.expire_raw_terminal_tombstones(now);
        while let Some(due) = self.next_due().filter(|due| due.at <= now) {
            let valid = self.entries.get(&due.request).is_some_and(|entry| {
                entry.generation == due.generation
                    && (due.queue_generation == 0 || entry.queue_generation == due.queue_generation)
            });
            if !valid {
                effects.push(Effect::Ignored(IgnoreReason::StaleQueueTicket));
                continue;
            }
            self.apply_due(due, now, effects);
        }
        if self
            .inquiry_cooldown_until
            .is_some_and(|deadline| deadline <= now)
        {
            self.inquiry_cooldown_until = None;
        }
    }

    fn next_due(&self) -> Option<DueWork> {
        self.entries
            .iter()
            .filter_map(|(id, entry)| {
                let phase_due = match entry.phase {
                    Phase::AwaitingAck { deadline, .. } => Some((deadline, 1, 0)),
                    Phase::AwaitingCompletion { deadline, .. } => Some((deadline, 1, 0)),
                    Phase::Executing { deadline, .. } => Some((deadline, 1, 0)),
                    Phase::AwaitingReply { deadline, .. } => Some((deadline, 1, 0)),
                    Phase::Backoff {
                        ready_at,
                        queue_generation,
                    } => Some((ready_at, 2, queue_generation)),
                    Phase::AwaitingCancellationResolution { deadline, .. }
                    | Phase::AwaitingLateAck { deadline } => Some((deadline, 0, 0)),
                    Phase::Ready { .. } | Phase::Sending { .. } => None,
                };
                let retry_budget_due = retry_budget_deadline(entry);
                let mut selected = phase_due;
                if let Some(budget) = retry_budget_due {
                    if selected.is_none_or(|(at, _, _)| budget <= at) {
                        selected = Some((budget, 1, 0));
                    }
                }
                if let Some(ambiguity) = cancellation_ambiguity(entry.cancellation) {
                    if selected.is_none_or(|(at, _, _)| ambiguity <= at) {
                        selected = Some((ambiguity, 0, 0));
                    }
                }
                let (mut at, mut kind_order, mut queue_generation) = selected?;
                if entry.cancellation_observation_open {
                    if let CancelState::AwaitingTerminal {
                        observation_deadline,
                        ..
                    } = entry.cancellation
                    {
                        if observation_deadline < at {
                            at = observation_deadline;
                            kind_order = 1;
                            queue_generation = 0;
                        }
                    }
                }
                Some(DueWork {
                    at,
                    admission_order: entry.admission_order,
                    kind_order,
                    request: *id,
                    generation: entry.generation,
                    queue_generation,
                })
            })
            .min_by_key(|due| due.key())
    }

    /// Whether an unconfirmable raw command must poison the whole session
    /// rather than fail on its own.
    ///
    /// True only in the strict opt-in mode on the raw envelope (issue #671).
    /// Sony correlation never needs the quarantine, so the flag is inert there
    /// even if a caller set it.
    const fn raw_unconfirmed_poison(&self) -> bool {
        matches!(self.policy.envelope, EnvelopeKind::Raw) && self.policy.strict_unconfirmed_poison
    }

    /// Poisons the whole session for the strict opt-in mode (issue #671).
    ///
    /// A poisoned session is terminal, so the error it hands to every request —
    /// and to any later operation through the owner's boundary verdict — must
    /// answer [`Error::requires_new_session`](crate::Error::requires_new_session)
    /// with `true` so a caller rebuilds it. [`Error::UnsequencedCommandUnconfirmed`]
    /// is deliberately *not* used here: it is now the per-request survivable
    /// outcome (its `requires_new_session()` is `false`), and a live-vs-dead
    /// session must never be reported by the same verdict. [`Error::StreamPoisoned`]
    /// is the crate's session-poison error, and its recovery guidance already
    /// covers a raw command whose completion is uncertain.
    fn poison_strict_unconfirmed(&mut self, effects: &mut Vec<Effect>) {
        self.terminate_session(
            SessionState::Poisoned,
            Error::StreamPoisoned {
                reason: Cow::Borrowed(
                    "raw command outcome could not be confirmed (strict_unconfirmed_poison)",
                ),
            },
            effects,
        );
    }

    /// Terminates one raw command whose ACK or completion can no longer be
    /// confirmed (issue #671).
    ///
    /// Strict opt-in mode poisons the whole session (the pre-fix behavior). The
    /// default fails exactly this request with
    /// [`Error::UnsequencedCommandUnconfirmed`] while holding whatever
    /// correlation it still owns — its command socket, or its place as the sole
    /// unacknowledged raw command on the target — quarantined until the
    /// ambiguity deadline. A late ACK or completion then resolves to the
    /// quarantine and is ignored ([`Self::frame`]) instead of binding to a later
    /// command that reuses the socket or the unacknowledged slot. The session
    /// and every unrelated request keep running.
    ///
    /// The caller must have already established that this is a raw command with
    /// no cancellation in flight; the quarantine phases are distinguished from
    /// their cancellation-driven uses by their [`CancelState::None`].
    fn terminate_unconfirmed_raw(
        &mut self,
        id: RequestId,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        if self.raw_unconfirmed_poison() {
            self.poison_strict_unconfirmed(effects);
            return;
        }
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        let ambiguity_deadline = add_duration(now, entry.request.context().timeout.ambiguity);
        let cancellation = entry.cancellation;
        match entry.phase {
            Phase::Executing { socket, .. } => {
                // Correlation is exact — this request owns `socket`. Hold it so a
                // late completion resolves here and is ignored, never bound to a
                // later command that reuses the socket.
                self.transition(
                    id,
                    Phase::AwaitingCancellationResolution {
                        socket,
                        deadline: ambiguity_deadline,
                    },
                    cancellation,
                    effects,
                );
            }
            Phase::AwaitingAck { .. } => {
                // No socket is owned yet; hold the sole-unacknowledged-command
                // slot so a late ACK resolves here and is ignored.
                self.transition(
                    id,
                    Phase::AwaitingLateAck {
                        deadline: ambiguity_deadline,
                    },
                    cancellation,
                    effects,
                );
            }
            Phase::AwaitingCompletion { .. } => {
                // Issue #700: a completion-only command owns no socket, so — like
                // AwaitingAck — it holds the sole-command slot rather than a
                // socket. Reuse the same late-slot quarantine: it keeps the target
                // reserved so no later command can be dispatched into the
                // ambiguity window, and any late completion is dropped by the
                // resolver (it no longer matches a completion-only candidate) or
                // by the quarantine guard in `frame`.
                self.transition(
                    id,
                    Phase::AwaitingLateAck {
                        deadline: ambiguity_deadline,
                    },
                    cancellation,
                    effects,
                );
            }
            Phase::Sending { transmission, .. } => {
                // The write is still in flight. Drop its correlation so the
                // returning transmission result is inert, abandon any ACK or
                // completion that raced the write (issue #297/#700 latches — this
                // attempt is over), and hold the unacknowledged slot the same way.
                self.transmissions.remove(&transmission);
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.deferred_ack = None;
                    entry.deferred_completion = None;
                }
                self.transition(
                    id,
                    Phase::AwaitingLateAck {
                        deadline: ambiguity_deadline,
                    },
                    cancellation,
                    effects,
                );
            }
            _ => {
                // Nothing correlated is at stake (Ready/Backoff/AwaitingReply/…):
                // fail immediately.
                self.finish(
                    id,
                    RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed),
                    effects,
                );
            }
        }
    }

    fn apply_due(&mut self, due: DueWork, now: Instant, effects: &mut Vec<Effect>) {
        let Some(entry) = self.entries.get(&due.request) else {
            return;
        };
        let phase = entry.phase;
        let ambiguity_due =
            cancellation_ambiguity(entry.cancellation).is_some_and(|deadline| deadline <= now);
        if due.kind_order == 0 && ambiguity_due {
            // The ambiguity window has already closed, so nothing further needs
            // quarantining here: fail this one request (default) or poison the
            // session (strict opt-in). Issue #671.
            let error = if self.policy.envelope == EnvelopeKind::Raw {
                Error::UnsequencedCommandUnconfirmed
            } else {
                Error::CancellationUnconfirmed
            };
            if self.raw_unconfirmed_poison() {
                self.poison_strict_unconfirmed(effects);
            } else {
                self.finish(due.request, RuntimeOutcome::Failed(error), effects);
            }
            return;
        }
        let cancellation_observation_due = entry.cancellation_observation_open
            && matches!(
                entry.cancellation,
                CancelState::AwaitingTerminal {
                    observation_deadline,
                    ..
                } if observation_deadline <= now
            );
        if due.kind_order == 1 && cancellation_observation_due {
            if let Some(entry) = self.entries.get_mut(&due.request) {
                entry.cancellation_observation_open = false;
            }
            effects.push(Effect::CancellationObservation {
                id: due.request,
                observation: CancellationObservation::Failed(Error::Timeout),
            });
            return;
        }
        let retry_budget_at = retry_budget_deadline(entry);
        let retry_budget_due = retry_budget_at.is_some_and(|deadline| deadline <= now);
        if due.kind_order == 1
            && retry_budget_due
            && retry_budget_at.is_some_and(|deadline| deadline == due.at)
        {
            let error = entry.last_error.clone().unwrap_or(Error::Timeout);
            let raw_active_command = self.policy.envelope == EnvelopeKind::Raw
                && !entry.request.is_inquiry()
                && matches!(
                    phase,
                    Phase::Sending { .. }
                        | Phase::AwaitingAck { .. }
                        | Phase::AwaitingCompletion { .. }
                        | Phase::Executing { .. }
                );
            if raw_active_command {
                // The active attempt still owns correlation (socket or the
                // unacknowledged-command slot); quarantine it and fail this one
                // request, or poison in strict mode. Issue #671.
                self.terminate_unconfirmed_raw(due.request, now, effects);
            } else {
                self.quarantine_raw_inquiry_correlation(due.request, now);
                self.finish(due.request, RuntimeOutcome::Failed(error), effects);
            }
            return;
        }
        match phase {
            Phase::AwaitingAck { deadline, .. } if deadline <= now => {
                let mark = effects.len();
                if let CancelState::Requested { ambiguity_deadline } = entry.cancellation {
                    self.transition(
                        due.request,
                        Phase::AwaitingLateAck {
                            deadline: ambiguity_deadline,
                        },
                        entry.cancellation,
                        effects,
                    );
                } else if self.policy.envelope == EnvelopeKind::Raw {
                    // Default: quarantine the sole unacknowledged raw command as
                    // a late-ACK slot and fail it UnsequencedCommandUnconfirmed
                    // at the ambiguity deadline; strict: poison. Issue #671.
                    self.terminate_unconfirmed_raw(due.request, now, effects);
                } else if entry.request.context().retry.ack_timeout {
                    self.schedule_retry(
                        due.request,
                        now,
                        Error::Timeout,
                        Backoff::AckCapped,
                        effects,
                    );
                } else {
                    self.finish(due.request, RuntimeOutcome::Failed(Error::Timeout), effects);
                }
                record_deadline_expiry(due.request, DeadlineKind::Ack, mark, effects);
            }
            Phase::AwaitingCompletion { deadline, .. } if deadline <= now => {
                let mark = effects.len();
                if let CancelState::Requested { ambiguity_deadline } = entry.cancellation {
                    // Cancelled but never socketed (issue #700): hold the
                    // socketless sole-command slot until the ambiguity deadline,
                    // exactly as the AwaitingAck cancel path does.
                    self.transition(
                        due.request,
                        Phase::AwaitingLateAck {
                            deadline: ambiguity_deadline,
                        },
                        entry.cancellation,
                        effects,
                    );
                } else if self.policy.envelope == EnvelopeKind::Raw {
                    // Issue #700: the completion never arrived. This shape has no
                    // ACK-timeout path; the completion deadline governs. Quarantine
                    // the sole-command slot and fail this one request
                    // UnsequencedCommandUnconfirmed at the ambiguity deadline
                    // (default), or poison (strict) — the #671 per-request model,
                    // never a session-wide poison by default.
                    self.terminate_unconfirmed_raw(due.request, now, effects);
                } else if entry.request.context().retry.completion_timeout {
                    self.schedule_retry(
                        due.request,
                        now,
                        Error::Timeout,
                        Backoff::Uncapped,
                        effects,
                    );
                } else {
                    self.finish(due.request, RuntimeOutcome::Failed(Error::Timeout), effects);
                }
                record_deadline_expiry(due.request, DeadlineKind::Completion, mark, effects);
            }
            Phase::Executing {
                socket, deadline, ..
            } if deadline <= now => {
                let mark = effects.len();
                if !matches!(entry.cancellation, CancelState::None) {
                    if let Some(ambiguity_deadline) = cancellation_ambiguity(entry.cancellation)
                        .filter(|ambiguity_deadline| *ambiguity_deadline > now)
                    {
                        self.transition(
                            due.request,
                            Phase::AwaitingCancellationResolution {
                                socket,
                                deadline: ambiguity_deadline,
                            },
                            entry.cancellation,
                            effects,
                        );
                    } else {
                        let error = if self.policy.envelope == EnvelopeKind::Raw {
                            Error::UnsequencedCommandUnconfirmed
                        } else {
                            Error::CancellationUnconfirmed
                        };
                        if self.raw_unconfirmed_poison() {
                            self.poison_strict_unconfirmed(effects);
                        } else {
                            self.finish(due.request, RuntimeOutcome::Failed(error), effects);
                        }
                    }
                } else if self.policy.envelope == EnvelopeKind::Raw {
                    // Default: correlation is exact (this request owns the
                    // socket); hold the socket quarantined and fail this one
                    // request UnsequencedCommandUnconfirmed at the ambiguity
                    // deadline; strict: poison. Issue #671.
                    self.terminate_unconfirmed_raw(due.request, now, effects);
                } else if entry.request.context().retry.completion_timeout {
                    self.schedule_retry(
                        due.request,
                        now,
                        Error::Timeout,
                        Backoff::Uncapped,
                        effects,
                    );
                } else {
                    self.finish(due.request, RuntimeOutcome::Failed(Error::Timeout), effects);
                }
                record_deadline_expiry(due.request, DeadlineKind::Completion, mark, effects);
            }
            Phase::AwaitingReply { deadline, .. } if deadline <= now => {
                let mark = effects.len();
                let retry_inquiry_timeout = entry.request.context().retry.inquiry_timeout;
                self.quarantine_raw_inquiry_correlation(due.request, now);
                if retry_inquiry_timeout {
                    self.schedule_retry(
                        due.request,
                        now,
                        Error::Timeout,
                        Backoff::Uncapped,
                        effects,
                    );
                } else {
                    self.finish(due.request, RuntimeOutcome::Failed(Error::Timeout), effects);
                }
                record_deadline_expiry(due.request, DeadlineKind::InquiryReply, mark, effects);
            }
            Phase::Backoff {
                ready_at,
                queue_generation,
            } if ready_at <= now && queue_generation == due.queue_generation => {
                self.promote_retry(due.request, effects);
            }
            Phase::AwaitingCancellationResolution { deadline, .. }
            | Phase::AwaitingLateAck { deadline }
                if deadline <= now =>
            {
                self.expire_quarantine(due.request, effects);
            }
            _ => effects.push(Effect::Ignored(IgnoreReason::StaleQueueTicket)),
        }
    }

    /// Closes an ambiguity/quarantine hold without allowing a later cancel
    /// request to extend an already-expired correlation window.
    fn expire_quarantine(&mut self, id: RequestId, effects: &mut Vec<Effect>) {
        let error = if self.policy.envelope == EnvelopeKind::Raw {
            Error::UnsequencedCommandUnconfirmed
        } else {
            Error::CancellationUnconfirmed
        };
        if self.raw_unconfirmed_poison() {
            self.poison_strict_unconfirmed(effects);
        } else {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
        }
    }

    fn promote_retry(&mut self, id: RequestId, effects: &mut Vec<Effect>) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        let Phase::Backoff {
            queue_generation, ..
        } = entry.phase
        else {
            effects.push(Effect::Ignored(IgnoreReason::StaleQueueTicket));
            return;
        };
        let ticket = QueueTicket {
            request: id,
            generation: entry.generation,
            queue_generation,
        };
        let inquiry = entry.request.is_inquiry();
        let priority = entry.request.context().control.class.priority_index();
        let cancellation = entry.cancellation;
        self.transition(id, Phase::Ready { ticket }, cancellation, effects);
        self.queue_mut(inquiry, priority).push_back(ticket);
    }

    /// Earliest scheduler deadline, retry eligibility, pacing release, or cooldown.
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.next_wake_inner(true)
    }

    /// Raw correlation scopes which can be released by advancing at `now`.
    ///
    /// Byte-stream owners use this typed projection before a due pass.  It
    /// keeps an exact socket release distinct from target-only ambiguity, so a
    /// split S1 completion survives an S2 release on the same camera.  The
    /// matching [`Self::raw_prefix_disposition`] is the sole policy authority
    /// for the owner-side retained-prefix decision.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn raw_correlation_releases_due(&self, now: Instant) -> RawCorrelationReleaseSet {
        let mut releases = RawCorrelationReleaseSet::default();
        if self.policy.envelope != EnvelopeKind::Raw {
            return releases;
        }
        // Keep this explicit list aligned with the target-indexed tombstone
        // table: the zero slot is inert and slots one through eight represent
        // every valid `CameraId`, including broadcast.  Iterating the typed
        // IDs avoids re-validating an internal table index with `expect`.
        for (target, tombstone) in [
            CameraId::CAMERA_1,
            CameraId::CAMERA_2,
            CameraId::CAMERA_3,
            CameraId::CAMERA_4,
            CameraId::CAMERA_5,
            CameraId::CAMERA_6,
            CameraId::CAMERA_7,
            CameraId::BROADCAST,
        ]
        .into_iter()
        .zip(self.raw_target_tombstones.iter().skip(1))
        {
            let Some(tombstone) = tombstone else {
                continue;
            };
            let release = releases.for_target_mut(target);
            if tombstone
                .terminal_deadline
                .is_some_and(|deadline| deadline <= now)
            {
                release.release_terminal_all();
            }
            if tombstone
                .inquiry_deadline
                .is_some_and(|deadline| deadline <= now)
            {
                release.release_inquiry_unkeyed();
            }
        }
        for entry in self.entries.values() {
            // Ordinary response deadlines may *install* a new tombstone in the
            // due pass. They do not release existing correlation before that
            // input-first turn, so a split valid frame must remain intact here.
            let due = cancellation_ambiguity(entry.cancellation)
                .is_some_and(|deadline| deadline <= now)
                || matches!(
                    entry.phase,
                    Phase::AwaitingCancellationResolution { deadline, .. }
                        | Phase::AwaitingLateAck { deadline }
                        if deadline <= now
                );
            if !due {
                continue;
            }
            let release = releases.for_target_mut(entry.request.context().target);
            match entry.phase {
                Phase::Executing { socket, .. }
                | Phase::AwaitingCancellationResolution { socket, .. } => {
                    release.release_exact_socket(socket);
                }
                Phase::AwaitingReply { .. } => release.release_inquiry_unkeyed(),
                Phase::Sending { .. }
                | Phase::AwaitingAck { .. }
                | Phase::AwaitingCompletion { .. }
                | Phase::AwaitingLateAck { .. } => release.release_pre_ack_unkeyed(),
                Phase::Ready { .. } | Phase::Backoff { .. } => {}
            }
        }
        releases
    }

    /// Classifies retained raw stream bytes before the owner advances a due
    /// turn.  This is intentionally conservative: an incomplete source, ACK,
    /// socketless completion, or socketless error cannot establish ownership,
    /// so it never permits release plus successor dispatch.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn raw_prefix_disposition(
        &self,
        releases: RawCorrelationReleaseSet,
        evidence: RawPrefixEvidence,
    ) -> RawPrefixDisposition {
        if releases.is_empty() {
            return RawPrefixDisposition::NoRelease;
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
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn resolve_raw_release_gate(
        &mut self,
        now: Instant,
        evidence: Option<RawPrefixEvidence>,
    ) -> RawReleaseGateAction {
        let releases = self.raw_correlation_releases_due(now);
        let Some(evidence) = evidence else {
            self.raw_release_gate = None;
            return RawReleaseGateAction::Advance;
        };
        if releases.is_empty() {
            self.raw_release_gate = None;
            return RawReleaseGateAction::Advance;
        }
        match self.raw_prefix_disposition(releases, evidence) {
            RawPrefixDisposition::NoRelease | RawPrefixDisposition::ReleasePreserving => {
                self.raw_release_gate = None;
                RawReleaseGateAction::Advance
            }
            RawPrefixDisposition::Discard => {
                self.raw_release_gate = None;
                RawReleaseGateAction::DiscardFirst
            }
            RawPrefixDisposition::Defer => {
                let await_until = self
                    .raw_release_gate
                    .filter(|gate| gate.releases == releases)
                    .map_or_else(
                        || add_duration(now, self.policy.raw_release_grace),
                        |gate| gate.await_until,
                    );
                if now >= await_until {
                    self.raw_release_gate = None;
                    RawReleaseGateAction::DiscardFirst
                } else {
                    self.raw_release_gate = Some(RawReleaseGate {
                        releases,
                        await_until,
                    });
                    RawReleaseGateAction::AwaitInputUntil(await_until)
                }
            }
        }
    }

    /// Earliest wake that must be serviced while ordinary dispatch is
    /// suppressed.  The blocking pre-ACK drain still needs protocol deadlines
    /// (including retry/backoff promotion and cancellation/quarantine
    /// deadlines) and pending cancellation pacing, but a ready request on any
    /// target is deliberately not a wake: dispatching it is forbidden for the
    /// duration of that turn.  Keeping ready-queue eligibility out of this
    /// projection prevents an unrelated ready request from turning the read
    /// into a zero-timeout loop while the submitting request's ACK is pending
    /// (issue #673).
    #[cfg(feature = "blocking")]
    pub(crate) fn next_wake_without_dispatch(&self) -> Option<Instant> {
        self.next_wake_inner(false)
    }

    fn next_wake_inner(&self, include_ready: bool) -> Option<Instant> {
        if self.state != SessionState::Running {
            return None;
        }
        let mut wake = self
            .next_due()
            .map(|due| due.at)
            .into_iter()
            .chain(self.raw_terminal_tombstone_wake())
            .min();
        for entry in self.entries.values() {
            let candidate = if include_ready
                && matches!(entry.phase, Phase::Ready { .. })
                && self.capacity_available_for(entry)
            {
                Some(self.candidate_send_at(entry))
            } else if pending_cancellation_socket(entry).is_some() {
                Some(self.cancellation_send_at(entry))
            } else {
                None
            };
            if let Some(candidate) = candidate {
                wake = Some(wake.map_or(candidate, |current| current.min(candidate)));
            }
        }
        wake
    }

    /// Earliest expiry across the fixed raw target tombstone slots.
    fn raw_terminal_tombstone_wake(&self) -> Option<Instant> {
        if self.policy.envelope != EnvelopeKind::Raw {
            return None;
        }
        self.raw_target_tombstones
            .iter()
            .flatten()
            .filter_map(|tombstone| tombstone.next_deadline())
            .min()
    }

    /// Releases every terminal tombstone whose bounded ambiguity window has
    /// elapsed.  This runs before ordinary dispatch, so a queued successor can
    /// make progress in the same wake that restores the target lane.
    fn expire_raw_terminal_tombstones(&mut self, now: Instant) {
        if self.policy.envelope != EnvelopeKind::Raw {
            return;
        }
        for tombstone in &mut self.raw_target_tombstones {
            if let Some(value) = tombstone {
                value.expire_at(now);
                if !value.is_active() {
                    *tombstone = None;
                }
            }
        }
    }

    fn capacity_available_for(&self, entry: &Entry) -> bool {
        if entry.request.is_inquiry() {
            !self.raw_tombstone_blocks_dispatch(entry, entry.request.context().target)
                && self.inquiries_inflight() < self.policy.inquiry_capacity
                && !self.uncorrelated_raw_shape_blocked(entry, entry.request.context().target)
        } else {
            let target = entry.request.context().target;
            self.targets[target.id() as usize].is_some_and(|policy| {
                !self.raw_tombstone_blocks_dispatch(entry, target)
                    && !self.raw_command_unacknowledged(target)
                    && self.commands_inflight(target) < usize::from(policy.command_sockets)
                    && !self.uncorrelated_raw_shape_blocked(entry, target)
            })
        }
    }

    fn finish(&mut self, id: RequestId, outcome: RuntimeOutcome, effects: &mut Vec<Effect>) {
        let Some(entry) = self.entries.remove(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        for queue in &mut self.command_queues {
            queue.retain(|ticket| !(ticket.request == id && ticket.generation == entry.generation));
        }
        for queue in &mut self.inquiry_queues {
            queue.retain(|ticket| !(ticket.request == id && ticket.generation == entry.generation));
        }
        for target in 1..self.socket_owners.len() {
            for socket in 0..2 {
                if self.socket_owners[target][socket].is_some_and(|owner| {
                    owner.request == id && owner.generation == entry.generation
                }) {
                    self.socket_owners[target][socket] = None;
                }
            }
            self.raw_inquiries[target]
                .retain(|owner| !(owner.request == id && owner.generation == entry.generation));
        }
        self.transmissions
            .retain(|_, owner| !(owner.request == id && owner.generation == entry.generation));
        for record in &entry.sequence_history {
            self.remove_correlation_owner(record.sequence, id, entry.generation, record.kind);
        }
        if matches!(outcome, RuntimeOutcome::Applied) {
            if let Some(projection) = entry.request.applied_state() {
                effects.push(Effect::AppliedState {
                    effect: AppliedStateEffect {
                        request: id,
                        target: entry.request.context().target,
                        projection,
                    },
                });
            }
        }
        if entry.cancellation_observation_open {
            let observation = match &outcome {
                RuntimeOutcome::Applied => Some(CancellationObservation::Completed),
                RuntimeOutcome::Cancelled => Some(CancellationObservation::Cancelled),
                RuntimeOutcome::Failed(error) => {
                    Some(CancellationObservation::Failed(error.clone()))
                }
                RuntimeOutcome::Written | RuntimeOutcome::Reply { .. } => None,
            };
            if let Some(observation) = observation {
                effects.push(Effect::CancellationObservation { id, observation });
            }
        }
        effects.push(Effect::Terminal { id, outcome });
    }

    fn terminate_session(&mut self, state: SessionState, error: Error, effects: &mut Vec<Effect>) {
        if self.state != SessionState::Running {
            return;
        }
        let from = self.state;
        self.state = state;
        self.terminal_error = Some(error.clone());
        effects.push(Effect::SessionChanged { from, to: state });
        let mut active: Vec<_> = self
            .entries
            .iter()
            .map(|(id, entry)| (entry.admission_order, *id))
            .collect();
        active.sort_unstable_by_key(|(order, _)| *order);
        for (_, id) in active {
            self.finish(id, RuntimeOutcome::Failed(error.clone()), effects);
        }
        self.transmissions.clear();
        self.sequences.clear();
        self.lower_sequences.clear();
        self.socket_owners = [[None; 2]; 9];
        for queue in &mut self.raw_inquiries {
            queue.clear();
        }
        for queue in &mut self.command_queues {
            queue.clear();
        }
        for queue in &mut self.inquiry_queues {
            queue.clear();
        }
    }

    /// Audits every derived index against authoritative entries.
    pub(crate) fn assert_invariants(&self) -> Result<(), Box<str>> {
        if self.entries.len() > self.policy.capacity {
            return Err("entry capacity exceeded".into());
        }
        let mut queued = 0_usize;
        for (inquiry, queues) in [(false, &self.command_queues), (true, &self.inquiry_queues)] {
            for (priority, queue) in queues.iter().enumerate() {
                queued = queued.saturating_add(queue.len());
                for ticket in queue {
                    let Some(entry) = self.entries.get(&ticket.request) else {
                        return Err("dispatch queue refers to missing entry".into());
                    };
                    if entry.generation != ticket.generation
                        || entry.queue_generation != ticket.queue_generation
                        || entry.request.is_inquiry() != inquiry
                        || entry.request.context().control.class.priority_index() != priority
                        || !matches!(entry.phase, Phase::Ready { ticket: active } if active == *ticket)
                    {
                        return Err("dispatch queue ticket is stale or misplaced".into());
                    }
                }
            }
        }
        if queued > self.entries.len() {
            return Err("dispatch queue bound exceeded".into());
        }
        let sequence_bound = self.entries.len().saturating_mul(MAX_SEQUENCE_HISTORY);
        if self.sequences.len() > sequence_bound || self.lower_sequences.len() > sequence_bound {
            return Err("correlation index bound exceeded".into());
        }
        for (transmission, owner) in &self.transmissions {
            let Some(entry) = self.entries.get(&owner.request) else {
                return Err("transmission refers to missing entry".into());
            };
            if entry.generation != owner.generation || entry.attempt != owner.attempt {
                return Err("transmission generation or attempt is stale".into());
            }
            let compatible = match owner.kind {
                CorrelationKind::Request => {
                    matches!(entry.phase, Phase::Sending { transmission: active, .. } if active == *transmission)
                }
                CorrelationKind::Cancellation => {
                    matches!(entry.cancellation, CancelState::Sending { transmission: active, .. } if active == *transmission)
                }
            };
            if !compatible {
                return Err("transmission is incompatible with authoritative state".into());
            }
        }
        for (target, sockets) in self.socket_owners.iter().enumerate().skip(1) {
            for (index, owner) in sockets.iter().enumerate() {
                let Some(owner) = owner else {
                    continue;
                };
                let Some(entry) = self.entries.get(&owner.request) else {
                    return Err("socket owner refers to missing entry".into());
                };
                let Some(socket) = ViscaSocket::from_index(index) else {
                    return Err("socket index is invalid".into());
                };
                if entry.generation != owner.generation
                    || usize::from(entry.request.context().target.id()) != target
                    || !phase_owns_socket(entry.phase, socket)
                {
                    return Err("socket owner is stale or phase-incompatible".into());
                }
            }
        }
        for (sequence, owners) in &self.sequences {
            for owner in owners {
                let Some(entry) = self.entries.get(&owner.request) else {
                    return Err("sequence owner refers to missing entry".into());
                };
                if entry.generation != owner.generation
                    || !correlation_phase_compatible(entry, owner.kind)
                    || !entry
                        .sequence_history
                        .iter()
                        .any(|record| record.sequence == *sequence && record.kind == owner.kind)
                {
                    return Err("sequence owner is stale or phase-incompatible".into());
                }
            }
        }
        for (lower, owners) in &self.lower_sequences {
            for owner in owners {
                let Some(entry) = self.entries.get(&owner.request) else {
                    return Err("lower16 owner refers to missing entry".into());
                };
                if entry.generation != owner.generation
                    || !entry
                        .sequence_history
                        .iter()
                        .any(|record| record.sequence as u16 == *lower && record.kind == owner.kind)
                {
                    return Err("lower16 owner is stale".into());
                }
            }
        }
        for (target, queue) in self.raw_inquiries.iter().enumerate().skip(1) {
            let mut seen = SmallVec::<[RequestId; 8]>::new();
            for owner in queue {
                if seen.contains(&owner.request) {
                    return Err("raw inquiry owns more than one FIFO position".into());
                }
                seen.push(owner.request);
                let Some(entry) = self.entries.get(&owner.request) else {
                    return Err("raw inquiry FIFO refers to missing entry".into());
                };
                if entry.generation != owner.generation
                    || usize::from(entry.request.context().target.id()) != target
                    || !matches!(entry.phase, Phase::AwaitingReply { .. })
                {
                    return Err("raw inquiry FIFO owner is stale".into());
                }
            }
        }
        // Released raw-response tombstones are fixed by protocol topology: one
        // slot per target. They must never exist for a sequenced envelope.
        if self.policy.envelope != EnvelopeKind::Raw
            && self.raw_target_tombstones.iter().any(Option::is_some)
        {
            return Err("raw terminal tombstone on a sequenced session".into());
        }
        for (id, entry) in &self.entries {
            if entry.sequence_history.len() > MAX_SEQUENCE_HISTORY {
                return Err("sequence history bound exceeded".into());
            }
            if let Phase::Ready { ticket } = entry.phase {
                if entry.generation != ticket.generation
                    || entry.queue_generation != ticket.queue_generation
                {
                    return Err("ready queue ticket is stale".into());
                }
                let occurrences = self
                    .command_queues
                    .iter()
                    .chain(&self.inquiry_queues)
                    .flat_map(|queue| queue.iter())
                    .filter(|queued| **queued == ticket)
                    .count();
                if occurrences != 1 {
                    return Err("ready entry does not own exactly one queue position".into());
                }
            }
            if entry.deferred_ack.is_some() && !matches!(entry.phase, Phase::Sending { .. }) {
                return Err("deferred ACK outlived the write it raced".into());
            }
            if entry.request.context().reply_shape == ReplyShape::CompletionOnly
                && (entry.deferred_ack.is_some()
                    || matches!(
                        entry.phase,
                        Phase::Executing { .. } | Phase::AwaitingCancellationResolution { .. }
                    )
                    || matches!(
                        entry.cancellation,
                        CancelState::Sending { .. }
                            | CancelState::AwaitingTerminal { .. }
                            | CancelState::ObservationFailed { .. }
                    )
                    || entry.cancel_attempted_socket.is_some())
            {
                return Err("completion-only command has ACK/socket cancellation state".into());
            }
            if entry.deferred_completion.is_some() && !matches!(entry.phase, Phase::Sending { .. })
            {
                return Err("deferred completion outlived the write it raced".into());
            }
            if let Phase::Sending { transmission, .. } = entry.phase {
                let Some(owner) = self.transmissions.get(&transmission) else {
                    return Err("sending entry has no transmission owner".into());
                };
                if owner.request != *id
                    || owner.generation != entry.generation
                    || owner.kind != CorrelationKind::Request
                {
                    return Err("sending entry transmission owner is incompatible".into());
                }
            }
            if let CancelState::Sending { transmission, .. } = entry.cancellation {
                let Some(owner) = self.transmissions.get(&transmission) else {
                    return Err("cancellation sending entry has no transmission owner".into());
                };
                if owner.request != *id
                    || owner.generation != entry.generation
                    || owner.kind != CorrelationKind::Cancellation
                {
                    return Err("cancellation transmission owner is incompatible".into());
                }
            }
            if let Phase::Executing { socket, .. }
            | Phase::AwaitingCancellationResolution { socket, .. } = entry.phase
            {
                let target = entry.request.context().target;
                let owner = self.socket_owners[target.id() as usize][socket.as_index()];
                if !owner.is_some_and(|owner| {
                    owner.request == *id && owner.generation == entry.generation
                }) {
                    return Err("socket-owning entry is absent from socket index".into());
                }
            }
            if self.policy.envelope == EnvelopeKind::Raw
                && matches!(entry.phase, Phase::AwaitingReply { .. })
            {
                let target = entry.request.context().target;
                let occurrences = self.raw_inquiries[target.id() as usize]
                    .iter()
                    .filter(|owner| owner.request == *id && owner.generation == entry.generation)
                    .count();
                if occurrences != 1 {
                    return Err("raw inquiry entry does not own exactly one FIFO position".into());
                }
            }
            for record in &entry.sequence_history {
                let exact = self.sequences.get(&record.sequence).is_some_and(|owners| {
                    owners.iter().any(|owner| {
                        owner.request == *id
                            && owner.generation == entry.generation
                            && owner.kind == record.kind
                    })
                });
                let lower = self
                    .lower_sequences
                    .get(&(record.sequence as u16))
                    .is_some_and(|owners| {
                        owners.iter().any(|owner| {
                            owner.request == *id
                                && owner.generation == entry.generation
                                && owner.kind == record.kind
                        })
                    });
                if !exact || !lower {
                    return Err("entry sequence history is absent from correlation index".into());
                }
            }
        }
        // Issue #671 / D8: audit the raw single-candidate rule that correlation
        // safety depends on. Raw dispatch keeps at most one command per target in
        // the window where a reply is attributed positionally rather than by an
        // owned socket. Keep this audit on the same predicate as dispatch:
        // completion-only `AwaitingCompletion` is uncorrelated for its entire
        // lifetime, while the socket quarantine
        // (`AwaitingCancellationResolution` with `CancelState::None`) is excluded
        // because its correlation stays exact. See issue_542_design_review.md,
        // “Decisions from this review”, item 7, and architecture_2_0.md,
        // “Correlation before ACK is envelope-specific” and “Operational
        // invariants”.
        if self.policy.envelope == EnvelopeKind::Raw {
            let mut unacknowledged = [0_u8; 9];
            for entry in self.entries.values() {
                if !raw_unacknowledged_command_candidate(entry) {
                    continue;
                }
                let target = usize::from(entry.request.context().target.id());
                unacknowledged[target] = unacknowledged[target].saturating_add(1);
                if unacknowledged[target] > 1 {
                    return Err("more than one raw command is unacknowledged on a target".into());
                }
            }
        } else {
            // The per-request unconfirmed quarantine is a raw-only construct: a
            // non-raw session must never hold a `CancelState::None` entry in a
            // late-ACK or socket-quarantine phase.
            for entry in self.entries.values() {
                if is_unconfirmed_quarantine(entry) {
                    return Err("unconfirmed quarantine on a non-raw session".into());
                }
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
    fn seed_allocators(&mut self, request: u64, transmission: u64, generation: u64) {
        self.next_request_id = IdAllocator::seeded(request);
        self.next_transmission_id = IdAllocator::seeded(transmission);
        self.next_generation = IdAllocator::seeded(generation);
    }

    #[cfg(test)]
    fn inject_queue_ticket(&mut self, inquiry: bool, priority: usize, ticket: QueueTicket) {
        self.queue_mut(inquiry, priority).push_front(ticket);
    }
}

fn add_duration(at: Instant, duration: Duration) -> Instant {
    at.checked_add(duration).unwrap_or(at)
}

/// A raw command which has no socket identity and therefore occupies the
/// target's single positional-correlation slot.
///
/// The raw dispatch gate and its invariant audit intentionally share this
/// predicate, so extending one cannot leave the other with a divergent phase
/// list.
fn raw_unacknowledged_command_candidate(entry: &Entry) -> bool {
    !entry.request.is_inquiry()
        && matches!(
            entry.phase,
            Phase::Sending { .. }
                | Phase::AwaitingAck { .. }
                | Phase::AwaitingCompletion { .. }
                | Phase::AwaitingLateAck { .. }
        )
}

/// The one admission-relative retry-budget deadline while it governs `entry`.
///
/// Cancellation and ambiguity/quarantine phases have their own deadline and
/// intentionally suppress this arm; expiry must never shorten those holds.
fn retry_budget_deadline(entry: &Entry) -> Option<Instant> {
    let total_budget = entry.request.context().retry.total_budget;
    (total_budget != Duration::ZERO
        && matches!(entry.cancellation, CancelState::None)
        && !is_quarantine_phase(entry.phase))
    .then(|| add_duration(entry.submitted_at, total_budget))
}

/// Extends one bounded terminal tombstone scope without replacing a newer hold
/// of the same provenance for the same fixed target slot.
fn extend_tombstone(slot: &mut Option<RawTerminalTombstone>, tombstone: RawTerminalTombstone) {
    let mut merged = slot.unwrap_or(RawTerminalTombstone {
        terminal_deadline: None,
        inquiry_deadline: None,
    });
    if let Some(deadline) = tombstone.terminal_deadline {
        merged.terminal_deadline = Some(
            merged
                .terminal_deadline
                .map_or(deadline, |existing| existing.max(deadline)),
        );
    }
    if let Some(deadline) = tombstone.inquiry_deadline {
        merged.inquiry_deadline = Some(
            merged
                .inquiry_deadline
                .map_or(deadline, |existing| existing.max(deadline)),
        );
    }
    *slot = Some(merged);
}

fn cancellation_ambiguity(cancellation: CancelState) -> Option<Instant> {
    match cancellation {
        CancelState::Requested { ambiguity_deadline }
        | CancelState::Sending {
            ambiguity_deadline, ..
        }
        | CancelState::AwaitingTerminal {
            ambiguity_deadline, ..
        }
        | CancelState::ObservationFailed {
            ambiguity_deadline, ..
        } => Some(ambiguity_deadline),
        CancelState::None => None,
    }
}

/// The effective response deadline for an already-correlated entry.
///
/// A response may only mutate the phase that is currently awaiting it. The
/// cancellation ambiguity window can close that phase earlier, so it bounds the
/// same response while cancellation is active. The observer-facing cancellation
/// timeout is deliberately absent: it resolves only the observer and leaves the
/// original response correlation live through the ambiguity deadline. The active
/// admission-to-terminal budget also bounds response correlation: otherwise an
/// input-first frame just after that earlier budget could complete the request
/// before its due work runs. A `Sending` request has no protocol phase deadline,
/// but it can latch a frame that raced its write, so the applicable budget or
/// cancellation ambiguity still bounds that latch.
fn correlated_response_deadline(entry: &Entry) -> Option<Instant> {
    let phase_deadline = match entry.phase {
        Phase::AwaitingAck { deadline, .. }
        | Phase::AwaitingCompletion { deadline, .. }
        | Phase::Executing { deadline, .. }
        | Phase::AwaitingReply { deadline, .. }
        | Phase::AwaitingCancellationResolution { deadline, .. }
        | Phase::AwaitingLateAck { deadline } => Some(deadline),
        Phase::Sending { .. } => None,
        Phase::Ready { .. } | Phase::Backoff { .. } => return None,
    };
    [
        phase_deadline,
        cancellation_ambiguity(entry.cancellation),
        retry_budget_deadline(entry),
    ]
    .into_iter()
    .flatten()
    .min()
}

fn pending_cancellation_socket(entry: &Entry) -> Option<ViscaSocket> {
    let socket = match entry.phase {
        Phase::Executing { socket, .. } | Phase::AwaitingCancellationResolution { socket, .. } => {
            socket
        }
        _ => return None,
    };
    matches!(entry.cancellation, CancelState::Requested { .. }).then_some(socket)
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
    effects: &mut Vec<Effect>,
) {
    let will_retry = effects[mark..].iter().any(
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
/// 1.x capped the ACK backoff exponent at 5 — 32x the initial delay — and left
/// completion, inquiry, protocol-error and transport-fault retries uncapped
/// (`main:src/runtime/core/mod.rs`, `delay_exponent_cap`). The rewrite dropped
/// the cap; this restores it.
///
/// What it does is bound a lost-ACK retry at `initial_backoff << 5` regardless
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
/// only through an explicit `OperationalTuning::retry_timing` override that
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
/// **1.x had no jitter at all.** `RetryConfig::calculate_delay` was exactly
/// `base * 2^(attempt - 1)` with no entropy anywhere on the path, so there is
/// nothing here to restore — this is new. It exists because the rewrite's
/// `maximum_backoff` ceiling makes retries *converge*: every command that times
/// out together against one camera saturates the same ceiling and then retries
/// on the same instant, forever, which is precisely the collision a backoff is
/// supposed to break up.
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
/// The ceiling is 1.x's exponential — `initial << (attempt - 1)`, bounded by
/// the ACK exponent cap where it applies and by `maximum_backoff` always. The
/// wait is then the equal-jitter half-open band `[ceiling / 2, ceiling)`, so
/// no request ever waits *longer* than 1.x would have, the ceiling is still
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

fn add_unique_owner(owners: &mut SmallVec<[CorrelationOwner; 2]>, owner: CorrelationOwner) {
    if !owners.contains(&owner) {
        owners.push(owner);
    }
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
        CorrelationKind::Request => matches!(
            entry.phase,
            Phase::AwaitingAck { .. }
                | Phase::AwaitingCompletion { .. }
                | Phase::Executing { .. }
                | Phase::AwaitingReply { .. }
                | Phase::AwaitingCancellationResolution { .. }
                | Phase::AwaitingLateAck { .. }
                | Phase::Backoff { .. }
                | Phase::Ready { .. }
                | Phase::Sending { .. }
        ),
        CorrelationKind::Cancellation => matches!(
            entry.cancellation,
            CancelState::Sending { .. } | CancelState::AwaitingTerminal { .. }
        ),
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
        CorrelationKind::Request if entry.request.is_inquiry() => {
            matches!(entry.phase, Phase::AwaitingReply { .. })
        }
        CorrelationKind::Request => {
            matches!(
                entry.phase,
                Phase::AwaitingAck { .. }
                    | Phase::AwaitingLateAck { .. }
                    | Phase::Executing { .. }
                    | Phase::AwaitingCancellationResolution { .. }
            ) || (entry.request.context().reply_shape == ReplyShape::CompletionOnly
                && matches!(entry.phase, Phase::AwaitingCompletion { .. }))
        }
        CorrelationKind::Cancellation => matches!(
            entry.cancellation,
            CancelState::Sending { .. } | CancelState::AwaitingTerminal { .. }
        ),
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
    let phase_socket = match entry.phase {
        Phase::Executing { socket, .. } | Phase::AwaitingCancellationResolution { socket, .. } => {
            socket
        }
        Phase::Ready { .. }
        | Phase::Sending { .. }
        | Phase::AwaitingAck { .. }
        | Phase::AwaitingCompletion { .. }
        | Phase::AwaitingReply { .. }
        | Phase::Backoff { .. }
        | Phase::AwaitingLateAck { .. } => return false,
    };
    match entry.cancellation {
        CancelState::Sending { socket, .. } => socket == phase_socket,
        CancelState::AwaitingTerminal { .. } => {
            matches!(entry.phase, Phase::AwaitingCancellationResolution { .. })
        }
        CancelState::None
        | CancelState::Requested { .. }
        | CancelState::ObservationFailed { .. } => false,
    }
}

/// The other of a target's two command sockets.
const fn other_socket(socket: ViscaSocket) -> ViscaSocket {
    match socket {
        ViscaSocket::S1 => ViscaSocket::S2,
        ViscaSocket::S2 => ViscaSocket::S1,
    }
}

fn phase_owns_socket(phase: Phase, socket: ViscaSocket) -> bool {
    matches!(
        phase,
        Phase::Executing { socket: owned, .. }
            | Phase::AwaitingCancellationResolution { socket: owned, .. }
            if owned == socket
    )
}

/// Whether an entry is a per-request unconfirmed-command quarantine (issue #671).
///
/// A raw command that has failed (or will fail at its ambiguity deadline) with
/// [`Error::UnsequencedCommandUnconfirmed`] holds its correlation slot until
/// then in [`Phase::AwaitingLateAck`] (its unacknowledged-command slot) or
/// [`Phase::AwaitingCancellationResolution`] (its owned socket). These two
/// phases are otherwise driven by the cancellation path, which always carries a
/// non-[`CancelState::None`] state, so [`CancelState::None`] uniquely marks the
/// quarantine. Any late frame that resolves to such an entry must be ignored,
/// never applied, so it cannot re-open the request or be misattributed.
fn is_unconfirmed_quarantine(entry: &Entry) -> bool {
    matches!(entry.cancellation, CancelState::None) && is_quarantine_phase(entry.phase)
}

/// Whether a phase is a quarantine hold — a late-ACK slot or an owned socket
/// held until an ambiguity/quarantine deadline.
///
/// Both the cancellation path and the issue #671 per-request quarantine park an
/// entry here. Such an entry is waiting for its own quarantine deadline, so the
/// retry budget (which bounds an *active* attempt) must not re-drive it.
const fn is_quarantine_phase(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::AwaitingLateAck { .. } | Phase::AwaitingCancellationResolution { .. }
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod cancellation_regression_tests {
    use std::{sync::Arc, time::Duration};

    use super::*;

    fn camera() -> CameraId {
        CameraId::new(1).expect("test camera")
    }

    fn policy(envelope: EnvelopeKind, command_spacing: Duration) -> ProtocolPolicy {
        ProtocolPolicy {
            capacity: 16,
            envelope,
            transport: TransportKind::Datagram,
            inquiry_capacity: 8,
            command_spacing,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::ZERO,
            raw_release_grace: Duration::from_millis(100),
            strict_unconfirmed_poison: false,
        }
    }

    fn context() -> RequestContext {
        RequestContext {
            target: camera(),
            timeout: TimeoutPolicy {
                ack: Duration::from_secs(1),
                completion: Duration::from_millis(5),
                inquiry: Duration::from_secs(1),
                cancellation: Duration::from_secs(1),
                ambiguity: Duration::from_millis(50),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        }
    }

    fn retrying_buffer_full() -> RetryPolicy {
        RetryPolicy {
            max_retries: 1,
            initial_backoff: Duration::from_millis(1),
            maximum_backoff: Duration::from_millis(1),
            total_budget: Duration::from_secs(1),
            buffer_full: true,
            ..RetryPolicy::NEVER
        }
    }

    fn command_with(reply_shape: ReplyShape, retry: RetryPolicy) -> RuntimeRequest {
        let mut request_context = context();
        request_context.reply_shape = reply_shape;
        request_context.retry = retry;
        RuntimeRequest::Command {
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0xff]).expect("test wire")),
            context: request_context,
            applied_state: None,
        }
    }

    fn command() -> RuntimeRequest {
        command_with(ReplyShape::AckThenCompletion, RetryPolicy::NEVER)
    }

    fn engine(envelope: EnvelopeKind, command_spacing: Duration) -> ProtocolEngine {
        let mut engine = ProtocolEngine::new(policy(envelope, command_spacing)).expect("engine");
        engine
            .register_target(
                camera(),
                TargetPolicy {
                    command_sockets: 2,
                    cancellation: CancellationPolicy::Supported,
                },
            )
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
                    outcome: RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed),
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
            },
            now,
        );
        let id = admitted(&admitted_effects);
        let request_transmission = request_transmission(&admitted_effects);
        let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
        engine.handle(
            Input::TransmissionFinished {
                transmission: request_transmission,
                result: Ok(TransmissionMeta { sequence }),
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
        engine.advance(completion_deadline);
        let deadline = match engine.entry(id).expect("quarantine entry").phase() {
            Phase::AwaitingCancellationResolution { deadline, .. } => deadline,
            phase => panic!("expected quarantine, got {phase:?}"),
        };
        (engine, id, deadline)
    }

    #[test]
    fn pacing_delayed_cancel_extends_raw_and_sony_quarantine() {
        let start = Instant::now();
        for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
            let (mut engine, id) = start_executing(envelope, Duration::from_millis(40), start);
            // Record cancellation while the command is still executing. The
            // completion deadline then opens the quarantine before pacing
            // permits the cancellation write for both envelope kinds.
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
            assert_eq!(engine.next_wake(), Some(expected_deadline));

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
    fn overdue_quarantine_expires_before_cancel_but_equal_deadline_wins() {
        let (mut equal_engine, equal_id, equal_deadline) = quarantine();
        let equal = equal_engine.handle(Input::Cancel { id: equal_id }, equal_deadline);
        assert!(equal.iter().any(
            |effect| matches!(effect, Effect::CancellationRecorded { id } if *id == equal_id)
        ));
        assert!(!terminal_unconfirmed(&equal, equal_id));
        assert!(equal_engine.entry(equal_id).is_some());

        let (mut overdue_engine, overdue_id, overdue_deadline) = quarantine();
        let overdue = overdue_engine.handle(
            Input::Cancel { id: overdue_id },
            overdue_deadline + Duration::from_nanos(1),
        );
        assert!(terminal_unconfirmed(&overdue, overdue_id));
        assert!(!overdue.iter().any(|effect| {
            matches!(
                effect,
                Effect::CancellationRecorded { id } | Effect::Transmit {
                    request: id,
                    kind: Transmission::Cancel { .. },
                    ..
                } if *id == overdue_id
            )
        }));
        assert!(overdue_engine.entry(overdue_id).is_none());
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
            },
            now,
        );
        let id = admitted(&admitted_effects);
        engine.handle(
            Input::TransmissionFinished {
                transmission: request_transmission(&admitted_effects),
                result: Ok(TransmissionMeta { sequence: None }),
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
                    outcome: RuntimeOutcome::Cancelled,
                } if *terminal == id
            )
        }));
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Failed(Error::NoSocket),
                } if *terminal == id
            ) || matches!(effect, Effect::RetryScheduled { id: scheduled, .. } if *scheduled == id)
        }));
        engine.assert_invariants().expect("engine invariants");
    }

    #[test]
    fn raw_completion_only_requested_cancel_no_socket_is_cancelled() {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, Duration::ZERO);
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with(ReplyShape::CompletionOnly, retrying_buffer_full()),
            },
            now,
        );
        let id = admitted(&admitted_effects);
        engine.handle(
            Input::TransmissionFinished {
                transmission: request_transmission(&admitted_effects),
                result: Ok(TransmissionMeta { sequence: None }),
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
                    outcome: RuntimeOutcome::Cancelled,
                } if *terminal == id
            )
        }));
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Failed(Error::NoSocket),
                } if *terminal == id
            ) || matches!(effect, Effect::RetryScheduled { id: scheduled, .. } if *scheduled == id)
        }));
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
            },
            now,
        );
        let id = admitted(&admitted_effects);
        engine.handle(
            Input::TransmissionFinished {
                transmission: request_transmission(&admitted_effects),
                result: Ok(TransmissionMeta { sequence: None }),
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
                transmission: cancel_transmission,
                result: Ok(TransmissionMeta { sequence: None }),
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
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Failed(Error::NoSocket),
                } if *terminal == id
            )
        }));
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id: terminal,
                    outcome: RuntimeOutcome::Cancelled,
                } if *terminal == id
            )
        }));
        engine.assert_invariants().expect("engine invariants");
    }
}
