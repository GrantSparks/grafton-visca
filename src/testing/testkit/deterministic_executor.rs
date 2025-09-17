//! Deterministic executor and virtual clock for testing.
//!
//! This module provides a deterministic executor that allows tests to control
//! time advancement explicitly, eliminating test flakiness from timing-dependent behavior.

#![cfg(feature = "test-utils")]
// Panics and expects in test utilities are intentional for detecting test failures
#![allow(clippy::panic, clippy::expect_used)]

//! Test Strategy Note:
//! Due to fundamental limitations with virtual time and timeout handling,
//! this executor should be used for:
//! - Logic and sequencing tests
//! - Tests that don't rely on actual timeout behavior
//!
//! For tests that need real timeout behavior, use real runtime executors
//! (TokioExecutor, AsyncStdExecutor, SmolExecutor) instead.

use async_executor::Executor as AsyncExec;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use crate::{executor::Executor, Error};

#[cfg(feature = "runtime-tokio")]
use crate::executor::TokioExecutor;

/// Extension trait for executor background task semantics in test utilities.
///
/// This trait provides true "fire-and-forget" task spawning capabilities
/// that are needed for scripted transport background operations.
#[cfg(any(test, feature = "test-utils"))]
pub trait ExecutorExt {
    /// Spawn a background task that continues running without needing to await a join handle.
    ///
    /// Unlike the regular `spawn` method which returns a join handle that must be polled,
    /// this method spawns tasks that run independently in the background.
    fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static;

    /// Spawn a background task that ignores results.
    ///
    /// This is a convenience method for spawning tasks that return Result<(), Error>
    /// but where we want to ignore any errors in the background.
    fn spawn_detached_ignore_result<F, T>(&self, fut: F)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        ExecutorExt::spawn_detached(self, async move {
            let _ = fut.await;
        });
    }

    /// Spawn a background task that logs errors from Result types.
    ///
    /// This variant logs errors to stderr for debugging purposes when tasks
    /// return Result<(), E> types, making failures visible during testing.
    fn spawn_detached_ignore_result_with_logging<F, E>(&self, fut: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Debug + Send + 'static,
    {
        ExecutorExt::spawn_detached(self, async move {
            if let Err(e) = fut.await {
                eprintln!("[det-runtime] background task returned error: {e:?}");
            }
        });
    }
}

/// A virtual clock that can be advanced deterministically.
///
/// The virtual clock maintains a current time and allows tests to advance
/// time explicitly, waking up any futures that were waiting for that time.
#[derive(Debug, Clone)]
pub struct VirtualClock {
    inner: Arc<Mutex<VirtualClockInner>>,
}

#[derive(Debug)]
struct VirtualClockInner {
    now: Instant,
    sleepers: Vec<SleepEntry>,
    next_id: u64,
}

#[derive(Debug)]
struct SleepEntry {
    id: u64,
    at: Instant,
    waker: Option<Waker>,
}

impl VirtualClock {
    fn new(start_time: Instant) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VirtualClockInner {
                now: start_time,
                sleepers: Vec::new(),
                next_id: 0,
            })),
        }
    }

    fn now(&self) -> Instant {
        self.inner.lock().expect("VirtualClock mutex poisoned").now
    }

    fn advance(&self, duration: Duration) {
        let mut inner = self.inner.lock().expect("VirtualClock mutex poisoned");
        inner.now += duration;

        // Wake up and remove triggered sleepers
        let mut i = 0;
        while i < inner.sleepers.len() {
            if inner.sleepers[i].at <= inner.now {
                if let Some(waker) = inner.sleepers[i].waker.take() {
                    waker.wake();
                }
                inner.sleepers.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn sleep(&self, duration: Duration) -> SleepFuture {
        let deadline = self.now() + duration;
        SleepFuture {
            clock: Arc::clone(&self.inner),
            deadline,
            id: None,
            registered: false,
        }
    }
}

/// A future that completes when the virtual clock reaches a specific time.
struct SleepFuture {
    clock: Arc<Mutex<VirtualClockInner>>,
    deadline: Instant,
    id: Option<u64>,
    registered: bool,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();

        // Fast path: already expired => remove our entry (if registered) and complete
        {
            let mut inner = me.clock.lock().expect("VirtualClock mutex poisoned");
            if inner.now >= me.deadline {
                if let Some(id) = me.id.take() {
                    if let Some(pos) = inner.sleepers.iter().position(|s| s.id == id) {
                        inner.sleepers.remove(pos);
                    }
                }
                return Poll::Ready(());
            }
        }

        let mut inner = me.clock.lock().expect("VirtualClock mutex poisoned");
        if !me.registered {
            me.registered = true;
            let id = inner.next_id;
            inner.next_id += 1;
            inner.sleepers.push(SleepEntry {
                id,
                at: me.deadline,
                waker: Some(cx.waker().clone()),
            });
            me.id = Some(id);
        } else if let Some(id) = me.id {
            if let Some(entry) = inner.sleepers.iter_mut().find(|s| s.id == id) {
                entry.waker = Some(cx.waker().clone());
            } else {
                // Our entry disappeared (e.g., fired); re-register to be safe.
                let id = inner.next_id;
                inner.next_id += 1;
                inner.sleepers.push(SleepEntry {
                    id,
                    at: me.deadline,
                    waker: Some(cx.waker().clone()),
                });
                me.id = Some(id);
            }
        }

        Poll::Pending
    }
}

// Cancel timer on drop so sleepers don't leak when futures lose races
impl Drop for SleepFuture {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            if let Ok(mut inner) = self.clock.lock() {
                if let Some(pos) = inner.sleepers.iter().position(|s| s.id == id) {
                    inner.sleepers.remove(pos);
                }
            }
        }
    }
}

