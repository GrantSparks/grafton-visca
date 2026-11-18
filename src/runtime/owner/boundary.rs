//! Executor-free boundary between owner handles and their serialized owner.
//!
//! Everything here is pure data and lock-based bookkeeping: the boundary
//! messages a handle enqueues, the one-way admission-deadline race, the
//! coalescing pre-admission rejection ingress, the shared lifecycle
//! linearization point, and [`OwnerHandleCore`], the cloneable half every
//! handle shares. Nothing touches an executor, a transport or the clock, so a
//! shell supplies only its native way of waiting on the channels (D24, #780).

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::Instant,
};

use crate::{
    runtime::engine::MotionEffect,
    target::{PerTarget, TargetSlot},
    Error,
};

#[cfg(all(test, feature = "runtime-tokio"))]
use super::OwnerMetrics;
#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
use super::SessionState;
use super::{
    AdmissionPermit, AdmissionPermitPool, Admitted, AdmittedOperation, CancellationRequest,
    CommandOutcome, DiagnosticSubscription, Lane, LaneRequest, LiveTuning, OwnerState,
    RuntimeRequest, TargetStateCache, TerminalLane, TerminalObserver, TerminalSlot,
};

/// The one-way decision for an admission constrained by an outer deadline.
///
/// The caller and owner race only to decide whether this boundary crosses the
/// admission boundary. Once the owner claims it, a caller at its deadline must
/// wait for that already-admitted reply and may later detach its observer by
/// the ordinary receipt path. Once the caller expires it, the owner must drop
/// the queued boundary without touching engine state.
#[derive(Debug, Clone)]
pub(super) struct AdmissionValidity {
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

/// The owner's linearized answer when it reaches a deadline-constrained
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AdmissionClaim {
    Claimed {
        remaining: crate::runtime::engine::NonZeroDuration,
    },
    /// This owner observed the deadline first and rejected the boundary.
    ExpiredHere,
    /// The caller had already rejected the boundary before this owner turn.
    ExpiredElsewhere,
}

impl AdmissionValidity {
    const PENDING: u8 = AdmissionValidityState::Pending as u8;
    const CLAIMED: u8 = AdmissionValidityState::Claimed as u8;
    const EXPIRED: u8 = AdmissionValidityState::Expired as u8;

    pub(super) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn until(deadline: Instant) -> Self {
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
    pub(super) fn claim_for_admission(&self, now: Instant) -> AdmissionClaim {
        // Saturation is zero exactly when the sampled instant has reached
        // the deadline. A successful claim carries this proved budget.
        let Ok(remaining) = crate::runtime::engine::NonZeroDuration::new(
            self.deadline.saturating_duration_since(now),
        ) else {
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
        };

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
            AdmissionClaim::Claimed { remaining }
        } else {
            AdmissionClaim::ExpiredElsewhere
        }
    }

