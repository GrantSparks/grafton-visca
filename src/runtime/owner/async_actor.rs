//! Runtime-neutral async actor and its bounded control boundary.

use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use futures_lite::future;

use crate::{
    completion, executor::Executor, AffectedAxes, CancellationOutcome, Error, ResponseDecoder,
};

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
use super::DiagnosticEvent;
#[cfg(all(test, feature = "runtime-tokio"))]
use super::OwnerMetrics;
use super::ReceiptObservation;
use super::{
    cancellation_receipt_for, clamp_receive_pause, completion_pair,
    normalize_cancellation_observation, normalize_command_outcome, normalize_inquiry_outcome,
    observation_outcome, prepend_effects, transient_receive_pause, AdmissionPermit, AppliedEffect,
    CancellationCore, CompletionObserver, DecodedFrame, DiagnosticSubscription, IdleReceiveRun,
    Input, OwnerInputTurn, OwnerPolicy, OwnerState, ReceiptCore, RejectedCancellation, RequestId,
    RequestLane, RuntimeOutcome, RuntimeRequest, SessionState, ShutdownReason, TargetStateCache,
    TransientFaultRun, TransmissionMeta, WaitSelection, WireWrite,
};

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
use super::{
    MAXIMUM_TRANSIENT_RECEIVE_PAUSE, TRANSIENT_RECEIVE_FAULT_LIMIT, TRANSIENT_RECEIVE_FAULT_RESET,
    TRANSIENT_RECEIVE_FAULT_SPAN, TRANSIENT_RECEIVE_PAUSE,
};
use crate::runtime::engine::{
    Effect, EngineTurn, IgnoreReason, RawCorrelationReleaseSet, RawPrefixEvidence,
    RawReleaseGateAction, TransportKind,
};

/// Independent cap for work deferred/discarded while one due raw correlation
/// release waits behind retained stream framing. This is intentionally not the
/// per-receive frame batch limit: several legal batches may already be buffered.
const RAW_CORRELATION_RELEASE_WORK_LIMIT: usize = 64;

/// Whether one actor turn keeps the session alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnOutcome {
    /// Keep running with protocol input first.
    Continue,
    /// Keep protocol input first because the stream framer still retains
    /// ordered input. This uses its own fixed work bound rather than the
    /// ordinary receive fairness ceiling.
    ContinueBuffered,
    /// This receive turn made no protocol progress, so poll the ordered
    /// boundary sources first on the next selection after one cooperative
    /// executor handoff.
    YieldBoundaries,
    /// The session is over.
    Stop,
}

/// Which source is the deterministic left-biased winner when both phases are
/// ready at once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SourcePhase {
    /// Poll meaningful protocol input before a simultaneous boundary.
    #[default]
    ReceiveFirst,
    /// After a non-progressing receive, give the ordered boundary sources first
    /// refusal.
    BoundariesFirst,
}

impl TurnOutcome {
    const fn next_source_phase(self) -> Option<SourcePhase> {
        match self {
            Self::Continue | Self::ContinueBuffered => Some(SourcePhase::ReceiveFirst),
            Self::YieldBoundaries => Some(SourcePhase::BoundariesFirst),
            Self::Stop => None,
        }
    }
}

/// Selects one source according to the actor's explicit phase.
///
/// `future::or` is left-biased, so this helper is also the executable contract
/// for simultaneous readiness rather than relying on an executor's wake order.
async fn select_source<T, Receive, Boundaries>(
    phase: SourcePhase,
    receive: Receive,
    boundaries: Boundaries,
) -> T
where
    Receive: Future<Output = T>,
    Boundaries: Future<Output = T>,
{
    match phase {
        SourcePhase::ReceiveFirst => future::or(receive, boundaries).await,
        SourcePhase::BoundariesFirst => future::or(boundaries, receive).await,
    }
}

/// Select the final two boundary sources without disturbing the priority of
/// shutdown, cancellation, or admission above them.
///
/// An already-due engine wake gets one control allowance, then wins this tail
/// on the following turn.  Keeping this separate from [`select_source`] makes
/// the control-vs-wake bound independent of receive-vs-boundary arbitration.
async fn select_control_or_wake<T, Control, Wake>(
    wake_first: bool,
    control: Control,
    wake: Wake,
) -> T
where
    Control: Future<Output = T>,
    Wake: Future<Output = T>,
{
    if wake_first {
        future::or(wake, control).await
    } else {
        future::or(control, wake).await
    }
}

/// Yield one poll to the current executor without depending on a runtime.
///
/// A ready transport can otherwise keep this actor inside one executor poll:
/// merely placing the pending boundary futures first still falls through to the
/// ready receive.  Returning `Pending` once lets callers and timers enqueue
/// before the forced boundary-first turn is selected.
async fn cooperative_yield() {
    let mut yielded = false;
    std::future::poll_fn(move |context| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            context.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}

trait OwnerClock: Send + Sync + 'static {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

impl<R> OwnerClock for R
where
    R: Executor,
{
    fn now(&self) -> Instant {
        Executor::now(self)
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(Executor::sleep(self, duration))
    }
}

#[derive(Clone)]
struct BoundClock(Arc<dyn OwnerClock>);

impl BoundClock {
    fn from_shared<R: Executor>(runtime: Arc<R>) -> Self {
        Self(runtime)
    }

    fn now(&self) -> Instant {
        self.0.now()
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.0.sleep(duration)
    }
}

impl std::fmt::Debug for BoundClock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("BoundClock").finish_non_exhaustive()
    }
}

/// Outcome of one async driver receive.
///
/// The distinction is load bearing on byte-stream transports: only a zero-length
/// transport read means the peer closed. A read that carried bytes but did not
/// finish a frame decodes to an empty batch, and the owner must keep pumping so
/// the remainder of the frame can arrive in a later read.
#[derive(Debug)]
pub(crate) enum AsyncReceive {
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

/// Async transport/framing adapter. Both operations finish outside any mutable
/// engine borrow. A receive may return multiple decoded frames in source order.
pub(crate) trait AsyncOwnerDriver: Send {
    fn write(
        &mut self,
        write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send;

    fn receive(
        &mut self,
        buffers: &mut super::OwnerBuffers,
        frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send;

    /// Whether this byte-stream driver retains input that has not yet been
    /// delivered to the engine. Datagram and stateless test drivers retain
    /// nothing by default.
    fn has_buffered_stream_input(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// Classify the first retained raw stream input. Complete frames defer to
    /// the ordinary decode/ignore path without early source attribution.
    /// Incomplete raw evidence is deliberately limited to the first two bytes
    /// and remains inert until the shared engine decides whether it belongs to
    /// an expiring correlation scope. `None` means no input is buffered.
    fn buffered_stream_input(&mut self) -> Result<Option<RawPrefixEvidence>, Error> {
        Ok(None)
    }

    /// Discard exactly the first raw frame/fragment retained from an old
    /// correlation interval, preserving later framed input when possible.
    fn discard_buffered_stream_input(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// The one-way decision for an admission constrained by an outer deadline.
///
/// The caller and actor race only to decide whether this boundary crosses the
/// admission boundary. Once the actor claims it, a caller at its deadline must
/// wait for that already-admitted reply and may later detach its observer by
/// the ordinary receipt path. Once the caller expires it, the actor must drop
/// the queued boundary without touching engine state.
#[derive(Debug, Clone)]
struct AdmissionValidity {
    deadline: Instant,
    state: Arc<AtomicU8>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdmissionValidityState {
    Pending,
    Claimed,
    Expired,
}

/// The actor's linearized answer when it reaches a deadline-constrained
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdmissionClaim {
    Claimed,
    /// This actor observed the deadline first and rejected the boundary.
    ExpiredHere,
    /// The caller had already rejected the boundary before this actor turn.
    ExpiredElsewhere,
}

impl AdmissionValidity {
    const PENDING: u8 = AdmissionValidityState::Pending as u8;
    const CLAIMED: u8 = AdmissionValidityState::Claimed as u8;
    const EXPIRED: u8 = AdmissionValidityState::Expired as u8;

    fn until(deadline: Instant) -> Self {
        Self {
            deadline,
            state: Arc::new(AtomicU8::new(Self::PENDING)),
        }
    }

    /// Claim the boundary immediately before engine admission.
    ///
    /// A caller that has already won expiry returns
    /// [`AdmissionClaim::ExpiredElsewhere`]. Conversely, claiming before
    /// expiry means the admission is authoritative, so the caller must observe
    /// its reply rather than turn that admitted work into a pre-admission
    /// timeout.
    fn claim_for_admission(&self, now: Instant) -> AdmissionClaim {
        if now >= self.deadline {
            return if self
                .state
                .compare_exchange(
                    Self::PENDING,
                    Self::EXPIRED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                AdmissionClaim::ExpiredHere
            } else {
                AdmissionClaim::ExpiredElsewhere
            };
        }

        if self
            .state
            .compare_exchange(
                Self::PENDING,
                Self::CLAIMED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            AdmissionClaim::Claimed
        } else {
            AdmissionClaim::ExpiredElsewhere
        }
    }

    /// Mark the boundary expired if no actor has already claimed admission.
    ///
    /// The boolean is the linearized answer to the caller's timeout race:
    /// `true` means it may return `Error::Timeout`; `false` means an admitted
    /// reply is authoritative and still has to be observed.
    fn expire_before_admission(&self) -> bool {
        self.state
            .compare_exchange(
                Self::PENDING,
                Self::EXPIRED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}

#[derive(Debug)]
struct AdmissionBoundary {
    request: RuntimeRequest,
    permit: AdmissionPermit,
    observer: Arc<super::ObserverCell>,
    reply: flume::Sender<Result<RequestId, Error>>,
    /// Present only for a caller deadline that applies before admission.
    validity: Option<AdmissionValidity>,
    /// The actor has already won the caller's pre-admission deadline race,
    /// but a raw-correlation release gate has retained this boundary until it
    /// can safely enter the engine.  Keeping that one-way answer on the
    /// boundary preserves the caller's established admission promise without
    /// letting the deferred admission run a due/dispatch turn early.
    validity_claimed: bool,
}

/// A rejection that happened before the async handle could allocate any
/// owner-owned admission state.
#[derive(Debug, Clone, Copy)]
struct PreAdmissionRejection {
    target: crate::CameraId,
    lane: RequestLane,
    error: crate::ErrorKind,
}

#[derive(Debug)]
struct PendingAdmissionRejections {
    events: VecDeque<PreAdmissionRejection>,
    /// Events evicted before the actor could enter them into its public ring.
    /// The actor folds this into the existing `dropped_diagnostics` metric.
    dropped: u64,
    /// A one-slot wake-up is already queued or being handled by the actor.
    /// This is protected by the same mutex as `events`, so a concurrent
    /// reporter can never lose the wake-up between an actor drain and its next
    /// empty receive.
    wake_pending: bool,
}

/// Bounded, coalescing ingress for failures that occur on cloneable async
/// handles before an `AdmissionBoundary` exists.
///
/// The actor remains the only diagnostic delivery and owner-metric writer.
/// Handles merely record a compact event and wake it. The scalar total is
/// atomic so a diagnostic ingress burst cannot undercount rejections when its
/// bounded event queue coalesces before the actor gets a turn.
#[derive(Debug)]
struct AdmissionRejectionIngress {
    total: AtomicU64,
    pending: Mutex<PendingAdmissionRejections>,
    capacity: usize,
}

impl AdmissionRejectionIngress {
    fn new(capacity: usize) -> Self {
        Self {
            total: AtomicU64::new(0),
            pending: Mutex::new(PendingAdmissionRejections {
                events: VecDeque::with_capacity(capacity),
                dropped: 0,
                wake_pending: false,
            }),
            capacity,
        }
    }

    /// Records one rejection and reports whether this caller must enqueue the
    /// one coalesced actor wake-up.
    fn record(&self, event: PreAdmissionRejection) -> bool {
        let mut total = self.total.load(Ordering::Acquire);
        loop {
            match self.total.compare_exchange_weak(
                total,
                total.saturating_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(observed) => total = observed,
            }
        }
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pending.events.len() == self.capacity {
            // The owner diagnostic ring is bounded too. Preserve the newest
            // rejection facts, which are the useful ones during saturation,
            // while the atomic total remains exact. The actor reports the
            // bounded loss through its existing diagnostics-drop metric.
            pending.events.pop_front();
            pending.dropped = pending.dropped.saturating_add(1);
        }
        pending.events.push_back(event);
        if pending.wake_pending {
            false
        } else {
            pending.wake_pending = true;
            true
        }
    }

    fn total(&self) -> u64 {
        self.total.load(Ordering::Acquire)
    }

    /// Moves pending bounded diagnostics into the actor's preallocated scratch
    /// queue. When `consumed_wake` is true, clearing the wake marker occurs
    /// under this same lock, closing the report/drain race.
    fn drain_into(
        &self,
        scratch: &mut VecDeque<PreAdmissionRejection>,
        consumed_wake: bool,
    ) -> u64 {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(scratch.is_empty());
        scratch.extend(pending.events.drain(..));
        if consumed_wake {
            pending.wake_pending = false;
        }
        let dropped = pending.dropped;
        pending.dropped = 0;
        dropped
    }
}

#[derive(Debug)]
enum AdmissionWait {
    Reply(Result<Result<RequestId, Error>, Error>),
    Deadline { expired_before_admission: bool },
}

#[derive(Debug)]
struct CancellationBoundary {
    receipt: ReceiptCore,
    /// A refused cancellation returns the receipt with the reason, so the
    /// caller keeps the observer for the original request the engine left
    /// running (#612).
    reply: flume::Sender<Result<CancellationCore, RejectedCancellation>>,
}

/// A boundary removed from its channel only because a due raw-correlation
/// release must first receive/frame one exact input turn. It is deliberately
/// not yet engine input: both admission and cancellation normally end an input
/// turn, which would otherwise run the due release and dispatch a successor
/// behind unresolved raw evidence.
///
/// `AdmissionBoundary` keeps its request inline so accepting an admission does
/// not add a heap allocation at the actor-channel boundary.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum DeferredRawBoundary {
    Admission(AdmissionBoundary),
    Cancellation(CancellationBoundary),
}

#[derive(Debug)]
enum ControlBoundary {
    /// Internal coalesced wake-up for a handle-side rejection that happened
    /// before an admission boundary existed. It uses the existing bounded
    /// control lane so source arbitration remains unchanged.
    FlushAdmissionRejections,
    // Built only by `AsyncOwnerHandle::snapshot`, called only from this module's tests (#636).
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    Snapshot(flume::Sender<OwnerSnapshot>),
    Metrics(flume::Sender<Result<crate::observability::MetricsSnapshot, Error>>),
    SubscribeDiagnostics {
        capacity: usize,
        reply: flume::Sender<Result<DiagnosticSubscription, Error>>,
    },
    /// Installs new session tuning on the live owner (#631).
    ///
    /// This lane is what makes the update serialized: the actor is the only
    /// writer of the shared tuning cell, so two handles reconfiguring at the
    /// same time resolve last-writer-wins in the order the actor accepted them
    /// and no reader ever observes a mixture of the two. Facade validation is
    /// carried in the same message so a terminal actor selects its retained
    /// cause before returning a proposed update's validation error (#690).
    Reconfigure {
        validated_tuning: Box<Result<crate::OperationalTuning, Error>>,
        reply: flume::Sender<Result<(), Error>>,
    },
}

/// Bounded diagnostic/metric copy returned by the actor task.
#[derive(Debug, Clone)]
pub(crate) struct OwnerSnapshot {
    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) metrics: OwnerMetrics,
    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) diagnostics: Vec<DiagnosticEvent>,
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) state: SessionState,
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) active: usize,
}

/// Async-mode linear command observation right.
#[derive(Debug)]
pub(crate) struct AsyncCommandReceipt {
    core: ReceiptCore,
}

/// Async-mode linear inquiry observation right and its exact decoder.
#[derive(Debug)]
pub(crate) struct AsyncInquiryReceipt<R> {
    core: ReceiptCore,
    decoder: ResponseDecoder<R>,
}

/// Async-mode linear operation observation right and its exact settlement and
/// originating-owner authority. Only this class exposes cancellation.
#[derive(Debug)]
pub(crate) struct AsyncOperationReceipt<K>
where
    K: completion::Kind,
{
    core: ReceiptCore,
    affected_axes: AffectedAxes,
    settlement: completion::Settlement<K>,
    owner: AsyncOwnerHandle,
    marker: PhantomData<fn() -> K>,
}

/// Async-mode observation of one exact cancellation request.
#[derive(Debug)]
pub(crate) struct AsyncCancellationReceipt {
    core: CancellationCore,
}

/// Selected owner and executor clock used only by receipt observation.
#[derive(Debug, Clone)]
pub(crate) struct AsyncReceiptControl {
    owner: AsyncOwnerHandle,
}

/// A targeted settled selection awaiting Phase-6 execution.
#[derive(Debug)]
pub(crate) struct AsyncSettlementWait {
    receipt: AsyncOperationReceipt<completion::Targeted>,
    selection: WaitSelection,
    control: AsyncReceiptControl,
}

/// Type-erased targeted wait used by dynamic facades without duplicating any
/// settlement policy or polling logic.
#[derive(Debug)]
// Built only by `AsyncSettlementWait::erase`, which only this module's tests call (#636).
#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
pub(crate) struct ErasedAsyncSettlementWait(AsyncSettlementWait);

/// Result of the applied portion of an async targeted settlement wait.
// Boxing the polling continuation would add an allocation at the settlement
// boundary; this enum is an internal, short-lived owner value.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AsyncAfterApplied {
    Settled(AsyncReceiptControl),
    Poll(AsyncPollingContinuation),
}

/// Exact polling work delegated to Phase 6 without claiming settlement.
#[derive(Debug)]
pub(crate) struct AsyncPollingContinuation {
    pub(crate) target: crate::CameraId,
    pub(crate) axes: AffectedAxes,
    pub(crate) plan: crate::prepared::SettlementPlan,
    pub(crate) deadline: Instant,
    pub(crate) control: AsyncReceiptControl,
    pub(crate) owner: AsyncOwnerHandle,
}

impl AsyncCommandReceipt {
    pub(crate) async fn wait(self, control: AsyncReceiptControl) -> Result<(), Error> {
        let timeout = self.core.configured_timeout();
        wait_core_for(self.core, control, timeout)
            .await
            .and_then(normalize_command_outcome)
    }

    // Used only by owner unit tests; the public session calls `wait`.
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) async fn wait_with_timeout(
        self,
        control: AsyncReceiptControl,
        timeout: Duration,
    ) -> Result<(), Error> {
        wait_core_for(self.core, control, timeout)
            .await
            .and_then(normalize_command_outcome)
    }

    // Used only by the owner-origin test below to release an otherwise
    // unobserved receipt while both actors are shut down.
    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) fn detach(self) {}
}

impl<T> AsyncInquiryReceipt<T> {
    pub(crate) async fn wait(self, control: AsyncReceiptControl) -> Result<T, Error> {
        let timeout = self.core.configured_timeout();
        let outcome = wait_core_for(self.core, control, timeout).await?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }

    async fn wait_until(self, control: AsyncReceiptControl, deadline: Instant) -> Result<T, Error> {
        let outcome = wait_core_until(self.core, control, deadline).await?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }
}

impl<K> AsyncOperationReceipt<K>
where
    K: completion::Kind,
{
    pub(crate) fn id(&self) -> u64 {
        self.core.id().get()
    }

    pub(crate) async fn applied(self, control: AsyncReceiptControl) -> Result<(), Error> {
        let timeout = self.core.configured_timeout();
        wait_core_for(self.core, control, timeout)
            .await
            .and_then(normalize_command_outcome)
    }

    pub(crate) async fn applied_with_timeout(
        self,
        control: AsyncReceiptControl,
        timeout: Duration,
    ) -> Result<(), Error> {
        wait_core_for(self.core, control, timeout)
            .await
            .and_then(normalize_command_outcome)
    }

    /// Records cancellation intent, returning this receipt intact when the
    /// owner refuses (#612).
    ///
    /// The large `Err` variant is the point: it is the caller's observation
    /// right travelling back rather than being destroyed. The public
    /// `Operation::cancel` boxes it into `CancelRejected` before it reaches a
    /// caller, so no public `Result` carries this size. This mirrors the
    /// blocking twin, `BlockingReceiptControl::cancel_operation`.
    #[allow(clippy::result_large_err)]
    pub(crate) async fn cancel(self) -> Result<AsyncCancellationReceipt, (Option<Self>, Error)> {
        let Self {
            core,
            affected_axes,
            settlement,
            owner,
            marker,
        } = self;
        match owner.cancel_core(core).await {
            Ok(cancellation) => Ok(cancellation),
            Err(RejectedCancellation { receipt, error }) => Err((
                receipt.map(|core| Self {
                    core,
                    affected_axes,
                    settlement,
                    owner,
                    marker,
                }),
                error,
            )),
        }
    }

    pub(crate) fn detach(self) {}
}

impl AsyncOperationReceipt<completion::Targeted> {
    pub(crate) fn settled(self, control: AsyncReceiptControl) -> AsyncSettlementWait {
        AsyncSettlementWait {
            receipt: self,
            selection: WaitSelection::Configured,
            control,
        }
    }

    pub(crate) fn settled_with_timeout(
        self,
        control: AsyncReceiptControl,
        timeout: Duration,
    ) -> AsyncSettlementWait {
        AsyncSettlementWait {
            receipt: self,
            selection: WaitSelection::Override(timeout),
            control,
        }
    }
}

impl AsyncSettlementWait {
    // Used only by owner unit tests; dynamic facades settle through `Operation`.
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) fn erase(self) -> ErasedAsyncSettlementWait {
        ErasedAsyncSettlementWait(self)
    }

    pub(crate) async fn wait(self) -> Result<AsyncReceiptControl, Error> {
        let budget = match self.selection {
            WaitSelection::Configured => {
                self.receipt.settlement.default_budget().ok_or_else(|| {
                    Error::InvalidState(
                        "targeted operation omitted its configured settlement budget".into(),
                    )
                })?
            }
            WaitSelection::Override(timeout) => timeout,
        };
        let deadline = self.control.owner.deadline_after(budget)?;
        match self.wait_applied_until(deadline).await? {
            AsyncAfterApplied::Settled(control) => Ok(control),
            AsyncAfterApplied::Poll(continuation) => continuation.wait().await,
        }
    }

    /// Waits for application using the Phase-6-selected absolute deadline. The
    /// same deadline is retained by any returned polling continuation.
    pub(crate) async fn wait_applied_until(
        self,
        deadline: Instant,
    ) -> Result<AsyncAfterApplied, Error> {
        let target = self.receipt.core.target();
        let outcome = wait_core_until(self.receipt.core, self.control.clone(), deadline).await?;
        normalize_command_outcome(outcome)?;
        match self.receipt.settlement.into_plan()?.into_inner() {
            crate::prepared::SettlementPlan::CompletionIsSettled {
                target: plan_target,
                ..
            } => {
                debug_assert_eq!(plan_target, target);
                Ok(AsyncAfterApplied::Settled(self.control))
            }
            plan @ crate::prepared::SettlementPlan::Poll {
                target: plan_target,
                axes,
                ..
            } => {
                debug_assert_eq!(plan_target, target);
                debug_assert_eq!(axes, self.receipt.affected_axes);
                Ok(AsyncAfterApplied::Poll(AsyncPollingContinuation {
                    target,
                    axes: self.receipt.affected_axes,
                    plan,
                    deadline,
                    control: self.control,
                    owner: self.receipt.owner,
                }))
            }
        }
    }
}

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
impl ErasedAsyncSettlementWait {
    // Used only by owner unit tests; dynamic facades settle through `Operation`.
    pub(crate) async fn wait(self) -> Result<(), Error> {
        self.0.wait().await.map(drop)
    }
}

impl AsyncPollingContinuation {
    async fn wait(self) -> Result<AsyncReceiptControl, Error> {
        let Self {
            target: continuation_target,
            axes: continuation_axes,
            plan,
            deadline,
            control,
            owner,
            ..
        } = self;
        let crate::prepared::SettlementPlan::Poll {
            target,
            queries,
            axes,
            tolerance,
            interval,
            ..
        } = plan
        else {
            return Err(Error::InvalidState(
                "polling continuation carried a non-poll settlement plan".into(),
            ));
        };
        debug_assert_eq!(target, continuation_target);
        debug_assert_eq!(axes, continuation_axes);
        let mut detector = crate::prepared::MotionDetector::new(axes, tolerance);
        let baseline = sample_positions_async(&owner, &control, &queries, deadline).await?;
        if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
            return Err(Error::InvalidState(
                "new movement detector rejected its baseline snapshot".into(),
            ));
        }
        loop {
            let remaining = deadline.saturating_duration_since(owner.now());
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            owner.clock.sleep(interval.min(remaining)).await;
            ensure_async_before_deadline(&owner, deadline)?;
            let snapshot = sample_positions_async(&owner, &control, &queries, deadline).await?;
            match detector.observe(snapshot)? {
                crate::prepared::MotionState::Settled => return Ok(control),
                crate::prepared::MotionState::Moving => {}
                crate::prepared::MotionState::NeedSample => {
                    return Err(Error::InvalidState(
                        "movement detector lost its baseline snapshot".into(),
                    ));
                }
            }
        }
    }
}

/// Samples exactly the prepared position inquiries before one absolute
/// owner-clock deadline. This helper is shared by targeted settlement and the
/// standalone owner-backed motion facade.
pub(crate) async fn sample_positions_async(
    owner: &AsyncOwnerHandle,
    control: &AsyncReceiptControl,
    queries: &crate::prepared::PositionQueryPlan,
    deadline: Instant,
) -> Result<crate::prepared::PositionSnapshot, Error> {
    let mut snapshot = crate::prepared::PositionSnapshot::default();
    if let Some(query) = &queries.pan_tilt {
        ensure_async_before_deadline(owner, deadline)?;
        let receipt = owner
            .submit_inquiry_until(query.instantiate(), deadline)
            .await?;
        let value = receipt.wait_until(control.clone(), deadline).await?;
        ensure_async_before_deadline(owner, deadline)?;
        snapshot.pan_tilt = Some(value);
    }
    if let Some(query) = &queries.zoom {
        ensure_async_before_deadline(owner, deadline)?;
        let receipt = owner
            .submit_inquiry_until(query.instantiate(), deadline)
            .await?;
        let value = receipt.wait_until(control.clone(), deadline).await?;
        ensure_async_before_deadline(owner, deadline)?;
        snapshot.zoom = Some(value);
    }
    if let Some(query) = &queries.focus {
        ensure_async_before_deadline(owner, deadline)?;
        let receipt = owner
            .submit_inquiry_until(query.instantiate(), deadline)
            .await?;
        let value = receipt.wait_until(control.clone(), deadline).await?;
        ensure_async_before_deadline(owner, deadline)?;
        snapshot.focus = Some(value);
    }
    if let Some(query) = &queries.iris {
        ensure_async_before_deadline(owner, deadline)?;
        let receipt = owner
            .submit_inquiry_until(query.instantiate(), deadline)
            .await?;
        let value = receipt.wait_until(control.clone(), deadline).await?;
        ensure_async_before_deadline(owner, deadline)?;
        snapshot.iris = Some(value);
    }
    if let Some(query) = &queries.nd_filter {
        ensure_async_before_deadline(owner, deadline)?;
        let receipt = owner
            .submit_inquiry_until(query.instantiate(), deadline)
            .await?;
        let value = receipt.wait_until(control.clone(), deadline).await?;
        ensure_async_before_deadline(owner, deadline)?;
        snapshot.nd_filter = Some(value);
    }
    Ok(snapshot)
}

/// Rechecks the owner-bound monotonic clock at every admission/sample
/// boundary. In particular, equality with the deadline is already too late
/// for another inquiry to be enqueued.
pub(crate) fn ensure_async_before_deadline(
    owner: &AsyncOwnerHandle,
    deadline: Instant,
) -> Result<(), Error> {
    if owner.now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}

impl AsyncCancellationReceipt {
    #[cfg(all(test, feature = "runtime-tokio"))]
    async fn recv_test(self) -> Result<crate::runtime::engine::CancellationObservation, Error> {
        self.core
            .recv_async()
            .await
            .map(test_cancellation_observation)
    }

    pub(crate) async fn outcome(
        self,
        control: AsyncReceiptControl,
        timeout: Duration,
    ) -> Result<CancellationOutcome, Error> {
        if !Arc::ptr_eq(&self.core.origin, &control.owner.origin) {
            return Err(Error::InvalidState(
                "cancellation receipt belongs to a different owner".into(),
            ));
        }
        let deadline = control.owner.deadline_after(timeout)?;
        wait_cancellation_until(self.core, &control.owner, deadline)
            .await
            .and_then(normalize_cancellation_observation)
    }

    pub(crate) fn detach(self) {}
}

async fn wait_core_for(
    core: ReceiptCore,
    control: AsyncReceiptControl,
    timeout: Duration,
) -> Result<RuntimeOutcome, Error> {
    let deadline = control.owner.deadline_after(timeout)?;
    wait_core_until(core, control, deadline).await
}

async fn wait_core_until(
    core: ReceiptCore,
    control: AsyncReceiptControl,
    deadline: Instant,
) -> Result<RuntimeOutcome, Error> {
    if !Arc::ptr_eq(&core.origin, &control.owner.origin) {
        return Err(Error::InvalidState(
            "receipt belongs to a different owner".into(),
        ));
    }
    if let Some(outcome) = core.try_outcome() {
        return Ok(outcome);
    }
    let remaining = deadline.saturating_duration_since(control.owner.now());
    let completion = async {
        core.completion
            .recv_async()
            .await
            .map(observation_outcome)
            .map_err(|_| control.owner.disconnected_error())
    };
    let actor_gone = async {
        // Nothing is ever sent on this lane. It resolves when the actor has
        // dropped its sender, including an unwind before `run` can latch a
        // terminal owner error. Re-check the observer after the liveness edge
        // so a terminal publication racing teardown still wins.
        while control.owner.actor_alive.recv_async().await.is_ok() {}
        core.try_outcome()
            .ok_or_else(|| control.owner.disconnected_error())
    };
    let timer = async {
        control.owner.clock.sleep(remaining).await;
        core.try_outcome().ok_or(Error::Timeout)
    };
    future::or(completion, future::or(actor_gone, timer)).await
}

async fn wait_cancellation_until(
    mut core: CancellationCore,
    owner: &AsyncOwnerHandle,
    deadline: Instant,
) -> Result<ReceiptObservation, Error> {
    if let Some(observation) = core.try_observation() {
        return Ok(observation);
    }
    let remaining = deadline.saturating_duration_since(owner.now());
    let completion_observer = core.completion;
    let completion = async {
        completion_observer
            .recv_async()
            .await
            .map_err(|_| owner.disconnected_error())
    };
    let actor_gone = async {
        // Cancellation keeps the same actor-liveness guarantee as ordinary
        // receipt waits. A cancellation acknowledgement may have raced the
        // actor edge, so retain the observer's final buffered observation.
        while owner.actor_alive.recv_async().await.is_ok() {}
        completion_observer
            .try_recv()
            .ok_or_else(|| owner.disconnected_error())
    };
    let timer = async {
        owner.clock.sleep(remaining).await;
        completion_observer.try_recv().ok_or(Error::Timeout)
    };
    future::or(completion, future::or(actor_gone, timer)).await
}

#[cfg(all(test, feature = "runtime-tokio"))]
fn test_cancellation_observation(
    observation: ReceiptObservation,
) -> crate::runtime::engine::CancellationObservation {
    match observation {
        ReceiptObservation::Terminal(RuntimeOutcome::Written) => {
            crate::runtime::engine::CancellationObservation::Failed(Error::InvalidState(
                "a local write outcome cannot authorize cancellation".into(),
            ))
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Applied) => {
            crate::runtime::engine::CancellationObservation::Completed
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Cancelled) => {
            crate::runtime::engine::CancellationObservation::Cancelled
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(error))
        | ReceiptObservation::CancellationFailed(error) => {
            crate::runtime::engine::CancellationObservation::Failed(error)
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Reply { .. }) => {
            crate::runtime::engine::CancellationObservation::Failed(Error::InvalidState(
                "an inquiry outcome cannot authorize cancellation".into(),
            ))
        }
    }
}

fn async_observer_deadline(clock: &BoundClock, timeout: Duration) -> Result<Instant, Error> {
    clock
        .now()
        .checked_add(timeout)
        .ok_or_else(|| Error::InvalidParameter {
            parameter: "observer timeout",
            value: format!("{timeout:?}").into(),
            reason: "duration exceeds the monotonic clock range".into(),
        })
}

/// Cloneable boundary handle. Admission, cancellation, ordinary control and
/// shutdown each have separate bounded queues.
#[derive(Debug, Clone)]
pub(crate) struct AsyncOwnerHandle {
    permits: super::AdmissionPermitPool,
    admissions: flume::Sender<AdmissionBoundary>,
    admission_rejections: Arc<AdmissionRejectionIngress>,
    cancellations: flume::Sender<CancellationBoundary>,
    control: flume::Sender<ControlBoundary>,
    shutdown: flume::Sender<()>,
    /// Disconnects after actor teardown, including driver/transport drop.
    /// Nothing is ever sent on it; see [`AsyncOwnerHandle::await_boundary_reply`].
    actor_alive: flume::Receiver<()>,
    shutdown_signal: Arc<Mutex<ShutdownSignalState>>,
    terminal_error: Arc<Mutex<Option<Error>>>,
    origin: Arc<()>,
    clock: BoundClock,
    state_cache: Arc<[Mutex<TargetStateCache>; 9]>,
    /// The owner's live operational tuning (#631). Reading it is a lock and a
    /// copy, so preparation never has to round-trip through the actor.
    tuning: super::LiveTuning,
}

