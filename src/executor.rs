//! Unified executor abstraction for async operations.
//!
//! This module provides a single `Executor` trait that combines all async runtime
//! operations into one coherent interface, preventing runtime/spawner mismatches.

use core::future::Future;
use std::time::{Duration, Instant};

/// A timer that can be re-armed without allocating another future.
///
/// Deadlines use the same clock as [`crate::runtime::Executor::now`]. Resetting cancels the
/// previous deadline, including after completion. The next poll observes the
/// new deadline; a pending timer wakes its most recently registered task.
pub trait ResettableTimer: Future<Output = ()> + Send + 'static {
    /// Cancel the previous deadline and arm this timer for `deadline`.
    fn reset(self: std::pin::Pin<&mut Self>, deadline: Instant);
}

/// The scheduling and clock operations an async owner needs from its runtime.
///
/// Timers, sleeps and the current time must use the same clock. Detached
/// tasks must continue running independently of the caller.
pub trait Executor: Clone + Send + Sync + 'static {
    /// Reusable timer driven by this executor's clock.
    type Timer: ResettableTimer;

    /// Create a timer at an absolute deadline on [`Self::now`]'s clock.
    ///
    /// Owners create their timers once and reset them for subsequent turns.
    fn timer(&self, deadline: Instant) -> Self::Timer;

    /// Schedule a task to run independently on this runtime.
    fn spawn_detached<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static;

    /// Sleep for the specified duration on this runtime's clock.
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_;

    /// Get the current time according to this executor.
    ///
    /// The default is the system monotonic clock. Virtual-time executors, including
    /// `ManualClockExecutor`, override it with their clock.
    fn now(&self) -> Instant {
        Instant::now()
    }
}

// Generic implementation for Arc<E> where E: Executor
#[allow(refining_impl_trait_reachable)]
impl<E> Executor for std::sync::Arc<E>
where
    E: Executor,
{
    type Timer = E::Timer;

    fn timer(&self, deadline: Instant) -> Self::Timer {
        (**self).timer(deadline)
    }

    fn spawn_detached<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        (**self).spawn_detached(future);
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        async move { (**self).sleep(duration).await }
    }

    fn now(&self) -> Instant {
        (**self).now()
    }
}

// Tokio executor implementation
#[cfg(feature = "runtime-tokio")]
mod tokio_impl {
    use crate::Error;
    use std::pin::Pin;

    use super::*;

    /// A future polled with one explicit Tokio runtime installed as the
    /// current context.
    ///
    /// Tokio binds timers and I/O resources through thread-local runtime
    /// context. `TokioExecutor::from_handle` must therefore not let an
    /// unrelated runtime that happens to poll a returned future become that
    /// context.
    #[derive(Debug)]
    pub(crate) struct TokioBoundFuture<F> {
        handle: tokio::runtime::Handle,
        inner: Pin<Box<F>>,
    }

    impl<F> TokioBoundFuture<F> {
        pub(crate) fn new(handle: tokio::runtime::Handle, inner: F) -> Self {
            Self {
                handle,
                inner: Box::pin(inner),
            }
        }
    }

    // Moving the wrapper never moves its heap-pinned inner future.
    impl<F> Unpin for TokioBoundFuture<F> {}

    impl<F> Future for TokioBoundFuture<F>
    where
        F: Future,
    {
        type Output = F::Output;

        fn poll(
            self: Pin<&mut Self>,
            context: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            let this = self.get_mut();
            let _entered = this.handle.enter();
            this.inner.as_mut().poll(context)
        }
    }

    /// A reusable timer bound to the Tokio runtime selected by its executor.
    #[derive(Debug)]
    pub struct TokioTimer {
        inner: TokioBoundFuture<tokio::time::Sleep>,
    }

    impl Future for TokioTimer {
        type Output = ();

        fn poll(self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<()> {
            Pin::new(&mut self.get_mut().inner).poll(context)
        }
    }

    impl ResettableTimer for TokioTimer {
        fn reset(self: Pin<&mut Self>, deadline: Instant) {
            let inner = &mut self.get_mut().inner;
            let _entered = inner.handle.enter();
            inner
                .inner
                .as_mut()
                .reset(tokio::time::Instant::from_std(deadline));
        }
    }

    /// Tokio-based executor implementation.
    #[derive(Debug, Clone)]
    pub struct TokioExecutor {
        handle: tokio::runtime::Handle,
    }

