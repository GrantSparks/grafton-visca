//! Serialized owners for the deterministic protocol engine.
//!
//! The engine is the only lifecycle-policy authority.  This module owns the
//! bounded resources around it (admission, observers, caches, diagnostics and
//! reusable I/O storage) and exposes small mode-native driver seams.

mod active_request;
mod adapter;
use active_request::{ActiveRequest, RetiredRequest};
#[cfg(any(feature = "async", feature = "blocking"))]
mod halt;
mod lane;
pub(crate) use lane::LaneRequest;
mod motion;
use crate::observability::DiagnosticEvent;
mod diagnostics;

use motion::{Admitted, AdmittedOperation, MotionRegistry, MotionStamp};
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
#[cfg(feature = "test-utils")]
mod turn_probe;
#[cfg(feature = "test-utils")]
pub(crate) use turn_probe::TurnProbe;
#[cfg(all(feature = "test-utils", feature = "async"))]
mod select_probe;
#[cfg(all(feature = "test-utils", feature = "async"))]
pub(crate) use select_probe::SelectProbe;

#[cfg(feature = "async")]
pub(crate) use async_actor::*;
#[cfg(feature = "blocking")]
pub(crate) use blocking::*;
#[cfg(feature = "blocking")]
pub(crate) use clock::Clock;

#[cfg(feature = "async")]
pub(crate) use adapter::AsyncTransportAdapter;
#[cfg(feature = "blocking")]
pub(crate) use adapter::BlockingTransportAdapter;
#[cfg(feature = "test-utils")]
pub(crate) use adapter::ReceiveProbe;
#[cfg(all(feature = "test-utils", feature = "blocking"))]
pub(crate) use adapter::{decode_response_target, RoutingState, TargetRegistry};
pub(crate) use adapter::{validate_profile_registry_topology, validate_profile_transport};

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

#[cfg(all(test, feature = "runtime-tokio"))]
use std::sync::atomic::AtomicUsize;
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
    command::{encode::WireEncode, system_wire::CommandCancelCommand},
    target::{PerTarget, TargetSlot},
    StateKey,
};
use crate::{raw::MAX_BYTES, CancellationOutcome, Error, ErrorKind};

use super::engine::{
    AdmissionSlot, AdmittedId, AppliedStateProjection, CancellationConclusion,
    CancellationObservation, CommandOutcome, CommandRequestId, DeadlineKind, DecodedFrame,
    DecodedResponse, Effect, EngineTurn, IgnoreReason, Input, InputTurn, InquiryOutcome, Lane,
    Lifecycle, MotionEffect, ProtocolEngine, ProtocolPolicy, RequestId, RuntimeOutcome,
    RuntimeRequest, SessionState, ShutdownReason, TargetPolicy, Transmission, TransmissionId,
    TransmissionMeta, TransportKind, WriteFraming, WriteOutcome,
};

#[cfg(test)]
use super::engine::{CancelState, FrameSequence, Phase};

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
                to: crate::observability::DiagnosticPhase::Sending,
                ..
            } => Some(CanonicalOwnerStep::Sending),
            DiagnosticEvent::WriteFinished {
                cancellation: false,
                ..
            } => Some(CanonicalOwnerStep::RequestWrite),
            DiagnosticEvent::Transition {
                to: crate::observability::DiagnosticPhase::AwaitingAck,
                ..
            } => Some(CanonicalOwnerStep::AwaitingAck),
            DiagnosticEvent::FrameReceived {
                response: crate::observability::DiagnosticResponse::Ack,
                ..
            } => Some(CanonicalOwnerStep::Ack),
            DiagnosticEvent::Transition {
                to: crate::observability::DiagnosticPhase::Executing,
                ..
            } => Some(CanonicalOwnerStep::Executing),
            DiagnosticEvent::FrameReceived {
                response: crate::observability::DiagnosticResponse::Completion,
                ..
            } => Some(CanonicalOwnerStep::Completion),
            DiagnosticEvent::Terminal {
                outcome: crate::observability::DiagnosticOutcome::Applied,
                ..
            } => Some(CanonicalOwnerStep::Applied),
            _ => None,
        })
        .collect()
}

/// Bounds for every owner-side collection and reusable byte store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OwnerLimits {
    pub(crate) staged_diagnostics: std::num::NonZeroUsize,
    pub(crate) control_requests: std::num::NonZeroUsize,
    pub(crate) diagnostic_subscribers: std::num::NonZeroUsize,
    pub(crate) diagnostic_events_per_subscription: std::num::NonZeroUsize,
    pub(crate) frames_per_receive: std::num::NonZeroUsize,
    pub(crate) receive_bytes: std::num::NonZeroUsize,
}

