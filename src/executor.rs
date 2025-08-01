//! Executor abstraction for async task spawning.
//!
//! This module provides traits and utilities for abstracting over different async runtimes.
//! It includes a minimal blocking executor for synchronous operation and a `Spawner` trait
//! that allows users to provide their own async runtime integration.

use core::future::Future;
use core::task::{Context, Poll};
#[cfg(feature = "async")]
use std::pin::Pin;

use crate::Error;

/// Type alias for a boxed future that can be spawned.
///
/// This type is only available when the `async` feature is enabled.
#[cfg(feature = "async")]
pub type SpawnableFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Trait for spawning asynchronous tasks.
///
/// This trait abstracts over different async runtimes, allowing users to provide
/// their own executor implementation. The library does not spawn any threads
/// implicitly when this trait is used.
///
/// This trait is only available when the `async` feature is enabled.
///
/// # Examples
///
/// ## Using with Tokio
/// ```no_run
/// # use grafton_visca::executor::Spawner;
/// # #[cfg(feature = "tokio")]
/// # {
/// let handle = tokio::runtime::Handle::current();
/// // The Handle implements Spawner automatically when the tokio feature is enabled
/// # }
/// ```
///
/// ## Using with async-std
/// ```ignore
/// use grafton_visca::executor::{Spawner, SpawnableFuture};
/// use grafton_visca::{Camera, CameraModel};
/// use std::sync::Arc;
///
/// #[derive(Clone)]
/// struct AsyncStdSpawner;
///
/// impl Spawner for AsyncStdSpawner {
///     fn spawn(&self, task: SpawnableFuture) {
///         async_std::task::spawn(task);
///     }
/// }
///
/// #[async_std::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create your transport (implement Transport trait for async-std)
///     let transport = create_async_std_transport("192.168.1.100:52381").await?;
///     
///     // Create camera with custom spawner
///     let spawner = AsyncStdSpawner;
///     let camera = Camera::new(transport).with_spawner(spawner);
///     
///     // Use the camera as normal
///     camera.pan_tilt_home().await?;
///     Ok(())
/// }
/// # fn create_async_std_transport(_: &str) -> impl std::future::Future<Output = Result<impl grafton_visca::transport::Transport, Box<dyn std::error::Error>>> {
/// #     async { Err("example only".into()) }
/// # }
/// ```
///
/// ## Using with smol
/// ```ignore
/// use grafton_visca::executor::{Spawner, SpawnableFuture};
/// use grafton_visca::{Camera, CameraModel};
///
/// #[derive(Clone)]
/// struct SmolSpawner(smol::Executor<'static>);
///
/// impl Spawner for SmolSpawner {
///     fn spawn(&self, task: SpawnableFuture) {
///         self.0.spawn(task).detach();
///     }
/// }
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let ex = smol::Executor::new();
///     
///     smol::block_on(ex.run(async {
///         // Create your transport
///         let transport = create_smol_transport("192.168.1.100:52381").await?;
///         
///         // Create camera with smol spawner
///         let spawner = SmolSpawner(ex.clone());
///         let camera = Camera::new(transport).with_spawner(spawner);
///         
///         // Use the camera
///         camera.power_on().await?;
///         Ok(())
///     }))
/// }
/// # async fn create_smol_transport(_: &str) -> Result<impl grafton_visca::transport::Transport, Box<dyn std::error::Error>> {
/// #     Err("example only".into())
/// # }
/// ```
///
/// ## Using with embassy (no_std)
/// ```ignore
/// use grafton_visca::executor::{Spawner, SpawnableFuture};
/// use embassy_executor::Spawner as EmbassySpawner;
///
/// struct EmbassySpawnerWrapper(EmbassySpawner);
///
/// impl Spawner for EmbassySpawnerWrapper {
///     fn spawn(&self, task: SpawnableFuture) {
///         // Note: Embassy requires static futures, so you may need to leak the task
///         // or use a task pool depending on your application requirements
///         self.0.spawn(async move {
///             task.await;
///         }).ok();
///     }
/// }
///
/// #[embassy_executor::main]
/// async fn main(spawner: EmbassySpawner) {
///     // Create your embedded transport
///     let transport = create_embassy_transport().await;
///     
///     // Wrap embassy spawner
///     let spawner = EmbassySpawnerWrapper(spawner);
///     let camera = Camera::new(transport).with_spawner(spawner);
///     
///     // Control camera
///     camera.zoom_in().await.ok();
/// }
/// # async fn create_embassy_transport() -> impl grafton_visca::transport::Transport {
/// #     struct DummyTransport;
/// #     impl grafton_visca::transport::Transport for DummyTransport {
/// #         type Error = core::convert::Infallible;
/// #         type SendFut<'a> = core::future::Ready<Result<(), Self::Error>>;
/// #         type RecvFut<'a> = core::future::Ready<Result<bytes::Bytes, Self::Error>>;
/// #         fn send<'a>(&'a self, _: &'a [u8]) -> Self::SendFut<'a> { core::future::ready(Ok(())) }
/// #         fn recv<'a>(&'a self) -> Self::RecvFut<'a> { core::future::ready(Ok(bytes::Bytes::new())) }
/// #     }
/// #     DummyTransport
/// # }
/// ```
#[cfg(feature = "async")]
pub trait Spawner: Send + Sync + 'static {
    /// Spawn a future as a background task.
    ///
    /// The spawned future will run independently of the caller. The executor
    /// is responsible for driving the future to completion.
    ///
    /// # Arguments
    /// * `task` - A future that yields no value when complete
    fn spawn(&self, task: SpawnableFuture);
}