    impl TokioExecutor {
        /// Create an executor from the current Tokio runtime.
        pub fn from_current() -> Result<Self, Error> {
            tokio::runtime::Handle::try_current()
                .map(|handle| Self { handle })
                .map_err(|_| Error::MissingRuntime)
        }

        /// Create an executor from a specific runtime handle.
        pub fn from_handle(handle: tokio::runtime::Handle) -> Self {
            Self { handle }
        }

        /// The Tokio runtime selected for this executor.
        pub(crate) fn handle(&self) -> &tokio::runtime::Handle {
            &self.handle
        }

        fn in_selected_context<T>(&self, operation: impl FnOnce() -> T) -> T {
            let _entered = self.handle.enter();
            operation()
        }
    }

    impl Executor for TokioExecutor {
        type Timer = TokioTimer;

        fn timer(&self, deadline: Instant) -> Self::Timer {
            let sleep = self.in_selected_context(|| {
                tokio::time::sleep_until(tokio::time::Instant::from_std(deadline))
            });
            TokioTimer {
                inner: TokioBoundFuture::new(self.handle.clone(), sleep),
            }
        }

        fn spawn_detached<F>(&self, future: F)
        where
            F: Future<Output = ()> + Send + 'static,
        {
            drop(self.handle.spawn(future));
        }

        #[allow(clippy::manual_async_fn)]
        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            let sleep = self.in_selected_context(|| tokio::time::sleep(duration));
            TokioBoundFuture::new(self.handle.clone(), sleep)
        }

        /// Return the current time based on Tokio's time source.
        ///
        /// Using Tokio's time here ensures tests annotated with
        /// `#[tokio::test(start_paused = true)]` advance logically
        /// when virtual time advances, preventing stalls.
        fn now(&self) -> Instant {
            self.in_selected_context(|| tokio::time::Instant::now().into_std())
        }
    }
}

#[cfg(all(feature = "runtime-tokio", feature = "transport-serial-tokio"))]
pub(crate) use tokio_impl::TokioBoundFuture;
#[cfg(feature = "runtime-tokio")]
pub use tokio_impl::{TokioExecutor, TokioTimer};

// smol executor implementation
#[cfg(feature = "runtime-smol")]
mod smol_impl {
    use std::pin::Pin;

    use super::*;

    /// A reusable timer driven by smol's async-io clock.
    #[derive(Debug)]
    pub struct SmolTimer(smol::Timer);

    impl Future for SmolTimer {
        type Output = ();

        fn poll(self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<()> {
            Pin::new(&mut self.get_mut().0).poll(context).map(|_| ())
        }
    }

    impl ResettableTimer for SmolTimer {
        fn reset(self: Pin<&mut Self>, deadline: Instant) {
            self.get_mut().0.set_at(deadline);
        }
    }

    /// smol-based executor implementation.
    #[derive(Debug, Clone, Copy)]
    pub struct SmolExecutor;

    impl SmolExecutor {
        /// Create a new smol executor.
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for SmolExecutor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Executor for SmolExecutor {
        type Timer = SmolTimer;

        fn timer(&self, deadline: Instant) -> Self::Timer {
            SmolTimer(smol::Timer::at(deadline))
        }

        fn spawn_detached<F>(&self, future: F)
        where
            F: Future<Output = ()> + Send + 'static,
        {
            smol::spawn(future).detach();
        }

        #[allow(clippy::manual_async_fn)]
        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            async move {
                smol::Timer::after(duration).await;
            }
        }
    }
}

#[cfg(feature = "runtime-smol")]
pub use smol_impl::{SmolExecutor, SmolTimer};

#[cfg(all(
    test,
    any(
        feature = "runtime-smol",
        feature = "runtime-tokio",
        all(feature = "async", feature = "test-utils")
    )
))]
mod spawn_tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::time::Duration;

    use super::Executor;

    async fn ready_operation_wins_elapsed_timeout(executor: &impl Executor) {
        for _ in 0..10_000 {
            assert_eq!(
                crate::timeout::within(std::future::ready(42), executor.sleep(Duration::ZERO))
                    .await,
                Some(42)
            );
        }
    }

    async fn detached_task_completes(executor: &impl Executor) {
        let (done, completed) = flume::bounded(1);
        executor.spawn_detached(async move {
            done.send(42).expect("driver waits for completion");
        });
        assert_eq!(
            crate::timeout::within(
                completed.recv_async(),
                executor.sleep(Duration::from_secs(2)),
            )
            .await,
            Some(Ok(42)),
        );
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_detached_spawn_and_timeout() {
        let executor = super::SmolExecutor::new();
        smol::block_on(async {
            ready_operation_wins_elapsed_timeout(&executor).await;
            detached_task_completes(&executor).await;
        });
    }

    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn tokio_detached_spawn_and_timeout() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let executor = super::TokioExecutor::from_handle(runtime.handle().clone());
        runtime.block_on(async {
            ready_operation_wins_elapsed_timeout(&executor).await;
            detached_task_completes(&executor).await;
        });
    }

    #[cfg(all(feature = "async", feature = "test-utils"))]
    #[test]
    fn manual_clock_detached_spawn_and_timeout() {
        use crate::testing::testkit::{ManualClock, ManualClockExecutor};
        let clock = ManualClock::new();
        let executor = ManualClockExecutor::local(&clock);
        executor.run_until(async {
            ready_operation_wins_elapsed_timeout(&executor).await;
            detached_task_completes(&executor).await;
        });
    }
}