impl Default for OwnerLimits {
    // Each literal is a compile-time positive collection bound.
    #[allow(clippy::unwrap_used)]
    fn default() -> Self {
        Self {
            staged_diagnostics: const { std::num::NonZeroUsize::new(128).unwrap() },
            control_requests: const { std::num::NonZeroUsize::new(16).unwrap() },
            diagnostic_subscribers: const { std::num::NonZeroUsize::new(4).unwrap() },
            diagnostic_events_per_subscription: const { std::num::NonZeroUsize::new(128).unwrap() },
            frames_per_receive: const { std::num::NonZeroUsize::new(64).unwrap() },
            receive_bytes: const { std::num::NonZeroUsize::new(4_096).unwrap() },
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
    timing: crate::profiles::ProfileTiming,
    pub(crate) command_spacing: Duration,
    pub(crate) inquiry_spacing: Duration,
    pub(crate) command_sockets: PerTarget<Option<crate::CommandSocketCount>>,
}

impl TuningBaseline {
    /// Retains immutable profile facts before any operational override is applied.
    pub(crate) fn from_registry(profiles: &PerTarget<Option<&crate::ProfileSpec>>) -> Self {
        let timing = profiles
            .iter()
            .find_map(|(_, profile)| profile.map(crate::ProfileSpec::timing))
            .unwrap_or_else(|| crate::profiles::ProfileId::GenericVisca.raw_facts().timing);
        let mut baseline = Self {
            timing,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            command_sockets: PerTarget::default(),
        };
        for (target, profile) in profiles.iter() {
            if let Some(profile) = profile {
                baseline.command_spacing = baseline
                    .command_spacing
                    .max(profile.timing().command_spacing());
                baseline.inquiry_spacing = baseline
                    .inquiry_spacing
                    .max(profile.timing().inquiry_spacing());
                baseline.command_sockets[target] = Some(profile.maximum_command_sockets());
            }
        }
        baseline
    }

    /// The same lowering is used at owner construction and on every retune.
    fn apply(
        self,
        tuning: crate::session_config::ValidatedTuning,
    ) -> (
        Duration,
        Duration,
        PerTarget<Option<crate::CommandSocketCount>>,
    ) {
        // Aggregate profile pacing is immutable, just like target-local
        // ProfileTiming. Feed it through the same checked override resolution.
        let mut timing = self.timing;
        timing.command_spacing = self.command_spacing;
        timing.inquiry_spacing = self.inquiry_spacing;
        let resolved = tuning.resolve(&timing);
        (
            resolved.command_spacing,
            resolved.inquiry_spacing,
            PerTarget::from_fn(|target| {
                self.command_sockets[target]
                    .map(|sockets| resolved.maximum_command_sockets(sockets))
            }),
        )
    }

    #[cfg(test)]
    fn for_test(protocol: &ProtocolPolicy, targets: &PerTarget<Option<TargetPolicy>>) -> Self {
        Self {
            timing: crate::profiles::ProfileId::GenericVisca.raw_facts().timing,
            command_spacing: protocol.command_spacing,
            inquiry_spacing: protocol.inquiry_spacing,
            command_sockets: PerTarget::from_fn(|target| {
                targets[target].map(|policy| policy.command_sockets)
            }),
        }
    }
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
    pub(crate) read_timeout: crate::timeout::NonZeroDuration,
    /// Maximum time one transport write may take before the owner
    /// abandons it as a stalled write (#675). A byte-stream write that cannot
    /// be confirmed poisons the session; a datagram write fails only its own
    /// request. Without this bound a stalled peer parks the actor and
    /// `close()` never returns.
    pub(crate) write_timeout: crate::timeout::NonZeroDuration,
}

/// Immutable owner facts retained after construction policy is distributed.
/// Tunable values belong to the engine and `LiveTuning` alone.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OwnerFacts {
    pub(crate) limits: OwnerLimits,
    pub(crate) read_timeout: crate::timeout::NonZeroDuration,
    pub(crate) write_timeout: crate::timeout::NonZeroDuration,
    pub(crate) baseline: TuningBaseline,
    #[cfg(test)]
    pub(crate) transport: TransportKind,
}

impl OwnerPolicy {
    /// Creates one immutable policy from target-local protocol facts.
    #[allow(clippy::expect_used)] // Shared transport defaults are fixed positive intervals.
    pub(crate) fn with_targets(
        protocol: ProtocolPolicy,
        targets: PerTarget<Option<TargetPolicy>>,
        tuning: crate::session_config::ValidatedTuning,
        baseline: TuningBaseline,
    ) -> Self {
        // The read and write bounds start from the one transport default
        // (#799); the production builder lowers the caller's configured
        // values over them (#675).
        let transport = crate::transport::TransportConfig::default();
        Self {
            protocol,
            targets,
            limits: OwnerLimits::default(),
            tuning,
            baseline,
            read_timeout: crate::timeout::NonZeroDuration::new(transport.read_timeout)
                .expect("default transport read timeout is positive"),
            write_timeout: crate::timeout::NonZeroDuration::new(transport.write_timeout)
                .expect("default transport write timeout is positive"),
        }
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
        let baseline = TuningBaseline::for_test(&protocol, &targets);
        Ok(Self::with_targets(
            protocol,
            targets,
            crate::session_config::ValidatedTuning::for_test(crate::OperationalTuning::new())?,
            baseline,
        ))
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
    pub(crate) cache_invalidations: u64,
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
    pub(crate) ignored_reply_shape_mismatches: u64,
    pub(crate) dropped_staged_diagnostics: u64,
    pub(crate) dropped_diagnostic_events: u64,
    pub(crate) dropped_observer_events: u64,
    pub(crate) dropped_boundary_work: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestSummary {
    target: TargetSlot,
    lane: Lane,
}

impl RequestSummary {
    fn new(request: &RuntimeRequest) -> Self {
        Self {
            target: request.context().target,
            lane: request.lane(),
        }
    }
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

    /// Takes one admission slot for a fully prepared request.
    ///
    /// An ordinary request that finds the ordinary budget full gets
    /// [`Error::RuntimeQueueFull`]. An urgent request that finds both its
    /// target's reserve and the ordinary budget full gets
    /// [`Error::ControlReserveExhausted`].
    pub(crate) fn try_acquire(&self, request: &RuntimeRequest) -> Result<AdmissionPermit, Error> {
        let target = request.context().target;
        let mut available = self.0.available.lock().unwrap_or_else(|p| p.into_inner());
        let slot = if request.is_stop() && available.reserve[target] > 0 {
            available.reserve[target] -= 1;
            AdmissionSlot::ControlReserve
        } else if available.ordinary > 0 {
            available.ordinary -= 1;
            AdmissionSlot::Ordinary
        } else if request.is_stop() {
            return Err(Error::ControlReserveExhausted {
                target: target.individual_camera(),
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
#[derive(Debug, Default)]
pub(crate) struct FrameBatch {
    frames: VecDeque<DecodedFrame>,
    limit: usize,
    pub(crate) discarded: usize,
}

impl FrameBatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn with_capacity(capacity: std::num::NonZeroUsize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity.get()),
            limit: capacity.get(),
            discarded: 0,
        }
    }

    /// Borrow a vacant slot before decoding consumes any stream bytes.
    fn vacancy(&mut self) -> Option<FrameVacancy<'_>> {
        (self.frames.len() < self.limit).then_some(FrameVacancy(self))
    }

    /// Refuse a frame beyond the batch's construction-time bound in probes.
    #[cfg(any(test, feature = "test-utils"))]
    fn try_push(&mut self, frame: DecodedFrame) -> Result<(), DecodedFrame> {
        if self.frames.len() >= self.limit {
            return Err(frame);
        }
        self.frames.push_back(frame);
        Ok(())
    }

    pub(crate) fn pop_front(&mut self) -> Option<DecodedFrame> {
        self.frames.pop_front()
    }

    fn clear(&mut self) {
        self.frames.clear();
        self.discarded = 0;
    }
}

/// Exclusive loan proving the batch has space for one decoded frame.
struct FrameVacancy<'a>(&'a mut FrameBatch);

impl FrameVacancy<'_> {
    fn fill(self, frame: DecodedFrame) {
        self.0.frames.push_back(frame);
    }
}

impl std::ops::Deref for FrameBatch {
    type Target = VecDeque<DecodedFrame>;
    fn deref(&self) -> &Self::Target {
        &self.frames
    }
}
#[cfg(test)]
impl From<Vec<DecodedFrame>> for FrameBatch {
    fn from(frames: Vec<DecodedFrame>) -> Self {
        Self {
            limit: frames.len(),
            frames: frames.into(),
            discarded: 0,
        }
    }
}
#[cfg(test)]
impl From<VecDeque<DecodedFrame>> for FrameBatch {
    fn from(frames: VecDeque<DecodedFrame>) -> Self {
        Self {
            limit: frames.len(),
            frames,
            discarded: 0,
        }
    }
}

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
    FramingFailure { cause: Error, discarded: usize },
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
                Error::Io { source: io }
                    if matches!(
                        io.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    Class::Idle
                }
                Error::Io { source: io }
                    if crate::transport::FATAL_RECEIVE_IO_KINDS.contains(&io.kind()) =>
                {
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

    #[cfg(test)]
    pub(crate) fn consumed_error(error: Error, transport: TransportKind) -> Self {
        Self::consumed_error_with_discards(error, transport, 0)
    }

    pub(crate) fn consumed_error_with_discards(
        error: Error,
        transport: TransportKind,
        discarded: usize,
    ) -> Self {
        match transport {
            TransportKind::Stream => Self::Fatal(ReceiveFatal::FramingFailure {
                cause: error,
                discarded,
            }),
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
pub(crate) type TerminalObserver<T = CommandOutcome> = Observer<T>;

/// The owner retains a lane-specific terminal cell for each admitted request.
#[derive(Debug)]
pub(crate) enum TerminalSlot {
    Command(Arc<ObserverCell<CommandOutcome>>),
    Inquiry(Arc<ObserverCell<InquiryOutcome>>),
    /// The heterogeneous internal test driver observes complete engine effects.
    #[cfg(test)]
    Test(Arc<ObserverCell<RuntimeOutcome>>),
}

pub(crate) trait TerminalLane: Clone + Send + std::fmt::Debug + 'static {
    type Id: Copy + Send + std::fmt::Debug;

    fn identity(admitted: AdmittedId) -> Result<Self::Id, Error>;
    fn request_id(id: Self::Id) -> RequestId;
    fn slot(cell: Arc<ObserverCell<Self>>) -> TerminalSlot;
}
impl TerminalLane for CommandOutcome {
    type Id = CommandRequestId;

    fn identity(admitted: AdmittedId) -> Result<Self::Id, Error> {
        match admitted {
            AdmittedId::Command(id) => Ok(id),
            AdmittedId::Inquiry(_) => Err(Error::InvalidState {
                reason: "command receipt received an inquiry identity".into(),
            }),
        }
    }

    fn request_id(id: Self::Id) -> RequestId {
        id.request_id()
    }

    fn slot(cell: Arc<ObserverCell<Self>>) -> TerminalSlot {
        TerminalSlot::Command(cell)
    }
}
impl TerminalLane for InquiryOutcome {
    type Id = RequestId;

    fn identity(admitted: AdmittedId) -> Result<Self::Id, Error> {
        match admitted {
            AdmittedId::Inquiry(id) => Ok(id),
            AdmittedId::Command(_) => Err(Error::InvalidState {
                reason: "inquiry receipt received a command identity".into(),
            }),
        }
    }

    fn request_id(id: Self::Id) -> RequestId {
        id
    }

    fn slot(cell: Arc<ObserverCell<Self>>) -> TerminalSlot {
        TerminalSlot::Inquiry(cell)
    }
}
#[cfg(test)]
impl TerminalLane for RuntimeOutcome {
    type Id = AdmittedId;

    fn identity(admitted: AdmittedId) -> Result<Self::Id, Error> {
        Ok(admitted)
    }

    fn request_id(id: Self::Id) -> RequestId {
        id.request_id()
    }

    fn slot(cell: Arc<ObserverCell<Self>>) -> TerminalSlot {
        TerminalSlot::Test(cell)
    }
}
impl<T: TerminalLane> From<Arc<ObserverCell<T>>> for TerminalSlot {
    fn from(cell: Arc<ObserverCell<T>>) -> Self {
        T::slot(cell)
    }
}
impl TerminalSlot {
    fn resolve_at(self, outcome: RuntimeOutcome, at: Instant) -> ObserverResolution {
        match (self, outcome) {
            (Self::Command(cell), RuntimeOutcome::Command(outcome)) => cell.resolve_at(outcome, at),
            (Self::Inquiry(cell), RuntimeOutcome::Inquiry(outcome)) => cell.resolve_at(outcome, at),
            #[cfg(test)]
            (Self::Test(cell), outcome) => cell.resolve_at(outcome, at),
            // Typed staging prevents a mismatch at the admission boundary.
            // If an internal producer ever violates that invariant, resolve
            // the actual observer with an error instead of unwinding the owner.
            (Self::Command(cell), RuntimeOutcome::Inquiry(_)) => {
                cell.resolve_at(CommandOutcome::Failed(terminal_lane_mismatch()), at)
            }
            (Self::Inquiry(cell), RuntimeOutcome::Command(_)) => {
                cell.resolve_at(InquiryOutcome::Failed(terminal_lane_mismatch()), at)
            }
        }
    }
}

fn terminal_lane_mismatch() -> Error {
    Error::InvalidState {
        reason: "terminal outcome lane disagrees with its admitted request".into(),
    }
}

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
    pub(crate) id: CommandRequestId,
    pub(crate) observer: Arc<ObserverCell<Error>>,
}

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
impl ReceiptCore<CommandOutcome> {
    fn command_id(&self) -> CommandRequestId {
        self.identity
    }
}

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
impl ReceiptCore<RuntimeOutcome> {
    #[allow(clippy::panic)]
    fn command_id(&self) -> CommandRequestId {
        match self.identity {
            AdmittedId::Command(id) => id,
            AdmittedId::Inquiry(_) => panic!("test cancellation requires an admitted command"),
        }
    }
}

/// Exact, linear observation authority shared by the mode-specific receipt
/// wrappers. It is deliberately neither cloneable nor publicly nameable.
#[derive(Debug)]
pub(crate) struct ReceiptCore<T: TerminalLane = CommandOutcome> {
    pub(crate) identity: T::Id,
    pub(crate) completion: TerminalObserver<T>,
    pub(crate) configured_timeout: Duration,
}

impl<T: TerminalLane> ReceiptCore<T> {
    fn admitted(
        admitted: Admitted,
        completion: TerminalObserver<T>,
        configured_timeout: Duration,
    ) -> Result<Self, Error> {
        Ok(Self {
            identity: T::identity(admitted.identity)?,
            completion,
            configured_timeout,
        })
    }

    pub(crate) fn id(&self) -> RequestId {
        T::request_id(self.identity)
    }

    const fn configured_timeout(&self) -> Duration {
        self.configured_timeout
    }

    /// The verdict of a terminal outcome delivered to this receipt's wait
    /// ending at `deadline`: its value when it counts, else an observation
    /// timeout.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn conclude(&self, outcome: Observed<T>, deadline: Instant) -> Result<T, Error> {
        if outcome.within(deadline) {
            Ok(outcome.value)
        } else {
            Err(observation_timeout(self.id()))
        }
    }

    /// The terminal outcome already delivered, if any, judged against
    /// `deadline`; `None` while the slot is still empty.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn try_conclude(&self, deadline: Instant) -> Option<Result<T, Error>> {
        self.completion
            .try_observed()
            .map(|outcome| self.conclude(outcome, deadline))
    }

