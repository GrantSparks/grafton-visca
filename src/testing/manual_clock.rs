//! A virtual clock for owner deadline tests: see [`ManualClock`].

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

#[cfg(all(feature = "async", feature = "test-utils"))]
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Wake, Waker},
};

#[cfg(all(feature = "async", feature = "test-utils"))]
use crate::{executor::Executor, Error};

/// Why a blocked thread was rung.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Bell {
    /// Re-check the sources without blocking, then confirm the round.
    Verify(u64),
    /// The wait's virtual deadline has passed.
    Expired,
}

/// The sources one clock wait selects over.
pub(crate) trait Source<T> {
    /// A value that is ready now, without blocking.
    fn poll(&mut self) -> Option<T>;

    /// Block until a value is ready, or `bell` rings with the returned reason.
    fn block(&mut self, bell: &flume::Receiver<Bell>) -> Result<T, Bell>;
}

/// A source that is only ever checked when the clock rings.
struct Polled<F>(F);

impl<T, F> Source<T> for Polled<F>
where
    F: FnMut() -> Option<T>,
{
    fn poll(&mut self) -> Option<T> {
        (self.0)()
    }

    fn block(&mut self, bell: &flume::Receiver<Bell>) -> Result<T, Bell> {
        // The wait holds the bell's sender until it returns, so the channel
        // cannot disconnect here; treat it as expiry rather than spin.
        Err(bell.recv().unwrap_or(Bell::Expired))
    }
}

/// One thread blocked in a clock wait.
#[derive(Debug)]
struct Waiter {
    deadline: Option<Instant>,
    bell: flume::Sender<Bell>,
    /// Registrations the blocked thread holds, removed from `running` while
    /// it is blocked.
    registrations: usize,
    /// The last verification round this waiter confirmed.
    confirmed: Option<u64>,
    /// The clock has ended this wait and counted its thread as running.
    expired: bool,
}

/// One tracked task's scheduling state.
#[cfg(all(feature = "async", feature = "test-utils"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    /// Woken and not yet polled.
    Scheduled,
    /// Being polled.
    Polling,
    /// Woken while being polled.
    PollingWoken,
    /// Pending, waiting for a wake.
    Idle,
}

/// Wake-ups collected under the state lock and delivered after it.
#[derive(Default)]
struct Wakes {
    bells: Vec<(flume::Sender<Bell>, Bell)>,
    #[cfg(all(feature = "async", feature = "test-utils"))]
    wakers: Vec<Waker>,
}

#[derive(Debug)]
struct State {
    now: Instant,
    /// Bumped by every change that can make a participant runnable or move a
    /// deadline; a verification round is valid only while it is unchanged.
    epoch: u64,
    /// The epoch a verification round was last started for.
    round: Option<u64>,
    /// Participant registrations whose thread is not blocked on the clock,
    /// including registrations not yet bound to their thread.
    running: usize,
    /// Registrations bound to each participant thread.
    threads: HashMap<ThreadId, usize>,
    waiters: BTreeMap<u64, Waiter>,
    next_id: u64,
    #[cfg(all(feature = "async", feature = "test-utils"))]
    tasks: HashMap<u64, TaskState>,
    /// Tasks scheduled or being polled.
    #[cfg(all(feature = "async", feature = "test-utils"))]
    runnable: usize,
    #[cfg(all(feature = "async", feature = "test-utils"))]
    timers: BTreeMap<(Instant, u64), Waker>,
    /// Timer wakes taken from `timers` but not yet delivered. Until they are,
    /// the tasks they wake are not yet marked runnable.
    #[cfg(all(feature = "async", feature = "test-utils"))]
    undelivered: usize,
}

