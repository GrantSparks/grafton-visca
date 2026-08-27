//! Deterministic, synchronous, runtime-neutral VISCA protocol engine.
//!
//! This is the sole authority for admitted request lifecycle, dispatch, timing,
//! retry, correlation, and protocol cancellation. It performs no I/O and knows
//! nothing about channels, executors, facade cameras, or observers.

#![allow(dead_code)] // Phase 3 will connect the crate-private owner boundary.

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
    sequence_history: SmallVec<[SequenceRecord; MAX_SEQUENCE_HISTORY]>,
    cancel_attempted_socket: Option<ViscaSocket>,
    cancellation_observation_open: bool,
}

impl Entry {
    pub(crate) const fn phase(&self) -> Phase {
        self.phase
    }

    pub(crate) const fn cancellation(&self) -> CancelState {
        self.cancellation
    }

    pub(crate) const fn attempt(&self) -> u32 {
        self.attempt
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

    fn candidate(&mut self) -> NonZeroU64 {
        let value = if self.next == 0 { 1 } else { self.next };
        self.next = value.wrapping_add(1);
        if self.next == 0 {
            self.next = 1;
        }
        // `value` is normalized above and therefore non-zero.
        match NonZeroU64::new(value) {
            Some(nonzero) => nonzero,
            None => NonZeroU64::MIN,
        }
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

/// Result of attempting one exact first dispatch without running due work.
#[derive(Debug)]
pub(crate) enum FirstDispatch {
    Effects(Vec<Effect>),
    WaitUntil(Instant),
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
    next_request_id: IdAllocator,
    next_transmission_id: IdAllocator,
    next_generation: IdAllocator,
    next_admission_order: u64,
    next_transmission_order: u64,
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
            next_request_id: IdAllocator::new(),
            next_transmission_id: IdAllocator::new(),
            next_generation: IdAllocator::new(),
            next_admission_order: 0,
            next_transmission_order: 0,
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
                Ok(())
            }
        }
    }

    pub(crate) const fn state(&self) -> SessionState {
        self.state
    }

    pub(crate) fn entry(&self, id: RequestId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    pub(crate) fn active_len(&self) -> usize {
        self.entries.len()
    }

    /// Earliest time a specific ready request may dispatch without waiting for
    /// another request to release protocol capacity. Blocking submission uses
    /// this to distinguish pacing from socket/inquiry backpressure.
    pub(crate) fn queued_dispatch_at(&self, id: RequestId) -> Option<Instant> {
        let entry = self.entries.get(&id)?;
        if !matches!(entry.phase, Phase::Ready { .. }) {
            return None;
        }
        if entry.request.is_inquiry() {
            if self.inquiries_inflight() >= self.policy.inquiry_capacity {
                return None;
            }
        } else {
            let target = entry.request.context().target;
            let policy = self.targets[target.id() as usize]?;
            if self.commands_inflight(target) >= usize::from(policy.command_sockets) {
                return None;
            }
        }
        Some(self.candidate_send_at(entry))
    }

    /// Applies one ordered external input first, then all work due at `now`.
    pub(crate) fn handle(&mut self, input: Input, now: Instant) -> Vec<Effect> {
        let turn = self.begin_input_turn(now);
        let mut effects = self.handle_in_turn(&turn, input);
        effects.extend(self.finish_input_turn(turn));
        effects
    }

    /// Starts an ordered external-input turn at one owner-sampled instant.
    ///
    /// The owner must fully drain the effects returned for one input before it
    /// applies the next input in the turn. Any identified transmission result
    /// produced by that draining is applied with [`Self::handle_in_turn`] and
    /// the same token. Once every frame in wire order is applied, the owner
    /// calls [`Self::finish_input_turn`] exactly once.
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
        effects
    }

    /// Ends an ordered input turn, then runs due work and one ordinary dispatch.
    ///
    /// External inputs at the turn timestamp therefore win over deadlines at
    /// that same timestamp, while frame and recursively produced effect order
    /// remains owner-controlled and deterministic.
    pub(crate) fn finish_input_turn(&mut self, turn: InputTurn) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.run_due(turn.now, &mut effects);
        self.dispatch_one(turn.now, &mut effects);
        effects
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

