//! Executor abstraction for async task spawning.
//!
//! This module provides traits and utilities for abstracting over different async runtimes.
//! It includes a minimal blocking executor for synchronous operation and a `Spawner` trait
//! that allows users to provide their own async runtime integration.

use crate::Error;
use core::future::Future;
use core::task::{Context, Poll, Waker};
#[cfg(feature = "async")]
use std::pin::Pin;
use std::sync::Arc;

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
/// ```no_run
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
///     let camera = Camera::new_with_spawner(transport, spawner);
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
/// ```no_run
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
///         let camera = Camera::new_with_spawner(transport, spawner);
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
/// ```no_run
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
///     let camera = Camera::new_with_spawner(transport, spawner);
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
#[derive(Debug, Clone)]
pub struct BlockingSpawner;

#[cfg(feature = "async")]
impl BlockingSpawner {
    /// Create a new blocking spawner.
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
            // Use the existing block_on function to run the future
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
    // Safety: We're pinning the future to execute it
    let mut fut = Box::pin(fut);

    // Create a no-op waker (blocking futures should be Ready)
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    // Poll the future - for Ready futures this returns immediately
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(val) => val,
        Poll::Pending => {
            // This should not happen with blocking transports
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
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return Ok(val),
            Poll::Pending => {
                if Instant::now() >= deadline {
                    return Err(Error::Timeout);
                }
                // For truly async futures in blocking context
                // Sleep briefly to avoid busy waiting
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}

/// Create a no-op waker for blocking execution.
fn noop_waker() -> Waker {
    struct NoopWaker;

    impl std::task::Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
        fn wake_by_ref(self: &Arc<Self>) {}
    }

    Arc::new(NoopWaker).into()
}
