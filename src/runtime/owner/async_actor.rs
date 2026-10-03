//! Runtime-neutral async actor and its bounded control boundary.

use std::{
    collections::VecDeque,
    future::Future,
    pin::{pin, Pin},
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex,
    },
    task::Poll,
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
use super::{
    clamp_receive_pause, normalize_command_outcome, normalize_inquiry_outcome, prepend_effects,
    transient_receive_pause, AdmissionPermit, AppliedEffect, CancellationObserver,
    CancellationRequest, DecodedFrame, DiagnosticSubscription, IdleReceiveRun, Input,
    OperationObservation, OwnerInputTurn, OwnerPolicy, OwnerState, RawReleaseResolution,
    ReceiptCore, RequestId, RequestLane, RetainedStreamInput, RuntimeOutcome, RuntimeRequest,
    SessionState, ShutdownReason, TargetStateCache, TerminalObserver, TransientFaultRun,
    TransmissionMeta, WireWrite,
};

use super::turn::{
    Disposition, OwnerCoordinator, ReceiveArm, ReceiveClass, Selected, Selection, Source, TimerArm,
    TurnOutcome, TurnPlan,
};
#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
use super::{
    MAXIMUM_TRANSIENT_RECEIVE_PAUSE, TRANSIENT_RECEIVE_FAULT_LIMIT, TRANSIENT_RECEIVE_FAULT_RESET,
    TRANSIENT_RECEIVE_FAULT_SPAN, TRANSIENT_RECEIVE_PAUSE,
};
use crate::runtime::engine::{Effect, EngineTurn, IgnoreReason, RawPrefixEvidence, TransportKind};

/// Poll the six actor sources in the coordinator's planned order.
///
/// Each poll walks [`Selection::order`] and returns the first ready source,
/// so simultaneous readiness is decided by that one definition rather than by
/// an executor's wake order. Sources the selection omits are never polled.
async fn select_in_order<T, Rx, Sh, Ca, Ad, Co, Ti>(
    selection: Selection,
    receive: Rx,
    shutdown: Sh,
    cancellation: Ca,
    admission: Ad,
    control: Co,
    timer: Ti,
) -> T
where
    Rx: Future<Output = T>,
    Sh: Future<Output = T>,
    Ca: Future<Output = T>,
    Ad: Future<Output = T>,
    Co: Future<Output = T>,
    Ti: Future<Output = T>,
{
    let mut receive = pin!(receive);
    let mut shutdown = pin!(shutdown);
    let mut cancellation = pin!(cancellation);
    let mut admission = pin!(admission);
    let mut control = pin!(control);
    let mut timer = pin!(timer);
    let order = selection.order();
    future::poll_fn(|context| {
        for source in order.clone() {
            let poll = match source {
                Source::Receive => receive.as_mut().poll(context),
                Source::Shutdown => shutdown.as_mut().poll(context),
                Source::Cancellation => cancellation.as_mut().poll(context),
                Source::Admission => admission.as_mut().poll(context),
                Source::Control => control.as_mut().poll(context),
                Source::Timer => timer.as_mut().poll(context),
            };
            if poll.is_ready() {
                return poll;
            }
        }
        Poll::Pending
    })
    .await
}

/// The actor's boundary channels.
#[derive(Clone, Copy)]
struct ActorBoundaryReceivers<'a> {
    shutdown: &'a flume::Receiver<()>,
    cancellations: &'a flume::Receiver<CancellationBoundary>,
    admissions: &'a flume::Receiver<AdmissionBoundary>,
    control: &'a flume::Receiver<ControlBoundary>,
}

/// Select one actor event as planned by the [`OwnerCoordinator`]. The caller
/// has already mapped the selection's receive and timer arms onto `receive`
/// and `timer`. A disconnected shutdown, cancellation or control lane never
/// becomes ready; a disconnected admission lane is reported.
async fn select_actor_event<Receive, Timer>(
    selection: Selection,
    receive: Receive,
    boundaries: ActorBoundaryReceivers<'_>,
    timer: Timer,
) -> ActorEvent
where
    Receive: Future<Output = ActorEvent>,
    Timer: Future<Output = ActorEvent>,
{
    let shutdown = async {
        match boundaries.shutdown.recv_async().await {
            Ok(()) => ActorEvent::Shutdown,
            Err(_) => future::pending().await,
        }
    };
    let cancellation = async {
        match boundaries.cancellations.recv_async().await {
            Ok(value) => ActorEvent::Cancellation(value),
            Err(_) => future::pending().await,
        }
    };
    let admission = async { ActorEvent::Admission(boundaries.admissions.recv_async().await) };
    let control = async {
        match boundaries.control.recv_async().await {
            Ok(value) => ActorEvent::Control(value),
            Err(_) => future::pending().await,
        }
    };
    select_in_order(
        selection,
        receive,
        shutdown,
        cancellation,
        admission,
        control,
        timer,
    )
    .await
}

