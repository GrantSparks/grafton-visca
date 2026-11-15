//! Serialized owners for the deterministic protocol engine.
//!
//! The engine is the only lifecycle-policy authority.  This module owns the
//! bounded resources around it (admission, observers, caches, diagnostics and
//! reusable I/O storage) and exposes small mode-native driver seams.

mod adapter;
#[cfg(any(feature = "async", feature = "blocking"))]
mod halt;
mod motion;
use motion::{Admitted, MotionRegistry, MotionStamp};
#[cfg(feature = "async")]
mod async_actor;
#[cfg(feature = "async")]
mod async_transport;
#[cfg(feature = "blocking")]
mod blocking;
#[cfg(feature = "blocking")]
mod blocking_transport;
#[cfg(any(feature = "async", feature = "blocking"))]
mod boundary;
#[cfg(feature = "blocking")]
mod clock;
#[cfg(any(feature = "async", feature = "blocking"))]
mod driver;
#[cfg(any(feature = "async", feature = "blocking"))]
mod handle;
#[cfg(any(feature = "async", feature = "blocking"))]
mod receipt;
#[cfg(any(feature = "async", feature = "blocking"))]
mod shell;
mod turn;

#[cfg(feature = "async")]
pub(crate) use async_actor::*;
#[cfg(feature = "blocking")]
pub(crate) use blocking::*;
#[cfg(feature = "blocking")]
pub(crate) use clock::Clock;

pub(crate) use adapter::validate_profile_transport;
#[cfg(feature = "async")]
pub(crate) use adapter::AsyncTransportAdapter;
#[cfg(feature = "blocking")]
pub(crate) use adapter::BlockingTransportAdapter;
#[cfg(feature = "test-utils")]
pub(crate) use adapter::ReceiveProbe;
#[cfg(all(feature = "test-utils", feature = "blocking"))]
pub(crate) use adapter::{decode_response_target, RoutingState, TargetRegistry};

#[cfg(any(feature = "async", feature = "blocking"))]
use turn::{clamp_receive_pause, transient_receive_pause, IdleReceiveRun, TransientFaultRun};

#[cfg(all(
    test,
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use turn::{
    MAXIMUM_TRANSIENT_RECEIVE_PAUSE, TRANSIENT_RECEIVE_FAULT_LIMIT, TRANSIENT_RECEIVE_FAULT_RESET,
    TRANSIENT_RECEIVE_FAULT_SPAN, TRANSIENT_RECEIVE_PAUSE,
};

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};

use bytes::BytesMut;

use crate::{
    command::{encode::WireEncode, system::CommandCancelCommand},
    target::{PerTarget, TargetSlot},
    StateKey,
};
use crate::{raw::MAX_BYTES, CancellationOutcome, Error, ErrorKind, ViscaSocket};

use super::engine::{
    AdmissionSlot, AdmissionTicket, AppliedStateProjection, CancellationObservation,
    CancellationPolicy, ControlClass, DeadlineKind, DecodedFrame, DecodedResponse, Effect,
    EngineTurn, EnvelopeKind, EnvelopeSequence, IgnoreReason, Input, InputTurn, Lane, Lifecycle,
    ProtocolEngine, ProtocolPolicy, RequestContext, RequestId, RetryPolicy, RuntimeOutcome,
    RuntimeRequest, SessionState, ShutdownReason, TargetPolicy, TimeoutPolicy, Transmission,
    TransmissionId, TransmissionMeta, TransportKind, WriteFraming, WriteOutcome,
};

#[cfg(test)]
use super::engine::{CancelState, Phase};

#[cfg(any(feature = "async", feature = "blocking"))]
use super::engine::{RawCorrelationReleaseSet, RawPrefixEvidence, RawReleaseGateAction};

#[cfg(test)]
mod tests;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Compared by the blocking worker tests and by `async_actor::tests` (Tokio);
// neither compiles on the plain `async` or smol legs (#636).
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
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

// Consumed by the blocking worker tests and by `async_actor::tests` (Tokio);
// neither compiles on the plain `async` or smol legs (#636).
#[cfg(all(
    test,
    any(
        feature = "blocking",
        all(feature = "async", feature = "runtime-tokio")
    )
))]
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