/// Blanket implementation for tokio::runtime::Handle when the tokio feature is enabled.
#[cfg(feature = "tokio")]
impl Spawner for tokio::runtime::Handle {
    fn spawn(&self, task: SpawnableFuture) {
        tokio::runtime::Handle::spawn(self, task);
    }
}

/// A simple spawner that uses std::thread::spawn for testing purposes.
///
/// This spawner creates a new OS thread for each spawned task and runs
/// a minimal executor on that thread. It's useful for unit tests or
/// environments where a full async runtime is not available.
///
/// This type is only available when the `async` feature is enabled.
///
/// # Example
/// ```no_run
/// # #[cfg(feature = "async")]
/// # {
/// use grafton_visca::executor::{Spawner, BlockingSpawner};
///
/// let spawner = BlockingSpawner::new();
/// spawner.spawn(Box::pin(async {
///     println!("Task running in background thread");
/// }));
/// # }
/// ```
#[cfg(feature = "async")]
#[derive(Debug, Clone, Copy)]
pub struct BlockingSpawner;

#[cfg(feature = "async")]
impl BlockingSpawner {
    /// Create a new blocking spawner.
    #[must_use]
    pub fn new() -> Self {
        BlockingSpawner
    }
}

#[cfg(feature = "async")]
impl Default for BlockingSpawner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "async")]
impl Spawner for BlockingSpawner {
    fn spawn(&self, task: SpawnableFuture) {
        std::thread::spawn(move || {
            block_on(task);
        });
    }
}

/// Block on a future, returning its output.
///
/// This is a minimal executor that simply polls the future once.
/// For `Ready` futures (as used by blocking transports), this is
/// optimized away by the compiler.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = Box::pin(fut);

    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(val) => val,
        Poll::Pending => {
            unreachable!("Blocking transport returned Pending future - this is a bug in the transport implementation");
        }
    }
}

