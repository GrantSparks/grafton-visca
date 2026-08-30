//! Runtime-neutral async actor and its bounded control boundary.

use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use futures_lite::future;

use crate::{
    completion, executor::Executor, AffectedAxes, CancellationOutcome, Error, ResponseDecoder,
};

use super::ReceiptObservation;
use super::{
    cancellation_receipt_for, completion_pair, normalize_cancellation_observation,
    normalize_command_outcome, normalize_inquiry_outcome, observation_outcome, prepend_effects,
    AdmissionPermit, AppliedEffect, CancellationCore, CompletionObserver, DecodedFrame,
    DiagnosticSubscription, Input, OwnerInputTurn, OwnerPolicy, OwnerState, ReceiptCore,
    RejectedCancellation, RequestId, RuntimeOutcome, RuntimeRequest, SessionState, ShutdownReason,
    TargetStateCache, TransmissionMeta, WaitSelection, WireWrite,
};
#[cfg(all(test, feature = "runtime-tokio"))]
use super::{DiagnosticEvent, OwnerMetrics};
use crate::runtime::engine::{Effect, IgnoreReason, TransportKind};

/// Pause applied after the first transient receive fault so a transport that
/// fails immediately cannot spin the actor. 1.x used the same bound.
const TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(10);

/// Ceiling on the escalating transient-fault pause.
const MAXIMUM_TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(250);

/// Consecutive transient receive faults, with no successful read between them,
/// after which the session ends with the underlying transport error.
///
/// A read that fails immediately and repeatedly is not a transient fault, it is
/// a broken transport wearing one. The escalating pause spreads this many
/// faults over roughly two seconds (10 + 20 + 40 + 80 + 160 ms, then 250 ms
/// each), and [`TRANSIENT_RECEIVE_FAULT_SPAN`] holds that floor even when the
/// pause is clamped away by a due deadline.
const TRANSIENT_RECEIVE_FAULT_LIMIT: u32 = 12;

/// Minimum wall-clock length of a fault run before it can end the session.
const TRANSIENT_RECEIVE_FAULT_SPAN: Duration = Duration::from_secs(1);

/// A gap this long between two transient faults proves the transport recovered
/// in between, so the run starts over rather than accumulating over hours.
const TRANSIENT_RECEIVE_FAULT_RESET: Duration = Duration::from_secs(5);

/// Clamp a transient pause so it can never push a due scheduler deadline past
/// its wake, mirroring the blocking owner's clamp to its caller's deadline.
fn clamp_transient_pause(pause: Duration, next_wake: Option<Instant>, now: Instant) -> Duration {
    next_wake.map_or(pause, |wake| pause.min(wake.saturating_duration_since(now)))
}

/// Escalating pause for the `run`-th consecutive transient receive fault.
fn transient_receive_pause(run: u32) -> Duration {
    let doublings = run.saturating_sub(1).min(6);
    TRANSIENT_RECEIVE_PAUSE
        .saturating_mul(1u32 << doublings)
        .min(MAXIMUM_TRANSIENT_RECEIVE_PAUSE)
}

/// One run of consecutive transient receive faults.
#[derive(Debug, Default)]
struct TransientFaultRun {
    length: u32,
    first_at: Option<Instant>,
    last_at: Option<Instant>,
}

impl TransientFaultRun {
    /// Record one transient fault and report the run it belongs to.
    fn record(&mut self, at: Instant) -> (u32, Duration) {
        let continues = self
            .last_at
            .is_some_and(|last| at.saturating_duration_since(last) < TRANSIENT_RECEIVE_FAULT_RESET);
        if continues {
            self.length = self.length.saturating_add(1);
        } else {
            self.length = 1;
            self.first_at = Some(at);
        }
        self.last_at = Some(at);
        let span = self
            .first_at
            .map_or(Duration::ZERO, |first| at.saturating_duration_since(first));
        (self.length, span)
    }

    /// A read that produced data, or that timed out cleanly, proves the
    /// transport is answering again.
    fn reset(&mut self) {
        self.length = 0;
        self.first_at = None;
        self.last_at = None;
    }

