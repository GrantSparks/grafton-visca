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

#[cfg(feature = "async")]
use crate::transport::protocol_detection::ProtocolDetector;
#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
#[cfg(feature = "async")]
use crate::{camera::Camera, executor::Executor, mode, transport::AsyncTransport};
use crate::{
    camera_id::CameraId,
    capabilities::{Profile, ProtocolStyle},
    error::Error,
    timeout::TimeoutConfig,
};
#[cfg(feature = "async")]
use std::sync::Arc;

/// Builder for creating cameras with explicit executor configuration.
///
/// This builder ensures that async cameras are always created with a
/// properly configured executor, preventing runtime mismatches.
pub struct CameraBuilder<E = ()> {
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
    protocol_style: Option<ProtocolStyle>,
    #[cfg(feature = "async")]
    auto_detect_protocol: bool,
    #[cfg(feature = "async")]
    executor: Option<Arc<E>>,
    #[cfg(not(feature = "async"))]
    _phantom: std::marker::PhantomData<E>,
}

impl<E> std::fmt::Debug for CameraBuilder<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = f.debug_struct("CameraBuilder");
        builder.field("camera_id", &self.camera_id);
        builder.field("timeout_config", &self.timeout_config);
        builder.field("protocol_style", &self.protocol_style);
        #[cfg(feature = "async")]
        builder.field("auto_detect_protocol", &self.auto_detect_protocol);
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
            protocol_style: None,
            #[cfg(feature = "async")]
            auto_detect_protocol: false,
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
            protocol_style: None,
            auto_detect_protocol: false,
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

    /// Override the protocol style.
    ///
    /// By default, the camera will use the protocol style declared by the profile.
    /// This method allows overriding that for specific deployments.
    pub fn protocol_style(mut self, style: ProtocolStyle) -> Self {
        self.protocol_style = Some(style);
        self
    }

    /// Enable automatic protocol detection (async only).
    ///
    /// When enabled, the builder will attempt to auto-detect the camera's
    /// protocol style using the ProtocolDetector. If detection succeeds,
    /// it will override both the profile's default and any manually set style.
    pub fn auto_detect_protocol(mut self) -> Self {
        self.auto_detect_protocol = true;
        self
    }

    /// Build an async camera with the specified profile and transport.
    ///
    /// The executor must have been set via `with_executor()`.
    pub async fn build_async<P, T>(
        self,
        mut transport: T,
    ) -> Result<Camera<mode::Async, P, T, E>, Error>
    where
        P: Profile + Default,
        T: AsyncTransport + Send + Sync + 'static,
        E: Send + Sync + 'static,
    {
        let executor = self.executor.ok_or_else(|| {
            Error::InvalidState("Executor not configured for async camera".into())
        })?;

        // Determine the protocol style to use
        let protocol_style = if self.auto_detect_protocol {
            // Auto-detection has highest priority
            let detector = ProtocolDetector::new();
            match detector
                .detect_protocol(&mut transport, executor.as_ref())
                .await?
            {
                crate::transport::protocol_detection::DetectionResult::SonyEncapsulated => {
                    ProtocolStyle::SonyEncapsulated
                }
                crate::transport::protocol_detection::DetectionResult::RawVisca => {
                    ProtocolStyle::RawVisca
                }
                crate::transport::protocol_detection::DetectionResult::NoResponse => {
                    // Fall back to explicit override or profile default
                    self.protocol_style.unwrap_or(P::PROTOCOL_STYLE)
                }
            }
        } else {
            // Use explicit override if provided, otherwise use profile default
            self.protocol_style.unwrap_or(P::PROTOCOL_STYLE)
        };

        let mut camera = Camera::<mode::Async, P, T, E>::new_async_with_style(
            transport,
            executor,
            protocol_style,
        )
        .await?;
        camera.set_camera_id(self.camera_id);
        camera.set_timeout_config(self.timeout_config);

        Ok(camera)
    }
}

/// Transport type for builder.
#[cfg(not(feature = "async"))]
#[derive(Debug, Clone, Copy)]
pub enum TransportType {
    /// TCP transport
    Tcp,
    /// UDP transport
    Udp,
}

/// Builder with transport configuration.
#[derive(Debug)]
#[cfg(not(feature = "async"))]
pub struct CameraBuilderWithTransport {
    transport_type: TransportType,
    address: String,
}

