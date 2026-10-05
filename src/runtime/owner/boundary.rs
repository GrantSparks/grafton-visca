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

use crate::Error;

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
use super::SessionState;
use super::{
    AdmissionPermit, AdmissionPermitPool, Admitted, CancellationRequest, DiagnosticSubscription,
    Lane, LiveTuning, ObserverCell, OwnerState, RuntimeOutcome, RuntimeRequest, TargetStateCache,
    TerminalObserver,
};
#[cfg(all(test, feature = "runtime-tokio"))]
use super::{DiagnosticEvent, OwnerMetrics};

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
    Claimed,
    /// This owner observed the deadline first and rejected the boundary.
    ExpiredHere,
    /// The caller had already rejected the boundary before this owner turn.
    ExpiredElsewhere,
}

impl AdmissionValidity {
    const PENDING: u8 = AdmissionValidityState::Pending as u8;
    const CLAIMED: u8 = AdmissionValidityState::Claimed as u8;
    const EXPIRED: u8 = AdmissionValidityState::Expired as u8;

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
    target: crate::CameraId,
    lane: Lane,
}

/// One admission a caller enqueued, awaiting the owner's reply.
///
/// It turns that reply into the receipt core, so both owner shells build
/// receipts the same way and differ only in how they wait for the reply.
#[derive(Debug)]
pub(super) struct PendingAdmission {
    target: crate::CameraId,
    completion: TerminalObserver,
    reply: AdmissionReply,
    expiry: Option<AdmissionExpiry>,
}

impl PendingAdmission {
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
    ) -> Result<super::ReceiptCore, Error> {
        Ok(super::ReceiptCore::admitted(
            reply??,
            self.target,
            self.completion,
            configured_timeout,
        ))
    }
}

#[derive(Debug)]
pub(super) struct AdmissionBoundary {
    pub(super) request: RuntimeRequest,
    pub(super) permit: AdmissionPermit,
    pub(super) observer: Arc<ObserverCell<RuntimeOutcome>>,
    pub(super) reply: flume::Sender<Result<Admitted, Error>>,
    /// Present only for a caller deadline that applies before admission.
    pub(super) validity: Option<AdmissionValidity>,
    /// The owner has already won the caller's pre-admission deadline race,
    /// but a raw-correlation release gate has retained this boundary until it
    /// can safely enter the engine.  Keeping that one-way answer on the
    /// boundary preserves the caller's established admission promise without
    /// letting the retained admission run a due/dispatch turn early.
    pub(super) validity_claimed: bool,
}

/// A rejection that happened before a handle could allocate any owner-owned
/// admission state.
#[derive(Debug, Clone, Copy)]
pub(super) struct PreAdmissionRejection {
    pub(super) target: crate::CameraId,
    pub(super) lane: Lane,
    pub(super) error: crate::ErrorKind,
}

#[derive(Debug)]
struct PendingAdmissionRejections {
    events: VecDeque<PreAdmissionRejection>,
    /// Events evicted before the owner could enter them into its public ring.
    /// The owner folds this into the existing `dropped_diagnostics` metric.
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
            // The owner diagnostic ring is bounded too. Preserve the newest
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
        capacity: usize,
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
        validated_tuning: Box<Result<crate::OperationalTuning, Error>>,
        reply: flume::Sender<Result<(), Error>>,
    },
}

/// Bounded diagnostic/metric copy returned by the owner.
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

/// The result of the one coalesced shutdown signal attempt. This is separate
/// from the owner's eventual terminal result: `shutdown` acknowledges only
/// acceptance into the bounded lane, while `close` waits for that result.
#[derive(Debug, Clone)]
enum ShutdownSignalState {
    Open,
    Accepted,
    Failed(Error),
}

/// The lifecycle facts a session's handles and its owner share.
///
/// Terminal publication, shutdown acceptance and admission enqueue all take
/// the signal lock first, so they have one linearization point. Without it, a
/// shutdown caller could observe `Open`, enqueue after the owner had already
/// terminated, and return `Ok(())` even though no owner turn could ever
/// consume the signal; an admission could likewise strand its permit in a
/// queue whose receiver will never poll it (#542 §4).
#[derive(Debug)]
pub(super) struct OwnerLifecycle {
    signal: Mutex<ShutdownSignalState>,
    terminal: Mutex<Option<Error>>,
    next_submission: AtomicU64,
}