impl State {
    fn new(now: Instant) -> Self {
        Self {
            now,
            epoch: 0,
            round: None,
            running: 0,
            threads: HashMap::new(),
            waiters: BTreeMap::new(),
            next_id: 0,
            #[cfg(all(feature = "async", feature = "test-utils"))]
            tasks: HashMap::new(),
            #[cfg(all(feature = "async", feature = "test-utils"))]
            runnable: 0,
            #[cfg(all(feature = "async", feature = "test-utils"))]
            timers: BTreeMap::new(),
            #[cfg(all(feature = "async", feature = "test-utils"))]
            undelivered: 0,
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn changed(&mut self) {
        self.epoch += 1;
    }

    /// Nothing can run until the clock wakes it.
    fn quiescent(&self) -> bool {
        #[cfg(all(feature = "async", feature = "test-utils"))]
        if self.runnable != 0 || self.undelivered != 0 {
            return false;
        }
        self.running == 0
    }

    /// After a change: when nothing is runnable, start a verification round
    /// for the blocked threads, or advance at once when there are none.
    fn settle(&mut self, wakes: &mut Wakes) {
        if !self.quiescent() || self.round == Some(self.epoch) {
            return;
        }
        self.round = Some(self.epoch);
        let round = self.epoch;
        let mut blocked = false;
        for waiter in self.waiters.values().filter(|waiter| !waiter.expired) {
            blocked = true;
            wakes.bells.push((waiter.bell.clone(), Bell::Verify(round)));
        }
        if !blocked {
            self.advance_to_next(wakes);
        }
    }

    /// Waiter `id` found nothing ready in `round`. The last confirmation of a
    /// still-valid round advances time.
    fn confirm(&mut self, id: u64, round: u64, wakes: &mut Wakes) {
        if round != self.epoch || !self.quiescent() {
            return;
        }
        if let Some(waiter) = self.waiters.get_mut(&id) {
            waiter.confirmed = Some(round);
        }
        if self
            .waiters
            .values()
            .all(|waiter| waiter.expired || waiter.confirmed == Some(round))
        {
            self.advance_to_next(wakes);
        }
    }

    /// Jump to the earliest pending deadline, if there is one.
    fn advance_to_next(&mut self, wakes: &mut Wakes) {
        let waits = self
            .waiters
            .values()
            .filter(|waiter| !waiter.expired)
            .filter_map(|waiter| waiter.deadline);
        #[cfg(all(feature = "async", feature = "test-utils"))]
        let waits = waits.chain(self.timers.keys().map(|(deadline, _)| *deadline));
        if let Some(next) = waits.min() {
            self.advance_to(next, wakes);
        }
    }

    /// Move to `target` (never backwards) and end every wait it passes.
    fn advance_to(&mut self, target: Instant, wakes: &mut Wakes) {
        self.now = self.now.max(target);
        self.changed();
        let now = self.now;
        let mut resumed = 0;
        for waiter in self.waiters.values_mut() {
            if !waiter.expired && waiter.deadline.is_some_and(|deadline| deadline <= now) {
                waiter.expired = true;
                resumed += waiter.registrations;
                wakes.bells.push((waiter.bell.clone(), Bell::Expired));
            }
        }
        self.running += resumed;
        #[cfg(all(feature = "async", feature = "test-utils"))]
        {
            let later = self.timers.split_off(&(now, u64::MAX));
            let due = std::mem::replace(&mut self.timers, later);
            self.undelivered += due.len();
            wakes.wakers.extend(due.into_values());
        }
    }

    fn registrations(&self, thread: ThreadId) -> usize {
        self.threads.get(&thread).copied().unwrap_or(0)
    }

    fn bind(&mut self, thread: ThreadId) {
        *self.threads.entry(thread).or_insert(0) += 1;
    }

    fn unbind(&mut self, thread: ThreadId) {
        if let Some(count) = self.threads.get_mut(&thread) {
            *count -= 1;
            if *count == 0 {
                self.threads.remove(&thread);
            }
        }
    }
}

/// A virtual clock: owner time that moves only when a test advances it or
/// every participant is blocked on it. Clones share one clock.
///
/// [`ManualClock`] is the time source a test injects into a blocking owner
/// (by opening the session through the clock) or an async owner (by running
/// it on a `ManualClockExecutor`). Owner deadlines, pacing, read timeouts and caller
/// waits are then measured on virtual time, which moves in two ways:
///
/// * explicitly, by [`ManualClock::advance`];
/// * on its own, as Tokio's paused clock does: when every participant is
///   blocked on the clock, time jumps to the earliest pending deadline and
///   wakes exactly the waits that deadline ends.
///
/// A participant is a thread registered with [`ManualClock::enter`] or started
/// by [`ManualClock::spawn`] (a blocking owner registers its worker itself),
/// or a task spawned or blocked on through a `ManualClockExecutor`. A thread
/// that is not a participant may still wait on the clock, but its running
/// never holds time back, so a blocking test enters the clock on its own
/// thread before it opens a session.
///
/// Quiescence is verified rather than inferred from counters. A blocked
/// thread can already have been handed an event that it has not yet
/// consumed, so when nothing is runnable the clock rings every blocked
/// thread to re-check its sources without blocking; time advances only once
/// each of them confirms the same round with nothing ready. A task is tracked
/// by its waker, which marks it runnable before the waking call returns.
///
/// ```
/// use std::time::Duration;
/// use grafton_visca::testing::testkit::ManualClock;
///
/// let clock = ManualClock::new();
/// let _driver = clock.enter();
/// let start = clock.now();
/// // The only participant is blocked, so time jumps straight to the deadline.
/// clock.sleep(Duration::from_secs(3_600));
/// assert_eq!(clock.now() - start, Duration::from_secs(3_600));
/// ```
#[derive(Clone)]
pub struct ManualClock {
    state: Arc<Mutex<State>>,
}

impl fmt::Debug for ManualClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManualClock")
            .field("now", &self.now())
            .finish_non_exhaustive()
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl ManualClock {
    /// A clock that starts at the current instant and moves only virtually.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::new(Instant::now()))),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Run `change` under the state lock, then deliver the wake-ups it
    /// collected.
    fn update<R>(&self, change: impl FnOnce(&mut State, &mut Wakes) -> R) -> R {
        let mut wakes = Wakes::default();
        let result = change(&mut self.lock(), &mut wakes);
        self.deliver(wakes);
        result
    }

    fn deliver(&self, wakes: Wakes) {
        for (bell, reason) in wakes.bells {
            // A waiter that already returned dropped its receiver.
            let _ = bell.send(reason);
        }
        #[cfg(all(feature = "async", feature = "test-utils"))]
        if !wakes.wakers.is_empty() {
            let delivered = wakes.wakers.len();
            for waker in wakes.wakers {
                waker.wake();
            }
            self.update(|state, wakes| {
                state.undelivered -= delivered;
                state.changed();
                state.settle(wakes);
            });
        }
    }

    /// The current virtual instant.
    #[must_use]
    pub fn now(&self) -> Instant {
        self.lock().now
    }

    /// Move virtual time forward by `duration`, ending every wait whose
    /// deadline that passes.
    pub fn advance(&self, duration: Duration) {
        self.update(|state, wakes| {
            let target = state.now.checked_add(duration).unwrap_or(state.now);
            state.advance_to(target, wakes);
        });
    }

    /// Register the calling thread as a participant until the returned guard
    /// drops. While it is registered, virtual time advances on its own only
    /// while this thread is blocked on the clock.
    pub fn enter(&self) -> ClockParticipant {
        self.register().enter()
    }

    /// A participant registration that counts as running until a thread
    /// binds it with [`PendingParticipant::enter`] and is then dropped. It is
    /// taken before a thread is spawned, so time cannot advance while the new
    /// thread is starting.
    pub(crate) fn register(&self) -> PendingParticipant {
        self.update(|state, _| {
            state.running += 1;
            state.changed();
        });
        PendingParticipant {
            clock: self.clone(),
            entered: false,
        }
    }

    /// Block the calling thread for `duration` of virtual time.
    pub fn sleep(&self, duration: Duration) {
        let deadline = self.now().checked_add(duration);
        self.wait_on(deadline, &mut Polled(|| None::<()>));
    }

    /// Block the calling thread until `poll` returns a value or virtual time
    /// reaches `deadline`, whichever is first; `None` means the deadline.
    ///
    /// `poll` is called now, whenever the clock re-checks the blocked threads
    /// (each time nothing else can run), and once more at the deadline, so a
    /// value that is ready at the deadline still wins.
    pub fn wait_until<T>(&self, deadline: Instant, poll: impl FnMut() -> Option<T>) -> Option<T> {
        self.wait_on(Some(deadline), &mut Polled(poll))
    }

    /// The clock wait every blocking primitive is built on: `source`'s value,
    /// or `None` once virtual time reaches `deadline`.
    pub(crate) fn wait_on<T>(
        &self,
        deadline: Option<Instant>,
        source: &mut impl Source<T>,
    ) -> Option<T> {
        if let Some(value) = source.poll() {
            return Some(value);
        }
        let (bell, rung) = flume::unbounded();
        let thread = thread::current().id();
        let id = self.update(|state, wakes| {
            if deadline.is_some_and(|deadline| deadline <= state.now) {
                return None;
            }
            let registrations = state.registrations(thread);
            state.running -= registrations;
            let id = state.next_id();
            state.waiters.insert(
                id,
                Waiter {
                    deadline,
                    bell,
                    registrations,
                    confirmed: None,
                    expired: false,
                },
            );
            state.changed();
            state.settle(wakes);
            Some(id)
        })?;
        let value = loop {
            match source.block(&rung) {
                Ok(value) => break Some(value),
                // A value ready at the deadline still wins.
                Err(Bell::Expired) => break source.poll(),
                Err(Bell::Verify(round)) => {
                    if let Some(value) = source.poll() {
                        break Some(value);
                    }
                    self.update(|state, wakes| state.confirm(id, round, wakes));
                }
            }
        };
        self.update(|state, wakes| {
            if let Some(waiter) = state.waiters.remove(&id) {
                if !waiter.expired {
                    state.running += waiter.registrations;
                }
            }
            state.changed();
            state.settle(wakes);
        });
        value
    }
}

#[cfg(feature = "test-utils")]
impl ManualClock {
    /// Spawn a participant thread running `f`. It is registered before the
    /// thread starts and leaves when `f` returns.
    pub fn spawn<F, T>(&self, f: F) -> thread::JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let registration = self.register();
        thread::spawn(move || {
            let _participant = registration.enter();
            f()
        })
    }
}

#[cfg(all(feature = "blocking", feature = "test-utils"))]
impl ManualClock {
    /// [`crate::blocking::Session::open`] with the owner keeping time on this
    /// clock: the worker's pacing, read slices and protocol deadlines, and
    /// every wait on the session's handles, are virtual.
    ///
    /// Enter the clock on the calling thread first, and give the session a
    /// transport whose reads also wait on this clock (see
    /// [`Self::wait_until`]); a transport that blocks in real time keeps
    /// virtual time from advancing while it reads.
    pub fn open_blocking_session<T>(
        &self,
        transport: T,
        config: crate::SessionConfig,
    ) -> crate::Result<crate::blocking::Session>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        crate::blocking::Session::open_on(
            transport,
            config.validated()?,
            crate::runtime::owner::Clock::Manual(self.clone()),
        )
    }

