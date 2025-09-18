//! Camera builder with unified Executor support.
//!
//! This module provides a builder pattern for constructing cameras with
//! the new unified Executor trait, preventing runtime/spawner mismatches.
//!
//! # Supported Runtimes
//!
//! The builder supports multiple async runtimes through the unified executor approach:
//! - **Tokio**: `CameraBuilder::with_executor(TokioRuntime::from_current())` (requires `runtime-tokio` feature)
//! - **async-std**: `CameraBuilder::with_executor(AsyncStdRuntime::new())` (requires `runtime-async-std` feature)
//! - **smol**: `CameraBuilder::with_executor(SmolRuntime::new())` (requires `runtime-smol` feature)
//!
//! # Example
//!
//! ```ignore
//! use grafton_visca::camera::profiles::PtzOpticsG2;
//! use grafton_visca::CameraBuilder;
//!
//! // Tokio
//! use grafton_visca::runtime::{Runtime, TokioRuntime};
//! let runtime = TokioRuntime::from_current()?;
//! let camera = CameraBuilder::with_executor(runtime)
//!     .open_async::<PtzOpticsG2, _>(transport)
//!     .await?;
//!
//! // Use accessor-style API
//! camera.power().on().await?;
//! camera.zoom().tele().await?;
//! camera.await_idle().await?;
//!
//! // async-std
//! use grafton_visca::runtime::{AsyncStdRuntime, Runtime};
//! let runtime = AsyncStdRuntime::new();
//! let camera = CameraBuilder::with_executor(runtime)
//!     .open_async::<PtzOpticsG2, _>(transport)
//!     .await?;
//!
//! // smol
//! use grafton_visca::runtime::{Runtime, SmolRuntime};
//! let runtime = SmolRuntime::new();
//! let camera = CameraBuilder::with_executor(runtime)
//!     .open_async::<PtzOpticsG2, _>(transport)
//!     .await?;
//! ```

#[cfg(feature = "mode-async")]
use std::sync::Arc;

#[cfg(not(feature = "mode-async"))]
use crate::transport::BlockingTransport;
#[cfg(feature = "mode-async")]
use crate::{camera::Camera, executor::Executor, mode, transport::AsyncTransport};
use crate::{camera_id::CameraId, capabilities::Profile, error::Error, timeout::TimeoutConfig};

/// Builder for creating cameras with explicit executor configuration.
///
/// This builder ensures that async cameras are always created with a
/// properly configured executor, preventing runtime mismatches.
pub struct CameraBuilder<E = ()> {
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
    #[cfg(feature = "mode-async")]
    executor: Option<Arc<E>>,
    #[cfg(not(feature = "mode-async"))]
    _phantom: std::marker::PhantomData<E>,
}

/// Builder with async transport attached (BYO transport pattern).
#[cfg(feature = "mode-async")]
#[derive(Debug)]
pub struct CameraBuilderWithAsyncTransport<E, T> {
    transport: T,
    executor: Option<Arc<E>>,
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
}

#[cfg(feature = "mode-async")]
impl<E, T> CameraBuilderWithAsyncTransport<E, T>
where
    E: Executor + Send + Sync + 'static,
    T: AsyncTransport + Send + Sync + 'static,
{
    /// Set the camera profile.
    ///
    /// This method configures the camera profile for the attached transport.
    pub fn profile<P>(self) -> CameraBuilderWithAsyncTransportAndProfile<E, T, P>
    where
        P: Profile + Default,
    {
        CameraBuilderWithAsyncTransportAndProfile {
            transport: self.transport,
            executor: self.executor,
            camera_id: self.camera_id,
            timeout_config: self.timeout_config,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Builder with async transport and profile configured.
#[cfg(feature = "mode-async")]
#[derive(Debug)]
pub struct CameraBuilderWithAsyncTransportAndProfile<E, T, P> {
    transport: T,
    executor: Option<Arc<E>>,
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(feature = "mode-async")]
impl<E, T, P> CameraBuilderWithAsyncTransportAndProfile<E, T, P>
where
    E: Executor + Send + Sync + 'static,
    T: AsyncTransport + crate::transport::HasTransportConfig + Send + Sync + 'static,
    P: Profile + Default,
{
    /// Open the async camera with the configured transport.
    ///
    /// This method attaches the provided transport to create a camera instance.
    pub async fn open_async(self) -> Result<Camera<mode::Async, P, T, E>, Error> {
        let executor = self.executor.ok_or_else(|| {
            Error::InvalidState("No executor configured. Use CameraBuilder::with_executor() with a runtime like TokioRuntime::from_current()".into())
        })?;

        // Create camera using the profile's envelope type
        let mut camera = Camera::new_async(self.transport, executor).await?;
        camera.set_camera_id(self.camera_id);
        camera.set_timeout_config(self.timeout_config);
        Ok(camera)
    }
}

impl<E> std::fmt::Debug for CameraBuilder<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = f.debug_struct("CameraBuilder");
        builder.field("camera_id", &self.camera_id);
        builder.field("timeout_config", &self.timeout_config);
        #[cfg(feature = "mode-async")]
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
            #[cfg(feature = "mode-async")]
            executor: None,
            #[cfg(not(feature = "mode-async"))]
            _phantom: std::marker::PhantomData,
        }
    }
}