/// Execute a future with a timeout.
///
/// For blocking transports, this uses a simple deadline-based approach.
pub fn timeout<F: Future>(duration: core::time::Duration, fut: F) -> Result<F::Output, Error> {
    use std::time::Instant;

    let deadline = Instant::now() + duration;
    let mut fut = Box::pin(fut);
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return Ok(val),
            Poll::Pending => {
                if Instant::now() >= deadline {
                    return Err(Error::Timeout);
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}

/// Runtime-agnostic sleep trait.
///
/// This trait abstracts over different async runtime sleep implementations,
/// allowing timeout operations to work with any async runtime, not just Tokio.
///
/// # Examples
///
/// ## Using with Tokio
/// ```no_run
/// # #[cfg(feature = "tokio")]
/// # {
/// use grafton_visca::executor::{Sleep, TokioSleep};
/// use std::time::Duration;
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let sleep_impl = TokioSleep;
/// sleep_impl.sleep(Duration::from_millis(100)).await;
/// # });
/// # }
/// ```
///
/// ## Using with async-std
/// ```ignore
/// use grafton_visca::executor::Sleep;
/// use std::time::Duration;
///
/// #[derive(Clone)]
/// struct AsyncStdSleep;
///
/// impl Sleep for AsyncStdSleep {
///     async fn sleep(&self, duration: Duration) {
///         async_std::task::sleep(duration).await;
///     }
/// }
/// ```
#[cfg(feature = "async")]
pub trait Sleep: Send + Sync + 'static {
    /// Sleep for the specified duration.
    fn sleep(
        &self,
        duration: core::time::Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/// Tokio-based sleep implementation.
///
/// This implementation uses `tokio::time::sleep` for the sleep operation.
#[cfg(feature = "tokio")]
#[derive(Debug, Clone, Copy)]
pub struct TokioSleep;

#[cfg(feature = "tokio")]
impl Sleep for TokioSleep {
    fn sleep(
        &self,
        duration: core::time::Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(tokio::time::sleep(duration))
    }
}

/// No-op sleep implementation for testing.
///
/// This implementation returns immediately without sleeping,
/// useful for unit tests where actual delays are not desired.
#[cfg(feature = "async")]
#[derive(Debug, Clone, Copy)]
pub struct NoopSleep;

#[cfg(feature = "async")]
impl Sleep for NoopSleep {
    fn sleep(
        &self,
        _duration: core::time::Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

/// Blocking sleep implementation.
///
/// This implementation blocks the current thread for the specified duration.
/// It should only be used in test scenarios or when no async runtime is available.
#[cfg(feature = "async")]
#[derive(Debug, Clone, Copy)]
pub struct BlockingSleep;

#[cfg(feature = "async")]
impl Sleep for BlockingSleep {
    fn sleep(
        &self,
        duration: core::time::Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            std::thread::sleep(duration);
        })
    }
}

/// Execute an async future with a timeout using a runtime-agnostic sleep implementation.
///
/// This function races the given future against a sleep timer, returning an error
/// if the timeout is reached before the future completes.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "async")]
/// # {
/// use grafton_visca::executor::{timeout_with_sleep, NoopSleep};
/// use std::time::Duration;
///
/// # futures::executor::block_on(async {
/// let sleep_impl = NoopSleep;
/// let result = timeout_with_sleep(&sleep_impl, Duration::from_secs(1), async {
///     // Some async operation
///     42
/// }).await;
/// assert_eq!(result.unwrap(), 42);
/// # });
/// # }
/// ```
#[cfg(feature = "async")]
pub async fn timeout_with_sleep<S, F, T>(
    sleep_impl: &S,
    duration: core::time::Duration,
    fut: F,
) -> Result<T, Error>
where
    S: Sleep + ?Sized,
    F: Future<Output = T>,
{
    use futures::future::{select, Either};
    use std::pin::pin;

    let sleep_fut = sleep_impl.sleep(duration);
    let work_fut = pin!(fut);

    match select(work_fut, sleep_fut).await {
        Either::Left((result, _)) => Ok(result),
        Either::Right(((), _)) => Err(Error::Timeout),
    }
}