    /// Mark the boundary expired if no owner has already claimed admission.
    ///
    /// The boolean is the linearized answer to the caller's timeout race:
    /// `true` means it may return an admission timeout; `false` means an admitted
    /// reply is authoritative and still has to be observed.
    pub(super) fn expire_before_admission(&self) -> bool {
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

/// The owner's answer to one enqueued admission: the admitted request's
/// identity, or why it was not admitted.
pub(super) type AdmissionReply = flume::Receiver<Result<Admitted, Error>>;

/// A deadline-constrained admission a caller has enqueued, with what it
/// must record if its deadline wins the race.
#[derive(Debug)]
pub(super) struct AdmissionExpiry {
    validity: AdmissionValidity,
    target: TargetSlot,
    lane: Lane,
}

/// One admission a caller enqueued, awaiting the owner's reply.
///
/// It turns that reply into the receipt core, so both owner shells build
/// receipts the same way and differ only in how they wait for the reply.
#[derive(Debug)]
pub(super) struct PendingAdmission<T> {
    completion: TerminalObserver<T>,
    reply: AdmissionReply,
    expiry: Option<AdmissionExpiry>,
}

impl<T: TerminalLane> PendingAdmission<T> {
    /// The receiver of the owner's admission reply.
    pub(super) fn reply(&self) -> &AdmissionReply {
        &self.reply
    }

    /// Settles a caller deadline that expired while the admission was
    /// queued; see [`OwnerHandleCore::expire_admission`]. An admission
    /// without a deadline never expires.
    pub(super) fn expire(&self, core: &OwnerHandleCore) -> Result<(), Error> {
        match &self.expiry {
            Some(expiry) => core.expire_admission(expiry),
            None => Ok(()),
        }
    }

    /// The receipt core for the owner's `reply`, observed under
    /// `configured_timeout`.
    pub(super) fn receipt(
        self,
        reply: Result<Result<Admitted, Error>, Error>,
        configured_timeout: std::time::Duration,
    ) -> Result<super::ReceiptCore<T>, Error> {
        super::ReceiptCore::admitted(reply??, self.completion, configured_timeout)
    }
}

pub(super) type QueuedAdmission<T, R> = (TerminalObserver<T>, flume::Receiver<Result<R, Error>>);

#[derive(Debug)]
pub(crate) enum AdmissionReplySink {
    Request(flume::Sender<Result<Admitted, Error>>),
    Operation {
        motion: MotionEffect,
        reply: flume::Sender<Result<AdmittedOperation, Error>>,
    },
}

/// Select the owner's reply and the receipt's settlement storage together.
pub(crate) trait SettlementAdmission: std::fmt::Debug + Sized + Send + Sync + Unpin {
    type Admission: Send;
    type Observation: std::fmt::Debug + Send + Sync + Unpin;

    fn reply(
        motion: MotionEffect,
        sender: flume::Sender<Result<Self::Admission, Error>>,
    ) -> AdmissionReplySink;
    fn observe(admitted: Self::Admission) -> (Admitted, Self::Observation);
}

impl SettlementAdmission for () {
    type Admission = Admitted;
    type Observation = ();

    fn reply(
        _motion: MotionEffect,
        sender: flume::Sender<Result<Admitted, Error>>,
    ) -> AdmissionReplySink {
        // The request already carries the motion effect. Plain admission
        // publishes its generations under the lock and returns only the ID.
        AdmissionReplySink::Request(sender)
    }

    fn observe(admitted: Admitted) -> (Admitted, Self::Observation) {
        (admitted, ())
    }
}

impl SettlementAdmission for crate::completion::TargetedSettlementPlan {
    type Admission = AdmittedOperation;
    type Observation = super::TargetedObservation;

    fn reply(
        motion: MotionEffect,
        reply: flume::Sender<Result<AdmittedOperation, Error>>,
    ) -> AdmissionReplySink {
        AdmissionReplySink::Operation { motion, reply }
    }

    fn observe(admitted: AdmittedOperation) -> (Admitted, Self::Observation) {
        (admitted.admitted, Self::Observation::new(admitted.motion))
    }
}

impl AdmissionReplySink {
    #[cfg(all(test, any(feature = "blocking", feature = "test-utils")))]
    #[allow(clippy::panic)]
    pub(super) fn into_request(self) -> flume::Sender<Result<Admitted, Error>> {
        match self {
            Self::Request(reply) => reply,
            Self::Operation { .. } => panic!("a plain request admission was expected"),
        }
    }

    fn reject(self, error: Error) {
        match self {
            Self::Request(reply) => {
                let _ = reply.try_send(Err(error));
            }
            Self::Operation { reply, .. } => {
                let _ = reply.try_send(Err(error));
            }
        }
    }
}

/// The common observation check can borrow either a sender directly or a
/// request admission's typed reply.
pub(super) trait BoundaryReply {
    fn is_disconnected(&self) -> bool;
}

impl<T> BoundaryReply for flume::Sender<T> {
    fn is_disconnected(&self) -> bool {
        flume::Sender::is_disconnected(self)
    }
}

impl BoundaryReply for AdmissionReplySink {
    fn is_disconnected(&self) -> bool {
        match self {
            Self::Request(sender) => sender.is_disconnected(),
            Self::Operation { reply: sender, .. } => sender.is_disconnected(),
        }
    }
}

#[derive(Debug)]
pub(super) struct AdmissionBoundary {
    request: RuntimeRequest,
    permit: AdmissionPermit,
    observer: TerminalSlot,
    reply: AdmissionReplySink,
    claim: AdmissionBoundaryClaim,
}

/// Why an admission failed its boundary claim, and which side owns telemetry.
#[derive(Debug)]
pub(super) struct AdmissionRejection {
    pub(super) error: Error,
    pub(super) record: bool,
}

/// A claimed boundary's inputs and its one admission answer.
pub(super) type AdmissionParts = (
    RuntimeRequest,
    AdmissionPermit,
    TerminalSlot,
    AdmissionReplySink,
);

impl AdmissionBoundary {
    pub(super) const fn request(&self) -> &RuntimeRequest {
        &self.request
    }

    /// Claim the deadline race while the boundary still owns its permit.
    pub(super) fn claim_for_admission(&mut self, now: Instant) -> Result<(), AdmissionRejection> {
        match &self.claim {
            AdmissionBoundaryClaim::Pending(claim) => {
                claim.claim(&self.reply, now)?;
                self.claim = AdmissionBoundaryClaim::Claimed;
                Ok(())
            }
            AdmissionBoundaryClaim::Unbounded | AdmissionBoundaryClaim::Claimed => {
                BoundaryClaim::ensure_observed(&self.reply, None)
            }
        }
    }

    /// Release admission capacity before the rejection wakes its receiver.
    pub(super) fn reject(self, error: Error) {
        let Self {
            request,
            permit,
            observer,
            reply,
            claim,
        } = self;
        drop(permit);
        reply.reject(error);
        // The admission answer remains authoritative over a dropped slot.
        drop(request);
        drop(observer);
        drop(claim);
    }

    /// Transfer a claimed boundary into the engine's consuming admission path.
    pub(super) fn into_parts(self) -> AdmissionParts {
        (self.request, self.permit, self.observer, self.reply)
    }
}

/// The owner's one-way answer to the pre-admission deadline race.
#[derive(Debug)]
enum AdmissionBoundaryClaim {
    /// No caller deadline constrains admission.
    Unbounded,
    /// The owner has not yet claimed the caller's bounded admission.
    Pending(BoundaryClaim),
    /// The owner won the race before retaining the boundary behind raw input.
    Claimed,
}

/// The shared pre-admission claim for bounded submissions and halts.
#[derive(Debug)]
pub(super) struct BoundaryClaim(AdmissionValidity);

impl From<AdmissionValidity> for BoundaryClaim {
    fn from(validity: AdmissionValidity) -> Self {
        Self(validity)
    }
}

impl BoundaryClaim {
    pub(super) const fn deadline(&self) -> Instant {
        self.0.deadline()
    }

    fn ensure_observed(
        reply: &impl BoundaryReply,
        pending: Option<&Self>,
    ) -> Result<(), AdmissionRejection> {
        if reply.is_disconnected() {
            // A caller-expired boundary already reported its rejection.
            return Err(AdmissionRejection {
                error: Error::CommandCancelled,
                record: pending.is_none_or(|claim| claim.0.expire_before_admission()),
            });
        }
        Ok(())
    }

    pub(super) fn claim(
        &self,
        reply: &impl BoundaryReply,
        now: Instant,
    ) -> Result<crate::runtime::engine::NonZeroDuration, AdmissionRejection> {
        Self::ensure_observed(reply, Some(self))?;
        match self.0.claim_for_admission(now) {
            AdmissionClaim::Claimed { remaining } => Ok(remaining),
            expired @ (AdmissionClaim::ExpiredHere | AdmissionClaim::ExpiredElsewhere) => {
                Err(AdmissionRejection {
                    error: Error::admission_timeout(),
                    record: matches!(expired, AdmissionClaim::ExpiredHere),
                })
            }
        }
    }
}

/// A rejection that happened before a handle could allocate any owner-owned
/// admission state.
#[derive(Debug, Clone, Copy)]
pub(super) struct PreAdmissionRejection {
    pub(super) target: TargetSlot,
    pub(super) lane: Lane,
    pub(super) error: crate::ErrorKind,
}

#[derive(Debug)]
struct PendingAdmissionRejections {
    events: VecDeque<PreAdmissionRejection>,
    /// Events evicted before the owner could deliver them to subscribers.
    /// The owner reports this as `dropped_staged_diagnostics`.
    dropped: u64,
    /// A one-slot wake-up is already queued or being handled by the owner.
    /// This is protected by the same mutex as `events`, so a concurrent
    /// reporter can never lose the wake-up between an owner drain and its next
    /// empty receive.
    wake_pending: bool,
}

/// Bounded, coalescing ingress for failures that occur on cloneable handles
/// before an `AdmissionBoundary` exists.
///
/// The owner remains the only diagnostic delivery and owner-metric writer.
/// Handles merely record a compact event and wake it. The scalar total is
/// atomic so a diagnostic ingress burst cannot undercount rejections when its
/// bounded event queue coalesces before the owner gets a turn.
#[derive(Debug)]
pub(super) struct AdmissionRejectionIngress {
    total: AtomicU64,
    /// The urgent stops among `total` that found their target's control
    /// reserve full (D26, #778).
    control_reserve_total: AtomicU64,
    pending: Mutex<PendingAdmissionRejections>,
    capacity: usize,
}

/// Adds one to a saturating atomic counter.
fn saturating_increment(counter: &AtomicU64) {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        match counter.compare_exchange_weak(
            current,
            current.saturating_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

impl AdmissionRejectionIngress {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            total: AtomicU64::new(0),
            control_reserve_total: AtomicU64::new(0),
            pending: Mutex::new(PendingAdmissionRejections {
                events: VecDeque::with_capacity(capacity),
                dropped: 0,
                wake_pending: false,
            }),
            capacity,
        }
    }

    /// Records one rejection and reports whether this caller must enqueue the
    /// one coalesced owner wake-up.
    pub(super) fn record(&self, event: PreAdmissionRejection, control_reserve: bool) -> bool {
        saturating_increment(&self.total);
        if control_reserve {
            saturating_increment(&self.control_reserve_total);
        }
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pending.events.len() == self.capacity {
            // Preserve the newest bounded pre-admission observations and
            // rejection facts, which are the useful ones during saturation,
            // while the atomic total remains exact. The owner reports the
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

    pub(super) fn total(&self) -> u64 {
        self.total.load(Ordering::Acquire)
    }

    pub(super) fn control_reserve_total(&self) -> u64 {
        self.control_reserve_total.load(Ordering::Acquire)
    }

    /// Moves pending bounded diagnostics into the owner's preallocated scratch
    /// queue. When `consumed_wake` is true, clearing the wake marker occurs
    /// under this same lock, closing the report/drain race.
    pub(super) fn drain_into(
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

/// The bounded priority lane shared by cancellation and owner halt.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(super) enum CancellationBoundary {
    Cancel {
        request: CancellationRequest,
        reply: flume::Sender<Result<(), Error>>,
    },
    Halt(super::halt::HaltBoundary),
}
impl CancellationBoundary {
    pub(super) fn fail(self, error: Error) {
        match self {
            Self::Cancel { reply, .. } => {
                let _ = reply.try_send(Err(error));
            }
            Self::Halt(halt) => {
                let _ = halt.reply.try_send(Err(error));
            }
        }
    }
}

/// An admission or cancellation selected while a raw-correlation release was
/// due. Its ordinary input turn would run due work before the release proof,
/// so the owner coordinator retains it in a single slot until the release
/// resolves. Selection makes cancellation and admission ineligible while the
/// slot is occupied, so a second boundary stays in its bounded channel
/// instead of replacing this one (#775).
///
/// `AdmissionBoundary` keeps its request inline so accepting an admission does
/// not add a heap allocation at the owner-channel boundary.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(super) enum RetainedBoundary {
    Admission(AdmissionBoundary),
    Cancellation(CancellationBoundary),
}

#[derive(Debug)]
pub(super) enum ControlBoundary {
    /// Internal coalesced wake-up for a handle-side rejection that happened
    /// before an admission boundary existed. It uses the existing bounded
    /// control lane so source arbitration remains unchanged.
    FlushAdmissionRejections,
    // Built only by `AsyncOwnerHandle::snapshot`, called only from the async
    // actor's tests (#636).
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    Snapshot(flume::Sender<OwnerSnapshot>),
    Metrics(flume::Sender<Result<crate::observability::MetricsSnapshot, Error>>),
    SubscribeDiagnostics {
        capacity: crate::observability::DiagnosticCapacity,
        reply: flume::Sender<Result<DiagnosticSubscription, Error>>,
    },
    /// Installs new session tuning on the live owner (#631).
    ///
    /// This lane is what makes the update serialized: the owner is the only
    /// writer of the shared tuning cell, so two handles reconfiguring at the
    /// same time resolve last-writer-wins in the order the owner accepted them
    /// and no reader ever observes a mixture of the two. Facade validation is
    /// carried in the same message so a terminal owner selects its retained
    /// cause before returning a proposed update's validation error (#690).
    Reconfigure {
        validated_tuning: Box<Result<crate::session_config::ValidatedTuning, Error>>,
        reply: flume::Sender<Result<(), Error>>,
    },
}

/// Bounded diagnostic/metric copy returned by the owner.
#[derive(Debug, Clone)]
pub(crate) struct OwnerSnapshot {
    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) metrics: OwnerMetrics,
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) state: SessionState,
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) active: usize,
}

/// Metrics are trusted only once normal conclusion has drained boundary work.
/// An unwind before that point cannot publish an engine snapshot.
// One bounded scalar copy per owner stays inline, including during teardown.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Copy)]
enum TerminalMetrics {
    Pending,
    Published(crate::observability::MetricsSnapshot),
    Unavailable,
}

/// One handle-side lifecycle fact, including shutdown acceptance and the
/// owner's authoritative terminal verdict and metrics.
#[derive(Debug, Clone)]
enum LifecycleState {
    Open,
    ShutdownAccepted,
    SignalFailed(Error),
    ShuttingDown {
        signal_accepted: bool,
        metrics: TerminalMetrics,
    },
    ShutDown {
        metrics: TerminalMetrics,
    },
    Terminated {
        error: Error,
        metrics: TerminalMetrics,
    },
}

impl LifecycleState {
    fn terminal(&self) -> Option<Error> {
        match self {
            Self::ShuttingDown { .. } | Self::ShutDown { .. } => Some(Error::RuntimeShutdown),
            Self::Terminated { error, .. } => Some(error.clone()),
            Self::Open | Self::ShutdownAccepted | Self::SignalFailed(_) => None,
        }
    }