// Consumed by the blocking worker tests and by `async_actor::tests` (Tokio);
// neither compiles on the plain `async` or smol legs (#636).
#[cfg(all(
    test,
    any(
        feature = "blocking",
        all(feature = "async", feature = "runtime-tokio")
    )
))]
pub(crate) fn canonical_owner_trace(
    diagnostics: impl IntoIterator<Item = DiagnosticEvent>,
) -> Vec<CanonicalOwnerStep> {
    diagnostics
        .into_iter()
        .filter_map(|event| match event {
            DiagnosticEvent::Admitted { .. } => Some(CanonicalOwnerStep::Admitted),
            DiagnosticEvent::Transition { to, .. }
                if matches!(to.phase(), Phase::Sending { .. }) =>
            {
                Some(CanonicalOwnerStep::Sending)
            }
            DiagnosticEvent::WriteFinished {
                cancellation: false,
                ..
            } => Some(CanonicalOwnerStep::RequestWrite),
            DiagnosticEvent::Transition { to, .. }
                if matches!(to.phase(), Phase::AwaitingAck { .. }) =>
            {
                Some(CanonicalOwnerStep::AwaitingAck)
            }
            DiagnosticEvent::FrameReceived {
                response: ResponseDiagnostic::Ack(_),
                ..
            } => Some(CanonicalOwnerStep::Ack),
            DiagnosticEvent::Transition { to, .. }
                if matches!(to.phase(), Phase::Executing { .. }) =>
            {
                Some(CanonicalOwnerStep::Executing)
            }
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
    pub(crate) staged_diagnostics: usize,
    pub(crate) control_requests: usize,
    pub(crate) diagnostic_subscribers: usize,
    pub(crate) diagnostic_events_per_subscription: usize,
    pub(crate) frames_per_receive: usize,
    pub(crate) receive_bytes: usize,
}

impl Default for OwnerLimits {
    fn default() -> Self {
        Self {
            staged_diagnostics: 128,
            control_requests: 16,
            diagnostic_subscribers: 4,
            diagnostic_events_per_subscription: 128,
            frames_per_receive: 64,
            receive_bytes: 4_096,
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
    pub(crate) command_sockets: PerTarget<Option<u8>>,
}

/// Construction policy. Target registration cannot change after the owner
/// begins accepting work; pacing, socket capacity, and the session tuning that
/// governs request preparation are reconfigurable through the owner's control
/// boundary.
#[derive(Debug, Clone)]
pub(crate) struct OwnerPolicy {
    pub(crate) protocol: ProtocolPolicy,
    pub(crate) targets: PerTarget<Option<TargetPolicy>>,
    pub(crate) limits: OwnerLimits,
    pub(crate) tuning: crate::session_config::ValidatedTuning,
    pub(crate) baseline: TuningBaseline,
    /// Maximum time one transport read may take before the owner treats the
    /// read as "no data arrived" (#675). The async owner enforces it around
    /// [`AsyncOwnerDriver::receive`](super::AsyncOwnerDriver) because the
    /// runtime-agnostic async transports have no timer of their own; the
    /// blocking worker reports no data once its sliced reads have been idle
    /// this long (#780). Lowered
    /// from the transport [`TransportConfig`](crate::transport::TransportConfig)
    /// by the production policy builder; [`Self::with_targets`] starts it at
    /// the `TransportConfig` default.
    pub(crate) read_timeout: Duration,
    /// Maximum time one async transport write may take before the owner
    /// abandons it as a stalled write (#675). A byte-stream write that cannot
    /// be confirmed poisons the session; a datagram write fails only its own
    /// request. Without this bound a stalled peer parks the actor and
    /// `close()` never returns.
    pub(crate) write_timeout: Duration,
}

impl OwnerPolicy {
    /// Creates one immutable policy from target-local protocol facts.
    pub(crate) fn with_targets(
        protocol: ProtocolPolicy,
        targets: PerTarget<Option<TargetPolicy>>,
        tuning: crate::session_config::ValidatedTuning,
    ) -> Result<Self, Error> {
        if targets.iter().all(|(_, target)| target.is_none()) {
            return Err(Error::InvalidRequest(
                "owner target registry must contain at least one camera".into(),
            ));
        }
        // The read and write bounds start from the one transport default
        // (#799); the production builder lowers the caller's configured
        // values over them (#675).
        let transport = crate::transport::TransportConfig::default();
        let baseline = TuningBaseline {
            command_spacing: protocol.command_spacing,
            inquiry_spacing: protocol.inquiry_spacing,
            command_sockets: PerTarget::from_fn(|target| {
                targets[target].map(|policy| policy.command_sockets)
            }),
        };
        Ok(Self {
            protocol,
            targets,
            limits: OwnerLimits::default(),
            tuning,
            baseline,
            read_timeout: transport.read_timeout,
            write_timeout: transport.write_timeout,
        })
    }

    // Test-only today: production goes through
    // `adapter::owner_policy_for_targets_with_tuning`.
    #[cfg(test)]
    pub(crate) fn single_target(
        protocol: ProtocolPolicy,
        target: TargetSlot,
        target_policy: TargetPolicy,
    ) -> Result<Self, Error> {
        let mut targets = PerTarget::default();
        targets[target] = Some(target_policy);
        Self::with_targets(
            protocol,
            targets,
            crate::session_config::ValidatedTuning::for_test(crate::OperationalTuning::new())?,
        )
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
    pub(crate) control_reserve_admitted: u64,
    pub(crate) control_reserve_rejected: u64,
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
    /// Valid decoded VISCA response frames observed from the peer.
    pub(crate) received_frames: u64,
    pub(crate) ignored_unmatched_sequenced_replies: u64,
    /// Delimited frames a byte stream discarded as malformed while staying
    /// Running (#672). The session continues past these frames; this counter makes
    /// the otherwise lossy-diagnostics-only signal a durable metric.
    pub(crate) ignored_malformed_frames: u64,
    pub(crate) dropped_staged_diagnostics: u64,
    pub(crate) dropped_diagnostic_events: u64,
    pub(crate) dropped_observer_events: u64,
    pub(crate) dropped_boundary_work: u64,
}

/// The camera reported that it cannot accept this request right now.
///
/// These are the codes the engine's own retry classification treats as
/// transient camera-side backpressure: command buffer full (`0x03`), no socket
/// available (`0x05`), and not executable in the current state (`0x41`).
///
/// `0x04` is deliberately not in this set: it is the cancellation reply,
/// which this engine consumes as the confirmation of a cancel rather than as a
/// failure, so counting it as a busy error would make every successful
/// cancellation look like camera backpressure.
const fn error_code_is_busy(code: u8) -> bool {
    matches!(code, 0x03 | 0x05 | 0x41)
}

/// The cancellation reply code, which is an answer rather than a fault.
const CANCELLATION_REPLY_CODE: u8 = 0x04;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseDiagnostic {
    Ack(Option<ViscaSocket>),
    Completion(Option<ViscaSocket>),
    InquiryReply,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutcomeDiagnostic {
    /// The local transport accepted a raw plain fire-and-forget write. This
    /// carries no camera protocol-application claim.
    Written,
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
    target: TargetSlot,
    lane: Lane,
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
            lane: request.lane(),
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
        target: TargetSlot,
        lane: Lane,
        timeout: TimeoutPolicy,
        retry: RetryPolicy,
        control: ControlClass,
        cancellation: CancellationPolicy,
    },
    AdmissionRejected {
        target: TargetSlot,
        lane: Lane,
        error: ErrorKind,
    },
    FrameReceived {
        target: TargetSlot,
        sequence: Option<EnvelopeSequence>,
        response: ResponseDiagnostic,
    },
    Transition {
        id: RequestId,
        target: TargetSlot,
        from: Lifecycle,
        to: Lifecycle,
    },
    WriteFinished {
        id: RequestId,
        transmission: TransmissionId,
        target: TargetSlot,
        cancellation: bool,
        envelope: EnvelopeKind,
        sequence: Option<u32>,
        success: bool,
    },
    RetryScheduled {
        id: RequestId,
        target: TargetSlot,
        attempt: u32,
        ready_at: Instant,
    },
    /// One of a request's own protocol deadlines expired, carrying the engine's
    /// actual retry decision so a subscriber never has to infer it from a
    /// `Transition` and the absence of a `RetryScheduled`.
    DeadlineExpired {
        id: RequestId,
        target: TargetSlot,
        deadline: DeadlineKind,
        will_retry: bool,
    },
    CancellationRecorded {
        id: RequestId,
        target: TargetSlot,
    },
    CancellationObserved {
        id: RequestId,
        target: TargetSlot,
        observation: CancellationDiagnostic,
    },
    AppliedState {
        id: RequestId,
        target: TargetSlot,
        key: StateKey,
    },
    Terminal {
        id: RequestId,
        target: TargetSlot,
        outcome: OutcomeDiagnostic,
    },
    SessionChanged {
        from: SessionState,
        to: SessionState,
        reason: ErrorKind,
    },
    Ignored(IgnoreReason),
}

#[derive(Debug, Default)]
pub(crate) struct TargetStateCache {
    values: crate::state_cache::PerStateKey<crate::state_cache::StateEntry>,
}

impl TargetStateCache {
    fn apply(&mut self, projection: AppliedStateProjection) {
        self.values[projection.key()] = match projection {
            AppliedStateProjection::Set(value) => crate::state_cache::StateEntry::Set(value),
            AppliedStateProjection::Invalidate { .. } => crate::state_cache::StateEntry::Unknown,
        };
    }

    pub(crate) fn entry(&self, key: StateKey) -> crate::state_cache::StateEntry {
        self.values[key]
    }
}

#[derive(Debug)]
struct PermitPoolInner {
    available: Mutex<PermitCounts>,
    capacity: usize,
    reserves: PerTarget<u8>,
}

/// Free admission slots: the ordinary budget, and each target's control
/// reserve (D26, #778).
#[derive(Debug)]
struct PermitCounts {
    ordinary: usize,
    reserve: PerTarget<u8>,
}

/// The admission limit covering boundary work and authoritative engine
/// entries. A permit moves from pending admission to the active record and is
/// released only when admission is rejected or a request reaches terminal.
///
/// Ordinary requests share `capacity` slots. Each registered target also has
/// a control reserve, one slot per typed STOP its profile supports, that only
/// an urgent request may take; such a request takes its target's reserve
/// first and an ordinary slot only when that reserve is held (D26, #778).
#[derive(Debug, Clone)]
pub(crate) struct AdmissionPermitPool(Arc<PermitPoolInner>);

impl AdmissionPermitPool {
    fn new(capacity: usize, reserves: PerTarget<u8>) -> Self {
        Self(Arc::new(PermitPoolInner {
            available: Mutex::new(PermitCounts {
                ordinary: capacity,
                reserve: reserves,
            }),
            capacity,
            reserves,
        }))
    }

    /// The ordinary admission capacity.
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.0.capacity
    }

    /// The most requests that can be pending or active at once: the ordinary
    /// capacity plus every target's control reserve.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn total_capacity(&self) -> usize {
        self.0
            .reserves
            .iter()
            .fold(self.0.capacity, |total, (_, reserve)| {
                total.saturating_add(usize::from(*reserve))
            })
    }

    /// Takes one admission slot for a request with `context`.
    ///
    /// An ordinary request that finds the ordinary budget full gets
    /// [`Error::RuntimeQueueFull`]. An urgent request that finds both its
    /// target's reserve and the ordinary budget full gets
    /// [`Error::ControlReserveExhausted`].
    pub(crate) fn try_acquire(&self, context: &RequestContext) -> Result<AdmissionPermit, Error> {
        let target = context.target;
        let mut available = self.0.available.lock().unwrap_or_else(|p| p.into_inner());
        let slot = if context.control.class == ControlClass::Urgent && available.reserve[target] > 0
        {
            available.reserve[target] -= 1;
            AdmissionSlot::ControlReserve
        } else if available.ordinary > 0 {
            available.ordinary -= 1;
            AdmissionSlot::Ordinary
        } else if context.control.class == ControlClass::Urgent {
            return Err(Error::ControlReserveExhausted {
                target: target.camera(),
                reserve: usize::from(self.0.reserves[target]),
            });
        } else {
            return Err(Error::RuntimeQueueFull {
                capacity: self.0.capacity,
            });
        };
        Ok(AdmissionPermit {
            pool: Arc::clone(&self.0),
            slot,
            target,
        })
    }

    /// Takes an ordinary slot, as any non-urgent request on camera 1 would.
    #[cfg(test)]
    pub(crate) fn try_acquire_ordinary(&self) -> Result<AdmissionPermit, Error> {
        let mut available = self.0.available.lock().unwrap_or_else(|p| p.into_inner());
        if available.ordinary == 0 {
            return Err(Error::RuntimeQueueFull {
                capacity: self.0.capacity,
            });
        }
        available.ordinary -= 1;
        Ok(AdmissionPermit {
            pool: Arc::clone(&self.0),
            slot: AdmissionSlot::Ordinary,
            target: TargetSlot::ALL[0],
        })
    }

    /// Free ordinary slots.
    #[cfg(test)]
    fn available(&self) -> usize {
        self.0
            .available
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .ordinary
    }
}

#[derive(Debug)]
pub(crate) struct AdmissionPermit {
    pool: Arc<PermitPoolInner>,
    slot: AdmissionSlot,
    target: TargetSlot,
}

impl AdmissionPermit {
    /// The admission budget this permit holds a slot in.
    pub(crate) const fn slot(&self) -> AdmissionSlot {
        self.slot
    }
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        let mut available = self
            .pool
            .available
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        match self.slot {
            AdmissionSlot::Ordinary => {
                debug_assert!(available.ordinary < self.pool.capacity);
                available.ordinary = available.ordinary.saturating_add(1).min(self.pool.capacity);
            }
            AdmissionSlot::ControlReserve => {
                let reserve = self.pool.reserves[self.target];
                let free = &mut available.reserve[self.target];
                debug_assert!(*free < reserve);
                *free = free.saturating_add(1).min(reserve);
            }
        }
    }
}

/// Reusable receive storage, consumed in source order and returned to the owner.
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) type FrameBatch = VecDeque<DecodedFrame>;

/// Closed receive verdict. Only the adapter classifies transport evidence;
/// the shell executes this decision without inspecting the error again.
#[derive(Debug)]
pub(crate) enum OwnerReceive {
    Frames(FrameBatch),
    NoProgress(NoProgressReason),
    Transient(Error),
    Fatal(ReceiveFatal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoProgressReason {
    EagerIdle,
    IdleTimeout,
    MalformedDatagram,
}

#[derive(Debug)]
pub(crate) enum ReceiveFatal {
    TransportClosed { cause: Option<Error> },
    FramingFailure { cause: Error },
}

impl OwnerReceive {
    /// A failed read consumed no bytes. Preserve the full original cause.
    pub(crate) fn read_error(error: Error) -> Self {
        enum Class {
            Idle,
            Fatal,
            Transient,
        }
        fn classify(error: &Error) -> Class {
            match error {
                Error::WithContext { source, .. } => classify(source),
                Error::TransportError { source, .. } => classify(source),
                Error::Timeout { .. } => Class::Idle,
                Error::Io(io)
                    if matches!(
                        io.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    Class::Idle
                }
                Error::Io(io) if crate::transport::FATAL_RECEIVE_IO_KINDS.contains(&io.kind()) => {
                    Class::Fatal
                }
                Error::RuntimeShutdown => Class::Fatal,
                error if error.requires_new_session() => Class::Fatal,
                _ => Class::Transient,
            }
        }
        match classify(&error) {
            Class::Idle => Self::NoProgress(NoProgressReason::EagerIdle),
            Class::Fatal => Self::Fatal(ReceiveFatal::TransportClosed { cause: Some(error) }),
            Class::Transient => Self::Transient(error),
        }
    }

    pub(crate) fn consumed_error(error: Error, transport: TransportKind) -> Self {
        match transport {
            TransportKind::Stream => Self::Fatal(ReceiveFatal::FramingFailure { cause: error }),
            TransportKind::Datagram => Self::NoProgress(NoProgressReason::MalformedDatagram),
        }
    }
}

/// The owner-side half of one bounded, one-shot observation slot.
///
/// The owner resolves it at most once. A handle holds the matching
/// [`Observer`]; dropping that receiver disconnects the channel, so the owner
/// can count an observation that nobody will read.
#[derive(Debug)]
pub(crate) struct ObserverCell<T> {
    resolved: AtomicBool,
    sender: flume::Sender<Observed<T>>,
}

/// A value and its authoritative owner-clock delivery instant.
#[derive(Debug, Clone)]
pub(crate) struct Observed<T> {
    pub(crate) at: Instant,
    pub(crate) value: T,
}

impl<T> Observed<T> {
    /// The observation deadline rule: a delivered value counts for a wait
    /// only if the owner delivered it no later than the wait's deadline. A
    /// value delivered exactly at the deadline still counts.
    pub(crate) fn within(&self, deadline: Instant) -> bool {
        self.at <= deadline
    }
}

/// The result of attempting to settle one receipt observer.
///
/// `ReceiverLost` is deliberately distinct from `AlreadyResolved`: only the
/// former means an observation was discarded because its receiver went away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObserverResolution {
    Delivered,
    ReceiverLost,
    AlreadyResolved,
}

impl<T> ObserverCell<T> {
    fn is_attached(&self) -> bool {
        !self.resolved.load(Ordering::Acquire) && !self.sender.is_disconnected()
    }

    #[cfg(test)]
    fn resolve(&self, observation: T) -> ObserverResolution {
        self.resolve_at(observation, Instant::now())
    }

    fn resolve_at(&self, observation: T, at: Instant) -> ObserverResolution {
        // A previous successful resolution wins even if the receiver was
        // subsequently dropped. That is a duplicate delivery attempt, not a
        // newly lost observer event.
        if self.resolved.load(Ordering::Acquire) {
            return ObserverResolution::AlreadyResolved;
        }
        if !self.is_attached() {
            return ObserverResolution::ReceiverLost;
        }
        if self
            .resolved
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return ObserverResolution::AlreadyResolved;
        }
        match self.sender.try_send(Observed {
            at,
            value: observation,
        }) {
            Ok(()) => ObserverResolution::Delivered,
            Err(flume::TrySendError::Disconnected(_)) => ObserverResolution::ReceiverLost,
            // A cell has one sender and is marked resolved before sending, so
            // this cannot arise from a second observer event. Do not misreport
            // it as receiver loss if that invariant is ever violated.
            Err(flume::TrySendError::Full(_)) => ObserverResolution::AlreadyResolved,
        }
    }
}

/// The handle-side half of one bounded, one-shot observation slot.
///
/// The handle holds only a weak reference to the owner-side cell, so the
/// owner's copy is the cell's only strong owner and dropping it unresolved
/// disconnects the slot: a wait then ends instead of hanging.
///
/// Dropping the observer is the detach operation: no actor message and no
/// protocol mutation are needed. A received value is never lost by an
/// abandoned wait: flume's receive future leaves an unread value queued when
/// it is dropped.
#[derive(Debug)]
pub(crate) struct Observer<T> {
    cell: Weak<ObserverCell<T>>,
    receiver: flume::Receiver<Observed<T>>,
}

impl<T> Observer<T> {
    /// Creates a slot. The returned cell must be installed where the
    /// observation is produced; dropping it unresolved disconnects the slot.
    pub(crate) fn pair() -> (Self, Arc<ObserverCell<T>>) {
        let (sender, receiver) = flume::bounded(1);
        let cell = Arc::new(ObserverCell {
            resolved: AtomicBool::new(false),
            sender,
        });
        let observer = Self {
            cell: Arc::downgrade(&cell),
            receiver,
        };
        (observer, cell)
    }

    /// The owner-side cell, while the owner still holds it.
    fn cell(&self) -> Option<Arc<ObserverCell<T>>> {
        self.cell.upgrade()
    }

    #[cfg(test)]
    fn try_recv(&self) -> Option<T> {
        self.try_observed().map(|observed| observed.value)
    }

    fn try_observed(&self) -> Option<Observed<T>> {
        self.receiver.try_recv().ok()
    }

    /// Waits for the value; `None` once the owner dropped its cell unresolved.
    #[cfg(feature = "async")]
    async fn recv_observed_async(&self) -> Option<Observed<T>> {
        self.receiver.recv_async().await.ok()
    }

    #[cfg(all(
        test,
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    async fn recv_async(&self) -> Option<T> {
        self.recv_observed_async()
            .await
            .map(|observed| observed.value)
    }

    /// The slot's receiver, for a blocking wait that selects over several
    /// slots. A receive error means the owner dropped its cell unresolved.
    #[cfg(feature = "blocking")]
    const fn receiver(&self) -> &flume::Receiver<Observed<T>> {
        &self.receiver
    }
}

/// Observes one request's authoritative terminal outcome.
pub(crate) type TerminalObserver = Observer<RuntimeOutcome>;

/// Observes a cancellation that failed *without* ending its operation: a
/// cancellation write that could not be sent, or a cancellation observation
/// deadline that expired. The operation's own terminal outcome is still
/// delivered through its [`TerminalObserver`] (#777).
pub(crate) type CancellationObserver = Observer<Error>;

/// One cancellation request for an admitted operation (#777).
///
/// The handle creates the cancellation observer and ships its owner-side half
/// with every request, so a retried or abandoned `cancel` observes the same
/// intent instead of creating a second one.
#[derive(Debug)]
pub(crate) struct CancellationRequest {
    pub(crate) id: RequestId,
    pub(crate) observer: Arc<ObserverCell<Error>>,
}

/// Exact, linear observation authority shared by the mode-specific receipt
/// wrappers. It is deliberately neither cloneable nor publicly nameable.
#[derive(Debug)]
pub(crate) struct ReceiptCore {
    motion: Option<MotionStamp>,
    pub(crate) id: RequestId,
    pub(crate) completion: TerminalObserver,
    pub(crate) configured_timeout: Duration,
}

impl ReceiptCore {
    fn admitted(admitted: Admitted, completion: TerminalObserver, timeout: Duration) -> Self {
        let mut core = Self::new(admitted.id, completion, timeout);
        core.motion = admitted.motion;
        core
    }

    fn new(id: RequestId, completion: TerminalObserver, configured_timeout: Duration) -> Self {
        Self {
            motion: None,
            id,
            completion,
            configured_timeout,
        }
    }

    const fn id(&self) -> RequestId {
        self.id
    }

    const fn configured_timeout(&self) -> Duration {
        self.configured_timeout
    }

    /// The verdict of a terminal outcome delivered to this receipt's wait
    /// ending at `deadline`: its value when it counts, else an observation
    /// timeout.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn conclude(
        &self,
        outcome: Observed<RuntimeOutcome>,
        deadline: Instant,
    ) -> Result<RuntimeOutcome, Error> {
        if outcome.within(deadline) {
            Ok(outcome.value)
        } else {
            Err(observation_timeout(self.id))
        }
    }

    /// The terminal outcome already delivered, if any, judged against
    /// `deadline`; `None` while the slot is still empty.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn try_conclude(&self, deadline: Instant) -> Option<Result<RuntimeOutcome, Error>> {
        self.completion
            .try_observed()
            .map(|outcome| self.conclude(outcome, deadline))
    }

    /// The terminal outcome already delivered within `deadline`, if any.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn observed_within(&self, deadline: Instant) -> Option<RuntimeOutcome> {
        self.completion
            .try_observed()
            .filter(|outcome| outcome.within(deadline))
            .map(|outcome| outcome.value)
    }

    // Consumed only by `async_actor::tests`, which additionally requires
    // `runtime-tokio` or `runtime-smol` (#636).
    #[cfg(all(
        test,
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    pub(crate) async fn terminal(&self) -> Result<RuntimeOutcome, Error> {
        self.completion
            .recv_async()
            .await
            .ok_or(Error::RuntimeShutdown)
    }
}