/// A deterministic executor that uses virtual time for testing.
///
/// This executor provides deterministic timing behavior by using a virtual clock
/// that can be advanced explicitly in tests, rather than relying on wall-clock time.
#[derive(Debug)]
pub struct DeterministicExecutor {
    executor: AsyncExec<'static>,
    clock: VirtualClock,
}

impl DeterministicExecutor {
    /// Create a new deterministic executor starting at the specified time.
    ///
    /// Returns both the executor and a clock handle that can be used to advance time.
    pub fn new_start_at(start_time: Instant) -> (Arc<Self>, DeterministicClock) {
        let clock = VirtualClock::new(start_time);
        let executor = Self {
            executor: AsyncExec::new(),
            clock: VirtualClock {
                inner: Arc::clone(&clock.inner),
            },
        };
        let clock_handle = DeterministicClock { clock };
        (Arc::new(executor), clock_handle)
    }

    /// Create a new deterministic executor starting at the current time.
    pub fn new() -> (Arc<Self>, DeterministicClock) {
        Self::new_start_at(Instant::now())
    }

    /// Run a future while continuously driving all background tasks on this executor.
    ///
    /// This method drives both the provided future and any background tasks spawned
    /// via `spawn_bg()`, making it suitable for testing scenarios where the runtime
    /// spawns background loops.
    ///
    /// Virtual time is advanced automatically when no tasks are ready to run.
    /// This version prevents livelock by budgeting task execution and advancing time
    /// even when the run queue remains busy.
    pub fn block_on_bg<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = flume::bounded::<T>(1);

        // Run the user future inside the executor and send its output back
        self.executor
            .spawn(async move {
                let out = fut.await;
                let _ = tx.send(out);
            })
            .detach();

        // ---- Tunables (keep constants to preserve determinism) ----
        const READY_BUDGET_PER_EPOCH: usize = 512; // how many ready tasks to run before considering time
        const BUSY_EPOCHS_BEFORE_TIME_BUMP: usize = 4; // tolerate a few busy epochs before advancing time
        const NO_PROGRESS_PANIC: usize = 50_000; // hard guardrail against infinite spin

        // ---- State ----
        let mut busy_epochs = 0usize;
        let mut spins = 0usize;

        loop {
            // Trace iteration start if RUNTIME_TRACE is set
            if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                eprintln!(
                    "[DeterministicExecutor] Loop iteration - time: {:?}, has_deadlines: {}",
                    self.now(),
                    self.has_pending_deadlines()
                );
            }