    fn disconnected_error(&self) -> Error {
        self.terminal().unwrap_or_else(missing_terminal_error)
    }

    fn metrics(&self) -> Option<Result<crate::observability::MetricsSnapshot, Error>> {
        match self {
            Self::ShuttingDown { metrics, .. }
            | Self::ShutDown { metrics }
            | Self::Terminated { metrics, .. } => match metrics {
                TerminalMetrics::Pending => None,
                TerminalMetrics::Published(snapshot) => Some(Ok(*snapshot)),
                TerminalMetrics::Unavailable => Some(Err(missing_terminal_error())),
            },
            Self::Open | Self::ShutdownAccepted | Self::SignalFailed(_) => None,
        }
    }

    /// Close boundary admission as soon as the engine ends. An orderly
    /// shutdown is provisional until the driver has been released.
    fn publish(&mut self, error: Error) {
        *self = match (&*self, error) {
            (Self::ShuttingDown { .. } | Self::ShutDown { .. } | Self::Terminated { .. }, _) => {
                return;
            }
            (Self::SignalFailed(error), _) => Self::Terminated {
                error: error.clone(),
                metrics: TerminalMetrics::Pending,
            },
            (state, Error::RuntimeShutdown) => Self::ShuttingDown {
                signal_accepted: matches!(state, Self::ShutdownAccepted),
                metrics: TerminalMetrics::Pending,
            },
            (_, error) => Self::Terminated {
                error,
                metrics: TerminalMetrics::Pending,
            },
        };
    }