fn normalize_command_outcome(outcome: RuntimeOutcome) -> Result<(), Error> {
    match outcome {
        RuntimeOutcome::Written | RuntimeOutcome::Applied => Ok(()),
        RuntimeOutcome::Cancelled => Err(Error::CommandCanceled),
        RuntimeOutcome::Failed(error) => Err(error),
        RuntimeOutcome::Reply { .. } => Err(Error::InvalidState(
            "a command observer received an inquiry reply".into(),
        )),
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
        RuntimeOutcome::Written | RuntimeOutcome::Applied => Err(Error::InvalidState(
            "an inquiry observer received a command outcome".into(),
        )),
    }
}

#[derive(Debug)]
struct PendingAdmission {
    motion: Option<MotionStamp>,
    permit: AdmissionPermit,
    observer: Arc<ObserverCell<RuntimeOutcome>>,
    reply: flume::Sender<Result<Admitted, Error>>,
    summary: RequestSummary,
}

#[derive(Debug)]
struct ActiveRequest {
    _permit: AdmissionPermit,
    observer: Arc<ObserverCell<RuntimeOutcome>>,
    summary: RequestSummary,
    /// The one cancellation intent a handle installed (#777). It lives exactly
    /// as long as the request, so it adds no unbounded state.
    cancellation: Option<CancellationIntent>,
}