/// Map the shared engine-owned retained-prefix deadline onto one async wake.
///
/// This is deliberately a separate future from transport receive: selection
/// still polls both ordered sources, so an eagerly-idle custom transport
/// cannot prevent the grace timer from becoming ready.
async fn raw_release_wake<R: Executor>(runtime: &R, deadline: Instant, now: Instant) -> ActorEvent {
    let remaining = deadline.saturating_duration_since(now);
    if !remaining.is_zero() {
        Executor::sleep(runtime, remaining).await;
    }
    ActorEvent::Wake
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
            Poll::Ready(())
        } else {
            yielded = true;
            context.waker().wake_by_ref();
            Poll::Pending
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
pub(crate) trait AsyncOwnerDriver: Send + RetainedStreamInput {
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
    /// `true` means it may return an admission timeout; `false` means an admitted
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
    observer: Arc<super::ObserverCell<RuntimeOutcome>>,
    reply: flume::Sender<Result<RequestId, Error>>,
    /// Present only for a caller deadline that applies before admission.
    validity: Option<AdmissionValidity>,
    /// The actor has already won the caller's pre-admission deadline race,
    /// but a raw-correlation release gate has retained this boundary until it
    /// can safely enter the engine.  Keeping that one-way answer on the
    /// boundary preserves the caller's established admission promise without
    /// letting the retained admission run a due/dispatch turn early.
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
    request: CancellationRequest,
    /// `Ok` once the intent is installed (or the operation already
    /// concluded); the refusal otherwise. The caller keeps its handle either
    /// way (#777).
    reply: flume::Sender<Result<(), Error>>,
}

/// An admission or cancellation selected while a raw-correlation release was
/// due. Its ordinary input turn would run due work before the release proof,
/// so the [`OwnerCoordinator`] retains it in a single slot until the release
/// resolves. Selection makes cancellation and admission ineligible while the
/// slot is occupied, so a second boundary stays in its bounded channel
/// instead of replacing this one (#775).
///
/// `AdmissionBoundary` keeps its request inline so accepting an admission does
/// not add a heap allocation at the actor-channel boundary.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum RetainedBoundary {
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

/// Async-mode observation of one admitted operation: its cached observation
/// state, settlement plan, and originating owner. Every wait borrows it, so
/// an abandoned or timed-out wait releases only that wait (#777). Only this
/// receipt class exposes cancellation.
#[derive(Debug)]
pub(crate) struct AsyncOperationReceipt<K>
where
    K: completion::Kind,
{
    observation: OperationObservation,
    affected_axes: AffectedAxes,
    settlement: completion::Settlement<K>,
    owner: AsyncOwnerHandle,
}

/// Selected owner and executor clock used only by receipt observation.
#[derive(Debug, Clone)]
pub(crate) struct AsyncReceiptControl {
    owner: AsyncOwnerHandle,
}

impl AsyncCommandReceipt {
    pub(crate) async fn wait(self, control: AsyncReceiptControl) -> Result<(), Error> {
        let deadline = control
            .owner
            .deadline_after(self.core.configured_timeout())?;
        wait_core_until(&self.core, &control.owner, deadline)
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
        let deadline = control.owner.deadline_after(timeout)?;
        wait_core_until(&self.core, &control.owner, deadline)
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
        let deadline = control
            .owner
            .deadline_after(self.core.configured_timeout())?;
        self.wait_until(&control.owner, deadline).await
    }

    async fn wait_until(self, owner: &AsyncOwnerHandle, deadline: Instant) -> Result<T, Error> {
        let outcome = wait_core_until(&self.core, owner, deadline).await?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }
}

impl<K> AsyncOperationReceipt<K>
where
    K: completion::Kind,
{
    pub(crate) fn id(&self) -> u64 {
        self.observation.id().get()
    }

    /// Waits for application, within `timeout` or the configured observer
    /// deadline.
    pub(crate) async fn applied(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        let timeout = timeout.unwrap_or_else(|| self.observation.applied_timeout());
        let deadline = self.owner.deadline_after(timeout)?;
        self.applied_until(deadline).await
    }

    async fn applied_until(&mut self, deadline: Instant) -> Result<(), Error> {
        observe_until(
            &self.owner,
            &mut self.observation,
            deadline,
            OperationObservation::applied,
        )
        .await?
    }

    /// Cancels the operation and waits for the cancellation's conclusion,
    /// within `timeout` or the configured cancellation deadline (#777).
    ///
    /// A known terminal outcome answers without sending anything. Otherwise
    /// the handle's one cancellation intent is requested; a request for an
    /// intent the owner already holds observes it instead of sending another.
    /// A refusal leaves the operation running and this receipt intact.
    pub(crate) async fn cancel(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<CancellationOutcome, Error> {
        let timeout = timeout.unwrap_or_else(|| self.observation.cancellation_timeout());
        let deadline = self.owner.deadline_after(timeout)?;
        if let Some(verdict) = self.observation.cancellation() {
            return verdict;
        }
        ensure_same_owner(&self.owner, self.observation.origin())?;
        let request = self.observation.cancellation_request();
        if let Err(error) = self.owner.request_cancellation(request, deadline).await {
            // A terminal outcome that raced the refusal still decides.
            return self.observation.cancellation().unwrap_or(Err(error));
        }
        observe_until(
            &self.owner,
            &mut self.observation,
            deadline,
            OperationObservation::cancellation,
        )
        .await?
    }
}

impl AsyncOperationReceipt<completion::Targeted> {
    /// Waits for application and then the profile-selected settlement
    /// condition, within `timeout` or the configured settlement budget.
    ///
    /// Application is cached, so a wait that is abandoned or times out during
    /// polling restarts from it with a fresh two-sample proof.
    pub(crate) async fn settled(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        if self.observation.is_settled() {
            return Ok(());
        }
        let budget = match timeout {
            Some(timeout) => timeout,
            None => self.settlement.default_budget().ok_or_else(|| {
                Error::InvalidState(
                    "targeted operation omitted its configured settlement budget".into(),
                )
            })?,
        };
        let deadline = self.owner.deadline_after(budget)?;
        self.applied_until(deadline).await?;
        match self.settlement.plan()? {
            crate::prepared::SettlementPlan::CompletionIsSettled { target, .. } => {
                debug_assert_eq!(*target, self.observation.target());
            }
            crate::prepared::SettlementPlan::Poll {
                target,
                queries,
                axes,
                tolerance,
                interval,
                ..
            } => {
                debug_assert_eq!(*target, self.observation.target());
                debug_assert_eq!(*axes, self.affected_axes);
                poll_settlement_async(&self.owner, queries, *axes, *tolerance, *interval, deadline)
                    .await
                    .map_err(|error| super::settlement_error(error, self.observation.id()))?;
            }
        }
        self.observation.mark_settled();
        Ok(())
    }
}

/// Proves settlement by position polling: one baseline snapshot, then
/// snapshots every `interval` until the detector reports no movement, all
/// before one absolute owner-clock deadline. All polling state is local, so an
/// abandoned wait loses nothing authoritative.
async fn poll_settlement_async(
    owner: &AsyncOwnerHandle,
    queries: &crate::prepared::PositionQueryPlan,
    axes: AffectedAxes,
    tolerance: crate::camera::MovementTolerance,
    interval: Duration,
    deadline: Instant,
) -> Result<(), Error> {
    let control = owner.receipt_control();
    let mut detector = crate::prepared::MotionDetector::new(axes, tolerance);
    let baseline = sample_positions_async(owner, &control, queries, deadline).await?;
    if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
        return Err(Error::InvalidState(
            "new movement detector rejected its baseline snapshot".into(),
        ));
    }
    loop {
        let remaining = deadline.saturating_duration_since(owner.now());
        if remaining.is_zero() {
            return Err(Error::query_timeout());
        }
        owner.clock.sleep(interval.min(remaining)).await;
        ensure_async_before_deadline(owner, deadline)?;
        let snapshot = sample_positions_async(owner, &control, queries, deadline).await?;
        match detector.observe(snapshot)? {
            crate::prepared::MotionState::Settled => return Ok(()),
            crate::prepared::MotionState::Moving => {}
            crate::prepared::MotionState::NeedSample => {
                return Err(Error::InvalidState(
                    "movement detector lost its baseline snapshot".into(),
                ));
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
    debug_assert!(Arc::ptr_eq(&owner.origin, &control.owner.origin));
    let mut snapshot = crate::prepared::PositionSnapshot::default();
    if let Some(query) = &queries.pan_tilt {
        snapshot.pan_tilt = Some(sample_async(owner, query, deadline).await?);
    }
    if let Some(query) = &queries.zoom {
        snapshot.zoom = Some(sample_async(owner, query, deadline).await?);
    }
    if let Some(query) = &queries.focus {
        snapshot.focus = Some(sample_async(owner, query, deadline).await?);
    }
    if let Some(query) = &queries.iris {
        snapshot.iris = Some(sample_async(owner, query, deadline).await?);
    }
    if let Some(query) = &queries.nd_filter {
        snapshot.nd_filter = Some(sample_async(owner, query, deadline).await?);
    }
    Ok(snapshot)
}

/// One position inquiry, admitted and answered strictly before `deadline`.
async fn sample_async<R>(
    owner: &AsyncOwnerHandle,
    query: &crate::prepared::PreparedInquiryTemplate<R>,
    deadline: Instant,
) -> Result<R, Error> {
    let sample = async {
        ensure_async_before_deadline(owner, deadline)?;
        let receipt = owner
            .submit_inquiry_until(query.instantiate(), deadline)
            .await?;
        let value = receipt.wait_until(owner, deadline).await?;
        ensure_async_before_deadline(owner, deadline)?;
        Ok(value)
    };
    sample.await.map_err(Error::into_query_timeout)
}

/// Rechecks the owner-bound monotonic clock at every admission/sample
/// boundary. In particular, equality with the deadline is already too late
/// for another inquiry to be enqueued.
pub(crate) fn ensure_async_before_deadline(
    owner: &AsyncOwnerHandle,
    deadline: Instant,
) -> Result<(), Error> {
    if owner.now() >= deadline {
        Err(Error::query_timeout())
    } else {
        Ok(())
    }
}

fn ensure_same_owner(owner: &AsyncOwnerHandle, origin: &Arc<()>) -> Result<(), Error> {
    if Arc::ptr_eq(origin, &owner.origin) {
        Ok(())
    } else {
        Err(Error::InvalidState(
            "receipt belongs to a different owner".into(),
        ))
    }
}

/// What ended one wait for an observation slot.
enum ObservationWake {
    /// The terminal slot delivered (`None`: the owner dropped it unresolved).
    Terminal(Option<RuntimeOutcome>),
    /// The cancellation slot delivered (`None`: the owner dropped it, which it
    /// does only when the operation's terminal outcome is already delivered).
    Cancellation(Option<Error>),
    /// The actor is gone.
    ActorGone,
    /// The observer deadline passed.
    Deadline,
}

/// Waits for the next event on an operation's observation slots. This is the
/// one race every async receipt wait uses: the terminal slot, then the
/// cancellation slot, then actor liveness, then the observer deadline, in
/// left-biased order. Each arm only reads a slot; flume leaves an unread value
/// queued if this future is dropped.
async fn next_observation(
    owner: &AsyncOwnerHandle,
    terminal: &TerminalObserver,
    cancellation: Option<&CancellationObserver>,
    deadline: Instant,
) -> ObservationWake {
    let remaining = deadline.saturating_duration_since(owner.now());
    let terminal = async { ObservationWake::Terminal(terminal.recv_async().await) };
    let cancellation = async {
        match cancellation {
            Some(observer) => ObservationWake::Cancellation(observer.recv_async().await),
            None => future::pending().await,
        }
    };
    let actor_gone = async {
        // Nothing is ever sent on this lane. It resolves when the actor has
        // dropped its sender, including an unwind before `run` can latch a
        // terminal owner error.
        while owner.actor_alive.recv_async().await.is_ok() {}
        ObservationWake::ActorGone
    };
    let timer = async {
        owner.clock.sleep(remaining).await;
        ObservationWake::Deadline
    };
    future::or(
        terminal,
        future::or(cancellation, future::or(actor_gone, timer)),
    )
    .await
}

/// Waits until `verdict` can be read from an operation's observation state.
///
/// Every value a slot delivers is recorded into the handle's cache in the same
/// poll that receives it, so dropping this future never loses one. After the
/// actor is gone or the deadline passes, the slots are read once more: a
/// terminal outcome that raced teardown, or arrived exactly at the deadline,
/// still decides.
async fn observe_until<T>(
    owner: &AsyncOwnerHandle,
    observation: &mut OperationObservation,
    deadline: Instant,
    verdict: impl Fn(&mut OperationObservation) -> Option<T>,
) -> Result<T, Error> {
    ensure_same_owner(owner, observation.origin())?;
    let mut cancellation_open = true;
    loop {
        if let Some(value) = verdict(observation) {
            return Ok(value);
        }
        let cancellation = observation
            .cancellation_observer()
            .filter(|_| cancellation_open);
        match next_observation(
            owner,
            observation.terminal_observer(),
            cancellation,
            deadline,
        )
        .await
        {
            ObservationWake::Terminal(Some(outcome)) => observation.record_terminal(outcome),
            ObservationWake::Cancellation(Some(error)) => {
                observation.record_cancellation_failure(error);
            }
            ObservationWake::Cancellation(None) => cancellation_open = false,
            ObservationWake::Terminal(None) | ObservationWake::ActorGone => {
                return verdict(observation).ok_or_else(|| owner.disconnected_error());
            }
            ObservationWake::Deadline => {
                return verdict(observation)
                    .ok_or_else(|| super::observation_timeout(observation.id()));
            }
        }
    }
}

/// Waits for a command or inquiry receipt's terminal outcome.
async fn wait_core_until(
    core: &ReceiptCore,
    owner: &AsyncOwnerHandle,
    deadline: Instant,
) -> Result<RuntimeOutcome, Error> {
    ensure_same_owner(owner, &core.origin)?;
    if let Some(outcome) = core.try_outcome() {
        return Ok(outcome);
    }
    match next_observation(owner, &core.completion, None, deadline).await {
        ObservationWake::Terminal(Some(outcome)) => Ok(outcome),
        ObservationWake::Terminal(None) | ObservationWake::ActorGone => {
            core.try_outcome().ok_or_else(|| owner.disconnected_error())
        }
        ObservationWake::Deadline => core
            .try_outcome()
            .ok_or_else(|| super::observation_timeout(core.id)),
        ObservationWake::Cancellation(_) => Err(Error::InvalidState(
            "a receipt without a cancellation slot observed one".into(),
        )),
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
            .admit_with(|request, affected_axes, settlement, timeouts| async move {
                self.submit_with_timeout(request, timeouts.applied)
                    .await
                    .map(|core| AsyncOperationReceipt {
                        observation: OperationObservation::new(core, timeouts.cancellation),
                        affected_axes,
                        settlement,
                        owner: self.clone(),
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
            let error = Error::admission_timeout();
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
                let error = Error::admission_timeout();
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
    /// Delivers one cancellation request to the actor and returns its answer,
    /// all before `deadline` (#777). A full cancellation lane is backpressure;
    /// a closed one is the session's terminal error.
    async fn request_cancellation(
        &self,
        request: CancellationRequest,
        deadline: Instant,
    ) -> Result<(), Error> {
        let id = request.id;
        let (reply, receiver) = flume::bounded(1);
        let delivered = async {
            self.cancellations
                .send_async(CancellationBoundary { request, reply })
                .await
                .map_err(|_| self.disconnected_error())?;
            self.await_boundary_reply(&receiver).await?
        };
        let expired = async {
            self.clock
                .sleep(deadline.saturating_duration_since(self.now()))
                .await;
            Err(super::observation_timeout(id))
        };
        future::or(delivered, expired).await
    }

    /// Requests cancellation of an admitted receipt's request. Owner tests
    /// observe the outcome on the receipt's terminal slot and a failed
    /// cancellation on the returned observer.
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) async fn cancel_test(
        &self,
        receipt: &ReceiptCore,
    ) -> Result<CancellationObserver, Error> {
        let (observer, cell) = CancellationObserver::pair();
        let deadline = self.deadline_after(Duration::from_secs(3600))?;
        self.request_cancellation(
            CancellationRequest {
                id: receipt.id,
                observer: cell,
            },
            deadline,
        )
        .await?;
        Ok(observer)
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
    ) -> Result<(TerminalObserver, flume::Receiver<Result<RequestId, Error>>), Error> {
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
        let (completion, observer) = TerminalObserver::pair();
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
    /// Executor-free source arbitration (#776): source phase and fairness, the
    /// control allowance, the receive-first raw-release proof shared with the
    /// blocking owner (#723), and the retained-boundary slot (#775). A field
    /// rather than a `run` local so `Drop` can answer a retained boundary.
    coordinator: OwnerCoordinator<RetainedBoundary>,
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
                coordinator: OwnerCoordinator::default(),
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
        self.coordinator
            .set_fairness_ceiling(self.state.policy().limits.frames_per_receive);
        // Enforced around each receive so the caller's advertised read timeout is
        // live on the async surface, where the runtime-agnostic transports have
        // no timer of their own (#675). A timed-out read consumed nothing, so it
        // is reported as an idle no-data receive.
        let read_timeout = self.state.policy().read_timeout;
        // Every arbitration decision below is the coordinator's; this loop only
        // samples the clock, polls the futures the plan describes, and runs
        // the selected event's engine turn.
        loop {
            if self.state.state() != SessionState::Running {
                break;
            }
            if self.coordinator.begin_turn() {
                // Polling boundaries first alone is not a cooperative handoff:
                // if they are all pending, the ready receive wins immediately
                // and this task can monopolize a single-thread executor.
                cooperative_yield().await;
            }
            // Sample after the cooperative yield: a timer that became due
            // while another task ran must not inherit a stale positive delay.
            let now = Executor::now(runtime.as_ref());
            let plan = self.coordinator.plan(
                now,
                self.state.next_wake(),
                self.state.raw_correlation_releases_due(now),
                || {
                    matches!(
                        driver.buffered_raw_prefix_evidence(),
                        Ok(Some(RawPrefixEvidence::Complete))
                    )
                },
            );

            let event = match plan {
                TurnPlan::Redeliver(retained) => {
                    // The retained boundary predates the just-cleared raw
                    // gate. Preserve shutdown's established priority without
                    // re-entering any channel ahead of it. A signal arriving
                    // after this nonblocking check races exactly as it did
                    // with an ordinary already-selected boundary; the final
                    // drain answers the retained payload if shutdown wins.
                    match self.shutdown.try_recv() {
                        Ok(()) => {
                            self.coordinator.restore(retained);
                            ActorEvent::Shutdown
                        }
                        Err(flume::TryRecvError::Empty | flume::TryRecvError::Disconnected) => {
                            match retained {
                                RetainedBoundary::Admission(admission) => {
                                    ActorEvent::Admission(Ok(admission))
                                }
                                RetainedBoundary::Cancellation(cancellation) => {
                                    ActorEvent::Cancellation(cancellation)
                                }
                            }
                        }
                    }
                }
                TurnPlan::Select(selection) => {
                    let frame_limit = self.state.policy().limits.frames_per_receive;
                    let boundaries = ActorBoundaryReceivers {
                        shutdown: &self.shutdown,
                        cancellations: &self.cancellations,
                        admissions: &self.admissions,
                        control: &self.control,
                    };
                    let receive = async {
                        if selection.receive == ReceiveArm::ProofComplete {
                            // The exact no-input probe for this release set is
                            // done: the receive source is immediately ready
                            // with the timer event instead of another read.
                            return ActorEvent::Wake;
                        }
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
                    let wake = async {
                        match selection.timer {
                            TimerArm::Grace { until } => {
                                raw_release_wake(runtime.as_ref(), until, now).await
                            }
                            TimerArm::Engine { at, due } => {
                                // Do not rely on a zero-duration executor
                                // sleep being ready on its first poll: the
                                // plan already established that this timer is
                                // due, and `ActorEvent::Wake` remains the only
                                // path that advances engine time.
                                if !due {
                                    let delay = at.map_or(Duration::from_secs(86_400), |wake| {
                                        wake.saturating_duration_since(now)
                                    });
                                    Executor::sleep(runtime.as_ref(), delay).await;
                                }
                                ActorEvent::Wake
                            }
                        }
                    };
                    select_actor_event(selection, receive, boundaries, wake).await
                }
            };

            // One sampled instant is carried through the selected event's
            // engine turn. If it is still before a hold deadline, that turn
            // cannot cross it merely because a later `Executor::now()` call
            // happens a few instructions later; the next plan then enters the
            // raw-release proof.
            let selected_at = Executor::now(runtime.as_ref());
            let selected = match &event {
                ActorEvent::Wake => Selected::Timer,
                ActorEvent::Admission(Ok(_)) | ActorEvent::Cancellation(_) => Selected::Boundary,
                ActorEvent::Receive {
                    result,
                    received_at,
                } => Selected::Receive {
                    due_at_receive: self.state.raw_correlation_releases_due(*received_at),
                    // A normalized idle fault is semantically identical to
                    // `NoData`: it consumed no bytes. A genuine transient
                    // fault cannot prove there is no ready stale frame
                    // behind it.
                    class: match result {
                        Ok(AsyncReceive::NoData) => ReceiveClass::NoInput,
                        Ok(AsyncReceive::Fault(error))
                            if super::receive_reported_no_data(error) =>
                        {
                            ReceiveClass::NoInput
                        }
                        Ok(AsyncReceive::Fault(_)) => ReceiveClass::TransientFault,
                        _ => ReceiveClass::Other,
                    },
                },
                _ => Selected::Other,
            };
            let disposition = self.coordinator.classify(
                selected,
                self.state.raw_correlation_releases_due(selected_at),
            );
            let (event, suppress_due) = match disposition {
                Disposition::Restart => continue,
                Disposition::Retain => match self.retain_selected_boundary(event, selected_at) {
                    None => {
                        self.coordinator.restart_after_retain();
                        continue;
                    }
                    // Unreachable while selection keeps boundaries ineligible
                    // behind an occupied slot; the boundary is applied rather
                    // than overwriting the retained one.
                    Some(event) => (event, false),
                },
                Disposition::Apply { suppress_due } => (event, suppress_due),
            };

            // `selected` describes the instant before the selection began. If
            // the timer matured while both tail futures were parked, the
            // left-biased control future can still win this poll. Re-sample
            // the engine before the control handler mutates it and charge that
            // control to the exact planned wake.
            let control_consumed_wake = if matches!(&event, ActorEvent::Control(_)) {
                let observed_at = Executor::now(runtime.as_ref());
                self.coordinator
                    .control_consumed_wake(self.state.next_wake(), observed_at)
            } else {
                None
            };
            let outcome = self
                .handle_event(
                    event,
                    &mut driver,
                    runtime.as_ref(),
                    selected_at,
                    suppress_due,
                )
                .await;
            if !self
                .coordinator
                .finish(selected, control_consumed_wake, outcome)
            {
                break;
            }
        }
        let boundary_error = self
            .state
            .boundary_error()
            .unwrap_or(Error::RuntimeShutdown);
        self.publish_terminal_error(boundary_error.clone());
        self.flush_pre_admission_rejections(true);
        if let Some(retained) = self.coordinator.take_retained() {
            self.answer_retained_boundary(retained, boundary_error.clone());
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
            ActorEvent::Wake => {
                let now = selected_at;
                // A due raw-correlation release is the last point at which
                // input retained by a byte-stream framer can still belong to
                // the old request. Receive-first selection above gives a
                // simultaneously ready completing tail precedence. If the
                // tail is not ready, remove the orphaned prefix before the due
                // pass can dispatch a successor; otherwise that successor
                // could consume the old frame after its tail arrives.
                let releases = self
                    .coordinator
                    .release_mut()
                    .observe(self.state.raw_correlation_releases_due(now))
                    .unwrap_or_default();
                if !releases.is_empty() {
                    // The selected receive proof belongs only to the exact
                    // set that was latched before this Wake. A second hold
                    // can become due while the selector is parked; do not let
                    // this Wake classify or advance that grown scope. The
                    // shared turn coordinator drops the old fence/grace and
                    // sends the replacement through its own receive-first
                    // turn, matching the blocking shell.
                    if self
                        .coordinator
                        .release_mut()
                        .replace_if_changed(self.state.raw_correlation_releases_due(now))
                    {
                        return TurnOutcome::Continue;
                    }
                    match self.state.resolve_retained_raw_input(driver, now) {
                        Ok(RawReleaseResolution::Advance) => {}
                        Ok(RawReleaseResolution::AwaitInputUntil(deadline)) => {
                            self.coordinator
                                .release_mut()
                                .wait_for_input_until(deadline);
                            return TurnOutcome::ContinueBuffered;
                        }
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
                    }
                }
                self.coordinator.release_mut().complete();
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
                    self.coordinator.release_mut().complete();
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
                            reason: super::transport_close_reason(&error),
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
    /// receive-fault pause. A retained-prefix grace supersedes an already-expired
    /// raw hold here: using that stale hold as a zero-duration clamp would make
    /// an immediately-idle custom transport spin before the real grace timer is
    /// selectable. The pause records no fault, does not clear an existing fault
    /// run, and spends no retry budget. Only a successful read or the five-second
    /// fault gap proves transient failures stopped accumulating.
    async fn absorb_idle_receive(
        &mut self,
        runtime: &R,
        buffered_stream_input: bool,
    ) -> TurnOutcome {
        let now = Executor::now(runtime);
        let deadline = self
            .coordinator
            .release_mut()
            .await_until()
            .filter(|deadline| *deadline > now)
            .or_else(|| self.state.next_wake());
        let pause = clamp_receive_pause(self.idle_receives.record(), deadline, now);
        if !pause.is_zero() {
            Executor::sleep(runtime, pause).await;
        }
        if buffered_stream_input {
            TurnOutcome::ContinueBuffered
        } else {
            TurnOutcome::YieldBoundaries
        }
    }

    /// Retain a boundary the coordinator classified as [`Disposition::Retain`],
    /// or answer it now. Returns `None` once the boundary is retained or
    /// answered. An admission whose claim expired is answered by the claim; a
    /// cancellation whose terminal result is already buffered is answered with
    /// it. Returns the event back only if the slot was unexpectedly occupied.
    fn retain_selected_boundary(
        &mut self,
        event: ActorEvent,
        selected_at: Instant,
    ) -> Option<ActorEvent> {
        let boundary = match event {
            ActorEvent::Admission(Ok(mut admission)) => {
                if !self.claim_admission_boundary(&mut admission, selected_at) {
                    return None;
                }
                RetainedBoundary::Admission(admission)
            }
            ActorEvent::Cancellation(cancellation) => {
                // An operation that already concluded has delivered its
                // terminal outcome, which answers the cancellation.
                if !self.state.is_active(cancellation.request.id) {
                    let _ = cancellation.reply.try_send(Ok(()));
                    return None;
                }
                RetainedBoundary::Cancellation(cancellation)
            }
            event => return Some(event),
        };
        match self.coordinator.retain(boundary) {
            Ok(()) => None,
            Err(RetainedBoundary::Admission(admission)) => {
                Some(ActorEvent::Admission(Ok(admission)))
            }
            Err(RetainedBoundary::Cancellation(cancellation)) => {
                Some(ActorEvent::Cancellation(cancellation))
            }
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
                let error = Error::admission_timeout();
                self.state.record_admission_rejection(target, lane, &error);
                let _ = admission.reply.try_send(Err(error));
                false
            }
            AdmissionClaim::ExpiredElsewhere => {
                let _ = admission.reply.try_send(Err(Error::admission_timeout()));
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
        let id = cancellation.request.id;
        if let Some(input) = self.state.begin_cancellation(cancellation.request) {
            let effects = self.state.input(input, accepted_at);
            self.drive(driver, effects, runtime).await;
        }
        // A refusal leaves the original request scheduled; the caller keeps
        // its handle and the operation's terminal slot (#612, #777).
        let _ = cancellation
            .reply
            .try_send(self.state.conclude_cancellation(id));
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
    fn answer_retained_boundary(&mut self, retained: RetainedBoundary, error: Error) {
        match retained {
            RetainedBoundary::Admission(admission) => {
                let _ = admission.reply.try_send(Err(error));
            }
            // The caller still holds its handle; a terminal outcome already
            // delivered to it takes precedence over this error (#777).
            RetainedBoundary::Cancellation(cancellation) => {
                let _ = cancellation.reply.try_send(Err(error));
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
                // The caller keeps its handle, and a terminal outcome already
                // delivered to it takes precedence over this error (#777).
                let _ = cancel.reply.try_send(Err(error.clone()));
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
        // A boundary retained behind a raw release when `run` unwound is no
        // longer in any channel; answer it before the queues.
        if let Some(retained) = self.coordinator.take_retained() {
            self.answer_retained_boundary(retained, error.clone());
        }
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
    Wake,
}

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
mod tests;