    fn driver_released(&mut self) {
        if let Self::ShuttingDown {
            signal_accepted,
            metrics,
        } = *self
        {
            *self = if signal_accepted {
                Self::ShutDown { metrics }
            } else {
                Self::Terminated {
                    error: Error::RuntimeShutdown,
                    metrics,
                }
            };
        }
    }

    fn publish_metrics(
        &mut self,
        snapshot: crate::observability::MetricsSnapshot,
    ) -> Result<crate::observability::MetricsSnapshot, Error> {
        match self {
            Self::ShuttingDown { metrics, .. }
            | Self::ShutDown { metrics }
            | Self::Terminated { metrics, .. } => match metrics {
                TerminalMetrics::Pending => {
                    *metrics = TerminalMetrics::Published(snapshot);
                    Ok(snapshot)
                }
                TerminalMetrics::Published(published) => Ok(*published),
                TerminalMetrics::Unavailable => Err(missing_terminal_error()),
            },
            _ => Err(missing_terminal_error()),
        }
    }

    fn fail_closed(&mut self) -> Error {
        match *self {
            Self::ShuttingDown { metrics, .. } => {
                *self = Self::Terminated {
                    error: missing_terminal_error(),
                    metrics,
                };
            }
            _ => self.publish(missing_terminal_error()),
        }
        match self {
            Self::ShutDown { metrics } | Self::Terminated { metrics, .. }
                if matches!(metrics, TerminalMetrics::Pending) =>
            {
                *metrics = TerminalMetrics::Unavailable;
            }
            _ => {}
        }
        self.disconnected_error()
    }
}

/// The lifecycle facts a session's handles and its owner share.
///
/// Terminal publication, shutdown acceptance and admission enqueue all take
/// the lifecycle lock, so they have one linearization point. Without it, a
/// shutdown caller could observe `Open`, enqueue after the owner had already
/// terminated, and return `Ok(())` even though no owner turn could ever
/// consume the signal; an admission could likewise strand its permit in a
/// queue whose receiver will never poll it (#542 §4).
#[derive(Debug)]
pub(super) struct OwnerLifecycle {
    state: Mutex<LifecycleState>,
    next_submission: AtomicU64,
}

impl OwnerLifecycle {
    fn new() -> Self {
        Self {
            state: Mutex::new(LifecycleState::Open),
            next_submission: AtomicU64::new(1),
        }
    }

    fn allocate_order(&self) -> Result<u64, Error> {
        self.next_submission
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Error::RuntimeIdentityExhausted)
    }

    pub(super) fn fence_order(
        &self,
        motion: &Arc<super::motion::MotionRegistry>,
        target: TargetSlot,
        axes: Option<crate::AffectedAxes>,
    ) -> Result<u64, Error> {
        let _guard = self.lock_open()?;
        let cutoff = self.allocate_order()?;
        // Lock order is ingress -> motion. No other path holds both, and no
        // I/O occurs here: enqueue and settlement commit see one acceptance.
        if let Some(axes) = axes {
            motion.fence(target, axes, cutoff);
        }
        Ok(cutoff)
    }

    fn lock_state(&self) -> MutexGuard<'_, LifecycleState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The published terminal error, if the owner has ended.
    pub(super) fn terminal(&self) -> Option<Error> {
        self.lock_state().terminal()
    }

    /// Preserve an owner's known stop reason when its engine never ended.
    /// An accepted shutdown alone is no evidence that shutdown completed.
    pub(super) fn conclusion_error(&self) -> Error {
        match &*self.lock_state() {
            LifecycleState::SignalFailed(error) | LifecycleState::Terminated { error, .. } => {
                error.clone()
            }
            LifecycleState::ShuttingDown { .. } | LifecycleState::ShutDown { .. } => {
                Error::RuntimeShutdown
            }
            LifecycleState::Open | LifecycleState::ShutdownAccepted => missing_terminal_error(),
        }
    }

    fn metrics(&self) -> Option<Result<crate::observability::MetricsSnapshot, Error>> {
        self.lock_state().metrics()
    }

    pub(super) fn publish_metrics(
        &self,
        snapshot: crate::observability::MetricsSnapshot,
    ) -> Result<crate::observability::MetricsSnapshot, Error> {
        self.lock_state().publish_metrics(snapshot)
    }

