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
//! The worker and its handles read time and wait through one [`Clock`]: the
//! system clock in production, or a test's virtual clock.
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
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::Error;

use super::boundary::OwnerHandleCore;
use super::clock::{Clock, Select};
use super::receipt::{ObservationWake, OperationReceipt};
use super::shell::{ClassifiedEvent, OwnerEvent, OwnerShellCore, TurnStep};
use super::turn::{ReceiveArm, Selection, Source, TimerArm, TurnOutcome, TurnPlan};
use super::{
    CancellationObserver, OwnerBuffers, OwnerInputTurn, OwnerPolicy, OwnerReceive, OwnerState,
    RetainedStreamInput, TerminalObserver, TransmissionMeta, WireWrite,
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
    clock: Clock,
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
            let now = self.clock.now();
            let event = match self.core.plan(now, &mut driver) {
                TurnPlan::Redeliver(retained) => self.core.redeliver(retained),
                TurnPlan::Select(selection) => self.select(selection, &mut driver, read_timeout),
            };
            let selected_at = self.clock.now();
            let Some(ClassifiedEvent {
                event,
                selected,
                suppress_due,
            }) = self.core.classify(event, selected_at)
            else {
                continue;
            };
            let control_consumed_wake = match &event {
                OwnerEvent::Control(_) => self.core.control_consumed_wake(self.clock.now()),
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
                        ReceiveArm::Paced { until } if self.clock.now() < until => None,
                        ReceiveArm::Poll | ReceiveArm::Paced { .. } => {
                            read = true;
                            let later_ready = selection
                                .order()
                                .skip(position.saturating_add(1))
                                .any(|later| self.is_ready(later, selection.timer));
                            let timeout = if later_ready {
                                READY_PROBE
                            } else {
                                read_slice(
                                    selection.timer,
                                    read_timeout.saturating_sub(idle),
                                    self.clock.now(),
                                )
                            };
                            receive(
                                &mut self.core.state,
                                driver,
                                &self.clock,
                                ReadBudget {
                                    frame_limit,
                                    timeout,
                                    read_timeout,
                                },
                                &mut idle,
                            )
                        }
                    },
                    Source::Timer => {
                        timer_ready(selection.timer, self.clock.now()).then_some(OwnerEvent::Wake)
                    }
                    lane => self.try_lane(lane),
                };
                if let Some(event) = event {
                    return event;
                }
            }
            if !read {
                // Receive is paced and no other source is ready: wait for the
                // pace, the timer or the next slice, whichever comes first.
                self.clock.sleep(paced_wait(selection, self.clock.now()));
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
            Source::Timer => timer_ready(timer, self.clock.now()),
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
                    self.core.pace_receive(pause, self.clock.now());
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
        while let Some(staged) = self.core.next_transmit(&mut effects, self.clock.now()) {
            let result = match self.core.state.prepare_write(&staged) {
                Ok(write) => driver.write(write),
                Err(error) => Err(error),
            };
            self.core
                .finish_transmit(turn, &staged, result, self.clock.now(), &mut effects);
        }
    }
}

/// The bounds of one transport read.
#[derive(Debug, Clone, Copy)]
struct ReadBudget {
    frame_limit: usize,
    /// How long this read may wait.
    timeout: Duration,
    /// The caller's read timeout, after which a selection's reads without
    /// data are an idle receive.
    read_timeout: Duration,
}

