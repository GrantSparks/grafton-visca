//! Unified executor abstraction for async operations.
//!
//! This module provides a single `Executor` trait that combines all async runtime
//! operations into one coherent interface, preventing runtime/spawner mismatches.

#[cfg(feature = "async")]
use core::future::Future;
#[cfg(feature = "async")]
use std::pin::Pin;

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
    use std::time::Duration;

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
}

#[cfg(feature = "rt-tokio")]
pub use tokio_impl::TokioExecutor;