impl AsyncOwnerHandle {
    /// Wait until the owner actor has finished its teardown.
    ///
    /// The liveness lane is disconnected only after [`AsyncOwnerActor::run`]
    /// has drained its boundary queues and explicitly dropped its driver.  A
    /// caller that needs a deterministic transport-release barrier (the async
    /// session's consuming `close`) waits here instead of relying on a
    /// detached executor task's completion semantics.
    pub(crate) async fn wait_closed(&self) -> Result<(), Error> {
        // Nothing is ever sent on this lane.  It resolves when the actor drops
        // its sender, after the driver/transport has been dropped.
        let _ = self.actor_alive.recv_async().await;

        // `run` publishes this before dropping the driver and liveness sender,
        // so this read is ordered after transport teardown.  An explicit
        // shutdown is the only terminal result that consuming `close` turns
        // into success; transport close/poison (and any other terminal owner
        // error) must remain observable at that boundary (#542 §Terminology,
        // §3 ordering and transport failure).
        match self
            .terminal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
        {
            Some(Error::RuntimeShutdown) => Ok(()),
            Some(error) => Err(error),
            None => Err(Self::missing_terminal_error()),
        }
    }

    /// Await one boundary reply, or the actor's disappearance, whichever
    /// happens first.
    ///
    /// #626: `run` answers every boundary message its final drain can still
    /// see, then drops its receivers. A message that reaches a queue between
    /// that drain and that drop is stranded — this handle's own sender keeps
    /// flume's queue alive, and with it the reply sender embedded in the
    /// stranded message, so waiting on the reply alone never disconnects and
    /// never returns. Teardown drops the actor's liveness sender after the
    /// boundary drain and driver drop, which resolves that wait with the
    /// session's terminal error instead.
    ///
    /// The reply is polled first, and re-checked once the actor is gone, so a
    /// message the drain *did* answer still returns its real answer.
    async fn await_boundary_reply<T>(&self, reply: &flume::Receiver<T>) -> Result<T, Error>
    where
        T: Send,
    {
        let answered = async {
            reply
                .recv_async()
                .await
                .map_err(|_| self.disconnected_error())
        };
        let actor_gone = async {
            // Nobody ever sends on this lane, so this resolves exactly once,
            // when actor teardown drops its end after the driver.
            while self.actor_alive.recv_async().await.is_ok() {}
            reply.try_recv().map_err(|_| self.disconnected_error())
        };
        future::or(answered, actor_gone).await
    }

    pub(crate) fn now(&self) -> Instant {
        self.clock.now()
    }

    pub(crate) fn sleep(
        &self,
        duration: Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.clock.sleep(duration)
    }

    pub(crate) fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
        async_observer_deadline(&self.clock, timeout)
    }

    pub(crate) fn receipt_control(&self) -> AsyncReceiptControl {
        AsyncReceiptControl {
            owner: self.clone(),
        }
    }

    /// Class-specific typed admission seam for ordinary commands.
    pub(crate) async fn submit_command(
        &self,
        prepared: crate::prepared::PreparedCommand,
    ) -> Result<AsyncCommandReceipt, Error> {
        prepared
            .admit_with(|request, timeout| async move {
                self.submit_with_timeout(request, timeout)
                    .await
                    .map(|core| AsyncCommandReceipt { core })
            })
            .await
    }

    /// Class-specific typed admission seam retaining the external decoder.
    pub(crate) async fn submit_inquiry<R>(
        &self,
        prepared: crate::prepared::PreparedInquiry<R>,
    ) -> Result<AsyncInquiryReceipt<R>, Error> {
        prepared
            .admit_with(|request, decoder, timeout| async move {
                self.submit_with_timeout(request, timeout)
                    .await
                    .map(|core| AsyncInquiryReceipt { core, decoder })
            })
            .await
    }

    async fn submit_inquiry_until<R>(
        &self,
        prepared: crate::prepared::PreparedInquiry<R>,
        deadline: Instant,
    ) -> Result<AsyncInquiryReceipt<R>, Error> {
        prepared
            .admit_with(|request, decoder, timeout| async move {
                self.submit_with_timeout_until(request, timeout, deadline)
                    .await
                    .map(|core| AsyncInquiryReceipt { core, decoder })
            })
            .await
    }

    /// Class-specific typed admission seam retaining operation semantics.
    pub(crate) async fn submit_operation<K>(
        &self,
        prepared: crate::prepared::PreparedOperation<K>,
    ) -> Result<AsyncOperationReceipt<K>, Error>
    where
        K: completion::Kind,
    {
        prepared
            .admit_with(|request, affected_axes, settlement, timeout| async move {
                self.submit_with_timeout(request, timeout)
                    .await
                    .map(|core| AsyncOperationReceipt {
                        core,
                        affected_axes,
                        settlement,
                        owner: self.clone(),
                        marker: PhantomData,
                    })
            })
            .await
    }

    /// Fails immediately when shared boundary/engine capacity is exhausted,
    /// then returns as soon as the actor applies the exact `Admitted` effect.
    // Used only by owner unit tests; the public session uses the typed seams.
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) async fn submit(&self, request: RuntimeRequest) -> Result<ReceiptCore, Error> {
        let timeout = if request.is_inquiry() {
            request.context().timeout.inquiry
        } else {
            request.context().timeout.completion
        };
        self.submit_with_timeout(request, timeout).await
    }

    async fn submit_with_timeout(
        &self,
        request: RuntimeRequest,
        configured_timeout: Duration,
    ) -> Result<ReceiptCore, Error> {
        let target = request.context().target;
        let (completion, admission) = self.enqueue_admission(request, None)?;
        match self.await_boundary_reply(&admission).await {
            Ok(Ok(id)) => Ok(ReceiptCore::new(
                id,
                target,
                completion,
                configured_timeout,
                Arc::clone(&self.origin),
            )),
            Ok(Err(error)) | Err(error) => Err(error),
        }
    }

    async fn submit_with_timeout_until(
        &self,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error> {
        let target = request.context().target;
        let lane = if request.is_inquiry() {
            RequestLane::Inquiry
        } else {
            RequestLane::Command
        };
        if self.now() >= deadline {
            let error = Error::Timeout;
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        }
        let validity = AdmissionValidity::until(deadline);
        let (completion, admission) = self.enqueue_admission(request, Some(validity.clone()))?;
        let remaining = deadline.saturating_duration_since(self.clock.now());
        let admitted = async { AdmissionWait::Reply(self.await_boundary_reply(&admission).await) };
        let timed_out = async {
            self.clock.sleep(remaining).await;
            AdmissionWait::Deadline {
                expired_before_admission: validity.expire_before_admission(),
            }
        };
        let id = match future::or(admitted, timed_out).await {
            AdmissionWait::Reply(Ok(Ok(id))) => id,
            AdmissionWait::Reply(Ok(Err(error)) | Err(error)) => return Err(error),
            AdmissionWait::Deadline {
                expired_before_admission: true,
            } => {
                let error = Error::Timeout;
                // Winning `expire_before_admission` is the sole caller-side
                // linearization point for this rejection. The actor observes
                // `ExpiredElsewhere` and deliberately does not record it again.
                self.record_pre_admission_rejection(target, lane, &error);
                return Err(error);
            }
            // The actor claimed this boundary before the caller could expire
            // it. Its reply is now authoritative; waiting for it preserves
            // normal post-admission observer-detach semantics instead of
            // leaving an admitted request behind a returned timeout.
            AdmissionWait::Deadline {
                expired_before_admission: false,
            } => match self.await_boundary_reply(&admission).await {
                Ok(Ok(id)) => id,
                Ok(Err(error)) | Err(error) => return Err(error),
            },
        };
        Ok(ReceiptCore::new(
            id,
            target,
            completion,
            configured_timeout,
            Arc::clone(&self.origin),
        ))
    }

    /// Non-waiting admission used by capacity-sensitive facades. Failure occurs
    /// before an observer or engine ID is created.
    // Used only by owner unit tests.
    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) fn try_submit(
        &self,
        request: RuntimeRequest,
    ) -> Result<impl Future<Output = Result<ReceiptCore, Error>> + Send + '_, Error> {
        let target = request.context().target;
        let configured_timeout = if request.is_inquiry() {
            request.context().timeout.inquiry
        } else {
            request.context().timeout.completion
        };
        let (completion, admission) = self.enqueue_admission(request, None)?;
        Ok(async move {
            match self.await_boundary_reply(&admission).await {
                Ok(Ok(id)) => Ok(ReceiptCore::new(
                    id,
                    target,
                    completion,
                    configured_timeout,
                    Arc::clone(&self.origin),
                )),
                Ok(Err(error)) | Err(error) => Err(error),
            }
        })
    }

    /// A full cancellation queue applies backpressure; cancellation is never
    /// discarded. Actor termination disconnects the sender and wakes all waits.
    async fn cancel_core(
        &self,
        receipt: ReceiptCore,
    ) -> Result<AsyncCancellationReceipt, RejectedCancellation> {
        if !Arc::ptr_eq(&receipt.origin, &self.origin) {
            return Err(RejectedCancellation::kept(
                receipt,
                Error::InvalidState("operation receipt belongs to a different owner".into()),
            ));
        }
        if let Some(observation) = receipt.completion.try_recv() {
            return Ok(AsyncCancellationReceipt {
                core: cancellation_receipt_for(receipt, Some(observation)),
            });
        }
        let (reply, receiver) = flume::bounded(1);
        // A full cancellation lane is backpressure, not loss; only a closed
        // lane fails, and it hands the boundary — receipt included — back.
        if let Err(returned) = self
            .cancellations
            .send_async(CancellationBoundary { receipt, reply })
            .await
        {
            return Err(RejectedCancellation::kept(
                returned.into_inner().receipt,
                self.disconnected_error(),
            ));
        }
        match self.await_boundary_reply(&receiver).await {
            // The owner is gone, so no receipt survives to be handed back and
            // none would be observable if it did.
            Err(error) => Err(RejectedCancellation::lost(error)),
            Ok(reply) => reply.map(|core| AsyncCancellationReceipt { core }),
        }
    }

    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) async fn cancel_test(
        &self,
        receipt: ReceiptCore,
    ) -> Result<AsyncCancellationReceipt, RejectedCancellation> {
        self.cancel_core(receipt).await
    }

    // Used only by owner unit tests.
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) async fn snapshot(&self) -> Result<OwnerSnapshot, Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::Snapshot(reply))
            .await
            .map_err(|_| self.disconnected_error())?;
        self.await_boundary_reply(&receiver).await
    }

    /// Reads scalar owner metrics through a dedicated bounded control request.
    /// This path never clones the diagnostic ring.
    pub(crate) async fn metrics(&self) -> Result<crate::observability::MetricsSnapshot, Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::Metrics(reply))
            .await
            .map_err(|_| self.disconnected_error())?;
        self.await_boundary_reply(&receiver).await?
    }

    pub(crate) fn state_cache(&self, target: crate::CameraId) -> crate::state_cache::StateCache {
        crate::state_cache::StateCache::from_registry(Arc::clone(&self.state_cache), target)
    }

    /// Reads the tuning the owner is currently preparing requests under.
    pub(crate) fn tuning(&self) -> crate::OperationalTuning {
        self.tuning.get()
    }

    /// Installs new session tuning through the owner's control boundary (#631).
    ///
    /// The actor applies the update on its own turn, so the write is ordered
    /// against every other boundary message and against the scheduler itself.
    /// This future resolves once the owner has applied it, which is what makes
    /// "the next request I prepare uses the new values" a guarantee rather than
    /// a race.
    pub(crate) async fn reconfigure(
        &self,
        validated_tuning: Result<crate::OperationalTuning, Error>,
    ) -> Result<(), Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::Reconfigure {
                validated_tuning: Box::new(validated_tuning),
                reply,
            })
            .await
            .map_err(|_| self.disconnected_error())?;
        self.await_boundary_reply(&receiver).await?
    }

    pub(crate) async fn subscribe_diagnostics(
        &self,
        capacity: usize,
    ) -> Result<DiagnosticSubscription, Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::SubscribeDiagnostics { capacity, reply })
            .await
            .map_err(|_| self.disconnected_error())?;
        self.await_boundary_reply(&receiver).await?
    }

    /// Coalesced idempotent shutdown. Only the winning caller occupies the
    /// single shutdown slot. The state lock covers the non-awaiting `try_send`,
    /// so concurrent callers observe the exact same accepted or failed result;
    /// the consuming `close` call waits separately on the liveness barrier.
    pub(crate) async fn shutdown(&self) -> Result<(), Error> {
        let mut signal = self
            .shutdown_signal
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Terminal publication and shutdown acceptance share this lifecycle
        // lock. Check the published engine verdict before occupying the
        // shutdown lane: a session that already terminalized itself must hand
        // that cause back to a cleanup caller rather than acknowledge a signal
        // no actor turn can consume.
        if let Some(error) = self
            .terminal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
        {
            if matches!(&error, Error::RuntimeShutdown)
                && matches!(&*signal, ShutdownSignalState::Accepted)
            {
                return Ok(());
            }
            *signal = ShutdownSignalState::Failed(error.clone());
            return Err(error);
        }
        match &*signal {
            ShutdownSignalState::Accepted => Ok(()),
            ShutdownSignalState::Failed(error) => Err(error.clone()),
            ShutdownSignalState::Open => match self.shutdown.try_send(()) {
                Ok(()) => {
                    *signal = ShutdownSignalState::Accepted;
                    Ok(())
                }
                Err(flume::TrySendError::Disconnected(_)) => {
                    let error = self.disconnected_error();
                    *signal = ShutdownSignalState::Failed(error.clone());
                    Err(error)
                }
                Err(flume::TrySendError::Full(_)) => {
                    // Only this method sends on the one-slot shutdown lane,
                    // so a full queue while the state is Open is an internal
                    // invariant failure. Remember it just like a disconnect,
                    // keeping every concurrent/repeated caller consistent.
                    let error = Error::InvalidState(
                        "shutdown signal lane was full before acceptance".into(),
                    );
                    *signal = ShutdownSignalState::Failed(error.clone());
                    Err(error)
                }
            },
        }
    }

    fn enqueue_admission(
        &self,
        request: RuntimeRequest,
        validity: Option<AdmissionValidity>,
    ) -> Result<
        (
            CompletionObserver,
            flume::Receiver<Result<RequestId, Error>>,
        ),
        Error,
    > {
        let target = request.context().target;
        let lane = if request.is_inquiry() {
            RequestLane::Inquiry
        } else {
            RequestLane::Command
        };
        if let Some(error) = self.admission_rejection() {
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        }
        let Some(permit) = self.permits.try_acquire() else {
            let error = Error::RuntimeQueueFull {
                capacity: self.permits.capacity(),
            };
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        };
        if let Some(error) = self.admission_rejection() {
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        }
        // Keep the terminal check and enqueue in one lifecycle critical
        // section. Otherwise a caller can pass the second check, the actor can
        // publish/drop and drain all boundaries, and this send can strand the
        // permit in a queue whose receiver will never poll it (#542 §4). Keep
        // the observer/reply/boundary construction after that check too: a
        // terminal pre-admission rejection must not allocate transient request
        // state before it returns.
        let signal = self
            .shutdown_signal
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(error) = match &*signal {
            ShutdownSignalState::Accepted => Some(Error::RuntimeShutdown),
            ShutdownSignalState::Failed(error) => Some(error.clone()),
            ShutdownSignalState::Open => self
                .terminal_error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        } {
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        }
        let (observer, completion) = completion_pair();
        let (reply, admission) = flume::bounded(1);
        let boundary = AdmissionBoundary {
            request,
            permit,
            observer,
            reply,
            validity,
            validity_claimed: false,
        };
        match self.admissions.try_send(boundary) {
            Ok(()) => Ok((completion, admission)),
            Err(flume::TrySendError::Disconnected(_)) => {
                let error = self.disconnected_error();
                self.record_pre_admission_rejection(target, lane, &error);
                Err(error)
            }
            Err(flume::TrySendError::Full(_)) => {
                let error =
                    Error::InvalidState("admission channel full after permit reservation".into());
                self.record_pre_admission_rejection(target, lane, &error);
                Err(error)
            }
        }
    }

    /// Reports a compact rejection without creating any admission-owned state.
    ///
    /// A full bounded control lane already has an actor turn reserved; a
    /// closed lane means teardown won and no live owner remains to deliver a
    /// diagnostic. The bounded ingress still retains the exact scalar total
    /// until that actor drops.
    fn record_pre_admission_rejection(
        &self,
        target: crate::CameraId,
        lane: RequestLane,
        error: &Error,
    ) {
        let wake = self.admission_rejections.record(PreAdmissionRejection {
            target,
            lane,
            error: error.kind(),
        });
        if wake {
            let _ = self
                .control
                .try_send(ControlBoundary::FlushAdmissionRejections);
        }
    }

    fn admission_rejection(&self) -> Option<Error> {
        let signal = self
            .shutdown_signal
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match &*signal {
            ShutdownSignalState::Accepted => Some(Error::RuntimeShutdown),
            ShutdownSignalState::Failed(error) => Some(error.clone()),
            ShutdownSignalState::Open => self
                .terminal_error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        }
    }

    fn missing_terminal_error() -> Error {
        Error::InvalidState("owner actor disconnected without publishing a terminal result".into())
    }

    fn disconnected_error(&self) -> Error {
        self.terminal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_else(Self::missing_terminal_error)
    }
}

/// The result of the one coalesced shutdown signal attempt. This is separate
/// from the actor's eventual terminal result: `shutdown` acknowledges only
/// acceptance into the bounded lane, while `close` waits for that result.
#[derive(Debug, Clone)]
enum ShutdownSignalState {
    Open,
    Accepted,
    Failed(Error),
}

#[derive(Debug)]
pub(crate) struct AsyncOwnerActor<R>
where
    R: Executor,
{
    state: OwnerState,
    admissions: flume::Receiver<AdmissionBoundary>,
    admission_rejections: Arc<AdmissionRejectionIngress>,
    /// Preallocated actor-owned scratch keeps ingress draining bounded without
    /// allocating on a rejected submission path.
    admission_rejection_scratch: VecDeque<PreAdmissionRejection>,
    /// Monotonic total already merged into `OwnerState` metrics.
    observed_pre_admission_rejections: u64,
    cancellations: flume::Receiver<CancellationBoundary>,
    control: flume::Receiver<ControlBoundary>,
    shutdown: flume::Receiver<()>,
    /// Dropped as `run` returns, after the driver/transport is explicitly
    /// dropped, disconnecting every handle's `actor_alive` receiver. Nothing
    /// is ever sent on it (#626).
    alive: flume::Sender<()>,
    /// Shared with the handle so terminal publication and a concurrent
    /// shutdown acceptance have one lifecycle linearization point. Without
    /// taking this lock while publishing the terminal error, a shutdown caller
    /// could observe `Open`, enqueue after the actor had already terminated,
    /// and return `Ok(())` even though no actor turn could ever consume it.
    shutdown_signal: Arc<Mutex<ShutdownSignalState>>,
    terminal_error: Arc<Mutex<Option<Error>>>,
    faults: TransientFaultRun,
    /// Consecutive receives that carried no data (an idle read timeout, or a
    /// driver that reports "no data" immediately). Escalates a cooperative
    /// pause so an immediately-returning idle read cannot hot-spin the actor,
    /// without recording a transport fault or spending any retry budget (#675).
    idle_receives: IdleReceiveRun,
    /// Engine-owned deadline currently keeping an ambiguous retained prefix
    /// under the old raw correlation scope (#713).
    raw_release_wait_until: Option<Instant>,
    runtime: Arc<R>,
}

