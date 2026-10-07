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
#[cfg(any(feature = "async", feature = "blocking"))]
mod handle;
#[cfg(any(feature = "async", feature = "blocking"))]
mod receipt;
#[cfg(any(feature = "async", feature = "blocking"))]
mod shell;
mod turn;

#[cfg(feature = "async")]
#[allow(unused_imports)]
pub(crate) use async_actor::*;
#[cfg(feature = "blocking")]
#[allow(unused_imports)]
pub(crate) use blocking::*;

#[cfg(feature = "async")]
pub(crate) use adapter::AsyncTransportAdapter;
#[cfg(feature = "blocking")]
pub(crate) use adapter::BlockingTransportAdapter;
#[allow(unused_imports)]
pub(crate) use adapter::{
    decode_response_target, owner_policy_for_targets_with_tuning, profile_supports_transport,
    validate_profile_transport, OwnerEnvelope, RoutingState, TargetRegistry,
};

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
    array,
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
    StateKey,
};
use crate::{raw::MAX_BYTES, CameraId, CancellationOutcome, Error, ErrorKind, ViscaSocket};

use super::engine::{
    AdmissionSlot, AdmissionTicket, AppliedStateEffect, AppliedStateProjection, CancelState,
    CancellationObservation, CancellationPolicy, ControlClass, DeadlineKind, DecodedFrame,
    DecodedResponse, Effect, EngineTurn, EnvelopeKind, EnvelopeSequence, IgnoreReason, Input,
    InputTurn, Lane, Phase, ProtocolEngine, ProtocolPolicy, RequestContext, RequestId, RetryPolicy,
    RuntimeOutcome, RuntimeRequest, SessionState, ShutdownReason, TargetPolicy, TimeoutPolicy,
    Transmission, TransmissionId, TransmissionMeta,
};

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
        // The read and write bounds start from the one transport default
        // (#799); the production builder lowers the caller's configured
        // values over them (#675).
        let transport = crate::transport::TransportConfig::default();
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
            read_timeout: transport.read_timeout,
            write_timeout: transport.write_timeout,
        })
    }

    // Test-only today: production goes through
    // `adapter::owner_policy_for_targets_with_tuning`.
    #[cfg(test)]
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
    target: CameraId,
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
        target: CameraId,
        lane: Lane,
        timeout: TimeoutPolicy,
        retry: RetryPolicy,
        control: ControlClass,
        cancellation: CancellationPolicy,
    },
    AdmissionRejected {
        target: CameraId,
        lane: Lane,
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
        key: StateKey,
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
    values: BTreeMap<StateKey, CachedProjection>,
    order: VecDeque<(StateKey, u64)>,
    generation: u64,
}

