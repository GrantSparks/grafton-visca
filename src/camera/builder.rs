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
//!     .open_async::<PtzOpticsG2, _>(transport)?;
//!
//! // async-std
//! let camera = CameraBuilder::async_std()
//!     .open_async::<PtzOpticsG2, _>(transport)?;
//!
//! // smol
//! let camera = CameraBuilder::smol()
//!     .open_async::<PtzOpticsG2, _>(transport)?;
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

/// Builder with async transport attached (BYO transport pattern).
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct CameraBuilderWithAsyncTransport<E, T> {
    transport: T,
    executor: Option<Arc<E>>,
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
    protocol_style: Option<ProtocolStyle>,
    auto_detect_protocol: bool,
}

#[cfg(feature = "async")]
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
            protocol_style: self.protocol_style,
            auto_detect_protocol: self.auto_detect_protocol,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Override the protocol style.
    ///
    /// By default, the camera will use the protocol style declared by the profile.
    /// This method allows overriding that for specific deployments.
    pub fn protocol_style(mut self, style: ProtocolStyle) -> Self {
        self.protocol_style = Some(style);
        self
    }

    /// Enable automatic protocol detection.
    ///
    /// When enabled, the builder will probe the camera to detect
    /// whether it uses Sony encapsulated or raw VISCA protocol.
    pub fn auto_detect_protocol(mut self, enabled: bool) -> Self {
        self.auto_detect_protocol = enabled;
        self
    }
}

/// Builder with async transport and profile configured.
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct CameraBuilderWithAsyncTransportAndProfile<E, T, P> {
    transport: T,
    executor: Option<Arc<E>>,
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
    protocol_style: Option<ProtocolStyle>,
    auto_detect_protocol: bool,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(feature = "async")]
impl<E, T, P> CameraBuilderWithAsyncTransportAndProfile<E, T, P>
where
    E: Executor + Send + Sync + 'static,
    T: AsyncTransport + Send + Sync + 'static,
    P: Profile + Default,
{
    /// Open the async camera with the configured transport.
    ///
    /// This method attaches the provided transport to create a camera instance.
    pub async fn open_async(self) -> Result<Camera<mode::Async, P, T, E>, Error> {
        let executor = self.executor.ok_or_else(|| {
            Error::InvalidState("No executor configured. Use CameraBuilder::with_executor() or a runtime-specific constructor like CameraBuilder::tokio()".into())
        })?;

        // Determine protocol style and build camera
        if self.auto_detect_protocol {
            // Create a mutable reference to transport for detection
            let mut transport = self.transport;
            let detector = ProtocolDetector::new();
            let detection_result = detector.detect_protocol(&mut transport, &*executor).await?;

            let protocol_style = match detection_result.to_protocol_style() {
                Some(style) => style,
                None => {
                    return Err(Error::ConnectionFailed {
                        addr: "camera".into(),
                        source: std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "No valid VISCA protocol response detected",
                        ),
                    });
                }
            };

            let mut camera =
                Camera::new_async_with_style(transport, executor, protocol_style).await?;
            camera.set_camera_id(self.camera_id);
            camera.set_timeout_config(self.timeout_config);
            Ok(camera)
        } else {
            let protocol_style = self.protocol_style.unwrap_or(P::PROTOCOL_STYLE);
            let mut camera =
                Camera::new_async_with_style(self.transport, executor, protocol_style).await?;
            camera.set_camera_id(self.camera_id);
            camera.set_timeout_config(self.timeout_config);
            Ok(camera)
        }
    }
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

    /// Create a builder from an existing transport (BYO transport pattern).
    ///
    /// This is the advanced path for users who want full control over transport
    /// configuration using the TransportBuilder or custom transports.
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
            protocol_style: self.protocol_style,
            auto_detect_protocol: self.auto_detect_protocol,
        }
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

    /// Open an async camera connection with the specified profile and transport.
    ///
    /// This method explicitly connects to the camera using the provided transport.
    /// The executor must have been set via `with_executor()`.
    pub async fn open_async<P, T>(
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
    protocol_style: Option<ProtocolStyle>,
}

/// Builder with BYO (Bring Your Own) transport.
#[cfg(not(feature = "async"))]
pub struct CameraBuilderWithBYOTransport {
    transport: Box<dyn SyncTransport>,
    protocol_style: Option<ProtocolStyle>,
}

#[cfg(not(feature = "async"))]
impl std::fmt::Debug for CameraBuilderWithBYOTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraBuilderWithBYOTransport")
            .field("transport", &"Box<dyn SyncTransport>")
            .field("protocol_style", &self.protocol_style)
            .finish()
    }
}

/// Builder for auto-detecting transport and protocol.
#[derive(Debug)]
#[cfg(not(feature = "async"))]
pub struct CameraBuilderWithAutoDetect {
    address: String,
}

