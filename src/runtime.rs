//! Runtime abstraction for async operations.
//!
//! This module provides a clean abstraction over different async runtimes,
//! allowing the library to work with tokio, async-std, smol, or any other runtime.

use crate::Error;
use core::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "async")]
use crate::executor::{Sleep, Spawner, SpawnableFuture};

/// Runtime abstraction that provides all async runtime operations.
///
/// This trait combines spawning and timing operations into a single interface,
/// making it easy to support different async runtimes.
#[cfg(feature = "async")]
pub trait Runtime: Send + Sync + 'static {
    /// Spawn a future as a background task.
    fn spawn(&self, task: SpawnableFuture);

    /// Sleep for the specified duration.
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    // Note: timeout is not part of the trait to keep it object-safe.
    // Use the free function `timeout_with_runtime` instead.
}

/// Tokio runtime implementation.
#[cfg(feature = "tokio")]
#[derive(Debug, Clone, Copy)]
pub struct TokioRuntime;

#[cfg(feature = "tokio")]
impl Runtime for TokioRuntime {
    fn spawn(&self, task: SpawnableFuture) {
        tokio::spawn(task);
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(tokio::time::sleep(duration))
    }

    // Use tokio's optimized timeout directly via the free function
}

/// Generic runtime implementation using Sleep and Spawner traits.
#[cfg(feature = "async")]
pub struct GenericRuntime<S: Sleep, P: Spawner> {
    sleep_impl: S,
    spawner: P,
}

#[cfg(feature = "async")]
impl<S: Sleep, P: Spawner> GenericRuntime<S, P> {
    /// Create a new generic runtime from sleep and spawner implementations.
    pub fn new(sleep_impl: S, spawner: P) -> Self {
        Self {
            sleep_impl,
            spawner,
        }
    }
}

#[cfg(feature = "async")]
impl<S: Sleep + 'static, P: Spawner + 'static> Runtime for GenericRuntime<S, P> {
    fn spawn(&self, task: SpawnableFuture) {
        self.spawner.spawn(task);
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.sleep_impl.sleep(duration)
    }
}

/// Type alias for a shared runtime.
#[cfg(feature = "async")]
pub type SharedRuntime = Arc<dyn Runtime>;

/// Get the default runtime based on enabled features.
#[cfg(feature = "tokio")]
pub fn default_runtime() -> SharedRuntime {
    Arc::new(TokioRuntime)
}

/// For async without tokio, users must provide their own runtime.
#[cfg(all(feature = "async", not(feature = "tokio")))]
pub fn default_runtime() -> Option<SharedRuntime> {
    None
}

/// Execute a future with a timeout using the provided runtime.
#[cfg(feature = "async")]
pub async fn timeout_with_runtime<R, F, T>(
    runtime: &R,
    duration: Duration,
    fut: F,
) -> Result<T, Error>
where
    R: Runtime + ?Sized,
    F: Future<Output = T>,
{
    use futures::future::{select, Either};
    use std::pin::pin;

    let sleep_fut = runtime.sleep(duration);
    let work_fut = pin!(fut);

    match select(work_fut, sleep_fut).await {
        Either::Left((result, _)) => Ok(result),
        Either::Right(((), _)) => Err(Error::Timeout),
    }
}