    /// [`crate::blocking::CameraSession::open`] on this clock, as
    /// [`Self::open_blocking_session`].
    pub fn open_blocking_camera_session<P, T>(
        &self,
        transport: T,
        config: &crate::camera::CameraConfig<P>,
    ) -> crate::Result<crate::blocking::CameraSession<P>>
    where
        P: crate::CompileTimeProfile,
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        let session = crate::blocking::Session::open_on(
            transport,
            config.validated_session_config()?,
            crate::runtime::owner::Clock::Manual(self.clone()),
        )?;
        crate::blocking::CameraSession::from_session(session, config.camera_id)
    }
}

/// A participant registration not yet bound to a thread.
#[derive(Debug)]
pub(crate) struct PendingParticipant {
    clock: ManualClock,
    /// Bound to a thread, whose [`ClockParticipant`] now owns the
    /// registration.
    entered: bool,
}

impl PendingParticipant {
    /// Bind the registration to the calling thread.
    pub(crate) fn enter(mut self) -> ClockParticipant {
        let thread = thread::current().id();
        self.entered = true;
        self.clock.update(|state, _| state.bind(thread));
        ClockParticipant {
            clock: self.clock.clone(),
            thread,
        }
    }
}

impl Drop for PendingParticipant {
    fn drop(&mut self) {
        if !self.entered {
            self.clock.update(|state, wakes| {
                state.running -= 1;
                state.changed();
                state.settle(wakes);
            });
        }
    }
}

