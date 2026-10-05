//! Runtime-neutral async owner actor and its handle over the shared boundary.

use std::{
    collections::VecDeque,
    future::Future,
    pin::{pin, Pin},
    sync::Arc,
    task::Poll,
    time::{Duration, Instant},
};

use futures_lite::future;

use crate::{executor::Executor, Error};

use super::boundary::{BoundaryReceivers, OwnerHandleCore, OwnerSnapshot};
use super::receipt::{ObservationWake, OperationReceipt};
use super::shell::{ClassifiedEvent, OwnerEvent, OwnerShellCore, TurnStep};
use super::{
    CancellationObserver, OwnerInputTurn, OwnerPolicy, OwnerReceive, OwnerState,
    RetainedStreamInput, TerminalObserver, TransmissionMeta, WireWrite,
};

use super::turn::{ReceiveArm, Selection, Source, TimerArm, TurnOutcome, TurnPlan};
#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
use super::{
    boundary::ControlBoundary, receipt::CommandReceipt, CancellationRequest, ReceiptCore,
    RuntimeRequest,
};
#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
use super::{
    MAXIMUM_TRANSIENT_RECEIVE_PAUSE, TRANSIENT_RECEIVE_FAULT_LIMIT, TRANSIENT_RECEIVE_FAULT_RESET,
    TRANSIENT_RECEIVE_FAULT_SPAN, TRANSIENT_RECEIVE_PAUSE,
};
use crate::runtime::engine::Effect;

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

/// Select one actor event as planned by the [`OwnerCoordinator`]. The caller
/// has already mapped the selection's receive and timer arms onto `receive`
/// and `timer`. A disconnected shutdown, cancellation or control lane never
/// becomes ready; a disconnected admission lane is reported.
async fn select_actor_event<Receive, Timer>(
    selection: Selection,
    receive: Receive,
    boundaries: &BoundaryReceivers,
    timer: Timer,
) -> OwnerEvent
where
    Receive: Future<Output = OwnerEvent>,
    Timer: Future<Output = OwnerEvent>,
{
    let shutdown = async {
        match boundaries.shutdown.recv_async().await {
            Ok(()) => OwnerEvent::Shutdown,
            Err(_) => future::pending().await,
        }
    };
    let cancellation = async {
        match boundaries.cancellations.recv_async().await {
            Ok(value) => OwnerEvent::Cancellation(value),
            Err(_) => future::pending().await,
        }
    };
    let admission = async { OwnerEvent::Admission(boundaries.admissions.recv_async().await) };
    let control = async {
        match boundaries.control.recv_async().await {
            Ok(value) => OwnerEvent::Control(value),
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
async fn raw_release_wake<R: Executor>(runtime: &R, deadline: Instant, now: Instant) -> OwnerEvent {
    let remaining = deadline.saturating_duration_since(now);
    if !remaining.is_zero() {
        Executor::sleep(runtime, remaining).await;
    }
    OwnerEvent::Wake
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
    ) -> impl Future<Output = Result<OwnerReceive, Error>> + Send;
}

/// An async operation receipt, as the public [`crate::Operation`] holds it.
pub(crate) type AsyncOperationReceipt<K> = OperationReceipt<K, AsyncOwnerHandle>;

super::handle::owner_handle_methods! {
    handle: AsyncOwnerHandle,
    async: [async],
    await: [.await],
    block: [async move],
}

/// Cloneable async owner handle: the shared boundary core plus the
/// executor clock its waits are measured against.
#[derive(Debug, Clone)]
pub(crate) struct AsyncOwnerHandle {
    core: OwnerHandleCore,
    clock: BoundClock,
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
        let _ = self.core.actor_alive.recv_async().await;
        self.core.closed_result()
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
    async fn boundary_reply<T>(&self, reply: &flume::Receiver<T>) -> Result<T, Error>
    where
        T: Send,
    {
        let answered = async {
            reply
                .recv_async()
                .await
                .map_err(|_| self.core.disconnected_error())
        };
        let actor_gone = async {
            // Nobody ever sends on this lane, so this resolves exactly once,
            // when actor teardown drops its end after the driver.
            while self.core.actor_alive.recv_async().await.is_ok() {}
            reply.try_recv().map_err(|_| self.core.disconnected_error())
        };
        future::or(answered, actor_gone).await
    }

    /// [`Self::boundary_reply`], or `None` once `deadline` passes first.
    async fn boundary_reply_until<T>(
        &self,
        reply: &flume::Receiver<T>,
        deadline: Instant,
    ) -> Option<Result<T, Error>>
    where
        T: Send,
    {
        self.until(deadline, self.boundary_reply(reply)).await
    }

    async fn send_lane<T>(&self, lane: &flume::Sender<T>, message: T) -> Result<(), Error>
    where
        T: Send,
    {
        lane.send_async(message)
            .await
            .map_err(|_| self.core.disconnected_error())
    }

    /// [`Self::send_lane`], or `None` once a full lane outlasts `deadline`.
    async fn send_lane_until<T>(
        &self,
        lane: &flume::Sender<T>,
        message: T,
        deadline: Instant,
    ) -> Option<Result<(), Error>>
    where
        T: Send,
    {
        self.until(deadline, self.send_lane(lane, message)).await
    }

    /// `work`'s output, or `None` once `deadline` passes on the owner clock
    /// first. The work is polled first, so one ready at the deadline wins.
    async fn until<T>(&self, deadline: Instant, work: impl Future<Output = T>) -> Option<T> {
        let remaining = deadline.saturating_duration_since(self.now());
        future::or(async { Some(work.await) }, async {
            self.sleep(remaining).await;
            None
        })
        .await
    }

    /// The owner clock every handle wait is measured against.
    pub(crate) fn now(&self) -> Instant {
        self.clock.now()
    }

    pub(crate) fn sleep(
        &self,
        duration: Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.clock.sleep(duration)
    }

    /// Waits for the next event on an operation's observation slots. This is
    /// the one race every async receipt wait uses: the terminal slot, then the
    /// cancellation slot, then actor liveness, then the observer deadline, in
    /// left-biased order. Each arm only reads a slot; flume leaves an unread
    /// value queued if this future is dropped.
    async fn next_observation(
        &self,
        terminal: &TerminalObserver,
        cancellation: Option<&CancellationObserver>,
        deadline: Instant,
    ) -> ObservationWake {
        let remaining = deadline.saturating_duration_since(self.now());
        let terminal = async { ObservationWake::Terminal(terminal.recv_observed_async().await) };
        let cancellation = async {
            match cancellation {
                Some(observer) => {
                    ObservationWake::Cancellation(observer.recv_observed_async().await)
                }
                None => future::pending().await,
            }
        };
        let owner_gone = async {
            // Nothing is ever sent on this lane. It resolves when the actor has
            // dropped its sender, including an unwind before `run` can latch a
            // terminal owner error.
            while self.core.actor_alive.recv_async().await.is_ok() {}
            ObservationWake::OwnerGone
        };
        let timer = async {
            self.sleep(remaining).await;
            ObservationWake::Deadline
        };
        future::or(
            terminal,
            future::or(cancellation, future::or(owner_gone, timer)),
        )
        .await
    }

    /// Admits `request` under the inquiry or completion timeout its own
    /// context configures.
    // Used only by owner unit tests; the public session uses the typed seams.
    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    pub(crate) async fn submit(&self, request: RuntimeRequest) -> Result<ReceiptCore, Error> {
        let timeout = test_observer_timeout(&request);
        self.submit_with_timeout(request, timeout).await
    }

    /// Non-waiting admission used by capacity-sensitive owner tests. Failure
    /// occurs before an observer or engine ID is created.
    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) fn try_submit(
        &self,
        request: RuntimeRequest,
    ) -> Result<impl Future<Output = Result<ReceiptCore, Error>> + Send + '_, Error> {
        let timeout = test_observer_timeout(&request);
        let pending = self.core.admit(request)?;
        Ok(async move {
            let reply = self.boundary_reply(pending.reply()).await;
            pending.receipt(reply, timeout)
        })
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
        self.control_request(ControlBoundary::Snapshot).await
    }
}