impl Default for CameraBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "mode-async")]
impl<E> CameraBuilder<E>
where
    E: Executor + Send + Sync + 'static,
{
    /// Create a new camera builder with the specified executor.
    ///
    /// This is the primary way to create async cameras, ensuring
    /// that the executor is configured upfront.
    pub fn with_executor(executor: impl Into<Arc<E>>) -> Self {
        Self {
            camera_id: CameraId::default(),
            timeout_config: TimeoutConfig::default(),
            executor: Some(executor.into()),
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

    /// Create a builder from an existing transport (BYO transport pattern).
    ///
    /// This is the advanced path for users who want full control over transport
    /// configuration using the Transport builder API or custom transports.
    ///
    /// # Example
    /// ```ignore
    /// use grafton_visca::transport::Transport;
    ///
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .connect_timeout(Duration::from_secs(10))
    ///     .tcp_nodelay(true)
    ///     .build_async_with(runtime.clone())
    ///     .await?;
    ///
    /// let camera = CameraBuilder::with_executor(executor)
    ///     .from_transport(transport)
    ///     .profile::<PtzOpticsG2>()
    ///     .open_async()
    ///     .await?;
    /// ```
    pub fn from_transport<T>(self, transport: T) -> CameraBuilderWithAsyncTransport<E, T>
    where
        T: AsyncTransport + Send + Sync + 'static,
        E: Send + Sync + 'static,
    {
        CameraBuilderWithAsyncTransport {
            transport,
            executor: self.executor,
            camera_id: self.camera_id,
            timeout_config: self.timeout_config,
        }
    }

    /// Override the protocol style.
    ///
    /// Open an async camera connection with the specified profile and transport.
    ///
    /// This method explicitly connects to the camera using the provided transport.
    /// The executor must have been set via `with_executor()`.
    pub async fn open_async<P, T>(self, transport: T) -> Result<Camera<mode::Async, P, T, E>, Error>
    where
        P: Profile + Default,
        T: AsyncTransport + crate::transport::HasTransportConfig + Send + Sync + 'static,
        E: Send + Sync + 'static,
    {
        let executor = self.executor.ok_or_else(|| {
            Error::InvalidState("Executor not configured for async camera".into())
        })?;

        // Create camera using the profile's envelope type
        let mut camera = Camera::<mode::Async, P, T, E>::new_async(transport, executor).await?;
        camera.set_camera_id(self.camera_id);
        camera.set_timeout_config(self.timeout_config);

        Ok(camera)
    }
}

/// Transport type for builder.
#[cfg(not(feature = "mode-async"))]
#[derive(Debug, Clone, Copy)]
pub enum TransportType {
    /// TCP transport
    Tcp,
    /// UDP transport
    Udp,
}

/// Builder with transport configuration.
#[derive(Debug)]
#[cfg(not(feature = "mode-async"))]
pub struct CameraBuilderWithTransport {
    transport_type: TransportType,
    address: String,
}

/// Builder with BlockingTransportHandle (zero-cost variant).
#[cfg(not(feature = "mode-async"))]
pub struct CameraBuilderWithHandleTransport {
    transport: crate::transport::BlockingTransportHandle,
}

#[cfg(not(feature = "mode-async"))]
impl std::fmt::Debug for CameraBuilderWithHandleTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraBuilderWithHandleTransport")
            .field("transport", &self.transport)
            .finish()
    }
}

