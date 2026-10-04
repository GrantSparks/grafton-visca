//! Native blocking owner worker and its handle (D24, #780).
//!
//! Every blocking session runs one native worker thread that owns the
//! transport and the shared [`OwnerShellCore`]. Callers talk to it through the
//! same bounded boundary lanes as the async actor, so overlapping callers never
//! contend for the transport, and timers, reads and correlation cleanup make
//! progress with no caller inside the session. The `blocking` feature stays
//! executor-free (#723): the worker is a `std` thread, and every wait below is
//! a `flume` channel operation, a sleep, or a bounded transport read.
//!
//! The worker loop is the async actor's loop without an executor. A selection
//! walks [`Selection::order`] once per pass: boundary lanes are polled without
//! blocking, the timer is compared with the clock, and the receive source is
//! one bounded transport read. A read cannot be interrupted, so it is kept
//! short: a [`READY_PROBE`] when a later source in the order is already ready,
//! otherwise at most a [`CONTROL_SLICE`]. A read that times out leaves the
//! receive pending and the same pass continues down the order, exactly as a
//! pending receive future does; only a full read timeout without data, or an
//! eager transport that answers "no data" before its timeout, is an idle
//! receive event (#675). One read keeps the worker away from its boundary
//! lanes for at most one slice. Queued boundaries, writes and protocol
//! eligibility can add to a control or STOP request's total latency.