#[derive(Debug)]
enum CancellationIntent {
    Requested(Arc<ObserverCell<Error>>),
    Recorded(Arc<ObserverCell<Error>>),
}

/// What a cancellation tells its caller once the operation's terminal outcome
/// is known: `Completed` after a successful application, `Cancelled` after a
/// cancellation, and the failure itself after a failed operation.
pub(crate) fn cancellation_outcome(
    terminal: &RuntimeOutcome,
) -> Result<CancellationOutcome, Error> {
    match terminal {
        RuntimeOutcome::Cancelled => Ok(CancellationOutcome::Cancelled),
        RuntimeOutcome::Applied => Ok(CancellationOutcome::Completed),
        RuntimeOutcome::Failed(error) => Err(error.clone()),
        RuntimeOutcome::Written => Err(Error::InvalidState(
            "a local write outcome cannot authorize cancellation".into(),
        )),
        RuntimeOutcome::Reply { .. } => Err(Error::InvalidState(
            "an inquiry outcome cannot authorize cancellation".into(),
        )),
    }
}

/// The handle-side observation state of one admitted operation (#777).
///
/// Every wait borrows it, so an abandoned or timed-out wait releases only that
/// wait. The authoritative terminal outcome, a failed cancellation, and the
/// handle's own settlement verdict are cached here once received, so later
/// waits return them without touching the owner.
#[derive(Debug)]
pub(crate) struct OperationObservation {
    core: ReceiptCore,
    cancellation_timeout: Duration,
    terminal: Option<Observed<RuntimeOutcome>>,
    settled: Option<Result<crate::Settlement, Error>>,
    cancellation: Option<CancellationObserver>,
    cancellation_failure: Option<Observed<Error>>,
}

impl OperationObservation {
    pub(crate) fn new(core: ReceiptCore, cancellation_timeout: Duration) -> Self {
        Self {
            core,
            cancellation_timeout,
            terminal: None,
            settled: None,
            cancellation: None,
            cancellation_failure: None,
        }
    }

    pub(crate) const fn id(&self) -> RequestId {
        self.core.id()
    }

    /// The configured observer deadline for application.
    pub(crate) const fn applied_timeout(&self) -> Duration {
        self.core.configured_timeout()
    }

    /// The configured observer deadline for a cancellation's conclusion.
    pub(crate) const fn cancellation_timeout(&self) -> Duration {
        self.cancellation_timeout
    }

    /// Moves every value the owner already delivered into the cache.
    fn poll(&mut self) {
        if self.terminal.is_none() {
            self.terminal = self.core.completion.try_observed();
        }
        if self.cancellation_failure.is_none() {
            self.cancellation_failure = self
                .cancellation
                .as_ref()
                .and_then(CancellationObserver::try_observed);
        }
    }

    /// The application verdict, once the terminal outcome is known.
    pub(crate) fn applied(&mut self, deadline: Instant) -> Option<Result<(), Error>> {
        self.poll();
        self.terminal
            .as_ref()
            .filter(|event| event.within(deadline))
            .map(|event| normalize_command_outcome(event.value.clone()))
    }

    /// The cancellation verdict, once known. The terminal outcome always
    /// decides it, so the answer does not depend on the order in which the
    /// owner emits a cancellation failure and a later terminal outcome.
    pub(crate) fn cancellation(
        &mut self,
        deadline: Instant,
    ) -> Option<Result<CancellationOutcome, Error>> {
        self.poll();
        match self
            .terminal
            .as_ref()
            .filter(|event| event.within(deadline))
        {
            Some(terminal) => Some(cancellation_outcome(&terminal.value)),
            None => self
                .cancellation_failure
                .as_ref()
                .filter(|event| event.within(deadline))
                .map(|event| Err(event.value.clone())),
        }
    }

    /// The request to send for this handle's single cancellation intent.
    ///
    /// While the owner holds the intent's cell, the request carries that same
    /// cell, so the owner observes the existing intent. A cell the owner
    /// dropped belonged to a refused request, which installed no intent; the
    /// next request starts a fresh slot.
    pub(crate) fn cancellation_request(&mut self) -> CancellationRequest {
        self.poll();
        let observer = match self.cancellation.as_ref().and_then(Observer::cell) {
            Some(cell) => cell,
            None => {
                let (observer, cell) = Observer::pair();
                self.cancellation = Some(observer);
                cell
            }
        };
        CancellationRequest {
            id: self.core.id(),
            observer,
        }
    }

    pub(crate) fn record_terminal(&mut self, outcome: Observed<RuntimeOutcome>) {
        self.terminal.get_or_insert(outcome);
    }

    pub(crate) fn record_cancellation_failure(&mut self, error: Observed<Error>) {
        self.cancellation_failure.get_or_insert(error);
    }

    pub(crate) const fn terminal_observer(&self) -> &TerminalObserver {
        &self.core.completion
    }

    pub(crate) fn cancellation_observer(&self) -> Option<&CancellationObserver> {
        self.cancellation.as_ref()
    }

    pub(crate) fn settled(&self) -> Option<Result<crate::Settlement, Error>> {
        self.settled.clone()
    }

    pub(crate) fn check_settlement(&mut self) -> Result<(), Error> {
        if let Some(stamp) = &self.core.motion {
            if let Err(error) =
                stamp.check(crate::OperationId::from_nonzero(self.id().as_nonzero()))
            {
                self.settled = Some(Err(error.clone()));
                return Err(error);
            }
        }
        Ok(())
    }

    pub(crate) fn commit_settlement(
        &mut self,
        result: Result<crate::Settlement, Error>,
        polled: bool,
    ) -> Result<crate::Settlement, Error> {
        if polled {
            if let Some(stamp) = &self.core.motion {
                return stamp.commit(
                    crate::OperationId::from_nonzero(self.id().as_nonzero()),
                    result,
                    &mut self.settled,
                );
            }
        }
        if let Ok(evidence) = result {
            self.settled = Some(Ok(evidence));
        }
        result
    }
}