impl<R> AsyncOwnerActor<R>
where
    R: Executor,
{
    pub(crate) fn new(policy: OwnerPolicy, runtime: R) -> Result<(AsyncOwnerHandle, Self), Error> {
        let state = OwnerState::new(policy)?;
        let runtime = Arc::new(runtime);
        let clock = BoundClock::from_shared(Arc::clone(&runtime));
        let origin = state.origin();
        let permits = state.permits();
        let state_cache = state.state_cache_registry();
        let tuning = state.live_tuning();
        let boundary_capacity = permits.capacity();
        // Cancellation is deliberately a small independent lane. Saturation
        // applies backpressure through `send_async`; it never falls back to a
        // lossy `try_send` path.
        let cancellation_capacity = 1;
        let control_capacity = state.policy().limits.applied_subscribers.max(1);
        let rejection_capacity = state.policy().limits.diagnostics;
        let (admission_tx, admissions) = flume::bounded(boundary_capacity);
        let (cancellation_tx, cancellations) = flume::bounded(cancellation_capacity);
        let (control_tx, control) = flume::bounded(control_capacity);
        let (shutdown_tx, shutdown) = flume::bounded(1);
        let (alive, actor_alive) = flume::bounded(1);
        let shutdown_signal = Arc::new(Mutex::new(ShutdownSignalState::Open));
        let terminal_error = Arc::new(Mutex::new(None));
        let admission_rejections = Arc::new(AdmissionRejectionIngress::new(rejection_capacity));
        Ok((
            AsyncOwnerHandle {
                permits,
                admissions: admission_tx,
                admission_rejections: Arc::clone(&admission_rejections),
                cancellations: cancellation_tx,
                control: control_tx,
                shutdown: shutdown_tx,
                actor_alive,
                shutdown_signal: Arc::clone(&shutdown_signal),
                terminal_error: Arc::clone(&terminal_error),
                origin,
                clock: clock.clone(),
                state_cache,
                tuning,
            },
            Self {
                state,
                admissions,
                admission_rejections,
                admission_rejection_scratch: VecDeque::with_capacity(rejection_capacity),
                observed_pre_admission_rejections: 0,
                cancellations,
                control,
                shutdown,
                alive,
                shutdown_signal: Arc::clone(&shutdown_signal),
                terminal_error,
                faults: TransientFaultRun::default(),
                idle_receives: IdleReceiveRun::default(),
                raw_release_wait_until: None,
                runtime,
            },
        ))
    }

    /// Publish a terminal engine verdict at the lifecycle linearization point.
    ///
    /// This is intentionally called while the terminal `SessionChanged` effect
    /// is being driven, rather than only from `run`'s epilogue. A shutdown
    /// caller can otherwise enter the still-live one-slot lane after
    /// `handle_event` has made the engine terminal but before the epilogue has
    /// run, and incorrectly receive `Ok(())`.
    fn publish_terminal_error(&self, error: Error) {
        let mut signal = self
            .shutdown_signal
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *self
            .terminal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(error.clone());
        // Only the caller whose signal produced an explicit shutdown retains
        // success. An engine close or poison supersedes even a signal accepted
        // earlier in the same ready-source race.
        if !matches!(&error, Error::RuntimeShutdown)
            || !matches!(&*signal, ShutdownSignalState::Accepted)
        {
            *signal = ShutdownSignalState::Failed(error);
        }
    }

    pub(crate) async fn run<D>(mut self, mut driver: D) -> OwnerSnapshot
    where
        D: AsyncOwnerDriver,
    {
        let runtime = Arc::clone(&self.runtime);
        // #542's deterministic source order keeps valid protocol input first:
        // an already-buffered ACK/completion settles state before concurrent
        // control observes it. A receive that makes no protocol progress
        // yields the next selection to the boundary channels. That makes an
        // always-failing or idle transport unable to starve shutdown,
        // cancellation, admission, control, or a due timer (#625), without the
        // previous arbitrary receive-history counter.
        //
        // The *succeeding* arm needs its own bound (#675): a peer that returns a
        // valid frame on every poll keeps winning the left-biased receive-first
        // selection forever and would starve those same boundary sources. A
        // fairness ceiling forces one boundary-first turn after this many
        // consecutive receive-first wins, restoring #625's acceptance criterion
        // — the boundary channels are always eventually polled — even against an
        // unbounded flood of valid frames. It is tied to the receive batch limit
        // because a burst that large is adversarial rather than a real camera's
        // reply stream, so the settle-first ordering above still holds for real
        // traffic.
        let fairness_ceiling = self.state.policy().limits.frames_per_receive.max(1);
        // Enforced around each receive so the caller's advertised read timeout is
        // live on the async surface, where the runtime-agnostic transports have
        // no timer of their own (#675). A timed-out read consumed nothing, so it
        // is reported as an idle no-data receive.
        let read_timeout = self.state.policy().read_timeout;
        let mut source_phase = SourcePhase::ReceiveFirst;
        let mut receive_first_streak: usize = 0;
        let mut buffered_receive_streak: usize = 0;
        let mut raw_buffered_cap_latched = false;
        // A ready `NoData`/non-consuming fault at an exact raw release is a
        // linearized proof that the one mandatory input probe found no frame.
        // It is deliberately scoped to the typed release set, not merely a
        // boolean: a new due raw scope must receive its own probe.
        let mut raw_release_no_input_fence: Option<RawCorrelationReleaseSet> = None;
        // Admission and cancellation are normally engine input turns.  Keep a
        // selected boundary here while the raw-release coordinator drains its
        // one mandatory input probe; otherwise `state.input` would run due
        // work and dispatch behind the unresolved evidence.
        let mut deferred_raw_boundary: Option<DeferredRawBoundary> = None;
        // The control lane normally precedes the timer. A due wake permits one
        // control observation, then wins the tail until it is advanced. Keep
        // the deadline that consumed that allowance rather than a bare bool:
        // a boundary can replace a wake, and a timer can become due while this
        // task is parked in the prior selection.
        //
        // Shutdown, cancellation, and admission remain above this tail.
        let mut control_allowance_consumed_for: Option<Instant> = None;
        loop {
            if self.state.state() != SessionState::Running {
                break;
            }
            // Even while the peer keeps making receive-first progress, force the
            // ordered boundary sources to the front once the streak reaches the
            // ceiling, then restart the count. When nothing is queued on a
            // boundary the receive still wins this turn, so a busy transport is
            // never stalled — only guaranteed to yield the front periodically.
            let yielded_boundary_turn = source_phase == SourcePhase::BoundariesFirst;
            let buffered_cap_reached =
                buffered_receive_streak >= RAW_CORRELATION_RELEASE_WORK_LIMIT;
            let forced_boundary_turn = source_phase == SourcePhase::ReceiveFirst
                && (receive_first_streak >= fairness_ceiling
                    || buffered_cap_reached
                    || raw_buffered_cap_latched);
            if forced_boundary_turn {
                receive_first_streak = 0;
                buffered_receive_streak = 0;
            }
            if forced_boundary_turn || yielded_boundary_turn {
                // Polling boundaries first alone is not a cooperative handoff:
                // if they are all pending, the ready receive wins immediately
                // and this task can monopolize a single-thread executor. This
                // covers both a forced fairness turn and every no-progress
                // receive's boundary-first retry, so shutdown, cancellation,
                // admission, control, and a due timer get one real executor
                // handoff without changing their fixed source order.
                cooperative_yield().await;
            }
            let effective_phase = if forced_boundary_turn {
                SourcePhase::BoundariesFirst
            } else {
                source_phase
            };
            // Compute this after the cooperative yield: a timer that became due
            // while another task ran must not inherit a stale positive delay.
            let now = Executor::now(runtime.as_ref());
            let wake_deadline = self.state.next_wake();
            let (wake_duration, wake_is_due) = wake_deadline.map_or_else(
                || (Duration::from_secs(86_400), false),
                |wake| {
                    let duration = wake.saturating_duration_since(now);
                    (duration, duration.is_zero())
                },
            );
            let raw_releases_due = self.state.raw_correlation_releases_due(now);
            let has_raw_release_due = !raw_releases_due.is_empty();
            if !has_raw_release_due {
                self.raw_release_wait_until = None;
            }
            if !has_raw_release_due
                || raw_release_no_input_fence.is_some_and(|fenced| fenced != raw_releases_due)
            {
                raw_release_no_input_fence = None;
            }
            let raw_release_is_fenced =
                raw_release_no_input_fence.is_some_and(|fenced| fenced == raw_releases_due);
            if wake_is_due && has_raw_release_due && buffered_cap_reached {
                raw_buffered_cap_latched = true;
            } else if !wake_is_due || !has_raw_release_due {
                raw_buffered_cap_latched = false;
            }
            // A retune or another higher-priority boundary can make the prior
            // wake irrelevant. Do not carry a consumed allowance over to an
            // absent, future, or replaced timer — including a replacement that
            // is already due.
            if !wake_is_due || control_allowance_consumed_for != wake_deadline {
                control_allowance_consumed_for = None;
            }
            let wake_precedes_control = wake_is_due
                && control_allowance_consumed_for
                    .is_some_and(|consumed| wake_deadline.is_some_and(|wake| wake == consumed));

            let event = {
                let frame_limit = self.state.policy().limits.frames_per_receive;
                // `future::or` is deliberately left-biased. Its nesting is the
                // normative all-ready order from #542; unlike `race`, it never
                // randomizes simultaneous readiness.
                let receive = async {
                    let result = Self::receive_within(
                        &mut driver,
                        self.state.buffers(),
                        frame_limit,
                        runtime.as_ref(),
                        read_timeout,
                    )
                    .await;
                    ActorEvent::Receive {
                        result,
                        received_at: Executor::now(runtime.as_ref()),
                    }
                };
                let shutdown = async {
                    match self.shutdown.recv_async().await {
                        Ok(()) => ActorEvent::Shutdown,
                        Err(_) => future::pending().await,
                    }
                };
                let cancellation = async {
                    match self.cancellations.recv_async().await {
                        Ok(value) => ActorEvent::Cancellation(value),
                        Err(_) => future::pending().await,
                    }
                };
                let admission = async { ActorEvent::Admission(self.admissions.recv_async().await) };
                let control = async {
                    match self.control.recv_async().await {
                        Ok(value) => ActorEvent::Control(value),
                        Err(_) => future::pending().await,
                    }
                };
                let wake = async {
                    // Do not rely on a zero-duration executor sleep being
                    // ready on its first poll. `next_wake` already established
                    // that this timer is due, and `ActorEvent::Wake` remains
                    // the only path that advances engine time.
                    if !wake_is_due {
                        Executor::sleep(runtime.as_ref(), wake_duration).await;
                    }
                    ActorEvent::Wake {
                        raw_buffered_cap_exhausted: raw_buffered_cap_latched,
                    }
                };
                let control_or_wake = select_control_or_wake(wake_precedes_control, control, wake);
                let boundaries = future::or(
                    shutdown,
                    future::or(cancellation, future::or(admission, control_or_wake)),
                );
                if raw_buffered_cap_latched {
                    // Shutdown is still the outer lifecycle priority.  Every
                    // other boundary, including cancellation/admission, must
                    // stay out of the engine once the retained-input cap has
                    // latched: fail closed before any successor can write.
                    future::or(
                        async {
                            match self.shutdown.recv_async().await {
                                Ok(()) => ActorEvent::Shutdown,
                                Err(_) => future::pending().await,
                            }
                        },
                        std::future::ready(ActorEvent::Wake {
                            raw_buffered_cap_exhausted: true,
                        }),
                    )
                    .await
                } else if has_raw_release_due {
                    if let Some(await_until) = self.raw_release_wait_until {
                        // An ambiguous retained prefix owns a real time budget,
                        // not a number of zero-time Wake polls (#713). Keep
                        // receive left-biased so a tail arriving inside the
                        // grace is decoded under the old correlation scope.
                        let release_wait = async {
                            let remaining = await_until.saturating_duration_since(now);
                            if !remaining.is_zero() {
                                Executor::sleep(runtime.as_ref(), remaining).await;
                            }
                            ActorEvent::Wake {
                                raw_buffered_cap_exhausted: false,
                            }
                        };
                        future::or(
                            async {
                                match self.shutdown.recv_async().await {
                                    Ok(()) => ActorEvent::Shutdown,
                                    Err(_) => future::pending().await,
                                }
                            },
                            future::or(receive, release_wait),
                        )
                        .await
                    } else if raw_release_is_fenced {
                        // The exact probe already returned no input.  Do not
                        // give an immediately-idle driver another chance to
                        // spin; wake now, still behind an explicit shutdown.
                        future::or(
                            async {
                                match self.shutdown.recv_async().await {
                                    Ok(()) => ActorEvent::Shutdown,
                                    Err(_) => future::pending().await,
                                }
                            },
                            std::future::ready(ActorEvent::Wake {
                                raw_buffered_cap_exhausted: false,
                            }),
                        )
                        .await
                    } else {
                        // The exact raw-release invariant: before due work can
                        // release correlation or dispatch, poll receive once
                        // left-biased against an already-ready wake.  A pending
                        // receive therefore lets Wake linearize the release;
                        // a ready complete stale frame wins first even after a
                        // prior `YieldBoundaries` turn.
                        future::or(
                            async {
                                match self.shutdown.recv_async().await {
                                    Ok(()) => ActorEvent::Shutdown,
                                    Err(_) => future::pending().await,
                                }
                            },
                            future::or(
                                receive,
                                std::future::ready(ActorEvent::Wake {
                                    raw_buffered_cap_exhausted: false,
                                }),
                            ),
                        )
                        .await
                    }
                } else if let Some(deferred) = deferred_raw_boundary.take() {
                    // The selected boundary predates the just-cleared raw
                    // gate.  Preserve shutdown's established priority without
                    // re-entering any channel ahead of it.  A signal arriving
                    // after this nonblocking check races exactly as it did with
                    // an ordinary already-selected boundary; the final drain
                    // retains the deferred payload if shutdown wins here.
                    match self.shutdown.try_recv() {
                        Ok(()) => {
                            deferred_raw_boundary = Some(deferred);
                            ActorEvent::Shutdown
                        }
                        Err(flume::TryRecvError::Empty | flume::TryRecvError::Disconnected) => {
                            match deferred {
                                DeferredRawBoundary::Admission(admission) => {
                                    ActorEvent::Admission(Ok(admission))
                                }
                                DeferredRawBoundary::Cancellation(cancellation) => {
                                    ActorEvent::Cancellation(cancellation)
                                }
                            }
                        }
                    }
                } else {
                    select_source(effective_phase, receive, boundaries).await
                }
            };

            // A raw release can mature while the ordinary selection is parked
            // (for example, concurrently with a ready cancellation or
            // admission).  Such a selected boundary has not touched engine
            // state yet, so retain it and restart at the coordinator instead
            // of allowing its normal `state.input` finish turn to advance due
            // work behind unread/raw-buffered evidence.
            // One sampled instant is carried through the selected boundary's
            // engine turn.  If it is still before H, that turn cannot cross H
            // merely because a later `Executor::now()` call happens a few
            // instructions later; the next loop then enters the raw-release
            // coordinator.  If it is at/after H, the checks below gate it.
            let selected_at = Executor::now(runtime.as_ref());
            let raw_releases_due_after_selection =
                self.state.raw_correlation_releases_due(selected_at);
            let event = if !raw_releases_due_after_selection.is_empty() {
                match event {
                    ActorEvent::Wake { .. } if !has_raw_release_due => {
                        // The raw deadline matured while an ordinary
                        // boundary-first selection was parked.  This Wake was
                        // selected from the pre-expiry view, so it has not yet
                        // earned the exact input-first release pass. Restart
                        // through the coordinator; its next selection polls
                        // receive left-biased against the now-ready wake.
                        source_phase = SourcePhase::ReceiveFirst;
                        continue;
                    }
                    ActorEvent::Admission(Ok(mut admission)) => {
                        if self.claim_admission_boundary(&mut admission, selected_at) {
                            debug_assert!(deferred_raw_boundary.is_none());
                            deferred_raw_boundary = Some(DeferredRawBoundary::Admission(admission));
                        }
                        // Do not reset the retained-input work counters: this
                        // boundary is intentionally invisible to scheduling
                        // until the raw coordinator has resolved the release.
                        source_phase = SourcePhase::ReceiveFirst;
                        continue;
                    }
                    ActorEvent::Cancellation(cancellation) => {
                        if let Some(observation) = cancellation.receipt.completion.try_recv() {
                            let _ = cancellation.reply.try_send(Ok(cancellation_receipt_for(
                                cancellation.receipt,
                                Some(observation),
                            )));
                        } else {
                            debug_assert!(deferred_raw_boundary.is_none());
                            deferred_raw_boundary =
                                Some(DeferredRawBoundary::Cancellation(cancellation));
                        }
                        // See the admission case above: a cancellation receipt
                        // may be observed, but a live cancellation must not
                        // enter its due-running engine turn yet.
                        source_phase = SourcePhase::ReceiveFirst;
                        continue;
                    }
                    event => event,
                }
            } else {
                event
            };

            // The special probe is intentionally one poll only.  A ready
            // no-input result fences this exact typed release set so the next
            // turn may wake without an immediate-NoData spin.  A transient
            // receive fault, by contrast, cannot prove the transport has no
            // ready stale input behind it: keep its engine input out of the due
            // pass, then take one fresh receive-vs-Wake arbitration.
            // The receive event records *when the read completed*.  Do not
            // mistake a later executor resume for a post-H input probe: a
            // `NoData` stamped H−ε but handled at H cannot fence a stale frame
            // which became ready during that gap.  Only the release set at
            // `received_at` proves an input result was actually sampled at or
            // after H.
            let raw_releases_due_at_receive = match &event {
                ActorEvent::Receive { received_at, .. } => {
                    self.state.raw_correlation_releases_due(*received_at)
                }
                _ => RawCorrelationReleaseSet::default(),
            };
            let receive_crossed_into_raw_release =
                !has_raw_release_due && !raw_releases_due_at_receive.is_empty();
            let raw_release_probe =
                (has_raw_release_due && !raw_release_is_fenced && !raw_buffered_cap_latched)
                    || receive_crossed_into_raw_release;
            let raw_release_probe_set = if receive_crossed_into_raw_release {
                raw_releases_due_at_receive
            } else {
                raw_releases_due
            };
            let raw_probe_no_input = raw_release_probe
                && match &event {
                    ActorEvent::Receive {
                        result: Ok(AsyncReceive::NoData),
                        ..
                    } => true,
                    ActorEvent::Receive {
                        result: Ok(AsyncReceive::Fault(error)),
                        ..
                    } => super::receive_reported_no_data(error),
                    _ => false,
                };
            // A normalized idle fault is semantically identical to `NoData`:
            // it consumed no bytes and is the exact no-input proof that lets
            // the due Wake proceed.  A genuine transient fault is different:
            // it cannot prove there is no ready stale frame behind it, so its
            // input turn must suppress due and force one fresh probe.
            let raw_probe_transient_fault = raw_release_probe
                && matches!(
                    &event,
                    ActorEvent::Receive {
                        result: Ok(AsyncReceive::Fault(error)),
                        ..
                    } if !super::receive_reported_no_data(error)
                );
            if raw_probe_no_input {
                raw_release_no_input_fence = Some(raw_release_probe_set);
            }
            if matches!(&event, ActorEvent::Wake { .. }) {
                // A wake either advances the release or deliberately defers it
                // behind retained framing.  In both cases the following turn
                // needs a fresh receive probe rather than reusing a prior
                // no-input observation.
                raw_release_no_input_fence = None;
            }

            // A receive that keeps the session running and makes protocol
            // progress is the only thing that lengthens the streak; any boundary
            // turn (or a non-progressing receive, which already yields) resets it
            // so the ceiling only ever fires against a genuine receive flood.
            let event_was_receive = matches!(event, ActorEvent::Receive { .. });
            // `wake_is_due` describes the instant before the selection began.
            // If the timer matured while both tail futures were parked, the
            // left-biased control future can still win this poll. Re-sample the
            // engine before the control handler mutates it and charge that
            // control to the exact selected wake in either case.
            let control_consumed_due_wake = matches!(&event, ActorEvent::Control(_))
                .then(|| {
                    let observed_at = Executor::now(runtime.as_ref());
                    self.state
                        .next_wake()
                        .filter(|current| Some(*current) == wake_deadline)
                        .filter(|wake| *wake <= observed_at)
                })
                .flatten();
            let wake_won = matches!(&event, ActorEvent::Wake { .. });
            let outcome = self
                .handle_event(
                    event,
                    &mut driver,
                    runtime.as_ref(),
                    selected_at,
                    raw_probe_transient_fault,
                )
                .await;
            if wake_won {
                control_allowance_consumed_for = None;
            } else if let Some(wake) = control_consumed_due_wake {
                control_allowance_consumed_for = Some(wake);
            }
            if event_was_receive {
                match outcome {
                    TurnOutcome::Continue => {
                        receive_first_streak = receive_first_streak.saturating_add(1);
                        buffered_receive_streak = 0;
                        raw_buffered_cap_latched = false;
                    }
                    TurnOutcome::ContinueBuffered => {
                        receive_first_streak = 0;
                        buffered_receive_streak = buffered_receive_streak.saturating_add(1);
                    }
                    TurnOutcome::YieldBoundaries | TurnOutcome::Stop => {
                        receive_first_streak = 0;
                        buffered_receive_streak = 0;
                    }
                }
            } else {
                receive_first_streak = 0;
                buffered_receive_streak = 0;
            }
            match outcome.next_source_phase() {
                Some(next) => source_phase = next,
                None => break,
            }
        }
        let boundary_error = self
            .state
            .boundary_error()
            .unwrap_or(Error::RuntimeShutdown);
        self.publish_terminal_error(boundary_error.clone());
        self.flush_pre_admission_rejections(true);
        if let Some(deferred) = deferred_raw_boundary.take() {
            self.drain_deferred_raw_boundary(deferred, boundary_error.clone());
        }
        self.drain_boundaries(boundary_error);
        #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
        let snapshot = self.snapshot_now();
        #[cfg(not(all(test, any(feature = "runtime-tokio", feature = "runtime-smol"))))]
        let snapshot = OwnerSnapshot {};
        // #626: drop the driver first so a waiter using the liveness lane gets
        // a deterministic transport-release barrier. The actor's `Drop`
        // implementation performs one final boundary drain before its alive
        // sender is dropped, covering messages that race the explicit drain.
        drop(driver);
        snapshot
    }

    async fn handle_event<D>(
        &mut self,
        event: ActorEvent,
        driver: &mut D,
        runtime: &R,
        selected_at: Instant,
        suppress_due_for_raw_probe_fault: bool,
    ) -> TurnOutcome
    where
        D: AsyncOwnerDriver,
    {
        match event {
            ActorEvent::Shutdown => {
                self.terminate_at(driver, runtime, ShutdownReason::Explicit, selected_at)
                    .await;
                TurnOutcome::Stop
            }
            ActorEvent::Cancellation(cancel) => {
                self.handle_cancellation(cancel, driver, runtime, selected_at)
                    .await;
                TurnOutcome::Continue
            }
            ActorEvent::Control(control) => {
                self.handle_control(control);
                TurnOutcome::Continue
            }
            ActorEvent::Admission(Ok(admission)) => {
                self.handle_admission(admission, driver, runtime, selected_at)
                    .await;
                TurnOutcome::Continue
            }
            ActorEvent::Admission(Err(_)) => {
                self.terminate_at(driver, runtime, ShutdownReason::Explicit, selected_at)
                    .await;
                TurnOutcome::Stop
            }
            ActorEvent::Wake {
                raw_buffered_cap_exhausted,
            } => {
                let now = selected_at;
                if raw_buffered_cap_exhausted {
                    let error = Error::InvalidState(
                        "async raw correlation release retained-input work cap exhausted".into(),
                    );
                    self.terminate_at(
                        driver,
                        runtime,
                        ShutdownReason::FramingFailure {
                            reason: error.to_string().into_boxed_str(),
                        },
                        now,
                    )
                    .await;
                    return TurnOutcome::Stop;
                }
                // A due raw-correlation release is the last point at which
                // input retained by a byte-stream framer can still belong to
                // the old request. Receive-first selection above gives a
                // simultaneously ready completing tail precedence. If the
                // tail is not ready, remove the orphaned prefix before the due
                // pass can dispatch a successor; otherwise that successor
                // could consume the old frame after its tail arrives.
                let releases = self.state.raw_correlation_releases_due(now);
                if !releases.is_empty() {
                    let work_limit = RAW_CORRELATION_RELEASE_WORK_LIMIT;
                    let framing = (|| -> Result<Option<Instant>, Error> {
                        let mut discarded = 0_usize;
                        loop {
                            let buffered = driver.has_buffered_stream_input()?;
                            let input = driver.buffered_stream_input()?;
                            if !buffered {
                                if input.is_some() {
                                    return Err(Error::InvalidState(
                                        "async stream decoder attributed absent buffered input"
                                            .into(),
                                    ));
                                }
                                return match self.state.resolve_raw_release_gate(now, None) {
                                    RawReleaseGateAction::Advance => Ok(None),
                                    RawReleaseGateAction::AwaitInputUntil(deadline) => {
                                        Ok(Some(deadline))
                                    }
                                    RawReleaseGateAction::DiscardFirst => Err(Error::InvalidState(
                                        "raw release gate requested a discard without retained input"
                                            .into(),
                                    )),
                                };
                            }
                            let input = input.ok_or_else(|| {
                                Error::InvalidState(
                                    "async raw stream input could not be attributed before correlation release"
                                        .into(),
                                )
                            })?;
                            match self.state.resolve_raw_release_gate(now, Some(input)) {
                                RawReleaseGateAction::Advance => return Ok(None),
                                RawReleaseGateAction::AwaitInputUntil(deadline) => {
                                    return Ok(Some(deadline));
                                }
                                RawReleaseGateAction::DiscardFirst => {
                                    if discarded >= work_limit {
                                        return Err(Error::InvalidState(
                                            "async raw correlation release framing work cap exhausted"
                                                .into(),
                                        ));
                                    }
                                    // Drop exactly the first framed fragment,
                                    // then ask the shared engine again. This
                                    // preserves later serial input and prevents
                                    // a stale fragment from crossing into a
                                    // successor's correlation interval.
                                    driver.discard_buffered_stream_input()?;
                                    let _ = self.state.apply_effect(Effect::Ignored(
                                        IgnoreReason::MalformedFrame,
                                    ));
                                    discarded = discarded.saturating_add(1);
                                }
                            }
                        }
                    })();
                    let await_input_until = match framing {
                        Ok(await_input_until) => await_input_until,
                        Err(error) => {
                            self.terminate_at(
                                driver,
                                runtime,
                                ShutdownReason::FramingFailure {
                                    reason: error.to_string().into_boxed_str(),
                                },
                                now,
                            )
                            .await;
                            return TurnOutcome::Stop;
                        }
                    };
                    if let Some(deadline) = await_input_until {
                        self.raw_release_wait_until = Some(deadline);
                        return TurnOutcome::ContinueBuffered;
                    }
                }
                self.raw_release_wait_until = None;
                let effects = self.state.advance(now);
                self.drive(driver, effects, runtime).await;
                TurnOutcome::Continue
            }
            ActorEvent::Receive {
                result: Ok(AsyncReceive::Closed),
                received_at,
            } => {
                self.terminate_at(
                    driver,
                    runtime,
                    ShutdownReason::TransportClosed { reason: None },
                    received_at,
                )
                .await;
                TurnOutcome::Stop
            }
            ActorEvent::Receive {
                result: Ok(AsyncReceive::NoData),
                received_at,
            } => {
                // An expired idle read timeout is not a fault: nothing was
                // consumed, nothing failed, and no request's retry budget is
                // touched. It made no protocol progress, so let a queued boundary
                // run before another idle read (#625). A driver that returns
                // NoData immediately would otherwise spin the actor, so pace the
                // idle-read rate (#675).
                match driver.has_buffered_stream_input() {
                    Ok(buffered) => self.absorb_idle_receive(runtime, buffered).await,
                    Err(error) => {
                        self.discard_undecodable_receive(driver, runtime, &error, received_at)
                            .await
                    }
                }
            }
            ActorEvent::Receive {
                result: Ok(AsyncReceive::Frames(frames)),
                received_at,
            } => {
                self.faults.reset();
                // Real bytes decoded: the transport is not idle, so restart the
                // no-data escalation (#675).
                self.idle_receives.reset();
                // #672: a stream tolerates a delimited frame that did not
                // classify by discarding it and staying Running, exactly as a
                // datagram already does and as 1.x did (log-and-continue). Record
                // one Ignored per discarded frame so the discard stays
                // observable; the session is never poisoned for it.
                for _ in 0..self.state.buffers().take_discarded_malformed() {
                    let _ = self
                        .state
                        .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
                }
                if let Err(error) = self.state.validate_frame_batch(&frames) {
                    return self
                        .discard_undecodable_receive(driver, runtime, &error, received_at)
                        .await;
                }
                if frames.is_empty() {
                    // The read carried bytes that did not finish a frame, or only
                    // frames that were discarded above. The framer holds any
                    // partial frame; keep pumping so the rest of it can arrive in
                    // a later read.
                    return match driver.has_buffered_stream_input() {
                        Ok(true) => TurnOutcome::ContinueBuffered,
                        Ok(false) => TurnOutcome::YieldBoundaries,
                        Err(error) => {
                            self.discard_undecodable_receive(driver, runtime, &error, received_at)
                                .await
                        }
                    };
                }
                let turn = self.state.begin_input_turn(received_at);
                for frame in frames {
                    let effects = self.state.input_in_turn(&turn, Input::Frame(frame));
                    self.drive_in_turn(driver, &turn, effects, runtime).await;
                }
                let buffered = match driver.has_buffered_stream_input() {
                    Ok(buffered) => buffered,
                    Err(error) => {
                        let inert = self.state.finish_input_turn(turn, EngineTurn::INPUT_ONLY);
                        self.drive(driver, inert, runtime).await;
                        return self
                            .discard_undecodable_receive(driver, runtime, &error, received_at)
                            .await;
                    }
                };
                // A frame-limit batch can leave a complete frame followed by
                // an incomplete stale prefix in the production framer. Do not
                // run due work until each complete retained frame has taken
                // its ordered input turn and any final orphan is classified at
                // the raw release boundary.
                let due = if buffered {
                    self.state.finish_input_turn(turn, EngineTurn::INPUT_ONLY)
                } else {
                    self.state.finish_input_turn(turn, EngineTurn::COMPLETE)
                };
                self.drive(driver, due, runtime).await;
                if buffered {
                    TurnOutcome::ContinueBuffered
                } else {
                    // A completed input turn drained the retained fragment
                    // before due work ran. If that input settled the old raw
                    // correlation at an exact boundary, `finish_input_turn`
                    // released it without another Wake turn, so its next
                    // independent hold must start with a fresh grace budget.
                    // Do not reset for an empty/partial receive: its unresolved
                    // prefix still owns the current deadline.
                    self.raw_release_wait_until = None;
                    TurnOutcome::Continue
                }
            }
            ActorEvent::Receive {
                result: Ok(AsyncReceive::Fault(error)),
                received_at,
            } => {
                if super::receive_reported_no_data(&error) {
                    // A driver that reports an idle timeout as a fault still
                    // means "no bytes arrived". Normalizing here as well as at
                    // the adapter keeps every driver on one contract (#637), and
                    // paces the idle-read rate so it cannot hot-spin (#675).
                    return match driver.has_buffered_stream_input() {
                        Ok(buffered) => self.absorb_idle_receive(runtime, buffered).await,
                        Err(error) => {
                            self.discard_undecodable_receive(driver, runtime, &error, received_at)
                                .await
                        }
                    };
                }
                if !super::receive_fault_is_transient(&error) {
                    self.terminate_at(
                        driver,
                        runtime,
                        ShutdownReason::TransportClosed {
                            reason: Some(error.to_string().into_boxed_str()),
                        },
                        received_at,
                    )
                    .await;
                    return TurnOutcome::Stop;
                }
                let (length, span) = self.faults.record(received_at);
                if TransientFaultRun::is_permanent(length, span) {
                    // A read that has failed this many times in a row, over
                    // this long, is a broken transport rather than a transient
                    // fault. End the session with the cause rather than
                    // retrying against it forever (#625).
                    self.terminate_at(
                        driver,
                        runtime,
                        ShutdownReason::TransportClosed {
                            reason: Some(
                                format!("{length} consecutive receive faults: {error}")
                                    .into_boxed_str(),
                            ),
                        },
                        received_at,
                    )
                    .await;
                    return TurnOutcome::Stop;
                }
                // The engine safely retries sequenced Sony work with its same
                // sequence; a raw command awaiting ACK is left to its own ACK
                // deadline (issue #671; the strict opt-in poisons instead)
                // rather than being replayed. The pause mirrors 1.x's guard
                // against hot-looping on an immediately failing transport; it
                // grows with the run and is clamped to the next scheduler
                // deadline exactly as the blocking owner clamps to its caller's.
                let buffered = match driver.has_buffered_stream_input() {
                    Ok(buffered) => buffered,
                    Err(framing_error) => {
                        return self
                            .discard_undecodable_receive(
                                driver,
                                runtime,
                                &framing_error,
                                received_at,
                            )
                            .await;
                    }
                };
                let effects = if buffered || suppress_due_for_raw_probe_fault {
                    let turn = self.state.begin_input_turn(received_at);
                    let mut effects = self
                        .state
                        .input_in_turn(&turn, Input::ReceiveFault { error });
                    effects.extend(self.state.finish_input_turn(turn, EngineTurn::INPUT_ONLY));
                    effects
                } else {
                    self.state.input(Input::ReceiveFault { error }, received_at)
                };
                self.drive(driver, effects, runtime).await;
                let pause = clamp_receive_pause(
                    transient_receive_pause(length),
                    self.state.next_wake(),
                    Executor::now(runtime),
                );
                if !pause.is_zero() {
                    Executor::sleep(runtime, pause).await;
                }
                if buffered {
                    TurnOutcome::ContinueBuffered
                } else {
                    TurnOutcome::YieldBoundaries
                }
            }
            ActorEvent::Receive {
                result: Err(error),
                received_at,
            } => {
                self.discard_undecodable_receive(driver, runtime, &error, received_at)
                    .await
            }
        }
    }

    /// Handle bytes that were consumed but did not decode.
    ///
    /// On a byte stream the position is now unknowable, so the session is
    /// poisoned. On a datagram transport it is one bad datagram: nothing else
    /// was consumed, the next datagram frames independently, and the blocking
    /// owner has always failed this per request and kept pumping (#637).
    async fn discard_undecodable_receive<D>(
        &mut self,
        driver: &mut D,
        runtime: &R,
        error: &Error,
        received_at: Instant,
    ) -> TurnOutcome
    where
        D: AsyncOwnerDriver,
    {
        if self.state.policy().protocol.transport != TransportKind::Stream {
            // The adapter reached this path only after consuming one complete
            // datagram (including an oversized datagram whose copied prefix was
            // rejected). That successful read proves the transport is live even
            // though its payload is unusable, so it clears both consecutive
            // receive-fault accounting and the idle no-data pacing run before
            // the discard becomes observable. A stream takes the terminal path
            // below and deliberately retains its existing framing semantics.
            self.faults.reset();
            self.idle_receives.reset();
            let _ = self
                .state
                .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
            return TurnOutcome::YieldBoundaries;
        }
        self.terminate_at(
            driver,
            runtime,
            ShutdownReason::FramingFailure {
                reason: error.to_string().into_boxed_str(),
            },
            received_at,
        )
        .await;
        TurnOutcome::Stop
    }

    /// A receive that carried no data made no protocol progress, so yield the
    /// next selection to the boundary sources. Before doing so, pace the
    /// idle-read rate: a transport that returns "no data" immediately (rather
    /// than after its read timeout) would otherwise spin the actor at hundreds
    /// of thousands of reads a second (#675). The pause escalates with the run
    /// and is clamped to the next scheduler deadline, exactly like the transient
    /// receive-fault pause — but it records no fault, does not clear an existing
    /// fault run, and spends no retry budget. Only a successful read or the
    /// five-second fault gap proves transient failures stopped accumulating.
    async fn absorb_idle_receive(
        &mut self,
        runtime: &R,
        buffered_stream_input: bool,
    ) -> TurnOutcome {
        let pause = clamp_receive_pause(
            self.idle_receives.record(),
            self.state.next_wake(),
            Executor::now(runtime),
        );
        if !pause.is_zero() {
            Executor::sleep(runtime, pause).await;
        }
        if buffered_stream_input {
            TurnOutcome::ContinueBuffered
        } else {
            TurnOutcome::YieldBoundaries
        }
    }

    /// Claims the one-way pre-admission deadline race before this boundary is
    /// either staged immediately or held behind an exact raw-release gate.
    /// Returning `false` means this method already answered the caller.
    fn claim_admission_boundary(
        &mut self,
        admission: &mut AdmissionBoundary,
        now: Instant,
    ) -> bool {
        if admission.validity_claimed {
            return true;
        }
        let target = admission.request.context().target;
        let lane = if admission.request.is_inquiry() {
            RequestLane::Inquiry
        } else {
            RequestLane::Command
        };
        let Some(validity) = admission.validity.as_ref() else {
            return true;
        };
        match validity.claim_for_admission(now) {
            AdmissionClaim::Claimed => {
                admission.validity_claimed = true;
                true
            }
            AdmissionClaim::ExpiredHere => {
                let error = Error::Timeout;
                self.state.record_admission_rejection(target, lane, &error);
                let _ = admission.reply.try_send(Err(error));
                false
            }
            AdmissionClaim::ExpiredElsewhere => {
                let _ = admission.reply.try_send(Err(Error::Timeout));
                false
            }
        }
    }

    async fn handle_admission<D>(
        &mut self,
        mut admission: AdmissionBoundary,
        driver: &mut D,
        runtime: &R,
        accepted_at: Instant,
    ) where
        D: AsyncOwnerDriver,
    {
        // A deadline that wins before this exact boundary is admitted is not
        // observer detachment: no engine entry exists yet. Drop the boundary
        // (and therefore its permit and observer) before staging any engine
        // input, so a stale admission can never become a later write. The
        // winner records the rejection exactly once: caller-expiry already
        // entered the handle-side ingress, while actor-expiry records directly
        // into the serialized owner state.
        if !self.claim_admission_boundary(&mut admission, accepted_at) {
            return;
        }
        let input = self.state.stage_admission_with(
            admission.request,
            admission.permit,
            admission.observer,
            admission.reply,
        );
        let effects = self.state.input(input, accepted_at);
        self.drive(driver, effects, runtime).await;
    }

    async fn handle_cancellation<D>(
        &mut self,
        cancellation: CancellationBoundary,
        driver: &mut D,
        runtime: &R,
        accepted_at: Instant,
    ) where
        D: AsyncOwnerDriver,
    {
        let id = cancellation.receipt.id;
        if let Some(observation) = cancellation.receipt.completion.try_recv() {
            let _ = cancellation.reply.try_send(Ok(cancellation_receipt_for(
                cancellation.receipt,
                Some(observation),
            )));
            return;
        }
        let registration = self.state.register_cancellation(id);
        let effects = self.state.input(Input::Cancel { id }, accepted_at);
        self.drive(driver, effects, runtime).await;
        // A refusal leaves the original request scheduled, so the receipt
        // travels back to the caller instead of dying here (#612).
        let result = match registration.acknowledgement.try_recv() {
            Ok(Ok(())) => Ok(cancellation_receipt_for(cancellation.receipt, None)),
            Ok(Err(error)) => Err(RejectedCancellation::kept(cancellation.receipt, error)),
            Err(_) => Err(RejectedCancellation::kept(
                cancellation.receipt,
                Error::InvalidState("engine did not acknowledge cancellation input".into()),
            )),
        };
        let _ = cancellation.reply.try_send(result);
    }

    /// Merges handle-side, pre-boundary rejection telemetry into actor-owned
    /// metrics and diagnostic delivery.
    ///
    /// A control request flushes opportunistically as well as an explicit
    /// ingress wake. That makes a `metrics()` or diagnostics subscription
    /// issued immediately after a fail-fast rejection observe that rejection
    /// without relying on scheduler timing.
    fn flush_pre_admission_rejections(&mut self, consumed_wake: bool) {
        let dropped_diagnostics = self
            .admission_rejections
            .drain_into(&mut self.admission_rejection_scratch, consumed_wake);
        // Load after the ingress lock is released. A reporter that raced this
        // drain either contributed an event to this batch or observed the
        // cleared wake marker and reserved the next actor wake, so its scalar
        // count cannot be stranded behind an already-consumed notification.
        let total = self.admission_rejections.total();
        let new_rejections = total.saturating_sub(self.observed_pre_admission_rejections);
        if new_rejections != 0 {
            self.state.record_pre_admission_rejections(new_rejections);
            self.observed_pre_admission_rejections = total;
        }
        if dropped_diagnostics != 0 {
            self.state.record_dropped_diagnostics(dropped_diagnostics);
        }
        while let Some(rejection) = self.admission_rejection_scratch.pop_front() {
            self.state.record_admission_rejection_diagnostic(
                rejection.target,
                rejection.lane,
                rejection.error,
            );
        }
    }

    fn handle_control(&mut self, control: ControlBoundary) {
        self.flush_pre_admission_rejections(true);
        match control {
            ControlBoundary::FlushAdmissionRejections => {}
            #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
            ControlBoundary::Snapshot(reply) => {
                let _ = reply.try_send(self.snapshot_now());
            }
            ControlBoundary::Metrics(reply) => {
                let _ = reply.try_send(Ok(self.state.metrics_snapshot()));
            }
            ControlBoundary::SubscribeDiagnostics { capacity, reply } => {
                let result = self.state.subscribe_diagnostics(capacity);
                let _ = reply.try_send(result);
            }
            ControlBoundary::Reconfigure {
                validated_tuning,
                reply,
            } => {
                let result = match self.state.boundary_error() {
                    Some(error) => Err(error),
                    None => (*validated_tuning).and_then(|tuning| self.state.retune(tuning)),
                };
                let _ = reply.try_send(result);
            }
        }
    }

    /// Run one transport read under the session's read timeout, expressed as a
    /// left-biased race so the caller's advertised knob is live on the async
    /// surface, where the runtime-agnostic transports carry no timer of their
    /// own (#675). A read that produces data always wins the left bias; only a
    /// read that outlasts `read_timeout` yields the idle branch, which consumed
    /// nothing and so reports no data rather than a fault. Free of `self`, with
    /// explicit borrows, so the spawned actor future stays `Send` for any
    /// lifetime.
    async fn receive_within<D>(
        driver: &mut D,
        buffers: &mut super::OwnerBuffers,
        frame_limit: usize,
        runtime: &R,
        read_timeout: Duration,
    ) -> Result<AsyncReceive, Error>
    where
        D: AsyncOwnerDriver,
    {
        let idle_after_timeout = async {
            Executor::sleep(runtime, read_timeout).await;
            Ok(AsyncReceive::NoData)
        };
        future::or(driver.receive(buffers, frame_limit), idle_after_timeout).await
    }

    /// Run one transport write under the session's write timeout so a stalled
    /// peer (a zero receive window, serial flow control) cannot park the actor
    /// and block `close()` forever (#675). A timeout surfaces as a write
    /// failure, which the engine turns into a byte-stream poison or an isolated
    /// datagram failure exactly as an underlying `send` error would — never an
    /// indefinite park. Free of `self` so it can run while the `WireWrite`
    /// borrows the owner state.
    async fn write_frame<D>(
        driver: &mut D,
        write: WireWrite<'_>,
        runtime: &R,
        write_timeout: Duration,
    ) -> Result<TransmissionMeta, Error>
    where
        D: AsyncOwnerDriver,
    {
        // A left-biased race, matching the actor's other bounded waits: the
        // write is polled first, so a write that completes always wins and only
        // one that outlasts `write_timeout` yields the failure branch. Using
        // `future::or` over `Executor::timeout` keeps the spawned actor future
        // `Send` for any lifetime (the trait's `timeout` return type binds the
        // wrapped future to the executor borrow's lifetime, which a spawn cannot
        // prove generally); both are executor-neutral and behave identically.
        let timed_out = async {
            Executor::sleep(runtime, write_timeout).await;
            Err(Error::TransportError(
                format!("transport write did not complete within {write_timeout:?}").into(),
            ))
        };
        future::or(driver.write(write), timed_out).await
    }

    async fn drive<D>(&mut self, driver: &mut D, mut effects: VecDeque<Effect>, runtime: &R)
    where
        D: AsyncOwnerDriver,
    {
        let write_timeout = self.state.policy().write_timeout;
        while let Some(effect) = effects.pop_front() {
            let terminal_transition = matches!(
                &effect,
                Effect::SessionChanged { to, .. } if *to != SessionState::Running
            );
            let applied = self.state.apply_effect(effect);
            if terminal_transition {
                let error = self
                    .state
                    .boundary_error()
                    .unwrap_or_else(AsyncOwnerHandle::missing_terminal_error);
                self.publish_terminal_error(error);
            }
            if let AppliedEffect::Transmit(staged) = applied {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => Self::write_frame(driver, write, runtime, write_timeout).await,
                    Err(error) => Err(error),
                };
                let finished_at = Executor::now(runtime);
                // A write can complete while the actor is awaiting the
                // transport and cross a raw correlation deadline.  Its
                // transmission-result input is real ordered protocol input,
                // but it must not run the due/dispatch tail before the loop's
                // raw-release coordinator has taken its mandatory receive
                // probe.  `drive_in_turn` already has this property by using
                // the active input turn; this is the equivalent seam for a
                // standalone effect chain.
                let produced = if self
                    .state
                    .raw_correlation_releases_due(finished_at)
                    .is_empty()
                {
                    self.state.finish_write(&staged, write_result, finished_at)
                } else {
                    self.state.finish_write_turn(
                        &staged,
                        write_result,
                        finished_at,
                        EngineTurn::INPUT_ONLY,
                    )
                };
                // The exact completion is recursively processed before the
                // next source effect or any channel input.
                prepend_effects(&mut effects, produced);
            }
        }
    }

    async fn drive_in_turn<D>(
        &mut self,
        driver: &mut D,
        turn: &OwnerInputTurn,
        mut effects: VecDeque<Effect>,
        runtime: &R,
    ) where
        D: AsyncOwnerDriver,
    {
        let write_timeout = self.state.policy().write_timeout;
        while let Some(effect) = effects.pop_front() {
            let terminal_transition = matches!(
                &effect,
                Effect::SessionChanged { to, .. } if *to != SessionState::Running
            );
            let applied = self.state.apply_effect(effect);
            if terminal_transition {
                let error = self
                    .state
                    .boundary_error()
                    .unwrap_or_else(AsyncOwnerHandle::missing_terminal_error);
                self.publish_terminal_error(error);
            }
            if let AppliedEffect::Transmit(staged) = applied {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => Self::write_frame(driver, write, runtime, write_timeout).await,
                    Err(error) => Err(error),
                };
                let produced = self.state.finish_write_in_turn(turn, &staged, write_result);
                prepend_effects(&mut effects, produced);
            }
        }
    }

    async fn terminate_at<D>(
        &mut self,
        driver: &mut D,
        runtime: &R,
        reason: ShutdownReason,
        observed_at: Instant,
    ) where
        D: AsyncOwnerDriver,
    {
        if self.state.state() == SessionState::Running {
            let effects = self.state.input(Input::Shutdown(reason), observed_at);
            self.drive(driver, effects, runtime).await;
        }
    }

    /// Answer a boundary which the raw-release coordinator had already removed
    /// from its channel when the session became terminal.  The ordinary queue
    /// drain cannot see this payload, so retaining its exact cancellation
    /// observation semantics here closes the same lifecycle edge.
    fn drain_deferred_raw_boundary(&mut self, deferred: DeferredRawBoundary, error: Error) {
        match deferred {
            DeferredRawBoundary::Admission(admission) => {
                let _ = admission.reply.try_send(Err(error));
            }
            DeferredRawBoundary::Cancellation(cancellation) => {
                let buffered = cancellation.receipt.completion.try_recv();
                let result = match buffered {
                    Some(observation) => Ok(cancellation_receipt_for(
                        cancellation.receipt,
                        Some(observation),
                    )),
                    None => Err(RejectedCancellation::kept(cancellation.receipt, error)),
                };
                let _ = cancellation.reply.try_send(result);
            }
        }
        self.state.fail_unstaged_boundary(1);
    }

    /// Answer every queued boundary message with the session's terminal error.
    ///
    /// The lanes are drained repeatedly until one whole pass finds all three
    /// empty, because a caller can enqueue on a lane that was already visited
    /// while a later one is still being drained (#626). The residual window
    /// between the last pass and the actor dropping its receivers is closed by
    /// the liveness lane, not here.
    fn drain_boundaries(&mut self, error: Error) {
        // A reporter may have coalesced behind the actor while it was handling
        // its terminal transition. Merge all compact facts the owner can still
        // deliver before answering/dropping ordinary boundary work.
        self.flush_pre_admission_rejections(true);
        let mut dropped = 0usize;
        loop {
            let before = dropped;
            while let Ok(admission) = self.admissions.try_recv() {
                let _ = admission.reply.try_send(Err(error.clone()));
                drop(admission);
                dropped = dropped.saturating_add(1);
            }
            while let Ok(cancel) = self.cancellations.try_recv() {
                let buffered = cancel.receipt.completion.try_recv();
                let result = match buffered {
                    Some(observation) => {
                        Ok(cancellation_receipt_for(cancel.receipt, Some(observation)))
                    }
                    // The session is ending, but the receipt still travels
                    // back: a refused cancellation never destroys the
                    // caller's observer (#612).
                    None => Err(RejectedCancellation::kept(cancel.receipt, error.clone())),
                };
                let _ = cancel.reply.try_send(result);
                dropped = dropped.saturating_add(1);
            }
            while let Ok(control) = self.control.try_recv() {
                match control {
                    ControlBoundary::FlushAdmissionRejections => {
                        self.flush_pre_admission_rejections(true);
                        continue;
                    }
                    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
                    ControlBoundary::Snapshot(reply) => {
                        let _ = reply.try_send(self.snapshot_now());
                    }
                    ControlBoundary::Metrics(reply) => {
                        let _ = reply.try_send(Err(error.clone()));
                    }
                    ControlBoundary::SubscribeDiagnostics { reply, .. } => {
                        let _ = reply.try_send(Err(error.clone()));
                    }
                    ControlBoundary::Reconfigure { reply, .. } => {
                        let _ = reply.try_send(Err(error.clone()));
                    }
                }
                dropped = dropped.saturating_add(1);
            }
            self.flush_pre_admission_rejections(true);
            if dropped == before {
                break;
            }
        }
        self.state.fail_unstaged_boundary(dropped);
    }

    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    fn snapshot_now(&self) -> OwnerSnapshot {
        OwnerSnapshot {
            #[cfg(feature = "runtime-tokio")]
            metrics: self.state.metrics(),
            #[cfg(feature = "runtime-tokio")]
            diagnostics: self.state.diagnostics().copied().collect(),
            state: self.state.state(),
            active: self.state.active_len(),
        }
    }
}