    /// Hold the lifecycle lock while the session still accepts boundary work,
    /// or return the error that rejects it. Holding the returned guard across
    /// an enqueue keeps that enqueue ordered before any terminal publication.
    fn lock_open(&self) -> Result<MutexGuard<'_, LifecycleState>, Error> {
        let state = self.lock_state();
        match &*state {
            LifecycleState::Open => Ok(state),
            LifecycleState::ShutdownAccepted
            | LifecycleState::ShuttingDown { .. }
            | LifecycleState::ShutDown { .. } => Err(Error::RuntimeShutdown),
            LifecycleState::SignalFailed(error) | LifecycleState::Terminated { error, .. } => {
                Err(error.clone())
            }
        }
    }

    /// Publish a terminal engine verdict at the lifecycle linearization point.
    ///
    /// The owner calls this while the terminal `SessionEnded` effect is
    /// being driven, rather than only once its loop has ended. A shutdown
    /// caller can otherwise enter the still-live one-slot lane after the
    /// engine has become terminal but before the loop's epilogue, and
    /// incorrectly receive `Ok(())`.
    pub(super) fn publish_terminal(&self, error: Error) {
        self.lock_state().publish(error);
    }

    pub(super) fn driver_released(&self) {
        self.lock_state().driver_released();
    }

    /// Fail closed when an owner is torn down, including by an unwind before
    /// it published a terminal result, and return the terminal error its
    /// remaining boundary work must be answered with.
    ///
    /// Publishing before the final drain keeps boundary callers from
    /// retaining an admission permit after an owner panic, and makes a
    /// concurrent shutdown observe rejection rather than accept a signal that
    /// no receiver can poll.
    pub(super) fn fail_closed(&self) -> Error {
        self.lock_state().fail_closed()
    }
}

/// The error for an owner that disappeared without publishing its terminal
/// result, which only an unwind can cause.
pub(super) fn missing_terminal_error() -> Error {
    Error::OwnerTerminated
}

/// The owner's ends of the bounded boundary lanes, one per source the
/// coordinator arbitrates.
#[derive(Debug)]
pub(super) struct BoundaryReceivers {
    pub(super) shutdown: flume::Receiver<()>,
    pub(super) cancellations: flume::Receiver<CancellationBoundary>,
    /// A dequeue may let a halt enqueue under the lifecycle lock.
    pub(super) halt_capacity: flume::Sender<()>,
    pub(super) admissions: flume::Receiver<AdmissionBoundary>,
    pub(super) control: flume::Receiver<ControlBoundary>,
}

/// Everything the owner keeps of the boundary [`OwnerHandleCore::new`]
/// creates.
#[derive(Debug)]
pub(super) struct OwnerEnds {
    pub(super) receivers: BoundaryReceivers,
    pub(super) admission_rejections: Arc<AdmissionRejectionIngress>,
    /// Dropped only once the owner has dropped its driver, disconnecting every
    /// handle's `actor_alive` receiver. Nothing is ever sent on it (#626).
    pub(super) alive: flume::Sender<()>,
    pub(super) lifecycle: Arc<OwnerLifecycle>,
}

/// The executor-free half of every owner handle. Admission, cancellation,
/// ordinary control and shutdown each have separate bounded lanes; a shell
/// adds only its native way of waiting on them.
#[derive(Debug, Clone)]
pub(super) struct OwnerHandleCore {
    pub(super) permits: AdmissionPermitPool,
    pub(super) admissions: flume::Sender<AdmissionBoundary>,
    admission_rejections: Arc<AdmissionRejectionIngress>,
    pub(super) cancellations: flume::Sender<CancellationBoundary>,
    pub(super) halt_capacity: flume::Receiver<()>,
    pub(super) control: flume::Sender<ControlBoundary>,
    shutdown: flume::Sender<()>,
    /// Disconnects after owner teardown, including driver/transport drop.
    /// Nothing is ever sent on it.
    pub(super) actor_alive: flume::Receiver<()>,
    lifecycle: Arc<OwnerLifecycle>,
    state_cache: Arc<PerTarget<Mutex<TargetStateCache>>>,
    /// The owner's live operational tuning (#631). Reading it is a lock and a
    /// copy, so preparation never has to round-trip through the owner.
    tuning: LiveTuning,
}

impl OwnerHandleCore {
    /// Create the bounded boundary between `state`'s future owner and its
    /// handles.
    pub(super) fn new(state: &OwnerState) -> (Self, OwnerEnds) {
        let permits = state.permits();
        let boundary_capacity = permits.total_capacity();
        // Cancellation is deliberately a small independent lane. Saturation
        // applies backpressure on the sending handle; it never falls back to
        // a lossy `try_send` path.
        let cancellation_capacity = 1;
        let control_capacity = state.facts().limits.control_requests.get();
        let rejection_capacity = state.facts().limits.staged_diagnostics.get();
        let (admission_tx, admissions) = flume::bounded(boundary_capacity);
        let (cancellation_tx, cancellations) = flume::bounded(cancellation_capacity);
        let (halt_capacity_tx, halt_capacity) = flume::bounded(1);
        let (control_tx, control) = flume::bounded(control_capacity);
        let (shutdown_tx, shutdown) = flume::bounded(1);
        let (alive, actor_alive) = flume::bounded(1);
        let lifecycle = Arc::new(OwnerLifecycle::new());
        let admission_rejections = Arc::new(AdmissionRejectionIngress::new(rejection_capacity));
        (
            Self {
                permits,
                admissions: admission_tx,
                admission_rejections: Arc::clone(&admission_rejections),
                cancellations: cancellation_tx,
                halt_capacity,
                control: control_tx,
                shutdown: shutdown_tx,
                actor_alive,
                lifecycle: Arc::clone(&lifecycle),
                state_cache: state.state_cache_registry(),
                tuning: state.live_tuning(),
            },
            OwnerEnds {
                receivers: BoundaryReceivers {
                    shutdown,
                    cancellations,
                    halt_capacity: halt_capacity_tx,
                    admissions,
                    control,
                },
                admission_rejections,
                alive,
                lifecycle,
            },
        )
    }