    /// The terminal outcome already delivered within `deadline`, if any.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn observed_within(&self, deadline: Instant) -> Option<T> {
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
    pub(crate) async fn terminal(&self) -> Result<T, Error> {
        self.completion
            .recv_async()
            .await
            .ok_or(Error::RuntimeShutdown)
    }
}

fn application_outcome(verdict: Result<CancellationOutcome, Error>) -> Result<(), Error> {
    match verdict? {
        CancellationOutcome::Completed => Ok(()),
        CancellationOutcome::Cancelled => Err(Error::CommandCancelled),
    }
}

fn normalize_command_outcome(outcome: CommandOutcome) -> Result<(), Error> {
    match outcome {
        CommandOutcome::Written | CommandOutcome::Applied => Ok(()),
        CommandOutcome::Cancelled => Err(Error::CommandCancelled),
        CommandOutcome::Failed(error) => Err(error),
    }
}

fn normalize_inquiry_outcome<R>(
    outcome: InquiryOutcome,
    decoder: &crate::ResponseDecoder<R>,
) -> Result<R, Error> {
    match outcome {
        InquiryOutcome::Reply { payload } => decoder.decode(&payload),
        InquiryOutcome::Failed(error) => Err(error),
    }
}

/// What a cancellation tells its caller once the operation's terminal outcome
/// is known: `Completed` after a successful application, `Cancelled` after a
/// cancellation, and the failure itself after a failed operation.
pub(crate) fn cancellation_outcome(
    terminal: &CommandOutcome,
) -> Result<CancellationOutcome, Error> {
    terminal.verdict()
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
    motion: MotionStamp,
    cancellation_timeout: Duration,
    terminal: Option<Observed<CommandOutcome>>,
    settled: Option<Result<crate::Settlement, Error>>,
    cancellation: Option<CancellationObserver>,
    cancellation_failure: Option<Observed<Error>>,
}

impl OperationObservation {
    pub(crate) fn new(
        core: ReceiptCore,
        motion: MotionStamp,
        cancellation_timeout: Duration,
    ) -> Self {
        Self {
            core,
            motion,
            cancellation_timeout,
            terminal: None,
            settled: None,
            cancellation: None,
            cancellation_failure: None,
        }
    }