#[derive(Debug)]
pub(crate) struct DiagnosticSubscription {
    receiver: flume::Receiver<DiagnosticEvent>,
    lifecycle: Arc<boundary::OwnerLifecycle>,
    #[cfg(feature = "blocking")]
    clock: Clock,
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
            .map_err(|_| self.disconnected_error())
    }

    fn disconnected_error(&self) -> Error {
        self.lifecycle
            .terminal()
            .unwrap_or_else(boundary::missing_terminal_error)
    }

    #[cfg(feature = "blocking")]
    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<Option<DiagnosticEvent>, Error> {
        let received = match &self.clock {
            Clock::System => match self.receiver.recv_timeout(timeout) {
                Ok(event) => Some(Ok(event)),
                Err(flume::RecvTimeoutError::Timeout) => None,
                Err(flume::RecvTimeoutError::Disconnected) => {
                    Some(Err(flume::RecvError::Disconnected))
                }
            },
            #[cfg(any(test, feature = "test-utils"))]
            Clock::Manual(_) => {
                let deadline = self.clock.now().checked_add(timeout);
                let selection = self.clock.select().recv(&self.receiver, |event| event);
                match deadline {
                    Some(deadline) => selection.wait_until(deadline),
                    None => Some(selection.wait()),
                }
            }
        };
        received.transpose().map_err(|_| self.disconnected_error())
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
    frames: FrameBatch,
    /// Count of delimited-but-unclassifiable frames the last stream decode
    /// discarded as malformed (#672). It is a side channel from the shared
    /// decode, which owns the framer but not the owner's diagnostics, back to
    /// the owner: the owner takes it after each decode and records one
    /// `Ignored(MalformedFrame)` per discarded frame. A genuine framing-position
    /// loss stays a decode `Err` and still poisons; this counts only the frames
    /// a byte stream tolerated and kept running past, exactly as a datagram
    /// already discards a malformed frame.
    discarded_malformed: usize,
}

impl OwnerBuffers {
    pub(crate) fn new(limits: OwnerLimits) -> Result<Self, Error> {
        if limits.receive_bytes == 0 {
            return Err(Error::InvalidRequest(
                "owner receive buffer must be non-zero".into(),
            ));
        }
        Ok(Self {
            send: Vec::with_capacity(MAX_BYTES),
            transmit: BytesMut::with_capacity(MAX_BYTES.saturating_add(8)),
            receive: vec![0; limits.receive_bytes].into_boxed_slice(),
            frames: FrameBatch::with_capacity(limits.frames_per_receive),
            discarded_malformed: 0,
        })
    }

    /// Records how many malformed frames the current decode discarded. Called by
    /// the shared decode; overwrites (never accumulates) so a value only ever
    /// reflects the most recent decode.
    pub(crate) fn set_discarded_malformed(&mut self, count: usize) {
        self.discarded_malformed = count;
    }

    /// Takes and clears the malformed-frame discard count from the last decode.
    pub(crate) fn take_discarded_malformed(&mut self) -> usize {
        std::mem::take(&mut self.discarded_malformed)
    }

    /// Peeks the malformed-frame discard count without clearing it, so a drain
    /// pass can decide it made progress and hand the count on to the owner.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn discarded_malformed(&self) -> usize {
        self.discarded_malformed
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
            Transmission::Cancel { target, socket, .. } => {
                let max_size = <CommandCancelCommand as crate::Request>::MAX_SIZE;
                if max_size > self.send.capacity() {
                    return Err(Error::ResponseTooLarge {
                        max_size: self.send.capacity(),
                    });
                }
                self.send.resize(max_size, 0);
                let command = CommandCancelCommand::new(*socket);
                let len = command.write_into(target.camera(), &mut self.send)?;
                self.send.truncate(len);
            }
        }
        Ok((&self.send, &mut self.transmit))
    }

    pub(crate) fn receive_mut(&mut self) -> &mut [u8] {
        &mut self.receive
    }

    /// Lend the warmed batch storage to one receive operation.
    pub(crate) fn take_frames(&mut self) -> FrameBatch {
        let mut frames = std::mem::take(&mut self.frames);
        frames.clear();
        frames
    }

    /// Reuse the allocation after the shell has consumed the complete batch.
    pub(crate) fn recycle_frames(&mut self, mut frames: FrameBatch) {
        frames.clear();
        if frames.capacity() > self.frames.capacity() {
            self.frames = frames;
        }
    }
}

/// Borrowed exact write handed to a mode-native transport adapter.
#[derive(Debug)]
pub(crate) struct WireWrite<'a> {
    // Read only by the test wire drivers in `async_actor::tests` (#636).
    #[cfg(all(
        test,
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    pub(crate) request: RequestId,
    pub(crate) bytes: &'a [u8],
    /// The concrete encoder plan bound by the session wire authority.
    pub(crate) framing: &'a WriteFraming,
    /// Reusable framing destination owned by the session. Envelope adapters
    /// must frame into this buffer instead of allocating per transmission.
    pub(crate) frame_buffer: &'a mut BytesMut,
    // Read only by the test wire drivers in `async_actor::tests` (#636).
    #[cfg(all(
        test,
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    pub(crate) cancellation: bool,
    pub(crate) inquiry: bool,
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl WireWrite<'_> {
    /// Executes the encoder plan already bound by the session authority.
    pub(crate) fn frame(&mut self) -> Result<TransmissionMeta, Error> {
        self.framing
            .frame_into(self.bytes, self.command_kind(), self.frame_buffer)
    }

    /// The command kind this write is framed and sent as.
    pub(crate) const fn command_kind(&self) -> crate::command::CommandKind {
        if self.inquiry {
            crate::command::CommandKind::Inquiry
        } else {
            crate::command::CommandKind::Command
        }
    }
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
pub(crate) struct OwnerInputTurn(InputTurn, Instant);

impl StagedWrite {
    fn framing(&self) -> &WriteFraming {
        match &self.kind {
            Transmission::Request { framing, .. } | Transmission::Cancel { framing, .. } => framing,
        }
    }

    fn target(&self) -> TargetSlot {
        match self.kind {
            Transmission::Request { target, .. } | Transmission::Cancel { target, .. } => target,
        }
    }
}

/// Result of applying one non-I/O effect.
#[derive(Debug)]
pub(crate) enum AppliedEffect {
    None,
    // Payload read only by `tests::lifecycle_trace`'s rejection rendering
    // (#636).
    #[cfg(test)]
    AdmissionRejected(Error),
    Transmit(StagedWrite),
}

/// The owner's live operational tuning, shared with every handle that prepares
/// requests against it.
///
/// The owner is the sole writer: an update reaches it through the control
/// boundary of either owner shell (#631, #780). Readers therefore see one whole [`crate::OperationalTuning`] value —
/// never a half-applied mixture of two updates — and two concurrent updates
/// resolve last-writer-wins in the order the owner accepted them.
#[derive(Debug, Clone)]
pub(crate) struct LiveTuning(Arc<Mutex<crate::session_config::ValidatedTuning>>);

impl LiveTuning {
    fn new(tuning: crate::session_config::ValidatedTuning) -> Self {
        Self(Arc::new(Mutex::new(tuning)))
    }

    /// Copies the tuning currently governing request preparation.
    pub(crate) fn get(&self) -> crate::session_config::ValidatedTuning {
        *self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn set(&self, tuning: crate::session_config::ValidatedTuning) {
        *self.0.lock().unwrap_or_else(|poison| poison.into_inner()) = tuning;
    }
}

/// Evidence and a nonzero progress measure for one retained input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetainedInput {
    pub(crate) len: std::num::NonZeroUsize,
    pub(crate) evidence: RetainedEvidence,
}

/// Raw-prefix facts or explicit non-raw retention, never independent flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetainedEvidence {
    Complete,
    Malformed,
    Incomplete {
        target: TargetSlot,
        kind: crate::protocol::framer::RawIncompletePrefix,
    },
    NotRaw,
}

impl RetainedEvidence {
    fn raw(self) -> Option<RawPrefixEvidence> {
        match self {
            Self::Complete => Some(RawPrefixEvidence::Complete),
            Self::Malformed => Some(RawPrefixEvidence::Malformed),
            Self::Incomplete { target, kind } => {
                Some(RawPrefixEvidence::Incomplete { target, kind })
            }
            Self::NotRaw => None,
        }
    }
}

