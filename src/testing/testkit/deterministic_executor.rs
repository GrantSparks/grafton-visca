//! Deterministic executor and virtual clock for testing.
//!
//! This module provides a deterministic executor that allows tests to control
//! time advancement explicitly, eliminating test flakiness from timing-dependent behavior.

#![cfg(feature = "test-utils")]
// Panics and expects in test utilities are intentional for detecting test failures
#![allow(clippy::panic, clippy::expect_used)]

#[cfg(feature = "rt-tokio")]
use crate::executor::TokioExecutor;

use async_executor::Executor as AsyncExec;
use futures_lite::future;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use crate::{executor::Executor, Error};

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
}

#[derive(Debug)]
struct SleepEntry {
    at: Instant,
    waker: Option<Waker>,
}

impl VirtualClock {
    fn new(start_time: Instant) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VirtualClockInner {
                now: start_time,
                sleepers: Vec::new(),
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
            registered: false,
        }
    }
}

/// A future that completes when the virtual clock reaches a specific time.
struct SleepFuture {
    clock: Arc<Mutex<VirtualClockInner>>,
    deadline: Instant,
    registered: bool,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();

        // Check if we've reached the deadline
        {
            let inner = me.clock.lock().expect("VirtualClock mutex poisoned");
            let now = inner.now;

            if now >= me.deadline {
                return Poll::Ready(());
            }
        }

        // Update or register the waker
        let mut inner = me.clock.lock().expect("VirtualClock mutex poisoned");
        if !me.registered {
            me.registered = true;
            inner.sleepers.push(SleepEntry {
                at: me.deadline,
                waker: Some(cx.waker().clone()),
            });
        } else {
            // Update the waker for this deadline if it already exists
            for entry in &mut inner.sleepers {
                if entry.at == me.deadline {
                    entry.waker = Some(cx.waker().clone());
                    break;
                }
            }
        }

        Poll::Pending
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
                    return out;
                }
            }

            // Completion check (in case fut finished while we were ticking)
            if let Ok(out) = rx.try_recv() {
                return out;
            }

            if ran == 0 {
                // No ready work -> advance time deterministically.
                busy_epochs = 0;
                if self.fire_due_timers() || self.advance_to_next_deadline() {
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
        = Pin<Box<dyn Future<Output = Result<T, crate::executor::ExecError>> + Send + 'static>>
    where
        T: Send + 'static;

    type LocalJoin<T>
        = Pin<Box<dyn Future<Output = Result<T, crate::executor::ExecError>> + 'static>>
    where
        T: 'static;

    fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let task = self.executor.spawn(fut);
        Box::pin(async move { Ok(task.await) })
    }

    fn spawn_local<F>(&self, fut: F) -> Self::LocalJoin<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // DeterministicExecutor runs tasks on a single-threaded executor in tests
        let task = self.executor.spawn(fut);
        Box::pin(async move { Ok(task.await) })
    }

    fn block_on<F: Future>(&self, fut: F) -> F::Output {
        future::block_on(self.executor.run(fut))
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        async move { self.clock.sleep(duration).await }
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
        async move {
            let timeout_future = TimeoutFuture {
                future: Box::pin(fut),
                sleep: self.clock.sleep(duration),
            };
            timeout_future.await
        }
    }

    fn timeout_owned<T>(
        &self,
        duration: Duration,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'static>>
    where
        T: Send + 'static,
    {
        let clock = self.clock.clone();
        Box::pin(async move {
            let timeout_future = TimeoutFuture {
                future: Box::pin(fut),
                sleep: clock.sleep(duration),
            };
            timeout_future.await
        })
    }

    fn now(&self) -> Instant {
        self.now()
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
#[cfg(all(feature = "rt-tokio", any(test, feature = "test-utils")))]
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

// Implement Executor for Arc<DeterministicExecutor> to match the pattern used by other executors
impl Executor for Arc<DeterministicExecutor> {
    type Join<T>
        = Pin<Box<dyn Future<Output = Result<T, crate::executor::ExecError>> + Send + 'static>>
    where
        T: Send + 'static;

    type LocalJoin<T>
        = Pin<Box<dyn Future<Output = Result<T, crate::executor::ExecError>> + 'static>>
    where
        T: 'static;

    fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.as_ref().spawn(fut)
    }

    fn spawn_local<F>(&self, fut: F) -> Self::LocalJoin<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.as_ref().spawn_local(fut)
    }

    fn block_on<F: Future>(&self, fut: F) -> F::Output {
        self.as_ref().block_on(fut)
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        async move { self.as_ref().sleep(duration).await }
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
        async move { self.as_ref().timeout(duration, fut).await }
    }

    fn timeout_owned<T>(
        &self,
        duration: Duration,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'static>>
    where
        T: Send + 'static,
    {
        self.as_ref().timeout_owned(duration, fut)
    }

    fn now(&self) -> Instant {
        self.as_ref().now()
    }
}

// Convenience extension trait for Arc<DeterministicExecutor>
pub trait DeterministicExecutorExt {
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
}

// Add ExecutorExt implementations for AsyncStdExecutor and SmolExecutor
#[cfg(all(feature = "async", feature = "rt-async-std"))]
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

#[cfg(all(feature = "async", feature = "rt-smol"))]
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