/// A thread's registration as a [`ManualClock`] participant; dropping it
/// leaves.
#[must_use = "the thread stops participating when this guard drops"]
#[derive(Debug)]
pub struct ClockParticipant {
    clock: ManualClock,
    thread: ThreadId,
}

impl Drop for ClockParticipant {
    fn drop(&mut self) {
        let thread = self.thread;
        self.clock.update(|state, wakes| {
            state.unbind(thread);
            state.running -= 1;
            state.changed();
            state.settle(wakes);
        });
    }
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl ManualClock {
    /// A future that completes once `duration` of virtual time has passed.
    fn sleep_async(&self, duration: Duration) -> Sleep {
        Sleep {
            clock: self.clone(),
            deadline: self.now().checked_add(duration),
            key: None,
        }
    }

    /// Wrap a task's root future so the clock tracks when it is runnable.
    fn track<F: Future>(&self, future: F) -> Tracked<F> {
        let id = self.update(|state, _| {
            let id = state.next_id();
            state.tasks.insert(id, TaskState::Scheduled);
            state.runnable += 1;
            state.changed();
            id
        });
        let track = Arc::new(TaskTrack {
            clock: self.clone(),
            id,
            executor: Mutex::new(None),
        });
        Tracked {
            waker: Waker::from(Arc::clone(&track)),
            track,
            future: Box::pin(future),
            finished: false,
        }
    }
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl State {
    fn task_woken(&mut self, id: u64) {
        match self.tasks.get_mut(&id) {
            Some(state @ TaskState::Idle) => {
                *state = TaskState::Scheduled;
                self.runnable += 1;
                self.changed();
            }
            Some(state @ TaskState::Polling) => *state = TaskState::PollingWoken,
            _ => {}
        }
    }

    fn task_poll_begin(&mut self, id: u64) {
        match self.tasks.get_mut(&id) {
            Some(state @ TaskState::Idle) => {
                *state = TaskState::Polling;
                self.runnable += 1;
                self.changed();
            }
            Some(state @ TaskState::Scheduled) => *state = TaskState::Polling,
            _ => {}
        }
    }

    fn task_poll_end(&mut self, id: u64, ready: bool, wakes: &mut Wakes) {
        if ready {
            if self.tasks.remove(&id).is_some() {
                self.runnable -= 1;
            }
        } else {
            match self.tasks.get_mut(&id) {
                Some(state @ TaskState::Polling) => {
                    *state = TaskState::Idle;
                    self.runnable -= 1;
                }
                Some(state @ TaskState::PollingWoken) => *state = TaskState::Scheduled,
                _ => {}
            }
        }
        self.changed();
        self.settle(wakes);
    }

    fn task_dropped(&mut self, id: u64, wakes: &mut Wakes) {
        if let Some(state) = self.tasks.remove(&id) {
            if state != TaskState::Idle {
                self.runnable -= 1;
            }
            self.changed();
            self.settle(wakes);
        }
    }
}

/// A tracked task's waker: it marks the task runnable, then wakes it on its
/// executor.
#[cfg(all(feature = "async", feature = "test-utils"))]
#[derive(Debug)]
struct TaskTrack {
    clock: ManualClock,
    id: u64,
    executor: Mutex<Option<Waker>>,
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl Wake for TaskTrack {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.clock.update(|state, _| state.task_woken(self.id));
        let executor = self
            .executor
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(executor) = executor {
            executor.wake();
        }
    }
}

/// A task's root future, polled under its tracking waker.
#[cfg(all(feature = "async", feature = "test-utils"))]
struct Tracked<F> {
    track: Arc<TaskTrack>,
    waker: Waker,
    future: Pin<Box<F>>,
    finished: bool,
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl<F: Future> Future for Tracked<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<F::Output> {
        let this = self.get_mut();
        {
            let mut executor = this
                .track
                .executor
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !executor
                .as_ref()
                .is_some_and(|waker| waker.will_wake(context.waker()))
            {
                *executor = Some(context.waker().clone());
            }
        }
        let id = this.track.id;
        let clock = &this.track.clock;
        clock.update(|state, _| state.task_poll_begin(id));
        let poll = this
            .future
            .as_mut()
            .poll(&mut Context::from_waker(&this.waker));
        this.finished = poll.is_ready();
        clock.update(|state, wakes| state.task_poll_end(id, poll.is_ready(), wakes));
        poll
    }
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl<F> Drop for Tracked<F> {
    fn drop(&mut self) {
        if !self.finished {
            let id = self.track.id;
            self.track
                .clock
                .update(|state, wakes| state.task_dropped(id, wakes));
        }
    }
}

/// A virtual-time sleep.
#[cfg(all(feature = "async", feature = "test-utils"))]
#[derive(Debug)]
struct Sleep {
    clock: ManualClock,
    /// `None` for a sleep too long to represent: it never completes.
    deadline: Option<Instant>,
    key: Option<(Instant, u64)>,
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        let Some(deadline) = this.deadline else {
            return Poll::Pending;
        };
        let key = &mut this.key;
        this.clock.update(|state, wakes| {
            if state.now >= deadline {
                if let Some(key) = key.take() {
                    state.timers.remove(&key);
                }
                return Poll::Ready(());
            }
            let key = *key.get_or_insert_with(|| (deadline, state.next_id()));
            state.timers.insert(key, context.waker().clone());
            state.changed();
            state.settle(wakes);
            Poll::Pending
        })
    }
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            // Removing a deadline wakes nothing, so a verification round in
            // flight stays valid.
            self.clock.update(|state, _| {
                state.timers.remove(&key);
            });
        }
    }
}