impl OwnerLifecycle {
    fn new() -> Self {
        Self {
            signal: Mutex::new(ShutdownSignalState::Open),
            terminal: Mutex::new(None),
            next_submission: AtomicU64::new(1),
        }
    }

    // The pinned nightly renamed fetch_update to try_update, which is newer
    // than our Rust 1.88 MSRV. Keep the identical checked atomic update until
    // that replacement is available on the minimum supported toolchain.
    #[allow(deprecated)]
    fn allocate_order(&self) -> Result<u64, Error> {
        self.next_submission
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Error::RuntimeIdentityExhausted)
    }

    pub(super) fn fence_order(
        &self,
        motion: &Arc<super::motion::MotionRegistry>,
        target: crate::CameraId,
        axes: Option<crate::AffectedAxes>,
    ) -> Result<u64, Error> {
        let _guard = self.lock_open()?;
        let cutoff = self.allocate_order()?;
        // Lock order is ingress -> motion. No other path holds both, and no
        // I/O occurs here: enqueue and settlement commit see one acceptance.
        if let Some(axes) = axes {
            motion.establish(target, axes, cutoff);
        }
        Ok(cutoff)
    }

    fn lock_signal(&self) -> MutexGuard<'_, ShutdownSignalState> {
        self.signal
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_terminal(&self) -> MutexGuard<'_, Option<Error>> {
        self.terminal
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The published terminal error, if the owner has ended.
    fn terminal(&self) -> Option<Error> {
        self.lock_terminal().clone()
    }

    /// Hold the signal lock while the session still accepts boundary work,
    /// or return the error that rejects it. Holding the returned guard across
    /// an enqueue keeps that enqueue ordered before any terminal publication.
    fn lock_open(&self) -> Result<MutexGuard<'_, ShutdownSignalState>, Error> {
        let signal = self.lock_signal();
        let rejection = match &*signal {
            ShutdownSignalState::Accepted => Some(Error::RuntimeShutdown),
            ShutdownSignalState::Failed(error) => Some(error.clone()),
            ShutdownSignalState::Open => self.terminal(),
        };
        match rejection {
            Some(error) => Err(error),
            None => Ok(signal),
        }
    }

    /// Publish a terminal engine verdict at the lifecycle linearization point.
    ///
    /// The owner calls this while the terminal `SessionChanged` effect is
    /// being driven, rather than only once its loop has ended. A shutdown
    /// caller can otherwise enter the still-live one-slot lane after the
    /// engine has become terminal but before the loop's epilogue, and
    /// incorrectly receive `Ok(())`.
    pub(super) fn publish_terminal(&self, error: Error) {
        let mut signal = self.lock_signal();
        *self.lock_terminal() = Some(error.clone());
        // Only the caller whose signal produced an explicit shutdown retains
        // success. An engine close or poison supersedes even a signal accepted
        // earlier in the same ready-source race.
        if !matches!(&error, Error::RuntimeShutdown)
            || !matches!(&*signal, ShutdownSignalState::Accepted)
        {
            *signal = ShutdownSignalState::Failed(error);
        }
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
        let mut signal = self.lock_signal();
        let mut terminal = self.lock_terminal();
        let error = terminal.clone().unwrap_or_else(missing_terminal_error);
        if terminal.is_none() {
            *terminal = Some(error.clone());
        }
        if !matches!(&error, Error::RuntimeShutdown)
            || !matches!(*signal, ShutdownSignalState::Accepted)
        {
            *signal = ShutdownSignalState::Failed(error.clone());
        }
        error
    }
}

/// The error for an owner that disappeared without publishing its terminal
/// result, which only an unwind can cause.
pub(super) fn missing_terminal_error() -> Error {
    Error::InvalidState("owner actor disconnected without publishing a terminal result".into())
}