#[cfg(all(test, feature = "runtime-smol"))]
mod smol_tests {
    use super::*;
    use crate::Error;
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll},
        time::Duration,
    };

    /// A future that never completes (always returns Pending) and tracks poll count.
    struct NeverComplete {
        poll_count: Arc<AtomicUsize>,
    }

    impl NeverComplete {
        fn new() -> (Self, Arc<AtomicUsize>) {
            let poll_count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    poll_count: poll_count.clone(),
                },
                poll_count,
            )
        }
    }

    impl Future for NeverComplete {
        type Output = ();

        fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.poll_count.fetch_add(1, Ordering::SeqCst);
            Poll::Pending
        }
    }

    /// Test that a never-completing future times out correctly.
    #[test]
    fn timeout_returns_error_for_never_completing_future() {
        let executor = SmolExecutor::new();
        let (never_complete, _poll_count) = NeverComplete::new();
        let result = smol::block_on(async {
            crate::timeout::within(never_complete, executor.sleep(Duration::from_millis(10)))
                .await
                .ok_or_else(Error::io_timeout)
        });
        assert!(matches!(result, Err(Error::Timeout { .. })));
    }

    /// Test that an immediately completing future returns success.
    #[test]
    fn timeout_returns_ok_for_immediate_completion() {
        let executor = SmolExecutor::new();
        let result = smol::block_on(async {
            crate::timeout::within(async { 42 }, executor.sleep(Duration::from_secs(10)))
                .await
                .ok_or_else(Error::io_timeout)
        });
        assert!(matches!(result, Ok(42)));
    }

    /// Regression test: Verify the timeout implementation is waker-driven, not busy polling.
    ///
    /// A busy-polling loop would poll the inner future repeatedly while waiting for the timer.
    /// A waker-driven implementation should only poll a small bounded number of times:
    /// - Once initially when the timeout future is first polled
    /// - Possibly once more when the timer fires (to complete the race)
    ///
    /// We use a generous bound (10 polls) to avoid flakiness while still catching busy loops.
    #[test]
    fn timeout_does_not_busy_poll() {
        let executor = SmolExecutor::new();
        let (never_complete, poll_count) = NeverComplete::new();

        let result = smol::block_on(async {
            crate::timeout::within(never_complete, executor.sleep(Duration::from_millis(50)))
                .await
                .ok_or_else(Error::io_timeout)
        });
        assert!(matches!(result, Err(Error::Timeout { .. })));

        let polls = poll_count.load(Ordering::SeqCst);
        // A waker-driven implementation polls very few times (typically 1-2).
        // A busy-polling loop would poll hundreds or thousands of times in 50ms.
        assert!(
            polls <= 10,
            "Expected ≤10 polls for waker-driven implementation, got {polls}. \
             This suggests a busy-polling regression."
        );
    }

    /// Test that a future completing just before the timeout succeeds.
    #[test]
    fn timeout_future_completing_before_deadline_succeeds() {
        let executor = SmolExecutor::new();
        let result = smol::block_on(async {
            crate::timeout::within(
                async {
                    smol::Timer::after(Duration::from_millis(10)).await;
                    "completed"
                },
                executor.sleep(Duration::from_millis(100)),
            )
            .await
            .ok_or_else(Error::io_timeout)
        });
        assert!(matches!(result, Ok("completed")));
    }
}