/// An [`Executor`] that runs on `E` but keeps time on a [`ManualClock`].
///
/// Spawning and `block_on` delegate to `E`, so tasks run on that runtime's
/// scheduler; `now`, `sleep` and `timeout` are virtual. An async owner reads
/// time only through its executor, so a session opened with this executor
/// measures every deadline on the clock. Each spawned task and each
/// `block_on` future is a clock participant: time advances on its own once
/// all of them are pending with no wake outstanding.
///
/// Every future that must hold virtual time back has to be spawned or
/// blocked on through this executor, not through `E` directly.
#[cfg(all(feature = "async", feature = "test-utils"))]
#[derive(Debug, Clone)]
pub struct ManualClockExecutor<E> {
    inner: E,
    clock: ManualClock,
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl<E: Executor> ManualClockExecutor<E> {
    /// Run on `inner`, keeping time on `clock`.
    pub fn new(inner: E, clock: &ManualClock) -> Self {
        Self {
            inner,
            clock: clock.clone(),
        }
    }

    /// The clock this executor keeps time on.
    #[must_use]
    pub fn clock(&self) -> &ManualClock {
        &self.clock
    }
}

#[cfg(all(feature = "async", feature = "test-utils"))]
impl<E: Executor> Executor for ManualClockExecutor<E> {
    type Join<T>
        = E::Join<T>
    where
        T: Send + 'static;