use std::{
    collections::VecDeque,
    ops::ControlFlow,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{completion, CancellationOutcome, Error};

use super::boundary::{CancellationBoundary, ControlBoundary, OwnerHandleCore};
use super::receipt::{
    observer_deadline, position_poll, settlement_budget, CommandReceipt, InquiryReceipt,
    ObservationWake, OperationReceipt, OperationWait, PositionPoll,
};
use super::shell::{ClassifiedEvent, OwnerEvent, OwnerShellCore, TurnStep};
use super::turn::{ReceiveArm, Selection, Source, TimerArm, TurnOutcome, TurnPlan};
use super::{
    normalize_command_outcome, normalize_inquiry_outcome, CancellationObserver,
    CancellationRequest, DiagnosticSubscription, OperationObservation, OwnerBuffers,
    OwnerInputTurn, OwnerPolicy, OwnerReceive, OwnerState, ReceiptCore, RetainedStreamInput,
    RuntimeOutcome, RuntimeRequest, TerminalObserver, TransmissionMeta, WireWrite,
};
use crate::runtime::engine::Effect;

/// The longest one transport read keeps the worker away from its boundary
/// lanes when nothing else is ready. This bounds the contribution of one idle
/// read to boundary latency, at the cost of about a hundred timed-out reads
/// a second.
pub(crate) const CONTROL_SLICE: Duration = Duration::from_millis(10);

/// The read taken when a source later in the selection order is already
/// ready: long enough for any transport to honor, short enough that the ready
/// source is selected almost at once when the transport has nothing.
pub(crate) const READY_PROBE: Duration = Duration::from_millis(1);

/// Blocking transport/framing driver owned by the worker thread.
pub(crate) trait BlockingOwnerDriver: Send + RetainedStreamInput {
    /// Write one framed request, bounded by the transport's write timeout.
    fn write(&mut self, write: WireWrite<'_>) -> Result<TransmissionMeta, Error>;

    /// Return complete frames the framer still holds, or read once for at
    /// most `timeout`. A read that times out reports [`OwnerReceive::NoData`].
    fn receive(
        &mut self,
        buffers: &mut OwnerBuffers,
        frame_limit: usize,
        timeout: Duration,
    ) -> Result<OwnerReceive, Error>;
}

/// The worker thread's state. The driver lives on the thread's stack, so the
/// worker can drop it before the shell core's liveness sender.
struct BlockingOwnerWorker {
    core: OwnerShellCore,
}

impl BlockingOwnerWorker {
    fn run<D>(mut self, mut driver: D)
    where
        D: BlockingOwnerDriver,
    {
        // The caller's advertised read timeout decides when an uninterrupted
        // read without data is an idle receive, as on the async surface (#675).
        let read_timeout = self.core.state.policy().read_timeout;
        // Every arbitration and turn decision below is the shell core's; this
        // loop only samples the clock, polls the sources the plan describes,
        // and runs the I/O the selected event's turn asks for.
        while self.core.is_running() {
            // Callers enqueue from their own threads, so a boundary-first turn
            // needs no cooperative handoff before it is planned.
            self.core.begin_turn();
            let now = Instant::now();
            let event = match self.core.plan(now, &mut driver) {
                TurnPlan::Redeliver(retained) => self.core.redeliver(retained),
                TurnPlan::Select(selection) => self.select(selection, &mut driver, read_timeout),
            };
            let selected_at = Instant::now();
            let Some(ClassifiedEvent {
                event,
                selected,
                suppress_due,
            }) = self.core.classify(event, selected_at)
            else {
                continue;
            };
            let control_consumed_wake = match &event {
                OwnerEvent::Control(_) => self.core.control_consumed_wake(Instant::now()),
                _ => None,
            };
            let outcome = self.handle_event(event, &mut driver, selected_at, suppress_due);
            if !self.core.finish(selected, control_consumed_wake, outcome) {
                break;
            }
        }
        self.core.conclude();
        // #626: drop the driver first so a waiter using the liveness lane gets
        // a deterministic transport-release barrier. The shell core's `Drop`
        // performs one final boundary drain before its alive sender is
        // dropped, covering messages that race the explicit drain.
        drop(driver);
    }

    /// Select one event in the planned order. A pass polls every source once;
    /// a pass in which nothing was ready is repeated, so the selection stays
    /// the one the coordinator planned until a source is ready.
    fn select<D>(
        &mut self,
        selection: Selection,
        driver: &mut D,
        read_timeout: Duration,
    ) -> OwnerEvent
    where
        D: BlockingOwnerDriver,
    {
        let frame_limit = self.core.state.policy().limits.frames_per_receive;
        // Time this selection has spent reading without data. Like the async
        // read under its timeout, it restarts with every selection.
        let mut idle = Duration::ZERO;
        loop {
            let mut read = false;
            for (position, source) in selection.order().enumerate() {
                let event = match source {
                    Source::Receive => match selection.receive {
                        // The exact no-input probe for this release set is
                        // done: the receive source is immediately ready with
                        // the timer event instead of another read.
                        ReceiveArm::ProofComplete => Some(OwnerEvent::Wake),
                        // An idle or faulted read paces the next one; the
                        // other sources stay live while this one waits.
                        ReceiveArm::Paced { until } if Instant::now() < until => None,
                        ReceiveArm::Poll | ReceiveArm::Paced { .. } => {
                            read = true;
                            let later_ready = selection
                                .order()
                                .skip(position.saturating_add(1))
                                .any(|later| self.is_ready(later, selection.timer));
                            let timeout = if later_ready {
                                READY_PROBE
                            } else {
                                read_slice(selection.timer, read_timeout.saturating_sub(idle))
                            };
                            receive(
                                &mut self.core.state,
                                driver,
                                frame_limit,
                                timeout,
                                read_timeout,
                                &mut idle,
                            )
                        }
                    },
                    Source::Timer => timer_ready(selection.timer).then_some(OwnerEvent::Wake),
                    lane => self.try_lane(lane),
                };
                if let Some(event) = event {
                    return event;
                }
            }
            if !read {
                // Receive is paced and no other source is ready: wait for the
                // pace, the timer or the next slice, whichever comes first.
                thread::sleep(paced_wait(selection));
            }
        }
    }

    /// Whether `source` would be selected now, without consuming anything.
    fn is_ready(&self, source: Source, timer: TimerArm) -> bool {
        let receivers = &self.core.receivers;
        match source {
            Source::Shutdown => !receivers.shutdown.is_empty(),
            Source::Cancellation => !receivers.cancellations.is_empty(),
            // A disconnected admission lane is itself the terminal event.
            Source::Admission => {
                !receivers.admissions.is_empty() || receivers.admissions.is_disconnected()
            }
            Source::Control => !receivers.control.is_empty(),
            Source::Timer => timer_ready(timer),
            Source::Receive => false,
        }
    }

    /// Poll one boundary lane without blocking. A disconnected shutdown,
    /// cancellation or control lane never becomes ready; a disconnected
    /// admission lane is reported, since it means every handle is gone.
    fn try_lane(&self, source: Source) -> Option<OwnerEvent> {
        let receivers = &self.core.receivers;
        match source {
            Source::Shutdown => receivers
                .shutdown
                .try_recv()
                .ok()
                .map(|()| OwnerEvent::Shutdown),
            Source::Cancellation => receivers
                .cancellations
                .try_recv()
                .ok()
                .map(OwnerEvent::Cancellation),
            Source::Admission => match receivers.admissions.try_recv() {
                Ok(admission) => Some(OwnerEvent::Admission(Ok(admission))),
                Err(flume::TryRecvError::Disconnected) => {
                    Some(OwnerEvent::Admission(Err(flume::RecvError::Disconnected)))
                }
                Err(flume::TryRecvError::Empty) => None,
            },
            Source::Control => receivers.control.try_recv().ok().map(OwnerEvent::Control),
            Source::Receive | Source::Timer => None,
        }
    }

    /// Run the engine turn of one classified event, performing the writes and
    /// pacing its [`TurnStep`]s ask for.
    fn handle_event<D>(
        &mut self,
        event: OwnerEvent,
        driver: &mut D,
        selected_at: Instant,
        suppress_due_for_raw_probe_fault: bool,
    ) -> TurnOutcome
    where
        D: BlockingOwnerDriver,
    {
        let mut step =
            self.core
                .handle(event, driver, selected_at, suppress_due_for_raw_probe_fault);
        loop {
            match step {
                TurnStep::Drive { effects, then } => {
                    self.drive(driver, effects, then.input_turn());
                    step = self.core.resume(then, driver);
                }
                TurnStep::Pace { pause, outcome } => {
                    self.core.pace_receive(pause, Instant::now());
                    return outcome;
                }
                TurnStep::Done(outcome) => return outcome,
            }
        }
    }

    /// Apply `effects` in order, writing each staged transmit. Inside an input
    /// `turn`, a transmit finishes in that turn; otherwise it finishes at the
    /// instant the write returned.
    fn drive<D>(
        &mut self,
        driver: &mut D,
        mut effects: VecDeque<Effect>,
        turn: Option<&OwnerInputTurn>,
    ) where
        D: BlockingOwnerDriver,
    {
        while let Some(staged) = self.core.next_transmit(&mut effects) {
            let result = match self.core.state.prepare_write(&staged) {
                Ok(write) => driver.write(write),
                Err(error) => Err(error),
            };
            match turn {
                Some(turn) => {
                    self.core
                        .finish_transmit_in_turn(turn, &staged, result, &mut effects);
                }
                None => {
                    let finished_at = Instant::now();
                    self.core
                        .finish_transmit(&staged, result, finished_at, &mut effects);
                }
            }
        }
    }
}

/// One bounded read. Returns `None` while the receive is still pending: the
/// read waited out its slice and the selection's read timeout has not elapsed.
fn receive<D>(
    state: &mut OwnerState,
    driver: &mut D,
    frame_limit: usize,
    timeout: Duration,
    read_timeout: Duration,
    idle: &mut Duration,
) -> Option<OwnerEvent>
where
    D: BlockingOwnerDriver,
{
    let started = Instant::now();
    let result = driver.receive(state.buffers(), frame_limit, timeout);
    // Sampled after the read returns: only this instant proves that input was
    // sampled at or after a raw hold deadline.
    let received_at = Instant::now();
    if matches!(result, Ok(OwnerReceive::NoData)) {
        let waited = received_at.saturating_duration_since(started);
        *idle = idle.saturating_add(waited);
        // A read that answered sooner than asked is an eager idle transport,
        // whose idle answer is itself the receive event and is paced (#675).
        if waited >= timeout && *idle < read_timeout {
            return None;
        }
    }
    Some(OwnerEvent::Receive {
        result,
        received_at,
    })
}

/// Whether the planned timer is due now.
fn timer_ready(timer: TimerArm) -> bool {
    match timer {
        TimerArm::Engine { due: true, .. } => true,
        TimerArm::Engine { at, due: false } => at.is_some_and(|at| at <= Instant::now()),
        TimerArm::Grace { until } => until <= Instant::now(),
    }
}

/// The time until the planned timer is due, or a whole slice without one.
fn until_timer(timer: TimerArm, now: Instant) -> Duration {
    let deadline = match timer {
        TimerArm::Engine { at, .. } => at,
        TimerArm::Grace { until } => Some(until),
    };
    deadline.map_or(CONTROL_SLICE, |deadline| {
        deadline.saturating_duration_since(now)
    })
}

/// A read slice: at most [`CONTROL_SLICE`], the rest of the selection's read
/// timeout, and the time to the planned timer, but never shorter than
/// [`READY_PROBE`], since a zero read timeout means "wait forever" to an OS
/// socket.
fn read_slice(timer: TimerArm, idle_budget: Duration) -> Duration {
    CONTROL_SLICE
        .min(idle_budget)
        .min(until_timer(timer, Instant::now()))
        .max(READY_PROBE)
}

/// How long to wait while receive is paced and nothing else is ready.
fn paced_wait(selection: Selection) -> Duration {
    let now = Instant::now();
    let pace = match selection.receive {
        ReceiveArm::Paced { until } => until.saturating_duration_since(now),
        ReceiveArm::Poll | ReceiveArm::ProofComplete => CONTROL_SLICE,
    };
    CONTROL_SLICE
        .min(pace)
        .min(until_timer(selection.timer, now))
}

/// Cloneable blocking owner handle: the shared boundary core plus the worker
/// thread a consuming close joins.
#[derive(Debug, Clone)]
pub(crate) struct BlockingOwnerHandle {
    core: OwnerHandleCore,
    /// Retained after another closer takes the join handle, so a transport
    /// callback can never wait for its own worker's teardown.
    worker_id: thread::ThreadId,
    /// Taken by the first `close`; every other clone waits on the liveness
    /// lane instead. Dropping the last handle detaches the worker, which then
    /// stops on its own because every boundary lane has disconnected.
    worker: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl BlockingOwnerHandle {
    /// Start the worker thread over `driver`.
    ///
    /// Every failure, including a failed thread spawn, drops the driver (and
    /// its transport) and returns the error; no handle and no partially
    /// started worker remain. `Ok` means the worker is running.
    pub(crate) fn spawn<D>(policy: OwnerPolicy, driver: D) -> Result<Self, Error>
    where
        D: BlockingOwnerDriver + 'static,
    {
        let state = OwnerState::new(policy)?;
        let (core, ends) = OwnerHandleCore::new(&state);
        let worker = BlockingOwnerWorker {
            core: OwnerShellCore::new(state, ends),
        };
        let worker = thread::Builder::new()
            .name("grafton-visca-owner".into())
            .spawn(move || worker.run(driver))?;
        Ok(Self {
            core,
            worker_id: worker.thread().id(),
            worker: Arc::new(Mutex::new(Some(worker))),
        })
    }

    pub(crate) fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
        observer_deadline(Instant::now(), timeout)
    }

    /// A wait for one boundary reply, or for the worker's disappearance.
    ///
    /// #626: the worker answers every boundary message its final drain can
    /// still see, then drops its receivers. A message that reaches a queue
    /// after that drain is stranded, and waiting on its reply alone would
    /// never return. Teardown drops the worker's liveness sender after the
    /// drain and driver drop, which resolves that wait with the session's
    /// terminal error instead. The reply is re-checked once the worker is
    /// gone, so a message the drain did answer still returns its answer.
    fn reply<'a, T>(
        &'a self,
        reply: &'a flume::Receiver<T>,
    ) -> flume::Selector<'a, Result<T, Error>> {
        flume::Selector::new()
            .recv(reply, |answer| {
                answer.map_err(|_| self.core.disconnected_error())
            })
            // Nothing is ever sent on this lane, so it fires exactly once,
            // when worker teardown drops its end after the driver.
            .recv(&self.core.actor_alive, |_| {
                reply.try_recv().map_err(|_| self.core.disconnected_error())
            })
    }

    /// Send one control request and wait for its answer.
    fn control_request<T>(
        &self,
        request: impl FnOnce(flume::Sender<T>) -> ControlBoundary,
    ) -> Result<T, Error> {
        let (reply, receiver) = flume::bounded(1);
        self.core
            .control
            .send(request(reply))
            .map_err(|_| self.core.disconnected_error())?;
        self.reply(&receiver).wait()
    }

    /// Class-specific typed admission seam for ordinary commands.
    pub(crate) fn submit_command(
        &self,
        prepared: crate::prepared::PreparedCommand,
    ) -> Result<CommandReceipt<Self>, Error> {
        prepared.admit_with(|request, timeout| {
            self.submit_with_timeout(request, timeout)
                .map(|core| CommandReceipt {
                    core,
                    owner: self.clone(),
                })
        })
    }

    /// Class-specific typed admission seam retaining the external decoder.
    pub(crate) fn submit_inquiry<R>(
        &self,
        prepared: crate::prepared::PreparedInquiry<R>,
    ) -> Result<InquiryReceipt<R, Self>, Error> {
        prepared.admit_with(|request, decoder, timeout| {
            self.submit_with_timeout(request, timeout)
                .map(|core| InquiryReceipt {
                    core,
                    decoder,
                    owner: self.clone(),
                })
        })
    }

    fn submit_inquiry_until<R>(
        &self,
        prepared: crate::prepared::PreparedInquiry<R>,
        deadline: Instant,
    ) -> Result<InquiryReceipt<R, Self>, Error> {
        prepared.admit_with(|request, decoder, timeout| {
            self.submit_with_timeout_until(request, timeout, deadline)
                .map(|core| InquiryReceipt {
                    core,
                    decoder,
                    owner: self.clone(),
                })
        })
    }

    /// Class-specific typed admission seam retaining operation semantics.
    pub(crate) fn submit_operation<K>(
        &self,
        prepared: crate::prepared::PreparedOperation<K>,
    ) -> Result<OperationReceipt<K, Self>, Error>
    where
        K: completion::Kind,
    {
        prepared.admit_with(|request, affected_axes, settlement, timeouts| {
            self.submit_with_timeout(request, timeouts.applied)
                .map(|core| OperationReceipt {
                    observation: OperationObservation::new(core, timeouts.cancellation),
                    affected_axes,
                    settlement,
                    owner: self.clone(),
                })
        })
    }

    /// Fails immediately when shared boundary/engine capacity is exhausted,
    /// then returns as soon as the worker admits the request. Admission is
    /// the success boundary: a transport write failure is reported through
    /// the receipt's outcome (D24).
    fn submit_with_timeout(
        &self,
        request: RuntimeRequest,
        configured_timeout: Duration,
    ) -> Result<ReceiptCore, Error> {
        let target = request.context().target;
        let (completion, admission) = self.core.enqueue_admission(request, None)?;
        let id = self.reply(&admission).wait()??;
        Ok(ReceiptCore::new(id, target, completion, configured_timeout))
    }

    fn submit_with_timeout_until(
        &self,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error> {
        let target = request.context().target;
        let (completion, admission, validity) =
            self.core
                .enqueue_admission_until(request, deadline, Instant::now())?;
        let reply = match self.reply(&admission).wait_deadline(deadline) {
            Ok(reply) => reply,
            Err(flume::select::SelectError::Timeout) => {
                self.core.expire_admission(&validity)?;
                // The worker claimed the boundary first, so its reply is
                // authoritative; the caller observes it and may later detach
                // its observer by the ordinary receipt path.
                self.reply(&admission).wait()
            }
        };
        let id = reply??;
        Ok(ReceiptCore::new(id, target, completion, configured_timeout))
    }

    /// Deliver one cancellation request to the worker and return its answer,
    /// all before `deadline` (#777). A full cancellation lane is
    /// backpressure; a closed one is the session's terminal error.
    fn request_cancellation(
        &self,
        request: CancellationRequest,
        deadline: Instant,
    ) -> Result<(), Error> {
        let id = request.id;
        let (reply, receiver) = flume::bounded(1);
        match self
            .core
            .cancellations
            .send_deadline(CancellationBoundary { request, reply }, deadline)
        {
            Ok(()) => {}
            Err(flume::SendTimeoutError::Timeout(_)) => {
                return Err(super::observation_timeout(id));
            }
            Err(flume::SendTimeoutError::Disconnected(_)) => {
                return Err(self.core.disconnected_error());
            }
        }
        match self.reply(&receiver).wait_deadline(deadline) {
            Ok(answer) => answer?,
            Err(flume::select::SelectError::Timeout) => Err(super::observation_timeout(id)),
        }
    }

    /// Reads scalar owner metrics through a dedicated bounded control request.
    /// This path never clones the diagnostic ring.
    pub(crate) fn metrics(&self) -> Result<crate::observability::MetricsSnapshot, Error> {
        self.control_request(ControlBoundary::Metrics)?
    }

    pub(crate) fn subscribe_diagnostics(
        &self,
        capacity: usize,
    ) -> Result<DiagnosticSubscription, Error> {
        self.control_request(|reply| ControlBoundary::SubscribeDiagnostics { capacity, reply })?
    }

    /// Installs new session tuning through the owner's control boundary (#631).
    ///
    /// The worker applies the update on its own turn, so the write is ordered
    /// against every other boundary message and against the scheduler itself,
    /// and this call returns once the owner has applied it.
    pub(crate) fn reconfigure(
        &self,
        validated_tuning: Result<crate::OperationalTuning, Error>,
    ) -> Result<(), Error> {
        self.control_request(|reply| ControlBoundary::Reconfigure {
            validated_tuning: Box::new(validated_tuning),
            reply,
        })?
    }

    pub(crate) fn state_cache(&self, target: crate::CameraId) -> crate::state_cache::StateCache {
        self.core.state_cache(target)
    }

    /// Reads the tuning the owner is currently preparing requests under.
    pub(crate) fn tuning(&self) -> crate::OperationalTuning {
        self.core.tuning()
    }

    /// Coalesced idempotent shutdown; see [`OwnerHandleCore::shutdown`].
    pub(crate) fn shutdown(&self) -> Result<(), Error> {
        self.core.shutdown()
    }

    /// Request shutdown and wait for the worker to release its transport.
    ///
    /// The first close joins the worker thread; a worker panic is reported as
    /// [`Error::InvalidState`]. Any other clone waits on the liveness lane,
    /// which the worker drops only after the transport. A close issued on the
    /// worker thread itself, by a custom transport calling back into its own
    /// session, requests shutdown but returns [`Error::InvalidState`] because
    /// it cannot wait for its own teardown. An external caller can still join
    /// it. Otherwise the result is the async `close` contract: an
    /// explicit shutdown returns `Ok(())`, a transport close or stream poison
    /// that won the race is returned unchanged, and a failed shutdown signal
    /// is preserved.
    pub(crate) fn close(&self) -> Result<(), Error> {
        let shutdown = self.shutdown();
        if self.worker_id == thread::current().id() {
            return Err(Error::InvalidState(
                "blocking owner worker cannot close its own session".into(),
            ));
        }
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let teardown = match worker {
            Some(worker) => match worker.join() {
                Ok(()) => self.core.closed_result(),
                Err(_) => Err(Error::InvalidState("blocking owner worker panicked".into())),
            },
            None => {
                // Nothing is ever sent on this lane; it disconnects once the
                // worker has dropped its transport.
                while self.core.actor_alive.recv().is_ok() {}
                self.core.closed_result()
            }
        };
        shutdown.and(teardown)
    }
}

/// A blocking operation receipt, as the public blocking `Operation` holds it.
pub(crate) type BlockingOperationReceipt<K> = OperationReceipt<K, BlockingOwnerHandle>;

impl CommandReceipt<BlockingOwnerHandle> {
    pub(crate) fn wait(self) -> Result<(), Error> {
        let deadline = self.owner.deadline_after(self.core.configured_timeout())?;
        wait_core_until(&self.core, &self.owner, deadline).and_then(normalize_command_outcome)
    }
}

impl<T> InquiryReceipt<T, BlockingOwnerHandle> {
    pub(crate) fn wait(self) -> Result<T, Error> {
        let deadline = self.owner.deadline_after(self.core.configured_timeout())?;
        self.wait_until(deadline)
    }

    fn wait_until(self, deadline: Instant) -> Result<T, Error> {
        let outcome = wait_core_until(&self.core, &self.owner, deadline)?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }
}

impl<K> OperationReceipt<K, BlockingOwnerHandle>
where
    K: completion::Kind,
{
    /// Waits for application, within `timeout` or the configured observer
    /// deadline.
    pub(crate) fn applied(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        let timeout = timeout.unwrap_or_else(|| self.observation.applied_timeout());
        let deadline = self.owner.deadline_after(timeout)?;
        self.applied_until(deadline)
    }

    fn applied_until(&mut self, deadline: Instant) -> Result<(), Error> {
        observe_until(
            &self.owner,
            &mut self.observation,
            deadline,
            OperationObservation::applied,
        )?
    }

    /// Cancels the operation and waits for the cancellation's conclusion,
    /// within `timeout` or the configured cancellation deadline (#777).
    ///
    /// A known terminal outcome answers without sending anything. Otherwise
    /// the handle's one cancellation intent is requested; a request for an
    /// intent the owner already holds observes it instead of sending another.
    /// A refusal leaves the operation running and this receipt intact.
    pub(crate) fn cancel(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<CancellationOutcome, Error> {
        let timeout = timeout.unwrap_or_else(|| self.observation.cancellation_timeout());
        let deadline = self.owner.deadline_after(timeout)?;
        if let Some(verdict) = self.observation.cancellation() {
            return verdict;
        }
        let request = self.observation.cancellation_request();
        if let Err(error) = self.owner.request_cancellation(request, deadline) {
            // A terminal outcome that raced the refusal still decides.
            return self.observation.cancellation().unwrap_or(Err(error));
        }
        observe_until(
            &self.owner,
            &mut self.observation,
            deadline,
            OperationObservation::cancellation,
        )?
    }
}

impl OperationReceipt<completion::Targeted, BlockingOwnerHandle> {
    /// Waits for application and then the profile-selected settlement
    /// condition, within `timeout` or the configured settlement budget.
    ///
    /// Application is cached, so a wait that times out during polling
    /// restarts from it with a fresh two-sample proof.
    pub(crate) fn settled(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        if self.observation.is_settled() {
            return Ok(());
        }
        let budget = settlement_budget(&self.settlement, timeout)?;
        let deadline = self.owner.deadline_after(budget)?;
        self.applied_until(deadline)?;
        if let Some(poll) = position_poll(&self.settlement, &self.observation, self.affected_axes)?
        {
            poll_settlement(&self.owner, poll, deadline)
                .map_err(|error| super::settlement_error(error, self.observation.id()))?;
        }
        self.observation.mark_settled();
        Ok(())
    }
}

/// Proves settlement by position polling: one baseline snapshot, then
/// snapshots every `interval` until the detector reports no movement, all
/// before one absolute deadline. The worker keeps the session progressing
/// between samples, so the caller only sleeps.
fn poll_settlement(
    owner: &BlockingOwnerHandle,
    poll: PositionPoll<'_>,
    deadline: Instant,
) -> Result<(), Error> {
    let mut detector = crate::prepared::MotionDetector::new(poll.axes, poll.tolerance);
    let baseline = sample_positions_blocking(owner, poll.queries, deadline)?;
    if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
        return Err(Error::InvalidState(
            "new movement detector rejected its baseline snapshot".into(),
        ));
    }
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Error::query_timeout());
        }
        thread::sleep(poll.interval.min(remaining));
        ensure_before_deadline(deadline)?;
        let snapshot = sample_positions_blocking(owner, poll.queries, deadline)?;
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
/// deadline. This helper is shared by targeted settlement and the standalone
/// owner-backed motion facade.
pub(crate) fn sample_positions_blocking(
    owner: &BlockingOwnerHandle,
    queries: &crate::prepared::PositionQueryPlan,
    deadline: Instant,
) -> Result<crate::prepared::PositionSnapshot, Error> {
    let mut snapshot = crate::prepared::PositionSnapshot::default();
    if let Some(query) = &queries.pan_tilt {
        snapshot.pan_tilt = Some(sample(owner, query, deadline)?);
    }
    if let Some(query) = &queries.zoom {
        snapshot.zoom = Some(sample(owner, query, deadline)?);
    }
    if let Some(query) = &queries.focus {
        snapshot.focus = Some(sample(owner, query, deadline)?);
    }
    if let Some(query) = &queries.iris {
        snapshot.iris = Some(sample(owner, query, deadline)?);
    }
    if let Some(query) = &queries.nd_filter {
        snapshot.nd_filter = Some(sample(owner, query, deadline)?);
    }
    Ok(snapshot)
}