    /// Runs only due internal work and ordinary dispatch.
    pub(crate) fn advance(&mut self, now: Instant) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.run_due(now, &mut effects);
        self.dispatch_one(now, &mut effects);
        effects
    }

    /// Admits one request without running due work or ordinary dispatch.
    pub(crate) fn admit_without_due(
        &mut self,
        ticket: AdmissionTicket,
        request: RuntimeRequest,
        now: Instant,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.admit(ticket, request, now, &mut effects);
        effects
    }

    /// Applies one identified write result without due work or dispatch.
    pub(crate) fn finish_write_without_due(
        &mut self,
        transmission: TransmissionId,
        result: Result<TransmissionMeta, Error>,
        now: Instant,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.transmission_finished(transmission, result, now, &mut effects);
        effects
    }

    /// Dispatches `id` only when it is the normative global scheduler winner.
    /// No queue, peer request, deadline, or pacing state is mutated when a
    /// different request would win: [`FirstDispatch::Blocked`] leaves `id`
    /// queued so an ordinary later turn can dispatch it once capacity frees.
    pub(crate) fn first_dispatch_without_due(
        &mut self,
        id: RequestId,
        now: Instant,
    ) -> FirstDispatch {
        if self.state != SessionState::Running {
            return FirstDispatch::Missing;
        }
        let Some(entry) = self.entries.get(&id) else {
            return FirstDispatch::Missing;
        };
        if !matches!(entry.phase, Phase::Ready { .. }) {
            return FirstDispatch::Missing;
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
            None if ready_at > now => FirstDispatch::WaitUntil(ready_at),
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
                sequence_history: SmallVec::new(),
                cancel_attempted_socket: None,
                cancellation_observation_open: false,
            },
        );
        self.queue_mut(inquiry, priority).push_back(queue_ticket);
        effects.push(Effect::Admitted { ticket, id });
    }

    fn allocate_request_id(&mut self) -> Option<RequestId> {
        for _ in 0..=self.entries.len() {
            let candidate = RequestId::from_nonzero(self.next_request_id.candidate());
            if !self.entries.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn allocate_transmission_id(&mut self) -> Option<TransmissionId> {
        for _ in 0..=self.transmissions.len() {
            let candidate = TransmissionId::from_nonzero(self.next_transmission_id.candidate());
            if !self.transmissions.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn allocate_generation(&mut self) -> Option<GenerationTicket> {
        for _ in 0..=self.entries.len() {
            let candidate = GenerationTicket(self.next_generation.candidate().get());
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

    fn eligible_ticket(
        &mut self,
        lane: Lane,
        priority: usize,
        now: Instant,
    ) -> Option<(usize, QueueTicket)> {
        self.prune_queue(lane, priority);
        let len = match lane {
            Lane::Command => self.command_queues[priority].len(),
            Lane::Inquiry => self.inquiry_queues[priority].len(),
        };
        (0..len).find_map(|index| {
            let ticket = match lane {
                Lane::Command => self.command_queues[priority].get(index).copied(),
                Lane::Inquiry => self.inquiry_queues[priority].get(index).copied(),
            }?;
            self.dispatch_eligible(ticket, now)
                .then_some((index, ticket))
        })
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
                (Some((index, ticket)), _) => Some(DispatchSelection {
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
        let wire = Arc::clone(entry.request.wire());
        let phase = Phase::Sending {
            transmission,
            started_at: now,
        };
        self.transition(ticket.request, phase, entry.cancellation, effects);
        self.transmissions.insert(
            transmission,
            TransmissionOwner {
                request: ticket.request,
                generation,
                attempt,
                kind: CorrelationKind::Request,
            },
        );
        self.last_request_sent = Some(now);
        if lane == Lane::Inquiry {
            self.last_inquiry_sent = Some(now);
        }
        effects.push(Effect::Transmit {
            transmission,
            request: ticket.request,
            kind: Transmission::Request { target, wire },
        });
    }

    fn dispatch_eligible(&self, ticket: QueueTicket, now: Instant) -> bool {
        let Some(entry) = self.entries.get(&ticket.request) else {
            return false;
        };
        if entry.request.is_inquiry() {
            if self.inquiries_inflight() >= self.policy.inquiry_capacity {
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
            if self.commands_inflight(target) >= usize::from(policy.command_sockets) {
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
                            | Phase::Executing { .. }
                            | Phase::AwaitingCancellationResolution { .. }
                            | Phase::AwaitingLateAck { .. }
                    )
            })
            .count()
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
        let Some(owner) = self.transmissions.remove(&transmission) else {
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
            effects.push(Effect::Ignored(
                IgnoreReason::IncompatibleTransmissionResult,
            ));
            return;
        }
        match result {
            Ok(meta) => self.successful_transmission(owner, meta, now, effects),
            Err(error) => self.failed_transmission(owner, error, effects),
        }
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
        if !metadata_valid {
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

    fn failed_transmission(
        &mut self,
        owner: TransmissionOwner,
        error: Error,
        effects: &mut Vec<Effect>,
    ) {
        if self.policy.transport == TransportKind::Stream {
            let reason = error.to_string();
            self.terminate_session(
                SessionState::Poisoned,
                Error::StreamPoisoned {
                    reason: Cow::Owned(reason),
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
        match frame.response {
            DecodedResponse::Ack { socket } => self.ack(id, socket, now, effects),
            DecodedResponse::Completion { socket } => {
                if correlation_kind == CorrelationKind::Cancellation {
                    self.confirm_cancelled(id, effects);
                } else {
                    self.completion(id, socket, effects);
                }
            }
            DecodedResponse::InquiryReply { route, payload } => {
                if correlation_kind == CorrelationKind::Cancellation {
                    effects.push(Effect::Ignored(IgnoreReason::MalformedFrame));
                } else {
                    self.inquiry_reply(id, route, payload, effects);
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

    fn resolve_raw(&self, frame: &DecodedFrame) -> Option<RequestId> {
        let target = frame.target;
        match &frame.response {
            DecodedResponse::Ack { .. } => self.oldest_entry(|entry| {
                !entry.request.is_inquiry()
                    && entry.request.context().target == target
                    && matches!(
                        entry.phase,
                        Phase::AwaitingAck { .. } | Phase::AwaitingLateAck { .. }
                    )
            }),
            DecodedResponse::Completion { socket } => self.socket_owner(target, *socket),
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
                    if let Some(owner) = self.socket_owner(target, *socket) {
                        return Some(owner);
                    }
                }
                self.raw_inquiry_front(target).or_else(|| {
                    self.newest_entry(|entry| {
                        !entry.request.is_inquiry()
                            && entry.request.context().target == target
                            && matches!(
                                entry.phase,
                                Phase::AwaitingAck { .. }
                                    | Phase::AwaitingLateAck { .. }
                                    | Phase::Executing { .. }
                                    | Phase::AwaitingCancellationResolution { .. }
                            )
                    })
                })
            }
            DecodedResponse::NetworkChange | DecodedResponse::Unknown => None,
        }
    }

    fn oldest_entry(&self, predicate: impl Fn(&Entry) -> bool) -> Option<RequestId> {
        self.entries
            .iter()
            .filter(|(_, entry)| predicate(entry))
            .min_by_key(|(_, entry)| entry.transmission_order.unwrap_or(u64::MAX))
            .map(|(id, _)| *id)
    }

    fn newest_entry(&self, predicate: impl Fn(&Entry) -> bool) -> Option<RequestId> {
        self.entries
            .iter()
            .filter(|(_, entry)| predicate(entry))
            .max_by_key(|(_, entry)| entry.transmission_order)
            .map(|(id, _)| *id)
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

    fn ack(&mut self, id: RequestId, socket: ViscaSocket, now: Instant, effects: &mut Vec<Effect>) {
        let Some(entry) = self.entries.get(&id) else {
            effects.push(Effect::Ignored(IgnoreReason::UnknownRequest));
            return;
        };
        if entry.request.is_inquiry()
            || !matches!(
                entry.phase,
                Phase::AwaitingAck { .. } | Phase::AwaitingLateAck { .. }
            )
        {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        let target = entry.request.context().target;
        if self
            .socket_owner(target, socket)
            .is_some_and(|owner| owner != id)
        {
            effects.push(Effect::Ignored(IgnoreReason::SocketConflict));
            return;
        }
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

    fn completion(&mut self, id: RequestId, socket: ViscaSocket, effects: &mut Vec<Effect>) {
        let compatible = self.entries.get(&id).is_some_and(|entry| {
            !entry.request.is_inquiry()
                && match entry.phase {
                    Phase::Executing { socket: owned, .. }
                    | Phase::AwaitingCancellationResolution { socket: owned, .. } => {
                        owned == socket
                    }
                    // A Sony exact completion can legitimately beat or replace an ACK.
                    Phase::AwaitingAck { .. } | Phase::AwaitingLateAck { .. } => {
                        self.policy.envelope == EnvelopeKind::Sony
                    }
                    _ => false,
                }
        });
        if !compatible {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
        self.finish(id, RuntimeOutcome::Applied, effects);
    }

    fn inquiry_reply(
        &mut self,
        id: RequestId,
        route: Option<InquiryRoute>,
        payload: SmallVec<[u8; INLINE_BYTES]>,
        effects: &mut Vec<Effect>,
    ) {
        let compatible = self.entries.get(&id).is_some_and(|entry| {
            entry.request.is_inquiry() && matches!(entry.phase, Phase::AwaitingReply { .. })
        });
        if !compatible {
            effects.push(Effect::Ignored(IgnoreReason::UnmatchedFrame));
            return;
        }
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
        let cancellation_active = !matches!(entry.cancellation, CancelState::None);
        if code == 0x04
            && (cancellation_active || correlation_kind == CorrelationKind::Cancellation)
        {
            self.confirm_cancelled(id, effects);
            return;
        }
        let retryable = correlation_kind == CorrelationKind::Request
            && self.camera_error_retryable(entry, code);
        let error = Error::from_code(code);
        if retryable {
            if cancellation_active {
                self.finish(id, RuntimeOutcome::Cancelled, effects);
            } else {
                self.schedule_retry(id, now, error, effects);
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
                if let Phase::Executing { socket, .. } = phase {
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
        let phase = entry.phase;
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
            },
        );
        effects.push(Effect::Transmit {
            transmission,
            request: id,
            kind: Transmission::Cancel { target, socket },
        });
    }

    fn schedule_retry(
        &mut self,
        id: RequestId,
        now: Instant,
        error: Error,
        effects: &mut Vec<Effect>,
    ) {
        let Some(entry) = self.entries.get(&id) else {
            return;
        };
        if !matches!(entry.cancellation, CancelState::None) {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
            return;
        }
        let policy = entry.request.context().retry;
        let submitted_at = entry.submitted_at;
        let next_attempt = entry.attempt.saturating_add(1);
        let elapsed = now.saturating_duration_since(entry.submitted_at);
        let within_duration =
            policy.total_budget == Duration::ZERO || elapsed < policy.total_budget;
        if next_attempt > policy.max_retries || !within_duration {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
            return;
        }
        self.release_attempt_ownership(id);
        let delay = retry_delay(policy, next_attempt);
        let ready_at = add_duration(now, delay);
        if policy.total_budget != Duration::ZERO
            && ready_at >= add_duration(submitted_at, policy.total_budget)
        {
            self.finish(id, RuntimeOutcome::Failed(error), effects);
            return;
        }
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        entry.attempt = next_attempt;
        entry.last_error = Some(error);
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
                let retry_budget = entry.request.context().retry.total_budget;
                let retry_budget_due = (entry.attempt > 0
                    && retry_budget != Duration::ZERO
                    && matches!(entry.phase, Phase::Ready { .. } | Phase::Backoff { .. }))
                .then(|| add_duration(entry.submitted_at, retry_budget));
                let mut selected = phase_due;
                if let Some(budget) = retry_budget_due {
                    if selected.is_none_or(|(at, _, _)| budget <= at) {
                        selected = Some((budget, 1, 0));
                    }
                }
                let (mut at, mut kind_order, mut queue_generation) = selected?;
                if let Some(ambiguity) = cancellation_ambiguity(entry.cancellation) {
                    if ambiguity <= at {
                        at = ambiguity;
                        kind_order = 0;
                        queue_generation = 0;
                    }
                }
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

    fn apply_due(&mut self, due: DueWork, now: Instant, effects: &mut Vec<Effect>) {
        let Some(entry) = self.entries.get(&due.request) else {
            return;
        };
        let phase = entry.phase;
        let ambiguity_due =
            cancellation_ambiguity(entry.cancellation).is_some_and(|deadline| deadline <= now);
        if due.kind_order == 0 && ambiguity_due {
            self.finish(
                due.request,
                RuntimeOutcome::Failed(Error::CancellationUnconfirmed),
                effects,
            );
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
        let retry = entry.request.context().retry;
        let retry_budget_due = entry.attempt > 0
            && retry.total_budget != Duration::ZERO
            && matches!(phase, Phase::Ready { .. } | Phase::Backoff { .. })
            && add_duration(entry.submitted_at, retry.total_budget) <= now;
        if due.kind_order == 1 && retry_budget_due {
            let error = entry.last_error.clone().unwrap_or(Error::Timeout);
            self.finish(due.request, RuntimeOutcome::Failed(error), effects);
            return;
        }
        match phase {
            Phase::AwaitingAck { deadline, .. } if deadline <= now => {
                if let CancelState::Requested { ambiguity_deadline } = entry.cancellation {
                    self.transition(
                        due.request,
                        Phase::AwaitingLateAck {
                            deadline: ambiguity_deadline,
                        },
                        entry.cancellation,
                        effects,
                    );
                } else if entry.request.context().retry.ack_timeout {
                    self.schedule_retry(due.request, now, Error::Timeout, effects);
                } else {
                    self.finish(due.request, RuntimeOutcome::Failed(Error::Timeout), effects);
                }
            }
            Phase::Executing {
                socket, deadline, ..
            } if deadline <= now => {
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
                        self.finish(
                            due.request,
                            RuntimeOutcome::Failed(Error::CancellationUnconfirmed),
                            effects,
                        );
                    }
                } else if entry.request.context().retry.completion_timeout {
                    self.schedule_retry(due.request, now, Error::Timeout, effects);
                } else {
                    self.finish(due.request, RuntimeOutcome::Failed(Error::Timeout), effects);
                }
            }
            Phase::AwaitingReply { deadline, .. } if deadline <= now => {
                if entry.request.context().retry.inquiry_timeout {
                    self.schedule_retry(due.request, now, Error::Timeout, effects);
                } else {
                    self.finish(due.request, RuntimeOutcome::Failed(Error::Timeout), effects);
                }
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
                self.finish(
                    due.request,
                    RuntimeOutcome::Failed(Error::CancellationUnconfirmed),
                    effects,
                );
            }
            _ => effects.push(Effect::Ignored(IgnoreReason::StaleQueueTicket)),
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
        if self.state != SessionState::Running {
            return None;
        }
        let mut wake = self.next_due().map(|due| due.at);
        for entry in self.entries.values().filter(|entry| {
            matches!(entry.phase, Phase::Ready { .. }) && self.capacity_available_for(entry)
        }) {
            let candidate = self.candidate_send_at(entry);
            wake = Some(wake.map_or(candidate, |current| current.min(candidate)));
        }
        wake
    }

    fn capacity_available_for(&self, entry: &Entry) -> bool {
        if entry.request.is_inquiry() {
            self.inquiries_inflight() < self.policy.inquiry_capacity
        } else {
            let target = entry.request.context().target;
            self.targets[target.id() as usize].is_some_and(|policy| {
                self.commands_inflight(target) < usize::from(policy.command_sockets)
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
                RuntimeOutcome::Reply { .. } => None,
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
        Ok(())
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

fn retry_delay(policy: RetryPolicy, attempt: u32) -> Duration {
    if policy.initial_backoff == Duration::ZERO {
        return Duration::ZERO;
    }
    let exponent = attempt.saturating_sub(1).min(31);
    let multiplier = 1_u32 << exponent;
    policy
        .initial_backoff
        .checked_mul(multiplier)
        .unwrap_or(policy.maximum_backoff)
        .min(policy.maximum_backoff)
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
        if !unique
            .iter()
            .any(|existing: &CorrelationOwner| existing.request == owner.request)
        {
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

fn phase_owns_socket(phase: Phase, socket: ViscaSocket) -> bool {
    matches!(
        phase,
        Phase::Executing { socket: owned, .. }
            | Phase::AwaitingCancellationResolution { socket: owned, .. }
            if owned == socket
    )
}