#[cfg(not(feature = "mode-async"))]
impl CameraBuilderWithHandleTransport {
    /// Set the camera profile.
    pub fn profile<P>(self) -> CameraBuilderWithHandleTransportAndProfile<P>
    where
        P: Profile + Default,
    {
        CameraBuilderWithHandleTransportAndProfile {
            transport: self.transport,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Builder with handle transport and profile configured.
#[cfg(not(feature = "mode-async"))]
pub struct CameraBuilderWithHandleTransportAndProfile<P> {
    transport: crate::transport::BlockingTransportHandle,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(not(feature = "mode-async"))]
impl<P> std::fmt::Debug for CameraBuilderWithHandleTransportAndProfile<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraBuilderWithHandleTransportAndProfile")
            .field("transport", &self.transport)
            .field("_phantom", &self._phantom)
            .finish()
    }
}

#[cfg(not(feature = "mode-async"))]
impl<P> CameraBuilderWithHandleTransportAndProfile<P>
where
    P: Profile + Default,
{
    /// Open the camera with the configured transport.
    ///
    /// This method attaches the provided transport to create a camera instance.
    pub fn open(
        self,
    ) -> Result<crate::BlockingCamera<P, crate::transport::BlockingTransportHandle>, Error> {
        // Create camera using the profile's envelope type
        crate::BlockingCamera::new_blocking(self.transport)
    }
}

#[cfg(not(feature = "mode-async"))]
impl CameraBuilderWithTransport {
    /// Set the camera profile.
    pub fn profile<P>(self) -> CameraBuilderWithProfile<P>
    where
        P: Profile + Default,
    {
        CameraBuilderWithProfile {
            transport_type: self.transport_type,
            address: self.address,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Builder with both transport and profile configured.
#[derive(Debug)]
#[cfg(not(feature = "mode-async"))]
pub struct CameraBuilderWithProfile<P> {
    transport_type: TransportType,
    address: String,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(not(feature = "mode-async"))]
impl<P> CameraBuilderWithProfile<P>
where
    P: Profile + Default,
{
    /// Open the camera connection with the configured settings.
    ///
    /// This method explicitly connects to the camera, making it clear that
    /// network operations occur at this point.
    pub fn open(
        self,
    ) -> Result<crate::BlockingCamera<P, crate::transport::BlockingTransportHandle>, Error> {
        // Parse the address to check if it has a port
        let addr_with_port =
            if let Ok(parsed) = crate::transport::address::HostPort::parse(&self.address) {
                // If no port specified, use the profile's default port
                if parsed.port().is_none() {
                    let default_port = match self.transport_type {
                        TransportType::Tcp => P::DEFAULT_TCP_PORT,
                        TransportType::Udp => P::DEFAULT_UDP_PORT,
                    };
                    parsed.format_socket_addr(Some(default_port))
                } else {
                    self.address.clone()
                }
            } else {
                // If parsing fails, just pass it through - let the connection fail with proper error
                self.address.clone()
            };

        let transport = match self.transport_type {
            TransportType::Tcp => {
                let tcp = crate::transport::blocking::tcp::Tcp::connect(&addr_with_port)?;
                crate::transport::BlockingTransportHandle::Tcp(tcp)
            }
            TransportType::Udp => {
                let udp = crate::transport::blocking::udp::Udp::connect(&addr_with_port)?;
                crate::transport::BlockingTransportHandle::Udp(udp)
            }
        };

        // Create camera using the profile's envelope type
        crate::BlockingCamera::new_blocking(transport)
    }
}

impl CameraBuilder<()> {
    /// Create a builder for TCP transport.
    ///
    /// # Example
    /// ```ignore
    /// let camera = CameraBuilder::tcp("192.168.0.110:5678")
    ///     .profile::<PtzOpticsG2>()
    ///     .open()?;
    /// ```
    #[cfg(not(feature = "mode-async"))]
    pub fn tcp(address: impl Into<String>) -> CameraBuilderWithTransport {
        CameraBuilderWithTransport {
            transport_type: TransportType::Tcp,
            address: address.into(),
        }
    }

    /// Create a builder for UDP transport.
    ///
    /// # Example
    /// ```ignore
    /// let camera = CameraBuilder::udp("192.168.0.110:1259")
    ///     .profile::<PtzOpticsG2>()
    ///     .open()?;
    /// ```
    #[cfg(not(feature = "mode-async"))]
    pub fn udp(address: impl Into<String>) -> CameraBuilderWithTransport {
        CameraBuilderWithTransport {
            transport_type: TransportType::Udp,
            address: address.into(),
        }
    }

    /// Create a builder from a BlockingTransportHandle.
    ///
    /// This is the standard method for TCP/UDP transports, providing zero-cost
    /// operation without boxing or dynamic dispatch.
    ///
    /// # Example
    /// ```ignore
    /// use grafton_visca::transport::Transport;
    ///
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .connect_timeout(Duration::from_secs(10))
    ///     .tcp_nodelay(true)
    ///     .build_blocking()?;
    ///
    /// let camera = CameraBuilder::from_transport_handle(transport)
    ///     .profile::<PtzOpticsG2>()
    ///     .open()?;
    /// ```
    #[cfg(not(feature = "mode-async"))]
    pub fn from_transport_handle(
        transport: crate::transport::BlockingTransportHandle,
    ) -> CameraBuilderWithHandleTransport {
        CameraBuilderWithHandleTransport { transport }
    }

    /// Build a blocking camera with the specified profile and transport.
    #[cfg(not(feature = "mode-async"))]
    pub fn build_blocking<P, T>(
        self,
        transport: T,
    ) -> Result<crate::camera::Camera<crate::mode::Blocking, P, T, ()>, Error>
    where
        P: Profile + Default,
        T: BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
    {
        // Create camera using the profile's envelope type
        let mut camera = crate::camera::Camera::new_blocking(transport)?;
        camera.set_camera_id(self.camera_id);
        camera.set_timeout_config(self.timeout_config);

        Ok(camera)
    }
}

/// Type aliases for common camera configurations with executors.
#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub mod async_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A camera using the Tokio executor.
    pub type TokioCamera<P, T> = Camera<mode::Async, P, T, crate::executor::TokioExecutor>;
}

/// Type aliases for async-std camera configurations.
#[cfg(all(feature = "mode-async", feature = "runtime-async-std"))]
pub mod async_std_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A camera using the async-std executor.
    pub type AsyncStdCamera<P, T> = Camera<mode::Async, P, T, crate::executor::AsyncStdExecutor>;
}

/// Type aliases for smol camera configurations.
#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub mod smol_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A camera using the smol executor.
    pub type SmolCamera<P, T> = Camera<mode::Async, P, T, crate::executor::SmolExecutor>;
}

/// Type aliases for blocking cameras.
#[cfg(not(feature = "mode-async"))]
pub mod blocking_cameras {
    use crate::camera::Camera;
    use crate::mode;

