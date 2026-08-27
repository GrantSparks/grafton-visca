//! Serialized owners for the deterministic protocol engine.
//!
//! The engine is the only lifecycle-policy authority.  This module owns the
//! bounded resources around it (admission, observers, caches, diagnostics and
//! reusable I/O storage) and exposes small mode-native driver seams.

#![allow(dead_code)] // Connected to the public operation facades in phases 4/5.

mod adapter;
#[cfg(feature = "async")]
mod async_actor;
#[cfg(feature = "async")]
mod async_transport;
#[cfg(feature = "blocking")]
mod blocking;
#[cfg(feature = "blocking")]
mod blocking_transport;

#[cfg(feature = "async")]
#[allow(unused_imports)]
pub(crate) use async_actor::*;
#[cfg(feature = "async")]
#[allow(unused_imports)]
pub(crate) use async_transport::*;
#[cfg(feature = "blocking")]
#[allow(unused_imports)]
pub(crate) use blocking::*;
#[cfg(feature = "blocking")]
#[allow(unused_imports)]
pub(crate) use blocking_transport::*;

#[allow(unused_imports)]
pub(crate) use adapter::{
    owner_policy_for, owner_policy_for_targets, owner_policy_for_targets_with_tuning,
    profile_supports_transport, validate_profile_transport, OwnerEnvelope, RoutingState,
    TargetRegistry,
};

use std::{
    array,
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use bytes::BytesMut;

use crate::command::semantics::WriteOnlyState;
use crate::{raw::MAX_BYTES, CameraId, CancellationOutcome, Error, ErrorKind, ViscaSocket};

use super::engine::{
    AdmissionTicket, AppliedStateEffect, AppliedStateProjection, CancelState,
    CancellationObservation, CancellationPolicy, ControlClass, DeadlineKind, DecodedFrame,
    DecodedResponse, Effect, EnvelopeKind, EnvelopeSequence, FirstDispatch, IgnoreReason, Input,
    InputTurn, Phase, ProtocolEngine, ProtocolPolicy, RequestId, RetryPolicy, RuntimeOutcome,
    RuntimeRequest, SessionState, ShutdownReason, TargetPolicy, TimeoutPolicy, Transmission,
    TransmissionId, TransmissionMeta,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanonicalOwnerStep {
    Admitted,
    Sending,
    RequestWrite,
    AwaitingAck,
    Ack,
    Executing,
    Completion,
    Applied,
}

#[cfg(test)]
pub(crate) const CANONICAL_OWNER_TRACE: &[CanonicalOwnerStep] = &[
    CanonicalOwnerStep::Admitted,
    CanonicalOwnerStep::Sending,
    CanonicalOwnerStep::RequestWrite,
    CanonicalOwnerStep::AwaitingAck,
    CanonicalOwnerStep::Ack,
    CanonicalOwnerStep::Executing,
    CanonicalOwnerStep::Completion,
    CanonicalOwnerStep::Applied,
];

#[cfg(test)]
pub(crate) fn canonical_owner_trace(
    diagnostics: impl IntoIterator<Item = DiagnosticEvent>,
) -> Vec<CanonicalOwnerStep> {
    diagnostics
        .into_iter()
        .filter_map(|event| match event {
            DiagnosticEvent::Admitted { .. } => Some(CanonicalOwnerStep::Admitted),
            DiagnosticEvent::Transition {
                to: Phase::Sending { .. },
                ..
            } => Some(CanonicalOwnerStep::Sending),
            DiagnosticEvent::WriteFinished {
                cancellation: false,
                ..
            } => Some(CanonicalOwnerStep::RequestWrite),
            DiagnosticEvent::Transition {
                to: Phase::AwaitingAck { .. },
                ..
            } => Some(CanonicalOwnerStep::AwaitingAck),
            DiagnosticEvent::FrameReceived {
                response: ResponseDiagnostic::Ack(_),
                ..
            } => Some(CanonicalOwnerStep::Ack),
            DiagnosticEvent::Transition {
                to: Phase::Executing { .. },
                ..
            } => Some(CanonicalOwnerStep::Executing),
            DiagnosticEvent::FrameReceived {
                response: ResponseDiagnostic::Completion(_),
                ..
            } => Some(CanonicalOwnerStep::Completion),
            DiagnosticEvent::Terminal {
                outcome: OutcomeDiagnostic::Applied,
                ..
            } => Some(CanonicalOwnerStep::Applied),
            _ => None,
        })
        .collect()
}

/// Bounds for every owner-side collection and reusable byte store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OwnerLimits {
    pub(crate) diagnostics: usize,
    pub(crate) diagnostic_subscribers: usize,
    pub(crate) diagnostic_events_per_subscription: usize,
    pub(crate) state_keys_per_target: usize,
    pub(crate) applied_subscribers: usize,
    pub(crate) applied_events_per_subscription: usize,
    pub(crate) frames_per_receive: usize,
    pub(crate) receive_bytes: usize,
    pub(crate) framing_bytes: usize,
}

impl Default for OwnerLimits {
    fn default() -> Self {
        Self {
            diagnostics: 128,
            diagnostic_subscribers: 4,
            diagnostic_events_per_subscription: 128,
            state_keys_per_target: 64,
            applied_subscribers: 16,
            applied_events_per_subscription: 64,
            frames_per_receive: 64,
            receive_bytes: 4_096,
            framing_bytes: 8_192,
        }
    }
}

/// Profile-derived pacing and socket facts *before* any operational tuning was
/// folded in.
///
/// Runtime reconfiguration (#631) re-derives the owner's live pacing and socket
/// capacity from this baseline rather than from the already-tuned values.
/// Deriving from the tuned values would be a ratchet: an override that widened
/// command pacing could never be relaxed again, because the widened value would
/// have become the new floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TuningBaseline {
    pub(crate) command_spacing: Duration,
    pub(crate) inquiry_spacing: Duration,
    pub(crate) command_sockets: [Option<u8>; 9],
}

/// Construction policy. Target registration cannot change after the owner
/// begins accepting work; pacing, socket capacity, and the session tuning that
/// governs request preparation are reconfigurable through the owner's control
/// boundary.
#[derive(Debug, Clone)]
pub(crate) struct OwnerPolicy {
    pub(crate) protocol: ProtocolPolicy,
    pub(crate) targets: [Option<TargetPolicy>; 9],
    pub(crate) limits: OwnerLimits,
    pub(crate) tuning: crate::OperationalTuning,
    pub(crate) baseline: TuningBaseline,
}

impl OwnerPolicy {
    /// Creates one immutable policy from target-local protocol facts.
    pub(crate) fn with_targets(
        protocol: ProtocolPolicy,
        targets: [Option<TargetPolicy>; 9],
    ) -> Result<Self, Error> {
        if targets[0].is_some() {
            return Err(Error::InvalidRequest(
                "owner target registry cannot contain camera zero".into(),
            ));
        }
        if targets[8].is_some() {
            return Err(Error::InvalidRequest(
                "owner target registry cannot contain broadcast".into(),
            ));
        }
        if targets[1..8].iter().all(Option::is_none) {
            return Err(Error::InvalidRequest(
                "owner target registry must contain at least one camera".into(),
            ));
        }
        Ok(Self {
            protocol,
            targets,
            limits: OwnerLimits::default(),
            tuning: crate::OperationalTuning::new(),
            baseline: TuningBaseline {
                command_spacing: protocol.command_spacing,
                inquiry_spacing: protocol.inquiry_spacing,
                command_sockets: array::from_fn(|index| {
                    targets[index].map(|policy| policy.command_sockets)
                }),
            },
        })
    }

    pub(crate) fn single_target(
        protocol: ProtocolPolicy,
        target: CameraId,
        target_policy: TargetPolicy,
    ) -> Result<Self, Error> {
        if !(1..=7).contains(&target.id()) {
            return Err(Error::InvalidRequest(
                "owner target must be an individual camera (1 through 7)".into(),
            ));
        }
        let mut targets = [None; 9];
        targets[usize::from(target.id())] = Some(target_policy);
        Self::with_targets(protocol, targets)
    }

    /// Lower validated profile and transport facts into one immutable owner
    /// policy. Request-specific semantics are supplied by prepared requests;
    /// this method only maps session-wide protocol and resource facts.
    pub(crate) fn from_profile(
        profile: &crate::ProfileSpec,
        config: &crate::transport::builder::TransportConfig,
        target: CameraId,
        semantics: crate::transport::SendSemantics,
    ) -> Result<Self, Error> {
        adapter::owner_policy_for(profile, config, target, semantics)
    }

