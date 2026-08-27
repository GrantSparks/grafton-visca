//! Runtime-neutral async actor and its bounded control boundary.

use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
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
    AdmissionPermit, AppliedEffect, AppliedStateSubscription, CancellationCore, CompletionObserver,
    DecodedFrame, DiagnosticEvent, DiagnosticSubscription, Input, OwnerInputTurn, OwnerMetrics,
    OwnerPolicy, OwnerState, ReceiptCore, RequestId, RuntimeOutcome, RuntimeRequest, SessionState,
    ShutdownReason, TargetStateCache, TransmissionMeta, WaitSelection, WireWrite,
};
use crate::runtime::engine::{Effect, TransportKind};

/// Pause applied after a transient receive fault so a transport that fails
/// immediately cannot spin the actor. 1.x used the same bound.
const TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(10);

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

#[derive(Debug)]
struct AdmissionBoundary {
    request: RuntimeRequest,
    permit: AdmissionPermit,
    observer: Arc<super::ObserverCell>,
    reply: flume::Sender<Result<RequestId, Error>>,
}

#[derive(Debug)]
struct CancellationBoundary {
    receipt: ReceiptCore,
    reply: flume::Sender<Result<CancellationCore, Error>>,
}

#[derive(Debug)]
enum ControlBoundary {
    Snapshot(flume::Sender<OwnerSnapshot>),
    Metrics(flume::Sender<Result<crate::observability::MetricsSnapshot, Error>>),
    Subscribe {
        target: Option<crate::CameraId>,
        capacity: usize,
        reply: flume::Sender<Result<AppliedStateSubscription, Error>>,
    },
    SubscribeDiagnostics {
        capacity: usize,
        reply: flume::Sender<Result<DiagnosticSubscription, Error>>,
    },
}

/// Bounded diagnostic/metric copy safe to expose through a later public facade.
#[derive(Debug, Clone)]
pub(crate) struct OwnerSnapshot {
    pub(crate) metrics: OwnerMetrics,
    pub(crate) diagnostics: Vec<DiagnosticEvent>,
    pub(crate) state: SessionState,
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
    pub(crate) id: RequestId,
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

    pub(crate) async fn wait_with_timeout(
        self,
        control: AsyncReceiptControl,
        timeout: Duration,
    ) -> Result<(), Error> {
        wait_core_for(self.core, control, timeout)
            .await
            .and_then(normalize_command_outcome)
    }

    pub(crate) fn detach(self) {}
}

impl<T> AsyncInquiryReceipt<T> {
    pub(crate) async fn wait(self, control: AsyncReceiptControl) -> Result<T, Error> {
        let timeout = self.core.configured_timeout();
        let outcome = wait_core_for(self.core, control, timeout).await?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }

    pub(crate) async fn wait_with_timeout(
        self,
        control: AsyncReceiptControl,
        timeout: Duration,
    ) -> Result<T, Error> {
        let outcome = wait_core_for(self.core, control, timeout).await?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }

    pub(crate) fn detach(self) {}

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

    pub(crate) async fn cancel(self) -> Result<AsyncCancellationReceipt, Error> {
        self.owner.cancel_core(self.core).await
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
        let id = self.receipt.core.id();
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
                    id,
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

    pub(crate) const fn selection(&self) -> WaitSelection {
        self.selection
    }
}

impl ErasedAsyncSettlementWait {
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
    #[cfg(test)]
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
        wait_cancellation_until(self.core, &control.owner.clock, deadline)
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
    let completion = async { core.completion.recv_async().await.map(observation_outcome) };
    let timer = async {
        control.owner.clock.sleep(remaining).await;
        core.try_outcome().ok_or(Error::Timeout)
    };
    future::or(completion, timer).await
}

async fn wait_cancellation_until(
    mut core: CancellationCore,
    clock: &BoundClock,
    deadline: Instant,
) -> Result<ReceiptObservation, Error> {
    if let Some(observation) = core.try_observation() {
        return Ok(observation);
    }
    let remaining = deadline.saturating_duration_since(clock.now());
    let completion_observer = core.completion;
    let completion = async { completion_observer.recv_async().await };
    let timer = async {
        clock.sleep(remaining).await;
        completion_observer.try_recv().ok_or(Error::Timeout)
    };
    future::or(completion, timer).await
}

#[cfg(test)]
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
    shutdown_requested: Arc<AtomicBool>,
    terminal_error: Arc<Mutex<Option<Error>>>,
    origin: Arc<()>,
    clock: BoundClock,
    state_cache: Arc<[Mutex<TargetStateCache>; 9]>,
}

