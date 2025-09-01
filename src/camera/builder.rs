//! Camera builder with unified Executor support.
//!
//! This module provides a builder pattern for constructing cameras with
//! the new unified Executor trait, preventing runtime/spawner mismatches.
//!
//! # Supported Runtimes
//!
//! The builder supports multiple async runtimes through convenience methods:
//! - **Tokio**: `CameraBuilder::tokio()` (requires `rt-tokio` feature)
//! - **async-std**: `CameraBuilder::async_std()` (requires `rt-async-std` feature)
//! - **smol**: `CameraBuilder::smol()` (requires `rt-smol` feature)
//!
//! # Example
//!
//! ```ignore
//! // Tokio
//! let camera = CameraBuilder::tokio()?
//!     .build_async::<PtzOpticsG2, _>(transport)?;
//!
//! // async-std
//! let camera = CameraBuilder::async_std()
//!     .build_async::<PtzOpticsG2, _>(transport)?;
//!
//! // smol
//! let camera = CameraBuilder::smol()
//!     .build_async::<PtzOpticsG2, _>(transport)?;
//! ```

#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
#[cfg(feature = "async")]
use crate::{camera::Camera, executor::Executor, mode, transport::AsyncTransport};
use crate::{camera_id::CameraId, capabilities::Profile, error::Error, timeout::TimeoutConfig};

/// Builder for creating cameras with explicit executor configuration.
///
/// This builder ensures that async cameras are always created with a
/// properly configured executor, preventing runtime mismatches.
pub struct CameraBuilder<E = ()> {
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
    #[cfg(feature = "async")]
    executor: Option<E>,
    #[cfg(not(feature = "async"))]
    _phantom: std::marker::PhantomData<E>,
}

impl<E> std::fmt::Debug for CameraBuilder<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = f.debug_struct("CameraBuilder");
        builder.field("camera_id", &self.camera_id);
        builder.field("timeout_config", &self.timeout_config);
        #[cfg(feature = "async")]
        builder.field("executor", &self.executor.is_some());
        builder.finish()
    }
}

impl CameraBuilder<()> {
    /// Create a new camera builder for blocking mode.
    pub fn new() -> Self {
        Self {
            camera_id: CameraId::default(),
            timeout_config: TimeoutConfig::default(),
            #[cfg(feature = "async")]
            executor: None,
            #[cfg(not(feature = "async"))]
            _phantom: std::marker::PhantomData,
        }
    }
}

impl Default for CameraBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "async")]
impl<E> CameraBuilder<E>
where
    E: Executor,
{
    /// Create a new camera builder with the specified executor.
    ///
    /// This is the primary way to create async cameras, ensuring
    /// that the executor is configured upfront.
    pub fn with_executor(executor: E) -> Self {
        Self {
            camera_id: CameraId::default(),
            timeout_config: TimeoutConfig::default(),
            executor: Some(executor),
        }
    }

    /// Set the camera ID.
    pub fn camera_id(mut self, id: CameraId) -> Self {
        self.camera_id = id;
        self
    }

    /// Set the timeout configuration.
    pub fn timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = config;
        self
    }

    /// Build an async camera with the specified profile and transport.
    ///
    /// The executor must have been set via `with_executor()`.
    pub async fn build_async<P, T>(
        self,
        transport: T,
    ) -> Result<Camera<mode::Async, P, T, E>, Error>
    where
        P: Profile + Default,
        T: AsyncTransport + Send + Sync + 'static,
        E: Clone,
    {
        let executor = self.executor.ok_or_else(|| {
            Error::InvalidState("Executor not configured for async camera".into())
        })?;

        let mut camera = Camera::new_async(transport, executor).await?;
        camera.set_camera_id(self.camera_id);
        camera.set_timeout_config(self.timeout_config);

        Ok(camera)
    }
}

impl CameraBuilder<()> {
    /// Set the camera ID.
    pub fn camera_id(mut self, id: CameraId) -> Self {
        self.camera_id = id;
        self
    }

    /// Set the timeout configuration.
    pub fn timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = config;
        self
    }

    /// Build a blocking camera with the specified profile and transport.
    #[cfg(not(feature = "async"))]
    pub fn build_blocking<P, T>(
        self,
        transport: T,
    ) -> Result<crate::camera::Camera<crate::mode::Blocking, P, T, ()>, Error>
    where
        P: Profile + Default,
        T: SyncTransport + Send + 'static,
    {
        let mut camera = crate::camera::Camera::new_blocking(transport)?;
        camera.set_camera_id(self.camera_id);
        camera.set_timeout_config(self.timeout_config);

        Ok(camera)
    }
}

// Convenience constructors for specific runtime implementations
#[cfg(feature = "rt-tokio")]
impl CameraBuilder<crate::executor::TokioExecutor> {
    /// Create a builder with the Tokio executor from the current runtime.
    ///
    /// This provides the easiest way to create a Tokio-based camera:
    /// ```ignore
    /// let camera = CameraBuilder::tokio()?
    ///     .build_async::<PtzOpticsG2, _>(transport)?;
    /// ```
    pub fn tokio() -> Result<Self, Error> {
        let executor = crate::executor::TokioExecutor::from_current()?;
        Ok(Self::with_executor(executor))
    }
}

#[cfg(feature = "rt-async-std")]
impl CameraBuilder<crate::executor::AsyncStdExecutor> {
    /// Create a builder with the async-std executor.
    ///
    /// This provides the easiest way to create an async-std-based camera:
    /// ```ignore
    /// let camera = CameraBuilder::async_std()
    ///     .build_async::<PtzOpticsG2, _>(transport)?;
    /// ```
    pub fn async_std() -> Self {
        let executor = crate::executor::AsyncStdExecutor::new();
        Self::with_executor(executor)
    }
}

#[cfg(feature = "rt-smol")]
impl CameraBuilder<crate::executor::SmolExecutor> {
    /// Create a builder with the smol executor.
    ///
    /// This provides the easiest way to create a smol-based camera:
    /// ```ignore
    /// let camera = CameraBuilder::smol()
    ///     .build_async::<PtzOpticsG2, _>(transport)?;
    /// ```
    pub fn smol() -> Self {
        let executor = crate::executor::SmolExecutor::new();
        Self::with_executor(executor)
    }
}

/// Type aliases for common camera configurations with executors.
#[cfg(all(feature = "async", feature = "rt-tokio"))]
pub mod async_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A camera using the Tokio executor.
    pub type TokioCamera<P, T> = Camera<mode::Async, P, T, crate::executor::TokioExecutor>;
}

/// Type aliases for async-std camera configurations.
#[cfg(all(feature = "async", feature = "rt-async-std"))]
pub mod async_std_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A camera using the async-std executor.
    pub type AsyncStdCamera<P, T> = Camera<mode::Async, P, T, crate::executor::AsyncStdExecutor>;
}

/// Type aliases for smol camera configurations.
#[cfg(all(feature = "async", feature = "rt-smol"))]
pub mod smol_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A camera using the smol executor.
    pub type SmolCamera<P, T> = Camera<mode::Async, P, T, crate::executor::SmolExecutor>;
}

/// Type aliases for blocking cameras.
#[cfg(not(feature = "async"))]
pub mod blocking_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A blocking camera (no executor needed).
    pub type BlockingCamera<P, T> = Camera<mode::Blocking, P, T, ()>;
}
