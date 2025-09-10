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
    /// Auto-detect transport and protocol.
    Auto {
        /// Host or host:port string (e.g., "192.168.0.110")
        address: String,
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

    /// Create auto-detect transport options.
    pub fn auto(address: impl Into<String>) -> Self {
        Self::Auto {
            address: address.into(),
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

        // Handle Auto transport option separately
        let (transport, protocol_style) = match &self.transport {
            TransportOptions::Auto { address } => {
                // Use the new unified auto_connect_and_detect function
                let (transport, detected_style) =
                    crate::transport::builder::auto_connect_and_detect(
                        address,
                        TransportConfig::default(),
                        &runtime,
                    )
                    .await?;
                (transport, detected_style)
            }
            _ => {
                // Create transport based on configuration
                let mut transport = match &self.transport {
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
                        // Serial requires serialport feature and RuntimeSerial implementation
                        // Since we can't add the constraint here, we return Unsupported
                        // Users should use the serial-specific methods like open_serial_async()
                        return Err(Error::Unsupported);
                    }
                    TransportOptions::Custom => {
                        return Err(Error::InvalidState(
                            "Custom transport requires manual session creation".into(),
                        ));
                    }
                    TransportOptions::Auto { .. } => {
                        unreachable!("Auto case handled above")
                    }
                };

                // Determine protocol style
                let protocol_style = match self.protocol {
                    ProtocolConfig::Explicit(style) => style,
                    ProtocolConfig::Auto => {
                        // Serial transport is handled separately via open_serial_async
                        // This path only handles TCP/UDP, so always use detection
                        // Use ProtocolDetector on TCP/UDP transports
                        use crate::transport::protocol_detection::ProtocolDetector;

                        let detector = ProtocolDetector::new();
                        let detection_result =
                            detector.detect_protocol(&mut transport, &runtime).await?;

                        detection_result.to_protocol_style().ok_or_else(|| {
                            Error::ConnectionFailed {
                                addr: "unknown".into(),
                                source: std::io::Error::other("Failed to detect protocol"),
                            }
                        })?
                    }
                };

                (transport, protocol_style)
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

    /// Open an async serial camera session using the configuration.
    ///
    /// This method is generic over any runtime that implements `RuntimeSerial`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
    /// use grafton_visca::runtime_trait::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    /// let config = CameraConfig::for::<PtzOpticsG2>()
    ///     .serial("/dev/ttyUSB0", 9600);
    ///
    /// let session = config.open_serial_async(runtime).await?;
    /// ```
    #[cfg(all(feature = "async", feature = "tokio-serial"))]
    pub async fn open_serial_async<R>(
        &self,
        runtime: R,
    ) -> Result<
        crate::camera::session::CameraSession<
            crate::mode::Async,
            P,
            <R as crate::runtime_trait::RuntimeSerial>::SerialTransport,
            R,
        >,
        Error,
    >
    where
        R: crate::runtime_trait::Runtime + crate::runtime_trait::RuntimeSerial,
    {
        match &self.transport {
            TransportOptions::Serial { port, baud_rate } => {
                // Create serial config from transport options
                let serial_config = crate::transport::serial::Config::new(port.clone())
                    .baud_rate(*baud_rate)
                    .camera_address(self.camera_id.id());

                // Connect using RuntimeSerial trait
                let serial = R::connect_serial(serial_config).await?;

                // Determine protocol style (serial always uses profile's default)
                let protocol_style = match self.protocol {
                    ProtocolConfig::Explicit(style) => style,
                    ProtocolConfig::Auto => P::PROTOCOL_STYLE,
                };

                // Create camera with determined protocol style
                let mut camera = crate::camera::UnifiedCamera::<crate::mode::Async, P, _, _>::new_async_with_style(
                    serial,
                    runtime,
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
            _ => Err(Error::InvalidState(
                "open_serial_async requires serial transport configuration".into(),
            )),
        }
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

        // Handle Auto transport option separately
        let (transport, protocol_style) = match &self.transport {
            TransportOptions::Auto { address } => {
                // Use the new unified auto_connect_and_detect_blocking function
                let (transport, detected_style) =
                    crate::transport::builder::auto_connect_and_detect_blocking(
                        address,
                        crate::transport::builder::TransportConfig::default(),
                    )?;
                (transport, detected_style)
            }
            _ => {
                // Track if we're using serial transport for protocol detection
                let is_serial = matches!(&self.transport, TransportOptions::Serial { .. });

                // Create transport based on configuration
                let mut transport: Box<dyn SyncTransport> = match &self.transport {
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
                    TransportOptions::Serial { port, baud_rate } => {
                        #[cfg(feature = "serialport")]
                        {
                            // Create serial config from transport options
                            let serial_config = crate::transport::serial::Config::new(port.clone())
                                .baud_rate(*baud_rate)
                                .camera_address(self.camera_id.id());

                            // Create blocking serial transport
                            let serial = crate::transport::serial_blocking::SerialTransport::new(
                                serial_config,
                            )?;
                            Box::new(serial)
                        }
                        #[cfg(not(feature = "serialport"))]
                        {
                            let _ = (port, baud_rate); // Silence unused warnings
                            return Err(Error::Unsupported);
                        }
                    }
                    TransportOptions::Custom => {
                        return Err(Error::InvalidState(
                            "Custom transport requires manual session creation".into(),
                        ));
                    }
                    TransportOptions::Auto { .. } => {
                        unreachable!("Auto case handled above")
                    }
                };

                // Determine protocol style
                let protocol_style = match self.protocol {
                    ProtocolConfig::Explicit(style) => style,
                    ProtocolConfig::Auto => {
                        // For serial, we can't do IP-style detection, so default to RawVisca
                        if is_serial {
                            // Serial uses Raw VISCA by default (or profile's default)
                            P::PROTOCOL_STYLE
                        } else {
                            // Use simplified detection for blocking mode TCP/UDP
                            // Try Sony encapsulated first, then raw VISCA
                            use crate::command::bytes::VISCA_TERMINATOR;
                            use crate::transport::envelope::TransportEnvelope;
                            use crate::transport::protocol_detection::ProtocolDetector;
                            use std::time::Duration;

                            let detector = ProtocolDetector::new();
                            let test_command = &[0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR];

                            // Try Sony encapsulated first
                            let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);
                            let buffer_config =
                                crate::transport::buffer::BufferConfig::for_sony_ip();
                            let buffer_manager =
                                crate::transport::buffer::BufferManager::new(buffer_config);
                            let framed = envelope.frame_bytes_with_kind(
                                test_command,
                                crate::command::CommandKind::Inquiry,
                                &buffer_manager,
                            );

                            if transport
                                .send_with_kind(&framed, crate::command::CommandKind::Inquiry)
                                .is_ok()
                            {
                                if let Ok(buffer) =
                                    transport.recv_with_timeout(Duration::from_millis(100))
                                {
                                    if let Ok(payload) = envelope.extract_response(&buffer) {
                                        if detector.is_valid_visca_response(&payload) {
                                            ProtocolStyle::SonyEncapsulated
                                        } else {
                                            // Try raw VISCA
                                            ProtocolStyle::RawVisca
                                        }
                                    } else {
                                        ProtocolStyle::RawVisca
                                    }
                                } else {
                                    ProtocolStyle::RawVisca
                                }
                            } else {
                                ProtocolStyle::RawVisca
                            }
                        }
                    }
                };

                (transport, protocol_style)
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

    /// Open a blocking serial camera session using the configuration.
    ///
    /// This method creates a blocking serial connection to the camera.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
    ///
    /// let config = CameraConfig::for::<PtzOpticsG2>()
    ///     .serial("/dev/ttyUSB0", 9600);
    ///
    /// let session = config.open_serial_blocking()?;
    /// ```
    #[cfg(feature = "serialport")]
    pub fn open_serial_blocking(
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
        match &self.transport {
            TransportOptions::Serial { port, baud_rate } => {
                // Create serial config from transport options
                let serial_config = crate::transport::serial::Config::new(port.clone())
                    .baud_rate(*baud_rate)
                    .camera_address(self.camera_id.id());

                // Create blocking serial transport
                let transport: Box<dyn crate::transport::SyncTransport> = Box::new(
                    crate::transport::serial_blocking::SerialTransport::new(serial_config)?,
                );

                // Determine protocol style (serial always uses profile's default)
                let protocol_style = match self.protocol {
                    ProtocolConfig::Explicit(style) => style,
                    ProtocolConfig::Auto => P::PROTOCOL_STYLE,
                };

                // Create camera with determined protocol style
                let mut camera = crate::camera::UnifiedCamera::<crate::mode::Blocking, P, _, _>::new_blocking_with_style(
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
            _ => Err(Error::InvalidState(
                "open_serial_blocking requires serial transport configuration".into(),
            )),
        }
    }
}