            // Drain up to READY_BUDGET_PER_EPOCH ready tasks.
            let mut ran = 0usize;
            for _ in 0..READY_BUDGET_PER_EPOCH {
                // try_tick() should return true if it ran at least one task this call.
                if self.executor.try_tick() {
                    ran += 1;
                } else {
                    break;
                }
                if let Ok(out) = rx.try_recv() {
                    // User future completed - drain remaining tasks before returning
                    if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                        eprintln!("[DeterministicExecutor] User future completed, draining background tasks");
                    }
                    self.drain_until_quiescent();
                    return out;
                }
            }

            // Completion check (in case fut finished while we were ticking)
            if let Ok(out) = rx.try_recv() {
                // User future completed - drain remaining tasks before returning
                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[DeterministicExecutor] User future completed, draining background tasks"
                    );
                }
                self.drain_until_quiescent();
                return out;
            }

            if ran == 0 {
                // No ready work -> advance time deterministically.
                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[DeterministicExecutor] No ready tasks, checking for time advancement"
                    );
                }
                busy_epochs = 0;
                if self.fire_due_timers() || self.advance_to_next_deadline() {
                    if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                        eprintln!("[DeterministicExecutor] Advanced time to {:?}", self.now());
                    }
                    spins = 0;
                    continue;
                }
            } else {
                // There is always ready work; don't starve timers forever.
                if self.has_pending_deadlines() {
                    busy_epochs += 1;
                    if busy_epochs >= BUSY_EPOCHS_BEFORE_TIME_BUMP {
                        // Deterministically jump to the next deadline even under load.
                        if self.advance_to_next_deadline() {
                            busy_epochs = 0;
                            spins = 0;
                            continue;
                        }
                    }
                } else {
                    busy_epochs = 0;
                }
            }

            // Progress guard (debug‑only panic is fine under test-utils)
            spins += 1;
            if spins >= NO_PROGRESS_PANIC {
                panic!(
                    "DeterministicExecutor::block_on_bg: no forward progress after {NO_PROGRESS_PANIC} iterations. \
                     Possible causes: perpetual busy loop without yields; real timers leaking into tests; \
                     or background loop exited early."
                );
            }

            // Tick once more to ensure any woken tasks get a chance to run
            // This is crucial for handling the case where we just advanced time
            // and woke up a timer, but haven't run the woken task yet
            if self.executor.try_tick() {
                spins = 0; // Reset progress counter if we made progress
                if let Ok(out) = rx.try_recv() {
                    // User future completed - drain remaining tasks before returning
                    if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                        eprintln!("[DeterministicExecutor] User future completed, draining background tasks");
                    }
                    self.drain_until_quiescent();
                    return out;
                }
            }

            // Be polite to the host thread without affecting determinism of task order.
            std::thread::yield_now();
        }
    }

    /// Fire any timers that are due and return true if any were fired.
    fn fire_due_timers(&self) -> bool {
        let mut inner = self
            .clock
            .inner
            .lock()
            .expect("VirtualClock mutex poisoned");
        let now = inner.now;
        let mut fired = false;

        let mut i = 0;
        while i < inner.sleepers.len() {
            if inner.sleepers[i].at <= now {
                if let Some(waker) = inner.sleepers[i].waker.take() {
                    waker.wake();
                    fired = true;
                }
                inner.sleepers.remove(i);
            } else {
                i += 1;
            }
        }

        fired
    }

    /// Check if there are any pending timers/deadlines that might benefit from time advancement.
    fn has_pending_deadlines(&self) -> bool {
        let inner = self
            .clock
            .inner
            .lock()
            .expect("VirtualClock mutex poisoned");
        !inner.sleepers.is_empty()
    }

    /// Get the current virtual time.
    pub fn now(&self) -> Instant {
        self.clock.now()
    }

    /// Run the executor until all currently ready tasks are complete.
    ///
    /// Returns true if any work was executed, false if already idle.
    /// This method does NOT advance time - it only runs tasks that are already ready.
    pub fn run_until_idle(&self) -> bool {
        let mut made_any_progress = false;
        const MAX_ITERATIONS: usize = 10_000;
        let mut iterations = 0;

        loop {
            // Run any ready tasks
            let made_progress = self.executor.try_tick();

            // Fire any due timers (at current time, without advancing)
            let timers_fired = self.fire_due_timers();

            // Track if we made any progress
            if made_progress || timers_fired {
                made_any_progress = true;
            }

            // If no progress was made this iteration, we're idle
            if !made_progress && !timers_fired {
                break;
            }

            iterations += 1;
            if iterations >= MAX_ITERATIONS {
                panic!("run_until_idle: exceeded maximum iterations ({MAX_ITERATIONS}), possible infinite loop");
            }
        }

        made_any_progress
    }

    /// Advance time to the next deadline and wake any ready timers.
    ///
    /// Returns true if time was advanced, false if there were no pending deadlines.
    pub fn advance_to_next_deadline(&self) -> bool {
        let mut inner = self
            .clock
            .inner
            .lock()
            .expect("VirtualClock mutex poisoned");

        if let Some(next_deadline) = inner.sleepers.iter().map(|s| s.at).min() {
            if next_deadline > inner.now {
                inner.now = next_deadline;

                // Wake up any sleepers that are now due
                let mut i = 0;
                while i < inner.sleepers.len() {
                    if inner.sleepers[i].at <= inner.now {
                        if let Some(waker) = inner.sleepers[i].waker.take() {
                            waker.wake();
                        }
                        inner.sleepers.remove(i);
                    } else {
                        i += 1;
                    }
                }

                return true;
            }
        }

        false
    }

    /// Drive the executor until all currently ready tasks are complete.
    ///
    /// This method does NOT advance time - it only runs tasks that are already ready.
    /// Use `drive_until_stalled` if you want to also advance time to pending deadlines.
    pub fn drive_until_idle(&self) {
        const MAX_ITERATIONS: usize = 10_000;
        let mut iterations = 0;

        loop {
            // Run any ready tasks
            let made_progress = self.executor.try_tick();

            // Fire any due timers (at current time, without advancing)
            let timers_fired = self.fire_due_timers();

            // If no progress was made, we're idle
            if !made_progress && !timers_fired {
                break;
            }

            iterations += 1;
            if iterations >= MAX_ITERATIONS {
                panic!("drive_until_idle: exceeded maximum iterations ({MAX_ITERATIONS}), possible infinite loop");
            }
        }
    }

    /// Drive the executor until all tasks are complete, including advancing time.
    ///
    /// This method will advance time to pending deadlines as needed to complete all work.
    pub fn drive_until_stalled(&self) {
        const MAX_ITERATIONS: usize = 10_000;
        let mut iterations = 0;

        loop {
            // Run any ready tasks
            let made_progress = self.executor.try_tick();

            // Fire any due timers
            let timers_fired = self.fire_due_timers();

            // Advance to next deadline if there are pending timers
            let time_advanced = if self.has_pending_deadlines() {
                self.advance_to_next_deadline()
            } else {
                false
            };

            // If no progress was made, we're stalled
            if !made_progress && !timers_fired && !time_advanced {
                break;
            }

            iterations += 1;
            if iterations >= MAX_ITERATIONS {
                panic!("drive_until_stalled: exceeded maximum iterations ({MAX_ITERATIONS}), possible infinite loop");
            }
        }
    }
}