/// The observer timeout owner tests admit a raw request under: its context's
/// inquiry or completion timeout.
#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
fn test_observer_timeout(request: &RuntimeRequest) -> Duration {
    if request.is_inquiry() {
        request.context().timeout.inquiry
    } else {
        request.context().timeout.completion
    }
}

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
impl CommandReceipt<AsyncOwnerHandle> {
    /// Waits for the command's outcome within `timeout` instead of its
    /// configured observer deadline.
    pub(crate) async fn wait_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        let deadline = self.owner.deadline_after(timeout)?;
        self.wait_until(deadline).await
    }
}

/// The async owner shell: an executor-driven I/O dispatcher over the shared
/// [`OwnerShellCore`], which makes every arbitration and turn decision.
#[derive(Debug)]
pub(crate) struct AsyncOwnerActor<R>
where
    R: Executor,
{
    core: OwnerShellCore,
    runtime: Arc<R>,
}

impl<R> AsyncOwnerActor<R>
where
    R: Executor,
{
    pub(crate) fn new(policy: OwnerPolicy, runtime: R) -> Result<(AsyncOwnerHandle, Self), Error> {
        let state = OwnerState::new(policy)?;
        let runtime = Arc::new(runtime);
        let (core, ends) = OwnerHandleCore::new(&state);
        Ok((
            AsyncOwnerHandle {
                core,
                clock: BoundClock::from_shared(Arc::clone(&runtime)),
            },
            Self {
                core: OwnerShellCore::new(state, ends),
                runtime,
            },
        ))
    }

    pub(crate) async fn run<D>(mut self, mut driver: D) -> OwnerSnapshot
    where
        D: AsyncOwnerDriver,
    {
        let runtime = Arc::clone(&self.runtime);
        // Enforced around each receive so the caller's advertised read timeout is
        // live on the async surface, where the runtime-agnostic transports have
        // no timer of their own (#675). A timed-out read consumed nothing, so it
        // is reported as an idle no-data receive.
        let read_timeout = self.core.state.policy().read_timeout;
        // Every arbitration and turn decision below is the shell core's; this
        // loop only samples the clock, polls the futures the plan describes,
        // and runs the I/O the selected event's turn asks for.
        while self.core.is_running() {
            if self.core.begin_turn() {
                // Polling boundaries first alone is not a cooperative handoff:
                // if they are all pending, the ready receive wins immediately
                // and this task can monopolize a single-thread executor.
                cooperative_yield().await;
            }
            // Sample after the cooperative yield: a timer that became due
            // while another task ran must not inherit a stale positive delay.
            let now = Executor::now(runtime.as_ref());
            let event = match self.core.plan(now, &mut driver) {
                TurnPlan::Redeliver(retained) => self.core.redeliver(retained),
                TurnPlan::Select(selection) => {
                    let frame_limit = self.core.state.policy().limits.frames_per_receive;
                    let receive = async {
                        match selection.receive {
                            // The exact no-input probe for this release set is
                            // done: the receive source is immediately ready
                            // with the timer event instead of another read.
                            ReceiveArm::ProofComplete => return OwnerEvent::Wake,
                            // An idle or faulted read paces the next one; the
                            // other sources stay live while this one waits.
                            ReceiveArm::Paced { until } => {
                                Executor::sleep(
                                    runtime.as_ref(),
                                    until.saturating_duration_since(now),
                                )
                                .await;
                            }
                            ReceiveArm::Poll => {}
                        }
                        let result = Self::receive_within(
                            &mut driver,
                            self.core.state.buffers(),
                            frame_limit,
                            runtime.as_ref(),
                            read_timeout,
                        )
                        .await;
                        OwnerEvent::Receive {
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
                                // due, and `OwnerEvent::Wake` remains the only
                                // path that advances engine time.
                                if !due {
                                    let delay = at.map_or(Duration::from_secs(86_400), |wake| {
                                        wake.saturating_duration_since(now)
                                    });
                                    Executor::sleep(runtime.as_ref(), delay).await;
                                }
                                OwnerEvent::Wake
                            }
                        }
                    };
                    select_actor_event(selection, receive, &self.core.receivers, wake).await
                }
            };

            let selected_at = Executor::now(runtime.as_ref());
            let Some(ClassifiedEvent {
                event,
                selected,
                suppress_due,
            }) = self.core.classify(event, selected_at)
            else {
                continue;
            };
            let control_consumed_wake = match &event {
                OwnerEvent::Control(_) => self
                    .core
                    .control_consumed_wake(Executor::now(runtime.as_ref())),
                _ => None,
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
            if !self.core.finish(selected, control_consumed_wake, outcome) {
                break;
            }
        }
        let snapshot = self.core.conclude();
        // #626: drop the driver first so a waiter using the liveness lane gets
        // a deterministic transport-release barrier. The shell core's `Drop`
        // performs one final boundary drain before its alive sender is
        // dropped, covering messages that race the explicit drain.
        drop(driver);
        snapshot
    }

    /// Run the engine turn of one classified event, performing the writes and
    /// pauses its [`TurnStep`]s ask for.
    async fn handle_event<D>(
        &mut self,
        event: OwnerEvent,
        driver: &mut D,
        runtime: &R,
        selected_at: Instant,
        suppress_due_for_raw_probe_fault: bool,
    ) -> TurnOutcome
    where
        D: AsyncOwnerDriver,
    {
        let mut step =
            self.core
                .handle(event, driver, selected_at, suppress_due_for_raw_probe_fault);
        loop {
            match step {
                TurnStep::Drive { effects, then } => {
                    self.drive(driver, effects, then.input_turn(), runtime)
                        .await;
                    step = self.core.resume(then, driver);
                }
                TurnStep::Pace { pause, outcome } => {
                    self.core.pace_receive(pause, Executor::now(runtime));
                    return outcome;
                }
                TurnStep::Done(outcome) => return outcome,
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
    ) -> Result<OwnerReceive, Error>
    where
        D: AsyncOwnerDriver,
    {
        let idle_after_timeout = async {
            Executor::sleep(runtime, read_timeout).await;
            Ok(OwnerReceive::NoData)
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

    /// Apply `effects` in order, writing each staged transmit under the
    /// session's write timeout. Inside an input `turn`, a transmit finishes in
    /// that turn; otherwise it finishes at the instant the write returned.
    async fn drive<D>(
        &mut self,
        driver: &mut D,
        mut effects: VecDeque<Effect>,
        turn: Option<&OwnerInputTurn>,
        runtime: &R,
    ) where
        D: AsyncOwnerDriver,
    {
        let write_timeout = self.core.state.policy().write_timeout;
        while let Some(staged) = self
            .core
            .next_transmit(&mut effects, OwnerClock::now(&self.runtime))
        {
            let result = match self.core.state.prepare_write(&staged) {
                Ok(write) => Self::write_frame(driver, write, runtime, write_timeout).await,
                Err(error) => Err(error),
            };
            self.core
                .finish_transmit(turn, &staged, result, Executor::now(runtime), &mut effects);
        }
    }
}

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
mod tests;