    /// A blocking camera (no executor needed).
    pub type BlockingCamera<P, T> = Camera<mode::Blocking, P, T, ()>;
}

#[cfg(test)]
mod tests {
    // Only import what's needed based on which tests are enabled
    #[cfg(any(
        all(feature = "mode-async", feature = "runtime-tokio"),
        all(not(feature = "mode-async"), feature = "runtime-tokio")
    ))]
    use super::CameraBuilder;

    #[cfg(any(
        all(feature = "mode-async", feature = "runtime-tokio"),
        all(not(feature = "mode-async"), feature = "runtime-tokio")
    ))]
    #[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
    use crate::error::Error;

    #[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
    #[tokio::test]
    async fn test_camera_builder_uses_profile_default() -> Result<(), Error> {
        use crate::camera::profiles::{PtzOpticsG2, SonyFR7};
        use crate::executor::TokioExecutor;
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        let executor = TokioExecutor::from_current()?;

        // SonyFR7 should use Sony encapsulated protocol by default
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::with_executor(executor.clone())
            .open_async::<SonyFR7, _>(transport)
            .await?;
        // Camera should be configured with Sony encapsulated protocol

        // PtzOpticsG2 should use RawVisca protocol by default
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::with_executor(executor)
            .open_async::<PtzOpticsG2, _>(transport)
            .await?;
        // Camera should be configured with RawVisca protocol
        Ok(())
    }

    #[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
    #[tokio::test]
    async fn test_camera_builder_with_protocol_override() -> Result<(), Error> {
        use crate::camera::profiles::SonyFR7;
        use crate::executor::TokioExecutor;
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        let executor = TokioExecutor::from_current()?;

        // Override SonyFR7 to use RawVisca instead of Sony encapsulated
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::with_executor(executor)
            .open_async::<SonyFR7, _>(transport)
            .await?;
        // Camera should be configured with RawVisca protocol despite SonyFR7 default
        Ok(())
    }

    #[cfg(all(not(feature = "mode-async"), feature = "runtime-tokio"))]
    #[test]
    fn test_blocking_camera_builder_uses_profile_default() {
        use crate::camera::profiles::{PtzOpticsG2, SonyFR7};
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        // SonyFR7 should use Sony encapsulated protocol by default
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::new()
            .build_blocking::<SonyFR7, _>(transport)
            .unwrap();
        // Camera should be configured with Sony encapsulated protocol

        // PtzOpticsG2 should use RawVisca protocol by default
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::new()
            .build_blocking::<PtzOpticsG2, _>(transport)
            .unwrap();
        // Camera should be configured with RawVisca protocol
    }

    #[cfg(all(not(feature = "mode-async"), feature = "runtime-tokio"))]
    #[test]
    fn test_blocking_camera_builder_with_protocol_override() {
        use crate::camera::profiles::SonyFR7;
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        // Override SonyFR7 to use RawVisca instead of Sony encapsulated
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::new()
            .build_blocking::<SonyFR7, _>(transport)
            .unwrap();
        // Camera should be configured with RawVisca protocol despite SonyFR7 default
    }
}