    /// Lower one profile and immutable operational tuning into owner policy.
    pub(crate) fn from_profile_with_tuning(
        profile: &crate::ProfileSpec,
        config: &crate::transport::builder::TransportConfig,
        target: CameraId,
        semantics: crate::transport::SendSemantics,
        tuning: crate::OperationalTuning,
    ) -> Result<Self, Error> {
        adapter::owner_policy_for_with_tuning(profile, config, target, semantics, tuning)
    }

    /// Lowers one policy for several immutable target/profile registrations.
    pub(crate) fn from_profiles(
        profiles: &[(CameraId, &crate::ProfileSpec)],
        config: &crate::transport::builder::TransportConfig,
        semantics: crate::transport::SendSemantics,
    ) -> Result<Self, Error> {
        adapter::owner_policy_for_targets(profiles, config, semantics)
    }

    /// Tuning-aware multi-target policy lowering.
    pub(crate) fn from_profiles_with_tuning(
        profiles: &[(CameraId, &crate::ProfileSpec)],
        config: &crate::transport::builder::TransportConfig,
        semantics: crate::transport::SendSemantics,
        tuning: crate::OperationalTuning,
    ) -> Result<Self, Error> {
        adapter::owner_policy_for_targets_with_tuning(profiles, config, semantics, tuning)
    }
}

/// Bounded counters designed to be projected into the later public metrics API.
///
/// Every field is a scalar the owner increments in place, so the whole struct
/// stays `Copy` and observing it can never allocate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OwnerMetrics {
    pub(crate) admitted: u64,
    pub(crate) admission_rejected: u64,
    pub(crate) writes: u64,
    pub(crate) write_failures: u64,
    pub(crate) terminal: u64,
    pub(crate) cancellations: u64,
    pub(crate) cache_updates: u64,
    pub(crate) ack_timeouts: u64,
    pub(crate) completion_timeouts: u64,
    pub(crate) inquiry_timeouts: u64,
    pub(crate) busy_errors: u64,
    pub(crate) protocol_errors: u64,
    pub(crate) retries_scheduled: u64,
    pub(crate) ignored_unmatched_sequenced_replies: u64,
    pub(crate) dropped_diagnostics: u64,
    pub(crate) dropped_diagnostic_events: u64,
    pub(crate) dropped_observer_events: u64,
    pub(crate) dropped_applied_events: u64,
    pub(crate) dropped_boundary_work: u64,
}

/// The camera reported that it cannot accept this request right now.
///
/// These are the codes the engine's own retry classification treats as
/// transient camera-side backpressure: command buffer full (`0x03`), no socket
/// available (`0x05`), and not executable in the current state (`0x41`).
///
/// 1.x bucketed `0x03 | 0x04` here instead. `0x04` is the cancellation reply,
/// which this engine consumes as the confirmation of a cancel rather than as a
/// failure, so counting it as a busy error would make every successful
/// cancellation look like camera backpressure.
const fn error_code_is_busy(code: u8) -> bool {
    matches!(code, 0x03 | 0x05 | 0x41)
}