/// One position inquiry, admitted and answered strictly before `deadline`.
fn sample<R>(
    owner: &BlockingOwnerHandle,
    query: &crate::prepared::PreparedInquiryTemplate<R>,
    deadline: Instant,
) -> Result<R, Error> {
    let sample = || {
        ensure_before_deadline(deadline)?;
        let receipt = owner.submit_inquiry_until(query.instantiate(), deadline)?;
        let value = receipt.wait_until(deadline)?;
        ensure_before_deadline(deadline)?;
        Ok(value)
    };
    sample().map_err(Error::into_query_timeout)
}

/// Rechecks the monotonic clock at every admission/sample boundary. In
/// particular, equality with the deadline is already too late for another
/// inquiry to be enqueued.
pub(crate) fn ensure_before_deadline(deadline: Instant) -> Result<(), Error> {
    if Instant::now() >= deadline {
        Err(Error::query_timeout())
    } else {
        Ok(())
    }
}

/// Waits for the next event on an operation's observation slots: the
/// terminal slot, the cancellation slot, worker liveness or the observer
/// deadline. Each arm only reads a slot, and the caller records every wake
/// before re-reading the verdict, so the order in which simultaneous wakes
/// are reported does not matter.
fn next_observation(
    owner: &BlockingOwnerHandle,
    terminal: &TerminalObserver,
    cancellation: Option<&CancellationObserver>,
    deadline: Instant,
) -> ObservationWake {
    let selector = flume::Selector::new()
        .recv(terminal.receiver(), |outcome| {
            ObservationWake::Terminal(outcome.ok())
        })
        // Nothing is ever sent on this lane. It fires when the worker has
        // dropped its sender, including an unwind before it could publish a
        // terminal owner error.
        .recv(&owner.core.actor_alive, |_| ObservationWake::OwnerGone);
    let selector = match cancellation {
        Some(observer) => selector.recv(observer.receiver(), |error| {
            ObservationWake::Cancellation(error.ok())
        }),
        None => selector,
    };
    selector
        .wait_deadline(deadline)
        .unwrap_or(ObservationWake::Deadline)
}

/// Waits until `verdict` can be read from an operation's observation state;
/// see [`OperationWait`].
fn observe_until<T>(
    owner: &BlockingOwnerHandle,
    observation: &mut OperationObservation,
    deadline: Instant,
    verdict: impl Fn(&mut OperationObservation) -> Option<T>,
) -> Result<T, Error> {
    let mut wait = OperationWait::new(observation, verdict);
    loop {
        if let Some(value) = wait.verdict() {
            return Ok(value);
        }
        let (terminal, cancellation) = wait.slots();
        let wake = next_observation(owner, terminal, cancellation, deadline);
        if let ControlFlow::Break(verdict) = wait.absorb(wake, &owner.core) {
            return verdict;
        }
    }
}

/// Waits for a command or inquiry receipt's terminal outcome.
fn wait_core_until(
    core: &ReceiptCore,
    owner: &BlockingOwnerHandle,
    deadline: Instant,
) -> Result<RuntimeOutcome, Error> {
    if let Some(outcome) = core.try_outcome() {
        return Ok(outcome);
    }
    next_observation(owner, &core.completion, None, deadline).conclude(core, &owner.core)
}

#[cfg(test)]
mod tests;
