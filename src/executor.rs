//! Unified executor abstraction for async operations.
//!
//! This module provides a single `Executor` trait that combines all async runtime
//! operations into one coherent interface, preventing runtime/spawner mismatches.

#[cfg(feature = "mode-async")]
use core::future::Future;
#[cfg(feature = "mode-async")]
use std::time::Instant;

use crate::Error;

/// Error type for executor operations.
#[derive(Debug, thiserror::Error)]
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
#[cfg(feature = "mode-async")]
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

    /// Create a timeout future that owns all its data.
    ///
    /// Unlike the `timeout` method, this returns a `'static` future that doesn't
    /// borrow from `&self`. This is useful when creating boxed static futures
    /// that need to embed timeout operations.
    ///
    /// The returned future will complete with `Ok(T)` if the input future
    /// completes within the duration, or `Err(Error::Timeout)` if it times out.
    fn timeout_owned<T>(
        &self,
        duration: std::time::Duration,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'static
    where
        T: Send + 'static;
}

// Generic implementation for Arc<E> where E: Executor
#[cfg(feature = "mode-async")]
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

    #[allow(clippy::manual_async_fn)]
    fn timeout_owned<T>(
        &self,
        duration: std::time::Duration,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'static
    where
        T: Send + 'static,
    {
        (**self).timeout_owned(duration, fut)
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
            async move { tokio::time::sleep(duration).await }
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
                match tokio::time::timeout(duration, fut).await {
                    Ok(value) => Ok(value),
                    Err(_) => Err(Error::Timeout),
                }
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
            async move {
                match tokio::time::timeout(duration, fut).await {
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
            tokio::time::Instant::now().into_std()
        }
    }
}

#[cfg(feature = "runtime-tokio")]
pub use tokio_impl::TokioExecutor;

// async-std executor implementation
#[cfg(feature = "runtime-async-std")]
mod async_std_impl {
    use std::{pin::Pin, time::Duration};

    use super::*;

    /// async-std based executor implementation.
    #[derive(Debug, Clone, Copy)]
    pub struct AsyncStdExecutor;

    impl AsyncStdExecutor {
        /// Create a new async-std executor.
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for AsyncStdExecutor {
        fn default() -> Self {
            Self::new()
        }
    }

    // Custom join handle wrapper for async-std
    #[derive(Debug)]
    pub struct AsyncStdJoin<T>(async_std::task::JoinHandle<T>);

    impl<T> Future for AsyncStdJoin<T>
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
                std::task::Poll::Ready(value) => std::task::Poll::Ready(Ok(value)),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
    }

    impl Executor for AsyncStdExecutor {
        type Join<T>
            = AsyncStdJoin<T>
        where
            T: Send + 'static;

        type Detach = ();

        fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            (AsyncStdJoin(async_std::task::spawn(fut)), ())
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            async_std::task::block_on(fut)
        }

        #[allow(clippy::manual_async_fn)]
        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            async move { async_std::task::sleep(duration).await }
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
                match async_std::future::timeout(duration, fut).await {
                    Ok(value) => Ok(value),
                    Err(_) => Err(Error::Timeout),
                }
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
            async move {
                match async_std::future::timeout(duration, fut).await {
                    Ok(value) => Ok(value),
                    Err(_) => Err(Error::Timeout),
                }
            }
        }
    }
}

#[cfg(feature = "runtime-async-std")]
pub use async_std_impl::AsyncStdExecutor;

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
            async move {
                let timer = smol::Timer::after(duration);

                futures_lite::pin!(fut);
                futures_lite::pin!(timer);

                loop {
                    if let Some(value) = futures_lite::future::poll_once(&mut fut).await {
                        return Ok(value);
                    }

                    if futures_lite::future::poll_once(&mut timer).await.is_some() {
                        return Err(Error::Timeout);
                    }

                    futures_lite::future::yield_now().await;
                }
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
            async move {
                let timer = smol::Timer::after(duration);

                futures_lite::pin!(fut);
                futures_lite::pin!(timer);

                loop {
                    if let Some(value) = futures_lite::future::poll_once(&mut fut).await {
                        return Ok(value);
                    }

                    if futures_lite::future::poll_once(&mut timer).await.is_some() {
                        return Err(Error::Timeout);
                    }

                    futures_lite::future::yield_now().await;
                }
            }
        }
    }
}

#[cfg(feature = "runtime-smol")]
pub use smol_impl::SmolExecutor;
