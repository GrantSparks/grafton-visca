//! Unified executor abstraction for async operations.
//!
//! This module provides a single `Executor` trait that combines all async runtime
//! operations into one coherent interface, preventing runtime/spawner mismatches.

use crate::Error;

#[cfg(feature = "async")]
use core::future::Future;
#[cfg(feature = "async")]
use std::pin::Pin;

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
#[cfg(feature = "async")]
pub trait Executor: Send + Sync + 'static {
    /// The join handle type for spawned tasks.
    type Join<T>: Future<Output = Result<T, ExecError>> + Send + 'static
    where
        T: Send + 'static;

    /// Spawn a future as a background task.
    ///
    /// Returns a join handle that can be used to await the task's completion.
    fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;

    /// Block on a future until it completes.
    ///
    /// This is primarily used for blocking operations in an async context.
    fn block_on<F: Future>(&self, fut: F) -> F::Output;

    /// Sleep for the specified duration.
    ///
    /// Returns a future that completes after the specified duration.
    fn sleep(&self, duration: std::time::Duration)
        -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Create a timeout future.
    ///
    /// Wraps the given future with a timeout. If the future doesn't complete
    /// within the specified duration, returns an error.
    fn timeout<'a, F, T>(
        &'a self,
        duration: std::time::Duration,
        fut: F,
    ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a;
}

// Tokio executor implementation
#[cfg(feature = "rt-tokio")]
mod tokio_impl {
    use super::*;
    use std::{sync::Arc, time::Duration};

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
    struct TokioJoin<T>(tokio::task::JoinHandle<T>);

    impl<T> Future for TokioJoin<T>
    where
        T: Send + 'static,
    {
        type Output = Result<T, ExecError>;

        fn poll(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            // Use Pin::as_mut() to project through the pin safely
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
            = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            let handle = self.handle.spawn(fut);
            Box::pin(TokioJoin(handle))
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.handle.block_on(fut)
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(tokio::time::sleep(duration))
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            fut: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            Box::pin(async move {
                match tokio::time::timeout(duration, fut).await {
                    Ok(value) => Ok(value),
                    Err(_) => Err(Error::Timeout),
                }
            })
        }
    }

    // Implement Executor for Arc<TokioExecutor> to match the pattern used by other executors
    impl Executor for Arc<TokioExecutor> {
        type Join<T>
            = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.as_ref().spawn(fut)
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.as_ref().block_on(fut)
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.as_ref().sleep(duration)
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            fut: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.as_ref().timeout(duration, fut)
        }
    }
}

#[cfg(feature = "rt-tokio")]
pub use tokio_impl::TokioExecutor;

// async-std executor implementation
#[cfg(feature = "rt-async-std")]
mod async_std_impl {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

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
    struct AsyncStdJoin<T>(async_std::task::JoinHandle<T>);

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
            = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            let handle = async_std::task::spawn(fut);
            Box::pin(AsyncStdJoin(handle))
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            async_std::task::block_on(fut)
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(async_std::task::sleep(duration))
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            fut: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            Box::pin(async move {
                match async_std::future::timeout(duration, fut).await {
                    Ok(value) => Ok(value),
                    Err(_) => Err(Error::Timeout),
                }
            })
        }
    }

    // Implement Executor for Arc<AsyncStdExecutor>
    impl Executor for Arc<AsyncStdExecutor> {
        type Join<T>
            = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.as_ref().spawn(fut)
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.as_ref().block_on(fut)
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.as_ref().sleep(duration)
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            fut: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.as_ref().timeout(duration, fut)
        }
    }
}

#[cfg(feature = "rt-async-std")]
pub use async_std_impl::AsyncStdExecutor;

// smol executor implementation
#[cfg(feature = "rt-smol")]
mod smol_impl {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    /// smol-based executor implementation.
    #[derive(Debug, Clone)]
    pub struct SmolExecutor {
        executor: Arc<async_executor::Executor<'static>>,
    }

    impl SmolExecutor {
        /// Create a new smol executor.
        pub fn new() -> Self {
            Self {
                executor: Arc::new(async_executor::Executor::new()),
            }
        }
    }

    impl Default for SmolExecutor {
        fn default() -> Self {
            Self::new()
        }
    }

    // Custom join handle wrapper for smol
    struct SmolJoin<T>(async_executor::Task<T>);

    impl<T> Future for SmolJoin<T>
    where
        T: Send + 'static,
    {
        type Output = Result<T, ExecError>;

        fn poll(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            let task = Pin::new(&mut self.0);
            match task.poll(cx) {
                std::task::Poll::Ready(value) => std::task::Poll::Ready(Ok(value)),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
    }

    impl Executor for SmolExecutor {
        type Join<T>
            = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            let task = self.executor.spawn(fut);
            Box::pin(SmolJoin(task))
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            smol::block_on(self.executor.run(fut))
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(async move {
                smol::Timer::after(duration).await;
            })
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            fut: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            Box::pin(async move {
                // Create a timer future
                let timer = smol::Timer::after(duration);

                // Pin both futures for select
                futures_lite::pin!(fut);
                futures_lite::pin!(timer);

                // Race the two futures
                loop {
                    if let Some(value) = futures_lite::future::poll_once(&mut fut).await {
                        return Ok(value);
                    }

                    if futures_lite::future::poll_once(&mut timer).await.is_some() {
                        return Err(Error::Timeout);
                    }

                    // Yield to executor
                    futures_lite::future::yield_now().await;
                }
            })
        }
    }

    // Implement Executor for Arc<SmolExecutor>
    impl Executor for Arc<SmolExecutor> {
        type Join<T>
            = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.as_ref().spawn(fut)
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.as_ref().block_on(fut)
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.as_ref().sleep(duration)
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            fut: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.as_ref().timeout(duration, fut)
        }
    }
}

#[cfg(feature = "rt-smol")]
pub use smol_impl::SmolExecutor;