/// One bounded read. Returns `None` while the receive is still pending: the
/// read waited out its slice and the selection's read timeout has not elapsed.
fn receive<D>(
    state: &mut OwnerState,
    driver: &mut D,
    clock: &Clock,
    budget: ReadBudget,
    idle: &mut Duration,
) -> Option<OwnerEvent>
where
    D: BlockingOwnerDriver,
{
    let ReadBudget {
        frame_limit,
        timeout,
        read_timeout,
    } = budget;
    let started = clock.now();
    let result = driver.receive(state.buffers(), frame_limit, timeout);
    // Sampled after the read returns: only this instant proves that input was
    // sampled at or after a raw hold deadline.
    let received_at = clock.now();
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

/// Whether the planned timer is due at `now`.
fn timer_ready(timer: TimerArm, now: Instant) -> bool {
    match timer {
        TimerArm::Engine { due: true, .. } => true,
        TimerArm::Engine { at, due: false } => at.is_some_and(|at| at <= now),
        TimerArm::Grace { until } => until <= now,
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
fn read_slice(timer: TimerArm, idle_budget: Duration, now: Instant) -> Duration {
    CONTROL_SLICE
        .min(idle_budget)
        .min(until_timer(timer, now))
        .max(READY_PROBE)
}

/// How long to wait while receive is paced and nothing else is ready.
fn paced_wait(selection: Selection, now: Instant) -> Duration {
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
    /// The clock the worker runs on and every handle wait is measured
    /// against.
    clock: Clock,
    /// Retained after another closer takes the join handle, so a transport
    /// callback can never wait for its own worker's teardown.
    worker_id: thread::ThreadId,
    /// Taken by the first `close`; every other clone waits on the liveness
    /// lane instead. Dropping the last handle detaches the worker, which then
    /// stops on its own because every boundary lane has disconnected.
    worker: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl BlockingOwnerHandle {
    /// Start the worker thread over `driver`, keeping time on `clock`.
    ///
    /// Every failure, including a failed thread spawn, drops the driver (and
    /// its transport) and returns the error; no handle and no partially
    /// started worker remain. `Ok` means the worker is running.
    pub(crate) fn spawn<D>(policy: OwnerPolicy, driver: D, clock: Clock) -> Result<Self, Error>
    where
        D: BlockingOwnerDriver + 'static,
    {
        let state = OwnerState::new(policy)?;
        let (core, ends) = OwnerHandleCore::new(&state);
        let worker = BlockingOwnerWorker {
            core: OwnerShellCore::new(state, ends),
            clock: clock.clone(),
        };
        // Registered before the spawn, so a virtual clock never advances
        // while the worker is starting.
        let registration = clock.register_thread();
        let worker = thread::Builder::new()
            .name("grafton-visca-owner".into())
            .spawn(move || {
                let _participation = registration.enter();
                worker.run(driver);
            })?;
        Ok(Self {
            core,
            clock,
            worker_id: worker.thread().id(),
            worker: Arc::new(Mutex::new(Some(worker))),
        })
    }

    /// The owner clock every handle wait is measured against.
    pub(crate) fn now(&self) -> Instant {
        self.clock.now()
    }

    fn sleep(&self, duration: Duration) {
        self.clock.sleep(duration);
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
    fn reply<'a, T: 'a>(&'a self, reply: &'a flume::Receiver<T>) -> Select<'a, Result<T, Error>> {
        self.clock
            .select()
            .recv(reply, |answer| {
                resolve_selected_receive(reply, answer).map_err(|_| self.core.disconnected_error())
            })
            // Nothing is ever sent on this lane, so it fires exactly once,
            // when worker teardown drops its end after the driver.
            .recv(&self.core.actor_alive, |_| {
                reply.try_recv().map_err(|_| self.core.disconnected_error())
            })
    }

    fn boundary_reply<T>(&self, reply: &flume::Receiver<T>) -> Result<T, Error> {
        self.reply(reply).wait()
    }

    /// [`Self::boundary_reply`], or `None` once `deadline` passes first.
    fn boundary_reply_until<T>(
        &self,
        reply: &flume::Receiver<T>,
        deadline: Instant,
    ) -> Option<Result<T, Error>> {
        self.reply(reply).wait_until(deadline)
    }

    fn send_lane<T>(&self, lane: &flume::Sender<T>, message: T) -> Result<(), Error> {
        lane.send(message)
            .map_err(|_| self.core.disconnected_error())
    }

    /// [`Self::send_lane`], or `None` once a full lane outlasts `deadline`.
    fn send_lane_until<T>(
        &self,
        lane: &flume::Sender<T>,
        message: T,
        deadline: Instant,
    ) -> Option<Result<(), Error>> {
        self.clock
            .send_until(lane, message, deadline)
            .map(|sent| sent.map_err(|_| self.core.disconnected_error()))
    }

    /// Waits for the next event on an operation's observation slots: the
    /// terminal slot, the cancellation slot, worker liveness or the observer
    /// deadline. Each arm only reads a slot, and the caller records every
    /// wake before re-reading the verdict, so the order in which
    /// simultaneous wakes are reported does not matter.
    fn next_observation(
        &self,
        terminal: &TerminalObserver,
        cancellation: Option<&CancellationObserver>,
        deadline: Instant,
    ) -> ObservationWake {
        let selector = self
            .clock
            .select()
            .recv(terminal.receiver(), |outcome| {
                ObservationWake::Terminal(
                    resolve_selected_receive(terminal.receiver(), outcome).ok(),
                )
            })
            // Nothing is ever sent on this lane. It fires when the worker has
            // dropped its sender, including an unwind before it could publish
            // a terminal owner error.
            .recv(&self.core.actor_alive, |_| ObservationWake::OwnerGone);
        let selector = match cancellation {
            Some(observer) => selector.recv(observer.receiver(), |error| {
                ObservationWake::Cancellation(
                    resolve_selected_receive(observer.receiver(), error).ok(),
                )
            }),
            None => selector,
        };
        selector
            .wait_until(deadline)
            .unwrap_or(ObservationWake::Deadline)
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
        // Nothing is ever sent on this lane; it disconnects once the worker
        // has dropped its transport, including on an unwind. Waiting for that
        // first keeps the join itself on the owner's clock.
        self.clock.await_disconnect(&self.core.actor_alive);
        let teardown = match worker {
            Some(worker) => match worker.join() {
                Ok(()) => self.core.closed_result(),
                Err(_) => Err(Error::InvalidState("blocking owner worker panicked".into())),
            },
            None => self.core.closed_result(),
        };
        shutdown.and(teardown)
    }
}

/// A blocking operation receipt, as the public blocking `Operation` holds it.
pub(crate) type BlockingOperationReceipt<K> = OperationReceipt<K, BlockingOwnerHandle>;

super::handle::owner_handle_methods! {
    handle: BlockingOwnerHandle,
    async: [],
    await: [],
    block: [],
}

/// A selector can observe Empty, then a final send followed by disconnection,
/// and report Disconnected while that last value is still queued. Recover it
/// before interpreting the event as owner loss or an empty observation slot.
fn resolve_selected_receive<T>(
    receiver: &flume::Receiver<T>,
    selected: Result<T, flume::RecvError>,
) -> Result<T, flume::RecvError> {
    selected.or_else(|_| {
        receiver
            .try_recv()
            .map_err(|_| flume::RecvError::Disconnected)
    })
}

#[cfg(test)]
mod tests;
