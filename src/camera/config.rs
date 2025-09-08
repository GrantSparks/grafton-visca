//! Pure configuration for camera connection and behavior.
//!
//! This module provides a pure-data configuration struct that holds all the settings
//! needed to establish a camera connection. The configuration is separate from the
//! actual connection process, allowing for easy cloning, reuse, and modification.

use std::time::Duration;

use crate::{
    camera_id::CameraId, capabilities::ProtocolStyle, error::Error, timeout::TimeoutConfig,
};

/// Transport configuration options.
#[derive(Debug, Clone)]
pub enum TransportOptions {
    /// TCP connection with address.
    Tcp {
        /// Host:port string (e.g., "192.168.0.110:5678")
        address: String,
    },
    /// UDP connection with address.
    Udp {
        /// Host:port string (e.g., "192.168.0.110:1259")
        address: String,
    },
    /// Serial connection.
    Serial {
        /// Serial port path (e.g., "/dev/ttyUSB0")
        port: String,
        /// Baud rate (default: 9600)
        baud_rate: u32,
    },
    /// Custom transport provided by user.
    Custom,
}

impl TransportOptions {
    /// Create TCP transport options.
    pub fn tcp(address: impl Into<String>) -> Self {
        Self::Tcp {
            address: address.into(),
        }
    }

    /// Create UDP transport options.
    pub fn udp(address: impl Into<String>) -> Self {
        Self::Udp {
            address: address.into(),
        }
    }

    /// Create serial transport options.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self {
        Self::Serial {
            port: port.into(),
            baud_rate,
        }
    }
}

/// Protocol configuration.
#[derive(Debug, Clone, Copy)]
pub enum ProtocolConfig {
    /// Use a specific protocol style.
    Explicit(ProtocolStyle),
    /// Automatically detect the protocol style.
    Auto,
}

/// Retry policy for command transmission.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_attempts: u32,
    /// Delay between retry attempts.
    pub retry_delay: Duration,
    /// Whether to use exponential backoff.
    pub exponential_backoff: bool,
}

impl RetryPolicy {
    /// No retries (suitable for TCP).
    pub const fn none() -> Self {
        Self {
            max_attempts: 0,
            retry_delay: Duration::from_millis(0),
            exponential_backoff: false,
        }
    }

    /// Recommended retry policy for UDP connections.
    pub const fn recommended_udp() -> Self {
        Self {
            max_attempts: 3,
            retry_delay: Duration::from_millis(500),
            exponential_backoff: true,
        }
    }

    /// Conservative retry policy with more attempts.
    pub const fn conservative() -> Self {
        Self {
            max_attempts: 5,
            retry_delay: Duration::from_secs(1),
            exponential_backoff: true,
        }
    }
}

/// Pure configuration for camera connection.
///
/// This struct holds all configuration needed to establish a camera connection
/// but performs no I/O itself. Configuration can be built, cloned, and reused.
///
/// # Example
///
/// ```ignore
/// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
///
/// let config = CameraConfig::for::<PtzOpticsG2>()
///     .address("192.168.0.110")
///     .auto_protocol()
///     .timeouts(TimeoutConfig::balanced());
///
/// // Configuration is pure data and can be cloned
/// let config2 = config.clone();
///
/// // Open connections using the configuration
/// let camera1 = config.open_async(&runtime).await?;
/// let camera2 = config2.open_blocking()?;
/// ```
#[derive(Debug, Clone)]
pub struct CameraConfig<P> {
    /// Transport configuration.
    pub(crate) transport: TransportOptions,
    /// Protocol configuration (explicit or auto-detect).
    pub(crate) protocol: ProtocolConfig,
    /// Command timeout configuration.
    pub(crate) timeouts: TimeoutConfig,
    /// Retry policy for failed commands.
    pub(crate) retries: RetryPolicy,
    /// Camera VISCA address (usually 1).
    pub(crate) camera_id: CameraId,
    /// Profile marker.
    pub(crate) _phantom: std::marker::PhantomData<P>,
}

