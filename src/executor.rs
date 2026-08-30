//! Unified executor abstraction for async operations.
//!
//! This module provides a single `Executor` trait that combines all async runtime
//! operations into one coherent interface, preventing runtime/spawner mismatches.

#[cfg(feature = "async")]
use core::future::Future;
#[cfg(feature = "async")]
use std::time::Instant;

use crate::Error;

/// Error type for executor operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExecError {
    /// Task was cancelled or panicked.
    #[error("Task execution failed: {0}")]
    TaskFailed(String),

    /// Join operation failed.
    #[error("Failed to join task: {0}")]
    JoinFailed(String),
}

impl From<ExecError> for Error {
    fn from(err: ExecError) -> Self {
        Error::InvalidState(err.to_string().into())
    }
}

/// Unified executor trait that combines spawning and timing operations.
///
/// This trait represents the complete execution context required by the library,
/// preventing misconfiguration by tying all async operations to a single type.
///
/// # Type Safety
///
/// By using a single `Executor` trait with an associated `Join` type, we ensure
/// that runtime and spawner come from the same ecosystem, preventing runtime
/// mismatches at compile time.
#[cfg(feature = "async")]
pub trait Executor: Clone + Send + Sync + 'static {
    /// The join handle type for spawned tasks.
    type Join<T>: Future<Output = Result<T, ExecError>> + Send + 'static
    where
        T: Send + 'static;

    /// Type indicating whether tasks require explicit detachment.
    ///
    /// Set to `()` if dropping the join handle detaches the task (e.g., Tokio),
    /// or a detachment token type otherwise (e.g., DeterministicExecutor).
    type Detach: Send + 'static;

    /// Spawn a future and return both join handle and optional detachment token.
    ///
    /// The detachment token ensures that tasks spawned for fire-and-forget
    /// operation will continue running even when both the join handle and
    /// detachment token are dropped. Executors where dropping the join handle
    /// is sufficient for detachment should return `()` as the detachment token.
    fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;

    /// Spawn a future as a background task.
    ///
    /// Returns a join handle that can be used to await the task's completion.
    /// This is a convenience wrapper around `spawn_with_detach` that discards
    /// the detachment token.
    fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawn_with_detach(fut).0
    }

    /// Block on a future until it completes.
    ///
    /// This is primarily used for blocking operations in an async context.
    fn block_on<F: Future>(&self, fut: F) -> F::Output;

    /// Sleep for the specified duration.
    ///
    /// Returns a future that completes after the specified duration.
    fn sleep(&self, duration: std::time::Duration) -> impl Future<Output = ()> + Send + '_;

    /// Create a timeout future.
    ///
    /// Wraps the given future with a timeout. If the future doesn't complete
    /// within the specified duration, returns an error.
    fn timeout<'a, F, T>(
        &'a self,
        duration: std::time::Duration,
        fut: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a;

    /// Spawn a background task and detach the join handle.
    ///
    /// This method guarantees that the task will continue running independently
    /// after being spawned, regardless of the executor's join handle semantics.
    /// The implementation uses `spawn_with_detach` to ensure proper detachment
    /// across all executor types.
    fn spawn_bg<F>(&self, fut: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (handle, detach_token) = self.spawn_with_detach(fut);
        drop(handle);
        drop(detach_token);
    }

    /// Get the current time according to this executor.
    ///
    /// Default implementation returns wall-clock time. Test executors
    /// (e.g., DeterministicExecutor) should override to return virtual time.
    fn now(&self) -> Instant {
        Instant::now()
    }
}

// Generic implementation for Arc<E> where E: Executor
#[cfg(feature = "async")]
#[allow(refining_impl_trait_reachable)]
impl<E> Executor for std::sync::Arc<E>
where
    E: Executor,
{
    type Join<T>
        = <E as Executor>::Join<T>
    where
        T: Send + 'static;

    type Detach = <E as Executor>::Detach;

    fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        (**self).spawn_with_detach(fut)
    }

    fn block_on<F: Future>(&self, fut: F) -> F::Output {
        (**self).block_on(fut)
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: std::time::Duration) -> impl Future<Output = ()> + Send + '_ {
        async move { (**self).sleep(duration).await }
    }

    #[allow(clippy::manual_async_fn)]
    fn timeout<'a, F, T>(
        &'a self,
        duration: std::time::Duration,
        fut: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        (**self).timeout(duration, fut)
    }

    fn now(&self) -> Instant {
        (**self).now()
    }
}

// Tokio executor implementation
#[cfg(feature = "runtime-tokio")]
mod tokio_impl {
    use std::{pin::Pin, time::Duration};

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

    // Custom join handle wrapper for Tokio
    #[derive(Debug)]
    pub struct TokioJoin<T>(tokio::task::JoinHandle<T>);