impl AsyncOwnerHandle {
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
        let (completion, admission) = self.enqueue_admission(request)?;
        match admission.recv_async().await {
            Ok(Ok(id)) => Ok(ReceiptCore::new(
                id,
                target,
                completion,
                configured_timeout,
                Arc::clone(&self.origin),
            )),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(self.disconnected_error()),
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
        let (completion, admission) = self.enqueue_admission(request)?;
        let remaining = deadline.saturating_duration_since(self.clock.now());
        let admitted = async {
            match admission.recv_async().await {
                Ok(result) => result,
                Err(_) => Err(self.disconnected_error()),
            }
        };
        let timed_out = async {
            self.clock.sleep(remaining).await;
            Err(Error::Timeout)
        };
        future::or(admitted, timed_out).await.map(|id| {
            ReceiptCore::new(
                id,
                target,
                completion,
                configured_timeout,
                Arc::clone(&self.origin),
            )
        })
    }

    /// Non-waiting admission used by capacity-sensitive facades. Failure occurs
    /// before an observer or engine ID is created.
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
        let (completion, admission) = self.enqueue_admission(request)?;
        Ok(async move {
            match admission.recv_async().await {
                Ok(Ok(id)) => Ok(ReceiptCore::new(
                    id,
                    target,
                    completion,
                    configured_timeout,
                    Arc::clone(&self.origin),
                )),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(self.disconnected_error()),
            }
        })
    }

    /// A full cancellation queue applies backpressure; cancellation is never
    /// discarded. Actor termination disconnects the sender and wakes all waits.
    async fn cancel_core(&self, receipt: ReceiptCore) -> Result<AsyncCancellationReceipt, Error> {
        if !Arc::ptr_eq(&receipt.origin, &self.origin) {
            return Err(Error::InvalidState(
                "operation receipt belongs to a different owner".into(),
            ));
        }
        if let Some(observation) = receipt.completion.try_recv() {
            return Ok(AsyncCancellationReceipt {
                core: cancellation_receipt_for(receipt, Some(observation)),
            });
        }
        let (reply, receiver) = flume::bounded(1);
        self.cancellations
            .send_async(CancellationBoundary { receipt, reply })
            .await
            .map_err(|_| self.disconnected_error())?;
        receiver
            .recv_async()
            .await
            .map_err(|_| self.disconnected_error())?
            .map(|core| AsyncCancellationReceipt { core })
    }

    #[cfg(test)]
    pub(crate) async fn cancel_test(
        &self,
        receipt: ReceiptCore,
    ) -> Result<AsyncCancellationReceipt, Error> {
        self.cancel_core(receipt).await
    }

    pub(crate) async fn snapshot(&self) -> Result<OwnerSnapshot, Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::Snapshot(reply))
            .await
            .map_err(|_| self.disconnected_error())?;
        receiver
            .recv_async()
            .await
            .map_err(|_| self.disconnected_error())
    }

    /// Reads scalar owner metrics through a dedicated bounded control request.
    /// This path never clones the diagnostic ring.
    pub(crate) async fn metrics(&self) -> Result<crate::observability::MetricsSnapshot, Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::Metrics(reply))
            .await
            .map_err(|_| self.disconnected_error())?;
        receiver
            .recv_async()
            .await
            .map_err(|_| self.disconnected_error())?
    }

    pub(crate) fn state_cache(&self, target: crate::CameraId) -> crate::state_cache::StateCache {
        crate::state_cache::StateCache::from_registry(Arc::clone(&self.state_cache), target)
    }

    pub(crate) async fn subscribe_applied(
        &self,
        target: Option<crate::CameraId>,
        capacity: usize,
    ) -> Result<AppliedStateSubscription, Error> {
        let (reply, receiver) = flume::bounded(1);
        self.control
            .send_async(ControlBoundary::Subscribe {
                target,
                capacity,
                reply,
            })
            .await
            .map_err(|_| self.disconnected_error())?;
        receiver
            .recv_async()
            .await
            .map_err(|_| self.disconnected_error())?
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
        receiver
            .recv_async()
            .await
            .map_err(|_| self.disconnected_error())?
    }

    /// Coalesced idempotent shutdown. Only the winning caller occupies the
    /// single shutdown slot.
    pub(crate) async fn shutdown(&self) -> Result<(), Error> {
        if self
            .shutdown_requested
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(());
        }
        self.shutdown
            .send_async(())
            .await
            .map_err(|_| self.disconnected_error())
    }

    fn enqueue_admission(
        &self,
        request: RuntimeRequest,
    ) -> Result<
        (
            CompletionObserver,
            flume::Receiver<Result<RequestId, Error>>,
        ),
        Error,
    > {
        if self.shutdown_requested.load(Ordering::Acquire) {
            return Err(self.disconnected_error());
        }
        let permit = self.permits.try_acquire().ok_or(Error::RuntimeQueueFull {
            capacity: self.permits.capacity(),
        })?;
        if self.shutdown_requested.load(Ordering::Acquire) {
            return Err(self.disconnected_error());
        }
        let (observer, completion) = completion_pair();
        let (reply, admission) = flume::bounded(1);
        let boundary = AdmissionBoundary {
            request,
            permit,
            observer,
            reply,
        };
        match self.admissions.try_send(boundary) {
            Ok(()) => Ok((completion, admission)),
            Err(flume::TrySendError::Disconnected(_)) => Err(self.disconnected_error()),
            Err(flume::TrySendError::Full(_)) => Err(Error::InvalidState(
                "admission channel full after permit reservation".into(),
            )),
        }
    }

    fn disconnected_error(&self) -> Error {
        self.terminal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or(Error::RuntimeShutdown)
    }
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
    terminal_error: Arc<Mutex<Option<Error>>>,
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
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let terminal_error = Arc::new(Mutex::new(None));
        Ok((
            AsyncOwnerHandle {
                permits,
                admissions: admission_tx,
                cancellations: cancellation_tx,
                control: control_tx,
                shutdown: shutdown_tx,
                shutdown_requested,
                terminal_error: Arc::clone(&terminal_error),
                origin,
                clock: clock.clone(),
                state_cache,
            },
            Self {
                state,
                admissions,
                cancellations,
                control,
                shutdown,
                terminal_error,
                runtime,
            },
        ))
    }

    pub(crate) const fn state(&self) -> &OwnerState {
        &self.state
    }

    pub(crate) async fn run<D>(mut self, mut driver: D) -> OwnerSnapshot
    where
        D: AsyncOwnerDriver,
    {
        let runtime = Arc::clone(&self.runtime);
        loop {
            if self.state.state() != SessionState::Running {
                break;
            }

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
                    let result = driver.receive(self.state.buffers(), frame_limit).await;
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
                future::or(
                    receive,
                    future::or(
                        shutdown,
                        future::or(
                            cancellation,
                            future::or(admission, future::or(control, wake)),
                        ),
                    ),
                )
                .await
            };

            if self
                .handle_event(event, &mut driver, runtime.as_ref())
                .await
            {
                break;
            }
        }
        let boundary_error = self
            .state
            .boundary_error()
            .unwrap_or(Error::RuntimeShutdown);
        *self
            .terminal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(boundary_error.clone());
        self.drain_boundaries(boundary_error);
        self.snapshot_now()
    }

    async fn handle_event<D>(&mut self, event: ActorEvent, driver: &mut D, runtime: &R) -> bool
    where
        D: AsyncOwnerDriver,
    {
        match event {
            ActorEvent::Shutdown => {
                self.terminate(driver, runtime, ShutdownReason::Explicit)
                    .await;
                true
            }
            ActorEvent::Cancellation(cancel) => {
                self.handle_cancellation(cancel, driver, runtime).await;
                false
            }
            ActorEvent::Control(control) => {
                self.handle_control(control);
                false
            }
            ActorEvent::Admission(Ok(admission)) => {
                self.handle_admission(admission, driver, runtime).await;
                false
            }
            ActorEvent::Admission(Err(_)) => {
                self.terminate(driver, runtime, ShutdownReason::Explicit)
                    .await;
                true
            }
            ActorEvent::Wake => {
                let effects = self.state.advance(Executor::now(runtime));
                self.drive(driver, effects, runtime).await;
                false
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
                true
            }
            ActorEvent::Receive {
                result: Ok(AsyncReceive::Frames(frames)),
                received_at,
            } => {
                if let Err(error) = self.state.validate_frame_batch(&frames) {
                    self.terminate_at(
                        driver,
                        runtime,
                        ShutdownReason::FramingFailure {
                            reason: error.to_string().into_boxed_str(),
                        },
                        received_at,
                    )
                    .await;
                    return true;
                }
                if frames.is_empty() {
                    // The read carried bytes that did not finish a frame. The
                    // framer holds the partial frame; keep pumping so the rest
                    // of it can arrive in a later read.
                    return false;
                }
                let turn = self.state.begin_input_turn(received_at);
                for frame in frames {
                    let effects = self.state.input_in_turn(&turn, Input::Frame(frame));
                    self.drive_in_turn(driver, &turn, effects).await;
                }
                let due = self.state.finish_input_turn(turn);
                self.drive(driver, due, runtime).await;
                false
            }
            ActorEvent::Receive {
                result: Ok(AsyncReceive::Fault(error)),
                received_at,
            } => {
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
                    return true;
                }
                // 1.x parity: retry every command still waiting for its ACK and
                // keep the session. The pause mirrors 1.x's own guard against
                // hot-looping on a transport that fails immediately.
                let effects = self.state.input(Input::ReceiveFault { error }, received_at);
                self.drive(driver, effects, runtime).await;
                Executor::sleep(runtime, TRANSIENT_RECEIVE_PAUSE).await;
                false
            }
            ActorEvent::Receive {
                result: Err(error),
                received_at,
            } => {
                let reason = error.to_string().into_boxed_str();
                let shutdown = if self.state.policy().protocol.transport == TransportKind::Stream {
                    ShutdownReason::FramingFailure { reason }
                } else {
                    ShutdownReason::TransportClosed {
                        reason: Some(reason),
                    }
                };
                self.terminate_at(driver, runtime, shutdown, received_at)
                    .await;
                true
            }
        }
    }

    async fn handle_admission<D>(
        &mut self,
        admission: AdmissionBoundary,
        driver: &mut D,
        runtime: &R,
    ) where
        D: AsyncOwnerDriver,
    {
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
        let result = match registration.acknowledgement.try_recv() {
            Ok(Ok(())) => Ok(cancellation_receipt_for(cancellation.receipt, None)),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Error::InvalidState(
                "engine did not acknowledge cancellation input".into(),
            )),
        };
        let _ = cancellation.reply.try_send(result);
    }

    fn handle_control(&mut self, control: ControlBoundary) {
        match control {
            ControlBoundary::Snapshot(reply) => {
                let _ = reply.try_send(self.snapshot_now());
            }
            ControlBoundary::Metrics(reply) => {
                let _ = reply.try_send(Ok(self.state.metrics_snapshot()));
            }
            ControlBoundary::Subscribe {
                target,
                capacity,
                reply,
            } => {
                let result = self.state.subscribe_applied(target, capacity);
                let _ = reply.try_send(result);
            }
            ControlBoundary::SubscribeDiagnostics { capacity, reply } => {
                let result = self.state.subscribe_diagnostics(capacity);
                let _ = reply.try_send(result);
            }
        }
    }

    async fn drive<D>(&mut self, driver: &mut D, mut effects: VecDeque<Effect>, runtime: &R)
    where
        D: AsyncOwnerDriver,
    {
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = self.state.apply_effect(effect) {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => driver.write(write).await,
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
    ) where
        D: AsyncOwnerDriver,
    {
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = self.state.apply_effect(effect) {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => driver.write(write).await,
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

    fn drain_boundaries(&mut self, error: Error) {
        let mut dropped = 0usize;
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
                None => Err(error.clone()),
            };
            let _ = cancel.reply.try_send(result);
            dropped = dropped.saturating_add(1);
        }
        while let Ok(control) = self.control.try_recv() {
            match control {
                ControlBoundary::Snapshot(reply) => {
                    let _ = reply.try_send(self.snapshot_now());
                }
                ControlBoundary::Metrics(reply) => {
                    let _ = reply.try_send(Err(error.clone()));
                }
                ControlBoundary::Subscribe { reply, .. } => {
                    let _ = reply.try_send(Err(error.clone()));
                }
                ControlBoundary::SubscribeDiagnostics { reply, .. } => {
                    let _ = reply.try_send(Err(error.clone()));
                }
            }
            dropped = dropped.saturating_add(1);
        }
        self.state.fail_unstaged_boundary(dropped);
    }

    fn snapshot_now(&self) -> OwnerSnapshot {
        OwnerSnapshot {
            metrics: self.state.metrics(),
            diagnostics: self.state.diagnostics().copied().collect(),
            state: self.state.state(),
            active: self.state.active_len(),
        }
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

    use std::sync::Mutex;

    use crate::runtime::Runtime;
    use crate::{
        runtime::engine::{
            CancellationPolicy, ControlPolicy, DecodedResponse, EncodedMessage, EnvelopeKind,
            ProtocolPolicy, RequestContext, RetryPolicy, RuntimeRequest, TargetPolicy,
            TimeoutPolicy, TransportKind,
        },
        CameraId, ViscaSocket,
    };

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
            },
            CameraId::CAMERA_1,
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap()
    }

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
            },
            applied_state: None,
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
        )
        .unwrap()
    }

    fn prepared_zoom(
        profile: &crate::ProfileSpec,
    ) -> crate::prepared::PreparedOperation<completion::Targeted> {
        crate::prepared::prepare_builtin_operation::<completion::Targeted, _>(
            &crate::request::builtin::ZoomTarget::new(
                crate::types::ZoomPosition::new(0x0100).unwrap(),
            ),
            CameraId::CAMERA_1,
            profile,
            crate::OperationalTuning::new(),
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
        let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let first = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), first.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
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
            .send_async(Ok(TransmissionMeta { sequence: None }))
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
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let actor_task = tokio::spawn(actor.run(harness.driver));

        let receipt = handle.submit(retrying_command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
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
}
