//! Pure configuration for camera connection and behavior.
//!
//! This module provides a pure-data configuration struct that holds all the settings
//! needed to establish a camera connection. The configuration is separate from the
//! actual connection process, allowing for easy cloning, reuse, and modification.

use crate::{camera_id::CameraId, error::Error, timeout::TimeoutConfig};

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

/// Pure configuration for camera connection.
///
/// This struct holds all configuration needed to establish a camera connection
/// but performs no I/O itself. Configuration can be built, cloned, and reused.
///
/// # Example
///
/// ```ignore
/// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
/// use grafton_visca::timeout::TimeoutConfig;
/// use grafton_visca::runtime::TokioRuntime;
///
/// let config = CameraConfig::<PtzOpticsG2>::new()
///     .address("192.168.0.110")
///     .timeouts(TimeoutConfig::balanced());
///
/// // Configuration is pure data and can be cloned
/// let config2 = config.clone();
///
/// // Open async connection using the configuration
/// let runtime = TokioRuntime::from_current()?;
/// let camera1 = config.open_async(runtime).await?;
///
/// // Open blocking connection using the configuration
/// let camera2 = config2.open_blocking()?;
///
/// // Use accessor-style API
/// camera1.power().on().await?;
/// camera2.power().on()?;
/// ```
#[derive(Debug, Clone)]
pub struct CameraConfig<P> {
    /// Transport configuration.
    pub(crate) transport: TransportOptions,
    /// Command timeout configuration.
    pub(crate) timeouts: TimeoutConfig,
    /// Retry configuration for failed commands.
    pub(crate) retries: crate::transport::RetryConfig,
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
            timeouts: TimeoutConfig::default(),
            retries: crate::transport::RetryConfig::default(),
            camera_id: CameraId::new(P::DEFAULT_CAMERA_ID).unwrap_or_default(),
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
    ///
    /// This method properly handles IPv6 addresses with and without brackets.
    /// Examples:
    /// - IPv4: `"192.168.1.1"`, `"192.168.1.1:5678"`
    /// - IPv6: `"::1"`, `"[::1]:5678"`, `"2001:db8::1"`
    /// - Hostnames: `"localhost"`, `"camera.local:5678"`
    pub fn address(mut self, address: impl Into<String>) -> Self {
        let addr = address.into();

        // Parse address using IPv6-safe parsing and add default port if needed
        let final_addr = match crate::transport::address::HostPort::parse(&addr) {
            Ok(parsed) => {
                if parsed.port().is_some() {
                    // Port already specified, use as-is
                    parsed.format_socket_addr(None)
                } else {
                    // No port specified, add default port based on transport type
                    let default_port = match &self.transport {
                        TransportOptions::Tcp { .. } => P::DEFAULT_TCP_PORT,
                        TransportOptions::Udp { .. } => P::DEFAULT_UDP_PORT,
                        _ => P::DEFAULT_TCP_PORT,
                    };
                    parsed.format_socket_addr(Some(default_port))
                }
            }
            Err(_) => {
                // If parsing fails, fall back to old behavior for compatibility
                // This handles edge cases where the input might not be a standard address
                if addr.contains(':') {
                    addr
                } else {
                    let default_port = match &self.transport {
                        TransportOptions::Tcp { .. } => P::DEFAULT_TCP_PORT,
                        TransportOptions::Udp { .. } => P::DEFAULT_UDP_PORT,
                        _ => P::DEFAULT_TCP_PORT,
                    };
                    format!("{addr}:{default_port}")
                }
            }
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

    /// Set timeout configuration.
    pub fn timeouts(mut self, timeouts: TimeoutConfig) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// Set retry configuration.
    pub fn retries(mut self, retries: crate::transport::RetryConfig) -> Self {
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
#[cfg(feature = "mode-async")]
impl<P> CameraConfig<P>
where
    P: crate::capabilities::Profile + Default,
{
    /// Open an async camera session using the configuration.
    ///
    /// This method performs all I/O operations needed to establish a connection
    /// to the camera, including transport setup.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The async runtime to use for I/O operations
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
    /// use grafton_visca::runtime::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    /// let config = CameraConfig::for::<PtzOpticsG2>()
    ///     .address("192.168.0.110");
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
            crate::runtime::TransportHandle<R>,
            R,
        >,
        Error,
    >
    where
        R: crate::runtime::Runtime,
    {
        use crate::runtime::TransportHandle;
        use crate::transport::builder::TransportConfig;

        // Create transport based on configuration
        let transport = match &self.transport {
            TransportOptions::Tcp { address } => {
                // Parse address and create TCP transport using Runtime trait
                let tcp = runtime
                    .connect_tcp(address, TransportConfig::default())
                    .await?;
                TransportHandle::Tcp(tcp)
            }
            TransportOptions::Udp { address } => {
                // Parse address and create UDP transport using Runtime trait
                let udp = runtime
                    .connect_udp(address, TransportConfig::default())
                    .await?;
                TransportHandle::Udp(udp)
            }
            TransportOptions::Serial { .. } => {
                // Serial requires serialport feature and RuntimeSerial implementation
                // Since we can't add the constraint here, we return Unsupported
                // Users should use the serial-specific methods like open_serial_async()
                return Err(Error::NotSupported);
            }
            TransportOptions::Custom => {
                return Err(Error::InvalidState(
                    "Custom transport requires manual session creation".into(),
                ));
            }
        };

        // Create camera using profile's envelope type
        let mut camera = crate::camera::Camera::<crate::mode::Async, P, _, _>::new_async(
            transport,
            runtime.clone(),
        )
        .await?;

        // Apply configuration
        camera.set_timeout_config(self.timeouts);
        if self.camera_id.id() != P::DEFAULT_CAMERA_ID {
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
    /// use grafton_visca::runtime::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    /// let config = CameraConfig::for::<PtzOpticsG2>()
    ///     .serial("/dev/ttyUSB0", 9600);
    ///
    /// let session = config.open_serial_async(runtime).await?;
    /// ```
    #[cfg(all(feature = "mode-async", feature = "transport-serial-tokio"))]
    pub async fn open_serial_async<R>(
        &self,
        runtime: R,
    ) -> Result<
        crate::camera::session::CameraSession<
            crate::mode::Async,
            P,
            <R as crate::runtime::RuntimeSerial>::SerialTransport,
            R,
        >,
        Error,
    >
    where
        R: crate::runtime::Runtime + crate::runtime::RuntimeSerial,
    {
        match &self.transport {
            TransportOptions::Serial { port, baud_rate } => {
                // Create serial config from transport options
                let serial_config = crate::transport::serial::Config::new(port.clone())
                    .baud_rate(*baud_rate)
                    .camera_address(self.camera_id.id());

                // Connect using RuntimeSerial trait
                let serial = runtime.connect_serial(serial_config).await?;

                // Create camera using profile's envelope type
                let mut camera = crate::camera::Camera::<crate::mode::Async, P, _, _>::new_async(
                    serial, runtime,
                )
                .await?;

                // Apply configuration
                camera.set_timeout_config(self.timeouts);
                if self.camera_id.id() != P::DEFAULT_CAMERA_ID {
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

#[cfg(not(feature = "mode-async"))]
impl<P> CameraConfig<P>
where
    P: crate::capabilities::Profile + Default,
{
    /// Open a blocking camera session using the configuration.
    ///
    /// This method performs all I/O operations needed to establish a connection
    /// to the camera, including transport setup.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
    ///
    /// let config = CameraConfig::for::<PtzOpticsG2>()
    ///     .address("192.168.0.110");
    ///
    /// let session = config.open_blocking()?;
    /// ```
    pub fn open_blocking(
        &self,
    ) -> Result<
        crate::camera::session::CameraSession<
            crate::mode::Blocking,
            P,
            crate::transport::BlockingTransportHandle,
            (),
        >,
        Error,
    > {
        use crate::transport::blocking::{Tcp, Udp};

        // Create transport based on configuration
        let transport = match &self.transport {
            TransportOptions::Tcp { address } => {
                // Parse address and add default port if needed
                let addr_with_port =
                    if let Ok(parsed) = crate::transport::address::HostPort::parse(address) {
                        if parsed.port().is_none() {
                            parsed.format_socket_addr(Some(P::DEFAULT_TCP_PORT))
                        } else {
                            address.clone()
                        }
                    } else {
                        address.clone()
                    };
                let tcp = Tcp::connect(&addr_with_port)?;
                crate::transport::BlockingTransportHandle::Tcp(tcp)
            }
            TransportOptions::Udp { address } => {
                // Parse address and add default port if needed
                let addr_with_port =
                    if let Ok(parsed) = crate::transport::address::HostPort::parse(address) {
                        if parsed.port().is_none() {
                            parsed.format_socket_addr(Some(P::DEFAULT_UDP_PORT))
                        } else {
                            address.clone()
                        }
                    } else {
                        address.clone()
                    };
                let udp = Udp::connect(&addr_with_port)?;
                crate::transport::BlockingTransportHandle::Udp(udp)
            }
            TransportOptions::Serial { .. } => {
                // Serial transport requires special handling due to it not being part of BlockingTransportHandle
                // This case should not be reached as serial should use open_serial_blocking() instead
                return Err(Error::InvalidState(
                    "Serial transport requires open_serial_blocking() method".into(),
                ));
            }
            TransportOptions::Custom => {
                return Err(Error::InvalidState(
                    "Custom transport requires manual session creation".into(),
                ));
            }
        };

        // Create camera using profile's envelope type
        let mut camera =
            crate::camera::Camera::<crate::mode::Blocking, P, _, ()>::new_blocking(transport)?;

        // Apply configuration
        camera.set_timeout_config(self.timeouts);
        if self.camera_id.id() != P::DEFAULT_CAMERA_ID {
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
    #[cfg(feature = "transport-serial")]
    pub fn open_serial_blocking(
        &self,
    ) -> Result<
        crate::camera::session::CameraSession<
            crate::mode::Blocking,
            P,
            crate::transport::BlockingTransportHandle,
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
                let serial_transport =
                    crate::transport::serial_blocking::SerialTransport::new(serial_config)?;
                let transport = crate::transport::BlockingTransportHandle::Serial(serial_transport);

                // Create camera using profile's envelope type
                let mut camera =
                    crate::camera::Camera::<crate::mode::Blocking, P, _, _>::new_blocking(
                        transport,
                    )?;

                // Apply configuration
                camera.set_timeout_config(self.timeouts);
                if self.camera_id.id() != P::DEFAULT_CAMERA_ID {
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