    /// The result a consuming `close` reports once the owner is gone.
    ///
    /// A caller that observes `actor_alive` disconnect reads the result after
    /// transport teardown. Explicit shutdown is successful when the driver
    /// has been released. A terminal failure remains observable after that
    /// release (#542 §Terminology, §3 ordering and transport failure).
    pub(super) fn closed_result(&self) -> Result<(), Error> {
        match &*self.lifecycle.lock_state() {
            LifecycleState::ShutDown { .. } => Ok(()),
            LifecycleState::Terminated { error, .. } => Err(error.clone()),
            LifecycleState::Open
            | LifecycleState::ShutdownAccepted
            | LifecycleState::ShuttingDown { .. }
            | LifecycleState::SignalFailed(_) => Err(missing_terminal_error()),
        }
    }

    /// Terminal publication precedes normal conclusion's final drain. A
    /// metrics caller at this point waits on the existing teardown barrier.
    pub(super) fn metrics_concluding(&self) -> bool {
        matches!(
            &*self.lifecycle.lock_state(),
            LifecycleState::ShuttingDown {
                metrics: TerminalMetrics::Pending,
                ..
            } | LifecycleState::ShutDown {
                metrics: TerminalMetrics::Pending
            } | LifecycleState::Terminated {
                metrics: TerminalMetrics::Pending,
                ..
            }
        )
    }

    /// A concluded owner's metrics, or `None` while the normal control path
    /// must wait for the owner to finish its drain.
    pub(super) fn terminal_metrics(
        &self,
    ) -> Option<Result<crate::observability::MetricsSnapshot, Error>> {
        self.lifecycle.metrics()
    }

    pub(super) fn state_cache(&self, target: TargetSlot) -> crate::state_cache::StateCache {
        crate::state_cache::StateCache::from_registry(Arc::clone(&self.state_cache), target)
    }

    /// Reads the tuning the owner is currently preparing requests under.
    pub(super) fn tuning(&self) -> crate::OperationalTuning {
        self.tuning.get().get()
    }

    pub(super) fn validated_tuning(&self) -> crate::session_config::ValidatedTuning {
        self.tuning.get()
    }

    /// Coalesced idempotent shutdown. Only the winning caller occupies the
    /// single shutdown slot. The state lock covers the non-blocking `try_send`,
    /// so concurrent callers observe the exact same accepted or failed result;
    /// the consuming `close` call waits separately on the liveness barrier.
    pub(super) fn shutdown(&self) -> Result<(), Error> {
        let mut state = self.lifecycle.lock_state();
        match &*state {
            LifecycleState::ShutdownAccepted
            | LifecycleState::ShuttingDown {
                signal_accepted: true,
                ..
            }
            | LifecycleState::ShutDown { .. } => Ok(()),
            LifecycleState::ShuttingDown {
                signal_accepted: false,
                ..
            } => Err(Error::RuntimeShutdown),
            LifecycleState::SignalFailed(error) | LifecycleState::Terminated { error, .. } => {
                Err(error.clone())
            }
            LifecycleState::Open => match self.shutdown.try_send(()) {
                Ok(()) => {
                    *state = LifecycleState::ShutdownAccepted;
                    Ok(())
                }
                Err(flume::TrySendError::Disconnected(_)) => {
                    let error = state.disconnected_error();
                    *state = LifecycleState::SignalFailed(error.clone());
                    Err(error)
                }
                Err(flume::TrySendError::Full(_)) => {
                    // Only this method sends on the one-slot shutdown lane,
                    // so a full queue while Open is an internal invariant
                    // failure. Repeated callers observe the same failure.
                    let error = Error::InvalidState {
                        reason: "shutdown signal lane was full before acceptance".into(),
                    };
                    *state = LifecycleState::SignalFailed(error.clone());
                    Err(error)
                }
            },
        }
    }

    /// Enqueue a halt in the same lifecycle critical section as an
    /// admission. Return a still-owned halt on backpressure, so the handle
    /// can wait for capacity without holding the lifecycle lock.
    pub(super) fn try_enqueue_cancellation(
        &self,
        boundary: CancellationBoundary,
    ) -> Result<Option<CancellationBoundary>, Error> {
        let lifecycle = self.lifecycle.lock_open()?;
        match self.cancellations.try_send(boundary) {
            Ok(()) => Ok(None),
            Err(flume::TrySendError::Full(boundary)) => Ok(Some(boundary)),
            Err(flume::TrySendError::Disconnected(_)) => Err(lifecycle.disconnected_error()),
        }
    }

    pub(super) fn enqueue_operation<S: SettlementAdmission>(
        &self,
        request: LaneRequest<CommandOutcome>,
        motion: MotionEffect,
    ) -> Result<QueuedAdmission<CommandOutcome, S::Admission>, Error> {
        self.enqueue_with_reply(request, None, |reply| S::reply(motion, reply))
    }

    /// Reserve a permit and enqueue one admission, returning the request's
    /// terminal observer and the receiver of the owner's admission reply.
    /// Every failure happens before an observer or engine ID is created and
    /// is recorded as a pre-admission rejection.
    pub(super) fn enqueue_admission<T: TerminalLane>(
        &self,
        request: impl Into<LaneRequest<T>>,
        validity: Option<AdmissionValidity>,
    ) -> Result<(TerminalObserver<T>, AdmissionReply), Error> {
        self.enqueue_with_reply(request.into(), validity, AdmissionReplySink::Request)
    }