/// The owner's ends of the bounded boundary lanes, one per source the
/// coordinator arbitrates.
#[derive(Debug)]
pub(super) struct BoundaryReceivers {
    pub(super) shutdown: flume::Receiver<()>,
    pub(super) cancellations: flume::Receiver<CancellationBoundary>,
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
    pub(super) control: flume::Sender<ControlBoundary>,
    shutdown: flume::Sender<()>,
    /// Disconnects after owner teardown, including driver/transport drop.
    /// Nothing is ever sent on it.
    pub(super) actor_alive: flume::Receiver<()>,
    lifecycle: Arc<OwnerLifecycle>,
    state_cache: Arc<[Mutex<TargetStateCache>; 9]>,
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
        let control_capacity = state.policy().limits.applied_subscribers.max(1);
        let rejection_capacity = state.policy().limits.diagnostics;
        let (admission_tx, admissions) = flume::bounded(boundary_capacity);
        let (cancellation_tx, cancellations) = flume::bounded(cancellation_capacity);
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
    /// The owner publishes its terminal error before dropping the driver and
    /// liveness sender, so a caller that has observed `actor_alive`
    /// disconnect reads it ordered after transport teardown. An explicit
    /// shutdown is the only terminal result that `close` turns into success;
    /// transport close/poison (and any other terminal owner error) must remain
    /// observable at that boundary (#542 §Terminology, §3 ordering and
    /// transport failure).
    pub(super) fn closed_result(&self) -> Result<(), Error> {
        match self.lifecycle.terminal() {
            Some(Error::RuntimeShutdown) => Ok(()),
            Some(error) => Err(error),
            None => Err(missing_terminal_error()),
        }
    }

    pub(super) fn state_cache(&self, target: crate::CameraId) -> crate::state_cache::StateCache {
        crate::state_cache::StateCache::from_registry(Arc::clone(&self.state_cache), target)
    }

    /// Reads the tuning the owner is currently preparing requests under.
    pub(super) fn tuning(&self) -> crate::OperationalTuning {
        self.tuning.get()
    }

    /// Coalesced idempotent shutdown. Only the winning caller occupies the
    /// single shutdown slot. The state lock covers the non-blocking `try_send`,
    /// so concurrent callers observe the exact same accepted or failed result;
    /// the consuming `close` call waits separately on the liveness barrier.
    pub(super) fn shutdown(&self) -> Result<(), Error> {
        let mut signal = self.lifecycle.lock_signal();
        // Terminal publication and shutdown acceptance share this lifecycle
        // lock. Check the published engine verdict before occupying the
        // shutdown lane: a session that already terminalized itself must hand
        // that cause back to a cleanup caller rather than acknowledge a signal
        // no owner turn can consume.
        if let Some(error) = self.lifecycle.terminal() {
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

    /// Reserve a permit and enqueue one admission, returning the request's
    /// terminal observer and the receiver of the owner's admission reply.
    /// Every failure happens before an observer or engine ID is created and
    /// is recorded as a pre-admission rejection.
    pub(super) fn enqueue_admission(
        &self,
        request: RuntimeRequest,
        validity: Option<AdmissionValidity>,
    ) -> Result<(TerminalObserver, AdmissionReply), Error> {
        let target = request.context().target;
        let lane = request.lane();
        if let Some(error) = self.admission_rejection() {
            self.record_pre_admission_rejection(target, lane, &error);
            return Err(error);
        }
        let permit = match self.permits.try_acquire(request.context()) {
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
        let _signal = match self.lifecycle.lock_open() {
            Ok(signal) => signal,
            Err(error) => {
                self.record_pre_admission_rejection(target, lane, &error);
                return Err(error);
            }
        };
        let mut request = request;
        request.context_mut().submission_order = self.lifecycle.allocate_order()?;
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

    /// [`enqueue_admission`](Self::enqueue_admission) as a
    /// [`PendingAdmission`].
    pub(super) fn admit(&self, request: RuntimeRequest) -> Result<PendingAdmission, Error> {
        let target = request.context().target;
        let (completion, reply) = self.enqueue_admission(request, None)?;
        Ok(PendingAdmission {
            target,
            completion,
            reply,
            expiry: None,
        })
    }

    /// [`admit`](Self::admit) under a caller deadline that applies before
    /// admission. A deadline already reached at `now` is rejected without
    /// enqueuing anything.
    pub(super) fn admit_until(
        &self,
        request: RuntimeRequest,
        deadline: Instant,
        now: Instant,
    ) -> Result<PendingAdmission, Error> {
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
            target,
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
        target: crate::CameraId,
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

    /// The session's terminal error, for a boundary whose owner is gone.
    pub(super) fn disconnected_error(&self) -> Error {
        self.lifecycle
            .terminal()
            .unwrap_or_else(missing_terminal_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CameraId;

    #[test]
    fn admission_rejection_ingress_total_saturates() {
        let ingress = AdmissionRejectionIngress::new(2);
        let rejection = PreAdmissionRejection {
            target: CameraId::CAMERA_1,
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
