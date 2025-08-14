//! Runtime abstraction for async operations.
//!
//! This module provides a clean abstraction over different async runtimes,
//! allowing the library to work with tokio, async-std, smol, or any other runtime.

#[cfg(feature = "async")]
use core::future::Future;
#[cfg(feature = "async")]
use std::{pin::Pin, sync::Arc, time::Duration};

#[cfg(feature = "async")]
use crate::{
    executor::{Sleep, SpawnableFuture, Spawner},
    Error,
};

/// Runtime abstraction that provides all async runtime operations.
///
/// This trait combines spawning and timing operations into a single interface,
/// making it easy to support different async runtimes.
#[cfg(feature = "async")]
pub trait Runtime: std::fmt::Debug + Send + Sync + 'static {
    /// Spawn a future as a background task.
    fn spawn(&self, task: SpawnableFuture);

    /// Sleep for the specified duration.
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/// Tokio runtime implementation.
#[cfg(feature = "rt-tokio")]
#[derive(Debug, Clone, Copy)]
pub struct TokioRuntime;

#[cfg(feature = "rt-tokio")]
impl Runtime for TokioRuntime {
    fn spawn(&self, task: SpawnableFuture) {
        tokio::spawn(task);
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(tokio::time::sleep(duration))
    }
}

/// Generic runtime implementation using Sleep and Spawner traits.
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct GenericRuntime<S: Sleep, P: Spawner> {
    sleep_impl: S,
    spawner: P,
}

#[cfg(feature = "async")]
impl<S: Sleep + std::fmt::Debug + 'static, P: Spawner + std::fmt::Debug + 'static> Runtime
    for GenericRuntime<S, P>
{
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
    use std::pin::pin;

    use futures::future::{select, Either};

    let sleep_fut = runtime.sleep(duration);
    let work_fut = pin!(fut);

    match select(work_fut, sleep_fut).await {
        Either::Left((result, _)) => Ok(result),
        Either::Right(((), _)) => Err(Error::Timeout),
    }
}

/// Sleep implementation that delegates to a Runtime.
#[cfg(feature = "async")]
#[derive(Clone)]
pub struct RuntimeSleep {
    runtime: SharedRuntime,
}

#[cfg(feature = "async")]
impl std::fmt::Debug for RuntimeSleep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeSleep")
            .field("runtime", &"<Runtime>")
            .finish()
    }
}

#[cfg(feature = "async")]
impl Sleep for RuntimeSleep {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.runtime.sleep(duration)
    }
}

/// Spawner implementation that delegates to a Runtime.
#[cfg(feature = "async")]
#[derive(Clone)]
pub struct RuntimeSpawner {
    runtime: SharedRuntime,
}

#[cfg(feature = "async")]
impl std::fmt::Debug for RuntimeSpawner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeSpawner")
            .field("runtime", &"<Runtime>")
            .finish()
    }
}

#[cfg(feature = "async")]
impl Spawner for RuntimeSpawner {
    fn spawn(&self, task: SpawnableFuture) {
        self.runtime.spawn(task);
    }
}