/// Infallible inspection of retained stream input. Only discarding mutates it.
pub(crate) trait RetainedStreamInput {
    fn retained(&self) -> Option<RetainedInput> {
        None
    }
    fn discard_first(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// How a due raw release may proceed once retained input is resolved.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawReleaseResolution {
    /// Release correlation and run due work now.
    Advance,
    /// Wait for a retained prefix's tail until this engine-owned deadline.
    AwaitInputUntil(Instant),
}

/// Common serialized owner state shared by blocking and async modes.
#[derive(Debug)]
pub(crate) struct OwnerState {
    engine: ProtocolEngine,
    observed_at: Instant,
    policy: OwnerPolicy,
    tuning: LiveTuning,
    permits: AdmissionPermitPool,
    pending: BTreeMap<AdmissionTicket, PendingAdmission>,
    active: BTreeMap<RequestId, ActiveRequest>,
    motion: Arc<MotionRegistry>,
    target_cache: Arc<PerTarget<Mutex<TargetStateCache>>>,
    diagnostic_subscribers: BTreeMap<u64, DiagnosticSubscriber>,
    // Read only by `subscribe_diagnostics`, whose consumers are feature-gated
    // (#636).
    #[cfg(any(feature = "async", feature = "blocking", test))]
    next_diagnostic_subscription: u64,
    next_ticket: u64,
    metrics: OwnerMetrics,
    buffers: OwnerBuffers,
}

impl OwnerState {
    pub(crate) fn new(policy: OwnerPolicy) -> Result<Self, Error> {
        if policy.limits.staged_diagnostics == 0
            || policy.limits.diagnostic_subscribers == 0
            || policy.limits.diagnostic_events_per_subscription == 0
            || policy.limits.control_requests == 0
            || policy.limits.frames_per_receive == 0
        {
            return Err(Error::InvalidRequest(
                "owner collection bounds must be non-zero".into(),
            ));
        }
        let mut engine = ProtocolEngine::new(policy.protocol.clone())?;
        for (target, target_policy) in policy.targets.iter() {
            if let Some(target_policy) = target_policy {
                engine.register_target(target, *target_policy)?;
            }
        }
        let permits = AdmissionPermitPool::new(
            policy.protocol.capacity,
            PerTarget::from_fn(|target| {
                policy.targets[target].map_or(0, |policy| policy.control_reserve)
            }),
        );
        let buffers = OwnerBuffers::new(policy.limits)?;
        let tuning = LiveTuning::new(policy.tuning);
        Ok(Self {
            engine,
            observed_at: Instant::now(),
            policy,
            tuning,
            permits,
            pending: BTreeMap::new(),
            active: BTreeMap::new(),
            motion: Arc::new(MotionRegistry::default()),
            target_cache: Arc::new(PerTarget::from_fn(|_| {
                Mutex::new(TargetStateCache::default())
            })),
            diagnostic_subscribers: BTreeMap::new(),
            #[cfg(any(feature = "async", feature = "blocking", test))]
            next_diagnostic_subscription: 1,
            next_ticket: 1,
            metrics: OwnerMetrics::default(),
            buffers,
        })
    }

    pub(crate) fn permits(&self) -> AdmissionPermitPool {
        self.permits.clone()
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
    /// The proof value was checked against every registered profile before
    /// crossing this boundary. Commit it to the live cell only after the
    /// engine accepts its pacing and capacity update.
    pub(crate) fn retune(
        &mut self,
        validated: crate::session_config::ValidatedTuning,
    ) -> Result<(), Error> {
        let tuning = validated.get();
        let baseline = self.policy.baseline;
        let command_spacing = tuning
            .command_spacing_override()
            .unwrap_or(baseline.command_spacing);
        let inquiry_spacing = tuning
            .inquiry_spacing_override()
            .unwrap_or(baseline.inquiry_spacing);
        let command_sockets: PerTarget<Option<u8>> = PerTarget::from_fn(|target| {
            baseline.command_sockets[target].map(|profile_sockets| {
                tuning
                    .maximum_command_sockets_override()
                    .unwrap_or(profile_sockets)
            })
        });
        self.engine
            .retune(command_spacing, inquiry_spacing, command_sockets)?;
        self.tuning.set(validated);
        Ok(())
    }

    pub(crate) const fn state(&self) -> SessionState {
        self.engine.state()
    }

    pub(crate) fn boundary_error(&self) -> Option<Error> {
        self.engine.terminal_error()
    }

    /// Whether `id` is still an active request: admitted, and its terminal
    /// outcome not yet delivered.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn is_active(&self, id: RequestId) -> bool {
        self.active.contains_key(&id)
    }

    // Used by owner tests and the test-only async snapshot helper (#636).
    #[cfg(test)]
    pub(crate) fn active_len(&self) -> usize {
        self.active.len()
    }

    // Used by owner tests and the test-only async snapshot helper (#636).
    #[cfg(test)]
    pub(crate) fn metrics(&self) -> OwnerMetrics {
        self.metrics
    }

    pub(crate) fn metrics_snapshot(&self, pending: usize) -> crate::observability::MetricsSnapshot {
        crate::observability::metrics_snapshot(
            self.metrics,
            self.active.len(),
            pending,
            self.state(),
        )
    }

    /// Records one request rejection before authoritative engine admission.
    pub(crate) fn record_admission_rejection(
        &mut self,
        target: TargetSlot,
        lane: Lane,
        error: &Error,
    ) {
        self.record_pre_admission_rejections(
            1,
            u64::from(matches!(error, Error::ControlReserveExhausted { .. })),
        );
        self.record_admission_rejection_diagnostic(target, lane, error.kind());
    }

    /// Records rejected work that never reached the serialized owner.
    ///
    /// Async handles can reject a submission while acquiring the shared
    /// admission permit, before an observer, ticket, or boundary item exists.
    /// Their bounded ingress is drained by the actor, which uses this method to
    /// merge the exact counter totals into owner-owned metrics. `control_reserve`
    /// counts the urgent stops among them that found their target's control
    /// reserve full (D26, #778).
    pub(crate) fn record_pre_admission_rejections(&mut self, count: u64, control_reserve: u64) {
        self.metrics.admission_rejected = self.metrics.admission_rejected.saturating_add(count);
        self.metrics.control_reserve_rejected = self
            .metrics
            .control_reserve_rejected
            .saturating_add(control_reserve);
    }

    /// Delivers the bounded diagnostic half of a pre-admission rejection.
    ///
    /// Keeping this separate from [`Self::record_pre_admission_rejections`]
    /// lets the async ingress account exactly even when several concurrent
    /// callers coalesce their wake-up into one bounded actor notification.
    pub(crate) fn record_admission_rejection_diagnostic(
        &mut self,
        target: TargetSlot,
        lane: Lane,
        error: ErrorKind,
    ) {
        self.record(DiagnosticEvent::AdmissionRejected {
            target,
            lane,
            error,
        });
    }

    /// Accounts for a bounded diagnostic ingress evicting an older event before
    /// the owner can deliver it to diagnostic subscribers.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn record_dropped_staged_diagnostics(&mut self, count: u64) {
        self.metrics.dropped_staged_diagnostics = self
            .metrics
            .dropped_staged_diagnostics
            .saturating_add(count);
    }

    pub(crate) fn state_cache_registry(&self) -> Arc<PerTarget<Mutex<TargetStateCache>>> {
        Arc::clone(&self.target_cache)
    }

    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.engine.next_wake()
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn raw_correlation_releases_due(&self, now: Instant) -> RawCorrelationReleaseSet {
        self.engine.raw_correlation_releases_due(now)
    }