#[cfg(not(feature = "async"))]
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
#[cfg(not(feature = "async"))]
pub struct CameraBuilderWithProfile<P> {
    transport_type: TransportType,
    address: String,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(not(feature = "async"))]
impl<P> CameraBuilderWithProfile<P>
where
    P: Profile + Default,
{
    /// Build the camera with the configured settings.
    pub fn build(self) -> Result<crate::BlockingCamera<P, Box<dyn SyncTransport>>, Error> {
        let transport: Box<dyn SyncTransport> = match self.transport_type {
            TransportType::Tcp => Box::new(crate::transport::blocking::tcp::Tcp::connect(
                &self.address,
            )?),
            TransportType::Udp => Box::new(crate::transport::blocking::udp::Udp::connect(
                &self.address,
            )?),
        };

        crate::BlockingCamera::new(transport)
    }
}

impl CameraBuilder<()> {
    /// Create a builder for TCP transport.
    ///
    /// # Example
    /// ```ignore
    /// let camera = CameraBuilder::tcp("192.168.0.110:5678")
    ///     .profile::<PtzOpticsG2>()
    ///     .build()?;
    /// ```
    #[cfg(not(feature = "async"))]
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
    ///     .build()?;
    /// ```
    #[cfg(not(feature = "async"))]
    pub fn udp(address: impl Into<String>) -> CameraBuilderWithTransport {
        CameraBuilderWithTransport {
            transport_type: TransportType::Udp,
            address: address.into(),
        }
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
        // Use explicit override if provided, otherwise use profile default
        let protocol_style = self.protocol_style.unwrap_or(P::PROTOCOL_STYLE);

        let mut camera = crate::camera::Camera::new_blocking_with_style(transport, protocol_style)?;
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

#[cfg(test)]
mod tests {
    // Only import what's needed based on which tests are enabled
    #[cfg(any(
        all(feature = "async", feature = "rt-tokio"),
        all(not(feature = "async"), feature = "rt-tokio")
    ))]
    use super::CameraBuilder;

    #[cfg(any(
        all(feature = "async", feature = "rt-tokio"),
        all(not(feature = "async"), feature = "rt-tokio")
    ))]
    use crate::capabilities::ProtocolStyle;

    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    use crate::error::Error;

    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    #[tokio::test]
    async fn test_camera_builder_uses_profile_default() -> Result<(), Error> {
        use crate::camera::profiles::{PtzOpticsG2, SonyFR7};
        use crate::executor::TokioExecutor;
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        let executor = TokioExecutor::from_current()?;

        // SonyFR7 should use Sony encapsulated protocol by default
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::with_executor(executor.clone())
            .build_async::<SonyFR7, _>(transport)
            .await?;
        // Camera should be configured with Sony encapsulated protocol

        // PtzOpticsG2 should use RawVisca protocol by default
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::with_executor(executor)
            .build_async::<PtzOpticsG2, _>(transport)
            .await?;
        // Camera should be configured with RawVisca protocol
        Ok(())
    }

    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    #[tokio::test]
    async fn test_camera_builder_with_protocol_override() -> Result<(), Error> {
        use crate::camera::profiles::SonyFR7;
        use crate::executor::TokioExecutor;
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        let executor = TokioExecutor::from_current()?;

        // Override SonyFR7 to use RawVisca instead of Sony encapsulated
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::with_executor(executor)
            .protocol_style(ProtocolStyle::RawVisca)
            .build_async::<SonyFR7, _>(transport)
            .await?;
        // Camera should be configured with RawVisca protocol despite SonyFR7 default
        Ok(())
    }

    #[cfg(all(not(feature = "async"), feature = "rt-tokio"))]
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

    #[cfg(all(not(feature = "async"), feature = "rt-tokio"))]
    #[test]
    fn test_blocking_camera_builder_with_protocol_override() {
        use crate::camera::profiles::SonyFR7;
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        // Override SonyFR7 to use RawVisca instead of Sony encapsulated
        let transport = ViscaCameraSimulator::new();
        let _camera = CameraBuilder::new()
            .protocol_style(ProtocolStyle::RawVisca)
            .build_blocking::<SonyFR7, _>(transport)
            .unwrap();
        // Camera should be configured with RawVisca protocol despite SonyFR7 default
    }
}