    type Detach = E::Detach;

    fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.inner.spawn_with_detach(self.clock.track(fut))
    }

    fn block_on<F: Future>(&self, fut: F) -> F::Output {
        self.inner.block_on(self.clock.track(fut))
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        self.clock.sleep_async(duration)
    }

    fn timeout<'a, F, T>(
        &'a self,
        duration: Duration,
        fut: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        let expiry = self.clock.sleep_async(duration);
        futures_lite::future::or(async move { Ok(fut.await) }, async move {
            expiry.await;
            Err(Error::io_timeout())
        })
    }

    fn now(&self) -> Instant {
        self.clock.now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(3_600);

    #[test]
    fn a_lone_blocked_participant_jumps_straight_to_its_deadline() {
        let clock = ManualClock::new();
        let _driver = clock.enter();
        let start = clock.now();
        clock.sleep(HOUR);
        assert_eq!(clock.now() - start, HOUR);
    }

    #[test]
    fn explicit_advance_ends_exactly_the_waits_it_passes() {
        let clock = ManualClock::new();
        // This thread participates and never blocks on the clock, so time
        // cannot advance on its own: only `advance` moves it.
        let _driver = clock.enter();
        let start = clock.now();
        let waiter = {
            let clock = clock.clone();
            thread::spawn(move || clock.wait_until(start + HOUR, || None::<()>))
        };
        clock.advance(HOUR - Duration::from_nanos(1));
        assert!(!waiter.is_finished(), "woken before its deadline");
        clock.advance(Duration::from_nanos(1));
        assert!(matches!(waiter.join(), Ok(None)));
        assert_eq!(clock.now() - start, HOUR);
    }

    #[test]
    fn time_waits_for_a_running_participant() {
        let clock = ManualClock::new();
        let _driver = clock.enter();
        let start = clock.now();
        let (sent, received) = flume::bounded(1);
        let registration = clock.register();
        let sender = {
            let clock = clock.clone();
            thread::spawn(move || {
                let _participant = registration.enter();
                clock.sleep(Duration::from_millis(10));
                assert!(sent.send(clock.now()).is_ok());
            })
        };
        // The sender's sleep is the earliest deadline, so this wait ends with
        // its value at exactly that instant, long before its own deadline.
        let at = clock.wait_until(start + HOUR, || received.try_recv().ok());
        assert_eq!(at.map(|at| at - start), Some(Duration::from_millis(10)));
        assert_eq!(at, Some(clock.now()));
        assert!(sender.join().is_ok());
    }

    #[test]
    fn a_value_ready_at_the_deadline_wins() {
        let clock = ManualClock::new();
        let _driver = clock.enter();
        let deadline = clock.now() + HOUR;
        let ready = clock.wait_until(deadline, || (clock.now() >= deadline).then_some("ready"));
        assert_eq!(ready, Some("ready"));
    }
}