/// The cancellation reply code, which is an answer rather than a fault.
const CANCELLATION_REPLY_CODE: u8 = 0x04;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestLane {
    Command,
    Inquiry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseDiagnostic {
    Ack(Option<ViscaSocket>),
    Completion(Option<ViscaSocket>),
    InquiryReply,
    Error {
        socket: Option<ViscaSocket>,
        code: u8,
    },
    NetworkChange,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutcomeDiagnostic {
    Applied,
    Reply,
    Cancelled,
    Failed(ErrorKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancellationDiagnostic {
    Recorded,
    Cancelled,
    Completed,
    Failed(ErrorKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestSummary {
    target: CameraId,
    lane: RequestLane,
    timeout: TimeoutPolicy,
    retry: RetryPolicy,
    control: ControlClass,
    cancellation: CancellationPolicy,
}

impl RequestSummary {
    fn new(request: &RuntimeRequest) -> Self {
        let context = request.context();
        Self {
            target: context.target,
            lane: if request.is_inquiry() {
                RequestLane::Inquiry
            } else {
                RequestLane::Command
            },
            timeout: context.timeout,
            retry: context.retry,
            control: context.control.class,
            cancellation: context.cancellation,
        }
    }
}

/// Compact, bounded diagnostic events. Wire payloads and arbitrary strings are
/// deliberately excluded so diagnostics cannot become an unbounded byte sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticEvent {
    Admitted {
        id: RequestId,
        target: CameraId,
        lane: RequestLane,
        timeout: TimeoutPolicy,
        retry: RetryPolicy,
        control: ControlClass,
        cancellation: CancellationPolicy,
    },
    AdmissionRejected {
        target: CameraId,
        lane: RequestLane,
        error: ErrorKind,
    },
    FrameReceived {
        target: CameraId,
        sequence: Option<EnvelopeSequence>,
        response: ResponseDiagnostic,
    },
    Transition {
        id: RequestId,
        target: CameraId,
        from: Phase,
        to: Phase,
        cancellation: CancelState,
    },
    WriteFinished {
        id: RequestId,
        transmission: TransmissionId,
        target: CameraId,
        cancellation: bool,
        envelope: EnvelopeKind,
        sequence: Option<u32>,
        success: bool,
    },
    RetryScheduled {
        id: RequestId,
        target: CameraId,
        attempt: u32,
        ready_at: Instant,
    },
    /// One of a request's own protocol deadlines expired, carrying the engine's
    /// actual retry decision so a subscriber never has to infer it from a
    /// `Transition` and the absence of a `RetryScheduled`.
    DeadlineExpired {
        id: RequestId,
        target: CameraId,
        deadline: DeadlineKind,
        will_retry: bool,
    },
    CancellationRecorded {
        id: RequestId,
        target: CameraId,
    },
    CancellationObserved {
        id: RequestId,
        target: CameraId,
        observation: CancellationDiagnostic,
    },
    AppliedState {
        id: RequestId,
        target: CameraId,
        key: WriteOnlyState,
    },
    Terminal {
        id: RequestId,
        target: CameraId,
        outcome: OutcomeDiagnostic,
    },
    SessionChanged {
        from: SessionState,
        to: SessionState,
        reason: ErrorKind,
    },
    Ignored(IgnoreReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedProjection {
    projection: AppliedStateProjection,
    generation: u64,
}

#[derive(Debug, Default)]
pub(crate) struct TargetStateCache {
    values: BTreeMap<WriteOnlyState, CachedProjection>,
    order: VecDeque<(WriteOnlyState, u64)>,
    generation: u64,
}

impl TargetStateCache {
    fn apply(&mut self, projection: AppliedStateProjection, capacity: usize) {
        if capacity == 0 {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let state = projection.state();
        self.values.insert(
            state,
            CachedProjection {
                projection,
                generation,
            },
        );
        self.order.push_back((state, generation));
        while self.values.len() > capacity {
            let Some((key, candidate_generation)) = self.order.pop_front() else {
                break;
            };
            if self
                .values
                .get(&key)
                .is_some_and(|value| value.generation == candidate_generation)
            {
                self.values.remove(&key);
            }
        }
        // Updates to the same key leave stale order tickets. Keep their number
        // bounded while preserving the most recent ticket for every value.
        if self.order.len() > capacity.saturating_mul(2) {
            self.order.retain(|(key, candidate_generation)| {
                self.values
                    .get(key)
                    .is_some_and(|value| value.generation == *candidate_generation)
            });
        }
    }

    fn get(&self, state: WriteOnlyState) -> Option<AppliedStateProjection> {
        self.values.get(&state).map(|value| value.projection)
    }

    pub(crate) fn entry(&self, state: WriteOnlyState) -> crate::state_cache::StateEntry {
        let Some(value) = self.get(state) else {
            return crate::state_cache::StateEntry::Unknown;
        };
        match value {
            AppliedStateProjection::Set { value, .. } => crate::state_cache::StateEntry::Set(
                crate::state_cache::StateValue::from_owner(value),
            ),
            AppliedStateProjection::Clear { value, .. } => crate::state_cache::StateEntry::Clear(
                crate::state_cache::StateValue::from_owner(value),
            ),
            AppliedStateProjection::Invalidate { .. } => crate::state_cache::StateEntry::Unknown,
        }
    }
}

#[derive(Debug)]
struct PermitPoolInner {
    available: Mutex<usize>,
    capacity: usize,
}

/// One shared admission limit covering boundary work and authoritative engine
/// entries. A permit moves from pending admission to the active record and is
/// released only when admission is rejected or a request reaches terminal.
#[derive(Debug, Clone)]
pub(crate) struct AdmissionPermitPool(Arc<PermitPoolInner>);

impl AdmissionPermitPool {
    fn new(capacity: usize) -> Self {
        Self(Arc::new(PermitPoolInner {
            available: Mutex::new(capacity),
            capacity,
        }))
    }

    pub(crate) fn capacity(&self) -> usize {
        self.0.capacity
    }

    pub(crate) fn try_acquire(&self) -> Option<AdmissionPermit> {
        let mut available = self.0.available.lock().unwrap_or_else(|p| p.into_inner());
        if *available == 0 {
            return None;
        }
        *available -= 1;
        Some(AdmissionPermit {
            pool: Arc::clone(&self.0),
            released: false,
        })
    }

    #[cfg(test)]
    fn available(&self) -> usize {
        *self.0.available.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[derive(Debug)]
pub(crate) struct AdmissionPermit {
    pool: Arc<PermitPoolInner>,
    released: bool,
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let mut available = self
            .pool
            .available
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        debug_assert!(*available < self.pool.capacity);
        *available = available.saturating_add(1).min(self.pool.capacity);
    }
}

#[derive(Debug)]
struct ObserverCell {
    detached: AtomicBool,
    resolved: AtomicBool,
    sender: flume::Sender<ReceiptObservation>,
}

impl ObserverCell {
    fn is_attached(&self) -> bool {
        !self.detached.load(Ordering::Acquire)
            && !self.resolved.load(Ordering::Acquire)
            && !self.sender.is_disconnected()
    }

    fn resolve(&self, observation: ReceiptObservation) -> Result<(), ()> {
        if !self.is_attached() {
            return Ok(());
        }
        if self
            .resolved
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(());
        }
        self.sender.try_send(observation).map_err(|_| ())
    }
}

#[derive(Debug, Clone)]
enum ReceiptObservation {
    Terminal(RuntimeOutcome),
    CancellationFailed(Error),
}

/// A bounded terminal observer. Dropping it is the detach operation: no actor
/// message and no protocol mutation are needed.
#[derive(Debug)]
pub(crate) struct CompletionObserver {
    cell: Arc<ObserverCell>,
    receiver: flume::Receiver<ReceiptObservation>,
}

impl CompletionObserver {
    fn try_recv(&self) -> Option<ReceiptObservation> {
        self.receiver.try_recv().ok()
    }

    fn recv(&self) -> Result<ReceiptObservation, Error> {
        self.receiver.recv().map_err(|_| Error::RuntimeShutdown)
    }

    #[cfg(feature = "async")]
    async fn recv_async(&self) -> Result<ReceiptObservation, Error> {
        self.receiver
            .recv_async()
            .await
            .map_err(|_| Error::RuntimeShutdown)
    }
}

impl Drop for CompletionObserver {
    fn drop(&mut self) {
        self.cell.detached.store(true, Ordering::Release);
    }
}

fn completion_pair() -> (Arc<ObserverCell>, CompletionObserver) {
    let (sender, receiver) = flume::bounded(1);
    let cell = Arc::new(ObserverCell {
        detached: AtomicBool::new(false),
        resolved: AtomicBool::new(false),
        sender,
    });
    (Arc::clone(&cell), CompletionObserver { cell, receiver })
}

/// Exact, linear observation authority shared by the mode-specific receipt
/// wrappers. It is deliberately neither cloneable nor publicly nameable.
#[derive(Debug)]
pub(crate) struct ReceiptCore {
    pub(crate) id: RequestId,
    pub(crate) target: CameraId,
    pub(crate) completion: CompletionObserver,
    pub(crate) configured_timeout: Duration,
    pub(crate) origin: Arc<()>,
}

impl ReceiptCore {
    fn new(
        id: RequestId,
        target: CameraId,
        completion: CompletionObserver,
        configured_timeout: Duration,
        origin: Arc<()>,
    ) -> Self {
        Self {
            id,
            target,
            completion,
            configured_timeout,
            origin,
        }
    }

    const fn id(&self) -> RequestId {
        self.id
    }

    const fn target(&self) -> CameraId {
        self.target
    }

    const fn configured_timeout(&self) -> Duration {
        self.configured_timeout
    }

    fn try_outcome(&self) -> Option<RuntimeOutcome> {
        self.completion.try_recv().map(observation_outcome)
    }

    #[cfg(all(test, feature = "blocking", not(feature = "async")))]
    pub(crate) fn terminal(&self) -> Option<RuntimeOutcome> {
        self.try_outcome()
    }

    #[cfg(all(test, feature = "async"))]
    pub(crate) async fn terminal(&self) -> Result<RuntimeOutcome, Error> {
        self.completion.recv_async().await.map(observation_outcome)
    }
}

/// Selection retained with a targeted settlement receipt until phase 6 creates
/// its one absolute deadline. Selecting an override never mutates engine time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaitSelection {
    Configured,
    Override(Duration),
}

fn normalize_command_outcome(outcome: RuntimeOutcome) -> Result<(), Error> {
    match outcome {
        RuntimeOutcome::Applied => Ok(()),
        RuntimeOutcome::Cancelled => Err(Error::CommandCanceled),
        RuntimeOutcome::Failed(error) => Err(error),
        RuntimeOutcome::Reply { .. } => Err(Error::InvalidState(
            "a command observer received an inquiry reply".into(),
        )),
    }
}

fn observation_outcome(observation: ReceiptObservation) -> RuntimeOutcome {
    match observation {
        ReceiptObservation::Terminal(outcome) => outcome,
        ReceiptObservation::CancellationFailed(error) => RuntimeOutcome::Failed(error),
    }
}

fn normalize_inquiry_outcome<R>(
    outcome: RuntimeOutcome,
    decoder: &crate::ResponseDecoder<R>,
) -> Result<R, Error> {
    match outcome {
        RuntimeOutcome::Reply { payload, .. } => decoder.decode(&payload),
        RuntimeOutcome::Failed(error) => Err(error),
        RuntimeOutcome::Cancelled => Err(Error::CommandCanceled),
        RuntimeOutcome::Applied => Err(Error::InvalidState(
            "an inquiry observer received a command outcome".into(),
        )),
    }
}

#[derive(Debug)]
struct PendingAdmission {
    permit: AdmissionPermit,
    observer: Arc<ObserverCell>,
    reply: flume::Sender<Result<RequestId, Error>>,
    summary: RequestSummary,
}

#[derive(Debug)]
struct ActiveRequest {
    _permit: AdmissionPermit,
    observer: Arc<ObserverCell>,
    summary: RequestSummary,
}

#[derive(Debug)]
struct CancellationWaiter {
    acknowledgement: flume::Sender<Result<(), Error>>,
    recorded: bool,
}

#[derive(Debug)]
struct CancellationRegistration {
    acknowledgement: flume::Receiver<Result<(), Error>>,
}

/// Receiver for the exact terminal engine cancellation observation. The
/// intermediate `Recorded` observation acknowledges `cancel()` and is not
/// exposed as a terminal token outcome.
#[derive(Debug)]
pub(crate) struct CancellationCore {
    id: RequestId,
    origin: Arc<()>,
    completion: CompletionObserver,
    buffered: Option<ReceiptObservation>,
}

impl CancellationCore {
    const fn id(&self) -> RequestId {
        self.id
    }

    fn try_observation(&mut self) -> Option<ReceiptObservation> {
        self.buffered.take().or_else(|| self.completion.try_recv())
    }

    fn recv(mut self) -> Result<ReceiptObservation, Error> {
        match self.buffered.take() {
            Some(observation) => Ok(observation),
            None => self.completion.recv(),
        }
    }

    #[cfg(feature = "async")]
    async fn recv_async(mut self) -> Result<ReceiptObservation, Error> {
        match self.buffered.take() {
            Some(observation) => Ok(observation),
            None => self.completion.recv_async().await,
        }
    }
}

fn cancellation_receipt_for(
    receipt: ReceiptCore,
    buffered: Option<ReceiptObservation>,
) -> CancellationCore {
    CancellationCore {
        id: receipt.id,
        origin: receipt.origin,
        completion: receipt.completion,
        buffered,
    }
}

fn normalize_cancellation_observation(
    observation: ReceiptObservation,
) -> Result<CancellationOutcome, Error> {
    match observation {
        ReceiptObservation::Terminal(RuntimeOutcome::Cancelled) => {
            Ok(CancellationOutcome::Cancelled)
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Applied) => Ok(CancellationOutcome::Completed),
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(error))
        | ReceiptObservation::CancellationFailed(error) => Err(error),
        ReceiptObservation::Terminal(RuntimeOutcome::Reply { .. }) => Err(Error::InvalidState(
            "an inquiry outcome cannot authorize cancellation".into(),
        )),
    }
}

/// Bounded applied-state event delivered after the target-local cache changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedStateEvent(pub(crate) AppliedStateEffect);

#[derive(Debug)]
pub(crate) struct AppliedStateSubscription {
    id: u64,
    receiver: flume::Receiver<AppliedStateEvent>,
}

impl AppliedStateSubscription {
    pub(crate) fn try_recv(&self) -> Option<AppliedStateEvent> {
        self.receiver.try_recv().ok()
    }
}

#[derive(Debug)]
struct AppliedSubscriber {
    target: Option<CameraId>,
    sender: flume::Sender<AppliedStateEvent>,
}

#[derive(Debug)]
pub(crate) struct DiagnosticSubscription {
    id: u64,
    receiver: flume::Receiver<DiagnosticEvent>,
}

impl DiagnosticSubscription {
    pub(crate) fn try_recv(&self) -> Option<DiagnosticEvent> {
        self.receiver.try_recv().ok()
    }

    #[cfg(feature = "async")]
    pub(crate) async fn recv_async(&self) -> Result<DiagnosticEvent, Error> {
        self.receiver
            .recv_async()
            .await
            .map_err(|_| Error::RuntimeShutdown)
    }
}

#[derive(Debug)]
struct DiagnosticSubscriber {
    sender: flume::Sender<DiagnosticEvent>,
}

/// Reused for every write/read/framing operation. Capacities never grow after
/// construction; over-size input is rejected before mutation.
#[derive(Debug)]
pub(crate) struct OwnerBuffers {
    send: Vec<u8>,
    transmit: BytesMut,
    receive: Box<[u8]>,
    framing: Vec<u8>,
    framing_limit: usize,
}

impl OwnerBuffers {
    fn new(limits: OwnerLimits) -> Result<Self, Error> {
        if limits.receive_bytes == 0 || limits.framing_bytes == 0 {
            return Err(Error::InvalidRequest(
                "owner receive and framing buffers must be non-zero".into(),
            ));
        }
        Ok(Self {
            send: Vec::with_capacity(MAX_BYTES),
            transmit: BytesMut::with_capacity(MAX_BYTES.saturating_add(8)),
            receive: vec![0; limits.receive_bytes].into_boxed_slice(),
            framing: Vec::with_capacity(limits.framing_bytes),
            framing_limit: limits.framing_bytes,
        })
    }

    fn prepare(&mut self, transmission: &Transmission) -> Result<(&[u8], &mut BytesMut), Error> {
        self.send.clear();
        self.transmit.clear();
        match transmission {
            Transmission::Request { wire, .. } => {
                if wire.as_bytes().len() > self.send.capacity() {
                    return Err(Error::ResponseTooLarge {
                        max_size: self.send.capacity(),
                    });
                }
                self.send.extend_from_slice(wire.as_bytes());
            }
            Transmission::Cancel { target, socket } => {
                self.send.extend_from_slice(&[
                    target.to_address_byte(),
                    socket.as_cancel_byte(),
                    0xff,
                ]);
            }
        }
        Ok((&self.send, &mut self.transmit))
    }

    pub(crate) fn receive_mut(&mut self) -> &mut [u8] {
        &mut self.receive
    }

    /// Copies one validated transport read into the owner-owned framing
    /// scratch without exposing the receive store to an adapter borrow.
    pub(crate) fn append_received(&mut self, received: usize) -> Result<(), Error> {
        if received > self.receive.len() {
            return Err(Error::InvalidResponse {
                expected: "transport read fitting the owner receive buffer".into(),
                actual: received.to_le_bytes().to_vec(),
            });
        }
        if self.framing.len().saturating_add(received) > self.framing_limit {
            return Err(Error::ResponseTooLarge {
                max_size: self.framing_limit,
            });
        }
        self.framing.extend_from_slice(&self.receive[..received]);
        Ok(())
    }

    pub(crate) fn framing(&self) -> &[u8] {
        &self.framing
    }

    pub(crate) fn append_framing(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.framing.len().saturating_add(bytes.len()) > self.framing_limit {
            return Err(Error::ResponseTooLarge {
                max_size: self.framing_limit,
            });
        }
        self.framing.extend_from_slice(bytes);
        Ok(())
    }

    pub(crate) fn consume_framing(&mut self, count: usize) {
        let count = count.min(self.framing.len());
        self.framing.drain(..count);
    }
}

/// Borrowed exact write handed to a mode-native transport adapter.
#[derive(Debug)]
pub(crate) struct WireWrite<'a> {
    pub(crate) transmission: TransmissionId,
    pub(crate) request: RequestId,
    pub(crate) target: CameraId,
    pub(crate) bytes: &'a [u8],
    /// Reusable framing destination owned by the session. Envelope adapters
    /// must frame into this buffer instead of allocating per transmission.
    pub(crate) frame_buffer: &'a mut BytesMut,
    pub(crate) cancellation: bool,
    pub(crate) inquiry: bool,
    pub(crate) envelope: EnvelopeKind,
}

#[derive(Debug)]
pub(crate) struct StagedWrite {
    pub(crate) transmission: TransmissionId,
    pub(crate) request: RequestId,
    pub(crate) kind: Transmission,
}

/// Owner boundary token for one ordered decoded-input batch.
///
/// The engine token is intentionally opaque here so blocking and async owners
/// cannot manufacture a different timestamp for recursive write completions.
#[derive(Debug)]
pub(crate) struct OwnerInputTurn(InputTurn);

impl StagedWrite {
    fn target(&self) -> CameraId {
        match self.kind {
            Transmission::Request { target, .. } | Transmission::Cancel { target, .. } => target,
        }
    }
}

/// Result of applying one non-I/O effect.
#[derive(Debug)]
pub(crate) enum AppliedEffect {
    None,
    Admitted(RequestId),
    AdmissionRejected(Error),
    Transmit(StagedWrite),
    Terminal(RequestId),
}

/// The owner's live operational tuning, shared with every handle that prepares
/// requests against it.
///
/// The owner is the sole writer: an update reaches it through the async control
/// boundary or, in the blocking mode, through the owner turn the caller thread
/// takes. Readers therefore see one whole [`crate::OperationalTuning`] value —
/// never a half-applied mixture of two updates — and two concurrent updates
/// resolve last-writer-wins in the order the owner accepted them.
#[derive(Debug, Clone)]
pub(crate) struct LiveTuning(Arc<Mutex<crate::OperationalTuning>>);

impl LiveTuning {
    fn new(tuning: crate::OperationalTuning) -> Self {
        Self(Arc::new(Mutex::new(tuning)))
    }

    /// Copies the tuning currently governing request preparation.
    pub(crate) fn get(&self) -> crate::OperationalTuning {
        *self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn set(&self, tuning: crate::OperationalTuning) {
        *self.0.lock().unwrap_or_else(|poison| poison.into_inner()) = tuning;
    }
}

/// Common serialized owner state shared by blocking and async modes.
#[derive(Debug)]
pub(crate) struct OwnerState {
    engine: ProtocolEngine,
    policy: OwnerPolicy,
    tuning: LiveTuning,
    permits: AdmissionPermitPool,
    pending: BTreeMap<AdmissionTicket, PendingAdmission>,
    active: BTreeMap<RequestId, ActiveRequest>,
    cancellation_waiters: BTreeMap<RequestId, CancellationWaiter>,
    target_cache: Arc<[Mutex<TargetStateCache>; 9]>,
    subscribers: BTreeMap<u64, AppliedSubscriber>,
    diagnostic_subscribers: BTreeMap<u64, DiagnosticSubscriber>,
    next_subscription: u64,
    next_diagnostic_subscription: u64,
    next_ticket: u64,
    diagnostics: VecDeque<DiagnosticEvent>,
    metrics: OwnerMetrics,
    buffers: OwnerBuffers,
    session_error: Option<Error>,
    origin: Arc<()>,
}

impl OwnerState {
    pub(crate) fn new(policy: OwnerPolicy) -> Result<Self, Error> {
        if policy.limits.diagnostics == 0
            || policy.limits.diagnostic_subscribers == 0
            || policy.limits.diagnostic_events_per_subscription == 0
            || policy.limits.state_keys_per_target == 0
            || policy.limits.applied_subscribers == 0
            || policy.limits.applied_events_per_subscription == 0
            || policy.limits.frames_per_receive == 0
        {
            return Err(Error::InvalidRequest(
                "owner collection bounds must be non-zero".into(),
            ));
        }
        if policy.targets[0].is_some() {
            return Err(Error::InvalidRequest(
                "owner target registry cannot contain camera zero".into(),
            ));
        }
        if policy.targets[8].is_some() {
            return Err(Error::InvalidRequest(
                "owner target registry cannot contain broadcast".into(),
            ));
        }
        let mut engine = ProtocolEngine::new(policy.protocol)?;
        for (index, target_policy) in policy.targets.iter().copied().enumerate().skip(1).take(7) {
            if let Some(target_policy) = target_policy {
                let target = CameraId::new(index as u8)?;
                engine.register_target(target, target_policy)?;
            }
        }
        let permits = AdmissionPermitPool::new(policy.protocol.capacity);
        let buffers = OwnerBuffers::new(policy.limits)?;
        let tuning = LiveTuning::new(policy.tuning);
        Ok(Self {
            engine,
            policy,
            tuning,
            permits,
            pending: BTreeMap::new(),
            active: BTreeMap::new(),
            cancellation_waiters: BTreeMap::new(),
            target_cache: Arc::new(array::from_fn(|_| Mutex::new(TargetStateCache::default()))),
            subscribers: BTreeMap::new(),
            diagnostic_subscribers: BTreeMap::new(),
            next_subscription: 1,
            next_diagnostic_subscription: 1,
            next_ticket: 1,
            diagnostics: VecDeque::new(),
            metrics: OwnerMetrics::default(),
            buffers,
            session_error: None,
            origin: Arc::new(()),
        })
    }

    pub(crate) fn permits(&self) -> AdmissionPermitPool {
        self.permits.clone()
    }

    pub(crate) fn origin(&self) -> Arc<()> {
        Arc::clone(&self.origin)
    }

    pub(crate) const fn policy(&self) -> &OwnerPolicy {
        &self.policy
    }

    /// Shares the live tuning cell with a handle that prepares requests.
    pub(crate) fn live_tuning(&self) -> LiveTuning {
        self.tuning.clone()
    }

    /// Installs new operational tuning on this owner (#631).
    ///
    /// Two things change, and the difference between them is the whole scope of
    /// runtime reconfiguration:
    ///
    /// * the session-wide pacing floor and per-target socket capacity are
    ///   re-derived from the profile baseline and applied to the engine at
    ///   once, so *queued* work is dispatched under the new values;
    /// * the tuning that request preparation reads is replaced, so every
    ///   request prepared after this call carries the new deadlines, retry
    ///   budget, and per-request pacing floor.
    ///
    /// A request that has already been admitted keeps the deadlines it was
    /// prepared with. Those are stamped once, at preparation, and the engine
    /// derives its absolute phase deadlines from them; nothing here rewrites an
    /// admitted request's context, so an in-flight command is never re-timed
    /// underneath its own observer.
    ///
    /// The caller is responsible for validating `tuning` against the registered
    /// profiles first — the owner does not retain `ProfileSpec` values, and the
    /// session facades reject exactly what construction rejects before the
    /// update reaches this point.
    pub(crate) fn retune(&mut self, tuning: crate::OperationalTuning) -> Result<(), Error> {
        let baseline = self.policy.baseline;
        let command_spacing = tuning
            .command_spacing_override()
            .unwrap_or(baseline.command_spacing);
        let inquiry_spacing = tuning
            .inquiry_spacing_override()
            .unwrap_or(baseline.inquiry_spacing);
        let command_sockets: [Option<u8>; 9] = array::from_fn(|index| {
            baseline.command_sockets[index].map(|profile_sockets| {
                tuning
                    .maximum_command_sockets_override()
                    .unwrap_or(profile_sockets)
            })
        });
        self.engine
            .retune(command_spacing, inquiry_spacing, command_sockets)?;
        self.policy.protocol.command_spacing = command_spacing;
        self.policy.protocol.inquiry_spacing = inquiry_spacing;
        for (slot, sockets) in self.policy.targets.iter_mut().zip(command_sockets.iter()) {
            if let (Some(policy), Some(sockets)) = (slot.as_mut(), sockets) {
                policy.command_sockets = *sockets;
            }
        }
        self.policy.tuning = tuning;
        self.tuning.set(tuning);
        Ok(())
    }

    pub(crate) const fn state(&self) -> SessionState {
        self.engine.state()
    }

    pub(crate) fn boundary_error(&self) -> Option<Error> {
        self.session_error.clone()
    }

    pub(crate) fn active_len(&self) -> usize {
        self.active.len()
    }

    pub(crate) fn metrics(&self) -> OwnerMetrics {
        self.metrics
    }

    pub(crate) fn metrics_snapshot(&self) -> crate::observability::MetricsSnapshot {
        crate::observability::metrics_snapshot(
            self.metrics,
            self.active.len(),
            self.pending.len(),
            self.state(),
        )
    }

    pub(crate) fn state_cache_registry(&self) -> Arc<[Mutex<TargetStateCache>; 9]> {
        Arc::clone(&self.target_cache)
    }

    pub(crate) fn state_cache(&self, target: CameraId) -> crate::state_cache::StateCache {
        crate::state_cache::StateCache::from_registry(self.state_cache_registry(), target)
    }

    pub(crate) fn diagnostics(&self) -> impl Iterator<Item = &DiagnosticEvent> {
        self.diagnostics.iter()
    }

    pub(crate) fn drain_diagnostics(&mut self) -> Vec<DiagnosticEvent> {
        self.diagnostics.drain(..).collect()
    }

    pub(crate) fn cached(
        &self,
        target: CameraId,
        state: WriteOnlyState,
    ) -> Option<AppliedStateProjection> {
        self.target_cache
            .get(usize::from(target.id()))
            .and_then(|slot| slot.lock().ok().and_then(|cache| cache.get(state)))
    }

    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.engine.next_wake()
    }

    pub(crate) fn dispatch_at(&self, id: RequestId) -> Option<Instant> {
        self.engine.queued_dispatch_at(id)
    }

    pub(crate) fn input(&mut self, input: Input, now: Instant) -> VecDeque<Effect> {
        self.observe_input(&input);
        self.engine.handle(input, now).into()
    }

    pub(crate) fn admit_without_due(
        &mut self,
        ticket: AdmissionTicket,
        request: RuntimeRequest,
        now: Instant,
    ) -> VecDeque<Effect> {
        self.engine.admit_without_due(ticket, request, now).into()
    }

    pub(crate) fn first_dispatch_without_due(
        &mut self,
        id: RequestId,
        now: Instant,
    ) -> FirstDispatch {
        self.engine.first_dispatch_without_due(id, now)
    }

    pub(crate) fn begin_input_turn(&self, now: Instant) -> OwnerInputTurn {
        OwnerInputTurn(self.engine.begin_input_turn(now))
    }

    /// Applies an input in an active decoded-input turn without running due
    /// deadlines or ordinary dispatch. Callers must drain the returned effects
    /// recursively before applying the next decoded frame.
    pub(crate) fn input_in_turn(
        &mut self,
        turn: &OwnerInputTurn,
        input: Input,
    ) -> VecDeque<Effect> {
        self.observe_input(&input);
        self.engine.handle_in_turn(&turn.0, input).into()
    }

    pub(crate) fn finish_input_turn(&mut self, turn: OwnerInputTurn) -> VecDeque<Effect> {
        self.engine.finish_input_turn(turn.0).into()
    }

    fn observe_input(&mut self, input: &Input) {
        if let Input::Frame(frame) = input {
            // Error frames are counted as the owner decodes them, so a camera
            // answering requests the engine can no longer correlate — the exact
            // case a field debugging session is trying to see — still shows up.
            if let DecodedResponse::Error { code, .. } = frame.response {
                if error_code_is_busy(code) {
                    self.metrics.busy_errors = self.metrics.busy_errors.saturating_add(1);
                } else if code != CANCELLATION_REPLY_CODE {
                    self.metrics.protocol_errors = self.metrics.protocol_errors.saturating_add(1);
                }
            }
            self.record(DiagnosticEvent::FrameReceived {
                target: frame.target,
                sequence: frame.sequence,
                response: response_diagnostic(&frame.response),
            });
        }
        if self.session_error.is_none() {
            self.session_error = boundary_error_for_input(input);
        }
    }

    pub(crate) fn advance(&mut self, now: Instant) -> VecDeque<Effect> {
        self.engine.advance(now).into()
    }

    pub(crate) fn finish_write(
        &mut self,
        staged: &StagedWrite,
        result: Result<TransmissionMeta, Error>,
        now: Instant,
    ) -> VecDeque<Effect> {
        self.observe_write(staged, &result);
        self.engine
            .handle(
                Input::TransmissionFinished {
                    transmission: staged.transmission,
                    result,
                },
                now,
            )
            .into()
    }

    pub(crate) fn finish_write_without_due(
        &mut self,
        staged: &StagedWrite,
        result: Result<TransmissionMeta, Error>,
        now: Instant,
    ) -> VecDeque<Effect> {
        self.observe_write(staged, &result);
        self.engine
            .finish_write_without_due(staged.transmission, result, now)
            .into()
    }

    /// Records and applies an identified write result in the same decoded-input
    /// turn that produced it, without allowing deadlines between source frames.
    pub(crate) fn finish_write_in_turn(
        &mut self,
        turn: &OwnerInputTurn,
        staged: &StagedWrite,
        result: Result<TransmissionMeta, Error>,
    ) -> VecDeque<Effect> {
        self.observe_write(staged, &result);
        self.engine
            .handle_in_turn(
                &turn.0,
                Input::TransmissionFinished {
                    transmission: staged.transmission,
                    result,
                },
            )
            .into()
    }

    fn observe_write(&mut self, staged: &StagedWrite, result: &Result<TransmissionMeta, Error>) {
        self.metrics.writes = self.metrics.writes.saturating_add(1);
        if result.is_err() {
            self.metrics.write_failures = self.metrics.write_failures.saturating_add(1);
        }
        let success = result.is_ok();
        let sequence = result.as_ref().ok().and_then(|meta| meta.sequence);
        if self.session_error.is_none()
            && result.is_err()
            && self.policy.protocol.transport == super::engine::TransportKind::Stream
        {
            let reason = result
                .as_ref()
                .err()
                .map_or_else(|| "stream write failed".to_owned(), ToString::to_string);
            self.session_error = Some(Error::StreamPoisoned {
                reason: reason.into(),
            });
        }
        self.record(DiagnosticEvent::WriteFinished {
            id: staged.request,
            transmission: staged.transmission,
            target: staged.target(),
            cancellation: matches!(staged.kind, Transmission::Cancel { .. }),
            envelope: self.policy.protocol.envelope,
            sequence,
            success,
        });
    }

    pub(crate) fn stage_admission(
        &mut self,
        request: RuntimeRequest,
        permit: AdmissionPermit,
    ) -> (
        Input,
        CompletionObserver,
        flume::Receiver<Result<RequestId, Error>>,
    ) {
        let (observer, receiver) = completion_pair();
        let (reply, admission) = flume::bounded(1);
        let input = self.stage_admission_with(request, permit, observer, reply);
        (input, receiver, admission)
    }

    fn stage_admission_with(
        &mut self,
        request: RuntimeRequest,
        permit: AdmissionPermit,
        observer: Arc<ObserverCell>,
        reply: flume::Sender<Result<RequestId, Error>>,
    ) -> Input {
        let ticket = self.allocate_ticket();
        let summary = RequestSummary::new(&request);
        let previous = self.pending.insert(
            ticket,
            PendingAdmission {
                permit,
                observer,
                reply,
                summary,
            },
        );
        debug_assert!(previous.is_none());
        Input::Admit { ticket, request }
    }

    fn allocate_ticket(&mut self) -> AdmissionTicket {
        loop {
            let value = self.next_ticket;
            self.next_ticket = self.next_ticket.wrapping_add(1);
            let ticket = AdmissionTicket(value);
            if !self.pending.contains_key(&ticket) {
                return ticket;
            }
        }
    }

    fn register_cancellation(&mut self, id: RequestId) -> CancellationRegistration {
        let (acknowledgement, acknowledgement_receiver) = flume::bounded(1);
        if !self.active.contains_key(&id) {
            let _ = acknowledgement.try_send(Err(Error::InvalidState(
                "cancellation target is not active".into(),
            )));
        } else {
            match self.cancellation_waiters.entry(id) {
                std::collections::btree_map::Entry::Occupied(_) => {
                    let _ = acknowledgement.try_send(Err(Error::InvalidState(
                        "cancellation is already being observed".into(),
                    )));
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(CancellationWaiter {
                        acknowledgement,
                        recorded: false,
                    });
                }
            }
        }
        CancellationRegistration {
            acknowledgement: acknowledgement_receiver,
        }
    }

    pub(crate) fn subscribe_applied(
        &mut self,
        target: Option<CameraId>,
        event_capacity: usize,
    ) -> Result<AppliedStateSubscription, Error> {
        if event_capacity == 0
            || event_capacity > self.policy.limits.applied_events_per_subscription
        {
            return Err(Error::InvalidRequest(
                "applied-state subscription capacity is outside owner bounds".into(),
            ));
        }
        self.subscribers
            .retain(|_, value| !value.sender.is_disconnected());
        if self.subscribers.len() >= self.policy.limits.applied_subscribers {
            return Err(Error::RuntimeQueueFull {
                capacity: self.policy.limits.applied_subscribers,
            });
        }
        let id = allocate_subscription_id(&mut self.next_subscription, &self.subscribers);
        let (sender, receiver) = flume::bounded(event_capacity);
        self.subscribers
            .insert(id, AppliedSubscriber { target, sender });
        Ok(AppliedStateSubscription { id, receiver })
    }

    pub(crate) fn unsubscribe_applied(&mut self, subscription: &AppliedStateSubscription) {
        self.subscribers.remove(&subscription.id);
    }

    pub(crate) fn subscribe_diagnostics(
        &mut self,
        event_capacity: usize,
    ) -> Result<DiagnosticSubscription, Error> {
        if event_capacity == 0
            || event_capacity > self.policy.limits.diagnostic_events_per_subscription
        {
            return Err(Error::InvalidRequest(
                "diagnostic subscription capacity is outside owner bounds".into(),
            ));
        }
        self.diagnostic_subscribers
            .retain(|_, value| !value.sender.is_disconnected());
        if self.diagnostic_subscribers.len() >= self.policy.limits.diagnostic_subscribers {
            return Err(Error::RuntimeQueueFull {
                capacity: self.policy.limits.diagnostic_subscribers,
            });
        }
        let id = allocate_subscription_id(
            &mut self.next_diagnostic_subscription,
            &self.diagnostic_subscribers,
        );
        let (sender, receiver) = flume::bounded(event_capacity);
        self.diagnostic_subscribers
            .insert(id, DiagnosticSubscriber { sender });
        Ok(DiagnosticSubscription { id, receiver })
    }

    pub(crate) fn unsubscribe_diagnostics(&mut self, subscription: &DiagnosticSubscription) {
        self.diagnostic_subscribers.remove(&subscription.id);
    }

    pub(crate) fn validate_frame_batch(&self, frames: &[DecodedFrame]) -> Result<(), Error> {
        if frames.len() > self.policy.limits.frames_per_receive {
            return Err(Error::ResponseTooLarge {
                max_size: self.policy.limits.frames_per_receive,
            });
        }
        if frames.iter().any(|frame| {
            matches!(
                &frame.response,
                DecodedResponse::InquiryReply { payload, .. }
                    if payload.len() > self.policy.limits.receive_bytes
            )
        }) {
            return Err(Error::ResponseTooLarge {
                max_size: self.policy.limits.receive_bytes,
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn request_state(&self, id: RequestId) -> Option<(Phase, CancelState)> {
        self.engine
            .entry(id)
            .map(|entry| (entry.phase(), entry.cancellation()))
    }

    pub(crate) fn prepare_write<'a>(
        &'a mut self,
        staged: &StagedWrite,
    ) -> Result<WireWrite<'a>, Error> {
        let target = staged.target();
        let cancellation = matches!(staged.kind, Transmission::Cancel { .. });
        let inquiry = self
            .active
            .get(&staged.request)
            .is_some_and(|active| active.summary.lane == RequestLane::Inquiry);
        let (bytes, frame_buffer) = self.buffers.prepare(&staged.kind)?;
        Ok(WireWrite {
            transmission: staged.transmission,
            request: staged.request,
            target,
            bytes,
            frame_buffer,
            cancellation,
            inquiry,
            envelope: self.policy.protocol.envelope,
        })
    }

    pub(crate) fn buffers(&mut self) -> &mut OwnerBuffers {
        &mut self.buffers
    }

    /// Apply an effect after it reaches the head of the ordered effect queue.
    /// `Transmit` is returned to the caller and must be executed immediately;
    /// its exact `TransmissionFinished` effects are prepended before processing
    /// any previously queued effect or unrelated boundary input.
    pub(crate) fn apply_effect(&mut self, effect: Effect) -> AppliedEffect {
        match effect {
            Effect::Admitted { ticket, id } => {
                let Some(pending) = self.pending.remove(&ticket) else {
                    self.record(DiagnosticEvent::Ignored(IgnoreReason::UnknownRequest));
                    return AppliedEffect::None;
                };
                let PendingAdmission {
                    permit,
                    observer,
                    reply,
                    summary,
                } = pending;
                self.active.insert(
                    id,
                    ActiveRequest {
                        _permit: permit,
                        observer,
                        summary,
                    },
                );
                self.metrics.admitted = self.metrics.admitted.saturating_add(1);
                self.record(DiagnosticEvent::Admitted {
                    id,
                    target: summary.target,
                    lane: summary.lane,
                    timeout: summary.timeout,
                    retry: summary.retry,
                    control: summary.control,
                    cancellation: summary.cancellation,
                });
                let _ = reply.try_send(Ok(id));
                AppliedEffect::Admitted(id)
            }
            Effect::AdmissionRejected { ticket, error } => {
                if let Some(pending) = self.pending.remove(&ticket) {
                    self.record(DiagnosticEvent::AdmissionRejected {
                        target: pending.summary.target,
                        lane: pending.summary.lane,
                        error: error.kind(),
                    });
                    let _ = pending.reply.try_send(Err(error.clone()));
                    // Dropping pending releases the boundary/engine permit and
                    // disconnects its terminal observer without allocating an ID.
                } else {
                    self.record(DiagnosticEvent::Ignored(IgnoreReason::UnknownRequest));
                }
                self.metrics.admission_rejected = self.metrics.admission_rejected.saturating_add(1);
                AppliedEffect::AdmissionRejected(error)
            }
            Effect::Transition {
                id,
                from,
                to,
                cancellation,
            } => {
                if let Some(target) = self.active.get(&id).map(|active| active.summary.target) {
                    self.record(DiagnosticEvent::Transition {
                        id,
                        target,
                        from,
                        to,
                        cancellation,
                    });
                }
                AppliedEffect::None
            }
            Effect::Transmit {
                transmission,
                request,
                kind,
            } => AppliedEffect::Transmit(StagedWrite {
                transmission,
                request,
                kind,
            }),
            Effect::RetryScheduled {
                id,
                attempt,
                ready_at,
            } => {
                // Counted where the retry is emitted, not where it is decided:
                // every retry reason the engine has — a busy camera, an expired
                // deadline, a transient receive fault (#565) — funnels through
                // this one effect, so the counter stays correct as retry policy
                // evolves.
                self.metrics.retries_scheduled = self.metrics.retries_scheduled.saturating_add(1);
                if let Some(target) = self.active.get(&id).map(|active| active.summary.target) {
                    self.record(DiagnosticEvent::RetryScheduled {
                        id,
                        target,
                        attempt,
                        ready_at,
                    });
                }
                AppliedEffect::None
            }
            Effect::DeadlineExpired {
                id,
                deadline,
                will_retry,
            } => {
                let counter = match deadline {
                    DeadlineKind::Ack => &mut self.metrics.ack_timeouts,
                    DeadlineKind::Completion => &mut self.metrics.completion_timeouts,
                    DeadlineKind::InquiryReply => &mut self.metrics.inquiry_timeouts,
                };
                *counter = counter.saturating_add(1);
                if let Some(target) = self.active.get(&id).map(|active| active.summary.target) {
                    self.record(DiagnosticEvent::DeadlineExpired {
                        id,
                        target,
                        deadline,
                        will_retry,
                    });
                }
                AppliedEffect::None
            }
            Effect::CancellationRecorded { id } => {
                self.metrics.cancellations = self.metrics.cancellations.saturating_add(1);
                if let Some(waiter) = self.cancellation_waiters.get_mut(&id) {
                    waiter.recorded = true;
                    let _ = waiter.acknowledgement.try_send(Ok(()));
                }
                if let Some(target) = self.active.get(&id).map(|active| active.summary.target) {
                    self.record(DiagnosticEvent::CancellationRecorded { id, target });
                }
                AppliedEffect::None
            }
            Effect::CancellationObservation { id, observation } => {
                let target = self.active.get(&id).map(|active| active.summary.target);
                let diagnostic = cancellation_diagnostic(&observation);
                match observation {
                    CancellationObservation::Recorded => {
                        if let Some(waiter) = self.cancellation_waiters.get_mut(&id) {
                            waiter.recorded = true;
                            let _ = waiter.acknowledgement.try_send(Ok(()));
                        }
                    }
                    CancellationObservation::Failed(error) => {
                        if let Some(waiter) = self.cancellation_waiters.remove(&id) {
                            if waiter.recorded {
                                if self.active.get(&id).is_some_and(|active| {
                                    active
                                        .observer
                                        .resolve(ReceiptObservation::CancellationFailed(error))
                                        .is_err()
                                }) {
                                    self.metrics.dropped_observer_events =
                                        self.metrics.dropped_observer_events.saturating_add(1);
                                }
                            } else {
                                let _ = waiter.acknowledgement.try_send(Err(error));
                            }
                        }
                    }
                    CancellationObservation::Cancelled | CancellationObservation::Completed => {
                        if let Some(waiter) = self.cancellation_waiters.get_mut(&id) {
                            if !waiter.recorded {
                                waiter.recorded = true;
                                let _ = waiter.acknowledgement.try_send(Ok(()));
                            }
                        }
                    }
                }
                if let Some(target) = target {
                    self.record(DiagnosticEvent::CancellationObserved {
                        id,
                        target,
                        observation: diagnostic,
                    });
                }
                AppliedEffect::None
            }
            Effect::AppliedState { effect } => {
                // Cache mutation is observer-independent and always precedes
                // best-effort subscriber delivery.
                if let Some(slot) = self.target_cache.get(usize::from(effect.target.id())) {
                    if let Ok(mut cache) = slot.lock() {
                        cache.apply(effect.projection, self.policy.limits.state_keys_per_target);
                    }
                }
                self.metrics.cache_updates = self.metrics.cache_updates.saturating_add(1);
                self.record(DiagnosticEvent::AppliedState {
                    id: effect.request,
                    target: effect.target,
                    key: effect.projection.state(),
                });
                let event = AppliedStateEvent(effect);
                self.subscribers.retain(|_, subscriber| {
                    if subscriber.sender.is_disconnected() {
                        return false;
                    }
                    if subscriber
                        .target
                        .is_none_or(|target| target == effect.target)
                        && subscriber.sender.try_send(event).is_err()
                    {
                        self.metrics.dropped_applied_events =
                            self.metrics.dropped_applied_events.saturating_add(1);
                    }
                    true
                });
                AppliedEffect::None
            }
            Effect::Terminal { id, outcome } => {
                let diagnostic = outcome_diagnostic(&outcome);
                if let Some(active) = self.active.remove(&id) {
                    let target = active.summary.target;
                    if active
                        .observer
                        .resolve(ReceiptObservation::Terminal(outcome.clone()))
                        .is_err()
                    {
                        self.metrics.dropped_observer_events =
                            self.metrics.dropped_observer_events.saturating_add(1);
                    }
                    // Dropping active releases admission capacity only here.
                    self.record(DiagnosticEvent::Terminal {
                        id,
                        target,
                        outcome: diagnostic,
                    });
                }
                if let Some(waiter) = self.cancellation_waiters.remove(&id) {
                    if !waiter.recorded {
                        let _ = waiter.acknowledgement.try_send(Ok(()));
                    }
                }
                self.metrics.terminal = self.metrics.terminal.saturating_add(1);
                AppliedEffect::Terminal(id)
            }
            Effect::SessionChanged { from, to } => {
                let reason = self
                    .session_error
                    .as_ref()
                    .map_or(ErrorKind::Other, Error::kind);
                self.record(DiagnosticEvent::SessionChanged { from, to, reason });
                AppliedEffect::None
            }
            Effect::Ignored(reason) => {
                if reason == IgnoreReason::UnmatchedSequencedFrame {
                    self.metrics.ignored_unmatched_sequenced_replies = self
                        .metrics
                        .ignored_unmatched_sequenced_replies
                        .saturating_add(1);
                }
                self.record(DiagnosticEvent::Ignored(reason));
                AppliedEffect::None
            }
        }
    }

    pub(crate) fn fail_unstaged_boundary(&mut self, count: usize) {
        self.metrics.dropped_boundary_work = self
            .metrics
            .dropped_boundary_work
            .saturating_add(count as u64);
    }

    fn record(&mut self, event: DiagnosticEvent) {
        if self.diagnostics.len() == self.policy.limits.diagnostics {
            self.diagnostics.pop_front();
            self.metrics.dropped_diagnostics = self.metrics.dropped_diagnostics.saturating_add(1);
        }
        self.diagnostics.push_back(event);
        self.diagnostic_subscribers.retain(|_, subscriber| {
            if subscriber.sender.is_disconnected() {
                return false;
            }
            if subscriber.sender.try_send(event).is_err() {
                self.metrics.dropped_diagnostic_events =
                    self.metrics.dropped_diagnostic_events.saturating_add(1);
            }
            true
        });
    }

    pub(crate) fn shutdown_input(reason: ShutdownReason) -> Input {
        Input::Shutdown(reason)
    }

    pub(crate) fn frame_input(frame: DecodedFrame) -> Input {
        Input::Frame(frame)
    }
}

fn allocate_subscription_id<T>(next: &mut u64, values: &BTreeMap<u64, T>) -> u64 {
    loop {
        let id = *next;
        *next = next.wrapping_add(1);
        if !values.contains_key(&id) {
            return id;
        }
    }
}

fn response_diagnostic(response: &DecodedResponse) -> ResponseDiagnostic {
    match response {
        DecodedResponse::Ack { socket } => ResponseDiagnostic::Ack(*socket),
        DecodedResponse::Completion { socket } => ResponseDiagnostic::Completion(*socket),
        DecodedResponse::InquiryReply { .. } => ResponseDiagnostic::InquiryReply,
        DecodedResponse::Error { socket, code } => ResponseDiagnostic::Error {
            socket: *socket,
            code: *code,
        },
        DecodedResponse::NetworkChange => ResponseDiagnostic::NetworkChange,
        DecodedResponse::Unknown => ResponseDiagnostic::Unknown,
    }
}

fn outcome_diagnostic(outcome: &RuntimeOutcome) -> OutcomeDiagnostic {
    match outcome {
        RuntimeOutcome::Applied => OutcomeDiagnostic::Applied,
        RuntimeOutcome::Reply { .. } => OutcomeDiagnostic::Reply,
        RuntimeOutcome::Cancelled => OutcomeDiagnostic::Cancelled,
        RuntimeOutcome::Failed(error) => OutcomeDiagnostic::Failed(error.kind()),
    }
}

fn cancellation_diagnostic(observation: &CancellationObservation) -> CancellationDiagnostic {
    match observation {
        CancellationObservation::Recorded => CancellationDiagnostic::Recorded,
        CancellationObservation::Cancelled => CancellationDiagnostic::Cancelled,
        CancellationObservation::Completed => CancellationDiagnostic::Completed,
        CancellationObservation::Failed(error) => CancellationDiagnostic::Failed(error.kind()),
    }
}

fn cancellation_observation_for(outcome: &RuntimeOutcome) -> CancellationObservation {
    match outcome {
        RuntimeOutcome::Applied => CancellationObservation::Completed,
        RuntimeOutcome::Cancelled => CancellationObservation::Cancelled,
        RuntimeOutcome::Failed(error) => CancellationObservation::Failed(error.clone()),
        RuntimeOutcome::Reply { .. } => CancellationObservation::Failed(Error::InvalidState(
            "an inquiry outcome cannot resolve cancellation".into(),
        )),
    }
}

fn boundary_error_for_input(input: &Input) -> Option<Error> {
    match input {
        Input::Close { reason } => Some(Error::ConnectionClosed {
            reason: reason.as_ref().map(|value| value.to_string().into()),
        }),
        Input::Poison { reason } => Some(Error::StreamPoisoned {
            reason: reason.to_string().into(),
        }),
        Input::Shutdown(reason) => Some(match reason {
            ShutdownReason::Explicit => Error::RuntimeShutdown,
            ShutdownReason::TransportClosed { reason } => Error::ConnectionClosed {
                reason: reason.as_ref().map(|value| value.to_string().into()),
            },
            ShutdownReason::FramingFailure { reason } => Error::StreamPoisoned {
                reason: reason.to_string().into(),
            },
        }),
        // A transient receive fault is explicitly not a boundary error: the
        // session survives it and retried work keeps its own outcome.
        Input::Admit { .. }
        | Input::TransmissionFinished { .. }
        | Input::Frame(_)
        | Input::Cancel { .. }
        | Input::ReceiveFault { .. }
        | Input::Wake => None,
    }
}

/// Whether one receive-side transport error means "no bytes arrived" rather
/// than "this read failed".
///
/// A transport with an internal read timeout is the natural custom
/// implementation — it is exactly the shape [`crate::transport::BlockingTransport::recv_into_with_timeout`]
/// documents — and an expired idle timeout consumed nothing and proved nothing.
/// Treating it as a receive fault would retransmit every command still waiting
/// for its ACK, burning whole retry budgets in milliseconds, and on the async
/// owner it would spin the actor's transient-fault arm (#625, #637).
///
/// Both owners therefore normalize these to "this read produced no frames" and
/// keep pumping, which is what the blocking adapter has always done for
/// [`Error::Timeout`].
pub(crate) fn receive_reported_no_data(error: &Error) -> bool {
    match error {
        Error::Timeout => true,
        Error::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::Interrupted
        ),
        Error::WithContext { source, .. } => receive_reported_no_data(source),
        _ => false,
    }
}

/// Normalize one datagram send failure into a per-request error.
///
/// A datagram send failure fails exactly one request and the session keeps
/// running, so the value the caller observes must not claim that a replacement
/// session is required: that combination is self-contradictory and a custom
/// transport can reach it just by returning [`Error::ConnectionClosed`] from
/// `send` (#637). The receive side is the authority on session death — it is
/// always pumping, and a socket that is genuinely gone fails its next read.
pub(crate) fn normalize_datagram_send_error(error: Error) -> Error {
    if error.requires_new_session() {
        Error::TransportError(format!("datagram send failed: {error}").into())
    } else {
        error
    }
}

/// Whether one receive-side transport failure leaves the session usable.
///
/// 1.x drew this line in the runtime loops: a `ConnectionClosed` read ended the
/// session, and every other read error became a `NetworkError` that retried
/// in-flight work and kept the loop alive. This keeps that contract and states
/// the remaining fatal cases explicitly instead of inheriting them from
/// [`ErrorKind`], which cannot separate "the peer went away" from "this read
/// failed".
///
/// Transient therefore includes the case the issue is about: a UDP `recv`
/// reporting ECONNREFUSED after an ICMP port-unreachable, which says nothing
/// about whether the camera is reachable now.
///
/// This answers "fatal or transient" only. Errors that report no data at all
/// are separated out by [`receive_reported_no_data`] first and never reach a
/// fault classification.
pub(crate) fn receive_fault_is_transient(error: &Error) -> bool {
    match error {
        // Proof the session is finished: the peer closed, the byte stream
        // position is unknowable, or the owner's transport/channel is gone.
        error if error.requires_new_session() => false,
        Error::RuntimeShutdown => false,
        // A raw I/O failure is fatal only when the operating system reported
        // that this connection itself is dead.
        Error::Io(io) => !matches!(
            io.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::NotConnected
        ),
        Error::WithContext { source, .. } => receive_fault_is_transient(source),
        _ => true,
    }
}

/// Prepend recursively produced effects while preserving their source order.
pub(crate) fn prepend_effects(queue: &mut VecDeque<Effect>, produced: VecDeque<Effect>) {
    for effect in produced.into_iter().rev() {
        queue.push_front(effect);
    }
}