    #[cfg(test)]
    fn command_id(&self) -> CommandRequestId {
        self.core.identity
    }

    pub(crate) const fn id(&self) -> RequestId {
        self.core.identity.request_id()
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
            .map(|event| application_outcome(cancellation_outcome(&event.value)))
    }

    /// The cancellation verdict, once known. An on-time terminal outcome
    /// takes precedence. The cancellation slot answers only when the
    /// cancellation attempt ended while the operation was still live.
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
            id: self.core.identity,
            observer,
        }
    }

    pub(crate) fn record_terminal(&mut self, outcome: Observed<CommandOutcome>) {
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
        self.motion
            .check(crate::OperationId::from_nonzero(self.id().as_nonzero()))
            .inspect_err(|error| self.settled = Some(Err(error.clone())))
    }

    pub(crate) fn commit_position_observation(
        &mut self,
        result: Result<crate::Settlement, Error>,
    ) -> Result<crate::Settlement, Error> {
        self.motion.commit(
            crate::OperationId::from_nonzero(self.id().as_nonzero()),
            result,
            &mut self.settled,
        )
    }

    pub(crate) fn commit_completion(&mut self, evidence: crate::Settlement) -> crate::Settlement {
        self.settled = Some(Ok(evidence));
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct DiagnosticSubscription {
    receiver: flume::Receiver<DiagnosticEvent>,
    lifecycle: Arc<boundary::OwnerLifecycle>,
    #[cfg(feature = "blocking")]
    clock: Clock,
    #[cfg(feature = "blocking")]
    worker_id: Option<std::thread::ThreadId>,
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
    pub(crate) fn recv_blocking(&self) -> Result<DiagnosticEvent, Error> {
        self.off_worker("diagnostics recv")?;
        self.clock
            .select()
            .recv(&self.receiver, |event| event)
            .wait()
            .map_err(|_| self.disconnected_error())
    }

    #[cfg(feature = "blocking")]
    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<Option<DiagnosticEvent>, Error> {
        if !timeout.is_zero() {
            self.off_worker("diagnostics recv_with_timeout")?;
        }
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

    #[cfg(feature = "blocking")]
    fn off_worker(&self, call: &str) -> Result<(), Error> {
        if self.worker_id == Some(std::thread::current().id()) {
            return Err(Error::InvalidState {
                reason: format!("blocking owner worker cannot call {call} on its own session")
                    .into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DiagnosticSubscriber {
    sender: flume::Sender<DiagnosticEvent>,
}

/// Bounds proven once when the transport policy is constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReceiveBounds {
    pub(crate) frames: std::num::NonZeroUsize,
    pub(crate) bytes: std::num::NonZeroUsize,
}

/// Reusable wire, receive and effect storage, sized from checked bounds.
#[derive(Debug)]
pub(crate) struct OwnerBuffers {
    bounds: ReceiveBounds,
    send: Vec<u8>,
    transmit: BytesMut,
    receive: Box<[u8]>,
    frames: FrameBatch,
    /// Lent to each engine decision and kept warm between turns.
    effect_scratch: Vec<Effect>,
    /// Lent to the mode-native drive loop and returned before continuation.
    effects: VecDeque<Effect>,
}

impl OwnerBuffers {
    pub(crate) fn new(limits: OwnerLimits) -> Self {
        let bounds = ReceiveBounds {
            frames: limits.frames_per_receive,
            bytes: limits.receive_bytes,
        };
        Self {
            bounds,
            send: Vec::with_capacity(MAX_BYTES),
            transmit: BytesMut::with_capacity(MAX_BYTES.saturating_add(8)),
            receive: vec![0; bounds.bytes.get()].into_boxed_slice(),
            frames: FrameBatch::with_capacity(bounds.frames),
            effect_scratch: Vec::new(),
            effects: VecDeque::new(),
        }
    }

    pub(crate) fn receive_bounds(&self) -> ReceiveBounds {
        self.bounds
    }

    /// Lend the warmed drive queue after appending this decision's effects.
    fn take_effects(&mut self) -> VecDeque<Effect> {
        let mut effects = std::mem::take(&mut self.effects);
        effects.extend(self.effect_scratch.drain(..));
        effects
    }

    /// A continuation may reuse the queue only after its drive has finished.
    pub(super) fn recycle_effects(&mut self, mut effects: VecDeque<Effect>) {
        effects.clear();
        if effects.capacity() > self.effects.capacity() {
            self.effects = effects;
        }
    }

    /// Recursive write/expiry results must precede effects already queued.
    fn prepend_effects(&mut self, effects: &mut VecDeque<Effect>) {
        for effect in self.effect_scratch.drain(..).rev() {
            effects.push_front(effect);
        }
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
        frames.limit = self.bounds.frames.get();
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
pub(crate) struct OwnerInputTurn(InputTurn);

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
#[must_use = "transmissions and session ends must be driven by the owner shell"]
#[derive(Debug)]
pub(crate) enum AppliedEffect {
    None,
    // Payload read only by `tests::lifecycle_trace`'s rejection rendering
    // (#636).
    #[cfg(test)]
    AdmissionRejected(Error),
    Transmit(StagedWrite),
    SessionEnded(Error),
}

/// The owner's live operational tuning, shared with every handle that prepares
/// requests against it.
///
/// The owner is the sole writer: an update reaches it through the control
/// boundary of either owner shell (#631, #780). Readers therefore see one whole [`crate::config::OperationalTuning`] value —
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

/// Raw retention carries a discard operation. Sony retention only requires a read.
#[derive(Debug)]
pub(crate) enum RetainedInput<'a> {
    Raw(RawRetention<'a>),
    Sony,
}

#[derive(Debug)]
pub(crate) enum RawRetention<'a> {
    Framed(crate::protocol::framer::RetainedRawFrame<'a>),
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    Flag {
        evidence: RawPrefixEvidence,
        buffered: &'a mut bool,
    },
    #[cfg(all(test, feature = "runtime-tokio"))]
    AtomicFlag {
        evidence: RawPrefixEvidence,
        buffered: &'a AtomicBool,
        discards: Option<&'a AtomicUsize>,
    },
}

impl RetainedInput<'_> {
    pub(crate) fn evidence(&self) -> Option<RawPrefixEvidence> {
        match self {
            Self::Raw(raw) => Some(raw.evidence()),
            Self::Sony => None,
        }
    }
}

impl RawRetention<'_> {
    fn len(&self) -> std::num::NonZeroUsize {
        match self {
            Self::Framed(frame) => frame.len(),
            #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
            Self::Flag { .. } => std::num::NonZeroUsize::MIN,
            #[cfg(all(test, feature = "runtime-tokio"))]
            Self::AtomicFlag { .. } => std::num::NonZeroUsize::MIN,
        }
    }

    fn evidence(&self) -> RawPrefixEvidence {
        use crate::protocol::framer::RawBufferedInput;
        match self {
            Self::Framed(frame) => match frame.evidence {
                RawBufferedInput::Complete => RawPrefixEvidence::Complete,
                RawBufferedInput::Malformed => RawPrefixEvidence::Malformed,
                RawBufferedInput::Incomplete { target, kind } => {
                    RawPrefixEvidence::Incomplete { target, kind }
                }
            },
            #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
            Self::Flag { evidence, .. } => *evidence,
            #[cfg(all(test, feature = "runtime-tokio"))]
            Self::AtomicFlag { evidence, .. } => *evidence,
        }
    }

    pub(crate) fn discard_first(self) -> std::num::NonZeroUsize {
        match self {
            Self::Framed(frame) => frame.discard_first(),
            #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
            Self::Flag { buffered, .. } => {
                *buffered = false;
                std::num::NonZeroUsize::MIN
            }
            #[cfg(all(test, feature = "runtime-tokio"))]
            Self::AtomicFlag {
                buffered, discards, ..
            } => {
                use std::sync::atomic::Ordering;
                if let Some(discards) = discards {
                    discards.fetch_add(1, Ordering::Relaxed);
                }
                buffered.store(false, Ordering::Release);
                std::num::NonZeroUsize::MIN
            }
        }
    }
}

/// Infallible inspection of retained stream input. Only discarding mutates it.
pub(crate) trait RetainedStreamInput {
    fn retained(&mut self) -> Option<RetainedInput<'_>> {
        None
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
    facts: OwnerFacts,
    tuning: LiveTuning,
    permits: AdmissionPermitPool,
    active: BTreeMap<RequestId, ActiveRequest>,
    motion: Arc<MotionRegistry>,
    target_cache: Arc<PerTarget<Mutex<TargetStateCache>>>,
    diagnostic_subscribers: BTreeMap<u64, DiagnosticSubscriber>,
    // Read only by `subscribe_diagnostics`, whose consumers are feature-gated
    // (#636).
    #[cfg(any(feature = "async", feature = "blocking", test))]
    next_diagnostic_subscription: u64,
    metrics: OwnerMetrics,
    buffers: OwnerBuffers,
}

impl OwnerState {
    pub(crate) fn new(policy: OwnerPolicy, now: Instant) -> Result<Self, Error> {
        let OwnerPolicy {
            protocol,
            targets,
            limits,
            tuning,
            baseline,
            read_timeout,
            write_timeout,
        } = policy;
        let facts = OwnerFacts {
            limits,
            read_timeout,
            write_timeout,
            baseline,
            #[cfg(test)]
            transport: protocol.transport,
        };
        let permits = AdmissionPermitPool::new(
            protocol.capacity.get(),
            PerTarget::from_fn(|target| targets[target].map_or(0, |policy| policy.control_reserve)),
        );
        let mut engine = ProtocolEngine::new(protocol, now)?;
        for (target, target_policy) in targets.iter() {
            if let Some(target_policy) = target_policy {
                engine.register_target(target, *target_policy)?;
            }
        }
        let buffers = OwnerBuffers::new(limits);
        let tuning = LiveTuning::new(tuning);
        Ok(Self {
            engine,
            facts,
            tuning,
            permits,
            active: BTreeMap::new(),
            motion: Arc::new(MotionRegistry::default()),
            target_cache: Arc::new(PerTarget::from_fn(|_| {
                Mutex::new(TargetStateCache::default())
            })),
            diagnostic_subscribers: BTreeMap::new(),
            #[cfg(any(feature = "async", feature = "blocking", test))]
            next_diagnostic_subscription: 1,
            metrics: OwnerMetrics::default(),
            buffers,
        })
    }

    pub(crate) fn permits(&self) -> AdmissionPermitPool {
        self.permits.clone()
    }

    pub(crate) const fn facts(&self) -> &OwnerFacts {
        &self.facts
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
        now: Instant,
    ) -> Result<(), Error> {
        let (command_spacing, inquiry_spacing, command_sockets) =
            self.facts.baseline.apply(validated);
        self.engine
            .retune(command_spacing, inquiry_spacing, command_sockets, now)?;
        self.tuning.set(validated);
        Ok(())
    }

    pub(crate) const fn state(&self) -> SessionState {
        self.engine.state()
    }

    pub(crate) fn boundary_error(&self) -> Option<Error> {
        self.engine.terminal_error().cloned()
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
            target: target.individual_camera(),
            lane: diagnostics::lane(lane),
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

    pub(crate) fn next_wake(&self, now: Instant) -> Option<Instant> {
        self.engine.next_wake(now)
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
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn resolve_retained_raw_input<I>(
        &mut self,
        input: &mut I,
        now: Instant,
    ) -> RawReleaseResolution
    where
        I: RetainedStreamInput + ?Sized,
    {
        if let Some(RetainedInput::Raw(mut raw)) = input.retained() {
            let mut remaining = raw.len().get();
            loop {
                match self
                    .engine
                    .resolve_raw_release_gate(now, Some(raw.evidence()))
                {
                    RawReleaseGateAction::Advance => return RawReleaseResolution::Advance,
                    RawReleaseGateAction::AwaitInputUntil(deadline) => {
                        return RawReleaseResolution::AwaitInputUntil(deadline);
                    }
                    RawReleaseGateAction::DiscardFirst => {
                        remaining -= raw.discard_first().get();
                        self.record_ignored(IgnoreReason::MalformedFrame);
                    }
                }
                if remaining == 0 {
                    break;
                }
                let Some(RetainedInput::Raw(next)) = input.retained() else {
                    break;
                };
                raw = next;
            }
        }
        // No retained raw input clears the previous grace scope.
        let _ = self.engine.resolve_raw_release_gate(now, None);
        RawReleaseResolution::Advance
    }

    pub(crate) fn input(&mut self, input: Input, now: Instant) -> VecDeque<Effect> {
        self.observe_input(&input);
        self.engine
            .handle_into(input, now, &mut self.buffers.effect_scratch);
        self.buffers.take_effects()
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
        self.engine
            .handle_in_turn_into(&turn.0, input, &mut self.buffers.effect_scratch);
        self.buffers.take_effects()
    }

    pub(crate) fn finish_input_turn(
        &mut self,
        turn: OwnerInputTurn,
        engine_turn: EngineTurn,
    ) -> VecDeque<Effect> {
        self.engine
            .finish_input_turn_into(turn.0, engine_turn, &mut self.buffers.effect_scratch);
        self.buffers.take_effects()
    }

    /// Complete an input-only decision after its already queued source effects.
    pub(super) fn finish_input_turn_into(
        &mut self,
        turn: OwnerInputTurn,
        engine_turn: EngineTurn,
        effects: &mut VecDeque<Effect>,
    ) {
        self.engine
            .finish_input_turn_into(turn.0, engine_turn, &mut self.buffers.effect_scratch);
        effects.extend(self.buffers.effect_scratch.drain(..));
    }

    fn observe_input(&mut self, input: &Input) {
        if let Input::Frame(frame) = input {
            self.metrics.received_frames = self.metrics.received_frames.saturating_add(1);
            // Error frames are counted as the owner decodes them, so a camera
            // answering requests the engine can no longer correlate — the exact
            // case a field debugging session is trying to see — still shows up.
            if let DecodedResponse::Error { code, .. } = frame.response {
                if code.is_busy() {
                    self.metrics.busy_errors = self.metrics.busy_errors.saturating_add(1);
                } else if !code.confirms_cancellation() {
                    self.metrics.protocol_errors = self.metrics.protocol_errors.saturating_add(1);
                }
            }
            self.record(DiagnosticEvent::FrameReceived {
                target: frame.target.individual_camera(),
                response: diagnostics::response(&frame.response),
            });
        }
    }

    pub(crate) fn advance(&mut self, now: Instant) -> VecDeque<Effect> {
        self.engine
            .advance_into(now, &mut self.buffers.effect_scratch);
        self.buffers.take_effects()
    }

    #[cfg(test)]
    pub(crate) fn finish_write(
        &mut self,
        staged: &StagedWrite,
        result: WriteOutcome,
        now: Instant,
    ) -> VecDeque<Effect> {
        self.finish_write_turn(staged, result, now, EngineTurn::Complete)
    }

    #[cfg(test)]
    pub(crate) fn finish_write_turn(
        &mut self,
        staged: &StagedWrite,
        result: WriteOutcome,
        now: Instant,
        turn: EngineTurn,
    ) -> VecDeque<Effect> {
        let mut effects = self.buffers.take_effects();
        self.finish_write_turn_into(staged, result, now, turn, &mut effects);
        effects
    }

    pub(super) fn finish_write_turn_into(
        &mut self,
        staged: &StagedWrite,
        result: WriteOutcome,
        now: Instant,
        turn: EngineTurn,
        effects: &mut VecDeque<Effect>,
    ) {
        self.observe_write(staged, &result);
        self.engine.handle_turn_into(
            Input::TransmissionFinished {
                request: staged.request,
                transmission: staged.transmission,
                result,
            },
            now,
            turn,
            &mut self.buffers.effect_scratch,
        );
        self.buffers.prepend_effects(effects);
    }

    /// Records and applies an identified write result in the same decoded-input
    /// turn that produced it, without allowing deadlines between source frames.
    pub(super) fn finish_write_in_turn_into(
        &mut self,
        turn: &OwnerInputTurn,
        staged: &StagedWrite,
        result: WriteOutcome,
        effects: &mut VecDeque<Effect>,
    ) {
        self.observe_write(staged, &result);
        self.engine.handle_in_turn_into(
            &turn.0,
            Input::TransmissionFinished {
                request: staged.request,
                transmission: staged.transmission,
                result,
            },
            &mut self.buffers.effect_scratch,
        );
        self.buffers.prepend_effects(effects);
    }

    pub(super) fn expire_unwritten_halt_into(
        &mut self,
        staged: &StagedWrite,
        now: Instant,
        effects: &mut VecDeque<Effect>,
    ) -> bool {
        let expired = self.engine.expire_unwritten_halt_into(
            staged.request,
            staged.transmission,
            now,
            &mut self.buffers.effect_scratch,
        );
        self.buffers.prepend_effects(effects);
        expired
    }

    fn observe_write(&mut self, staged: &StagedWrite, result: &WriteOutcome) {
        let success = match result {
            WriteOutcome::NotSent(_) => return,
            WriteOutcome::Sent(_) => true,
            WriteOutcome::Failed(_) => false,
        };
        self.metrics.writes = self.metrics.writes.saturating_add(1);
        if !success {
            self.metrics.write_failures = self.metrics.write_failures.saturating_add(1);
        }
        self.record(DiagnosticEvent::WriteFinished {
            id: diagnostics::id(staged.request),
            target: staged.target().individual_camera(),
            cancellation: matches!(staged.kind, Transmission::Cancel { .. }),
            success,
        });
    }

    /// Admit, stamp motion and install the active observer before due/dispatch.
    /// The shell and owner halt share this synchronous path.
    fn admit_in_turn(
        &mut self,
        turn: &OwnerInputTurn,
        request: RuntimeRequest,
        permit: AdmissionPermit,
        observer: impl Into<TerminalSlot>,
    ) -> Result<Admitted, Error> {
        let summary = RequestSummary::new(&request);
        let context = *request.context();
        let verdict = {
            let motion_admission = context.motion.map(|motion| {
                self.motion
                    .begin_admission(context.target, motion.axes, context.submission_order)
            });
            let verdict = self
                .engine
                .admit_owner_in_turn(&turn.0, request, permit.slot());
            match motion_admission {
                Some(admission) => admission.admit(verdict),
                None => verdict,
            }
        };
        self.finish_admission(verdict.map(|id| (id, ())), summary, permit, observer)
            .map(|(admitted, ())| admitted)
    }

    fn admit_operation_in_turn(
        &mut self,
        turn: &OwnerInputTurn,
        request: RuntimeRequest,
        motion: MotionEffect,
        permit: AdmissionPermit,
        observer: impl Into<TerminalSlot>,
    ) -> Result<AdmittedOperation, Error> {
        let summary = RequestSummary::new(&request);
        let context = *request.context();
        let admission =
            self.motion
                .begin_admission(context.target, motion.axes, context.submission_order);
        let verdict = self
            .engine
            .admit_owner_in_turn(&turn.0, request, permit.slot());
        let verdict = admission.admit_operation(verdict);
        let (admitted, motion) = self.finish_admission(verdict, summary, permit, observer)?;
        Ok(AdmittedOperation { admitted, motion })
    }

    fn finish_admission<T>(
        &mut self,
        verdict: Result<(AdmittedId, T), Error>,
        summary: RequestSummary,
        permit: AdmissionPermit,
        observer: impl Into<TerminalSlot>,
    ) -> Result<(Admitted, T), Error> {
        let (identity, retained) = verdict.inspect_err(|error| {
            self.record_admission_rejection(summary.target, summary.lane, error);
        })?;
        let slot = permit.slot();
        let id = identity.request_id();
        self.active
            .insert(id, ActiveRequest::new(permit, observer.into(), summary));
        self.metrics.admitted = self.metrics.admitted.saturating_add(1);
        if slot == AdmissionSlot::ControlReserve {
            self.metrics.control_reserve_admitted =
                self.metrics.control_reserve_admitted.saturating_add(1);
        }
        self.record(DiagnosticEvent::Admitted {
            id: diagnostics::id(id),
            target: summary.target.individual_camera(),
            lane: diagnostics::lane(summary.lane),
        });
        Ok((Admitted { identity }, retained))
    }

    /// Deliver a cancellation failure to the observer installed after the
    /// engine accepted its intent. The operation's terminal slot is untouched.
    fn cancellation_failed(&mut self, id: RequestId, error: Error, delivered_at: Instant) {
        let Some(observer) = self
            .active
            .get(&id)
            .and_then(|active| active.cancellation.as_ref())
        else {
            return;
        };
        if matches!(
            observer.resolve_at(Error::cancellation_observation_failed(error), delivered_at),
            ObserverResolution::ReceiverLost
        ) {
            self.metrics.dropped_observer_events =
                self.metrics.dropped_observer_events.saturating_add(1);
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
        let Some(active) = self.active.get_mut(&request.id.request_id()) else {
            return (self.buffers.take_effects(), Ok(()));
        };
        if active.cancellation.is_some() {
            return (self.buffers.take_effects(), Ok(()));
        }
        let turn = self.engine.begin_input_turn(now);
        let answer =
            self.engine
                .cancel_in_turn_into(&turn, request.id, &mut self.buffers.effect_scratch);
        if answer.is_ok() {
            // The synchronous verdict is authoritative before any effects run.
            // A refused cancellation leaves no provisional owner state.
            active.cancellation = Some(request.observer);
        }
        self.engine.finish_input_turn_into(
            turn,
            EngineTurn::Complete,
            &mut self.buffers.effect_scratch,
        );
        (self.buffers.take_effects(), answer)
    }

    #[cfg(any(feature = "async", feature = "blocking", test))]
    fn subscribe_diagnostics(
        &mut self,
        event_capacity: usize,
        lifecycle: Arc<boundary::OwnerLifecycle>,
    ) -> Result<DiagnosticSubscription, Error> {
        if event_capacity == 0
            || event_capacity > self.facts.limits.diagnostic_events_per_subscription.get()
        {
            return Err(Error::InvalidRequest {
                reason: "diagnostic subscription capacity is outside owner bounds".into(),
            });
        }
        self.diagnostic_subscribers
            .retain(|_, value| !value.sender.is_disconnected());
        if self.diagnostic_subscribers.len() >= self.facts.limits.diagnostic_subscribers.get() {
            return Err(Error::DiagnosticSubscriberLimit {
                capacity: self.facts.limits.diagnostic_subscribers.get(),
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
            #[cfg(feature = "blocking")]
            worker_id: None,
        })
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
            if cache.pan_tilt().speed_mode() != Some(crate::command::PanTiltSpeedMode::Fine50) {
                return Err(Error::InvalidRequest { reason: format!(
                    "pan/tilt speeds above {} require the fine speed mode to be applied on camera {}",
                    crate::command::PanTiltSpeedMode::STANDARD_MAXIMUM,
                    staged.target().camera()).into() });
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
    pub(crate) fn apply_effect(&mut self, effect: Effect, delivered_at: Instant) -> AppliedEffect {
        match effect {
            // Admission effects are emitted only by test compatibility shims;
            // production admission installs its owner record synchronously.
            #[cfg(test)]
            Effect::Admitted { .. } => AppliedEffect::None,
            #[cfg(test)]
            Effect::AdmissionRejected { error, .. } => AppliedEffect::AdmissionRejected(error),
            Effect::Transition { id, from, to } => {
                if let Some(target) = self.active.get(&id).map(|active| active.summary().target) {
                    self.record(DiagnosticEvent::Transition {
                        id: diagnostics::id(id),
                        target: target.individual_camera(),
                        from: diagnostics::phase(from),
                        to: diagnostics::phase(to),
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
            Effect::RetryScheduled { id, attempt, .. } => {
                // Counted where the retry is emitted, not where it is decided:
                // every retry reason the engine has — a busy camera, an expired
                // deadline, a transient receive fault (#565) — funnels through
                // this one effect, so the counter stays correct as retry policy
                // evolves.
                self.metrics.retries_scheduled = self.metrics.retries_scheduled.saturating_add(1);
                if let Some(target) = self.active.get(&id).map(|active| active.summary().target) {
                    self.record(DiagnosticEvent::RetryScheduled {
                        id: diagnostics::id(id),
                        target: target.individual_camera(),
                        attempt,
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
                if let Some(target) = self.active.get(&id).map(|active| active.summary().target) {
                    self.record(DiagnosticEvent::DeadlineExpired {
                        id: diagnostics::id(id),
                        target: target.individual_camera(),
                        deadline: diagnostics::deadline(deadline),
                        will_retry,
                    });
                }
                AppliedEffect::None
            }
            Effect::CancellationRecorded { id } => {
                self.metrics.cancellations = self.metrics.cancellations.saturating_add(1);
                if let Some(target) = self.active.get(&id).map(|active| active.summary().target) {
                    self.record(DiagnosticEvent::CancellationRecorded {
                        id: diagnostics::id(id),
                        target: target.individual_camera(),
                    });
                }
                AppliedEffect::None
            }
            Effect::CancellationObservation { id, observation } => {
                let target = self.active.get(&id).map(|active| active.summary().target);
                let diagnostic = diagnostics::cancellation(&observation);
                if let CancellationObservation::AttemptFailed(error) = observation {
                    self.cancellation_failed(id, error, delivered_at);
                }
                if let Some(target) = target {
                    self.record(DiagnosticEvent::CancellationObserved {
                        id: diagnostics::id(id),
                        target: target.individual_camera(),
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
                let id = diagnostics::id(effect.request);
                let target = effect.target.individual_camera();
                let event = match effect.projection {
                    AppliedStateProjection::Set(value) => {
                        self.metrics.cache_updates = self.metrics.cache_updates.saturating_add(1);
                        DiagnosticEvent::AppliedState {
                            id,
                            target,
                            key: value.key(),
                        }
                    }
                    AppliedStateProjection::Invalidate { key } => {
                        self.metrics.cache_invalidations =
                            self.metrics.cache_invalidations.saturating_add(1);
                        DiagnosticEvent::StateInvalidated { id, target, key }
                    }
                };
                self.record(event);
                AppliedEffect::None
            }
            Effect::Terminal { id, outcome } => {
                let diagnostic = diagnostics::outcome(&outcome);
                if let Some(active) = self.active.remove(&id) {
                    let RetiredRequest { observer, summary } = active.retire();
                    let target = summary.target;
                    if matches!(
                        observer.resolve_at(outcome, delivered_at),
                        ObserverResolution::ReceiverLost
                    ) {
                        self.metrics.dropped_observer_events =
                            self.metrics.dropped_observer_events.saturating_add(1);
                    }
                    self.record(DiagnosticEvent::Terminal {
                        id: diagnostics::id(id),
                        target: target.individual_camera(),
                        outcome: diagnostic,
                    });
                }
                self.metrics.terminal = self.metrics.terminal.saturating_add(1);
                AppliedEffect::None
            }
            Effect::SessionEnded(end) => {
                self.record(DiagnosticEvent::SessionChanged {
                    from: crate::observability::SessionStatus::Running,
                    to: crate::observability::SessionStatus::from_engine(end.status.state()),
                    reason: end.cause.kind(),
                });
                AppliedEffect::SessionEnded(end.cause)
            }
            Effect::Ignored(reason) => {
                self.record_ignored(reason);
                AppliedEffect::None
            }
        }
    }

    /// Record a discarded frame without selecting a terminal delivery instant.
    pub(super) fn record_ignored(&mut self, reason: IgnoreReason) {
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
        if reason == IgnoreReason::ReplyShapeMismatch {
            self.metrics.ignored_reply_shape_mismatches = self
                .metrics
                .ignored_reply_shape_mismatches
                .saturating_add(1);
        }
        self.record(DiagnosticEvent::Ignored {
            reason: diagnostics::ignored(reason),
        });
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
            Error::Io { .. }
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

/// Prepend test-driver effects while preserving their source order.
#[cfg(test)]
pub(crate) fn prepend_effects(queue: &mut VecDeque<Effect>, produced: VecDeque<Effect>) {
    for effect in produced.into_iter().rev() {
        queue.push_front(effect);
    }
}

#[cfg(test)]
use crate::runtime::engine::RequestContext;