    fn enqueue_with_reply<T: TerminalLane, R>(
        &self,
        request: LaneRequest<T>,
        validity: Option<AdmissionValidity>,
        reply_kind: impl FnOnce(flume::Sender<Result<R, Error>>) -> AdmissionReplySink,
    ) -> Result<QueuedAdmission<T, R>, Error> {
        let request = request.into_inner();
        let target = request.context().target;
        let lane = request.lane();
        if let Some(error) = self.admission_rejection() {
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        }
        let permit = match self.permits.try_acquire(&request) {
            Ok(permit) => permit,
            Err(error) => {
                self.record_pre_admission_rejection(target, lane, &error);
                return Err(error);
            }
        };
        // Keep the terminal check and enqueue in one lifecycle critical
        // section. Otherwise a caller can pass the check, the owner can
        // publish/drop and drain all boundaries, and this send can strand the
        // permit in a queue whose receiver will never poll it (#542 §4). Keep
        // the observer/reply/boundary construction after that check too: a
        // terminal pre-admission rejection must not allocate transient request
        // state before it returns.
        let lifecycle = match self.lifecycle.lock_open() {
            Ok(signal) => signal,
            Err(error) => {
                self.record_pre_admission_rejection(target, lane, &error);
                return Err(error);
            }
        };
        let mut request = request;
        request.context_mut().submission_order = self.lifecycle.allocate_order()?;
        let (completion, cell) = TerminalObserver::<T>::pair();
        let observer = T::slot(cell);
        let (reply, admission) = flume::bounded(1);
        let boundary = AdmissionBoundary {
            request,
            permit,
            observer,
            reply: reply_kind(reply),
            claim: validity.map_or(AdmissionBoundaryClaim::Unbounded, |validity| {
                AdmissionBoundaryClaim::Pending(validity.into())
            }),
        };
        match self.admissions.try_send(boundary) {
            Ok(()) => Ok((completion, admission)),
            Err(flume::TrySendError::Disconnected(_)) => {
                let error = lifecycle.disconnected_error();
                self.record_pre_admission_rejection(target, lane, &error);
                Err(error)
            }
            Err(flume::TrySendError::Full(_)) => {
                let error = Error::InvalidState {
                    reason: "admission channel full after permit reservation".into(),
                };
                self.record_pre_admission_rejection(target, lane, &error);
                Err(error)
            }
        }
    }

    /// [`enqueue_admission`](Self::enqueue_admission) as a
    /// [`PendingAdmission`].
    pub(super) fn admit<T: TerminalLane>(
        &self,
        request: impl Into<LaneRequest<T>>,
    ) -> Result<PendingAdmission<T>, Error> {
        let (completion, reply) = self.enqueue_admission(request, None)?;
        Ok(PendingAdmission {
            completion,
            reply,
            expiry: None,
        })
    }

    /// [`admit`](Self::admit) under a caller deadline that applies before
    /// admission. A deadline already reached at `now` is rejected without
    /// enqueuing anything.
    pub(super) fn admit_until<T: TerminalLane>(
        &self,
        request: impl Into<LaneRequest<T>>,
        deadline: Instant,
        now: Instant,
    ) -> Result<PendingAdmission<T>, Error> {
        let request = request.into();
        let target = request.context().target;
        let lane = request.lane();
        if now >= deadline {
            let error = Error::admission_timeout();
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        }
        let validity = AdmissionValidity::until(deadline);
        let (completion, reply) = self.enqueue_admission(request, Some(validity.clone()))?;
        Ok(PendingAdmission {
            completion,
            reply,
            expiry: Some(AdmissionExpiry {
                validity,
                target,
                lane,
            }),
        })
    }

    /// Settle a caller deadline that expired while its admission was queued.
    ///
    /// Returns the admission timeout when the caller won the race. `Ok(())`
    /// means the owner claimed the boundary first: its reply is authoritative,
    /// and waiting for it preserves normal post-admission observer-detach
    /// semantics instead of leaving an admitted request behind a returned
    /// timeout.
    fn expire_admission(&self, expiry: &AdmissionExpiry) -> Result<(), Error> {
        if !expiry.validity.expire_before_admission() {
            return Ok(());
        }
        let error = Error::admission_timeout();
        // Winning `expire_before_admission` is the sole caller-side
        // linearization point for this rejection. The owner observes
        // `ExpiredElsewhere` and deliberately does not record it again.
        self.record_pre_admission_rejection(expiry.target, expiry.lane, &error);
        Err(error)
    }

    /// Reports a compact rejection without creating any admission-owned state.
    ///
    /// A full bounded control lane already has an owner turn reserved; a
    /// closed lane means teardown won and no live owner remains to deliver a
    /// diagnostic. The bounded ingress still retains the exact scalar total
    /// until that owner drops.
    pub(super) fn record_pre_admission_rejection(
        &self,
        target: TargetSlot,
        lane: Lane,
        error: &Error,
    ) {
        let wake = self.admission_rejections.record(
            PreAdmissionRejection {
                target,
                lane,
                error: error.kind(),
            },
            matches!(error, Error::ControlReserveExhausted { .. }),
        );
        if wake {
            let _ = self
                .control
                .try_send(ControlBoundary::FlushAdmissionRejections);
        }
    }

    fn admission_rejection(&self) -> Option<Error> {
        self.lifecycle.lock_open().err()
    }

    /// Refuse new handle work once the owner has published its terminal verdict.
    pub(super) fn terminal_error(&self) -> Option<Error> {
        self.lifecycle.terminal()
    }

    /// The session's terminal error, for a boundary whose owner is gone.
    pub(super) fn disconnected_error(&self) -> Error {
        self.lifecycle.lock_state().disconnected_error()
    }
}