impl TargetStateCache {
    fn apply(&mut self, projection: AppliedStateProjection, capacity: usize) {
        if capacity == 0 {
            return;
        }
        // The cache stores the public semantic discriminator, not a
        // profile-specific wire variant. `PanTiltLimitCorner` has only the
        // two Standard VISCA values documented for this opcode: down-left
        // (`0x00`) and up-right (`0x01`). A limit set and a limit clear are
        // both local to one corner, so each must name a valid one; refuse a
        // malformed projection before it can replace a previously known limit
        // update.
        let invalid_limit_corner = match projection {
            AppliedStateProjection::Set {
                key: StateKey::PanTiltLimits,
                value,
            }
            | AppliedStateProjection::Clear {
                key: StateKey::PanTiltLimits,
                value,
            } => !matches!(
                value.values[0..value.value_count as usize].first(),
                Some(0 | 1)
            ),
            _ => false,
        };
        if invalid_limit_corner {
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

    fn get(&self, state: StateKey) -> Option<AppliedStateProjection> {
        self.values.get(&state).map(|value| value.projection)
    }

    pub(crate) fn entry(&self, state: StateKey) -> crate::state_cache::StateEntry {
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
    available: Mutex<PermitCounts>,
    capacity: usize,
    reserves: [u8; 9],
}

/// Free admission slots: the ordinary budget, and each target's control
/// reserve (D26, #778).
#[derive(Debug)]
struct PermitCounts {
    ordinary: usize,
    reserve: [u8; 9],
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
    fn new(capacity: usize, reserves: [u8; 9]) -> Self {
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
            .fold(self.0.capacity, |total, reserve| {
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
        let index = usize::from(target.id());
        let mut available = self.0.available.lock().unwrap_or_else(|p| p.into_inner());
        let slot = if context.control.class == ControlClass::Urgent && available.reserve[index] > 0
        {
            available.reserve[index] -= 1;
            AdmissionSlot::ControlReserve
        } else if available.ordinary > 0 {
            available.ordinary -= 1;
            AdmissionSlot::Ordinary
        } else if context.control.class == ControlClass::Urgent {
            return Err(Error::ControlReserveExhausted {
                target,
                reserve: usize::from(self.0.reserves[index]),
            });
        } else {
            return Err(Error::RuntimeQueueFull {
                capacity: self.0.capacity,
            });
        };
        Ok(AdmissionPermit {
            pool: Arc::clone(&self.0),
            slot,
            target: index,
            released: false,
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
            target: 1,
            released: false,
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
    target: usize,
    released: bool,
}

impl AdmissionPermit {
    /// The admission budget this permit holds a slot in.
    pub(crate) const fn slot(&self) -> AdmissionSlot {
        self.slot
    }
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

/// Outcome of one owner driver receive.
///
/// The distinction is load bearing on byte-stream transports: only a zero-length
/// transport read means the peer closed. A read that carried bytes but did not
/// finish a frame decodes to an empty batch, and the owner must keep pumping so
/// the remainder of the frame can arrive in a later read.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
pub(crate) enum OwnerReceive {
    /// The transport reported end of stream, i.e. a zero-length read.
    Closed,
    /// The read carried bytes and decoded to zero or more complete frames, in
    /// source order. An empty batch means the chunk only advanced a partially
    /// received frame; the transport is still open.
    Frames(Vec<DecodedFrame>),
    /// The read reported that no bytes arrived — an expired idle read timeout,
    /// which is how a custom transport implements a non-blocking read. Nothing
    /// was consumed and nothing failed: the owner keeps the session, keeps the
    /// framing state, and does not touch any request's retry budget (#625).
    NoData,
    /// The transport read itself failed and consumed nothing, so framing state
    /// is intact. The owner classifies the error: a transient fault retries
    /// in-flight work and keeps the session, a fatal one ends it.
    ///
    /// This is deliberately distinct from `Err`, which the driver reserves for
    /// a framing or decode failure over bytes that were already consumed.
    Fault(Error),
}

/// The owner-side half of one bounded, one-shot observation slot.
///
/// The owner resolves it at most once. A handle holds the matching
/// [`Observer`]; dropping that observer marks the cell detached, so the owner
/// can count an observation that nobody will read.
#[derive(Debug)]
pub(crate) struct ObserverCell<T> {
    detached: AtomicBool,
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
        !self.detached.load(Ordering::Acquire)
            && !self.resolved.load(Ordering::Acquire)
            && !self.sender.is_disconnected()
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
            detached: AtomicBool::new(false),
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

impl<T> Drop for Observer<T> {
    fn drop(&mut self) {
        if let Some(cell) = self.cell.upgrade() {
            cell.detached.store(true, Ordering::Release);
        }
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
    pub(crate) target: CameraId,
    pub(crate) completion: TerminalObserver,
    pub(crate) configured_timeout: Duration,
}

impl ReceiptCore {
    fn admitted(
        admitted: Admitted,
        target: CameraId,
        completion: TerminalObserver,
        timeout: Duration,
    ) -> Self {
        let mut core = Self::new(admitted.id, target, completion, timeout);
        core.motion = admitted.motion;
        core
    }

    fn new(
        id: RequestId,
        target: CameraId,
        completion: TerminalObserver,
        configured_timeout: Duration,
    ) -> Self {
        Self {
            motion: None,
            id,
            target,
            completion,
            configured_timeout,
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
struct CancellationIntent {
    observer: Arc<ObserverCell<Error>>,
    recorded: bool,
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

    pub(crate) const fn target(&self) -> CameraId {
        self.core.target()
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
            if let Err(error) = stamp.check(crate::OperationId::from_raw(self.id().get())) {
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
                    crate::OperationId::from_raw(self.id().get()),
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

/// Bounded applied-state event delivered after the target-local cache changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedStateEvent(pub(crate) AppliedStateEffect);

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct AppliedStateSubscription {
    receiver: flume::Receiver<AppliedStateEvent>,
}

#[cfg(test)]
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

    #[cfg(feature = "blocking")]
    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<Option<DiagnosticEvent>, Error> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(flume::RecvTimeoutError::Timeout) => Ok(None),
            Err(flume::RecvTimeoutError::Disconnected) => Err(Error::RuntimeShutdown),
        }
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
                let len = command.write_into(*target, &mut self.send)?;
                self.send.truncate(len);
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

    pub(crate) fn consume_framing(&mut self, count: usize) {
        let count = count.min(self.framing.len());
        self.framing.drain(..count);
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
    /// Sequence requested by the engine. `None` lets Sony framing allocate a
    /// fresh identity; an explicit value is a retry of the same logical wire
    /// message. Raw framing rejects an explicit value.
    pub(crate) requested_sequence: Option<u32>,
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
    pub(crate) envelope: EnvelopeKind,
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl WireWrite<'_> {
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
    pub(crate) requested_sequence: Option<u32>,
}

/// Owner boundary token for one ordered decoded-input batch.
///
/// The engine token is intentionally opaque here so blocking and async owners
/// cannot manufacture a different timestamp for recursive write completions.
#[derive(Debug)]
pub(crate) struct OwnerInputTurn(InputTurn, Instant);

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

/// Retained byte-stream input as both owner shells see it at a raw
/// correlation release (#776). Datagram and stateless drivers retain nothing,
/// which the defaults describe.
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) trait RetainedStreamInput {
    /// Whether the stream framer retains input not yet delivered to the
    /// engine.
    fn has_buffered_stream_input(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// A monotonic measure of the first retained input. Production adapters
    /// return the framer's exact byte count; the default suits a
    /// single-fragment test seam. A successful discard must reduce it (or
    /// remove it), which lets the owner prove progress without a turn cap.
    fn buffered_stream_input_len(&mut self) -> Result<Option<usize>, Error> {
        Ok(self.has_buffered_stream_input()?.then_some(1))
    }

    /// Evidence visible in the first retained raw input. A complete frame
    /// defers to the ordinary decode path; an incomplete prefix is classified
    /// only from its first two bytes, just far enough for the engine to decide
    /// whether it belongs to an expiring correlation scope. `None` means no
    /// input is retained.
    fn buffered_raw_prefix_evidence(&mut self) -> Result<Option<RawPrefixEvidence>, Error> {
        Ok(None)
    }

    /// Discard exactly the first retained raw frame or incomplete fragment,
    /// preserving all later input.
    fn discard_buffered_stream_input(&mut self) -> Result<(), Error> {
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
    /// A cancellation the engine refused while handling the request that is
    /// being driven right now; read back by `conclude_cancellation`.
    cancellation_refusal: Option<(RequestId, Error)>,
    motion: Arc<MotionRegistry>,
    target_cache: Arc<[Mutex<TargetStateCache>; 9]>,
    subscribers: BTreeMap<u64, AppliedSubscriber>,
    diagnostic_subscribers: BTreeMap<u64, DiagnosticSubscriber>,
    // Read only by `subscribe_applied`, whose consumers are feature-gated
    // (#636).
    #[cfg(test)]
    next_subscription: u64,
    // Read only by `subscribe_diagnostics`, whose consumers are feature-gated
    // (#636).
    #[cfg(any(feature = "async", feature = "blocking", test))]
    next_diagnostic_subscription: u64,
    next_ticket: u64,
    diagnostics: VecDeque<DiagnosticEvent>,
    metrics: OwnerMetrics,
    buffers: OwnerBuffers,
    session_error: Option<Error>,
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
        let permits = AdmissionPermitPool::new(
            policy.protocol.capacity,
            policy
                .targets
                .map(|target| target.map_or(0, |target| target.control_reserve)),
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
            cancellation_refusal: None,
            motion: Arc::new(MotionRegistry::default()),
            target_cache: Arc::new(array::from_fn(|_| Mutex::new(TargetStateCache::default()))),
            subscribers: BTreeMap::new(),
            diagnostic_subscribers: BTreeMap::new(),
            #[cfg(test)]
            next_subscription: 1,
            #[cfg(any(feature = "async", feature = "blocking", test))]
            next_diagnostic_subscription: 1,
            next_ticket: 1,
            diagnostics: VecDeque::new(),
            metrics: OwnerMetrics::default(),
            buffers,
            session_error: None,
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
    /// The caller is responsible for validating `tuning` against the registered
    /// profiles first — the owner does not retain `ProfileSpec` values, and the
    /// session facades perform those profile-safety checks before the update
    /// reaches this point.
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

    pub(crate) fn metrics_snapshot(&self) -> crate::observability::MetricsSnapshot {
        crate::observability::metrics_snapshot(
            self.metrics,
            self.active.len(),
            self.pending.len(),
            self.state(),
        )
    }

    /// Records one request rejection before authoritative engine admission.
    pub(crate) fn record_admission_rejection(
        &mut self,
        target: CameraId,
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
        target: CameraId,
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
    /// the owner can place it in its public diagnostic ring.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn record_dropped_diagnostics(&mut self, count: u64) {
        self.metrics.dropped_diagnostics = self.metrics.dropped_diagnostics.saturating_add(count);
    }

    pub(crate) fn state_cache_registry(&self) -> Arc<[Mutex<TargetStateCache>; 9]> {
        Arc::clone(&self.target_cache)
    }

    // Used by owner tests and the test-only async snapshot helper (#636).
    #[cfg(test)]
    pub(crate) fn diagnostics(&self) -> impl Iterator<Item = &DiagnosticEvent> {
        self.diagnostics.iter()
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
            let buffered = input.has_buffered_stream_input()?;
            let evidence = input.buffered_raw_prefix_evidence()?;
            if buffered != evidence.is_some() {
                return Err(Error::InvalidState(
                    if buffered {
                        "retained raw stream input could not be classified before correlation release"
                    } else {
                        "stream decoder described absent buffered input"
                    }
                    .into(),
                ));
            }
            match self.engine.resolve_raw_release_gate(now, evidence) {
                RawReleaseGateAction::Advance => return Ok(RawReleaseResolution::Advance),
                RawReleaseGateAction::AwaitInputUntil(deadline) => {
                    return Ok(RawReleaseResolution::AwaitInputUntil(deadline));
                }
                RawReleaseGateAction::DiscardFirst if !buffered => {
                    return Err(Error::InvalidState(
                        "raw release gate requested a discard without retained input".into(),
                    ));
                }
                RawReleaseGateAction::DiscardFirst => {
                    let before = input.buffered_stream_input_len()?.ok_or_else(|| {
                        Error::InvalidState(
                            "stream decoder reported buffered input without a progress measure"
                                .into(),
                        )
                    })?;
                    input.discard_buffered_stream_input()?;
                    if input
                        .buffered_stream_input_len()?
                        .is_some_and(|after| after >= before)
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
        if self.session_error.is_none() {
            self.session_error = boundary_error_for_input(input);
        }
    }

    pub(crate) fn advance(&mut self, now: Instant) -> VecDeque<Effect> {
        self.observed_at = now;
        self.engine.advance(now).into()
    }

    pub(crate) fn finish_write(
        &mut self,
        staged: &StagedWrite,
        result: Result<TransmissionMeta, Error>,
        now: Instant,
    ) -> VecDeque<Effect> {
        self.finish_write_turn(staged, result, now, EngineTurn::COMPLETE)
    }

    pub(crate) fn finish_write_turn(
        &mut self,
        staged: &StagedWrite,
        result: Result<TransmissionMeta, Error>,
        now: Instant,
        turn: EngineTurn,
    ) -> VecDeque<Effect> {
        self.observed_at = now;
        self.observe_write(staged, &result);
        self.engine
            .handle_turn(
                Input::TransmissionFinished {
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
        result: Result<TransmissionMeta, Error>,
    ) -> VecDeque<Effect> {
        self.observed_at = turn.1;
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
            intent.recorded = true;
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
            Some(intent) if intent.recorded => {
                if matches!(
                    intent.observer.resolve_at(error, self.observed_at),
                    ObserverResolution::ReceiverLost
                ) {
                    self.metrics.dropped_observer_events =
                        self.metrics.dropped_observer_events.saturating_add(1);
                }
            }
            Some(_) => {
                active.cancellation = None;
                self.cancellation_refusal = Some((id, error));
            }
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

    /// Installs a handle's cancellation intent and returns the engine input to
    /// drive, or `None` when there is nothing to send (#777):
    ///
    /// - the request is no longer active, so its terminal outcome has already
    ///   been delivered to the handle;
    /// - the handle's intent is already installed, so cancelling again
    ///   observes it instead of sending a second cancellation.
    pub(crate) fn begin_cancellation(&mut self, request: CancellationRequest) -> Option<Input> {
        self.cancellation_refusal = None;
        let active = self.active.get_mut(&request.id)?;
        if active.cancellation.is_some() {
            return None;
        }
        active.cancellation = Some(CancellationIntent {
            observer: request.observer,
            recorded: false,
        });
        Some(Input::Cancel { id: request.id })
    }

    /// The answer to a cancellation request once its input has been driven:
    /// the engine's refusal, if it refused, else success.
    pub(crate) fn conclude_cancellation(&mut self, id: RequestId) -> Result<(), Error> {
        match self.cancellation_refusal.take() {
            Some((refused, error)) if refused == id => Err(error),
            _ => Ok(()),
        }
    }

    // Consumed by the async actor's `Subscribe` control arm and the owner
    // lifecycle fixture (#636).
    #[cfg(test)]
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
        Ok(AppliedStateSubscription { receiver })
    }

    #[cfg(any(feature = "async", feature = "blocking", test))]
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
        Ok(DiagnosticSubscription { receiver })
    }

    pub(crate) fn validate_frame_batch(&self, frames: &[DecodedFrame]) -> Result<(), Error> {
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
        staged: &StagedWrite,
    ) -> Result<WireWrite<'a>, Error> {
        #[cfg(all(
            test,
            feature = "async",
            any(feature = "runtime-tokio", feature = "runtime-smol")
        ))]
        let cancellation = matches!(staged.kind, Transmission::Cancel { .. });
        let inquiry = self
            .active
            .get(&staged.request)
            .is_some_and(|active| active.summary.lane == Lane::Inquiry);
        let (bytes, frame_buffer) = self.buffers.prepare(&staged.kind)?;
        Ok(WireWrite {
            #[cfg(all(
                test,
                feature = "async",
                any(feature = "runtime-tokio", feature = "runtime-smol")
            ))]
            request: staged.request,
            bytes,
            requested_sequence: staged.requested_sequence,
            frame_buffer,
            #[cfg(all(
                test,
                feature = "async",
                any(feature = "runtime-tokio", feature = "runtime-smol")
            ))]
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
            } => {
                let requested_sequence = match &kind {
                    Transmission::Request {
                        requested_sequence, ..
                    }
                    | Transmission::Cancel {
                        requested_sequence, ..
                    } => *requested_sequence,
                };
                AppliedEffect::Transmit(StagedWrite {
                    transmission,
                    request,
                    kind,
                    requested_sequence,
                })
            }
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
                    {
                        match subscriber.sender.try_send(event) {
                            Ok(()) => {}
                            Err(flume::TrySendError::Full(_)) => {
                                self.metrics.dropped_applied_events =
                                    self.metrics.dropped_applied_events.saturating_add(1);
                            }
                            Err(flume::TrySendError::Disconnected(_)) => return false,
                        }
                    }
                    true
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
                // Issue #680: an engine-initiated terminal transition (deadline
                // expiry, the strict `strict_unconfirmed_poison` opt-in, a
                // stream/framing self-poison) reaches the owner only as this
                // payload-free effect — it does not pass through `observe_input`
                // with a `Close`/`Poison`/`Shutdown` input, so `session_error`
                // would otherwise stay `None`. Learn the engine's actual
                // terminal error on the first non-`Running` transition and latch
                // it, so `boundary_error()` (hence `shutdown()`/`close()` and
                // `set_tuning`) surfaces the true cause with the correct
                // `requires_new_session()` verdict instead of masking it as
                // `RuntimeShutdown`, and so the diagnostic `reason` below is the
                // real `ErrorKind` rather than `Other`. An owner-supplied
                // boundary input already set `session_error` first, so the
                // `is_none()` guard leaves that verdict untouched.
                if self.session_error.is_none() && !matches!(to, SessionState::Running) {
                    self.session_error = self.engine.terminal_error();
                }
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
        if self.diagnostics.len() == self.policy.limits.diagnostics {
            self.diagnostics.pop_front();
            self.metrics.dropped_diagnostics = self.metrics.dropped_diagnostics.saturating_add(1);
        }
        self.diagnostics.push_back(event);
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

fn boundary_error_for_input(input: &Input) -> Option<Error> {
    match input {
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
        | Input::ReceiveFault { .. } => None,
        Input::Wake => None,
    }
}

/// A caller's wait on `id` expired while the owner still holds the request
/// (D20, #783).
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) fn observation_timeout(id: RequestId) -> Error {
    Error::ObservationTimeout {
        operation: crate::OperationId::from_raw(id.get()),
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
            operation: crate::OperationId::from_raw(id.get()),
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
pub(crate) fn receive_reported_no_data(error: &Error) -> bool {
    match error {
        Error::Timeout { .. } => true,
        Error::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ),
        Error::WithContext { source, .. } => receive_reported_no_data(source),
        _ => false,
    }
}

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
    if error.requires_new_session() {
        Error::TransportError(format!("datagram send failed: {error}").into())
    } else {
        error
    }
}

/// Whether one receive-side transport failure leaves the session usable.
///
/// A `ConnectionClosed` read ends the session, and every other read error is a
/// transient fault that retries in-flight work where that is safe and keeps
/// the loop alive. The remaining fatal cases are stated explicitly instead of
/// being inherited from [`ErrorKind`], which cannot separate "the peer went away" from "this read
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
        // position is unknowable, or the owner's transport is gone.
        error if error.requires_new_session() => false,
        Error::RuntimeShutdown => false,
        // A raw I/O failure is fatal only when the operating system reported
        // that this connection itself is dead.
        Error::Io(io) => !matches!(
            io.kind(),
            std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::InvalidInput
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