impl DeterministicExecutor {
    /// Drain all pending tasks and timers until the executor is quiescent.
    ///
    /// This method is called after the user future completes to ensure all background
    /// tasks are given a chance to observe shutdown and complete gracefully.
    pub fn drain_until_quiescent(&self) {
        const MAX_SPINS: usize = 1024;
        let mut spins = 0;

        if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
            eprintln!("[DeterministicExecutor] Starting drain_until_quiescent");
        }

        loop {
            // 1) Drive any ready tasks
            let has_tasks = self.executor.try_tick();

            // 2) Drain timers that are already due (<= now)
            let fired = self.fire_due_timers();

            // 3) See if anything remains
            let has_deadlines = self.has_pending_deadlines();

            if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                eprintln!(
                    "[DeterministicExecutor] Drain iteration: has_tasks={}, fired={}, has_deadlines={}, spins={}",
                    has_tasks, fired, has_deadlines, spins
                );
            }

            // If no tasks, no fired timers, and no deadlines, we're done
            if !has_tasks && !fired && !has_deadlines {
                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!("[DeterministicExecutor] Quiescent - no tasks or deadlines");
                }
                break;
            }

            // 4) If there are future deadlines, jump to the next one
            if has_deadlines && self.advance_to_next_deadline() {
                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[DeterministicExecutor] Advanced to next deadline: {:?}",
                        self.now()
                    );
                }
                spins = 0;
                continue;
            }

            if has_tasks || fired {
                spins = 0;
                continue;
            }

            // Guard against infinite spin
            spins += 1;
            if spins > MAX_SPINS {
                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!("[DeterministicExecutor] Breaking after {} spins", MAX_SPINS);
                }
                break;
            }

            std::thread::yield_now();
        }

        if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
            eprintln!("[DeterministicExecutor] drain_until_quiescent completed");
        }
    }
}