#[cfg(test)]
pub(super) mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Choose the owner side of the real pre-admission deadline race, then
    /// answer through the same consuming rejection production uses.
    #[cfg(any(feature = "blocking", feature = "test-utils"))]
    #[allow(clippy::panic)]
    pub(in crate::runtime::owner) fn expire_waiter_from_owner(
        admission: AdmissionBoundary,
        state: &mut OwnerState,
        selected_at: Instant,
    ) {
        let AdmissionBoundaryClaim::Pending(validity) = &admission.claim else {
            panic!("waiter admission must still be pending");
        };
        assert_eq!(validity.deadline(), selected_at);
        let rejection = validity.claim(&admission.reply, selected_at).unwrap_err();
        assert!(
            rejection.record,
            "the caller must not have expired this admission"
        );
        let error = rejection.error;
        state.record_admission_rejection(
            admission.request().context().target,
            admission.request().lane(),
            &error,
        );
        admission.reject(error);
    }

    fn lifecycle_handle() -> (OwnerHandleCore, OwnerEnds) {
        let policy = super::super::OwnerPolicy::single_target(
            crate::runtime::engine::ProtocolPolicy::test_default(),
            TargetSlot::CAMERA_1,
            crate::runtime::engine::TargetPolicy::test_default(),
        )
        .unwrap();
        let state = OwnerState::new(policy, Instant::now()).unwrap();
        OwnerHandleCore::new(&state)
    }

    fn result_error(result: Result<(), Error>) -> Option<String> {
        result.err().map(|error| format!("{error:?}"))
    }

    fn assert_lifecycle(
        state: LifecycleState,
        shutdown: Option<&str>,
        closed: Option<&str>,
        rejection: Option<&str>,
        disconnected: &str,
        terminal: Option<&str>,
    ) {
        let (handle, _ends) = lifecycle_handle();
        *handle.lifecycle.lock_state() = state;
        assert_eq!(
            handle
                .lifecycle
                .terminal()
                .map(|error| format!("{error:?}")),
            terminal.map(str::to_owned),
        );
        assert_eq!(
            handle
                .lifecycle
                .lock_open()
                .err()
                .map(|error| format!("{error:?}")),
            rejection.map(str::to_owned),
        );
        assert_eq!(format!("{:?}", handle.disconnected_error()), disconnected);
        assert_eq!(
            result_error(handle.closed_result()),
            closed.map(str::to_owned)
        );
        assert_eq!(result_error(handle.shutdown()), shutdown.map(str::to_owned));
    }

    #[test]
    fn lifecycle_reachable_states_preserve_every_boundary_verdict() {
        const MISSING: &str = "OwnerTerminated";
        const SHUTDOWN: &str = "RuntimeShutdown";
        const FAILED: &str = "InvalidState { reason: \"signal failed\" }";
        assert_lifecycle(
            LifecycleState::Open,
            None,
            Some(MISSING),
            None,
            MISSING,
            None,
        );
        assert_lifecycle(
            LifecycleState::ShutdownAccepted,
            None,
            Some(MISSING),
            Some(SHUTDOWN),
            MISSING,
            None,
        );
        assert_lifecycle(
            LifecycleState::SignalFailed(Error::InvalidState {
                reason: "signal failed".into(),
            }),
            Some(FAILED),
            Some(MISSING),
            Some(FAILED),
            MISSING,
            None,
        );
        for (signal_accepted, shutdown) in [(true, None), (false, Some(SHUTDOWN))] {
            assert_lifecycle(
                LifecycleState::ShuttingDown {
                    signal_accepted,
                    metrics: TerminalMetrics::Pending,
                },
                shutdown,
                Some(MISSING),
                Some(SHUTDOWN),
                SHUTDOWN,
                Some(SHUTDOWN),
            );
        }
        assert_lifecycle(
            LifecycleState::ShutDown {
                metrics: TerminalMetrics::Pending,
            },
            None,
            None,
            Some(SHUTDOWN),
            SHUTDOWN,
            Some(SHUTDOWN),
        );
        assert_lifecycle(
            LifecycleState::Terminated {
                error: Error::RuntimeShutdown,
                metrics: TerminalMetrics::Pending,
            },
            Some(SHUTDOWN),
            Some(SHUTDOWN),
            Some(SHUTDOWN),
            SHUTDOWN,
            Some(SHUTDOWN),
        );
        assert_lifecycle(
            LifecycleState::Terminated {
                error: Error::InvalidState {
                    reason: "signal failed".into(),
                },
                metrics: TerminalMetrics::Pending,
            },
            Some(FAILED),
            Some(FAILED),
            Some(FAILED),
            FAILED,
            Some(FAILED),
        );
    }

    #[test]
    fn conclusion_preserves_a_signal_failure_before_the_engine_ends() {
        let lifecycle = OwnerLifecycle::new();
        let error = Error::InvalidState {
            reason: "shutdown signal failed".into(),
        };
        *lifecycle.lock_state() = LifecycleState::SignalFailed(error.clone());
        let concluded = lifecycle.conclusion_error();
        assert_eq!(format!("{concluded:?}"), format!("{error:?}"));
        lifecycle.publish_terminal(concluded);
        assert_eq!(
            format!("{:?}", lifecycle.terminal().unwrap()),
            format!("{error:?}")
        );
    }

    #[test]
    fn orderly_engine_shutdown_requires_driver_release_for_close_success() {
        for released in [true, false] {
            let (handle, _ends) = lifecycle_handle();
            assert!(handle.shutdown().is_ok());
            let lifecycle = &handle.lifecycle;
            lifecycle.publish_terminal(Error::RuntimeShutdown);
            lifecycle.publish_terminal(Error::OwnerTerminated);
            assert!(matches!(
                handle.closed_result(),
                Err(Error::OwnerTerminated)
            ));

            if released {
                lifecycle.driver_released();
                lifecycle.driver_released();
            }
            for _ in 0..2 {
                let final_error = lifecycle.fail_closed();
                if released {
                    assert!(matches!(final_error, Error::RuntimeShutdown));
                    assert!(handle.closed_result().is_ok());
                } else {
                    assert!(matches!(final_error, Error::OwnerTerminated));
                    assert!(matches!(
                        handle.closed_result(),
                        Err(Error::OwnerTerminated)
                    ));
                }
            }
        }
    }

    #[test]
    fn fail_closed_preserves_a_prior_terminal_or_signalling_error() {
        let error = Error::InvalidState {
            reason: "earlier failure".into(),
        };
        for initial in [
            LifecycleState::SignalFailed(error.clone()),
            LifecycleState::Terminated {
                error: error.clone(),
                metrics: TerminalMetrics::Pending,
            },
        ] {
            let (handle, _ends) = lifecycle_handle();
            let lifecycle = &handle.lifecycle;
            *lifecycle.lock_state() = initial;
            let expected = format!("{error:?}");
            assert_eq!(format!("{:?}", lifecycle.fail_closed()), expected);
            lifecycle.publish_terminal(Error::RuntimeShutdown);
            lifecycle.driver_released();
            assert_eq!(result_error(handle.closed_result()), Some(expected));
        }
    }

    #[test]
    fn admission_rejection_ingress_total_saturates() {
        let ingress = AdmissionRejectionIngress::new(2);
        let rejection = PreAdmissionRejection {
            target: TargetSlot::CAMERA_1,
            lane: Lane::Command,
            error: crate::ErrorKind::BufferFull,
        };
        ingress.total.store(u64::MAX - 1, Ordering::Release);

        assert!(ingress.record(rejection, false));
        assert_eq!(ingress.total(), u64::MAX);
        assert!(!ingress.record(rejection, false));
        assert_eq!(ingress.total(), u64::MAX);
    }
}