    /// Resolve retained raw stream input at a due correlation release.
    ///
    /// Each retained fragment the engine assigns to the expiring scope is
    /// discarded, one at a time and with proven progress, so a stale fragment
    /// never crosses into a successor's correlation interval while later input
    /// is preserved. The result says whether due work may advance now or must
    /// wait for a tail until the engine's grace deadline. Both owner shells use
    /// this one loop (#776).
    ///
    /// Every error means the owner can no longer prove a safe release; the
    /// caller ends the session with a framing failure.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn resolve_retained_raw_input<I>(
        &mut self,
        input: &mut I,
        now: Instant,
    ) -> Result<RawReleaseResolution, Error>
    where
        I: RetainedStreamInput + ?Sized,
    {
        loop {
            let Some(retained) = input.retained() else {
                // No retained input clears the previous grace scope.
                let _ = self.engine.resolve_raw_release_gate(now, None);
                return Ok(RawReleaseResolution::Advance);
            };
            let Some(evidence) = retained.evidence.raw() else {
                let _ = self.engine.resolve_raw_release_gate(now, None);
                return Ok(RawReleaseResolution::Advance);
            };
            match self.engine.resolve_raw_release_gate(now, Some(evidence)) {
                RawReleaseGateAction::Advance => return Ok(RawReleaseResolution::Advance),
                RawReleaseGateAction::AwaitInputUntil(deadline) => {
                    return Ok(RawReleaseResolution::AwaitInputUntil(deadline));
                }
                RawReleaseGateAction::DiscardFirst => {
                    input.discard_first()?;
                    if input
                        .retained()
                        .is_some_and(|after| after.len >= retained.len)
                    {
                        return Err(Error::InvalidState(
                            "raw stream decoder did not consume the discarded prefix".into(),
                        ));
                    }
                    let _ = self.apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
                }
            }
        }
    }

    pub(crate) fn input(&mut self, input: Input, now: Instant) -> VecDeque<Effect> {
        self.observed_at = now;
        self.observe_input(&input);
        self.engine_input(input, |engine, input| engine.handle(input, now))
    }

    pub(crate) fn begin_input_turn(&self, now: Instant) -> OwnerInputTurn {
        OwnerInputTurn(self.engine.begin_input_turn(now), now)
    }

    /// Applies an input in an active decoded-input turn without running due
    /// deadlines or ordinary dispatch. Callers must drain the returned effects
    /// recursively before applying the next decoded frame.
    pub(crate) fn input_in_turn(
        &mut self,
        turn: &OwnerInputTurn,
        input: Input,
    ) -> VecDeque<Effect> {
        self.observed_at = turn.1;
        self.observe_input(&input);
        self.engine_input(input, |engine, input| engine.handle_in_turn(&turn.0, input))
    }

    fn engine_input(
        &mut self,
        input: Input,
        apply: impl FnOnce(&mut ProtocolEngine, Input) -> Vec<Effect>,
    ) -> VecDeque<Effect> {
        let admission = match &input {
            Input::Admit {
                ticket, request, ..
            } => request
                .context()
                .motion
                .map(|motion| (*ticket, *request.context(), motion)),
            _ => None,
        };
        if let Some((ticket, context, motion)) = admission {
            let registry = Arc::clone(&self.motion);
            let (effects, stamp) = registry.admit(
                context.target,
                motion.axes,
                context.submission_order,
                || apply(&mut self.engine, input),
            );
            if let Some(pending) = self.pending.get_mut(&ticket) {
                pending.motion = stamp;
            }
            effects.into()
        } else {
            apply(&mut self.engine, input).into()
        }
    }

    pub(crate) fn finish_input_turn(
        &mut self,
        turn: OwnerInputTurn,
        engine_turn: EngineTurn,
    ) -> VecDeque<Effect> {
        self.observed_at = turn.1;
        self.engine.finish_input_turn(turn.0, engine_turn).into()
    }

    fn observe_input(&mut self, input: &Input) {
        if let Input::Frame(frame) = input {
            self.metrics.received_frames = self.metrics.received_frames.saturating_add(1);
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
    }

    pub(crate) fn advance(&mut self, now: Instant) -> VecDeque<Effect> {
        self.observed_at = now;
        self.engine.advance(now).into()
    }

    pub(crate) fn finish_write(
        &mut self,
        staged: &StagedWrite,
        result: WriteOutcome,
        now: Instant,
    ) -> VecDeque<Effect> {
        self.finish_write_turn(staged, result, now, EngineTurn::COMPLETE)
    }

    pub(crate) fn finish_write_turn(
        &mut self,
        staged: &StagedWrite,
        result: WriteOutcome,
        now: Instant,
        turn: EngineTurn,
    ) -> VecDeque<Effect> {
        self.observed_at = now;
        self.observe_write(staged, &result);
        self.engine
            .handle_turn(
                Input::TransmissionFinished {
                    request: staged.request,
                    transmission: staged.transmission,
                    result,
                },
                now,
                turn,
            )
            .into()
    }

    /// Records and applies an identified write result in the same decoded-input
    /// turn that produced it, without allowing deadlines between source frames.
    pub(crate) fn finish_write_in_turn(
        &mut self,
        turn: &OwnerInputTurn,
        staged: &StagedWrite,
        result: WriteOutcome,
    ) -> VecDeque<Effect> {
        self.observed_at = turn.1;
        self.observe_write(staged, &result);
        self.engine
            .handle_in_turn(
                &turn.0,
                Input::TransmissionFinished {
                    request: staged.request,
                    transmission: staged.transmission,
                    result,
                },
            )
            .into()
    }

    fn observe_write(&mut self, staged: &StagedWrite, result: &WriteOutcome) {
        let (success, sequence) = match result {
            WriteOutcome::NotSent(_) => return,
            WriteOutcome::Sent(meta) => (true, meta.sequence()),
            WriteOutcome::Failed(_) => (false, None),
        };
        self.metrics.writes = self.metrics.writes.saturating_add(1);
        if !success {
            self.metrics.write_failures = self.metrics.write_failures.saturating_add(1);
        }
        self.record(DiagnosticEvent::WriteFinished {
            id: staged.request,
            transmission: staged.transmission,
            target: staged.target(),
            cancellation: matches!(staged.kind, Transmission::Cancel { .. }),
            envelope: self.policy.protocol.wire.kind(),
            sequence,
            success,
        });
    }

    fn stage_admission_with(
        &mut self,
        request: RuntimeRequest,
        permit: AdmissionPermit,
        observer: Arc<ObserverCell<RuntimeOutcome>>,
        reply: flume::Sender<Result<Admitted, Error>>,
    ) -> Input {
        let ticket = self.allocate_ticket();
        let summary = RequestSummary::new(&request);
        let slot = permit.slot();
        let previous = self.pending.insert(
            ticket,
            PendingAdmission {
                motion: None,
                permit,
                observer,
                reply,
                summary,
            },
        );
        debug_assert!(previous.is_none());
        Input::Admit {
            ticket,
            request,
            slot,
        }
    }

    fn mark_cancellation_recorded(&mut self, id: RequestId) {
        if let Some(intent) = self
            .active
            .get_mut(&id)
            .and_then(|active| active.cancellation.as_mut())
        {
            if let CancellationIntent::Requested(observer) = intent {
                *intent = CancellationIntent::Recorded(Arc::clone(observer));
            }
        }
    }

    /// A failed cancellation observation. Before the intent was recorded it is
    /// the engine refusing the request, which leaves the operation running and
    /// is answered to the cancelling caller; afterwards it is a cancellation
    /// that failed without ending the operation, which reaches the handle's
    /// cancellation observer. The operation's terminal slot is untouched
    /// either way (#777).
    fn cancellation_failed(&mut self, id: RequestId, error: Error) {
        let Some(active) = self.active.get_mut(&id) else {
            return;
        };
        match &active.cancellation {
            Some(CancellationIntent::Recorded(observer)) => {
                if matches!(
                    observer.resolve_at(
                        Error::cancellation_observation_failed(error),
                        self.observed_at
                    ),
                    ObserverResolution::ReceiverLost
                ) {
                    self.metrics.dropped_observer_events =
                        self.metrics.dropped_observer_events.saturating_add(1);
                }
            }
            Some(CancellationIntent::Requested(_)) => active.cancellation = None,
            None => {}
        }
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

    /// Installs the handle's one intent and returns the engine's synchronous
    /// refusal alongside the effects to drive. A missing or already-recorded
    /// request needs no new engine input.
    pub(crate) fn begin_cancellation(
        &mut self,
        request: CancellationRequest,
        now: Instant,
    ) -> (VecDeque<Effect>, Result<(), Error>) {
        let Some(active) = self.active.get_mut(&request.id) else {
            return (VecDeque::new(), Ok(()));
        };
        if active.cancellation.is_some() {
            return (VecDeque::new(), Ok(()));
        }
        active.cancellation = Some(CancellationIntent::Requested(request.observer));
        self.observed_at = now;
        let turn = self.engine.begin_input_turn(now);
        let (mut effects, answer) = self.engine.cancel_in_turn(&turn, request.id);
        if answer.is_err() {
            // A refusal records no intent; the original operation stays live.
            if let Some(active) = self.active.get_mut(&request.id) {
                active.cancellation = None;
            }
        }
        effects.extend(self.engine.finish_input_turn(turn, EngineTurn::COMPLETE));
        (effects.into(), answer)
    }

    #[cfg(any(feature = "async", feature = "blocking", test))]
    fn subscribe_diagnostics(
        &mut self,
        event_capacity: usize,
        lifecycle: Arc<boundary::OwnerLifecycle>,
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
            return Err(Error::DiagnosticSubscriberLimit {
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
        Ok(DiagnosticSubscription {
            receiver,
            lifecycle,
            #[cfg(feature = "blocking")]
            clock: Clock::System,
        })
    }

    pub(crate) fn validate_frame_batch(&self, frames: &FrameBatch) -> Result<(), Error> {
        if frames.len() > self.policy.limits.frames_per_receive {
            // A frame-count limit, not a byte-size limit: name frames rather than
            // reusing the "N bytes" `ResponseTooLarge` message. Since the shared
            // decode now stops draining at this same limit (#674), this is a
            // defense-in-depth guard on the batch the owner receives.
            return Err(Error::InvalidResponse {
                expected: format!(
                    "a receive within the per-receive limit of {} frames",
                    self.policy.limits.frames_per_receive
                )
                .into(),
                actual: Vec::new(),
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
        staged: &'a StagedWrite,
    ) -> Result<WireWrite<'a>, Error> {
        #[cfg(all(
            test,
            feature = "async",
            any(feature = "runtime-tokio", feature = "runtime-smol")
        ))]
        let cancellation = matches!(staged.kind, Transmission::Cancel { .. });
        let (inquiry, requires_extended_pan_tilt_speed) = match staged.kind {
            Transmission::Request {
                lane,
                requires_extended_pan_tilt_speed,
                ..
            } => (lane == Lane::Inquiry, requires_extended_pan_tilt_speed),
            Transmission::Cancel { .. } => (false, false),
        };
        if requires_extended_pan_tilt_speed {
            let cache = crate::state_cache::StateCache::from_registry(
                Arc::clone(&self.target_cache),
                staged.target(),
            );
            if cache.variable_speed_mode() != Some(crate::command::VariableSpeedMode::Fine50) {
                return Err(Error::InvalidRequest(format!(
                    "Sony FR7 speeds above 24 require an applied Fine50 variable-speed mode for camera {}",
                    staged.target().camera()).into()));
            }
        }
        let framing = staged.framing();
        let (bytes, frame_buffer) = self.buffers.prepare(&staged.kind)?;
        Ok(WireWrite {
            #[cfg(all(
                test,
                feature = "async",
                any(feature = "runtime-tokio", feature = "runtime-smol")
            ))]
            request: staged.request,
            bytes,
            framing,
            frame_buffer,
            #[cfg(all(
                test,
                feature = "async",
                any(feature = "runtime-tokio", feature = "runtime-smol")
            ))]
            cancellation,
            inquiry,
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
                    motion,
                    permit,
                    observer,
                    reply,
                    summary,
                } = pending;
                let permit_slot = permit.slot();
                self.active.insert(
                    id,
                    ActiveRequest {
                        _permit: permit,
                        observer,
                        summary,
                        cancellation: None,
                    },
                );
                self.metrics.admitted = self.metrics.admitted.saturating_add(1);
                if permit_slot == AdmissionSlot::ControlReserve {
                    self.metrics.control_reserve_admitted =
                        self.metrics.control_reserve_admitted.saturating_add(1);
                }
                self.record(DiagnosticEvent::Admitted {
                    id,
                    target: summary.target,
                    lane: summary.lane,
                    timeout: summary.timeout,
                    retry: summary.retry,
                    control: summary.control,
                    cancellation: summary.cancellation,
                });
                let _ = reply.try_send(Ok(Admitted { id, motion }));
                AppliedEffect::None
            }
            Effect::AdmissionRejected { ticket, error } => {
                if let Some(pending) = self.pending.remove(&ticket) {
                    self.record_admission_rejection(
                        pending.summary.target,
                        pending.summary.lane,
                        &error,
                    );
                    let _ = pending.reply.try_send(Err(error.clone()));
                    // Dropping pending releases the boundary/engine permit and
                    // disconnects its terminal observer without allocating an ID.
                } else {
                    self.record(DiagnosticEvent::Ignored(IgnoreReason::UnknownRequest));
                    self.metrics.admission_rejected =
                        self.metrics.admission_rejected.saturating_add(1);
                }
                #[cfg(test)]
                {
                    AppliedEffect::AdmissionRejected(error)
                }
                #[cfg(not(test))]
                {
                    AppliedEffect::None
                }
            }
            Effect::Transition { id, from, to } => {
                if let Some(target) = self.active.get(&id).map(|active| active.summary.target) {
                    self.record(DiagnosticEvent::Transition {
                        id,
                        target,
                        from,
                        to,
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
                self.mark_cancellation_recorded(id);
                if let Some(target) = self.active.get(&id).map(|active| active.summary.target) {
                    self.record(DiagnosticEvent::CancellationRecorded { id, target });
                }
                AppliedEffect::None
            }
            Effect::CancellationObservation { id, observation } => {
                let target = self.active.get(&id).map(|active| active.summary.target);
                let diagnostic = cancellation_diagnostic(&observation);
                match observation {
                    CancellationObservation::Recorded
                    | CancellationObservation::Cancelled
                    | CancellationObservation::Completed => self.mark_cancellation_recorded(id),
                    CancellationObservation::Failed(error) => {
                        self.cancellation_failed(id, error);
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
                // Cache mutation precedes terminal observation and diagnostic
                // delivery, independently of any observer staying attached.
                if let Ok(mut cache) = self.target_cache[effect.target].lock() {
                    cache.apply(effect.projection);
                }
                self.metrics.cache_updates = self.metrics.cache_updates.saturating_add(1);
                self.record(DiagnosticEvent::AppliedState {
                    id: effect.request,
                    target: effect.target,
                    key: effect.projection.key(),
                });
                AppliedEffect::None
            }
            Effect::Terminal { id, outcome } => {
                let diagnostic = outcome_diagnostic(&outcome);
                if let Some(active) = self.active.remove(&id) {
                    let target = active.summary.target;
                    if matches!(
                        active
                            .observer
                            .resolve_at(outcome.clone(), self.observed_at),
                        ObserverResolution::ReceiverLost
                    ) {
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
                self.metrics.terminal = self.metrics.terminal.saturating_add(1);
                AppliedEffect::None
            }
            Effect::SessionChanged { from, to } => {
                // The engine records the exact cause before emitting this
                // transition, including engine-initiated stream poison.
                let reason = self
                    .engine
                    .terminal_error()
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
                if reason == IgnoreReason::MalformedFrame {
                    self.metrics.ignored_malformed_frames =
                        self.metrics.ignored_malformed_frames.saturating_add(1);
                }
                self.record(DiagnosticEvent::Ignored(reason));
                AppliedEffect::None
            }
        }
    }

    // Consumed only by the async actor's boundary drain (#636).
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn fail_unstaged_boundary(&mut self, count: usize) {
        self.metrics.dropped_boundary_work = self
            .metrics
            .dropped_boundary_work
            .saturating_add(count as u64);
    }

    fn record(&mut self, event: DiagnosticEvent) {
        self.diagnostic_subscribers.retain(|_, subscriber| {
            if subscriber.sender.is_disconnected() {
                return false;
            }
            match subscriber.sender.try_send(event) {
                Ok(()) => {}
                Err(flume::TrySendError::Full(_)) => {
                    self.metrics.dropped_diagnostic_events =
                        self.metrics.dropped_diagnostic_events.saturating_add(1);
                }
                Err(flume::TrySendError::Disconnected(_)) => return false,
            }
            true
        });
    }

    // Used only by owner unit tests; production owners construct `Input::Frame`
    // inline from their decoded batches.
    #[cfg(test)]
    pub(crate) fn frame_input(frame: DecodedFrame) -> Input {
        Input::Frame(frame)
    }
}

// Private helper for the owner subscription paths.
#[cfg(any(feature = "async", feature = "blocking", test))]
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
        DecodedResponse::SonyControl { code } => ResponseDiagnostic::SonyControl { code: *code },
        DecodedResponse::NetworkChange => ResponseDiagnostic::NetworkChange,
        DecodedResponse::Unknown => ResponseDiagnostic::Unknown,
    }
}

fn outcome_diagnostic(outcome: &RuntimeOutcome) -> OutcomeDiagnostic {
    match outcome {
        RuntimeOutcome::Written => OutcomeDiagnostic::Written,
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

/// A caller's wait on `id` expired while the owner still holds the request
/// (D20, #783).
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) fn observation_timeout(id: RequestId) -> Error {
    Error::ObservationTimeout {
        operation: crate::OperationId::from_nonzero(id.as_nonzero()),
    }
}

/// A settlement wait on operation `id` that ran out of caller time, while
/// waiting for application or between position samples, is that operation's
/// observation timeout: the operation may still be moving.
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) fn settlement_error(error: Error, id: RequestId) -> Error {
    if error.is_caller_deadline() {
        observation_timeout(id)
    } else {
        Error::SettlementObservationFailed {
            operation: crate::OperationId::from_nonzero(id.as_nonzero()),
            source: Box::new(error),
        }
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
/// Both owners therefore normalize any [`Error::Timeout`] and the
/// non-fatal raw `WouldBlock` / `Interrupted` spellings to "this read produced
/// no frames" and keep pumping. A raw `io::ErrorKind::TimedOut` is different:
/// on a connected TCP socket it is the first error Linux commonly reports when
/// keepalive exhausts, so it remains a fatal receive fault rather than being
/// mistaken for an application-owned idle timer (#719).
/// Retains a fatal receive cause without nesting the public
/// `Connection closed:` display prefix inside a new `ConnectionClosed`.
///
/// Custom transports are allowed to report [`Error::ConnectionClosed`]
/// directly. Both owners still normalize that event through their common
/// shutdown input, so unwrap an existing close reason before constructing the
/// canonical boundary error. Context wrappers are retained without restoring
/// the redundant variant prefix (#729).
pub(crate) fn transport_close_reason(error: &Error) -> Option<Box<str>> {
    match error {
        Error::ConnectionClosed { reason } => reason.as_deref().map(Box::<str>::from),
        Error::WithContext { context, source } => {
            let reason = match transport_close_reason(source) {
                Some(source) => format!("{context}: {source}"),
                None => context.to_string(),
            };
            Some(reason.into_boxed_str())
        }
        _ => Some(error.to_string().into_boxed_str()),
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
    fn transport_failure(error: &Error) -> bool {
        match error {
            Error::Io(_)
            | Error::ConnectionFailed { .. }
            | Error::ConnectionClosed { .. }
            | Error::StreamPoisoned { .. }
            | Error::OwnerTerminated
            | Error::RuntimeShutdown
            | Error::TransportError { .. } => true,
            Error::WithContext { source, .. } => transport_failure(source),
            _ => false,
        }
    }
    if transport_failure(&error) {
        Error::transport_error(error)
    } else {
        error
    }
}

/// Prepend recursively produced effects while preserving their source order.
pub(crate) fn prepend_effects(queue: &mut VecDeque<Effect>, produced: VecDeque<Effect>) {
    for effect in produced.into_iter().rev() {
        queue.push_front(effect);
    }
}
