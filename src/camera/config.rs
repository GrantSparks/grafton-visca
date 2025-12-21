//! Pure configuration for camera connection and behavior.
//!
//! This module provides a pure-data configuration struct that holds all the settings
//! needed to establish a camera connection. The configuration is separate from the
//! actual connection process, allowing for easy cloning, reuse, and modification.

use std::marker::PhantomData;

use crate::{camera_id::CameraId, error::Error, timeout::TimeoutConfig};

/// Transport configuration options.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum TransportOptions {
    /// TCP connection with address.
    #[cfg_attr(feature = "serde", serde(rename = "TCP"))]
    Tcp {
        /// Host:port string (e.g., "192.168.0.110:5678")
        address: String,
    },
    /// UDP connection with address.
    #[cfg_attr(feature = "serde", serde(rename = "UDP"))]
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
    pub(crate) _phantom: PhantomData<P>,
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
            _phantom: PhantomData,
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
    /// Unbracketed IPv6 addresses with ports (e.g., `2001:db8::1:5678`) are automatically
    /// canonicalized to the bracketed form (`[2001:db8::1]:5678`).
    ///
    /// Examples:
    /// - IPv4: `"192.168.1.1"`, `"192.168.1.1:5678"`
    /// - IPv6: `"::1"`, `"[::1]:5678"`, `"2001:db8::1"`, `"2001:db8::1:5678"`
    /// - Hostnames: `"localhost"`, `"camera.local:5678"`
    pub fn address(mut self, address: impl Into<String>) -> Self {
        let addr = address.into();

        // Determine default port based on transport type
        let default_port = match &self.transport {
            TransportOptions::Tcp { .. } => P::DEFAULT_TCP_PORT,
            TransportOptions::Udp { .. } => P::DEFAULT_UDP_PORT,
            _ => P::DEFAULT_TCP_PORT,
        };

        // Use centralized canonicalization (handles IPv6 bracketing and default port)
        let final_addr =
            match crate::transport::address::canonicalize_endpoint(&addr, Some(default_port)) {
                Ok(canonical) => canonical,
                Err(_) => {
                    // If parsing fails, fall back to old behavior for compatibility
                    // This handles edge cases where the input might not be a standard address
                    if addr.contains(':') {
                        addr
                    } else {
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

        // Create transport config with retry settings from camera config
        let transport_config = TransportConfig {
            retry_config: self.retries,
            ..TransportConfig::default()
        };

        // Create transport based on configuration
        let transport = match &self.transport {
            TransportOptions::Tcp { address } => {
                // Canonicalize address at the connection boundary (handles IPv6 bracketing)
                let canonical_addr = crate::transport::address::canonicalize_endpoint(
                    address,
                    Some(P::DEFAULT_TCP_PORT),
                )?;
                let tcp = runtime
                    .connect_tcp(&canonical_addr, transport_config)
                    .await?;
                TransportHandle::Tcp(tcp)
            }
            TransportOptions::Udp { address } => {
                // Canonicalize address at the connection boundary (handles IPv6 bracketing)
                let canonical_addr = crate::transport::address::canonicalize_endpoint(
                    address,
                    Some(P::DEFAULT_UDP_PORT),
                )?;
                let udp = runtime
                    .connect_udp(&canonical_addr, transport_config)
                    .await?;
                TransportHandle::Udp(udp)
            }
            TransportOptions::Serial { .. } => {
                // Serial transport must use open_serial_async() method which has RuntimeSerial bound
                return Err(Error::InvalidState(
                    "Serial transport requires open_serial_async() method for proper trait bounds"
                        .into(),
                ));
            }
            TransportOptions::Custom => {
                return Err(Error::InvalidState(
                    "Custom transport requires manual session creation".into(),
                ));
            }
        };

        // Create camera using profile's envelope type with explicit timeout and retry configs
        // This ensures the runtime uses the same configs as configured in CameraConfig
        let mut camera =
            crate::camera::Camera::<crate::mode::Async, P, _, _>::new_async_with_config(
                transport,
                runtime.clone(),
                self.timeouts,
                self.retries,
            )
            .await?;

        // Apply camera ID if different from profile default
        if self.camera_id.id() != P::DEFAULT_CAMERA_ID {
            camera.set_camera_id(self.camera_id);
        }

        // Wrap in session
        Ok(crate::camera::session::CameraSession::new(camera))
    }

    /// Open an async serial camera session using the configuration.
    ///
    /// This method requires a runtime that implements `RuntimeSerial` and returns
    /// a session using the unified `TransportHandle` type, allowing downstream users
    /// to implement traits uniformly across all transport types.
    ///
    /// Currently, serial transport is only supported for the Tokio runtime.
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
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial_async<R>(
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
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        use crate::runtime::TransportHandle;

        match &self.transport {
            TransportOptions::Serial { port, baud_rate } => {
                // Create serial config from transport options
                let serial_config = crate::transport::serial::Config::new(port.clone())
                    .baud_rate(*baud_rate)
                    .camera_address(self.camera_id.id());

                // Connect using RuntimeSerial trait
                let serial = runtime.connect_serial(serial_config).await?;
                let transport = TransportHandle::Serial(Box::new(serial));

                // Create camera using profile's envelope type with explicit timeout and retry configs
                // This ensures the runtime uses the same configs as configured in CameraConfig
                let mut camera =
                    crate::camera::Camera::<crate::mode::Async, P, _, _>::new_async_with_config(
                        transport,
                        runtime.clone(),
                        self.timeouts,
                        self.retries,
                    )
                    .await?;

                // Apply camera ID if different from profile default
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
                // Canonicalize address at the connection boundary (handles IPv6 bracketing)
                let canonical_addr = crate::transport::address::canonicalize_endpoint(
                    address,
                    Some(P::DEFAULT_TCP_PORT),
                )?;
                let tcp = Tcp::connect(&canonical_addr)?;
                crate::transport::BlockingTransportHandle::Tcp(tcp)
            }
            TransportOptions::Udp { address } => {
                // Canonicalize address at the connection boundary (handles IPv6 bracketing)
                let canonical_addr = crate::transport::address::canonicalize_endpoint(
                    address,
                    Some(P::DEFAULT_UDP_PORT),
                )?;
                let udp = Udp::connect(&canonical_addr)?;
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

        // Create camera using profile's envelope type with explicit timeout and retry configs
        // This ensures the blocking runner uses the same configs as configured in CameraConfig
        let mut camera =
            crate::camera::Camera::<crate::mode::Blocking, P, _, ()>::new_blocking_with_config(
                transport,
                self.timeouts,
                self.retries,
            )?;

        // Apply camera ID if different from profile default
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

                // Create camera using profile's envelope type with explicit timeout and retry configs
                // This ensures the blocking runner uses the same configs as configured in CameraConfig
                let mut camera =
                    crate::camera::Camera::<crate::mode::Blocking, P, _, _>::new_blocking_with_config(
                        transport,
                        self.timeouts,
                        self.retries,
                    )?;

                // Apply camera ID if different from profile default
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

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "serde")]
    fn test_transport_options_serialization() {
        use super::TransportOptions;

        // Test TCP serialization
        let tcp = TransportOptions::Tcp {
            address: "192.168.0.110:5678".to_string(),
        };
        if let Ok(json) = serde_json::to_value(&tcp) {
            assert_eq!(json["type"], "TCP");
            assert_eq!(json["address"], "192.168.0.110:5678");
            if let Ok(pretty) = serde_json::to_string_pretty(&tcp) {
                println!("TCP: {pretty}");
            }
        }

        // Test UDP serialization
        let udp = TransportOptions::Udp {
            address: "192.168.0.110:1259".to_string(),
        };
        if let Ok(json) = serde_json::to_value(&udp) {
            assert_eq!(json["type"], "UDP");
            assert_eq!(json["address"], "192.168.0.110:1259");
            if let Ok(pretty) = serde_json::to_string_pretty(&udp) {
                println!("UDP: {pretty}");
            }
        }

        // Test Serial serialization
        let serial = TransportOptions::Serial {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
        };
        if let Ok(json) = serde_json::to_value(&serial) {
            assert_eq!(json["type"], "Serial");
            assert_eq!(json["port"], "/dev/ttyUSB0");
            assert_eq!(json["baud_rate"], 9600);
            if let Ok(pretty) = serde_json::to_string_pretty(&serial) {
                println!("Serial: {pretty}");
            }
        }
    }
}