impl Clone for DeterministicExecutor {
    fn clone(&self) -> Self {
        Self {
            executor: AsyncExec::new(),
            clock: VirtualClock {
                inner: Arc::clone(&self.clock.inner),
            },
        }
    }
}

/// Wrapper for deterministic executor join handles.
#[derive(Debug)]
pub struct DetJoin<T>(async_executor::Task<T>);

impl<T> Future for DetJoin<T>
where
    T: Send + 'static,
{
    type Output = Result<T, crate::executor::ExecError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(v) => Poll::Ready(Ok(v)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Wrapper for local tasks (async_executor treats all tasks the same)
#[derive(Debug)]
pub struct DetLocalJoin<T>(async_executor::Task<T>);

impl<T> Future for DetLocalJoin<T>
where
    T: 'static,
{
    type Output = Result<T, crate::executor::ExecError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(v) => Poll::Ready(Ok(v)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Handle for controlling the virtual clock in tests.
///
/// This allows tests to advance time deterministically and observe the effects
/// on futures that are waiting for timeouts or delays.
#[derive(Debug)]
pub struct DeterministicClock {
    clock: VirtualClock,
}

impl Clone for DeterministicClock {
    fn clone(&self) -> Self {
        Self {
            clock: VirtualClock {
                inner: Arc::clone(&self.clock.inner),
            },
        }
    }
}

impl DeterministicClock {
    /// Advance the virtual clock by the specified duration.
    ///
    /// This will wake up any futures that were sleeping and whose deadline
    /// has now been reached.
    pub fn advance(&self, duration: Duration) {
        self.clock.advance(duration);
    }

    /// Get the current virtual time.
    pub fn now(&self) -> Instant {
        self.clock.now()
    }

    /// Check if there are any pending deadlines/timers.
    pub fn has_pending_deadlines(&self) -> bool {
        let inner = self
            .clock
            .inner
            .lock()
            .expect("VirtualClock mutex poisoned");
        !inner.sleepers.is_empty()
    }
}

impl Executor for DeterministicExecutor {
    type Join<T>
        = DetJoin<T>
    where
        T: Send + 'static;

    type LocalJoin<T>
        = DetLocalJoin<T>
    where
        T: 'static;

    fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        DetJoin(self.executor.spawn(fut))
    }

    fn spawn_local<F>(&self, fut: F) -> Self::LocalJoin<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // DeterministicExecutor runs tasks on a single-threaded executor in tests
        DetLocalJoin(self.executor.spawn(fut))
    }

    fn block_on<F: Future>(&self, fut: F) -> F::Output {
        // For DeterministicExecutor, we use the simpler approach of just running
        // the future on the executor. This works well for tests that don't rely
        // on timeout behavior. For tests that need actual timeout behavior,
        // use real runtime executors instead (see issue #394).
        use futures_lite::future;
        future::block_on(self.executor.run(fut))
    }

    #[allow(clippy::manual_async_fn)]
    #[allow(refining_impl_trait)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + 'static {
        let clock = self.clock.clone();
        async move { clock.sleep(duration).await }
    }

    #[allow(clippy::manual_async_fn)]
    fn timeout<'a, F, T>(
        &'a self,
        duration: Duration,
        fut: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        // Clone the clock outside the async block to avoid capturing &self
        let clock = self.clock.clone();
        async move {
            let timeout_future = TimeoutFuture {
                future: Box::pin(fut),
                sleep: clock.sleep(duration),
            };
            timeout_future.await
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn timeout_owned<T>(
        &self,
        duration: Duration,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'static
    where
        T: Send + 'static,
    {
        let clock = self.clock.clone();
        async move {
            let timeout_future = TimeoutFuture {
                future: Box::pin(fut),
                sleep: clock.sleep(duration),
            };
            timeout_future.await
        }
    }

    fn now(&self) -> Instant {
        self.clock.now()
    }

    // Override spawn_bg to properly detach the task.
    // The default implementation drops the join handle returned by spawn(),
    // but for DeterministicExecutor, that wrapper owns the underlying Task
    // and dropping it cancels the task. We must detach it instead.
    fn spawn_bg<F>(&self, fut: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.spawn(fut).detach();
    }
}

/// A timeout future that properly integrates with the virtual clock.
///
/// This custom implementation ensures that when the virtual clock advances,
/// the timeout future is properly woken up, avoiding issues with third-party
/// combinator functions that might not propagate wakeups correctly.
struct TimeoutFuture<F> {
    future: Pin<Box<F>>,
    sleep: SleepFuture,
}

impl<F, T> Future for TimeoutFuture<F>
where
    F: Future<Output = T>,
{
    type Output = Result<T, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Poll the main future first
        if let Poll::Ready(value) = self.future.as_mut().poll(cx) {
            return Poll::Ready(Ok(value));
        }

        // Poll the sleep future to check for timeout
        if let Poll::Ready(()) = Pin::new(&mut self.sleep).poll(cx) {
            return Poll::Ready(Err(Error::Timeout));
        }

        // Neither future is ready, so we're still pending
        Poll::Pending
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl ExecutorExt for DeterministicExecutor {
    fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // async-executor Task must be detached to survive dropped handles
        self.executor.spawn(fut).detach();
    }

    fn spawn_detached_ignore_result_with_logging<F, E>(&self, fut: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Debug + Send + 'static,
    {
        ExecutorExt::spawn_detached(self, async move {
            if let Err(e) = fut.await {
                eprintln!("[det-runtime] background task returned error: {e:?}");
            }
        });
    }
}

// Implement ExecutorExt for TokioExecutor
#[cfg(all(feature = "runtime-tokio", any(test, feature = "test-utils")))]
impl ExecutorExt for TokioExecutor {
    fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // tokio::spawn returns a JoinHandle, but dropping it makes it fire-and-forget
        drop(self.spawn(fut));
    }

    fn spawn_detached_ignore_result_with_logging<F, E>(&self, fut: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Debug + Send + 'static,
    {
        ExecutorExt::spawn_detached(self, async move {
            if let Err(e) = fut.await {
                eprintln!("[det-runtime] background task returned error: {e:?}");
            }
        });
    }
}

// Generic implementation for Arc<T> where T implements ExecutorExt
#[cfg(any(test, feature = "test-utils"))]
impl<T> ExecutorExt for Arc<T>
where
    T: ExecutorExt + ?Sized,
{
    fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        ExecutorExt::spawn_detached(self.as_ref(), fut);
    }

    fn spawn_detached_ignore_result_with_logging<F, E>(&self, fut: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Debug + Send + 'static,
    {
        (**self).spawn_detached_ignore_result_with_logging(fut);
    }
}

/// Convenience extension trait for `Arc<DeterministicExecutor>`
pub trait DeterministicExecutorExt {
    /// Execute a future in the background and block until it completes
    fn block_on_bg<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;

    /// Drive the executor until all tasks are idle.
    fn drive_until_idle(&self);

    /// Drive the executor until all tasks are complete, including advancing time.
    fn drive_until_stalled(&self);
}

impl DeterministicExecutorExt for Arc<DeterministicExecutor> {
    /// Run a future while continuously driving all background tasks on this executor.
    ///
    /// This is a convenience method that delegates to the inner executor's `block_on_bg`.
    fn block_on_bg<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.as_ref().block_on_bg(fut)
    }

    /// Drive the executor until all tasks are idle.
    ///
    /// This is a convenience method that delegates to the inner executor's `drive_until_idle`.
    fn drive_until_idle(&self) {
        self.as_ref().drive_until_idle()
    }

    /// Drive the executor until all tasks are complete, including advancing time.
    ///
    /// This is a convenience method that delegates to the inner executor's `drive_until_stalled`.
    fn drive_until_stalled(&self) {
        self.as_ref().drive_until_stalled()
    }
}

#[cfg(all(test, feature = "test-utils"))]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_virtual_clock_advance() {
        let start = Instant::now();
        let clock = VirtualClock::new(start);

        assert_eq!(clock.now(), start);

        clock.advance(Duration::from_secs(1));
        assert_eq!(clock.now(), start + Duration::from_secs(1));
    }

    #[test]
    fn test_deterministic_executor_creation() {
        let start = Instant::now();
        let (_executor, clock) = DeterministicExecutor::new_start_at(start);

        assert_eq!(clock.now(), start);

        // Verify we can advance time
        clock.advance(Duration::from_millis(100));
        assert_eq!(clock.now(), start + Duration::from_millis(100));
    }

    #[test]
    fn test_det_drives_spawned_tasks() {
        let (executor, _clock) = DeterministicExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));
        let flag2 = flag.clone();

        // Use ExecutorExt::spawn_bg to properly spawn background task
        // Use fully-qualified path for ExecutorExt to avoid ambiguity

        ExecutorExt::spawn_detached(&executor, async move {
            flag2.store(true, Ordering::SeqCst);
        });

        // Drive executor until idle
        executor.drive_until_idle();
        assert!(flag.load(Ordering::SeqCst), "Spawned task should have run");
    }

    #[test]
    fn test_det_sleep_fires_only_when_time_advances() {
        let (executor, clock) = DeterministicExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));
        let flag2 = flag.clone();
        let executor2 = executor.clone();

        // Use ExecutorExt::spawn_bg to properly spawn background task

        ExecutorExt::spawn_detached(&executor, async move {
            executor2.sleep(Duration::from_millis(50)).await;
            flag2.store(true, Ordering::SeqCst);
        });

        // Nothing should happen yet
        executor.drive_until_idle();
        assert!(
            !flag.load(Ordering::SeqCst),
            "Sleep should not complete without time advancement"
        );

        // Advance time and drive again
        clock.advance(Duration::from_millis(50));
        executor.drive_until_idle();
        assert!(
            flag.load(Ordering::SeqCst),
            "Sleep should complete after time advancement"
        );
    }

    #[test]
    fn test_multiple_sleeps_wake_in_order() {
        let (executor, clock) = DeterministicExecutor::new();
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn tasks with different sleep durations
        for i in 1..=3 {
            let counter_clone = counter.clone();
            let executor_clone = executor.clone();
            let expected_value = i;

            ExecutorExt::spawn_detached(&executor, async move {
                executor_clone
                    .sleep(Duration::from_millis(i as u64 * 10))
                    .await;
                counter_clone.store(expected_value, Ordering::SeqCst);
            });
        }

        // Drive initially - nothing should happen
        executor.drive_until_idle();
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // Advance to 10ms - first task should complete
        clock.advance(Duration::from_millis(10));
        executor.drive_until_idle();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Advance to 20ms total - second task should complete
        clock.advance(Duration::from_millis(10));
        executor.drive_until_idle();
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        // Advance to 30ms total - third task should complete
        clock.advance(Duration::from_millis(10));
        executor.drive_until_idle();
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_block_on_bg_completes() {
        let (executor, _clock) = DeterministicExecutor::new();
        let executor_clone = executor.clone();

        // This should complete because block_on_bg drives the executor
        let result = executor.block_on_bg(async move {
            executor_clone.sleep(Duration::from_millis(100)).await;
            42
        });

        assert_eq!(
            result, 42,
            "block_on_bg should complete and return the value"
        );
    }

    #[test]
    fn test_run_until_idle() {
        let (executor, _clock) = DeterministicExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));
        let flag2 = flag.clone();

        // Spawn a task

        ExecutorExt::spawn_detached(&executor, async move {
            flag2.store(true, Ordering::SeqCst);
        });

        // Run until idle - should run the task
        let progressed = executor.run_until_idle();
        assert!(progressed, "Should have made progress");
        assert!(flag.load(Ordering::SeqCst), "Task should have run");

        // Run again - should not progress (already idle)
        let progressed = executor.run_until_idle();
        assert!(!progressed, "Should not have made progress when idle");
    }

    #[test]
    fn test_advance_to_next_deadline() {
        let (executor, clock) = DeterministicExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));
        let flag2 = flag.clone();
        let executor2 = executor.clone();

        let initial_time = clock.now();

        // Spawn a task with a sleep

        ExecutorExt::spawn_detached(&executor, async move {
            executor2.sleep(Duration::from_millis(100)).await;
            flag2.store(true, Ordering::SeqCst);
        });

        // Run until idle - task should be waiting on sleep
        executor.run_until_idle();
        assert!(!flag.load(Ordering::SeqCst), "Task should still be waiting");

        // Advance to next deadline
        let advanced = executor.advance_to_next_deadline();
        assert!(advanced, "Should have advanced time");

        // Verify that time actually advanced by 100ms
        assert_eq!(
            clock.now(),
            initial_time + Duration::from_millis(100),
            "Time should have advanced exactly to the sleep deadline"
        );

        // Run until idle again - task should complete
        executor.run_until_idle();
        assert!(flag.load(Ordering::SeqCst), "Task should have completed");
    }

    #[test]
    fn test_drive_until_stalled() {
        let (executor, _clock) = DeterministicExecutor::new();
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn multiple tasks with different sleep durations
        for i in 1..=3 {
            let counter_clone = counter.clone();
            let executor_clone = executor.clone();

            ExecutorExt::spawn_detached(&executor, async move {
                executor_clone.sleep(Duration::from_millis(i * 50)).await;
                counter_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        // Drive until stalled - should complete all tasks
        executor.drive_until_stalled();
        assert_eq!(
            counter.load(Ordering::SeqCst),
            3,
            "All tasks should have completed"
        );
    }

    #[test]
    fn test_spawn_bg_runs() {
        // Test that spawn_bg properly detaches the task and it runs
        let (executor, _clock) = DeterministicExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        // Use spawn_bg directly from the Executor trait
        executor.spawn_bg(async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Drive the executor to run the background task
        executor.drive_until_idle();

        // The task should have run
        assert!(flag.load(Ordering::SeqCst), "spawn_bg task should have run");
    }

    #[test]
    fn test_spawn_bg_with_arc() {
        // Test that Arc<DeterministicExecutor> also correctly implements spawn_bg
        let (executor, _clock) = DeterministicExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        // Use spawn_bg on Arc<DeterministicExecutor>
        <Arc<DeterministicExecutor> as Executor>::spawn_bg(&executor, async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Drive the executor to run the background task
        executor.drive_until_idle();

        // The task should have run
        assert!(
            flag.load(Ordering::SeqCst),
            "spawn_bg task on Arc should have run"
        );
    }

    #[test]
    fn test_sleep_cleanup_in_race() {
        use futures_lite::future;

        let (executor, clock) = DeterministicExecutor::new();

        // Test that when a sleep loses a race, it's cleaned up
        let exec = executor.clone();
        executor.block_on_bg(async move {
            // Create a race between a sleep and an immediate value
            let winner = future::race(
                async {
                    exec.sleep(Duration::from_millis(100)).await;
                    "sleep"
                },
                async { "immediate" },
            )
            .await;

            assert_eq!(winner, "immediate", "Immediate value should win");
        });

        // After the future completes, there should be no pending deadlines
        // because the losing sleep should have been dropped and cleaned up
        assert!(
            !clock.has_pending_deadlines(),
            "Sleep future should have been cleaned up when it lost the race"
        );
    }

    #[test]
    fn test_multiple_sleep_cleanup() {
        use futures_lite::future;

        let (executor, clock) = DeterministicExecutor::new();

        let exec = executor.clone();
        executor.block_on_bg(async move {
            // Start multiple sleeps in a race
            let _winner = future::race(
                future::race(
                    exec.sleep(Duration::from_millis(50)),
                    exec.sleep(Duration::from_millis(100)),
                ),
                async {
                    // Immediate completion
                },
            )
            .await;
        });

        // All sleeps should be cleaned up
        assert!(
            !clock.has_pending_deadlines(),
            "All sleep futures should have been cleaned up"
        );
    }
}

// Add ExecutorExt implementations for AsyncStdExecutor and SmolExecutor
#[cfg(all(feature = "mode-async", feature = "runtime-async-std"))]
impl ExecutorExt for crate::executor::AsyncStdExecutor {
    fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // async-std::task::spawn returns a JoinHandle, but dropping it makes it fire-and-forget
        drop(self.spawn(fut));
    }

    fn spawn_detached_ignore_result_with_logging<F, E>(&self, fut: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Debug + Send + 'static,
    {
        ExecutorExt::spawn_detached(self, async move {
            if let Err(e) = fut.await {
                eprintln!("[async-std-runtime] background task returned error: {e:?}");
            }
        });
    }
}

#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
impl ExecutorExt for crate::executor::SmolExecutor {
    fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // smol::spawn returns a Task, but dropping it makes it fire-and-forget
        drop(self.spawn(fut));
    }

    fn spawn_detached_ignore_result_with_logging<F, E>(&self, fut: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Debug + Send + 'static,
    {
        ExecutorExt::spawn_detached(self, async move {
            if let Err(e) = fut.await {
                eprintln!("[smol-runtime] background task returned error: {e:?}");
            }
        });
    }
}