impl<R> Drop for AsyncOwnerActor<R>
where
    R: Executor,
{
    fn drop(&mut self) {
        // `run` can unwind before it reaches its normal terminal publication
        // and drain. Publish a fail-closed terminal result before draining so
        // boundary callers cannot retain an admission permit after an actor
        // panic, and so a concurrent shutdown observes rejection rather than
        // accepting a signal that no receiver can poll.
        let error = {
            let mut signal = self
                .shutdown_signal
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut terminal = self
                .terminal_error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let error = terminal
                .clone()
                .unwrap_or_else(AsyncOwnerHandle::missing_terminal_error);
            if terminal.is_none() {
                *terminal = Some(error.clone());
            }
            if !matches!(&error, Error::RuntimeShutdown)
                || !matches!(*signal, ShutdownSignalState::Accepted)
            {
                *signal = ShutdownSignalState::Failed(error.clone());
            }
            error
        };

        // The explicit normal-path drain may race with a sender that was
        // already admitted. A final drain closes that residual window before
        // the sender fields are released. Keep the liveness sender borrowed
        // through this drain so its disconnect remains the final teardown
        // barrier for waiters.
        let _alive_during_drain = &self.alive;
        self.drain_boundaries(error);
    }
}

/// Admission events retain the inline request owned by the actor channel; boxing
/// it here would add a heap allocation to every accepted admission.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum ActorEvent {
    Admission(Result<AdmissionBoundary, flume::RecvError>),
    Cancellation(CancellationBoundary),
    Control(ControlBoundary),
    Shutdown,
    Receive {
        result: Result<AsyncReceive, Error>,
        received_at: Instant,
    },
    Wake {
        raw_buffered_cap_exhausted: bool,
    },
}

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
mod tests {
    #![allow(
        clippy::await_holding_lock,
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used,
        unused_qualifications
    )]

    use std::sync::{atomic::Ordering, Mutex};

    use crate::runtime::Runtime;
    use crate::{
        runtime::engine::{
            CancellationPolicy, ControlPolicy, DecodedResponse, EncodedMessage, EnvelopeKind,
            InquiryRoute, ProtocolPolicy, ReplyShape, RequestContext, RetryPolicy, RuntimeRequest,
            TargetPolicy, TimeoutPolicy, TransportKind,
        },
        CameraId, ViscaSocket,
    };

    #[cfg(feature = "runtime-tokio")]
    use crate::runtime::engine::{EnvelopeSequence, SequenceWidth};

    #[cfg(feature = "runtime-tokio")]
    use crate::runtime::engine::CancellationObservation;
    #[cfg(feature = "runtime-tokio")]
    use crate::runtime::owner::{canonical_owner_trace, CANONICAL_OWNER_TRACE};
    #[cfg(feature = "runtime-smol")]
    use crate::runtime::SmolRuntime;
    #[cfg(feature = "runtime-tokio")]
    use crate::runtime::TokioRuntime;

    use super::*;

    #[cfg(feature = "runtime-tokio")]
    #[derive(Clone)]
    struct ManualRuntime {
        executor: TokioRuntime,
        now: Arc<Mutex<Instant>>,
        sleep_wakers: Arc<Mutex<Vec<std::task::Waker>>>,
        sleep_started: Option<flume::Sender<Duration>>,
        advance_after_next_now: Arc<Mutex<Option<Duration>>>,
        polling_sleeps: bool,
    }

    #[cfg(feature = "runtime-tokio")]
    impl ManualRuntime {
        fn new(now: Instant) -> Self {
            Self::with_sleep_behavior(now, false)
        }

        /// Makes a timer ready when this manual clock has advanced past it and
        /// its future is polled again. Tests that manually poll an owner use
        /// this to control an already-constructed timer race exactly.
        fn with_polling_sleeps(now: Instant) -> Self {
            Self::with_sleep_behavior(now, true)
        }

        /// Returns a manual clock plus a one-shot-friendly proof that an actor
        /// actually polled a sleep.  Raw-boundary tests use it to inject input
        /// *during* a clamped idle sleep, without trusting a wall-clock delay.
        fn with_polling_sleeps_and_sleep_barrier(
            now: Instant,
        ) -> (Self, flume::Receiver<Duration>) {
            let mut runtime = Self::with_polling_sleeps(now);
            let (sleep_started, observed) = flume::unbounded();
            runtime.sleep_started = Some(sleep_started);
            (runtime, observed)
        }

        fn with_sleep_behavior(now: Instant, polling_sleeps: bool) -> Self {
            Self {
                executor: TokioRuntime::from_current().unwrap(),
                now: Arc::new(Mutex::new(now)),
                sleep_wakers: Arc::new(Mutex::new(Vec::new())),
                sleep_started: None,
                advance_after_next_now: Arc::new(Mutex::new(None)),
                polling_sleeps,
            }
        }

        fn advance(&self, duration: Duration) {
            self.advance_silently(duration);
            // A manually advanced deadline must also wake an actor currently
            // parked inside its executor-neutral sleep future.  Tests use this
            // as a deterministic sleep-entered/read-consumed barrier rather
            // than a wall-clock delay.
            let wakers = std::mem::take(&mut *self.sleep_wakers.lock().unwrap());
            for waker in wakers {
                waker.wake();
            }
        }

        /// Advance the virtual clock without waking a parked timer.  A focused
        /// boundary race can then make a receive ready at H and use that
        /// receive's own waker to poll the already-due timer in the same
        /// selection, reproducing the real all-ready linearization exactly.
        fn advance_silently(&self, duration: Duration) {
            let mut now = self.now.lock().unwrap();
            *now = now.checked_add(duration).unwrap();
        }

        /// Make exactly the next clock sample return the current instant, then
        /// advance the virtual clock for the following sample.  This models a
        /// receive completing just before H while the executor resumes the
        /// actor's boundary handler just after H, without a scheduler sleep.
        fn advance_after_next_now(&self, duration: Duration) {
            *self.advance_after_next_now.lock().unwrap() = Some(duration);
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::executor::Executor for ManualRuntime {
        type Join<T>
            = <TokioRuntime as crate::executor::Executor>::Join<T>
        where
            T: Send + 'static;

        type Detach = <TokioRuntime as crate::executor::Executor>::Detach;

        fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            crate::executor::Executor::spawn_with_detach(&self.executor, future)
        }

        fn block_on<F: Future>(&self, future: F) -> F::Output {
            crate::executor::Executor::block_on(&self.executor, future)
        }

        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            let deadline = {
                let now = *self.now.lock().unwrap();
                now.checked_add(duration).unwrap_or(now)
            };
            let now = Arc::clone(&self.now);
            let sleep_wakers = Arc::clone(&self.sleep_wakers);
            let sleep_started = self.sleep_started.clone();
            let polling_sleeps = self.polling_sleeps;
            let mut announced = false;
            std::future::poll_fn(move |context| {
                if !announced {
                    if let Some(observed) = sleep_started.as_ref() {
                        let _ = observed.try_send(duration);
                    }
                    announced = true;
                }
                if polling_sleeps && *now.lock().unwrap() >= deadline {
                    std::task::Poll::Ready(())
                } else {
                    sleep_wakers.lock().unwrap().push(context.waker().clone());
                    std::task::Poll::Pending
                }
            })
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            future: F,
        ) -> impl Future<Output = Result<T, Error>> + Send + 'a
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            crate::executor::Executor::timeout(&self.executor, duration, future)
        }

        fn now(&self) -> Instant {
            let mut now = self.now.lock().unwrap();
            let observed = *now;
            if let Some(duration) = self.advance_after_next_now.lock().unwrap().take() {
                *now = now.checked_add(duration).unwrap();
            }
            observed
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl Runtime for ManualRuntime {
        type TcpTransport = <TokioRuntime as Runtime>::TcpTransport;
        type UdpTransport = <TokioRuntime as Runtime>::UdpTransport;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = std::convert::Infallible;

        async fn connect_tcp(
            &self,
            _addr: &str,
            _cfg: crate::transport::builder::TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            Err(Error::NotSupported)
        }

        async fn connect_udp(
            &self,
            _addr: &str,
            _cfg: crate::transport::builder::TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            Err(Error::NotSupported)
        }

        fn now(&self) -> Instant {
            crate::executor::Executor::now(self)
        }
    }

    type RecordedWrites = Arc<Mutex<Vec<(RequestId, Vec<u8>, bool)>>>;

    #[test]
    fn admission_rejection_ingress_total_saturates() {
        let ingress = AdmissionRejectionIngress::new(2);
        let rejection = PreAdmissionRejection {
            target: CameraId::CAMERA_1,
            lane: RequestLane::Command,
            error: crate::ErrorKind::BufferFull,
        };
        ingress.total.store(u64::MAX - 1, Ordering::Release);

        assert!(ingress.record(rejection));
        assert_eq!(ingress.total(), u64::MAX);
        assert!(!ingress.record(rejection));
        assert_eq!(ingress.total(), u64::MAX);
    }

    #[test]
    fn simultaneous_source_readiness_follows_the_explicit_phase() {
        let receive = || std::future::ready("receive");
        let boundary = || std::future::ready("boundary");

        assert_eq!(
            future::block_on(select_source(
                SourcePhase::ReceiveFirst,
                receive(),
                boundary(),
            )),
            "receive",
        );
        assert_eq!(
            future::block_on(select_source(
                SourcePhase::BoundariesFirst,
                receive(),
                boundary(),
            )),
            "boundary",
        );
        assert_eq!(
            TurnOutcome::Continue.next_source_phase(),
            Some(SourcePhase::ReceiveFirst),
        );
        assert_eq!(
            TurnOutcome::ContinueBuffered.next_source_phase(),
            Some(SourcePhase::ReceiveFirst),
        );
        assert_eq!(
            TurnOutcome::YieldBoundaries.next_source_phase(),
            Some(SourcePhase::BoundariesFirst),
        );
    }

    fn policy(capacity: usize) -> OwnerPolicy {
        OwnerPolicy::single_target(
            ProtocolPolicy {
                capacity,
                envelope: EnvelopeKind::Raw,
                transport: TransportKind::Datagram,
                inquiry_capacity: capacity,
                command_spacing: Duration::ZERO,
                inquiry_spacing: Duration::ZERO,
                inquiry_cooldown: Duration::ZERO,
                raw_inquiry_release_hold: Duration::from_secs(1),
                raw_release_grace: Duration::from_millis(100),
                strict_unconfirmed_poison: false,
            },
            CameraId::CAMERA_1,
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap()
    }

    // Sony's envelope sequence is the correlation key that permits more than
    // one command to wait for an ACK before any socket has been assigned.
    // Keep this separate from the raw policy used by the rest of the actor
    // fixtures so each test states which wire contract it exercises.
    #[cfg(feature = "runtime-tokio")]
    fn sony_policy(capacity: usize) -> OwnerPolicy {
        let mut owner = policy(capacity);
        owner.protocol.envelope = EnvelopeKind::Sony;
        owner
    }

    // Used only by the runtime-tokio stream tests below; dead on the runtime-smol leg (#636).
    #[allow(dead_code)]
    fn stream_policy(capacity: usize) -> OwnerPolicy {
        let mut owner = policy(capacity);
        owner.protocol.transport = TransportKind::Stream;
        owner
    }

    /// Two independently registered raw targets used to make an unrelated
    /// write park across target 1's exact ambiguity release.  The regression
    /// must exercise `drive()` itself, rather than a synthetic Wake, because
    /// only an awaited transmission completion recursively enters the engine
    /// between source selections.
    #[cfg(feature = "runtime-tokio")]
    fn two_target_raw_policy(transport: TransportKind) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity: 3,
            envelope: EnvelopeKind::Raw,
            transport,
            inquiry_capacity: 1,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::from_secs(1),
            raw_release_grace: Duration::from_millis(100),
            strict_unconfirmed_poison: false,
        };
        let target = TargetPolicy {
            command_sockets: 2,
            cancellation: CancellationPolicy::Supported,
        };
        let mut targets = [None; 9];
        targets[usize::from(CameraId::CAMERA_1.id())] = Some(target);
        targets[usize::from(CameraId::CAMERA_2.id())] = Some(target);
        OwnerPolicy::with_targets(protocol, targets).unwrap()
    }

    fn command() -> RuntimeRequest {
        command_for(CameraId::CAMERA_1)
    }

    fn command_for(target: CameraId) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(
                EncodedMessage::new(&[0x80_u8.saturating_add(target.id()), 0x01, 0x04, 0x00, 0xff])
                    .unwrap(),
            ),
            context: RequestContext {
                target,
                timeout: TimeoutPolicy {
                    ack: Duration::from_secs(5),
                    completion: Duration::from_secs(5),
                    inquiry: Duration::from_secs(5),
                    cancellation: Duration::from_secs(1),
                    ambiguity: Duration::from_secs(1),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            applied_state: None,
        }
    }

    /// A command whose deadlines are all short, so a liveness test that leaves it
    /// in flight settles quickly on the fix's success path (#675).
    fn command_with_short_deadlines() -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff]).unwrap()),
            context: RequestContext {
                target: CameraId::CAMERA_1,
                timeout: TimeoutPolicy {
                    ack: Duration::from_millis(100),
                    completion: Duration::from_millis(100),
                    inquiry: Duration::from_millis(100),
                    cancellation: Duration::from_millis(100),
                    ambiguity: Duration::from_millis(100),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            applied_state: None,
        }
    }

    /// A raw command with an individually chosen completion deadline and a
    /// wire marker visible in the production transport script.  The marker is
    /// deliberately carried in the otherwise inert fixture wire so assertions
    /// can prove a successor did not write before the retained frame settled.
    #[cfg(feature = "runtime-tokio")]
    fn raw_command_with_completion_deadline(marker: u8, completion: Duration) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, marker, 0xff]).unwrap()),
            context: RequestContext {
                target: CameraId::CAMERA_1,
                timeout: TimeoutPolicy {
                    ack: Duration::from_secs(5),
                    completion,
                    inquiry: Duration::from_secs(5),
                    cancellation: Duration::from_secs(1),
                    ambiguity: Duration::from_secs(1),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            applied_state: None,
        }
    }

    fn inquiry() -> RuntimeRequest {
        inquiry_for(CameraId::CAMERA_1)
    }

    /// An inquiry whose successful write immediately reaches its response
    /// deadline. Boundary tests use this to create the genuine late-reply
    /// hold that remains after timeout (#712).
    #[cfg(feature = "runtime-tokio")]
    fn timed_out_inquiry() -> RuntimeRequest {
        let mut request = inquiry();
        let RuntimeRequest::Inquiry { context, .. } = &mut request else {
            unreachable!("inquiry helper always constructs an inquiry");
        };
        context.timeout.inquiry = Duration::ZERO;
        request
    }

    fn inquiry_for(target: CameraId) -> RuntimeRequest {
        RuntimeRequest::Inquiry {
            wire: Arc::new(
                EncodedMessage::new(&[0x80_u8.saturating_add(target.id()), 0x09, 0x04, 0x00, 0xff])
                    .unwrap(),
            ),
            context: RequestContext {
                target,
                timeout: TimeoutPolicy {
                    ack: Duration::from_secs(5),
                    completion: Duration::from_secs(5),
                    inquiry: Duration::from_secs(5),
                    cancellation: Duration::from_secs(1),
                    ambiguity: Duration::from_secs(1),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            route: InquiryRoute::UNKNOWN,
        }
    }

    /// Wrap decoded frames as one nonzero-length read for the fake driver.
    fn batch(frames: Vec<DecodedFrame>) -> Result<AsyncReceive, Error> {
        Ok(AsyncReceive::Frames(frames))
    }

    fn ack(socket: ViscaSocket) -> DecodedFrame {
        DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Ack {
                socket: Some(socket),
            },
        }
    }

    fn completion(socket: ViscaSocket) -> DecodedFrame {
        DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Completion {
                socket: Some(socket),
            },
        }
    }

    #[cfg(feature = "runtime-tokio")]
    fn sequenced(sequence: u32, response: DecodedResponse) -> DecodedFrame {
        DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: Some(EnvelopeSequence {
                value: sequence,
                width: SequenceWidth::Full32,
            }),
            response,
        }
    }

    #[derive(Debug)]
    struct FakeAsyncDriver {
        writes: RecordedWrites,
        started: flume::Sender<RequestId>,
        gates: flume::Receiver<Result<TransmissionMeta, Error>>,
        frames: flume::Receiver<Result<AsyncReceive, Error>>,
    }

    impl AsyncOwnerDriver for FakeAsyncDriver {
        fn write(
            &mut self,
            write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            let id = write.request;
            self.writes.lock().unwrap().push((
                write.request,
                write.bytes.to_vec(),
                write.cancellation,
            ));
            let started = self.started.clone();
            let gates = self.gates.clone();
            async move {
                let _ = started.send_async(id).await;
                gates
                    .recv_async()
                    .await
                    .unwrap_or(Err(Error::RuntimeShutdown))
            }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let frames = self.frames.clone();
            async move {
                frames
                    .recv_async()
                    .await
                    .unwrap_or(Err(Error::RuntimeShutdown))
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct PanickingReceiveDriver;

    /// A driver that reports the Tokio runtime executing the real owner task
    /// before ending the session.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct RuntimeAffinityDriver {
        observed: flume::Sender<tokio::runtime::Id>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for RuntimeAffinityDriver {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let observed = self.observed.clone();
            async move {
                let _ = observed.try_send(tokio::runtime::Handle::current().id());
                Ok(AsyncReceive::Closed)
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for PanickingReceiveDriver {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        #[allow(clippy::manual_async_fn)]
        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            async {
                panic!("test driver panic before terminal publication");
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct PanickingAfterAdmissionDriver {
        panic_signal: flume::Receiver<()>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for PanickingAfterAdmissionDriver {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        #[allow(clippy::manual_async_fn)]
        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let panic_signal = self.panic_signal.clone();
            async move {
                panic_signal
                    .recv_async()
                    .await
                    .expect("the panic signal must remain connected");
                panic!("test driver panic after admission");
            }
        }
    }

    struct Harness {
        driver: FakeAsyncDriver,
        started: flume::Receiver<RequestId>,
        gates: flume::Sender<Result<TransmissionMeta, Error>>,
        frames: flume::Sender<Result<AsyncReceive, Error>>,
        writes: RecordedWrites,
    }

    fn harness() -> Harness {
        let (started_tx, started) = flume::bounded(8);
        let (gate_tx, gates) = flume::bounded(8);
        let (frame_tx, frames) = flume::bounded(8);
        let writes = Arc::new(Mutex::new(Vec::new()));
        Harness {
            driver: FakeAsyncDriver {
                writes: Arc::clone(&writes),
                started: started_tx,
                gates,
                frames,
            },
            started,
            gates: gate_tx,
            frames: frame_tx,
            writes,
        }
    }

    fn prepared_focus(profile: &crate::ProfileSpec) -> crate::prepared::PreparedCommand {
        crate::prepared::prepare_command(
            &crate::request::builtin::FocusModeCommand::Manual,
            CameraId::CAMERA_1,
            profile,
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .unwrap()
    }

    fn prepared_zoom(
        profile: &crate::ProfileSpec,
    ) -> crate::prepared::PreparedOperation<completion::Targeted> {
        crate::prepared::prepare_operation::<completion::Targeted, _>(
            &crate::request::builtin::ZoomTarget::new(
                crate::types::ZoomPosition::new(0x0100).unwrap(),
            ),
            CameraId::CAMERA_1,
            profile,
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .unwrap()
    }

    async fn typed_async_receipt_matrix<R>(runtime: R)
    where
        R: Runtime,
    {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime.clone()).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let writes = Arc::clone(&harness.writes);
        let client = async {
            let success = handle
                .submit_command(prepared_focus(&profile))
                .await
                .unwrap();
            let _ = started.recv_async().await.unwrap();
            gates
                .send_async(Ok(TransmissionMeta { sequence: None }))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![ack(ViscaSocket::S1)]))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![completion(ViscaSocket::S1)]))
                .await
                .unwrap();
            assert_eq!(handle.snapshot().await.unwrap().active, 0);
            success
                .wait_with_timeout(handle.receipt_control(), Duration::ZERO)
                .await
                .unwrap();

            let failed = handle
                .submit_command(prepared_focus(&profile))
                .await
                .unwrap();
            let _ = started.recv_async().await.unwrap();
            gates
                .send_async(Ok(TransmissionMeta { sequence: None }))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![ack(ViscaSocket::S1)]))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![DecodedFrame {
                    target: CameraId::CAMERA_1,
                    sequence: None,
                    response: DecodedResponse::Error {
                        socket: Some(ViscaSocket::S1),
                        code: 0x02,
                    },
                }]))
                .await
                .unwrap();
            assert_eq!(handle.snapshot().await.unwrap().active, 0);
            assert!(matches!(
                failed.wait(handle.receipt_control()).await,
                Err(Error::SyntaxError)
            ));

            let operation = handle
                .submit_operation(prepared_zoom(&profile))
                .await
                .unwrap();
            let _ = started.recv_async().await.unwrap();
            gates
                .send_async(Ok(TransmissionMeta { sequence: None }))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![ack(ViscaSocket::S1)]))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![completion(ViscaSocket::S1)]))
                .await
                .unwrap();
            assert_eq!(handle.snapshot().await.unwrap().active, 0);
            let cancellation = operation.cancel().await.unwrap();
            assert_eq!(
                cancellation
                    .outcome(handle.receipt_control(), Duration::ZERO)
                    .await
                    .unwrap(),
                CancellationOutcome::Completed
            );

            let detached = handle
                .submit_command(prepared_focus(&profile))
                .await
                .unwrap();
            let _ = started.recv_async().await.unwrap();
            gates
                .send_async(Ok(TransmissionMeta { sequence: None }))
                .await
                .unwrap();
            assert!(matches!(
                detached
                    .wait_with_timeout(handle.receipt_control(), Duration::ZERO)
                    .await,
                Err(Error::Timeout)
            ));
            assert_eq!(
                writes
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(_, _, cancellation)| *cancellation)
                    .count(),
                0,
                "observer timeout never sends cancellation"
            );
            frames
                .send_async(batch(vec![ack(ViscaSocket::S1)]))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![completion(ViscaSocket::S1)]))
                .await
                .unwrap();
            assert_eq!(handle.snapshot().await.unwrap().active, 0);

            handle.shutdown().await.unwrap();
        };
        let ((), snapshot) = future::zip(client, actor.run(harness.driver)).await;
        assert_eq!(snapshot.active, 0);
    }

    async fn targeted_settlement_matrix<R>(runtime: R)
    where
        R: Runtime,
    {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(3), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let writes = Arc::clone(&harness.writes);
        let client = async {
            let operation = handle
                .submit_operation(prepared_zoom(&profile))
                .await
                .unwrap();
            let _ = started.recv_async().await.unwrap();
            gates
                .send_async(Ok(TransmissionMeta { sequence: None }))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![ack(ViscaSocket::S1)]))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![completion(ViscaSocket::S1)]))
                .await
                .unwrap();
            assert_eq!(handle.snapshot().await.unwrap().active, 0);

            let settlement = operation
                .settled_with_timeout(handle.receipt_control(), Duration::from_secs(1))
                .erase()
                .wait();
            let replies = async {
                for _ in 0..2 {
                    let _ = started.recv_async().await.unwrap();
                    gates
                        .send_async(Ok(TransmissionMeta { sequence: None }))
                        .await
                        .unwrap();
                    frames
                        .send_async(batch(vec![DecodedFrame {
                            target: CameraId::CAMERA_1,
                            sequence: None,
                            response: DecodedResponse::InquiryReply {
                                route: None,
                                payload: smallvec::smallvec![0x0, 0x1, 0x0, 0x0],
                            },
                        }]))
                        .await
                        .unwrap();
                }
            };
            let (settled, ()) = future::zip(settlement, replies).await;
            assert!(settled.is_ok());
            assert_eq!(handle.snapshot().await.unwrap().active, 0);
            let writes = writes.lock().unwrap();
            assert_eq!(writes.len(), 3);
            assert_eq!(writes[1].1, writes[2].1);
            assert_eq!(writes[1].1, vec![0x81, 0x09, 0x04, 0x47, 0xff]);
            drop(writes);
            handle.shutdown().await.unwrap();
        };
        let ((), snapshot) = future::zip(client, actor.run(harness.driver)).await;
        assert_eq!(snapshot.active, 0);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_typed_receipts_are_completion_first_and_timeout_only_detaches() {
        typed_async_receipt_matrix(TokioRuntime::from_current().unwrap()).await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_targeted_settlement_uses_ordinary_prepared_inquiries() {
        targeted_settlement_matrix(TokioRuntime::from_current().unwrap()).await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn async_manual_clock_rejects_settlement_query_at_deadline() {
        let now = Instant::now();
        let runtime = ManualRuntime::new(now);
        let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime.clone()).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let writes = Arc::clone(&harness.writes);
        let actor_task = tokio::spawn(actor.run(harness.driver));
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();

        let operation = handle
            .submit_operation(prepared_zoom(&profile))
            .await
            .unwrap();
        assert_eq!(started.recv_async().await.unwrap(), operation.core.id());
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 0);
        assert_eq!(Runtime::now(&runtime), now);

        let error = operation
            .settled_with_timeout(handle.receipt_control(), Duration::ZERO)
            .erase()
            .wait()
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Timeout));
        assert_eq!(
            writes.lock().unwrap().len(),
            1,
            "no position inquiry is written at the exact owner deadline"
        );

        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.metrics.admitted, 1);
        assert_eq!(snapshot.active, 0);
    }

    /// A caller deadline before actor admission is a rejected boundary, not an
    /// observer timeout. In particular, starting the actor after the caller
    /// timed out must neither create engine state nor transmit the stale work,
    /// and dropping that boundary must return its shared capacity permit.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(start_paused = true)]
    async fn expired_pre_admission_boundary_never_writes_and_releases_capacity() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let deadline = handle.deadline_after(Duration::from_millis(1)).unwrap();
        let expiring_handle = handle.clone();
        let expiring = tokio::spawn(async move {
            expiring_handle
                .submit_with_timeout_until(inquiry(), Duration::from_secs(5), deadline)
                .await
        });

        tokio::task::yield_now().await;
        assert_eq!(
            handle.permits.available(),
            0,
            "the queued boundary owns capacity until the actor observes its expiry"
        );
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(matches!(expiring.await.unwrap(), Err(Error::Timeout)));

        // Only now let the actor consume the expired boundary. A pre-fix actor
        // staged it and wrote it after this point because the caller had merely
        // dropped its reply receiver.
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let writes = Arc::clone(&harness.writes);
        let actor_task = tokio::spawn(actor.run(harness.driver));
        let snapshot = handle.snapshot().await.unwrap();
        assert_eq!(snapshot.metrics.admitted, 0);
        assert_eq!(snapshot.metrics.admission_rejected, 1);
        assert_eq!(snapshot.metrics.writes, 0);
        assert_eq!(snapshot.active, 0);
        assert_eq!(
            snapshot.diagnostics,
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: super::super::RequestLane::Inquiry,
                error: crate::ErrorKind::Timeout,
            }],
            "the caller-expiry winner records exactly one pre-admission rejection"
        );
        assert!(
            writes.lock().unwrap().is_empty(),
            "expired work was not written"
        );
        assert_eq!(
            handle.permits.available(),
            handle.permits.capacity(),
            "dropping the stale boundary returns its capacity permit"
        );

        // Reusing the only slot proves that no invisible pending admission is
        // retaining capacity after the caller saw `Timeout`.
        let receipt = handle.try_submit(inquiry()).unwrap().await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        assert_eq!(writes.lock().unwrap().len(), 1);
        drop(receipt);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        handle.shutdown().await.unwrap();
        let terminal = actor_task.await.unwrap();
        assert_eq!(terminal.state, SessionState::Shutdown);
    }

    /// A deadline already reached before a boundary exists is still a rejected
    /// submission. It must use the same bounded handle-side telemetry path as
    /// a capacity rejection without allocating admission state.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn immediate_pre_admission_deadline_is_telemetrized() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let error = handle
            .submit_with_timeout_until(inquiry(), Duration::from_secs(1), handle.now())
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Timeout));

        actor.flush_pre_admission_rejections(true);
        let metrics = actor.state.metrics_snapshot();
        assert_eq!(metrics.admission_rejected, 1);
        assert_eq!(metrics.admitted, 0);
        assert_eq!(metrics.active, 0);
        assert_eq!(metrics.pending, 0);
        assert_eq!(metrics.writes, 0);
        assert_eq!(
            actor.state.diagnostics().copied().collect::<Vec<_>>(),
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: super::super::RequestLane::Inquiry,
                error: crate::ErrorKind::Timeout,
            }]
        );
    }

    /// If the actor sees an expired boundary before its caller polls the
    /// deadline, it is the one authoritative telemetry writer. The caller
    /// observes the reply and must not create a second rejection event.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(start_paused = true)]
    async fn actor_expired_pre_admission_boundary_is_telemetrized_once() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
        let deadline = handle.deadline_after(Duration::from_millis(1)).unwrap();
        let validity = AdmissionValidity::until(deadline);
        let (completion, reply) = handle
            .enqueue_admission(inquiry(), Some(validity))
            .expect("a future deadline can enter the bounded admission lane");
        drop(completion);
        tokio::time::advance(Duration::from_millis(1)).await;

        let boundary = actor
            .admissions
            .try_recv()
            .expect("the actor owns the queued boundary");
        let harness = harness();
        let writes = Arc::clone(&harness.writes);
        let mut driver = harness.driver;
        actor
            .handle_admission(boundary, &mut driver, &runtime, Executor::now(&runtime))
            .await;

        assert!(matches!(reply.recv_async().await, Ok(Err(Error::Timeout))));
        let metrics = actor.state.metrics_snapshot();
        assert_eq!(metrics.admission_rejected, 1);
        assert_eq!(metrics.admitted, 0);
        assert_eq!(metrics.active, 0);
        assert_eq!(metrics.pending, 0);
        assert_eq!(metrics.writes, 0);
        assert!(writes.lock().unwrap().is_empty());
        assert_eq!(
            actor.state.diagnostics().copied().collect::<Vec<_>>(),
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: super::super::RequestLane::Inquiry,
                error: crate::ErrorKind::Timeout,
            }]
        );
    }

    /// The owner task follows `TokioRuntime::from_handle`, even when both the
    /// runtime value and actor future are constructed and awaited on a distinct
    /// Tokio runtime. This is the affinity that keeps a selected transport,
    /// actor timers and I/O together.
    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn tokio_from_handle_runs_the_owner_on_the_selected_runtime() {
        let selected = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let selected_handle = selected.handle().clone();
        let ambient = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        ambient.block_on(async move {
            let runtime = TokioRuntime::from_handle(selected_handle.clone());
            let (observed, receiver) = flume::bounded(1);
            let (_owner, actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
            let actor_task = crate::executor::Executor::spawn(
                &runtime,
                actor.run(RuntimeAffinityDriver { observed }),
            );

            assert_eq!(receiver.recv_async().await.unwrap(), selected_handle.id());
            assert_eq!(actor_task.await.unwrap().state, SessionState::Closed);
        });
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_typed_receipts_are_completion_first_and_timeout_only_detaches() {
        smol::block_on(typed_async_receipt_matrix(SmolRuntime::new()));
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_targeted_settlement_uses_ordinary_prepared_inquiries() {
        smol::block_on(targeted_settlement_matrix(SmolRuntime::new()));
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn async_admission_precedes_write_and_matches_blocking_terminal_trace() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let writes = Arc::clone(&harness.writes);
        let actor_task = tokio::spawn(actor.run(harness.driver));

        // The actor is intentionally stalled in the first write. Admission must
        // still resolve because its effect precedes Transmit in source order.
        let receipt = tokio::time::timeout(Duration::from_secs(1), handle.submit(command()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        assert_eq!(writes.lock().unwrap().len(), 1);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));
        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert_eq!(snapshot.active, 0);
        assert_eq!(
            canonical_owner_trace(snapshot.diagnostics.iter().copied()),
            CANONICAL_OWNER_TRACE
        );
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn async_receive_batch_precedes_a_completion_deadline_at_equality() {
        let runtime = ManualRuntime::new(Instant::now());
        let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime.clone()).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let first = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), first.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 1);
        let completion_deadline = Executor::now(&runtime)
            .checked_add(Duration::from_secs(5))
            .expect("the fixed completion deadline must be representable");

        let second = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), second.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 2);

        // Deliver ACK S2 and completion S1 at S1's exact completion deadline.
        // The ordered receive batch must settle S1 before due work runs; a
        // strictly late correlated completion is rejected by the engine.
        runtime.advance(Duration::from_secs(5));
        assert_eq!(Executor::now(&runtime), completion_deadline);
        frames
            .send_async(batch(vec![
                ack(ViscaSocket::S2),
                completion(ViscaSocket::S1),
            ]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 1);
        let first_outcome = first.terminal().await.unwrap();
        assert!(
            matches!(first_outcome, RuntimeOutcome::Applied),
            "completion from the receive batch must win: {first_outcome:?}"
        );

        drop(second);
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A receive sampled even one nanosecond after its correlated completion
    /// deadline is ordinary late input: the frame is ignored, the due
    /// transition quarantines the raw socket, and quarantine expiry reports
    /// the command's unconfirmed outcome. This keeps the actor-level overdue
    /// half of the strict boundary contract alongside the equality test above
    /// (#731).
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn async_receive_batch_rejects_an_overdue_completion() {
        let runtime = ManualRuntime::new(Instant::now());
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let receipt = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 1);

        runtime.advance(Duration::from_secs(5) + Duration::from_nanos(1));
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        let snapshot = handle.snapshot().await.unwrap();
        assert_eq!(snapshot.active, 1, "the raw socket remains quarantined");
        assert!(snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
        )));
        runtime.advance(Duration::from_secs(1));
        // A control may consume the actor's one due-boundary allowance. The
        // second round trip proves that the ambiguity deadline was serviced.
        let _ = handle.snapshot().await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        let outcome = terminal_within_test_deadline(
            &receipt,
            "the overdue completion leaves the command to its ambiguity outcome",
        )
        .await;
        assert!(matches!(
            outcome,
            RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed)
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A due engine wake may allow one ordinary control observation, but a
    /// chained backlog of real public controls cannot keep it from advancing
    /// engine time. The first metrics call intentionally observes the pending
    /// inquiry; the second must observe its reply deadline having fired.
    ///
    /// Before the tail fairness bound, every metrics call won
    /// `future::or(control, wake)`, so all four observed `active == 1` and an
    /// unbounded caller chain could postpone the deadline forever.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(start_paused = true)]
    async fn due_wake_is_not_starved_by_chained_public_controls() {
        const CONTROL_CHAIN: usize = 4;

        let runtime = TokioRuntime::from_current().unwrap();
        let mut owner_policy = policy(1);
        // The actor's bounded control lane is intentionally full before it
        // starts, making this a deterministic chain rather than a scheduler
        // race between the caller and the ready timer.
        owner_policy.limits.applied_subscribers = CONTROL_CHAIN;
        let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
        let (_frames, receives) = flume::bounded(1);
        let mut driver = UngatedDriver { receives };

        // Install one sent inquiry without running the event loop yet. Its
        // reply deadline is therefore the next authoritative engine wake.
        let (completion, admitted) = handle.enqueue_admission(inquiry(), None).unwrap();
        let admission = actor
            .admissions
            .try_recv()
            .expect("the staged inquiry must be waiting for the actor");
        actor
            .handle_admission(admission, &mut driver, &runtime, Executor::now(&runtime))
            .await;
        assert!(admitted.recv_async().await.unwrap().is_ok());
        assert_eq!(actor.state.active_len(), 1);
        let deadline = actor
            .state
            .next_wake()
            .expect("the sent inquiry must own a reply deadline");
        assert_eq!(
            deadline.saturating_duration_since(Executor::now(&runtime)),
            Duration::from_secs(5)
        );

        // Send each request through the public control API and wait until it
        // has joined the actor's FIFO lane before starting its successor. No
        // actor is polling yet, so the resulting sequence is deterministic.
        let mut controls = Vec::with_capacity(CONTROL_CHAIN);
        for expected_queued in 1..=CONTROL_CHAIN {
            let control_handle = handle.clone();
            controls.push(tokio::spawn(async move { control_handle.metrics().await }));
            while handle.control.len() < expected_queued {
                tokio::task::yield_now().await;
            }
        }
        assert!(handle.control.is_full());

        // Make the protocol deadline due before selection begins. Tokio's
        // paused clock keeps this exact and avoids a wall-clock liveness race.
        tokio::time::advance(Duration::from_secs(5)).await;
        assert_eq!(
            deadline.saturating_duration_since(Executor::now(&runtime)),
            Duration::ZERO,
            "the chained controls race an already-due protocol deadline"
        );
        let actor_task = tokio::spawn(actor.run(driver));

        let first = controls.remove(0).await.unwrap().unwrap();
        let second = controls.remove(0).await.unwrap().unwrap();
        assert_eq!(
            first.active, 1,
            "one control may observe the pre-wake state"
        );
        assert_eq!(
            second.active, 0,
            "the due Wake must advance the engine before a second queued control",
        );
        assert!(matches!(
            completion.recv_async().await.unwrap(),
            ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
        ));

        for control in controls {
            control.await.unwrap().unwrap();
        }
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A timer can mature after the actor has constructed its tail selection.
    /// The first control then wins the old left-biased race, but it must still
    /// spend the due wake's one allowance before the loop is rebuilt.
    ///
    /// This manually polls the same actor future before and after advancing a
    /// test clock, so no Tokio task scheduling order can accidentally let Wake
    /// run before the two controls are queued.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(start_paused = true)]
    async fn parked_future_wake_charges_the_first_ready_control() {
        let runtime = ManualRuntime::with_polling_sleeps(Instant::now());
        let mut owner_policy = policy(1);
        owner_policy.limits.applied_subscribers = 2;
        // Keep the receive timeout after the inquiry deadline so the parked
        // receive arm cannot become the source that wakes this test.
        owner_policy.read_timeout = Duration::from_secs(10);
        let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
        let (_frames, receives) = flume::bounded(1);
        let mut driver = UngatedDriver { receives };

        let (completion, admitted) = handle.enqueue_admission(inquiry(), None).unwrap();
        let admission = actor
            .admissions
            .try_recv()
            .expect("the staged inquiry must be waiting for the actor");
        actor
            .handle_admission(admission, &mut driver, &runtime, Executor::now(&runtime))
            .await;
        assert!(admitted.recv_async().await.unwrap().is_ok());
        let deadline = actor
            .state
            .next_wake()
            .expect("the sent inquiry must own a reply deadline");

        let mut run = Box::pin(actor.run(driver));
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        assert!(std::future::Future::poll(run.as_mut(), &mut context).is_pending());

        // The original tail was built with a positive delay and is now parked.
        // Advance the logical clock without polling the actor, then enqueue two
        // controls before its next poll. Both the timer and first control are
        // ready when that already-built left-biased tail resumes.
        runtime.advance(Duration::from_secs(5));
        assert_eq!(
            deadline.saturating_duration_since(Executor::now(&runtime)),
            Duration::ZERO,
        );
        let (first_reply, first_result) = flume::bounded(1);
        let (second_reply, second_result) = flume::bounded(1);
        handle
            .control
            .try_send(ControlBoundary::Metrics(first_reply))
            .unwrap();
        handle
            .control
            .try_send(ControlBoundary::Metrics(second_reply))
            .unwrap();

        assert!(std::future::Future::poll(run.as_mut(), &mut context).is_pending());
        let first = first_result.try_recv().unwrap().unwrap();
        let second = second_result.try_recv().unwrap().unwrap();
        assert_eq!(first.active, 1, "the first control owns the allowance");
        assert_eq!(
            second.active, 0,
            "the matured wake must run before a second queued control",
        );
        assert!(matches!(
            completion.recv_async().await.unwrap(),
            ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
        ));
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn capacity_rejection_is_pre_identity_and_cancel_full_waits_without_drop() {
        let runtime = TokioRuntime::from_current().unwrap();
        // Raw VISCA admits only one unacknowledged command per target because
        // the camera has not supplied a socket to correlate a second ACK. The
        // capacity/cancellation assertion is about the bounded owner lanes,
        // so use Sony's explicit sequence key for genuine pre-ACK pipelining.
        let (handle, actor) = AsyncOwnerActor::new(sony_policy(2), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let first = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), first.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
            .await
            .unwrap();
        let second = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), second.id);
        let error = handle.try_submit(command()).err().unwrap();
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 2 }));

        let cancel_handle = handle.clone();
        let first_cancel = tokio::spawn(async move { cancel_handle.cancel_test(first).await });
        tokio::task::yield_now().await;
        let cancel_handle = handle.clone();
        let mut second_cancel =
            tokio::spawn(async move { cancel_handle.cancel_test(second).await });
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut second_cancel)
                .await
                .is_err(),
            "the full dedicated lane applies backpressure"
        );

        gates
            .send_async(Ok(TransmissionMeta { sequence: Some(2) }))
            .await
            .unwrap();
        let first_observation = first_cancel.await.unwrap().unwrap();
        let second_observation = second_cancel.await.unwrap().unwrap();
        drop((first_observation, second_observation));

        let (left, right) = tokio::join!(handle.shutdown(), handle.shutdown());
        left.unwrap();
        right.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.active, 0);
        assert_eq!(snapshot.metrics.admitted, 2);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn ordinary_async_submit_is_fail_fast_at_capacity() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let first = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), first.id);
        let error = tokio::time::timeout(Duration::from_millis(20), handle.submit(command()))
            .await
            .expect("capacity failure must not register a waiter")
            .unwrap_err();
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));
        drop(first);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.metrics.admitted, 1);
    }

    /// A handle-side capacity rejection happens before an admission boundary
    /// can allocate an observer, request ID, or pending slot. It still belongs
    /// to the stable pre-admission telemetry contract, so the actor receives a
    /// compact bounded ingress event without turning the fail-fast call into a
    /// wait for the owner.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn fail_fast_capacity_rejection_records_metrics_and_diagnostic() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let writes = Arc::clone(&harness.writes);
        let actor_task = tokio::spawn(actor.run(harness.driver));
        let diagnostics = handle.subscribe_diagnostics(16).await.unwrap();

        let first = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), first.id);
        assert_eq!(writes.lock().unwrap().len(), 1);

        let error = tokio::time::timeout(Duration::from_millis(20), handle.submit(command()))
            .await
            .expect("capacity failure must not register an admission waiter")
            .unwrap_err();
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));

        // The first write is deliberately held so the rejected work cannot be
        // confused with a newly admitted request. Release it only after the
        // fail-fast result, then query the actor-owned metric snapshot.
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        let metrics = handle.metrics().await.unwrap();
        assert_eq!(metrics.admitted, 1);
        assert_eq!(metrics.admission_rejected, 1);
        assert_eq!(metrics.active, 1);
        assert_eq!(metrics.pending, 0);
        assert_eq!(metrics.writes, 1);
        assert_eq!(
            writes.lock().unwrap().len(),
            1,
            "a rejected admission must not reach transport I/O"
        );

        let rejected = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let event = diagnostics.recv_async().await.unwrap();
                if matches!(event, DiagnosticEvent::AdmissionRejected { .. }) {
                    break event;
                }
            }
        })
        .await
        .expect("the bounded ingress must emit one rejection diagnostic");
        assert_eq!(
            rejected,
            DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: super::super::RequestLane::Command,
                error: crate::ErrorKind::BufferFull,
            }
        );

        drop(first);
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// The handle-side ingress is runtime-neutral. Exercise the same
    /// fail-fast capacity path under smol without requiring a transport task:
    /// the actor remains the sole writer when it drains the compact event.
    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_fail_fast_capacity_rejection_records_telemetry() {
        let (handle, mut actor) = AsyncOwnerActor::new(policy(1), SmolRuntime::new()).unwrap();
        let held = handle
            .permits
            .try_acquire()
            .expect("the test reserves the only admission slot");

        let error = match handle.enqueue_admission(inquiry(), None) {
            Ok(_) => panic!("a full admission pool must fail fast"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));

        actor.flush_pre_admission_rejections(true);
        let metrics = actor.state.metrics_snapshot();
        assert_eq!(metrics.admission_rejected, 1);
        assert_eq!(metrics.admitted, 0);
        assert_eq!(metrics.active, 0);
        assert_eq!(metrics.pending, 0);
        assert_eq!(metrics.writes, 0);
        assert_eq!(
            actor.state.diagnostics().copied().collect::<Vec<_>>(),
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: super::super::RequestLane::Inquiry,
                error: crate::ErrorKind::BufferFull,
            }]
        );
        drop(held);
    }

    /// The ingress is deliberately bounded. If a burst outruns its one-slot
    /// diagnostic staging queue, metrics keep the exact rejection total and
    /// the already-public dropped-diagnostics counter makes the evicted event
    /// visible instead of silently inventing another loss class.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn pre_admission_rejection_ingress_reports_bounded_diagnostic_loss() {
        let runtime = TokioRuntime::from_current().unwrap();
        let mut owner_policy = policy(1);
        owner_policy.limits.diagnostics = 1;
        let (_handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime).unwrap();
        let first = PreAdmissionRejection {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Command,
            error: crate::ErrorKind::BufferFull,
        };
        let second = PreAdmissionRejection {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Inquiry,
            error: crate::ErrorKind::IoClosed,
        };

        assert!(actor.admission_rejections.record(first));
        assert!(
            !actor.admission_rejections.record(second),
            "one bounded actor wake coalesces the burst"
        );
        actor.flush_pre_admission_rejections(true);

        let metrics = actor.state.metrics();
        assert_eq!(metrics.admission_rejected, 2);
        assert_eq!(metrics.dropped_diagnostics, 1);
        assert_eq!(
            actor.state.diagnostics().copied().collect::<Vec<_>>(),
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: super::super::RequestLane::Inquiry,
                error: crate::ErrorKind::IoClosed,
            }],
            "the bounded queue retains the newest rejection fact"
        );
    }

    /// A receiver can disappear in the narrow interval after the handle passes
    /// its lifecycle check but before its boundary send. That is still a
    /// rejection before authoritative admission, so it must use the same
    /// metric/diagnostic ingress as fail-fast capacity exhaustion.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn disconnected_pre_boundary_admission_is_telemetrized() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let (replacement_sender, replacement_receiver) = flume::bounded(1);
        drop(replacement_sender);
        let original_receiver = std::mem::replace(&mut actor.admissions, replacement_receiver);
        drop(original_receiver);

        let error = match handle.try_submit(command()) {
            Ok(_) => panic!("a disconnected admission receiver must reject"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            Error::InvalidState(message)
                if message.contains("without publishing a terminal result")
        ));

        actor.flush_pre_admission_rejections(true);
        assert_eq!(actor.state.metrics().admission_rejected, 1);
        assert_eq!(
            actor.state.diagnostics().copied().collect::<Vec<_>>(),
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: super::super::RequestLane::Command,
                error: crate::ErrorKind::NotExecutable,
            }]
        );
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn ready_transport_close_precedes_shutdown_and_queued_admission() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let frames = harness.frames.clone();
        let admission = handle.try_submit(command()).unwrap();
        handle.shutdown().await.unwrap();
        frames.send_async(Ok(AsyncReceive::Closed)).await.unwrap();
        let snapshot = actor.run(harness.driver).await;
        assert_eq!(snapshot.state, SessionState::Closed);
        assert!(matches!(
            admission.await.unwrap_err(),
            Error::ConnectionClosed { .. }
        ));
        let frame_or_close = snapshot
            .diagnostics
            .iter()
            .position(|event| matches!(event, DiagnosticEvent::SessionChanged { .. }))
            .unwrap();
        assert_eq!(frame_or_close, 0, "close is the first applied ready source");
    }

    /// The engine's terminal verdict is published while `handle_event` drives
    /// its `SessionChanged` effect, not only in `run`'s epilogue. Keeping this
    /// seam direct makes the race deterministic: the actor has returned `Stop`
    /// but still owns a live shutdown receiver, exactly where a pre-fix caller
    /// could enqueue and incorrectly receive `Ok(())`.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn terminal_stop_is_published_before_shutdown_can_enter_the_live_lane() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
        let mut driver = harness().driver;

        let outcome = actor
            .handle_event(
                ActorEvent::Receive {
                    result: Ok(AsyncReceive::Closed),
                    received_at: Executor::now(&runtime),
                },
                &mut driver,
                &runtime,
                Executor::now(&runtime),
                false,
            )
            .await;
        assert_eq!(outcome, TurnOutcome::Stop);

        let error = handle.shutdown().await.unwrap_err();
        assert!(matches!(error, Error::ConnectionClosed { .. }));
        assert!(
            actor.shutdown.is_empty(),
            "a terminal session must not retain a shutdown signal it can never poll"
        );
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn ready_frame_is_observed_before_explicit_shutdown() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let frames = harness.frames.clone();
        frames
            .send_async(batch(vec![DecodedFrame {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: DecodedResponse::Unknown,
            }]))
            .await
            .unwrap();
        handle.shutdown().await.unwrap();
        let snapshot = actor.run(harness.driver).await;
        let frame = snapshot
            .diagnostics
            .iter()
            .position(|event| matches!(event, DiagnosticEvent::FrameReceived { .. }))
            .unwrap();
        let shutdown = snapshot
            .diagnostics
            .iter()
            .position(|event| matches!(event, DiagnosticEvent::SessionChanged { .. }))
            .unwrap();
        assert!(frame < shutdown);
        assert_eq!(snapshot.state, SessionState::Shutdown);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn async_cancellation_returns_after_recording_and_retains_terminal() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));
        let operation = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), operation.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        let cancel_handle = handle.clone();
        let cancel_task = tokio::spawn(async move { cancel_handle.cancel_test(operation).await });
        let _cancel_write = started.recv_async().await.unwrap();
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        let cancellation = cancel_task.await.unwrap().unwrap();
        frames
            .send_async(batch(vec![DecodedFrame {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x04,
                },
            }]))
            .await
            .unwrap();
        assert!(matches!(
            cancellation.recv_test().await.unwrap(),
            CancellationObservation::Cancelled
        ));
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().active, 0);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn shutdown_drain_reuses_buffered_operation_receiver_for_queued_cancel() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let writes = Arc::clone(&harness.writes);
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let operation = handle.submit(command()).await.unwrap();
        let _ = started.recv_async().await.unwrap();
        let cancel_handle = handle.clone();
        let cancel_task = tokio::spawn(async move { cancel_handle.cancel_test(operation).await });
        while handle.cancellations.is_empty() {
            tokio::task::yield_now().await;
        }
        frames
            .send_async(batch(vec![
                ack(ViscaSocket::S1),
                completion(ViscaSocket::S1),
            ]))
            .await
            .unwrap();
        handle.shutdown().await.unwrap();
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();

        let cancellation = cancel_task.await.unwrap().unwrap();
        assert!(matches!(
            cancellation.recv_test().await.unwrap(),
            CancellationObservation::Completed
        ));
        assert_eq!(
            writes
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, _, cancellation)| *cancellation)
                .count(),
            0,
            "queued cancellation is never written before shutdown"
        );
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn receipt_control_is_bound_to_its_originating_owner_and_clock() {
        let now = Instant::now();
        let first_runtime = ManualRuntime::new(now);
        let second_runtime = ManualRuntime::new(now.checked_add(Duration::from_secs(60)).unwrap());
        let (first, first_actor) = AsyncOwnerActor::new(policy(1), first_runtime).unwrap();
        let (second, second_actor) = AsyncOwnerActor::new(policy(1), second_runtime).unwrap();
        let first_harness = harness();
        let second_harness = harness();
        let first_started = first_harness.started.clone();
        let second_started = second_harness.started.clone();
        let first_gates = first_harness.gates.clone();
        let second_gates = second_harness.gates.clone();
        let first_task = tokio::spawn(first_actor.run(first_harness.driver));
        let second_task = tokio::spawn(second_actor.run(second_harness.driver));
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();

        let first_receipt = first
            .submit_command(prepared_focus(&profile))
            .await
            .unwrap();
        let second_receipt = second
            .submit_command(prepared_focus(&profile))
            .await
            .unwrap();
        let _ = first_started.recv_async().await.unwrap();
        let _ = second_started.recv_async().await.unwrap();
        assert!(matches!(
            first_receipt
                .wait_with_timeout(second.receipt_control(), Duration::ZERO)
                .await,
            Err(Error::InvalidState(_))
        ));
        second_receipt.detach();
        first_gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        second_gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        first.shutdown().await.unwrap();
        second.shutdown().await.unwrap();
        assert_eq!(first_task.await.unwrap().state, SessionState::Shutdown);
        assert_eq!(second_task.await.unwrap().state, SessionState::Shutdown);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn stream_poison_resolves_active_and_drains_unstaged_boundary() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(2), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let first = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), first.id);
        let queued_handle = handle.clone();
        let queued = tokio::spawn(async move { queued_handle.submit(command()).await });
        while handle.permits.available() != 0 {
            tokio::task::yield_now().await;
        }
        // The permit is acquired before the bounded boundary send. One more
        // yield lets that infallible next step enqueue before poison is released.
        tokio::task::yield_now().await;
        gates.send_async(Err(Error::Timeout)).await.unwrap();

        assert!(matches!(
            first.terminal().await.unwrap(),
            RuntimeOutcome::Failed(Error::StreamPoisoned { .. })
        ));
        assert!(matches!(
            queued.await.unwrap().unwrap_err(),
            Error::StreamPoisoned { .. }
        ));
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Poisoned);
        assert_eq!(snapshot.active, 0);
        assert_eq!(
            snapshot
                .metrics
                .dropped_boundary_work
                .saturating_add(snapshot.metrics.admission_rejected),
            1,
            "queued work is either drained before staging or rejected by the poisoned engine"
        );
    }

    /// Byte-stream transport whose reads are handed over one chunk at a time,
    /// so a test can split a single VISCA reply across two reads.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct ChunkedStreamTransport {
        config: crate::transport::builder::TransportConfig,
        chunks: flume::Receiver<Vec<u8>>,
        sent: flume::Sender<Vec<u8>>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::HasTransportConfig for ChunkedStreamTransport {
        fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
            &self.config
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::AsyncTransport for ChunkedStreamTransport {
        async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
            self.sent
                .send_async(bytes.to_vec())
                .await
                .map_err(|_| Error::RuntimeShutdown)
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            // An exhausted script parks instead of reporting end of stream, so
            // the test controls exactly when the transport closes.
            let Ok(chunk) = self.chunks.recv_async().await else {
                return future::pending().await;
            };
            let len = chunk.len().min(dst.len());
            dst[..len].copy_from_slice(&chunk[..len]);
            Ok(len)
        }

        fn send_semantics(&self) -> crate::transport::SendSemantics {
            crate::transport::SendSemantics::Stream
        }

        fn addressing_mode_hint(&self) -> Option<crate::transport::builder::AddressingMode> {
            Some(self.config.addressing)
        }
    }

    /// Driver-level stream script for raw-correlation boundary tests. `Tail`
    /// only produces a frame while an earlier `Prefix` remains buffered, which
    /// models the production framer closely enough to distinguish a completed
    /// boundary frame from a tail delivered after an orphan reset.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    enum BoundaryStreamRead {
        Prefix,
        Tail(DecodedFrame),
        Complete(DecodedFrame),
        NoData,
        Fault(Error),
    }

    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct BoundaryStreamDriver {
        reads: flume::Receiver<BoundaryStreamRead>,
        reads_observed: flume::Sender<()>,
        writes: flume::Sender<RequestId>,
        buffered: Arc<std::sync::atomic::AtomicBool>,
        discards: Arc<std::sync::atomic::AtomicUsize>,
        refuse_discard: bool,
        prefix_kind: crate::protocol::framer::RawIncompletePrefix,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for BoundaryStreamDriver {
        fn write(
            &mut self,
            write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            let id = write.request;
            let writes = self.writes.clone();
            async move {
                writes
                    .send_async(id)
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                Ok(TransmissionMeta { sequence: None })
            }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let reads = self.reads.clone();
            let reads_observed = self.reads_observed.clone();
            let buffered = Arc::clone(&self.buffered);
            async move {
                let read = reads
                    .recv_async()
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                let _ = reads_observed.try_send(());
                Ok(match read {
                    BoundaryStreamRead::Prefix => {
                        buffered.store(true, Ordering::Release);
                        AsyncReceive::Frames(Vec::new())
                    }
                    BoundaryStreamRead::Tail(frame) => {
                        if buffered.swap(false, Ordering::AcqRel) {
                            AsyncReceive::Frames(vec![frame])
                        } else {
                            AsyncReceive::Frames(Vec::new())
                        }
                    }
                    BoundaryStreamRead::Complete(frame) => {
                        buffered.store(false, Ordering::Release);
                        AsyncReceive::Frames(vec![frame])
                    }
                    BoundaryStreamRead::NoData => AsyncReceive::NoData,
                    BoundaryStreamRead::Fault(error) => AsyncReceive::Fault(error),
                })
            }
        }

        fn has_buffered_stream_input(&mut self) -> Result<bool, Error> {
            Ok(self.buffered.load(Ordering::Acquire))
        }

        fn buffered_stream_input(&mut self) -> Result<Option<RawPrefixEvidence>, Error> {
            Ok(self
                .buffered
                .load(Ordering::Acquire)
                .then_some(RawPrefixEvidence::Incomplete {
                    target: CameraId::CAMERA_1,
                    // The fixture defaults to an exact named terminal, keeping
                    // its original stale-prefix tests about discard mechanics.
                    // A focused actor test may override this with ambiguous
                    // evidence to exercise the local deferral budget.
                    kind: self.prefix_kind,
                }))
        }

        fn discard_buffered_stream_input(&mut self) -> Result<(), Error> {
            self.discards.fetch_add(1, Ordering::Relaxed);
            if !self.refuse_discard {
                self.buffered.store(false, Ordering::Release);
            }
            Ok(())
        }
    }

    #[cfg(feature = "runtime-tokio")]
    fn raw_inquiry_reply(payload: u8) -> DecodedFrame {
        DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::InquiryReply {
                route: None,
                payload: smallvec::smallvec![payload],
            },
        }
    }

    #[cfg(feature = "runtime-tokio")]
    struct BoundaryStreamHarness {
        driver: Option<BoundaryStreamDriver>,
        reads: flume::Sender<BoundaryStreamRead>,
        reads_observed: flume::Receiver<()>,
        writes: flume::Receiver<RequestId>,
        buffered: Arc<std::sync::atomic::AtomicBool>,
        discards: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(feature = "runtime-tokio")]
    fn boundary_stream_harness(refuse_discard: bool) -> BoundaryStreamHarness {
        let (read_tx, reads) = flume::bounded(128);
        let (read_observed_tx, reads_observed) = flume::unbounded();
        let (writes, write_rx) = flume::bounded(16);
        let buffered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let discards = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        BoundaryStreamHarness {
            driver: Some(BoundaryStreamDriver {
                reads,
                reads_observed: read_observed_tx,
                writes,
                buffered: Arc::clone(&buffered),
                discards: Arc::clone(&discards),
                refuse_discard,
                prefix_kind: crate::protocol::framer::RawIncompletePrefix::NamedCompletionOrError(
                    ViscaSocket::S1,
                ),
            }),
            reads: read_tx,
            reads_observed,
            writes: write_rx,
            buffered,
            discards,
        }
    }

    /// Minimal raw receive/write script used to prove the exact-release probe
    /// on both stream and datagram policies.  The read-consumed channel is an
    /// actor-side barrier: a test never infers consumption from a scheduler
    /// yield or wall-clock sleep.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    enum RawReleaseProbeRead {
        NoData,
        NoDataThenComplete(DecodedFrame),
        Empty,
        Fault(Error),
        RepeatingFault(Error),
        Complete(DecodedFrame),
    }

    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct RawReleaseProbeDriver {
        reads: flume::Receiver<RawReleaseProbeRead>,
        reads_observed: flume::Sender<()>,
        receive_polled: flume::Sender<()>,
        after_no_data: Arc<Mutex<Option<DecodedFrame>>>,
        repeating_fault: Arc<Mutex<Option<Error>>>,
        writes: flume::Sender<RequestId>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for RawReleaseProbeDriver {
        fn write(
            &mut self,
            write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            let writes = self.writes.clone();
            let id = write.request;
            async move {
                writes
                    .send_async(id)
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                Ok(TransmissionMeta { sequence: None })
            }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let reads = self.reads.clone();
            let reads_observed = self.reads_observed.clone();
            let receive_polled = self.receive_polled.clone();
            let after_no_data = Arc::clone(&self.after_no_data);
            let repeating_fault = Arc::clone(&self.repeating_fault);
            async move {
                let _ = receive_polled.try_send(());
                if let Some(frame) = after_no_data.lock().unwrap().take() {
                    let _ = reads_observed.try_send(());
                    return Ok(AsyncReceive::Frames(vec![frame]));
                }
                if let Some(error) = repeating_fault.lock().unwrap().as_ref().cloned() {
                    let _ = reads_observed.try_send(());
                    return Ok(AsyncReceive::Fault(error));
                }
                let read = reads
                    .recv_async()
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                let _ = reads_observed.try_send(());
                Ok(match read {
                    RawReleaseProbeRead::NoData => AsyncReceive::NoData,
                    RawReleaseProbeRead::NoDataThenComplete(frame) => {
                        *after_no_data.lock().unwrap() = Some(frame);
                        AsyncReceive::NoData
                    }
                    RawReleaseProbeRead::Empty => AsyncReceive::Frames(Vec::new()),
                    RawReleaseProbeRead::Fault(error) => AsyncReceive::Fault(error),
                    RawReleaseProbeRead::RepeatingFault(error) => {
                        *repeating_fault.lock().unwrap() = Some(error.clone());
                        AsyncReceive::Fault(error)
                    }
                    RawReleaseProbeRead::Complete(frame) => AsyncReceive::Frames(vec![frame]),
                })
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    struct RawReleaseProbeHarness {
        driver: Option<RawReleaseProbeDriver>,
        reads: flume::Sender<RawReleaseProbeRead>,
        reads_observed: flume::Receiver<()>,
        receive_polled: flume::Receiver<()>,
        repeating_fault: Arc<Mutex<Option<Error>>>,
        writes: flume::Receiver<RequestId>,
    }

    #[cfg(feature = "runtime-tokio")]
    fn raw_release_probe_harness() -> RawReleaseProbeHarness {
        let (read_tx, reads) = flume::bounded(16);
        let (read_observed_tx, reads_observed) = flume::unbounded();
        let (receive_polled_tx, receive_polled) = flume::unbounded();
        let (writes, write_rx) = flume::bounded(16);
        let after_no_data = Arc::new(Mutex::new(None));
        let repeating_fault = Arc::new(Mutex::new(None));
        RawReleaseProbeHarness {
            driver: Some(RawReleaseProbeDriver {
                reads,
                reads_observed: read_observed_tx,
                receive_polled: receive_polled_tx,
                after_no_data,
                repeating_fault: Arc::clone(&repeating_fault),
                writes,
            }),
            reads: read_tx,
            reads_observed,
            receive_polled,
            repeating_fault,
            writes: write_rx,
        }
    }

    /// A raw driver whose target-2 write deliberately parks.  It models the
    /// production `drive()` seam: target 1 has an expired correlation
    /// tombstone, but the actor is inside an unrelated transport write when H
    /// is crossed.  The read-consumed channel makes the return-to-coordinator
    /// ordering observable without relying on task scheduling.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct ParkedWriteRawReleaseDriver {
        reads: flume::Receiver<RawReleaseProbeRead>,
        reads_observed: flume::Sender<()>,
        writes: flume::Sender<RequestId>,
        target_two_gate: flume::Receiver<Result<TransmissionMeta, Error>>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for ParkedWriteRawReleaseDriver {
        fn write(
            &mut self,
            write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            let request = write.request;
            // The raw request fixture encodes Camera 2 as the first wire byte
            // (0x80 + target id).  Read it before returning the future, while
            // `WireWrite` is still borrowed from the owner.
            let target_two = write.bytes.first() == Some(&0x82);
            let writes = self.writes.clone();
            let target_two_gate = self.target_two_gate.clone();
            async move {
                writes
                    .send_async(request)
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                if target_two {
                    target_two_gate
                        .recv_async()
                        .await
                        .unwrap_or(Err(Error::RuntimeShutdown))
                } else {
                    Ok(TransmissionMeta { sequence: None })
                }
            }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let reads = self.reads.clone();
            let reads_observed = self.reads_observed.clone();
            async move {
                let read = reads
                    .recv_async()
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                let _ = reads_observed.try_send(());
                Ok(match read {
                    RawReleaseProbeRead::NoData | RawReleaseProbeRead::NoDataThenComplete(_) => {
                        AsyncReceive::NoData
                    }
                    RawReleaseProbeRead::Empty => AsyncReceive::Frames(Vec::new()),
                    RawReleaseProbeRead::Fault(error)
                    | RawReleaseProbeRead::RepeatingFault(error) => AsyncReceive::Fault(error),
                    RawReleaseProbeRead::Complete(frame) => AsyncReceive::Frames(vec![frame]),
                })
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    struct ParkedWriteRawReleaseHarness {
        driver: Option<ParkedWriteRawReleaseDriver>,
        reads: flume::Sender<RawReleaseProbeRead>,
        reads_observed: flume::Receiver<()>,
        writes: flume::Receiver<RequestId>,
        target_two_gate: flume::Sender<Result<TransmissionMeta, Error>>,
    }

    #[cfg(feature = "runtime-tokio")]
    fn parked_write_raw_release_harness() -> ParkedWriteRawReleaseHarness {
        let (read_tx, reads) = flume::bounded(16);
        let (reads_observed_tx, reads_observed) = flume::unbounded();
        let (writes, writes_rx) = flume::bounded(16);
        let (target_two_gate, target_two_gates) = flume::bounded(1);
        ParkedWriteRawReleaseHarness {
            driver: Some(ParkedWriteRawReleaseDriver {
                reads,
                reads_observed: reads_observed_tx,
                writes,
                target_two_gate: target_two_gates,
            }),
            reads: read_tx,
            reads_observed,
            writes: writes_rx,
            target_two_gate,
        }
    }

    #[cfg(feature = "runtime-tokio")]
    async fn establish_raw_inquiry_tombstone_with_probe_driver(
        handle: &AsyncOwnerHandle,
        harness: &RawReleaseProbeHarness,
    ) -> ReceiptCore {
        let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
        assert_eq!(harness.writes.recv_async().await.unwrap(), predecessor.id);
        assert!(matches!(
            predecessor.terminal().await.unwrap(),
            RuntimeOutcome::Failed(Error::Timeout)
        ));

        let successor = handle.submit(inquiry()).await.unwrap();
        assert!(
            harness.writes.try_recv().is_err(),
            "the raw tombstone must retain the successor before its boundary"
        );
        successor
    }

    /// Exercises the H-δ idle path that used to let a boundary-first Wake
    /// release B before an already-ready stale A was even framed.  The
    /// read-consumed and sleep-entered barriers make the ordering independent
    /// of Tokio task timing: stale A is injected only after NoData has started
    /// its clamp-to-H sleep, then H is advanced explicitly.
    #[cfg(feature = "runtime-tokio")]
    async fn assert_raw_release_probe_precedes_stale_frame_after_idle_sleep(policy: OwnerPolicy) {
        let initial = Instant::now();
        let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(initial);
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
        let mut harness = raw_release_probe_harness();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

        // The predecessor installed its one-second raw inquiry tombstone at
        // `initial`.  At H−1ms, consume an idle read; its escalating pause is
        // clamped to exactly the release boundary.
        runtime.advance(Duration::from_millis(999));
        harness
            .reads
            .send_async(RawReleaseProbeRead::NoData)
            .await
            .unwrap();
        harness.reads_observed.recv_async().await.unwrap();
        loop {
            if sleeps.recv_async().await.unwrap() == Duration::from_millis(1) {
                break;
            }
        }

        // A is now transport-ready but cannot have been framed while the
        // actor is parked in the clamped idle sleep.  A pre-fix boundary-first
        // Wake would dispatch B at H before consuming this message.
        harness
            .reads
            .send_async(RawReleaseProbeRead::Complete(raw_inquiry_reply(0xa1)))
            .await
            .unwrap();
        assert!(
            harness.reads_observed.try_recv().is_err(),
            "the stale frame must remain unframed until H wakes the actor"
        );
        runtime.advance(Duration::from_millis(1));

        enum BoundaryOrder {
            StaleRead,
            SuccessorWrite,
        }
        let first_after_h = future::or(
            async {
                harness.reads_observed.recv_async().await.unwrap();
                BoundaryOrder::StaleRead
            },
            async {
                let _ = harness.writes.recv_async().await.unwrap();
                BoundaryOrder::SuccessorWrite
            },
        )
        .await;
        assert!(
            matches!(first_after_h, BoundaryOrder::StaleRead),
            "the exact raw-release probe must consume stale A before B writes"
        );

        assert_eq!(
            harness.writes.recv_async().await.unwrap(),
            successor.id,
            "only the post-A due pass may dispatch B"
        );
        let snapshot = handle.snapshot().await.unwrap();
        assert!(snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
        )));

        harness
            .reads
            .send_async(RawReleaseProbeRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// Datagram input has no retained framer, but the pre-H idle/readiness
    /// race is the same actor-level source-order bug as the stream path.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_datagram_release_probe_precedes_stale_frame_after_idle_sleep() {
        assert_raw_release_probe_precedes_stale_frame_after_idle_sleep(policy(1)).await;
    }

    /// The stream path must use the identical exact-release probe before its
    /// own framing-specific retained-prefix rules are considered.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_release_probe_precedes_stale_frame_after_idle_sleep() {
        assert_raw_release_probe_precedes_stale_frame_after_idle_sleep(stream_policy(1)).await;
    }

    /// A read's timestamp, rather than the later time at which the actor
    /// handles its event, defines whether it discharged the exact-H probe. A
    /// pre-H `NoData` can resume after H with an old frame already waiting; it
    /// must cause another receive-first pass, not fence the due Wake.
    #[cfg(feature = "runtime-tokio")]
    async fn assert_pre_h_idle_read_resumed_at_h_requires_fresh_probe(policy: OwnerPolicy) {
        let initial = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(initial);
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
        let mut harness = raw_release_probe_harness();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

        // Put the actor in an ordinary H-δ boundary selection with a receive
        // already pending.  This avoids any scheduler sleep assumption: the
        // driver's poll notification is the exact barrier.
        while harness.receive_polled.try_recv().is_ok() {}
        runtime.advance(Duration::from_millis(999));
        harness
            .reads
            .send_async(RawReleaseProbeRead::Empty)
            .await
            .unwrap();
        harness.reads_observed.recv_async().await.unwrap();
        harness.receive_polled.recv_async().await.unwrap();

        // The scripted receive is stamped at H-ε, then `now()` advances the
        // virtual clock before the selected event is handled.  It leaves a
        // complete stale A ready for the next receive.  Keying the fence on
        // `selected_at` would let B write first; keying it on `received_at`
        // makes that next probe consume A first.
        runtime.advance_after_next_now(Duration::from_millis(1));
        harness
            .reads
            .try_send(RawReleaseProbeRead::NoDataThenComplete(raw_inquiry_reply(
                0xa1,
            )))
            .unwrap();
        harness.reads_observed.recv_async().await.unwrap();

        enum BoundaryOrder {
            StaleRead,
            SuccessorWrite,
        }
        let first_after_h = future::or(
            async {
                harness.reads_observed.recv_async().await.unwrap();
                BoundaryOrder::StaleRead
            },
            async {
                let _ = harness.writes.recv_async().await.unwrap();
                BoundaryOrder::SuccessorWrite
            },
        )
        .await;
        assert!(
            matches!(first_after_h, BoundaryOrder::StaleRead),
            "a pre-H NoData resumed at H must not fence the stale-frame probe"
        );
        assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
        assert!(handle
            .snapshot()
            .await
            .unwrap()
            .diagnostics
            .iter()
            .any(|event| matches!(
                event,
                DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
            )));

        harness
            .reads
            .send_async(RawReleaseProbeRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_datagram_pre_h_idle_read_resumed_at_h_requires_fresh_probe() {
        assert_pre_h_idle_read_resumed_at_h_requires_fresh_probe(policy(1)).await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_pre_h_idle_read_resumed_at_h_requires_fresh_probe() {
        assert_pre_h_idle_read_resumed_at_h_requires_fresh_probe(stream_policy(1)).await;
    }

    /// A custom async driver may report an idle read timeout through its
    /// `Fault` result instead of `NoData`.  That path is normalized by the
    /// owner, so an exact-H timeout must install the same no-input fence.  The
    /// driver repeats the idle fault forever after its first result: without
    /// the fence a left-biased receive probe hot-loops and B never writes.
    #[cfg(feature = "runtime-tokio")]
    async fn assert_raw_release_idle_fault_fences_once(policy: OwnerPolicy) {
        let initial = Instant::now();
        let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(initial);
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
        let mut harness = raw_release_probe_harness();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

        runtime.advance(Duration::from_millis(999));
        harness
            .reads
            .send_async(RawReleaseProbeRead::NoData)
            .await
            .unwrap();
        harness.reads_observed.recv_async().await.unwrap();
        loop {
            if sleeps.recv_async().await.unwrap() == Duration::from_millis(1) {
                break;
            }
        }

        // This fault is the documented non-consuming idle condition, not a
        // transient ConnectionRefused-style error.  It is queued before H so
        // the exact coordinator consumes it on the first post-H probe.
        harness
            .reads
            .try_send(RawReleaseProbeRead::RepeatingFault(Error::Timeout))
            .unwrap();
        runtime.advance(Duration::from_millis(1));
        harness.reads_observed.recv_async().await.unwrap();

        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), harness.writes.recv_async())
                .await
                .expect("an exact idle fault must fence once rather than hot-loop")
                .unwrap(),
            successor.id
        );

        // Stop the intentionally adversarial fixture before terminating its
        // owner; real idle transports become pending between reads, whereas
        // this test's value is specifically that it would otherwise stay
        // perpetually ready if the fence were removed.
        *harness.repeating_fault.lock().unwrap() = None;
        handle.shutdown().await.unwrap();
        // The actor may have started its ordinary post-write idle pacing sleep
        // just before the fixture was cleared; wake that sleep so the queued
        // shutdown boundary is observed without depending on wall clock.
        runtime.advance(Duration::from_secs(1));
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_datagram_release_idle_fault_fences_once() {
        assert_raw_release_idle_fault_fences_once(policy(1)).await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_release_idle_fault_fences_once() {
        assert_raw_release_idle_fault_fences_once(stream_policy(1)).await;
    }

    /// A parked target-2 write may finish exactly as target 1's raw tombstone
    /// expires.  `finish_write` used to run an ordinary due/dispatch tail
    /// recursively from `drive()`, allowing B to write before the actor ever
    /// returned to receive stale A.  The async no-due completion path must
    /// instead return to the top-level raw coordinator, where stale A wins the
    /// left-biased input probe before B's due pass.
    #[cfg(feature = "runtime-tokio")]
    async fn assert_parked_cross_target_write_returns_to_raw_coordinator(policy: OwnerPolicy) {
        let initial = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(initial);
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
        let mut harness = parked_write_raw_release_harness();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));

        let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
        assert_eq!(harness.writes.recv_async().await.unwrap(), predecessor.id);
        assert!(matches!(
            predecessor.terminal().await.unwrap(),
            RuntimeOutcome::Failed(Error::Timeout)
        ));

        let successor = handle.submit(inquiry()).await.unwrap();
        assert!(
            harness.writes.try_recv().is_err(),
            "target-1's raw tombstone must hold B before H"
        );

        // C is independent target-2 work.  Its driver write begins at H-δ
        // and is held there while stale A becomes transport-ready; therefore
        // no receive can consume A before C's `finish_write` executes at H.
        runtime.advance(Duration::from_millis(999));
        let parked = handle
            .submit(command_for(CameraId::CAMERA_2))
            .await
            .unwrap();
        assert_eq!(harness.writes.recv_async().await.unwrap(), parked.id);
        harness
            .reads
            .try_send(RawReleaseProbeRead::Complete(raw_inquiry_reply(0xa1)))
            .unwrap();
        runtime.advance_silently(Duration::from_millis(1));
        harness
            .target_two_gate
            .try_send(Ok(TransmissionMeta { sequence: None }))
            .unwrap();

        enum BoundaryOrder {
            StaleRead,
            SuccessorWrite,
        }
        let first_after_parked_write = future::or(
            async {
                harness.reads_observed.recv_async().await.unwrap();
                BoundaryOrder::StaleRead
            },
            async {
                let _ = harness.writes.recv_async().await.unwrap();
                BoundaryOrder::SuccessorWrite
            },
        )
        .await;
        assert!(
            matches!(first_after_parked_write, BoundaryOrder::StaleRead),
            "a parked write completion crossing H must return to the raw receive probe before B writes"
        );
        assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
        assert!(handle
            .snapshot()
            .await
            .unwrap()
            .diagnostics
            .iter()
            .any(|event| matches!(
                event,
                DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
            )));

        harness
            .reads
            .send_async(RawReleaseProbeRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_datagram_parked_cross_target_write_returns_to_raw_coordinator() {
        assert_parked_cross_target_write_returns_to_raw_coordinator(two_target_raw_policy(
            TransportKind::Datagram,
        ))
        .await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_parked_cross_target_write_returns_to_raw_coordinator() {
        assert_parked_cross_target_write_returns_to_raw_coordinator(two_target_raw_policy(
            TransportKind::Stream,
        ))
        .await;
    }

    /// Reproduces the other temporal hole: an ordinary boundary-first
    /// selection begins at H−δ, then a transient receive fault wakes that
    /// selection after H alongside the due timer.  The fault itself cannot
    /// authorize a raw release; stale A behind it must be read before B can
    /// write.  Both read barriers must therefore beat B's write barrier.
    #[cfg(feature = "runtime-tokio")]
    async fn assert_raw_release_fault_then_stale_frame_stays_input_first(policy: OwnerPolicy) {
        let initial = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(initial);
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
        let mut harness = raw_release_probe_harness();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

        // The actor is normally already blocked in receive after B's admission.
        // Drain its old poll notification, then use an empty nonzero receive to
        // enter `YieldBoundaries` without the idle-read sleep path.
        while harness.receive_polled.try_recv().is_ok() {}
        runtime.advance(Duration::from_millis(999));
        harness
            .reads
            .send_async(RawReleaseProbeRead::Empty)
            .await
            .unwrap();
        harness.reads_observed.recv_async().await.unwrap();
        harness.receive_polled.recv_async().await.unwrap();

        // Move virtual time to H without waking its timer, then make the
        // already-pending receive ready with a transient fault followed by
        // stale A.  That receive wake polls the due timer in the same
        // all-ready boundary-first selection; no wall-clock timing is involved.
        runtime.advance_silently(Duration::from_millis(1));
        harness
            .reads
            .try_send(RawReleaseProbeRead::Fault(Error::Io(Arc::new(
                std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
            ))))
            .unwrap();
        harness
            .reads
            .try_send(RawReleaseProbeRead::Complete(raw_inquiry_reply(0xa1)))
            .unwrap();

        enum BoundaryOrder {
            Read,
            SuccessorWrite,
        }
        for description in ["the fault", "stale A"] {
            let next = future::or(
                async {
                    harness.reads_observed.recv_async().await.unwrap();
                    BoundaryOrder::Read
                },
                async {
                    let _ = harness.writes.recv_async().await.unwrap();
                    BoundaryOrder::SuccessorWrite
                },
            )
            .await;
            assert!(
                matches!(next, BoundaryOrder::Read),
                "{description} must be consumed before the due release can write B"
            );
        }
        assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
        let snapshot = handle.snapshot().await.unwrap();
        assert!(snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
        )));

        harness
            .reads
            .send_async(RawReleaseProbeRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_datagram_release_fault_then_stale_frame_stays_input_first() {
        assert_raw_release_fault_then_stale_frame_stays_input_first(policy(1)).await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_release_fault_then_stale_frame_stays_input_first() {
        assert_raw_release_fault_then_stale_frame_stays_input_first(stream_policy(1)).await;
    }

    /// The retained-stream cap must be terminal before an admission channel is
    /// allowed to end an ordinary engine input turn.  This reaches the actual
    /// `raw_buffered_cap_latched` path: 63 retained empty reads occur before
    /// H, the 64th crosses H, and C is already queued when the forced boundary
    /// turn runs.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_buffered_cap_preempts_queued_admission_before_any_write() {
        let initial = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(initial);
        let mut policy = stream_policy(3);
        // Raw inquiry tombstones are intentionally a single-flight production
        // rule, independent of the broader admission capacity used here to
        // queue C behind B.
        policy.protocol.inquiry_capacity = 1;
        policy.limits.frames_per_receive = RAW_CORRELATION_RELEASE_WORK_LIMIT + 1;
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;
        while harness.reads_observed.try_recv().is_ok() {}

        for _ in 0..RAW_CORRELATION_RELEASE_WORK_LIMIT - 1 {
            harness.reads.try_send(BoundaryStreamRead::Prefix).unwrap();
        }
        for _ in 0..RAW_CORRELATION_RELEASE_WORK_LIMIT - 1 {
            harness.reads_observed.recv_async().await.unwrap();
        }

        // Do not wake the old timer yet.  The already-pending receive observes
        // both this final retained fragment and C, at H, in one deterministic
        // source race; receive-first consumes the fragment and arms the cap.
        runtime.advance_silently(Duration::from_secs(1));
        let (_c_completion, c_admitted) = handle.enqueue_admission(inquiry(), None).unwrap();
        harness.reads.try_send(BoundaryStreamRead::Prefix).unwrap();
        harness.reads_observed.recv_async().await.unwrap();

        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), successor.terminal())
                .await
                .expect("the latched retained-input cap must terminate"),
            Ok(RuntimeOutcome::Failed(Error::StreamPoisoned { .. }))
        ));
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), c_admitted.recv_async())
                .await
                .expect("the queued admission must be answered by terminal drain"),
            Ok(Err(Error::StreamPoisoned { .. }))
        ));
        assert!(
            harness.writes.try_recv().is_err(),
            "neither B nor queued C may write after the retained-input cap latches"
        );
        assert_eq!(actor_task.await.unwrap().state, SessionState::Poisoned);
    }

    /// The same cap must preempt a valid cancellation boundary.  A cancellation
    /// is normally an engine input turn too; if it ran first it could release
    /// B/emit a cancellation write before the stream is poisoned.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_buffered_cap_preempts_valid_cancellation_before_any_write() {
        let initial = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(initial);
        let mut policy = stream_policy(2);
        policy.protocol.inquiry_capacity = 1;
        policy.limits.frames_per_receive = RAW_CORRELATION_RELEASE_WORK_LIMIT + 1;
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;
        while harness.reads_observed.try_recv().is_ok() {}

        for _ in 0..RAW_CORRELATION_RELEASE_WORK_LIMIT - 1 {
            harness.reads.try_send(BoundaryStreamRead::Prefix).unwrap();
        }
        for _ in 0..RAW_CORRELATION_RELEASE_WORK_LIMIT - 1 {
            harness.reads_observed.recv_async().await.unwrap();
        }

        runtime.advance_silently(Duration::from_secs(1));
        let (cancellation_reply, cancellation_result) = flume::bounded(1);
        handle
            .cancellations
            .try_send(CancellationBoundary {
                receipt: successor,
                reply: cancellation_reply,
            })
            .unwrap();
        harness.reads.try_send(BoundaryStreamRead::Prefix).unwrap();
        harness.reads_observed.recv_async().await.unwrap();

        let cancellation =
            tokio::time::timeout(Duration::from_secs(1), cancellation_result.recv_async())
                .await
                .expect("the queued cancellation must be answered by terminal drain")
                .unwrap()
                .expect("a terminal request observation remains a valid cancellation receipt");
        assert!(matches!(
            cancellation.recv_async().await.unwrap(),
            ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::StreamPoisoned { .. }))
        ));
        assert!(
            harness.writes.try_recv().is_err(),
            "the retained-input cap must poison before B or a cancellation can write"
        );
        assert_eq!(actor_task.await.unwrap().state, SessionState::Poisoned);
    }

    #[cfg(feature = "runtime-tokio")]
    async fn terminal_within_test_deadline(
        receipt: &ReceiptCore,
        context: &'static str,
    ) -> RuntimeOutcome {
        tokio::time::timeout(Duration::from_secs(1), receipt.terminal())
            .await
            .expect(context)
            .expect("the receipt observation channel remains live")
    }

    #[cfg(feature = "runtime-tokio")]
    async fn establish_raw_inquiry_tombstone(
        handle: &AsyncOwnerHandle,
        harness: &BoundaryStreamHarness,
    ) -> ReceiptCore {
        let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
        assert_eq!(harness.writes.recv_async().await.unwrap(), predecessor.id);
        assert!(matches!(
            terminal_within_test_deadline(
                &predecessor,
                "the predecessor terminalizes at its inquiry deadline",
            )
            .await,
            RuntimeOutcome::Failed(Error::Timeout)
        ));

        let successor = handle.submit(inquiry()).await.unwrap();
        assert!(
            harness.writes.try_recv().is_err(),
            "the raw tombstone must retain the successor before its boundary"
        );
        successor
    }

    /// Start two raw commands on the same target and assign their distinct
    /// camera sockets through the real async transport/framer. X remains live
    /// on S1 while Y's shorter completion deadline will enter its exact-S2
    /// ambiguity quarantine. Z is queued so the boundary test can prove when
    /// dispatch becomes legal.
    #[cfg(feature = "runtime-tokio")]
    async fn establish_production_two_socket_boundary(
        handle: &AsyncOwnerHandle,
        chunks: &flume::Sender<Vec<u8>>,
        sent: &flume::Receiver<Vec<u8>>,
    ) -> (ReceiptCore, ReceiptCore, ReceiptCore) {
        let x = handle
            .submit(raw_command_with_completion_deadline(
                0x11,
                Duration::from_secs(30),
            ))
            .await
            .unwrap();
        assert_eq!(
            sent.recv_async().await.unwrap(),
            vec![0x81, 0x01, 0x04, 0x11, 0xff],
            "X must be the first physical write",
        );
        chunks.send_async(vec![0x90, 0x41, 0xff]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();

        let y = handle
            .submit(raw_command_with_completion_deadline(
                0x22,
                Duration::from_secs(5),
            ))
            .await
            .unwrap();
        assert_eq!(
            sent.recv_async().await.unwrap(),
            vec![0x81, 0x01, 0x04, 0x22, 0xff],
            "X's ACK must free the second raw camera socket",
        );
        chunks.send_async(vec![0x90, 0x42, 0xff]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();

        let z = handle
            .submit(raw_command_with_completion_deadline(
                0x33,
                Duration::from_secs(30),
            ))
            .await
            .unwrap();
        assert!(
            sent.try_recv().is_err(),
            "both raw camera sockets are occupied before Y's quarantine releases",
        );
        (x, y, z)
    }

    /// Let Y first cross its completion deadline into the exact-S2 quarantine,
    /// leaving only the one-second ambiguity release for the test to race.
    #[cfg(feature = "runtime-tokio")]
    async fn enter_production_s2_quarantine(handle: &AsyncOwnerHandle, runtime: &ManualRuntime) {
        runtime.advance(Duration::from_secs(5));
        let _ = handle.snapshot().await.unwrap();
        // The first control may consume its one documented allowance ahead of
        // the mature timer. The next selection makes that timer precede another
        // control, so this second round-trip is an observable barrier: Y has
        // entered its S2 ambiguity quarantine before a test installs bytes for
        // the later exact-release boundary.
        let _ = handle.snapshot().await.unwrap();
    }

    /// A tail ready at the exact raw tombstone expiry is decoded and made
    /// inert before the due pass releases the successor. This is the original
    /// split-tail correlation regression.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_split_tail_at_tombstone_boundary_precedes_release() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let reads = harness.reads.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

        reads.send_async(BoundaryStreamRead::Prefix).await.unwrap();
        while !harness.buffered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.advance(Duration::from_secs(1));
        reads
            .send_async(BoundaryStreamRead::Tail(raw_inquiry_reply(0xa1)))
            .await
            .unwrap();

        assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
        reads
            .send_async(BoundaryStreamRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));
        assert_eq!(harness.discards.load(Ordering::Relaxed), 0);

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// Retained stream input survives a transient-fault turn. Even when both
    /// fault and tail are queued at the exact boundary, the completing tail
    /// keeps receive precedence until it has been made inert.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_prefix_fault_tail_keeps_boundary_input_precedence() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let reads = harness.reads.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

        reads.send_async(BoundaryStreamRead::Prefix).await.unwrap();
        while !harness.buffered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.advance(Duration::from_secs(1));
        reads
            .send_async(BoundaryStreamRead::Fault(Error::Io(Arc::new(
                std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
            ))))
            .await
            .unwrap();
        reads
            .send_async(BoundaryStreamRead::Tail(raw_inquiry_reply(0xa1)))
            .await
            .unwrap();

        assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
        reads
            .send_async(BoundaryStreamRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// The idle/no-data classification also preserves a positively identified
    /// prefix instead of yielding the due raw boundary ahead of its ready tail.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_prefix_idle_tail_keeps_boundary_input_precedence() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let reads = harness.reads.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

        reads.send_async(BoundaryStreamRead::Prefix).await.unwrap();
        while !harness.buffered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.advance(Duration::from_secs(1));
        reads.send_async(BoundaryStreamRead::NoData).await.unwrap();
        reads
            .send_async(BoundaryStreamRead::Tail(raw_inquiry_reply(0xa1)))
            .await
            .unwrap();

        assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
        reads
            .send_async(BoundaryStreamRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// If no completing tail is ready at release, the old prefix is discarded
    /// before the successor writes. A later tail therefore cannot become the
    /// successor's response.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_orphan_prefix_is_reset_before_successor_dispatch() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let reads = harness.reads.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

        reads.send_async(BoundaryStreamRead::Prefix).await.unwrap();
        while !harness.buffered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.advance(Duration::from_secs(1));
        let _ = handle.snapshot().await.unwrap();
        assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
        assert_eq!(
            harness.discards.load(Ordering::Relaxed),
            1,
            "the orphaned fragment is discarded exactly once"
        );

        reads
            .send_async(BoundaryStreamRead::Tail(raw_inquiry_reply(0xa1)))
            .await
            .unwrap();
        reads
            .send_async(BoundaryStreamRead::Complete(raw_inquiry_reply(0xb2)))
            .await
            .unwrap();
        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A decoder that claims to discard but retains the old prefix makes safe
    /// correlation release impossible. Fail closed instead of dispatching the
    /// successor behind unverifiable framing state.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_nonclearing_decoder_poisons_at_release() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(true);
        let reads = harness.reads.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

        reads.send_async(BoundaryStreamRead::Prefix).await.unwrap();
        while !harness.buffered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.advance(Duration::from_secs(1));
        let _ = handle.snapshot().await.unwrap();

        assert!(matches!(
            successor.terminal().await.unwrap(),
            RuntimeOutcome::Failed(Error::StreamPoisoned { .. })
        ));
        assert_eq!(
            harness.discards.load(Ordering::Relaxed),
            64,
            "a non-clearing decoder is retried only to the bounded framing work cap"
        );
        assert_eq!(actor_task.await.unwrap().state, SessionState::Poisoned);
    }

    /// An ambiguous retained prefix consumes elapsed grace, not a fixed number
    /// of immediately-ready actor turns. At expiry it is discarded and the
    /// successor remains usable (#713).
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_ambiguous_prefix_expires_without_poisoning_successor() {
        let now = Instant::now();
        let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(now);
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        // Only an identity-weak prefix is unresolved at an inquiry release.
        // The default named-terminal fixture is intentionally discarded by
        // policy, which is correct for its other stale-prefix tests but cannot
        // exercise this fail-closed deferral cap.
        harness.driver.as_mut().unwrap().prefix_kind =
            crate::protocol::framer::RawIncompletePrefix::SourceOnly;
        let reads = harness.reads.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
        let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

        reads.send_async(BoundaryStreamRead::Prefix).await.unwrap();
        while !harness.buffered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        // Move to H and wake the old timer. The engine now establishes one
        // 100 ms grace deadline for the retained source-only prefix.
        runtime.advance(Duration::from_secs(1));
        loop {
            if sleeps.recv_async().await.unwrap() == Duration::from_millis(100) {
                break;
            }
        }
        runtime.advance(Duration::from_millis(100));
        let successor_id =
            tokio::time::timeout(Duration::from_secs(1), harness.writes.recv_async())
                .await
                .expect("the successor writes after the orphan grace")
                .unwrap();
        assert_eq!(successor_id, successor.id);
        assert_eq!(harness.discards.load(Ordering::Relaxed), 1);

        reads
            .try_send(BoundaryStreamRead::Complete(raw_inquiry_reply(0xb2)))
            .unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), successor.terminal())
                .await
                .expect("the successor remains usable after orphan discard")
                .unwrap(),
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A completed tail can perform the due release from `finish_input_turn`,
    /// without another `Wake` arm. Its former ambiguous-prefix deferrals must
    /// not be charged to a later, independent raw hold.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn raw_stream_completed_tail_resets_next_hold_grace_budget() {
        let initial = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(initial);
        let (handle, mut actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let mut driver = harness.driver.take().unwrap();
        // Make retained bytes deliberately unkeyed so the engine assigns a
        // grace deadline rather than discarding immediately.
        driver.prefix_kind = crate::protocol::framer::RawIncompletePrefix::SourceOnly;

        // A times out after its successful write, which installs the genuine
        // late-reply hold retained by #712. B remains queued behind it.
        let (a_completion, a_admitted) =
            handle.enqueue_admission(timed_out_inquiry(), None).unwrap();
        let a_boundary = actor.admissions.try_recv().unwrap();
        actor
            .handle_admission(a_boundary, &mut driver, &runtime, Executor::now(&runtime))
            .await;
        let a = a_admitted.recv_async().await.unwrap().unwrap();
        assert_eq!(harness.writes.recv_async().await.unwrap(), a);
        assert!(matches!(
            a_completion.recv_async().await.unwrap(),
            ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
        ));

        let (b_completion, b_admitted) =
            handle.enqueue_admission(timed_out_inquiry(), None).unwrap();
        let b_boundary = actor.admissions.try_recv().unwrap();
        actor
            .handle_admission(b_boundary, &mut driver, &runtime, Executor::now(&runtime))
            .await;
        let _b = b_admitted.recv_async().await.unwrap().unwrap();
        assert!(harness.writes.try_recv().is_err());

        driver.buffered.store(true, Ordering::Release);
        runtime.advance(Duration::from_secs(1));
        assert_eq!(
            actor
                .handle_event(
                    ActorEvent::Wake {
                        raw_buffered_cap_exhausted: false,
                    },
                    &mut driver,
                    &runtime,
                    Executor::now(&runtime),
                    false,
                )
                .await,
            TurnOutcome::ContinueBuffered
        );
        assert!(actor.raw_release_wait_until.is_some());

        // The completing tail is real decoded input. Because no fragment is
        // retained, this turn runs the due release and dispatches B directly.
        driver.buffered.store(false, Ordering::Release);
        let now = Executor::now(&runtime);
        assert_eq!(
            actor
                .handle_event(
                    ActorEvent::Receive {
                        result: batch(vec![raw_inquiry_reply(0xa1)]),
                        received_at: now,
                    },
                    &mut driver,
                    &runtime,
                    Executor::now(&runtime),
                    false,
                )
                .await,
            TurnOutcome::Continue
        );
        assert!(actor.raw_release_wait_until.is_none());
        assert_eq!(harness.writes.recv_async().await.unwrap(), _b);

        // B's zero-length response deadline creates its own timeout hold as
        // soon as that dispatch succeeds. C waits behind the new hold and must
        // receive a fresh grace budget rather than inheriting A's deadline.
        assert!(matches!(
            b_completion.recv_async().await.unwrap(),
            ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
        ));

        let (_c_completion, c_admitted) = handle.enqueue_admission(inquiry(), None).unwrap();
        let c_boundary = actor.admissions.try_recv().unwrap();
        actor
            .handle_admission(c_boundary, &mut driver, &runtime, Executor::now(&runtime))
            .await;
        let c = c_admitted.recv_async().await.unwrap().unwrap();
        assert!(harness.writes.try_recv().is_err());

        driver.buffered.store(true, Ordering::Release);
        runtime.advance(Duration::from_secs(1));
        assert_eq!(
            actor
                .handle_event(
                    ActorEvent::Wake {
                        raw_buffered_cap_exhausted: false,
                    },
                    &mut driver,
                    &runtime,
                    Executor::now(&runtime),
                    false,
                )
                .await,
            TurnOutcome::ContinueBuffered,
            "each independent hold receives its own grace deadline"
        );

        driver.buffered.store(false, Ordering::Release);
        let now = Executor::now(&runtime);
        assert_eq!(
            actor
                .handle_event(
                    ActorEvent::Receive {
                        result: batch(vec![raw_inquiry_reply(0xb2)]),
                        received_at: now,
                    },
                    &mut driver,
                    &runtime,
                    Executor::now(&runtime),
                    false,
                )
                .await,
            TurnOutcome::Continue
        );
        assert!(actor.raw_release_wait_until.is_none());
        assert_eq!(harness.writes.recv_async().await.unwrap(), c);
    }

    /// Production adapter/framer coverage for the original literal-byte hole:
    /// a raw reply split at the exact tombstone boundary remains attributed to
    /// the old interval, and only B's own later reply settles B.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_stream_literal_split_tail_precedes_tombstone_release() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let mut actor_policy = adapter.policy().clone();
        actor_policy.limits.frames_per_receive = 1;
        actor_policy.protocol.raw_inquiry_release_hold = Duration::from_secs(1);
        let (handle, actor) = AsyncOwnerActor::new(actor_policy, runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
        let _ = sent_rx.recv_async().await.unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &predecessor,
                "the production split-tail predecessor terminalizes",
            )
            .await,
            RuntimeOutcome::Failed(Error::Timeout)
        ));

        let successor = handle.submit(inquiry()).await.unwrap();
        assert!(sent_rx.try_recv().is_err());
        chunk_tx.send_async(vec![0x90, 0x50]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        runtime.advance(Duration::from_secs(1));
        chunk_tx.send_async(vec![0xa1, 0xff]).await.unwrap();

        let _ = sent_rx.recv_async().await.unwrap();
        let boundary_snapshot = handle.snapshot().await.unwrap();
        assert!(boundary_snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
        )));
        chunk_tx
            .send_async(vec![0x90, 0x50, 0xb2, 0xff])
            .await
            .unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &successor,
                "the production split-tail successor receives its own reply",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A byte-stream release gives an ownerless prefix one real grace interval,
    /// then discards it without poisoning. This runs the production adapter and
    /// framer for source-only, ACK, and socketless-completion prefixes (#713).
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_ambiguous_prefixes_expire_by_time_without_poison() {
        for prefix in [vec![0x90], vec![0x90, 0x41], vec![0x90, 0x50]] {
            let now = Instant::now();
            let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(now);
            let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
                .expect("generic raw profile");
            let (chunk_tx, chunks) = flume::bounded(16);
            let (sent, sent_rx) = flume::bounded(16);
            let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
                ChunkedStreamTransport {
                    config: crate::transport::builder::TransportConfig::default(),
                    chunks,
                    sent,
                },
                &profile,
                CameraId::CAMERA_1,
            )
            .unwrap();
            let mut actor_policy = adapter.policy().clone();
            actor_policy.limits.frames_per_receive = 1;
            actor_policy.protocol.raw_inquiry_release_hold = Duration::from_secs(1);
            let (handle, actor) = AsyncOwnerActor::new(actor_policy, runtime.clone()).unwrap();
            let actor_task = tokio::spawn(actor.run(adapter));

            let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
            let _ = sent_rx.recv_async().await.unwrap();
            assert!(matches!(
                terminal_within_test_deadline(
                    &predecessor,
                    "the ambiguous-prefix predecessor terminalizes",
                )
                .await,
                RuntimeOutcome::Failed(Error::Timeout)
            ));
            let successor = handle.submit(inquiry()).await.unwrap();
            assert!(sent_rx.try_recv().is_err());

            chunk_tx.send_async(prefix.clone()).await.unwrap();
            let _ = handle.snapshot().await.unwrap();
            runtime.advance(Duration::from_secs(1));
            // Wait until the engine's grace has actually been mapped onto an
            // executor sleep, then advance virtual time through that deadline.
            loop {
                if sleeps.recv_async().await.unwrap() == Duration::from_millis(100) {
                    break;
                }
            }
            runtime.advance(Duration::from_millis(100));
            let _sent_successor =
                tokio::time::timeout(Duration::from_secs(1), sent_rx.recv_async())
                    .await
                    .expect("the successor writes after grace expiry")
                    .unwrap();

            let snapshot = handle.snapshot().await.unwrap();
            assert_eq!(snapshot.state, SessionState::Running);
            assert!(snapshot.diagnostics.iter().any(|event| matches!(
                event,
                DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
            )));
            chunk_tx
                .send_async(vec![0x90, 0x50, 0xb2, 0xff])
                .await
                .unwrap();
            assert!(matches!(
                terminal_within_test_deadline(
                    &successor,
                    "the successor remains usable after orphan discard",
                )
                .await,
                RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
            ));
            handle.shutdown().await.unwrap();
            assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
        }
    }

    /// A source-only fragment cannot prove which same-target socket it belongs
    /// to. At Y/S2's ambiguity expiry it must therefore keep input first; when
    /// its exact S1 tail arrives at equality, X settles before Z is allowed to
    /// write. This is the target-mask deletion bug that motivated the typed
    /// release set: `[90]` followed by `[51 FF]` must never be erased merely
    /// because another socket on camera A releases.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_s1_split_tail_precedes_same_target_s2_release() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (handle, actor) =
            AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));
        let (x, _y, _z) =
            establish_production_two_socket_boundary(&handle, &chunk_tx, &sent_rx).await;

        enter_production_s2_quarantine(&handle, &runtime).await;
        chunk_tx.send_async(vec![0x90]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        runtime.advance(Duration::from_secs(1));
        // Receive is deliberately ready at the same manual instant as the due
        // release. The actor's fixed ordering must decode this tail before
        // `advance` can free Y/S2 and write Z.
        chunk_tx.send_async(vec![0x51, 0xff]).await.unwrap();

        assert!(matches!(
            terminal_within_test_deadline(
                &x,
                "the literal S1 tail completes X at the release boundary",
            )
            .await,
            RuntimeOutcome::Applied
        ));
        assert_eq!(
            sent_rx.recv_async().await.unwrap(),
            vec![0x81, 0x01, 0x04, 0x33, 0xff],
            "Z may write only after X's equal-boundary completion is correlated",
        );

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// An already-buffered named S1 prefix is exact evidence for live X, not
    /// stale evidence for Y/S2. The due pass may release Y while preserving
    /// `[90 51]`; its trailing terminator still completes X through the real
    /// `ProtocolFramer` after Z has been dispatched.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_live_s1_prefix_survives_same_target_s2_release() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (handle, actor) =
            AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));
        let (x, _y, _z) =
            establish_production_two_socket_boundary(&handle, &chunk_tx, &sent_rx).await;

        enter_production_s2_quarantine(&handle, &runtime).await;
        chunk_tx.send_async(vec![0x90, 0x51]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        runtime.advance(Duration::from_secs(1));
        let _ = handle.snapshot().await.unwrap();

        assert_eq!(
            sent_rx.recv_async().await.unwrap(),
            vec![0x81, 0x01, 0x04, 0x33, 0xff],
            "Y/S2 may release without deleting live X/S1 evidence",
        );
        chunk_tx.send_async(vec![0xff]).await.unwrap();
        assert!(matches!(
            terminal_within_test_deadline(&x, "the retained live S1 prefix completes X").await,
            RuntimeOutcome::Applied
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// Conversely, a named prefix for the releasing S2 is stale. It is
    /// discarded one raw fragment before Z writes; a later terminator must not
    /// turn the old fragment into a completion for either X or Z.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_stale_s2_prefix_is_discarded_before_successor_write() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (handle, actor) =
            AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));
        let (x, _y, _z) =
            establish_production_two_socket_boundary(&handle, &chunk_tx, &sent_rx).await;

        enter_production_s2_quarantine(&handle, &runtime).await;
        chunk_tx.send_async(vec![0x90, 0x52]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        runtime.advance(Duration::from_secs(1));
        let _ = handle.snapshot().await.unwrap();
        assert_eq!(
            sent_rx.recv_async().await.unwrap(),
            vec![0x81, 0x01, 0x04, 0x33, 0xff],
            "the exact stale S2 fragment is removed before successor dispatch",
        );

        // If the stale prefix survived, this byte would finish `[90 52 FF]`.
        // On its own it is malformed and ignored, so only X's explicit S1
        // completion below may settle X.
        chunk_tx.send_async(vec![0xff]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), x.terminal())
                .await
                .is_err(),
            "the stale S2 tail must not complete live X/S1",
        );
        chunk_tx.send_async(vec![0x90, 0x51, 0xff]).await.unwrap();
        assert!(matches!(
            terminal_within_test_deadline(&x, "X completes only after its explicit S1 response",)
                .await,
            RuntimeOutcome::Applied
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// Raw serial holds are target-local. Expiring A while camera C has a
    /// retained partial reply must preserve C's prefix; its later literal tail
    /// still resolves C before the globally single-flight inquiry lane admits
    /// A's queued successor.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_serial_a_release_preserves_c_partial_reply() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let config = crate::transport::builder::TransportConfig {
            addressing: crate::transport::builder::AddressingMode::Serial,
            ..crate::transport::builder::TransportConfig::default()
        };
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new_with_targets(
            ChunkedStreamTransport {
                config,
                chunks,
                sent,
            },
            &[
                (CameraId::CAMERA_1, &profile),
                (CameraId::CAMERA_3, &profile),
            ],
            crate::OperationalTuning::new(),
            std::num::NonZeroUsize::new(4).unwrap(),
            false,
        )
        .unwrap();
        let (handle, actor) =
            AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let predecessor = handle
            .submit(inquiry_for(CameraId::CAMERA_1))
            .await
            .unwrap();
        let _ = sent_rx.recv_async().await.unwrap();
        chunk_tx
            .send_async(vec![0x90, 0x50, 0xa1, 0xff])
            .await
            .unwrap();
        let _ = terminal_within_test_deadline(
            &predecessor,
            "the serial camera-A predecessor receives its reply",
        )
        .await;

        let camera_c = handle
            .submit(inquiry_for(CameraId::CAMERA_3))
            .await
            .unwrap();
        let _ = sent_rx.recv_async().await.unwrap();
        let successor = handle
            .submit(inquiry_for(CameraId::CAMERA_1))
            .await
            .unwrap();
        assert!(sent_rx.try_recv().is_err());

        chunk_tx.send_async(vec![0xb0, 0x50]).await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        runtime.advance(Duration::from_secs(1));
        // The control allowance wakes the actor at the new manual instant; the
        // following due turn sees C's target and leaves its prefix untouched.
        let _ = handle.snapshot().await.unwrap();
        chunk_tx.send_async(vec![0xc3, 0xff]).await.unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &camera_c,
                "camera C completes from its preserved partial reply",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xc3]
        ));

        let _ = sent_rx.recv_async().await.unwrap();
        chunk_tx
            .send_async(vec![0x90, 0x50, 0xb2, 0xff])
            .await
            .unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &successor,
                "camera A's queued successor receives its own reply",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A frame-limit batch can leave a complete camera-C reply ahead of an
    /// incomplete camera-A prefix. The async owner must decode C without due
    /// work, then keep the ambiguous socketless A prefix input-first until its
    /// tail is delimited before releasing A's successor. This covers hidden
    /// suffix ordering, the fail-closed socketless rule, and target-local raw
    /// serial framing through the production adapter and `ProtocolFramer`.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_serial_frame_limit_drains_c_before_discarding_a_prefix() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let config = crate::transport::builder::TransportConfig {
            addressing: crate::transport::builder::AddressingMode::Serial,
            buffer_config: crate::transport::buffer::BufferConfig {
                recv_buffer_size: 512,
                ..crate::transport::buffer::BufferConfig::default()
            },
            ..crate::transport::builder::TransportConfig::default()
        };
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new_with_targets(
            ChunkedStreamTransport {
                config,
                chunks,
                sent,
            },
            &[
                (CameraId::CAMERA_1, &profile),
                (CameraId::CAMERA_3, &profile),
            ],
            crate::OperationalTuning::new(),
            std::num::NonZeroUsize::new(4).unwrap(),
            false,
        )
        .unwrap();
        let frame_limit = adapter.policy().limits.frames_per_receive;
        let (handle, actor) =
            AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let predecessor = handle
            .submit(inquiry_for(CameraId::CAMERA_1))
            .await
            .unwrap();
        let _ = sent_rx.recv_async().await.unwrap();
        chunk_tx
            .send_async(vec![0x90, 0x50, 0xa1, 0xff])
            .await
            .unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &predecessor,
                "the frame-limit predecessor receives its reply",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xa1]
        ));

        let camera_c = handle
            .submit(inquiry_for(CameraId::CAMERA_3))
            .await
            .unwrap();
        let _ = sent_rx.recv_async().await.unwrap();
        let successor = handle
            .submit(inquiry_for(CameraId::CAMERA_1))
            .await
            .unwrap();
        assert!(sent_rx.try_recv().is_err());

        runtime.advance(Duration::from_secs(1));
        let mut burst = Vec::new();
        for _ in 0..frame_limit {
            burst.extend_from_slice(&[0xb0, 0x38, 0xff]);
        }
        // C's complete reply is the first retained item after the frame-limit
        // batch. A's socketless prefix sits behind it and must not reach B
        // until it becomes a complete, ordinary input frame.
        burst.extend_from_slice(&[0xb0, 0x50, 0xc3, 0xff]);
        burst.extend_from_slice(&[0x90, 0x50]);
        chunk_tx.send_async(burst).await.unwrap();
        // Queue the tail before yielding the actor: after it drains the
        // frame-limited batch and C's retained complete reply, receive-first
        // must consume this tail rather than spinning the already-due,
        // socketless prefix to the fail-closed cap.
        chunk_tx.send_async(vec![0xa1, 0xff]).await.unwrap();

        assert!(matches!(
            terminal_within_test_deadline(
                &camera_c,
                "camera C completes after the frame-limit batch",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xc3]
        ));
        // The complete old inquiry reply is consumed on the ordinary input
        // path before due work. It is inert under its terminal hold, after
        // which A's successor may write.
        let _ = sent_rx.recv_async().await.unwrap();
        chunk_tx
            .send_async(vec![0x90, 0x50, 0xb2, 0xff])
            .await
            .unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &successor,
                "camera A's successor completes after retained input drains",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A complete malformed stream frame retained behind the frame limit is
    /// still handled by #672's ordinary delimited-frame discard path. The raw
    /// release hook checks completeness before source attribution, so a forced
    /// boundary wake cannot turn this recoverable input into framing poison.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn production_raw_stream_retained_complete_malformed_frame_is_ignored() {
        let now = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let mut actor_policy = adapter.policy().clone();
        actor_policy.limits.frames_per_receive = 1;
        actor_policy.protocol.raw_inquiry_release_hold = Duration::from_secs(1);
        let (handle, actor) = AsyncOwnerActor::new(actor_policy, runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
        let _ = sent_rx.recv_async().await.unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &predecessor,
                "the retained-malformed predecessor terminalizes",
            )
            .await,
            RuntimeOutcome::Failed(Error::Timeout)
        ));
        let successor = handle.submit(inquiry()).await.unwrap();
        assert!(sent_rx.try_recv().is_err());

        runtime.advance(Duration::from_secs(1));
        chunk_tx
            .send_async(vec![
                0x90, 0x38, 0xff, // one valid frame fills the batch
                0x80, 0x50, 0xdd, 0xff, // complete but invalid response source
            ])
            .await
            .unwrap();

        let _ = sent_rx.recv_async().await.unwrap();
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running
        );
        chunk_tx
            .send_async(vec![0x90, 0x50, 0xb2, 0xff])
            .await
            .unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &successor,
                "the successor survives the retained malformed frame",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));

        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert!(snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
        )));
    }

    /// Regression test for a reply split across two stream reads (#560). A read
    /// that only advances a partial frame decodes to an empty batch, which must
    /// not be mistaken for a transport close.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn stream_reply_split_across_two_reads_keeps_the_session_running() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let (chunk_tx, chunks) = flume::bounded(8);
        let (sent, sent_rx) = flume::bounded(8);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        assert_eq!(adapter.policy().protocol.transport, TransportKind::Stream);
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let receipt = handle.submit(command()).await.unwrap();
        let _written = sent_rx.recv_async().await.unwrap();

        // Half of the ack. The framer buffers it and decodes nothing.
        chunk_tx.send_async(vec![0x90, 0x41]).await.unwrap();
        while !chunk_tx.is_empty() {
            tokio::task::yield_now().await;
        }
        // A snapshot only answers while the actor is still pumping; under the
        // #560 bug the actor has already terminated by this point.
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running,
            "a partial frame must not close the session"
        );

        // The rest of the ack, then the completion split the same way.
        chunk_tx.send_async(vec![0xff]).await.unwrap();
        chunk_tx.send_async(vec![0x90, 0x51]).await.unwrap();
        chunk_tx.send_async(vec![0xff]).await.unwrap();

        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));
        assert_eq!(handle.snapshot().await.unwrap().active, 0);
        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
    }

    /// A zero-length stream read is still the only close signal.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn zero_length_stream_read_still_closes_the_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let (chunk_tx, chunks) = flume::bounded(8);
        let (sent, sent_rx) = flume::bounded(8);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let receipt = handle.submit(command()).await.unwrap();
        let _written = sent_rx.recv_async().await.unwrap();
        chunk_tx.send_async(Vec::new()).await.unwrap();

        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Failed(Error::ConnectionClosed { .. })
        ));
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Closed);
    }

    /// A command whose retry policy allows one more attempt.
    #[cfg(feature = "runtime-tokio")]
    fn retrying_command() -> RuntimeRequest {
        let mut request = command();
        if let RuntimeRequest::Command { context, .. } = &mut request {
            context.retry = RetryPolicy {
                max_retries: 2,
                initial_backoff: Duration::ZERO,
                maximum_backoff: Duration::ZERO,
                total_budget: Duration::ZERO,
                ack_timeout: true,
                completion_timeout: true,
                inquiry_timeout: true,
                buffer_full: true,
                movement_not_executable: true,
                builtin_inquiry_syntax: true,
            };
        }
        request
    }

    /// Issue #565: a transient receive failure (the classic case is a UDP
    /// `recv` reporting ECONNREFUSED after an ICMP port-unreachable) retries
    /// the in-flight command and leaves the session running.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn transient_receive_fault_retries_and_keeps_the_session_running() {
        let runtime = TokioRuntime::from_current().unwrap();
        // A receive fault while a raw command awaits its socket is
        // intentionally session-poisoning: replay could duplicate actuation.
        // Sony's envelope gives the retry an exact same-sequence identity, so
        // this fixture exercises the supported recovery path.
        let (handle, actor) = AsyncOwnerActor::new(sony_policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let receipt = handle.submit(retrying_command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
            .await
            .unwrap();

        frames
            .send_async(Ok(AsyncReceive::Fault(Error::Io(Arc::new(
                std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
            )))))
            .await
            .unwrap();

        // The very same request is written again rather than failed.
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![sequenced(
                1,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            )]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![sequenced(
                1,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            )]))
            .await
            .unwrap();
        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));
        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
    }

    /// A receive failure that proves the connection is gone still ends the
    /// session, and does so as a close rather than a byte-stream poison.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn fatal_receive_fault_closes_the_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let receipt = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(Ok(AsyncReceive::Fault(Error::Io(Arc::new(
                std::io::Error::from(std::io::ErrorKind::BrokenPipe),
            )))))
            .await
            .unwrap();

        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Failed(Error::ConnectionClosed { .. })
        ));
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Closed);
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_actor_has_the_same_admit_write_terminal_order() {
        smol::block_on(async {
            let runtime = SmolRuntime::new();
            let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
            let harness = harness();
            let started = harness.started.clone();
            let gates = harness.gates.clone();
            let frames = harness.frames.clone();
            let task = smol::spawn(actor.run(harness.driver));

            let receipt = handle.submit(command()).await.unwrap();
            assert_eq!(started.recv_async().await.unwrap(), receipt.id);
            gates
                .send_async(Ok(TransmissionMeta { sequence: None }))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![ack(ViscaSocket::S1)]))
                .await
                .unwrap();
            frames
                .send_async(batch(vec![completion(ViscaSocket::S1)]))
                .await
                .unwrap();
            assert!(matches!(
                receipt.terminal().await.unwrap(),
                RuntimeOutcome::Applied
            ));
            handle.shutdown().await.unwrap();
            let snapshot = task.await;
            assert_eq!(snapshot.state, SessionState::Shutdown);
            assert_eq!(snapshot.active, 0);
        });
    }

    // ---------------------------------------------------------------------
    // #625: a repeatedly-failing transport must not livelock the actor.
    // ---------------------------------------------------------------------

    /// A driver whose read always fails immediately, so the receive branch of
    /// the event race is permanently ready. This is the shape a disconnected
    /// USB-serial adapter has: `EIO`/`ENXIO` surface as `ErrorKind::Other`,
    /// which classifies as a transient fault.
    #[derive(Debug)]
    struct AlwaysFailingReceive {
        reads: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl AsyncOwnerDriver for AlwaysFailingReceive {
        // The private driver trait requires an explicitly `Send` future.
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            self.reads.fetch_add(1, Ordering::Relaxed);
            async {
                Ok(AsyncReceive::Fault(Error::Io(Arc::new(
                    std::io::Error::other("simulated adapter unplugged"),
                ))))
            }
        }
    }

    /// Alternates a transient transport fault with a clean no-data receive.
    /// A no-data read paces the actor but is not a successful read, so it must
    /// not break the fault run that this driver deliberately keeps within the
    /// five-second recovery window.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct AlternatingFaultNoData {
        reads: Arc<std::sync::atomic::AtomicUsize>,
        faults: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for AlternatingFaultNoData {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let reads = Arc::clone(&self.reads);
            let faults = Arc::clone(&self.faults);
            async move {
                if reads.fetch_add(1, Ordering::Relaxed).is_multiple_of(2) {
                    faults.fetch_add(1, Ordering::Relaxed);
                    Ok(AsyncReceive::Fault(Error::TransportError(
                        "simulated intermittent adapter fault".into(),
                    )))
                } else {
                    Ok(AsyncReceive::NoData)
                }
            }
        }
    }

    /// Issue #625. A transport that fails every read keeps the receive branch
    /// of the left-biased race permanently ready. Before the fix that starved
    /// shutdown, admissions, cancellations and control forever: `submit` never
    /// returned, `shutdown` only queued a message nobody read, and the actor
    /// kept reading hundreds of times a second with the session unkillable.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_transport_failing_every_read_still_serves_the_boundary() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let actor_task = tokio::spawn(actor.run(AlwaysFailingReceive {
            reads: Arc::clone(&reads),
        }));

        // Admission is polled even though the transport is always ready.
        // A raw command awaiting ACK would, by default, ride to its ACK
        // deadline and quarantine per-request (issue #671). Use an inquiry so
        // this test isolates source arbitration and boundary liveness without
        // creating an unconfirmed-command outcome.
        let receipt = tokio::time::timeout(Duration::from_secs(5), handle.submit(inquiry()))
            .await
            .expect("admission must not be starved by a failing transport")
            .unwrap();
        drop(receipt);
        // So is ordinary control.
        let metrics = tokio::time::timeout(Duration::from_secs(5), handle.metrics())
            .await
            .expect("control must not be starved by a failing transport")
            .unwrap();
        assert_eq!(
            metrics.session,
            crate::observability::SessionStatus::Running
        );

        // And so is shutdown: the session is killable.
        tokio::time::timeout(Duration::from_secs(5), handle.shutdown())
            .await
            .expect("shutdown must not be starved by a failing transport")
            .unwrap();
        let snapshot = tokio::time::timeout(Duration::from_secs(5), actor_task)
            .await
            .expect("the actor task must tear down within a bound")
            .unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert!(
            reads.load(Ordering::Relaxed) < 500,
            "the escalating pause must stop the actor hot-looping on a failing read"
        );
    }

    /// A task that disappears before `run` can publish its terminal result is
    /// not an orderly runtime shutdown. Every handle-facing wait must fail
    /// closed instead of manufacturing `RuntimeShutdown` or success.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn actor_disconnect_without_terminal_result_fails_closed() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(PanickingReceiveDriver));

        let error = tokio::time::timeout(Duration::from_secs(1), handle.wait_closed())
            .await
            .expect("liveness must observe the panicking actor")
            .unwrap_err();
        assert!(matches!(
            error,
            Error::InvalidState(message)
                if message.contains("without publishing a terminal result")
        ));
        assert!(actor_task.await.is_err(), "the test driver must panic");

        let error = handle.shutdown().await.unwrap_err();
        assert!(matches!(
            error,
            Error::InvalidState(message)
                if message.contains("without publishing a terminal result")
        ));
    }

    /// A boundary admission queued before an actor panic is answered by the
    /// same fail-closed terminal result observed by later handle calls, and
    /// its permit is returned. This is the bounded ownership guarantee
    /// (#542 §4; architecture §Operational invariants) without relying on a
    /// scheduler sleep to arrange the panic/admission order.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn queued_admission_actor_disconnect_fails_closed_and_releases_capacity() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();

        let queued = handle.try_submit(inquiry()).unwrap();
        assert_eq!(handle.permits.available(), 0);

        // The receive-first actor turn panics before it can consume the
        // already-buffered admission. Awaiting the task makes Drop's final
        // boundary drain a deterministic happens-before edge for the checks
        // below; no timing or polling sleep is involved.
        let actor_task = tokio::spawn(actor.run(PanickingReceiveDriver));
        assert!(actor_task.await.is_err(), "the test driver must panic");

        let queued_error = queued.await.unwrap_err();
        assert!(matches!(
            &queued_error,
            Error::InvalidState(message)
                if message.contains("without publishing a terminal result")
        ));
        let published = handle.shutdown().await.unwrap_err();
        assert_eq!(queued_error.to_string(), published.to_string());
        assert_eq!(
            handle.permits.available(),
            handle.permits.capacity(),
            "the drained admission must return its permit"
        );
    }

    /// An admitted request whose actor disappeared must fail closed promptly,
    /// rather than wait for its long protocol deadline or report an orderly
    /// `RuntimeShutdown`.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn active_receipt_actor_disconnect_fails_closed() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let (panic_signal, panic_signal_rx) = flume::bounded(1);
        let actor_task = tokio::spawn(actor.run(PanickingAfterAdmissionDriver {
            panic_signal: panic_signal_rx,
        }));

        let receipt = tokio::time::timeout(Duration::from_secs(1), handle.submit(inquiry()))
            .await
            .expect("admission must complete before the actor panic")
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 1);
        panic_signal.send_async(()).await.unwrap();

        let error = tokio::time::timeout(
            Duration::from_secs(1),
            wait_core_for(receipt, handle.receipt_control(), Duration::from_secs(5)),
        )
        .await
        .expect("receipt wait must observe the actor disappearance")
        .unwrap_err();
        assert!(matches!(
            error,
            Error::InvalidState(message)
                if message.contains("without publishing a terminal result")
        ));
        assert!(actor_task.await.is_err(), "the test driver must panic");
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn active_cancellation_actor_disconnect_fails_closed() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let (panic_signal, panic_signal_rx) = flume::bounded(1);
        let actor_task = tokio::spawn(actor.run(PanickingAfterAdmissionDriver {
            panic_signal: panic_signal_rx,
        }));

        let receipt = tokio::time::timeout(Duration::from_secs(1), handle.submit(command()))
            .await
            .expect("admission must complete before the actor panic")
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 1);
        let cancellation = handle.cancel_test(receipt).await.unwrap();
        panic_signal.send_async(()).await.unwrap();

        let error = tokio::time::timeout(
            Duration::from_secs(1),
            cancellation.outcome(handle.receipt_control(), Duration::from_secs(5)),
        )
        .await
        .expect("cancellation wait must observe the actor disappearance")
        .unwrap_err();
        assert!(matches!(
            error,
            Error::InvalidState(message)
                if message.contains("without publishing a terminal result")
        ));
        assert!(actor_task.await.is_err(), "the test driver must panic");
    }

    /// The same guarantee on the other executor: the actor is executor-generic
    /// and the fairness fix lives in its event race, not in tokio.
    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_transport_failing_every_read_still_serves_the_boundary() {
        smol::block_on(async {
            let runtime = SmolRuntime::new();
            let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
            let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let task = smol::spawn(actor.run(AlwaysFailingReceive {
                reads: Arc::clone(&reads),
            }));

            // Keep the fixture focused on boundary progress. A raw command
            // awaiting ACK would be poisoned by the first receive fault under
            // the no-replay rule; an inquiry has no ambiguous actuation.
            let receipt = handle.submit(inquiry()).await.unwrap();
            drop(receipt);
            assert_eq!(
                handle.snapshot().await.unwrap().state,
                SessionState::Running
            );
            handle.shutdown().await.unwrap();
            assert_eq!(task.await.state, SessionState::Shutdown);
        });
    }

    /// Issue #625. A transport that never recovers is not transient. The run
    /// escalates its pause and then ends the session with the underlying cause,
    /// rather than retrying against a dead adapter for the process lifetime.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_permanently_failing_transport_eventually_ends_the_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let snapshot = tokio::time::timeout(
            Duration::from_secs(30),
            actor.run(AlwaysFailingReceive {
                reads: Arc::clone(&reads),
            }),
        )
        .await
        .expect("a permanently failing transport must terminate the session");

        assert_eq!(snapshot.state, SessionState::Closed);
        assert!(
            reads.load(Ordering::Relaxed)
                >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
            "the session must survive a genuinely transient burst first"
        );
        let error = handle.metrics().await.unwrap_err();
        let Error::ConnectionClosed {
            reason: Some(reason),
        } = error
        else {
            panic!("the terminal error must name the transport cause: {error:?}");
        };
        assert!(
            reason.contains("consecutive receive faults"),
            "the reason must say why the fault run stopped counting as transient: {reason}"
        );
    }

    /// A clean no-data receive is neither a transport fault nor proof that an
    /// earlier transient fault recovered. Alternating the two inside the reset
    /// window must therefore still reach the permanent-fault threshold.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(start_paused = true)]
    async fn alternating_fault_and_no_data_receives_still_end_the_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let faults = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let snapshot = tokio::time::timeout(
            Duration::from_secs(10),
            actor.run(AlternatingFaultNoData {
                reads: Arc::clone(&reads),
                faults: Arc::clone(&faults),
            }),
        )
        .await
        .expect("alternating no-data must not reset a transient fault run");

        assert_eq!(snapshot.state, SessionState::Closed);
        assert!(
            faults.load(Ordering::Relaxed)
                >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
            "the full fault threshold must survive intervening no-data receives"
        );
        assert!(
            reads.load(Ordering::Relaxed)
                >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT.saturating_mul(2) - 1).unwrap(),
            "the scripted driver must actually alternate faults with no-data reads"
        );
        let error = handle.shutdown().await.unwrap_err();
        assert!(matches!(error, Error::ConnectionClosed { .. }));
    }

    /// Issue #625. Genuinely transient faults still behave exactly as #620
    /// specified: every command still awaiting its ACK is retransmitted, the
    /// session survives, and a successful read clears the run so the next burst
    /// starts from zero.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_burst_of_transient_faults_then_recovery_keeps_the_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        // Same-sequence Sony retries are safe after a receive fault. A raw
        // command in this phase must poison the session and is covered by the
        // engine tests; this actor fixture must not weaken that rule.
        let (handle, actor) = AsyncOwnerActor::new(sony_policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let receipt = handle.submit(retrying_command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
            .await
            .unwrap();

        // Two consecutive faults, each retransmitting the very same request.
        for _ in 0..2 {
            frames
                .send_async(Ok(AsyncReceive::Fault(Error::TransportError(
                    "ICMP port unreachable".into(),
                ))))
                .await
                .unwrap();
            assert_eq!(started.recv_async().await.unwrap(), receipt.id);
            gates
                .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
                .await
                .unwrap();
        }

        frames
            .send_async(batch(vec![sequenced(
                1,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            )]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![sequenced(
                1,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            )]))
            .await
            .unwrap();
        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));

        // The successful read cleared the run, so more faults are still
        // transient and the session is still answering.
        for _ in 0..3 {
            frames
                .send_async(Ok(AsyncReceive::Fault(Error::TransportError(
                    "ICMP port unreachable".into(),
                ))))
                .await
                .unwrap();
        }
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running
        );
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// Issue #625/#719. An application idle timeout is no data, not a fault.
    /// A transport with an internal idle timer — the shape the public trait
    /// documents — must not retransmit anything, and must not starve the
    /// boundary either. A raw I/O `TimedOut` is intentionally excluded: it can
    /// be the OS reporting keepalive exhaustion and therefore ends the session.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn an_idle_read_timeout_is_not_a_receive_fault() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let writes = Arc::clone(&harness.writes);
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let receipt = handle.submit(retrying_command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();

        for timeout in [
            Error::Timeout,
            Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::WouldBlock,
            ))),
            Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::Interrupted,
            ))),
        ] {
            frames
                .send_async(Ok(AsyncReceive::Fault(timeout)))
                .await
                .unwrap();
        }
        frames.send_async(Ok(AsyncReceive::NoData)).await.unwrap();

        // Control still answers, and nothing was retransmitted.
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running
        );
        assert_eq!(
            writes.lock().unwrap().len(),
            1,
            "an idle read timeout must not spend a request's retry budget"
        );

        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// The escalation is bounded, monotonic, and long enough that a run only
    /// ends the session after seconds of uninterrupted failure.
    #[test]
    fn the_transient_pause_escalates_and_stays_bounded() {
        assert_eq!(transient_receive_pause(1), TRANSIENT_RECEIVE_PAUSE);
        assert_eq!(transient_receive_pause(2), Duration::from_millis(20));
        assert_eq!(transient_receive_pause(3), Duration::from_millis(40));
        let mut previous = Duration::ZERO;
        let mut total = Duration::ZERO;
        for run in 1..TRANSIENT_RECEIVE_FAULT_LIMIT {
            let pause = transient_receive_pause(run);
            assert!(pause >= previous, "the pause must never shrink");
            assert!(pause <= MAXIMUM_TRANSIENT_RECEIVE_PAUSE);
            previous = pause;
            total = total.saturating_add(pause);
        }
        assert!(
            total >= TRANSIENT_RECEIVE_FAULT_SPAN,
            "a full fault run must span at least the documented minimum: {total:?}"
        );
    }

    /// A run only counts consecutive failures; a gap proves recovery, and a
    /// successful read resets it outright.
    #[test]
    fn a_fault_run_only_counts_consecutive_failures() {
        let start = Instant::now();
        let mut run = TransientFaultRun::default();
        assert_eq!(run.record(start).0, 1);
        assert_eq!(run.record(start + Duration::from_millis(10)).0, 2);
        run.reset();
        assert_eq!(run.record(start + Duration::from_millis(20)).0, 1);
        // A gap longer than the reset window starts a fresh run.
        assert_eq!(
            run.record(start + Duration::from_millis(20) + TRANSIENT_RECEIVE_FAULT_RESET)
                .0,
            1
        );

        // The limit alone is not enough; the run must also be old enough.
        assert!(!TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT,
            Duration::ZERO
        ));
        assert!(!TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT - 1,
            TRANSIENT_RECEIVE_FAULT_SPAN
        ));
        assert!(TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT,
            TRANSIENT_RECEIVE_FAULT_SPAN
        ));
    }

    /// Issue #625. The blocking owner clamps its transient pause to the
    /// caller's deadline; the async owner clamps to the next scheduler wake, so
    /// a fault can never delay a due deadline by the length of the pause.
    #[test]
    fn the_transient_pause_never_outlives_the_next_wake() {
        let now = Instant::now();
        let pause = Duration::from_millis(250);
        assert_eq!(clamp_receive_pause(pause, None, now), pause);
        assert_eq!(
            clamp_receive_pause(pause, Some(now + Duration::from_secs(1)), now),
            pause
        );
        assert_eq!(
            clamp_receive_pause(pause, Some(now + Duration::from_millis(3)), now),
            Duration::from_millis(3)
        );
        assert_eq!(
            clamp_receive_pause(pause, Some(now - Duration::from_millis(5)), now),
            Duration::ZERO,
            "an overdue deadline is serviced immediately"
        );
    }

    // ---------------------------------------------------------------------
    // #626: a boundary request racing teardown must never hang.
    // ---------------------------------------------------------------------

    /// Driver whose writes complete immediately and whose reads come from one
    /// channel the test controls. Writes never stall, so the actor is only ever
    /// waiting on its own event race.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct UngatedDriver {
        receives: flume::Receiver<Result<AsyncReceive, Error>>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AsyncOwnerDriver for UngatedDriver {
        // The private driver trait requires an explicitly `Send` future.
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let receives = self.receives.clone();
            async move {
                // An exhausted script parks instead of reporting a close, so
                // the test decides exactly when the transport ends.
                match receives.recv_async().await {
                    Ok(next) => next,
                    Err(_) => future::pending().await,
                }
            }
        }
    }

    /// Issue #626. `run` drains the boundary lanes once and then drops its
    /// receivers. A message that lands in between used to be stranded forever:
    /// this handle's own sender keeps flume's queue alive, and with it the
    /// reply sender inside the stranded message, so the caller's wait never
    /// disconnected. The losing caller is typically the one that just watched
    /// the session die and immediately asked a follow-up question.
    ///
    /// The window is only reachable when the caller runs on another thread, so
    /// this drives a multi-threaded runtime and repeats enough to hit it.
    ///
    /// The driver is deliberately not `harness()`'s. That one parks in `write`
    /// until the test releases a gate, which is exactly what several ordering
    /// tests need and exactly wrong here: a command admitted before the close
    /// is read would park the actor mid-transmission, and every later question
    /// would then hang on an actor that is stalled rather than racing its own
    /// teardown. That is a property of the fake transport, not of the boundary,
    /// and it is not what this probe is for.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_boundary_request_racing_teardown_never_hangs() {
        for iteration in 0..64 {
            let runtime = TokioRuntime::from_current().unwrap();
            let (handle, actor) = AsyncOwnerActor::new(policy(8), runtime).unwrap();
            let (frames, receives) = flume::bounded(1);
            let actor_task = tokio::spawn(actor.run(UngatedDriver { receives }));

            let asking = handle.clone();
            let questions = tokio::spawn(async move {
                // Keep asking until the session answers with its terminal
                // error. Under the bug one of these calls parks forever.
                loop {
                    if asking.metrics().await.is_err() {
                        break;
                    }
                    if asking.snapshot().await.is_err() {
                        break;
                    }
                    match asking.submit(command()).await {
                        Ok(receipt) => drop(receipt),
                        // Nothing acknowledges these commands, so the lane
                        // fills up and stays full. Capacity is a "ask again"
                        // answer, not a terminal one: the probe is only over
                        // when the session itself answers.
                        Err(Error::RuntimeQueueFull { .. }) => {}
                        Err(_) => break,
                    }
                }
            });

            // Let the questioner get in flight first, so the close lands while
            // boundary work is actually moving. The exact interleaving is left
            // to the scheduler; over this many iterations both orders occur.
            tokio::task::yield_now().await;
            frames.send_async(Ok(AsyncReceive::Closed)).await.unwrap();

            // The bug this guards is an unbounded park, so the bound only has
            // to be longer than a healthy teardown ever takes. It is generous
            // because CI runners are small, not because the answer is slow.
            tokio::time::timeout(Duration::from_secs(30), questions)
                .await
                .unwrap_or_else(|_| {
                    panic!("iteration {iteration}: a boundary request outlived the actor")
                })
                .unwrap();
            let snapshot = tokio::time::timeout(Duration::from_secs(30), actor_task)
                .await
                .unwrap_or_else(|_| panic!("iteration {iteration}: the actor never finished"))
                .unwrap();
            assert_eq!(snapshot.state, SessionState::Closed);
        }
    }

    /// The drain still answers what it can see: a cancellation queued before
    /// teardown keeps its buffered terminal observation rather than being
    /// replaced by the session error.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn teardown_answers_queued_work_before_the_liveness_lane_fires() {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let frames = harness.frames.clone();
        let control_handle = handle.clone();
        let queued = tokio::spawn(async move { control_handle.metrics().await });
        while handle.control.is_empty() {
            tokio::task::yield_now().await;
        }
        frames.send_async(Ok(AsyncReceive::Closed)).await.unwrap();
        let snapshot = actor.run(harness.driver).await;
        assert_eq!(snapshot.state, SessionState::Closed);
        assert!(matches!(
            queued.await.unwrap().unwrap_err(),
            Error::ConnectionClosed { .. }
        ));
    }

    // ---------------------------------------------------------------------
    // #637: the owners agree on a malformed datagram.
    // ---------------------------------------------------------------------

    /// A datagram transport whose reads are handed over one datagram at a time.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct ScriptedDatagramTransport {
        config: crate::transport::builder::TransportConfig,
        datagrams: flume::Receiver<Vec<u8>>,
        sent: flume::Sender<Vec<u8>>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::HasTransportConfig for ScriptedDatagramTransport {
        fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
            &self.config
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::AsyncTransport for ScriptedDatagramTransport {
        async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
            self.sent
                .send_async(bytes.to_vec())
                .await
                .map_err(|_| Error::RuntimeShutdown)
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            let Ok(datagram) = self.datagrams.recv_async().await else {
                return future::pending().await;
            };
            let len = datagram.len().min(dst.len());
            dst[..len].copy_from_slice(&datagram[..len]);
            Ok(len)
        }

        fn send_semantics(&self) -> crate::transport::SendSemantics {
            crate::transport::SendSemantics::Datagram
        }
    }

    /// Production-adapter fixture that alternates a failed read with an
    /// oversized datagram whose copied prefix happens to be a valid ACK. The
    /// adapter must classify the latter as a consumed bad datagram, not a
    /// transient transport fault.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct AlternatingFaultAndTruncatedDatagrams {
        config: crate::transport::builder::TransportConfig,
        reads: Arc<std::sync::atomic::AtomicUsize>,
        faults: Arc<std::sync::atomic::AtomicUsize>,
        truncated: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(feature = "runtime-tokio")]
    impl AlternatingFaultAndTruncatedDatagrams {
        fn next_receive(&self, dst: &mut [u8]) -> Result<crate::transport::ReceiveOutcome, Error> {
            if self.reads.fetch_add(1, Ordering::Relaxed).is_multiple_of(2) {
                self.faults.fetch_add(1, Ordering::Relaxed);
                return Err(Error::TransportError(
                    "simulated intermittent datagram adapter fault".into(),
                ));
            }
            let prefix = [0x90, 0x41, 0xff];
            dst[..prefix.len()].copy_from_slice(&prefix);
            self.truncated.fetch_add(1, Ordering::Relaxed);
            Ok(crate::transport::ReceiveOutcome::Truncated {
                copied: prefix.len(),
            })
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::HasTransportConfig for AlternatingFaultAndTruncatedDatagrams {
        fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
            &self.config
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::AsyncTransport for AlternatingFaultAndTruncatedDatagrams {
        async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
            Ok(())
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            self.next_receive(dst).map(|outcome| outcome.copied_len())
        }

        async fn recv_into_with_outcome(
            &mut self,
            dst: &mut [u8],
        ) -> Result<crate::transport::ReceiveOutcome, Error> {
            self.next_receive(dst)
        }

        fn send_semantics(&self) -> crate::transport::SendSemantics {
            crate::transport::SendSemantics::Datagram
        }
    }

    /// A discarded, consumed UDP datagram is successful transport activity,
    /// even though its VISCA content is malformed. Alternating it with enough
    /// transient read faults to otherwise cross the permanent-fault threshold
    /// must leave the production adapter's owner Running.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(start_paused = true)]
    async fn consumed_truncated_datagrams_reset_the_async_fault_run() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let faults = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let truncated = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let config = crate::transport::builder::TransportConfig {
            buffer_config: crate::transport::buffer::BufferConfig {
                recv_buffer_size: 3,
                ..crate::transport::buffer::BufferConfig::default()
            },
            ..crate::transport::builder::TransportConfig::default()
        };
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            AlternatingFaultAndTruncatedDatagrams {
                config,
                reads,
                faults: Arc::clone(&faults),
                truncated: Arc::clone(&truncated),
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        assert_eq!(adapter.policy().protocol.transport, TransportKind::Datagram);
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));
        let started_at = tokio::time::Instant::now();

        // Advancing in steps lets each transient-fault pause complete before
        // the next consumed oversized datagram, so the old accounting would
        // accumulate twelve faults over more than the one-second terminal span.
        for _ in 0..20 {
            tokio::time::advance(Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
            if faults.load(Ordering::Relaxed)
                >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap()
                && truncated.load(Ordering::Relaxed)
                    >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap()
            {
                break;
            }
        }
        assert!(
            tokio::time::Instant::now().duration_since(started_at) >= Duration::from_secs(1),
            "the regression must cross the permanent-fault time threshold"
        );
        assert!(
            faults.load(Ordering::Relaxed)
                >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
            "the fixture must issue the whole transient-fault threshold"
        );
        assert!(
            truncated.load(Ordering::Relaxed)
                >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
            "every intervening oversized datagram must be consumed and discarded"
        );
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running,
            "a consumed malformed datagram resets the fault run rather than closing the session"
        );

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// Issue #637. One malformed datagram — the review's probe is `01 41 ff`,
    /// a controller source byte that no camera ever sends — used to kill the
    /// whole async session, while the blocking owner failed it per request and
    /// kept pumping. A stray UDP datagram is not proof the session is dead.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_malformed_datagram_does_not_kill_the_async_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let (datagram_tx, datagrams) = flume::bounded(8);
        let (sent, sent_rx) = flume::bounded(8);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ScriptedDatagramTransport {
                config: crate::transport::builder::TransportConfig::default(),
                datagrams,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        assert_eq!(adapter.policy().protocol.transport, TransportKind::Datagram);
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let receipt = handle.submit(command()).await.unwrap();
        let _written = sent_rx.recv_async().await.unwrap();
        datagram_tx
            .send_async(vec![0x01, 0x41, 0xff])
            .await
            .unwrap();
        while !datagram_tx.is_empty() {
            tokio::task::yield_now().await;
        }

        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running,
            "one undecodable datagram is not a session verdict"
        );

        // The next well-formed datagram still completes the in-flight command.
        datagram_tx
            .send_async(vec![0x90, 0x41, 0xff])
            .await
            .unwrap();
        datagram_tx
            .send_async(vec![0x90, 0x51, 0xff])
            .await
            .unwrap();
        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));

        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert!(
            snapshot.diagnostics.iter().any(|event| matches!(
                event,
                DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
            )),
            "the discarded datagram must still be observable"
        );
    }

    /// #672: a delimited-but-unclassifiable frame on a byte stream is a
    /// malformed frame to discard, not a lost framing position. The framer kept
    /// its place, so the stream stays Running, the frame is recorded as
    /// `Ignored(MalformedFrame)`, and the in-flight command is settled by the
    /// next well-formed reply — the log-and-continue tolerance 1.x had. A genuine
    /// framing failure (buffer overflow / no boundary) still poisons and is
    /// pinned separately.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_malformed_stream_frame_is_discarded_and_keeps_the_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let (chunk_tx, chunks) = flume::bounded(8);
        let (sent, sent_rx) = flume::bounded(8);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        assert_eq!(adapter.policy().protocol.transport, TransportKind::Stream);
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let receipt = handle.submit(command()).await.unwrap();
        let _written = sent_rx.recv_async().await.unwrap();
        // A padded ACK: delimited at its `FF`, but four bytes where an ACK is
        // exactly three, so it does not classify.
        chunk_tx
            .send_async(vec![0x90, 0x41, 0x00, 0xff])
            .await
            .unwrap();
        while !chunk_tx.is_empty() {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running,
            "one malformed stream frame is not a session verdict"
        );

        // The next well-formed ACK and completion still settle the command.
        chunk_tx.send_async(vec![0x90, 0x41, 0xff]).await.unwrap();
        chunk_tx.send_async(vec![0x90, 0x51, 0xff]).await.unwrap();
        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));

        handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert!(
            snapshot.diagnostics.iter().any(|event| matches!(
                event,
                DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
            )),
            "the discarded malformed stream frame must still be observable"
        );
    }

    /// #674: a single stream read that decodes more than the per-receive frame
    /// limit (default 64) must not poison the session. The read stops at the
    /// limit and the async owner drains the buffered remainder on the next turn
    /// (protocol input first), so all frames are attributed and the command is
    /// settled. Sixty-three harmless network-change notices sit ahead of the ACK
    /// and completion, so the completion falls past the limit and is only
    /// reachable through the drain path.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_stream_burst_over_the_frame_limit_keeps_the_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let (chunk_tx, chunks) = flume::bounded(8);
        let (sent, sent_rx) = flume::bounded(8);
        // A large receive buffer so one read can carry a burst past the frame
        // limit, as a real 256-byte raw-IP session would.
        let mut config = crate::transport::builder::TransportConfig::default();
        config.buffer_config.recv_buffer_size = 1024;
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config,
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        assert_eq!(adapter.policy().protocol.transport, TransportKind::Stream);
        let frame_limit = adapter.policy().limits.frames_per_receive;
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let receipt = handle.submit(command()).await.unwrap();
        let _written = sent_rx.recv_async().await.unwrap();
        // One read: (frame_limit - 1) network-change notices, then the ACK and
        // completion. The completion is the (frame_limit + 1)-th frame, so it is
        // only reached after the limit-saturated read is drained.
        let mut burst = Vec::new();
        for _ in 0..frame_limit.saturating_sub(1) {
            burst.extend_from_slice(&[0x90, 0x38, 0xff]);
        }
        burst.extend_from_slice(&[0x90, 0x41, 0xff]);
        burst.extend_from_slice(&[0x90, 0x51, 0xff]);
        chunk_tx.send_async(burst).await.unwrap();

        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running,
            "a large-but-valid burst is not a session verdict"
        );

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// #681: a single-target IP session whose camera answers with its non-default
    /// chain address (`0xA0` for VISCA address 2) still attributes the reply to
    /// the sole outstanding command, rather than poisoning the stream.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_single_target_ip_chain_address_reply_settles_the_command() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let (chunk_tx, chunks) = flume::bounded(8);
        let (sent, sent_rx) = flume::bounded(8);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let receipt = handle.submit(command()).await.unwrap();
        let _written = sent_rx.recv_async().await.unwrap();
        // The camera answers with chain address 2 for both frames.
        chunk_tx.send_async(vec![0xa0, 0x41, 0xff]).await.unwrap();
        chunk_tx.send_async(vec![0xa0, 0x51, 0xff]).await.unwrap();

        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    /// A datagram transport whose every send reports the connection as closed.
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct ClosedSendDatagramTransport {
        config: crate::transport::builder::TransportConfig,
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::HasTransportConfig for ClosedSendDatagramTransport {
        fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
            &self.config
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl crate::transport::AsyncTransport for ClosedSendDatagramTransport {
        async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
            Err(Error::ConnectionClosed {
                reason: Some("socket closed".into()),
            })
        }

        async fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
            future::pending().await
        }

        fn send_semantics(&self) -> crate::transport::SendSemantics {
            crate::transport::SendSemantics::Datagram
        }
    }

    /// Issue #637. A datagram send failure fails exactly one request and the
    /// session keeps running, so the error the caller observes must not tell it
    /// to open a replacement session. Before the fix a custom transport reached
    /// that self-contradiction just by returning `ConnectionClosed` from `send`.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn a_datagram_send_failure_never_demands_a_new_session() {
        let runtime = TokioRuntime::from_current().unwrap();
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("the generic profile is valid");
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ClosedSendDatagramTransport {
                config: crate::transport::builder::TransportConfig::default(),
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        assert_eq!(adapter.policy().protocol.transport, TransportKind::Datagram);
        let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let receipt = handle.submit(command()).await.unwrap();
        let RuntimeOutcome::Failed(error) = receipt.terminal().await.unwrap() else {
            panic!("a datagram send failure fails its own request");
        };
        assert!(
            !error.requires_new_session(),
            "a live session must never hand out a replacement-session verdict: {error:?}"
        );
        assert!(matches!(error, Error::TransportError(_)));
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running,
            "a datagram send failure is per request, not a session verdict"
        );

        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }

    // ---- #675: async arbitration liveness ----
    //
    // These run the actor on a spawned task (a real worker thread on the
    // multi-thread tokio runtime and on smol's global executor) so an adversarial
    // transport that never yields the CPU cannot wedge the single test thread —
    // a regression fails the bounded wait cleanly instead of hanging the suite.

    /// A peer that produces a valid frame on every poll. `receive` returns
    /// immediately, so without the fairness ceiling it wins the left-biased
    /// receive-first selection forever and starves every boundary source.
    #[derive(Debug, Default)]
    struct BabblingDriver;

    impl AsyncOwnerDriver for BabblingDriver {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        #[allow(clippy::manual_async_fn)]
        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            async {
                Ok(AsyncReceive::Frames(vec![DecodedFrame {
                    target: CameraId::CAMERA_1,
                    sequence: None,
                    response: DecodedResponse::Unknown,
                }]))
            }
        }
    }

    /// A babbling peer with an external test-only escape hatch. The watchdog
    /// uses `stop` only after declaring the single-thread liveness check
    /// failed, so a regressed actor can be released rather than wedging the
    /// whole test process.
    #[derive(Debug)]
    struct CountingBabblingDriver {
        reads: Arc<std::sync::atomic::AtomicU64>,
        stop: Arc<std::sync::atomic::AtomicBool>,
        /// `true` models the empty batch the stream adapter returns after it
        /// discards one or more delimited malformed frames.
        empty_batches: bool,
    }

    impl AsyncOwnerDriver for CountingBabblingDriver {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let reads = Arc::clone(&self.reads);
            let stop = Arc::clone(&self.stop);
            let empty_batches = self.empty_batches;
            async move {
                reads.fetch_add(1, Ordering::Relaxed);
                if stop.load(Ordering::Acquire) {
                    Ok(AsyncReceive::Closed)
                } else if empty_batches {
                    Ok(AsyncReceive::Frames(Vec::new()))
                } else {
                    Ok(AsyncReceive::Frames(vec![DecodedFrame {
                        target: CameraId::CAMERA_1,
                        sequence: None,
                        response: DecodedResponse::Unknown,
                    }]))
                }
            }
        }
    }

    /// A peer that accepts the write but never completes it, and never delivers a
    /// read. Without a write timeout the actor parks in the write and `close()`
    /// never returns.
    #[derive(Debug, Default)]
    struct StallingWriteDriver;

    impl AsyncOwnerDriver for StallingWriteDriver {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { future::pending().await }
        }

        #[allow(clippy::manual_async_fn)]
        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            async { future::pending().await }
        }
    }

    /// A peer that reports "no data" on every poll, immediately. Without the
    /// idle-read pace this spins the actor at hundreds of thousands of reads a
    /// second.
    #[derive(Debug)]
    struct NoDataDriver {
        reads: Arc<std::sync::atomic::AtomicU64>,
    }

    impl AsyncOwnerDriver for NoDataDriver {
        #[allow(clippy::manual_async_fn)]
        fn write(
            &mut self,
            _write: WireWrite<'_>,
        ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
            async { Ok(TransmissionMeta { sequence: None }) }
        }

        fn receive(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _frame_limit: usize,
        ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
            let reads = Arc::clone(&self.reads);
            async move {
                reads.fetch_add(1, Ordering::Relaxed);
                Ok(AsyncReceive::NoData)
            }
        }
    }

    // Shared assertions, generic over the runtime used for the *test's* bounded
    // waits. The actor itself runs on a dedicated OS thread with its own runtime
    // instance (see the harnesses below), so an adversarial peer that never
    // yields cannot wedge the test thread, and the actor's own timers are driven
    // by that thread rather than the one running these bounds.

    async fn assert_babble_never_starves_boundaries<R>(
        runtime: R,
        handle: AsyncOwnerHandle,
        terminated: flume::Receiver<OwnerSnapshot>,
    ) where
        R: Runtime,
    {
        // Admission is a boundary source; it must be serviced within a bound
        // despite the continuous flood of valid frames.
        let receipt = Executor::timeout(&runtime, Duration::from_secs(5), handle.submit(command()))
            .await
            .expect("a babbling peer must not starve admission")
            .expect("admission rejected");
        // Shutdown enters its one-slot lane; the actor must then terminate — the
        // `close()` liveness the P0 is about — within a bound.
        handle.shutdown().await.unwrap();
        let snapshot = Executor::timeout(&runtime, Duration::from_secs(5), terminated.recv_async())
            .await
            .expect("a babbling peer must not starve shutdown/close")
            .expect("actor terminated");
        assert_eq!(snapshot.state, SessionState::Shutdown);
        drop(receipt);
    }

    async fn assert_stalled_write_never_parks_close<R>(
        runtime: R,
        handle: AsyncOwnerHandle,
        terminated: flume::Receiver<OwnerSnapshot>,
    ) where
        R: Runtime,
    {
        // Short request deadlines so the fix's success path (the write is
        // abandoned at its timeout, then the request settles on its own
        // deadline) completes quickly and is cleanly separated from the broken
        // "never returns" case the bound catches.
        let receipt = Executor::timeout(
            &runtime,
            Duration::from_secs(5),
            handle.submit(command_with_short_deadlines()),
        )
        .await
        .expect("admission must resolve before the stalled write")
        .expect("admission rejected");
        // The write is abandoned at its 50 ms timeout, unparking the actor, so
        // shutdown/close is serviced rather than blocked behind the stalled peer.
        handle.shutdown().await.unwrap();
        let snapshot = Executor::timeout(&runtime, Duration::from_secs(5), terminated.recv_async())
            .await
            .expect("a stalled write must not park close")
            .expect("actor terminated");
        assert_eq!(snapshot.state, SessionState::Shutdown);
        drop(receipt);
    }

    async fn assert_nodata_never_hot_spins<R>(
        runtime: R,
        handle: AsyncOwnerHandle,
        terminated: flume::Receiver<OwnerSnapshot>,
        reads: Arc<std::sync::atomic::AtomicU64>,
    ) where
        R: Runtime,
    {
        // Let the idle transport run for a bounded wall-clock window.
        Executor::sleep(&runtime, Duration::from_millis(500)).await;
        let observed = reads.load(Ordering::Relaxed);
        handle.shutdown().await.unwrap();
        let _ = Executor::timeout(&runtime, Duration::from_secs(5), terminated.recv_async()).await;
        // The escalating idle pause caps at 250 ms, so a correctly paced actor
        // does single digits of reads here; the unbounded spin does hundreds of
        // thousands.
        assert!(
            observed < 2_000,
            "idle no-data receive hot-spun: {observed} reads in 500 ms"
        );
    }

    // Per-runtime harnesses: run `actor` on its own OS thread with its own
    // runtime instance, returning the terminal snapshot on a channel. tokio
    // timers are driven by that thread's runtime; smol timers by that thread's
    // `block_on`, which reacts on async-io whenever the actor parks.
    #[cfg(feature = "runtime-tokio")]
    fn run_isolated_tokio_actor<D>(
        owner_policy: OwnerPolicy,
        driver: D,
    ) -> (
        AsyncOwnerHandle,
        flume::Receiver<OwnerSnapshot>,
        std::thread::JoinHandle<()>,
    )
    where
        D: AsyncOwnerDriver + Send + 'static,
    {
        let actor_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let actor_runtime = TokioRuntime::from_handle(actor_rt.handle().clone());
        let (handle, actor) = AsyncOwnerActor::new(owner_policy, actor_runtime).unwrap();
        let (tx, rx) = flume::bounded::<OwnerSnapshot>(1);
        let join = std::thread::spawn(move || {
            let snapshot = actor_rt.block_on(actor.run(driver));
            let _ = tx.send(snapshot);
        });
        (handle, rx, join)
    }

    #[cfg(feature = "runtime-smol")]
    fn run_isolated_smol_actor<D>(
        owner_policy: OwnerPolicy,
        driver: D,
    ) -> (
        AsyncOwnerHandle,
        flume::Receiver<OwnerSnapshot>,
        std::thread::JoinHandle<()>,
    )
    where
        D: AsyncOwnerDriver + Send + 'static,
    {
        let (handle, actor) = AsyncOwnerActor::new(owner_policy, SmolRuntime::new()).unwrap();
        let (tx, rx) = flume::bounded::<OwnerSnapshot>(1);
        let join = std::thread::spawn(move || {
            let snapshot = smol::block_on(actor.run(driver));
            let _ = tx.send(snapshot);
        });
        (handle, rx, join)
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tokio_babbling_peer_never_starves_boundaries() {
        let (handle, terminated, join) = run_isolated_tokio_actor(policy(1), BabblingDriver);
        assert_babble_never_starves_boundaries(
            TokioRuntime::from_current().unwrap(),
            handle,
            terminated,
        )
        .await;
        join.join().unwrap();
    }

    /// The fairness ceiling must surrender the executor, not merely reverse
    /// polling order. This puts the actor, a caller admission, a control
    /// request, and a timer on one Tokio current-thread runtime. The outer
    /// watchdog lives on a separate OS thread so the pre-fix hot loop cannot
    /// hang the test binary; it asks the test driver to close only after the
    /// liveness deadline has already failed.
    #[cfg(feature = "runtime-tokio")]
    fn tokio_current_thread_ready_receive_yields_to_boundaries(
        empty_batches: bool,
        include_cancellation: bool,
        scenario: &'static str,
    ) {
        const WATCHDOG: Duration = Duration::from_secs(2);

        let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (finished, result) = flume::bounded(1);
        let worker_reads = Arc::clone(&reads);
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let outcome: Result<(), String> = runtime.block_on(async move {
                let actor_runtime =
                    TokioRuntime::from_current().map_err(|error| error.to_string())?;
                let owner_policy = policy(1);
                let fairness_ceiling = owner_policy.limits.frames_per_receive.max(1) as u64;
                let ready_reads = if empty_batches { 1 } else { fairness_ceiling };
                let (handle, actor) = AsyncOwnerActor::new(owner_policy, actor_runtime)
                    .map_err(|error| error.to_string())?;
                let actor_task = tokio::spawn(actor.run(CountingBabblingDriver {
                    reads: Arc::clone(&worker_reads),
                    stop: Arc::clone(&worker_stop),
                    empty_batches,
                }));

                // Do not enqueue boundary work until the actor has reached its
                // first forced (valid batch) or boundary-first (empty batch)
                // turn. Before the cooperative yield this loop is never polled
                // again; after it, all work queues on the same executor.
                while worker_reads.load(Ordering::Acquire) < ready_reads {
                    tokio::task::yield_now().await;
                }

                let caller_handle = handle.clone();
                let caller = tokio::spawn(async move {
                    caller_handle
                        .submit(command())
                        .await
                        .map_err(|error| error.to_string())
                });
                let control_handle = handle.clone();
                let control = tokio::spawn(async move {
                    control_handle
                        .snapshot()
                        .await
                        .map_err(|error| error.to_string())
                });
                let timer = tokio::spawn(async {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                });

                let (caller, control, timer) = tokio::join!(caller, control, timer);
                let receipt = caller.map_err(|error| format!("caller task failed: {error}"))??;
                let snapshot =
                    control.map_err(|error| format!("control task failed: {error}"))??;
                timer.map_err(|error| format!("timer task failed: {error}"))?;
                if snapshot.state != SessionState::Running {
                    return Err(format!(
                        "control observed an unexpected owner state: {:?}",
                        snapshot.state
                    ));
                }

                if include_cancellation {
                    let cancellation = handle
                        .cancel_test(receipt)
                        .await
                        .map_err(|error| format!("{error:?}"))?;
                    drop(cancellation);
                } else {
                    drop(receipt);
                }

                handle.shutdown().await.map_err(|error| error.to_string())?;
                let terminal = actor_task
                    .await
                    .map_err(|error| format!("actor task failed: {error}"))?;
                if terminal.state != SessionState::Shutdown {
                    return Err(format!(
                        "actor ended in an unexpected state: {:?}",
                        terminal.state
                    ));
                }
                Ok(())
            });
            let _ = finished.send(outcome);
        });

        match result.recv_timeout(WATCHDOG) {
            Ok(Ok(())) => worker.join().unwrap(),
            Ok(Err(error)) => {
                worker.join().unwrap();
                panic!("{scenario} single-thread liveness scenario failed: {error}");
            }
            Err(flume::RecvTimeoutError::Timeout) => {
                // The old implementation remains inside the ready receive loop.
                // Let this test-only driver turn that loop into a terminal read,
                // then join if it unwinds as expected; never wait indefinitely.
                stop.store(true, Ordering::Release);
                if result.recv_timeout(WATCHDOG).is_ok() {
                    worker.join().unwrap();
                } else {
                    drop(worker);
                }
                panic!(
                    "{scenario} monopolized Tokio's current-thread runtime before caller, control, cancellation, or timer work could run"
                );
            }
            Err(flume::RecvTimeoutError::Disconnected) => {
                worker.join().unwrap();
                panic!("{scenario} single-thread liveness worker exited without a result");
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn tokio_current_thread_babbling_peer_yields_to_caller_control_and_timer() {
        tokio_current_thread_ready_receive_yields_to_boundaries(false, false, "a babbling peer");
    }

    /// Delimited malformed stream frames reach the actor as an empty frame
    /// batch after the adapter records and discards them. Unlike `NoData`, that
    /// path has no pacing sleep, so this current-thread probe requires its own
    /// cooperative handoff. It queues every boundary class plus a timer after
    /// the first malformed batch; a pre-fix actor never gives those tasks a
    /// chance to enqueue on the same executor.
    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn tokio_current_thread_malformed_stream_batches_yield_to_all_boundaries() {
        tokio_current_thread_ready_receive_yields_to_boundaries(
            true,
            true,
            "discarded malformed stream frames",
        );
    }

    /// The same no-progress handoff on a single-thread, runtime-neutral
    /// executor. `AsyncOwnerActor` uses no Tokio scheduling primitive here:
    /// a discarded malformed batch must let independently spawned admission,
    /// control, timer, and shutdown work run under smol as well.
    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_current_thread_malformed_stream_batches_yield_to_boundaries() {
        const WATCHDOG: Duration = Duration::from_secs(2);

        let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (finished, result) = flume::bounded(1);
        let worker_reads = Arc::clone(&reads);
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            let local = async_executor::LocalExecutor::new();
            let outcome: Result<(), String> = future::block_on(local.run(async {
                let (handle, actor) = AsyncOwnerActor::new(policy(1), SmolRuntime::new())
                    .map_err(|error| error.to_string())?;
                let actor_task = local.spawn(actor.run(CountingBabblingDriver {
                    reads: Arc::clone(&worker_reads),
                    stop: Arc::clone(&worker_stop),
                    empty_batches: true,
                }));

                while worker_reads.load(Ordering::Acquire) == 0 {
                    future::yield_now().await;
                }

                let caller_handle = handle.clone();
                let caller = local.spawn(async move {
                    caller_handle
                        .submit(command())
                        .await
                        .map_err(|error| error.to_string())
                });
                let control_handle = handle.clone();
                let control = local.spawn(async move {
                    control_handle
                        .snapshot()
                        .await
                        .map_err(|error| error.to_string())
                });
                let timer = local.spawn(async {
                    smol::Timer::after(Duration::from_millis(1)).await;
                });

                drop(caller.await?);
                let snapshot = control.await?;
                timer.await;
                if snapshot.state != SessionState::Running {
                    return Err(format!(
                        "control observed an unexpected owner state: {:?}",
                        snapshot.state
                    ));
                }

                handle.shutdown().await.map_err(|error| error.to_string())?;
                let terminal = actor_task.await;
                if terminal.state != SessionState::Shutdown {
                    return Err(format!(
                        "actor ended in an unexpected state: {:?}",
                        terminal.state
                    ));
                }
                Ok(())
            }));
            let _ = finished.send(outcome);
        });

        match result.recv_timeout(WATCHDOG) {
            Ok(Ok(())) => worker.join().unwrap(),
            Ok(Err(error)) => {
                worker.join().unwrap();
                panic!("malformed-frame smol liveness scenario failed: {error}");
            }
            Err(flume::RecvTimeoutError::Timeout) => {
                stop.store(true, Ordering::Release);
                if result.recv_timeout(WATCHDOG).is_ok() {
                    worker.join().unwrap();
                } else {
                    drop(worker);
                }
                panic!(
                    "discarded malformed stream frames monopolized smol's current-thread executor before boundary or timer work could run"
                );
            }
            Err(flume::RecvTimeoutError::Disconnected) => {
                worker.join().unwrap();
                panic!("malformed-frame smol liveness worker exited without a result");
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tokio_stalled_write_never_parks_close() {
        let mut owner_policy = policy(1);
        owner_policy.write_timeout = Duration::from_millis(50);
        let (handle, terminated, join) =
            run_isolated_tokio_actor(owner_policy, StallingWriteDriver);
        assert_stalled_write_never_parks_close(
            TokioRuntime::from_current().unwrap(),
            handle,
            terminated,
        )
        .await;
        join.join().unwrap();
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tokio_nodata_receive_never_hot_spins() {
        let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (handle, terminated, join) = run_isolated_tokio_actor(
            policy(1),
            NoDataDriver {
                reads: Arc::clone(&reads),
            },
        );
        assert_nodata_never_hot_spins(
            TokioRuntime::from_current().unwrap(),
            handle,
            terminated,
            reads,
        )
        .await;
        join.join().unwrap();
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_babbling_peer_never_starves_boundaries() {
        let (handle, terminated, join) = run_isolated_smol_actor(policy(1), BabblingDriver);
        smol::block_on(assert_babble_never_starves_boundaries(
            SmolRuntime::new(),
            handle,
            terminated,
        ));
        join.join().unwrap();
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_stalled_write_never_parks_close() {
        let mut owner_policy = policy(1);
        owner_policy.write_timeout = Duration::from_millis(50);
        let (handle, terminated, join) = run_isolated_smol_actor(owner_policy, StallingWriteDriver);
        smol::block_on(assert_stalled_write_never_parks_close(
            SmolRuntime::new(),
            handle,
            terminated,
        ));
        join.join().unwrap();
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_nodata_receive_never_hot_spins() {
        let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (handle, terminated, join) = run_isolated_smol_actor(
            policy(1),
            NoDataDriver {
                reads: Arc::clone(&reads),
            },
        );
        smol::block_on(assert_nodata_never_hot_spins(
            SmolRuntime::new(),
            handle,
            terminated,
            reads,
        ));
        join.join().unwrap();
    }
}