    impl<T> Future for TokioJoin<T>
    where
        T: Send + 'static,
    {
        type Output = Result<T, ExecError>;

        fn poll(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            let join_handle = Pin::new(&mut self.0);
            match join_handle.poll(cx) {
                std::task::Poll::Ready(Ok(value)) => std::task::Poll::Ready(Ok(value)),
                std::task::Poll::Ready(Err(e)) => {
                    std::task::Poll::Ready(Err(ExecError::JoinFailed(e.to_string())))
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
    }

    impl Executor for TokioExecutor {
        type Join<T>
            = TokioJoin<T>
        where
            T: Send + 'static;

        type Detach = ();

        fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            (TokioJoin(self.handle.spawn(fut)), ())
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.handle.block_on(fut)
        }

        #[allow(clippy::manual_async_fn)]
        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            let sleep = self.in_selected_context(|| tokio::time::sleep(duration));
            TokioBoundFuture::new(self.handle.clone(), sleep)
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
            let timeout = self.in_selected_context(|| tokio::time::timeout(duration, fut));
            let timeout = TokioBoundFuture::new(self.handle.clone(), timeout);
            async move {
                match timeout.await {
                    Ok(value) => Ok(value),
                    Err(_) => Err(Error::Timeout),
                }
            }
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
pub use tokio_impl::TokioExecutor;

// smol executor implementation
#[cfg(feature = "runtime-smol")]
mod smol_impl {
    use std::{pin::Pin, time::Duration};

    use super::*;

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

    // Custom join handle wrapper for smol
    #[derive(Debug)]
    pub struct SmolJoin<T>(flume::Receiver<T>);

    impl<T> Future for SmolJoin<T>
    where
        T: Send + 'static,
    {
        type Output = Result<T, ExecError>;

        fn poll(
            self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            let this = self.get_mut();
            let fut = this.0.recv_async();
            futures_lite::pin!(fut);
            match fut.poll(cx) {
                std::task::Poll::Ready(Ok(v)) => std::task::Poll::Ready(Ok(v)),
                std::task::Poll::Ready(Err(e)) => {
                    std::task::Poll::Ready(Err(ExecError::JoinFailed(e.to_string())))
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
    }

    /// Race a future against a timer, returning `Err(Error::Timeout)` if the timer fires first.
    ///
    /// This is a waker-driven timeout implementation that avoids busy polling by using
    /// `futures_lite::future::race` to efficiently wait for whichever completes first.
    async fn race_timeout<T>(
        duration: Duration,
        fut: impl Future<Output = T> + Send,
    ) -> Result<T, Error> {
        futures_lite::future::race(async { Ok(fut.await) }, async {
            smol::Timer::after(duration).await;
            Err(Error::Timeout)
        })
        .await
    }

    impl Executor for SmolExecutor {
        type Join<T>
            = SmolJoin<T>
        where
            T: Send + 'static;

        type Detach = ();

        fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            let (tx, rx) = flume::bounded(1);
            smol::spawn(async move {
                let _ = tx.send_async(fut.await).await;
            })
            .detach();
            (SmolJoin(rx), ())
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            smol::block_on(fut)
        }

        #[allow(clippy::manual_async_fn)]
        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            async move {
                smol::Timer::after(duration).await;
            }
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
            race_timeout(duration, fut)
        }
    }
}

#[cfg(feature = "runtime-smol")]
pub use smol_impl::SmolExecutor;

// Miri skip: every test in this module reaches `async-io`'s reactor through
// `SmolExecutor::block_on`, and that reactor calls `timerfd_create`, a foreign
// function Miri does not implement (a Miri limitation, not undefined behaviour in
// this crate). Miri aborts the whole test binary on the first unsupported call, so
// each test carries `#[cfg_attr(miri, ignore = ...)]`. They still run in the normal
// CI matrix. See https://github.com/GrantSparks/grafton-visca/issues/585.
#[cfg(all(test, feature = "runtime-smol"))]
mod smol_tests {
    use super::*;
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
    #[cfg_attr(
        miri,
        ignore = "async-io's reactor needs timerfd_create, unsupported by Miri (issue #585)"
    )]
    fn timeout_returns_error_for_never_completing_future() {
        let executor = SmolExecutor::new();
        let (never_complete, _poll_count) = NeverComplete::new();
        let result = executor.block_on(executor.timeout(Duration::from_millis(10), never_complete));
        assert!(matches!(result, Err(Error::Timeout)));
    }

    /// Test that an immediately completing future returns success.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "async-io's reactor needs timerfd_create, unsupported by Miri (issue #585)"
    )]
    fn timeout_returns_ok_for_immediate_completion() {
        let executor = SmolExecutor::new();
        let result = executor.block_on(executor.timeout(Duration::from_secs(10), async { 42 }));
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
    #[cfg_attr(
        miri,
        ignore = "async-io's reactor needs timerfd_create, unsupported by Miri (issue #585)"
    )]
    fn timeout_does_not_busy_poll() {
        let executor = SmolExecutor::new();
        let (never_complete, poll_count) = NeverComplete::new();

        let result = executor.block_on(executor.timeout(Duration::from_millis(50), never_complete));
        assert!(matches!(result, Err(Error::Timeout)));

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
    #[cfg_attr(
        miri,
        ignore = "async-io's reactor needs timerfd_create, unsupported by Miri (issue #585)"
    )]
    fn timeout_future_completing_before_deadline_succeeds() {
        let executor = SmolExecutor::new();
        let result = executor.block_on(executor.timeout(Duration::from_millis(100), async {
            smol::Timer::after(Duration::from_millis(10)).await;
            "completed"
        }));
        assert!(matches!(result, Ok("completed")));
    }
}