impl<P> CameraConfig<P>
where
    P: crate::capabilities::Profile + Default,
{
    /// Create a new camera configuration for the specified profile.
    ///
    /// This initializes configuration with profile defaults.
    pub fn new() -> Self {
        Self {
            transport: TransportOptions::Tcp {
                address: format!("192.168.0.100:{}", P::DEFAULT_TCP_PORT),
            },
            protocol: ProtocolConfig::Explicit(P::PROTOCOL_STYLE),
            timeouts: TimeoutConfig::default(),
            retries: RetryPolicy::none(),
            camera_id: CameraId::new(P::DEFAULT_ADDRESS).unwrap_or_default(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Convenience constructor using the `for` naming convention.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = CameraConfig::for::<SonyFR7>()
    ///     .address("192.168.0.108");
    /// ```
    pub fn for_camera() -> Self {
        Self::new()
    }

    /// Set the connection address.
    ///
    /// For TCP/UDP, this should be a host:port string like "192.168.0.110:5678".
    /// If no port is specified, the profile's default port will be used.
    pub fn address(mut self, address: impl Into<String>) -> Self {
        let addr = address.into();

        // Parse address and add default port if needed
        let final_addr = if addr.contains(':') {
            addr
        } else {
            // Determine default port based on current transport type
            let default_port = match &self.transport {
                TransportOptions::Tcp { .. } => P::DEFAULT_TCP_PORT,
                TransportOptions::Udp { .. } => P::DEFAULT_UDP_PORT,
                _ => P::DEFAULT_TCP_PORT,
            };
            format!("{}:{}", addr, default_port)
        };

        // Update transport with new address, preserving transport type
        self.transport = match self.transport {
            TransportOptions::Tcp { .. } => TransportOptions::tcp(final_addr),
            TransportOptions::Udp { .. } => TransportOptions::udp(final_addr),
            other => other,
        };
        self
    }

    /// Use TCP transport.
    pub fn tcp(mut self) -> Self {
        if let TransportOptions::Tcp { address } | TransportOptions::Udp { address } =
            &self.transport
        {
            self.transport = TransportOptions::tcp(address.clone());
        } else {
            self.transport =
                TransportOptions::tcp(format!("192.168.0.100:{}", P::DEFAULT_TCP_PORT));
        }
        self
    }

    /// Use UDP transport.
    pub fn udp(mut self) -> Self {
        if let TransportOptions::Tcp { address } | TransportOptions::Udp { address } =
            &self.transport
        {
            self.transport = TransportOptions::udp(address.clone());
        } else {
            self.transport =
                TransportOptions::udp(format!("192.168.0.100:{}", P::DEFAULT_UDP_PORT));
        }
        self
    }

    /// Use serial transport.
    pub fn serial(mut self, port: impl Into<String>, baud_rate: u32) -> Self {
        self.transport = TransportOptions::serial(port, baud_rate);
        self
    }

    /// Set custom transport options.
    pub fn transport(mut self, transport: TransportOptions) -> Self {
        self.transport = transport;
        self
    }

    /// Use explicit protocol style.
    pub fn protocol(mut self, style: ProtocolStyle) -> Self {
        self.protocol = ProtocolConfig::Explicit(style);
        self
    }

    /// Enable automatic protocol detection.
    ///
    /// When enabled, the connection process will probe the camera to detect
    /// whether it uses Sony encapsulated or raw VISCA protocol.
    pub fn auto_protocol(mut self) -> Self {
        self.protocol = ProtocolConfig::Auto;
        self
    }

    /// Set timeout configuration.
    pub fn timeouts(mut self, timeouts: TimeoutConfig) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// Set retry policy.
    pub fn retries(mut self, retries: RetryPolicy) -> Self {
        self.retries = retries;
        self
    }

    /// Set camera VISCA address.
    pub fn camera_id(mut self, id: u8) -> Result<Self, Error> {
        self.camera_id = CameraId::new(id)?;
        Ok(self)
    }
}

impl<P> Default for CameraConfig<P>
where
    P: crate::capabilities::Profile + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

// Convenience alias for cleaner API
pub use CameraConfig as Config;

// Implementation of open methods
#[cfg(feature = "async")]
impl<P> CameraConfig<P>
where
    P: crate::capabilities::Profile + Default,
{
    /// Detect the protocol style by probing the camera.
    async fn detect_protocol_async<Profile, R>(&self, runtime: R) -> Result<ProtocolStyle, Error>
    where
        Profile: crate::capabilities::Profile + Default,
        R: crate::runtime_trait::Runtime,
    {
        use crate::runtime_trait::TransportHandle;
        use crate::transport::{builder::TransportConfig, AsyncTransport};
        use std::time::Duration;

        // Extract host from address
        let (host, _explicit_port) = match &self.transport {
            TransportOptions::Tcp { address } | TransportOptions::Udp { address } => {
                if let Some(colon_pos) = address.rfind(':') {
                    // Check if this is actually a port (not IPv6)
                    if address[colon_pos + 1..].parse::<u16>().is_ok() {
                        (
                            address[..colon_pos].to_string(),
                            Some(&address[colon_pos + 1..]),
                        )
                    } else {
                        (address.clone(), None)
                    }
                } else {
                    (address.clone(), None)
                }
            }
            _ => {
                return Err(Error::InvalidState(
                    "Auto-detection requires TCP or UDP transport".into(),
                ))
            }
        };

        // Detection candidates in priority order based on profile defaults
        let candidates = if Profile::PROTOCOL_STYLE == ProtocolStyle::SonyEncapsulated {
            vec![
                // Sony cameras first
                (
                    format!("{}:52381", &host),
                    true, // is_udp
                    ProtocolStyle::SonyEncapsulated,
                ),
                (
                    format!("{}:52381", &host),
                    false, // is_tcp
                    ProtocolStyle::SonyEncapsulated,
                ),
                // Then PTZOptics cameras
                (format!("{}:1259", &host), true, ProtocolStyle::RawVisca),
                (format!("{}:5678", &host), false, ProtocolStyle::RawVisca),
            ]
        } else {
            vec![
                // PTZOptics cameras first
                (format!("{}:1259", &host), true, ProtocolStyle::RawVisca),
                (format!("{}:5678", &host), false, ProtocolStyle::RawVisca),
                // Then Sony cameras
                (
                    format!("{}:52381", &host),
                    true,
                    ProtocolStyle::SonyEncapsulated,
                ),
                (
                    format!("{}:52381", &host),
                    false,
                    ProtocolStyle::SonyEncapsulated,
                ),
            ]
        };

        // Try each candidate
        for (address, is_udp, protocol_style) in candidates {
            // Try to connect
            let transport_result = if is_udp {
                R::connect_udp(&address, TransportConfig::default())
                    .await
                    .map(|t| TransportHandle::<R>::Udp(t, TransportConfig::default()))
            } else {
                R::connect_tcp(&address, TransportConfig::default())
                    .await
                    .map(|t| TransportHandle::<R>::Tcp(t, TransportConfig::default()))
            };

            if let Ok(mut transport) = transport_result {
                // Test with a simple inquiry command
                let test_command = &[0x81, 0x09, 0x00, 0x02, 0xFF]; // Version Inquiry

                // Frame the command according to protocol style
                let envelope = crate::transport::envelope::TransportEnvelope::new(protocol_style);
                let buffer_config = crate::transport::buffer::BufferConfig::default();
                let buffer_manager = crate::transport::buffer::BufferManager::new(buffer_config);
                let framed = envelope.frame_bytes_with_kind(
                    test_command,
                    crate::command::CommandKind::Inquiry,
                    &buffer_manager,
                );

                // Send command and check for response
                if transport.send(&framed).await.is_ok() {
                    // Try to receive with timeout
                    let recv_future = transport.recv();
                    let timeout_result = runtime
                        .timeout(Duration::from_millis(100), recv_future)
                        .await;

                    if let Ok(Ok(response)) = timeout_result {
                        // Check if response looks valid
                        if !response.is_empty() && self.is_valid_response(&response, protocol_style)
                        {
                            // Found working protocol style
                            return Ok(protocol_style);
                        }
                    }
                }
            }
        }

        Err(Error::ConnectionFailed {
            addr: host.into(),
            source: std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "No VISCA protocol response detected from camera - verify camera is powered on and address is correct",
            ),
        })
    }

    /// Check if a response is valid for the given protocol style.
    fn is_valid_response(&self, data: &[u8], protocol_style: ProtocolStyle) -> bool {
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

    /// Open an async camera session using the configuration.
    ///
    /// This method performs all I/O operations needed to establish a connection
    /// to the camera, including transport setup and protocol detection if configured.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The async runtime to use for I/O operations
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
    /// use grafton_visca::runtime_trait::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    /// let config = CameraConfig::for::<PtzOpticsG2>()
    ///     .address("192.168.0.110")
    ///     .auto_protocol();
    ///
    /// let session = config.open_async(runtime).await?;
    /// ```
    pub async fn open_async<R>(
        &self,
        runtime: R,
    ) -> Result<
        crate::camera::session::CameraSession<
            crate::mode::Async,
            P,
            crate::runtime_trait::TransportHandle<R>,
            R,
        >,
        Error,
    >
    where
        R: crate::runtime_trait::Runtime,
    {
        use crate::runtime_trait::TransportHandle;
        use crate::transport::builder::TransportConfig;

        // Create transport based on configuration
        let transport = match &self.transport {
            TransportOptions::Tcp { address } => {
                // Parse address and create TCP transport using Runtime trait
                let tcp = R::connect_tcp(address, TransportConfig::default()).await?;
                TransportHandle::Tcp(tcp, TransportConfig::default())
            }
            TransportOptions::Udp { address } => {
                // Parse address and create UDP transport using Runtime trait
                let udp = R::connect_udp(address, TransportConfig::default()).await?;
                TransportHandle::Udp(udp, TransportConfig::default())
            }
            TransportOptions::Serial { .. } => {
                return Err(Error::Unsupported);
            }
            TransportOptions::Custom => {
                return Err(Error::InvalidState(
                    "Custom transport requires manual session creation".into(),
                ));
            }
        };

        // Determine protocol style
        let protocol_style = match self.protocol {
            ProtocolConfig::Explicit(style) => style,
            ProtocolConfig::Auto => {
                // Perform protocol detection inline
                let detected_style = self.detect_protocol_async::<P, R>(runtime.clone()).await?;
                detected_style
            }
        };

        // Create camera with determined protocol style
        let mut camera =
            crate::camera::UnifiedCamera::<crate::mode::Async, P, _, _>::new_async_with_style(
                transport,
                runtime.clone(),
                protocol_style,
            )
            .await?;

        // Apply configuration
        camera.set_timeout_config(self.timeouts);
        if self.camera_id.id() != P::DEFAULT_ADDRESS {
            camera.set_camera_id(self.camera_id);
        }

        // Wrap in session
        Ok(crate::camera::session::CameraSession::new(camera))
    }
}

#[cfg(not(feature = "async"))]
impl<P> CameraConfig<P>
where
    P: crate::capabilities::Profile + Default,
{
    /// Open a blocking camera session using the configuration.
    ///
    /// This method performs all I/O operations needed to establish a connection
    /// to the camera, including transport setup and protocol detection if configured.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
    ///
    /// let config = CameraConfig::for::<PtzOpticsG2>()
    ///     .address("192.168.0.110")
    ///     .auto_protocol();
    ///
    /// let session = config.open_blocking()?;
    /// ```
    pub fn open_blocking(
        &self,
    ) -> Result<
        crate::camera::session::CameraSession<
            crate::mode::Blocking,
            P,
            Box<dyn crate::transport::SyncTransport>,
            (),
        >,
        Error,
    > {
        use crate::transport::{
            blocking::{Tcp, Udp},
            SyncTransport,
        };

        // Create transport based on configuration
        let transport: Box<dyn SyncTransport> = match &self.transport {
            TransportOptions::Tcp { address } => {
                // Parse address and create TCP transport
                let tcp = Tcp::connect(address)?;
                Box::new(tcp)
            }
            TransportOptions::Udp { address } => {
                // Parse address and create UDP transport
                let udp = Udp::connect(address)?;
                Box::new(udp)
            }
            TransportOptions::Serial { .. } => {
                return Err(Error::Unsupported);
            }
            TransportOptions::Custom => {
                return Err(Error::InvalidState(
                    "Custom transport requires manual session creation".into(),
                ));
            }
        };

        // Determine protocol style
        let protocol_style = match self.protocol {
            ProtocolConfig::Explicit(style) => style,
            ProtocolConfig::Auto => {
                // Auto-detect protocol by probing the camera
                self.detect_protocol_blocking()?
            }
        };

        // Create camera with determined protocol style
        let mut camera =
            crate::camera::UnifiedCamera::<crate::mode::Blocking, P, _, ()>::new_blocking_with_style(
                transport,
                protocol_style,
            )?;

        // Apply configuration
        camera.set_timeout_config(self.timeouts);
        if self.camera_id.id() != P::DEFAULT_ADDRESS {
            camera.set_camera_id(self.camera_id);
        }

        // Wrap in session
        Ok(crate::camera::session::CameraSession::new(camera))
    }
}

// Extension methods for blocking auto-detection
#[cfg(not(feature = "async"))]
impl<P> CameraConfig<P>
where
    P: crate::capabilities::Profile + Default,
{
    /// Detect the protocol style by probing the camera.
    fn detect_protocol_blocking(&self) -> Result<ProtocolStyle, Error> {
        use crate::camera::builder::TransportType;
        use crate::transport::{
            blocking::{Tcp, Udp},
            SyncTransport,
        };
        use std::time::Duration;

        // Extract host from address (remove port if present)
        let host = match &self.transport {
            TransportOptions::Tcp { address } | TransportOptions::Udp { address } => {
                if let Some(colon_pos) = address.rfind(':') {
                    // Check if this is actually a port (not IPv6)
                    if address[colon_pos + 1..].parse::<u16>().is_ok() {
                        address[..colon_pos].to_string()
                    } else {
                        address.clone()
                    }
                } else {
                    address.clone()
                }
            }
            _ => {
                return Err(Error::InvalidState(
                    "Auto-detection requires TCP or UDP transport".into(),
                ))
            }
        };

        // Detection candidates in priority order based on profile defaults
        let candidates = if P::PROTOCOL_STYLE == ProtocolStyle::SonyEncapsulated {
            vec![
                // Sony cameras first
                (
                    format!("{}:52381", &host),
                    TransportType::Udp,
                    ProtocolStyle::SonyEncapsulated,
                ),
                (
                    format!("{}:52381", &host),
                    TransportType::Tcp,
                    ProtocolStyle::SonyEncapsulated,
                ),
                // Then PTZOptics cameras
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
            ]
        } else {
            vec![
                // PTZOptics cameras first
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
                // Then Sony cameras
                (
                    format!("{}:52381", &host),
                    TransportType::Udp,
                    ProtocolStyle::SonyEncapsulated,
                ),
                (
                    format!("{}:52381", &host),
                    TransportType::Tcp,
                    ProtocolStyle::SonyEncapsulated,
                ),
            ]
        };

        // Try each candidate
        for (address, transport_type, protocol_style) in candidates {
            // Try to connect
            let transport_result: Result<Box<dyn SyncTransport>, Error> = match transport_type {
                TransportType::Tcp => match Tcp::connect(&address) {
                    Ok(t) => Ok(Box::new(t)),
                    Err(_) => continue, // Try next candidate
                },
                TransportType::Udp => match Udp::connect(&address) {
                    Ok(t) => Ok(Box::new(t)),
                    Err(_) => continue, // Try next candidate
                },
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
                        if !response.is_empty() && self.is_valid_response(&response, protocol_style)
                        {
                            // Found working protocol style
                            return Ok(protocol_style);
                        }
                    }
                }
            }
        }

        Err(Error::ConnectionFailed {
            addr: host.into(),
            source: std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "No VISCA protocol response detected from camera - verify camera is powered on and address is correct",
            ),
        })
    }

    /// Check if a response is valid for the given protocol style.
    fn is_valid_response(&self, data: &[u8], protocol_style: ProtocolStyle) -> bool {
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