#[cfg(not(feature = "async"))]
impl CameraBuilderWithAutoDetect {
    /// Set the camera profile.
    pub fn profile<P>(self) -> CameraBuilderWithAutoDetectAndProfile<P>
    where
        P: Profile + Default,
    {
        CameraBuilderWithAutoDetectAndProfile {
            address: self.address,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Builder with auto-detect and profile configured.
#[derive(Debug)]
#[cfg(not(feature = "async"))]
pub struct CameraBuilderWithAutoDetectAndProfile<P> {
    address: String,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(not(feature = "async"))]
impl<P> CameraBuilderWithAutoDetectAndProfile<P>
where
    P: Profile + Default,
{
    /// Open the camera with auto-detected transport and protocol.
    ///
    /// This method tries multiple transport/protocol combinations to find
    /// the one that works with the camera.
    pub fn open(self) -> Result<crate::BlockingCamera<P, Box<dyn SyncTransport>>, Error> {
        use crate::transport::blocking::{tcp::Tcp, udp::Udp};
        use std::time::Duration;

        // Extract host from address (remove port if present)
        let host = if let Some(colon_pos) = self.address.rfind(':') {
            // Check if this is actually a port (not IPv6)
            if self.address[colon_pos + 1..].parse::<u16>().is_ok() {
                &self.address[..colon_pos]
            } else {
                &self.address
            }
        } else {
            &self.address
        };

        // Detection candidates in priority order
        let candidates = vec![
            // Sony cameras
            (
                format!("{}:52381", host),
                TransportType::Udp,
                ProtocolStyle::SonyEncapsulated,
            ),
            (
                format!("{}:52381", host),
                TransportType::Tcp,
                ProtocolStyle::SonyEncapsulated,
            ),
            // PTZOptics cameras
            (
                format!("{}:1259", host),
                TransportType::Udp,
                ProtocolStyle::RawVisca,
            ),
            (
                format!("{}:5678", host),
                TransportType::Tcp,
                ProtocolStyle::RawVisca,
            ),
        ];

        // Try each candidate
        for (address, transport_type, protocol_style) in candidates {
            // Try to connect
            let transport_result: Result<Box<dyn SyncTransport>, Error> = match transport_type {
                TransportType::Tcp => {
                    match Tcp::connect(&address) {
                        Ok(t) => Ok(Box::new(t)),
                        Err(_) => continue, // Try next candidate
                    }
                }
                TransportType::Udp => {
                    match Udp::connect(&address) {
                        Ok(t) => Ok(Box::new(t)),
                        Err(_) => continue, // Try next candidate
                    }
                }
            };

            if let Ok(mut transport) = transport_result {
                // Test with a simple inquiry command
                let test_command = &[0x81, 0x09, 0x00, 0x02, 0xFF]; // Version Inquiry

                // Send command and check for response
                if transport
                    .send_with_kind(test_command, crate::command::CommandKind::Inquiry)
                    .is_ok()
                {
                    // Try to receive with timeout
                    if let Ok(response) = transport.recv_with_timeout(Duration::from_millis(100)) {
                        // Check if response looks valid
                        if !response.is_empty()
                            && Self::is_valid_response(&response, protocol_style)
                        {
                            // Found working combination
                            return crate::BlockingCamera::new_with_style(
                                transport,
                                protocol_style,
                            );
                        }
                    }
                }
            }
        }

        Err(Error::ConnectionFailed {
            addr: self.address.clone().into(),
            source: std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "No VISCA protocol response detected from camera - verify camera is powered on and address is correct"
            ),
        })
    }

    /// Check if a response is valid for the given protocol style.
    fn is_valid_response(data: &[u8], protocol_style: ProtocolStyle) -> bool {
        match protocol_style {
            ProtocolStyle::SonyEncapsulated => {
                // Sony response should have at least 8-byte header
                data.len() >= 8 && data[0] == 0x01 && data[1] == 0x11
            }
            ProtocolStyle::RawVisca => {
                // Raw VISCA response should start with 0x90
                !data.is_empty() && (data[0] == 0x90 || data[0] == 0x50)
            }
        }
    }
}

#[cfg(not(feature = "async"))]
impl CameraBuilderWithBYOTransport {
    /// Set the camera profile.
    pub fn profile<P>(self) -> CameraBuilderWithBYOTransportAndProfile<P>
    where
        P: Profile + Default,
    {
        CameraBuilderWithBYOTransportAndProfile {
            transport: self.transport,
            protocol_style: self.protocol_style,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Override the protocol style.
    ///
    /// By default, the camera will use the protocol style declared by the profile.
    /// This method allows overriding that for specific deployments.
    pub fn protocol_style(mut self, style: ProtocolStyle) -> Self {
        self.protocol_style = Some(style);
        self
    }
}

/// Builder with BYO transport and profile configured.
#[cfg(not(feature = "async"))]
pub struct CameraBuilderWithBYOTransportAndProfile<P> {
    transport: Box<dyn SyncTransport>,
    protocol_style: Option<ProtocolStyle>,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(not(feature = "async"))]
impl<P> std::fmt::Debug for CameraBuilderWithBYOTransportAndProfile<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraBuilderWithBYOTransportAndProfile")
            .field("transport", &"Box<dyn SyncTransport>")
            .field("protocol_style", &self.protocol_style)
            .field("_phantom", &self._phantom)
            .finish()
    }
}

#[cfg(not(feature = "async"))]
impl<P> CameraBuilderWithBYOTransportAndProfile<P>
where
    P: Profile + Default,
{
    /// Override the protocol style.
    ///
    /// By default, the camera will use the protocol style declared by the profile.
    /// This method allows overriding that for specific deployments.
    pub fn protocol_style(mut self, style: ProtocolStyle) -> Self {
        self.protocol_style = Some(style);
        self
    }

    /// Open the camera with the configured transport.
    ///
    /// This method attaches the provided transport to create a camera instance.
    pub fn open(self) -> Result<crate::BlockingCamera<P, Box<dyn SyncTransport>>, Error> {
        // Use protocol style override if provided, otherwise use profile default
        let protocol_style = self.protocol_style.unwrap_or(P::PROTOCOL_STYLE);
        crate::BlockingCamera::new_with_style(self.transport, protocol_style)
    }
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
            protocol_style: self.protocol_style,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Override the protocol style.
    ///
    /// By default, the camera will use the protocol style declared by the profile.
    /// This method allows overriding that for specific deployments.
    pub fn protocol_style(mut self, style: ProtocolStyle) -> Self {
        self.protocol_style = Some(style);
        self
    }
}

/// Builder with both transport and profile configured.
#[derive(Debug)]
#[cfg(not(feature = "async"))]
pub struct CameraBuilderWithProfile<P> {
    transport_type: TransportType,
    address: String,
    protocol_style: Option<ProtocolStyle>,
    _phantom: std::marker::PhantomData<P>,
}

#[cfg(not(feature = "async"))]
impl<P> CameraBuilderWithProfile<P>
where
    P: Profile + Default,
{
    /// Override the protocol style.
    ///
    /// By default, the camera will use the protocol style declared by the profile.
    /// This method allows overriding that for specific deployments.
    pub fn protocol_style(mut self, style: ProtocolStyle) -> Self {
        self.protocol_style = Some(style);
        self
    }

    /// Open the camera connection with the configured settings.
    ///
    /// This method explicitly connects to the camera, making it clear that
    /// network operations occur at this point.
    pub fn open(self) -> Result<crate::BlockingCamera<P, Box<dyn SyncTransport>>, Error> {
        let transport: Box<dyn SyncTransport> = match self.transport_type {
            TransportType::Tcp => Box::new(crate::transport::blocking::tcp::Tcp::connect(
                &self.address,
            )?),
            TransportType::Udp => Box::new(crate::transport::blocking::udp::Udp::connect(
                &self.address,
            )?),
        };

        // Use protocol style override if provided, otherwise use profile default
        let protocol_style = self.protocol_style.unwrap_or(P::PROTOCOL_STYLE);
        crate::BlockingCamera::new_with_style(transport, protocol_style)
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
    #[cfg(not(feature = "async"))]
    pub fn tcp(address: impl Into<String>) -> CameraBuilderWithTransport {
        CameraBuilderWithTransport {
            transport_type: TransportType::Tcp,
            address: address.into(),
            protocol_style: None,
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
    #[cfg(not(feature = "async"))]
    pub fn udp(address: impl Into<String>) -> CameraBuilderWithTransport {
        CameraBuilderWithTransport {
            transport_type: TransportType::Udp,
            address: address.into(),
            protocol_style: None,
        }
    }

    /// Create a builder that automatically detects the transport and protocol.
    ///
    /// This method tries multiple transport/protocol combinations to find
    /// the one that the camera responds to:
    /// 1. UDP 52381 with Sony encapsulated (primary Sony path)
    /// 2. TCP 52381 with Sony encapsulated (some stacks support TCP)
    /// 3. UDP 1259 with raw VISCA (PTZOptics default)
    /// 4. TCP 5678 with raw VISCA (PTZOptics TCP)
    ///
    /// # Example
    /// ```ignore
    /// let camera = CameraBuilder::connect_auto("192.168.0.110")
    ///     .profile::<PtzOpticsG2>()
    ///     .open()?;
    /// ```
    #[cfg(not(feature = "async"))]
    pub fn connect_auto(address: impl Into<String>) -> CameraBuilderWithAutoDetect {
        CameraBuilderWithAutoDetect {
            address: address.into(),
        }
    }

    /// Create a builder from an existing transport (BYO transport pattern).
    ///
    /// This is the advanced path for users who want full control over transport
    /// configuration using the TransportBuilder or custom transports.
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
    /// let camera = CameraBuilder::from_transport(transport)
    ///     .profile::<PtzOpticsG2>()
    ///     .open()?;
    /// ```
    #[cfg(not(feature = "async"))]
    pub fn from_transport(transport: Box<dyn SyncTransport>) -> CameraBuilderWithBYOTransport {
        CameraBuilderWithBYOTransport {
            transport,
            protocol_style: None,
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
    ///     .open_async::<PtzOpticsG2, _>(transport).await?;
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
    ///     .open_async::<PtzOpticsG2, _>(transport).await?;
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
    ///     .open_async::<PtzOpticsG2, _>(transport).await?;
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
            .open_async::<SonyFR7, _>(transport)
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