    /// Whether this run is long enough, and old enough, to be called permanent.
    const fn is_permanent(length: u32, span: Duration) -> bool {
        length >= TRANSIENT_RECEIVE_FAULT_LIMIT
            && span.as_nanos() >= TRANSIENT_RECEIVE_FAULT_SPAN.as_nanos()
    }
}

/// Whether one actor turn keeps the session alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnOutcome {
    /// Keep running with protocol input first.
    Continue,
    /// This receive turn made no protocol progress, so poll the ordered
    /// boundary sources first on the next selection.
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
            Self::Continue => Some(SourcePhase::ReceiveFirst),
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
    /// A caller that has already won expiry leaves this false. Conversely,
    /// claiming before expiry means the admission is authoritative, so the
    /// caller must observe its reply rather than turn that admitted work into
    /// a pre-admission timeout.
    fn claim_for_admission(&self, now: Instant) -> bool {
        if now >= self.deadline {
            let _ = self.state.compare_exchange(
                Self::PENDING,
                Self::EXPIRED,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            return false;
        }

        self.state
            .compare_exchange(
                Self::PENDING,
                Self::CLAIMED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
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

#[derive(Debug)]
enum ControlBoundary {
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
    /// and no reader ever observes a mixture of the two.
    Reconfigure {
        tuning: Box<crate::OperationalTuning>,
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
        if self.now() >= deadline {
            return Err(Error::Timeout);
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
            } => return Err(Error::Timeout),
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
    pub(crate) async fn reconfigure(&self, tuning: crate::OperationalTuning) -> Result<(), Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::Reconfigure {
                tuning: Box::new(tuning),
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
        if let Some(error) = self.admission_rejection() {
            return Err(error);
        }
        let permit = self.permits.try_acquire().ok_or(Error::RuntimeQueueFull {
            capacity: self.permits.capacity(),
        })?;
        if let Some(error) = self.admission_rejection() {
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
        };

        // Keep the terminal check and enqueue in one lifecycle critical
        // section. Otherwise a caller can pass the second check, the actor can
        // publish/drop and drain all boundaries, and this send can strand the
        // permit in a queue whose receiver will never poll it (#542 §4).
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
            return Err(error);
        }
        match self.admissions.try_send(boundary) {
            Ok(()) => Ok((completion, admission)),
            Err(flume::TrySendError::Disconnected(_)) => Err(self.disconnected_error()),
            Err(flume::TrySendError::Full(_)) => Err(Error::InvalidState(
                "admission channel full after permit reservation".into(),
            )),
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
    idle_receive_run: u32,
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
        let (admission_tx, admissions) = flume::bounded(boundary_capacity);
        let (cancellation_tx, cancellations) = flume::bounded(cancellation_capacity);
        let (control_tx, control) = flume::bounded(control_capacity);
        let (shutdown_tx, shutdown) = flume::bounded(1);
        let (alive, actor_alive) = flume::bounded(1);
        let shutdown_signal = Arc::new(Mutex::new(ShutdownSignalState::Open));
        let terminal_error = Arc::new(Mutex::new(None));
        Ok((
            AsyncOwnerHandle {
                permits,
                admissions: admission_tx,
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
                cancellations,
                control,
                shutdown,
                alive,
                shutdown_signal: Arc::clone(&shutdown_signal),
                terminal_error,
                faults: TransientFaultRun::default(),
                idle_receive_run: 0,
                runtime,
            },
        ))
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
        loop {
            if self.state.state() != SessionState::Running {
                break;
            }
            // Even while the peer keeps making receive-first progress, force the
            // ordered boundary sources to the front once the streak reaches the
            // ceiling, then restart the count. When nothing is queued on a
            // boundary the receive still wins this turn, so a busy transport is
            // never stalled — only guaranteed to yield the front periodically.
            let forced_boundary_turn = source_phase == SourcePhase::ReceiveFirst
                && receive_first_streak >= fairness_ceiling;
            if forced_boundary_turn {
                receive_first_streak = 0;
                // Polling boundaries first alone is not a cooperative handoff:
                // if they are all pending, the ready receive wins immediately
                // and this task can monopolize a single-thread executor. Yield
                // before the forced turn so caller work and timers can become
                // ready without changing the boundary source order.
                cooperative_yield().await;
            }
            let effective_phase = if forced_boundary_turn {
                SourcePhase::BoundariesFirst
            } else {
                source_phase
            };
            // Compute this after the cooperative yield: a timer that became due
            // while another task ran must not inherit a stale positive delay.
            let wake_duration = self
                .state
                .next_wake()
                .map(|wake| wake.saturating_duration_since(Executor::now(runtime.as_ref())))
                .unwrap_or(Duration::from_secs(86_400));

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
                    Executor::sleep(runtime.as_ref(), wake_duration).await;
                    ActorEvent::Wake
                };
                let boundaries = future::or(
                    shutdown,
                    future::or(
                        cancellation,
                        future::or(admission, future::or(control, wake)),
                    ),
                );
                select_source(effective_phase, receive, boundaries).await
            };

            // A receive that keeps the session running and makes protocol
            // progress is the only thing that lengthens the streak; any boundary
            // turn (or a non-progressing receive, which already yields) resets it
            // so the ceiling only ever fires against a genuine receive flood.
            let event_was_receive = matches!(event, ActorEvent::Receive { .. });
            let outcome = self
                .handle_event(event, &mut driver, runtime.as_ref())
                .await;
            if event_was_receive && outcome == TurnOutcome::Continue {
                receive_first_streak = receive_first_streak.saturating_add(1);
            } else {
                receive_first_streak = 0;
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
        // Publish the terminal result under the same lifecycle lock used by
        // `AsyncOwnerHandle::shutdown`. This makes the result and shutdown
        // acceptance linearisable: once terminal publication wins, shutdown
        // cannot enqueue a signal into a receiver the actor will never poll.
        // Keep the accepted state only for an orderly explicit shutdown. A
        // transport close/poison supersedes an earlier accepted signal so a
        // later shutdown/close call cannot mask the real terminal cause.
        {
            let mut signal = self
                .shutdown_signal
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *self
                .terminal_error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(boundary_error.clone());
            if !matches!(&boundary_error, Error::RuntimeShutdown)
                || !matches!(*signal, ShutdownSignalState::Accepted)
            {
                *signal = ShutdownSignalState::Failed(boundary_error.clone());
            }
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
    ) -> TurnOutcome
    where
        D: AsyncOwnerDriver,
    {
        match event {
            ActorEvent::Shutdown => {
                self.terminate(driver, runtime, ShutdownReason::Explicit)
                    .await;
                TurnOutcome::Stop
            }
            ActorEvent::Cancellation(cancel) => {
                self.handle_cancellation(cancel, driver, runtime).await;
                TurnOutcome::Continue
            }
            ActorEvent::Control(control) => {
                self.handle_control(control);
                TurnOutcome::Continue
            }
            ActorEvent::Admission(Ok(admission)) => {
                self.handle_admission(admission, driver, runtime).await;
                TurnOutcome::Continue
            }
            ActorEvent::Admission(Err(_)) => {
                self.terminate(driver, runtime, ShutdownReason::Explicit)
                    .await;
                TurnOutcome::Stop
            }
            ActorEvent::Wake => {
                let effects = self.state.advance(Executor::now(runtime));
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
                ..
            } => {
                // An expired idle read timeout is not a fault: nothing was
                // consumed, nothing failed, and no request's retry budget is
                // touched. It made no protocol progress, so let a queued boundary
                // run before another idle read (#625). A driver that returns
                // NoData immediately would otherwise spin the actor, so pace the
                // idle-read rate (#675).
                self.absorb_idle_receive(runtime).await
            }
            ActorEvent::Receive {
                result: Ok(AsyncReceive::Frames(frames)),
                received_at,
            } => {
                self.faults.reset();
                // Real bytes decoded: the transport is not idle, so restart the
                // no-data escalation (#675).
                self.idle_receive_run = 0;
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
                    return TurnOutcome::YieldBoundaries;
                }
                let turn = self.state.begin_input_turn(received_at);
                for frame in frames {
                    let effects = self.state.input_in_turn(&turn, Input::Frame(frame));
                    self.drive_in_turn(driver, &turn, effects, runtime).await;
                }
                let due = self.state.finish_input_turn(turn);
                self.drive(driver, due, runtime).await;
                TurnOutcome::Continue
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
                    return self.absorb_idle_receive(runtime).await;
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
                let effects = self.state.input(Input::ReceiveFault { error }, received_at);
                self.drive(driver, effects, runtime).await;
                let pause = clamp_transient_pause(
                    transient_receive_pause(length),
                    self.state.next_wake(),
                    Executor::now(runtime),
                );
                if !pause.is_zero() {
                    Executor::sleep(runtime, pause).await;
                }
                TurnOutcome::YieldBoundaries
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
    /// receive-fault pause — but it records no fault and spends no retry budget,
    /// so an indefinitely idle transport is never mistaken for a broken one.
    async fn absorb_idle_receive(&mut self, runtime: &R) -> TurnOutcome {
        self.faults.reset();
        self.idle_receive_run = self.idle_receive_run.saturating_add(1);
        let pause = clamp_transient_pause(
            transient_receive_pause(self.idle_receive_run),
            self.state.next_wake(),
            Executor::now(runtime),
        );
        if !pause.is_zero() {
            Executor::sleep(runtime, pause).await;
        }
        TurnOutcome::YieldBoundaries
    }

    async fn handle_admission<D>(
        &mut self,
        admission: AdmissionBoundary,
        driver: &mut D,
        runtime: &R,
    ) where
        D: AsyncOwnerDriver,
    {
        // A deadline that wins before this exact boundary is admitted is not
        // observer detachment: no engine entry exists yet. Drop the boundary
        // (and therefore its permit and observer) before staging any engine
        // input, so a stale admission can never become a later write.
        if admission
            .validity
            .as_ref()
            .is_some_and(|validity| !validity.claim_for_admission(Executor::now(runtime)))
        {
            let _ = admission.reply.try_send(Err(Error::Timeout));
            return;
        }
        let input = self.state.stage_admission_with(
            admission.request,
            admission.permit,
            admission.observer,
            admission.reply,
        );
        let effects = self.state.input(input, Executor::now(runtime));
        self.drive(driver, effects, runtime).await;
    }

    async fn handle_cancellation<D>(
        &mut self,
        cancellation: CancellationBoundary,
        driver: &mut D,
        runtime: &R,
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
        let effects = self
            .state
            .input(Input::Cancel { id }, Executor::now(runtime));
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

    fn handle_control(&mut self, control: ControlBoundary) {
        match control {
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
            ControlBoundary::Reconfigure { tuning, reply } => {
                let result = self.state.retune(*tuning);
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
            if let AppliedEffect::Transmit(staged) = self.state.apply_effect(effect) {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => Self::write_frame(driver, write, runtime, write_timeout).await,
                    Err(error) => Err(error),
                };
                let produced =
                    self.state
                        .finish_write(&staged, write_result, Executor::now(runtime));
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
            if let AppliedEffect::Transmit(staged) = self.state.apply_effect(effect) {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => Self::write_frame(driver, write, runtime, write_timeout).await,
                    Err(error) => Err(error),
                };
                let produced = self.state.finish_write_in_turn(turn, &staged, write_result);
                prepend_effects(&mut effects, produced);
            }
        }
    }

    async fn terminate<D>(&mut self, driver: &mut D, runtime: &R, reason: ShutdownReason)
    where
        D: AsyncOwnerDriver,
    {
        self.terminate_at(driver, runtime, reason, Executor::now(runtime))
            .await;
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

    /// Answer every queued boundary message with the session's terminal error.
    ///
    /// The lanes are drained repeatedly until one whole pass finds all three
    /// empty, because a caller can enqueue on a lane that was already visited
    /// while a later one is still being drained (#626). The residual window
    /// between the last pass and the actor dropping its receivers is closed by
    /// the liveness lane, not here.
    fn drain_boundaries(&mut self, error: Error) {
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
    Wake,
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
    }

    #[cfg(feature = "runtime-tokio")]
    impl ManualRuntime {
        fn new(now: Instant) -> Self {
            Self {
                executor: TokioRuntime::from_current().unwrap(),
                now: Arc::new(Mutex::new(now)),
            }
        }

        fn advance(&self, duration: Duration) {
            let mut now = self.now.lock().unwrap();
            *now = now.checked_add(duration).unwrap();
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

        fn sleep(&self, _duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            future::pending()
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
            *self.now.lock().unwrap()
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

    fn command() -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff]).unwrap()),
            context: RequestContext {
                target: CameraId::CAMERA_1,
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

    fn inquiry() -> RuntimeRequest {
        RuntimeRequest::Inquiry {
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x09, 0x04, 0x00, 0xff]).unwrap()),
            context: RequestContext {
                target: CameraId::CAMERA_1,
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
        assert_eq!(snapshot.active, 0);
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
    async fn async_receive_batch_precedes_an_overdue_completion_deadline() {
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

        let second = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), second.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 2);

        runtime.advance(Duration::from_secs(6));
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

    /// Issue #625. An idle read timeout is no data, not a fault. A transport
    /// with an internal read timeout — the shape the public trait documents —
    /// must not retransmit anything, and must not starve the boundary either.
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
            Error::Io(Arc::new(std::io::Error::from(std::io::ErrorKind::TimedOut))),
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
        assert_eq!(clamp_transient_pause(pause, None, now), pause);
        assert_eq!(
            clamp_transient_pause(pause, Some(now + Duration::from_secs(1)), now),
            pause
        );
        assert_eq!(
            clamp_transient_pause(pause, Some(now + Duration::from_millis(3)), now),
            Duration::from_millis(3)
        );
        assert_eq!(
            clamp_transient_pause(pause, Some(now - Duration::from_millis(5)), now),
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
    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct CountingBabblingDriver {
        reads: Arc<std::sync::atomic::AtomicU64>,
        stop: Arc<std::sync::atomic::AtomicBool>,
    }

    #[cfg(feature = "runtime-tokio")]
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
            async move {
                reads.fetch_add(1, Ordering::Relaxed);
                if stop.load(Ordering::Acquire) {
                    Ok(AsyncReceive::Closed)
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
    #[test]
    fn tokio_current_thread_babbling_peer_yields_to_caller_control_and_timer() {
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
                let (handle, actor) = AsyncOwnerActor::new(owner_policy, actor_runtime)
                    .map_err(|error| error.to_string())?;
                let actor_task = tokio::spawn(actor.run(CountingBabblingDriver {
                    reads: Arc::clone(&worker_reads),
                    stop: Arc::clone(&worker_stop),
                }));

                // Do not enqueue any boundary work until the actor has reached
                // its first forced turn. Before the cooperative yield this loop
                // is never polled again; after it, the test queues all work on
                // the same one-thread executor.
                while worker_reads.load(Ordering::Acquire) < fairness_ceiling {
                    tokio::task::yield_now().await;
                }

                let caller_handle = handle.clone();
                let caller = tokio::spawn(async move {
                    caller_handle
                        .submit(command())
                        .await
                        .map(drop)
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
                caller.map_err(|error| format!("caller task failed: {error}"))??;
                let snapshot =
                    control.map_err(|error| format!("control task failed: {error}"))??;
                timer.map_err(|error| format!("timer task failed: {error}"))?;
                if snapshot.state != SessionState::Running {
                    return Err(format!(
                        "control observed an unexpected owner state: {:?}",
                        snapshot.state
                    ));
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
                panic!("single-thread liveness scenario failed: {error}");
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
                    "a babbling peer monopolized Tokio's current-thread runtime before caller, control, or timer work could run"
                );
            }
            Err(flume::RecvTimeoutError::Disconnected) => {
                worker.join().unwrap();
                panic!("single-thread liveness worker exited without a result");
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
