//! One-liner convenience methods for quick camera setup.
//!
//! This module provides simple, one-line methods to quickly connect to cameras
//! with sensible defaults.

use crate::{capabilities::Profile, error::Error};

#[cfg(any(feature = "mode-async", feature = "transport-serial"))]
use crate::camera::config::CameraConfig;
#[cfg(feature = "mode-async")]
use crate::camera::CameraSession;

/// Convenience methods for connecting to cameras with one-liner setup.
#[derive(Debug, Clone, Copy)]
pub struct Connect;

/// Builder for creating camera connections with runtime-neutral configuration.
///
/// This builder provides a fluent API for configuring camera connections
/// without assuming any specific runtime, making it suitable for use with
/// tokio, smol, or blocking mode.
///
/// # Example
/// ```rust,ignore
/// use grafton_visca::Connect;
/// use grafton_visca::camera::profiles::PtzOpticsG2;
///
/// // Simple TCP connection with default port
/// let cam = Connect::builder()
///     .tcp("192.168.0.10")
///     .with_default_port()  // Uses PtzOpticsG2::DEFAULT_TCP_PORT (5678)
///     .open::<PtzOpticsG2>()
///     .await?;
///
/// // UDP with explicit port
/// let cam = Connect::builder()
///     .udp("192.168.0.10:1259")
///     .open::<GenericVisca>()
///     .await?;
/// ```
#[derive(Debug, Clone)]
pub struct ConnectBuilder {
    transport_type: Option<TransportType>,
    address: Option<String>,
    serial_port: Option<String>,
    baud_rate: Option<u32>,
    use_default_port: bool,
}

/// Transport type selection.
#[derive(Debug, Clone, Copy)]
enum TransportType {
    Tcp,
    Udp,
    Serial,
}

#[cfg(feature = "mode-async")]
impl Connect {
    /// Open a TCP async camera connection.
    ///
    /// Connects to the camera using TCP with the profile's default protocol style.
    ///
    /// Note: The async runtime implementation handles adding the profile's
    /// DEFAULT_TCP_PORT if no port is specified in the address.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    ///
    /// // With explicit port
    /// let cam = Camera::open_tcp_async::<PtzOpticsG2>("192.168.0.110:5678", &tokio_runtime).await?;
    ///
    /// // Without port - runtime adds PtzOpticsG2::DEFAULT_TCP_PORT (5678)
    /// let cam = Camera::open_tcp_async::<PtzOpticsG2>("192.168.0.110", &tokio_runtime).await?;
    /// ```
    pub async fn open_tcp_async<P, R>(
        addr: impl Into<String>,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::new()
            .tcp()
            .address(addr)
            .open_async(runtime)
            .await
    }

    /// Open a UDP async camera connection.
    ///
    /// Connects to the camera using UDP with the profile's default protocol style.
    ///
    /// Note: The async runtime implementation handles adding the profile's
    /// DEFAULT_UDP_PORT if no port is specified in the address.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::GenericVisca};
    ///
    /// // With explicit port
    /// let cam = Camera::open_udp_async::<GenericVisca>("192.168.0.110:1259", &tokio_runtime).await?;
    ///
    /// // Without port - runtime adds GenericVisca::DEFAULT_UDP_PORT (1259)
    /// let cam = Camera::open_udp_async::<GenericVisca>("192.168.0.110", &tokio_runtime).await?;
    /// ```
    pub async fn open_udp_async<P, R>(
        addr: impl Into<String>,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::new()
            .udp()
            .address(addr)
            .open_async(runtime)
            .await
    }

    /// Open a serial async camera connection.
    ///
    /// Connects to the camera using serial port with the profile's default protocol style.
    /// This method is generic over any runtime that implements `RuntimeSerial`.
    ///
    /// # Arguments
    ///
    /// * `port` - The serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows)
    /// * `baud_rate` - The baud rate (typically 9600 or 38400 for VISCA)
    /// * `runtime` - The async runtime (must implement RuntimeSerial trait)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    /// use grafton_visca::runtime::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    /// let cam = Camera::open_serial_async::<PtzOpticsG2, _>("/dev/ttyUSB0", 9600, runtime).await?;
    /// cam.power().on().await?;
    /// cam.close().await?;
    /// ```
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial_async<P, R>(
        port: impl Into<String>,
        baud_rate: u32,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        CameraConfig::<P>::new()
            .serial(port, baud_rate)
            .open_serial_async(runtime)
            .await
    }
}

#[cfg(not(feature = "mode-async"))]
impl Connect {
    /// Open a TCP blocking camera connection.
    ///
    /// Connects to the camera using TCP with the profile's default protocol style.
    /// Returns a camera using BlockingTransportHandle for zero-cost operation.
    ///
    /// If no port is specified in the address, the profile's DEFAULT_TCP_PORT will be used.
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:5678` becomes `[2001:db8::1]:5678`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    ///
    /// // With explicit port
    /// let cam = Camera::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
    ///
    /// // Without port - uses PtzOpticsG2::DEFAULT_TCP_PORT (5678)
    /// let cam = Camera::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
    ///
    /// // IPv6 with explicit port (canonicalized automatically)
    /// let cam = Camera::open_tcp_blocking::<PtzOpticsG2>("[::1]:5678")?;
    /// ```
    pub fn open_tcp_blocking<P>(
        addr: impl Into<String>,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        // Canonicalize the address (handles IPv6 bracketing and default port)
        let canonical_addr = crate::transport::address::canonicalize_endpoint(
            &addr.into(),
            Some(P::DEFAULT_TCP_PORT),
        )?;

        let tcp = crate::transport::blocking::tcp::Tcp::connect(&canonical_addr)?;
        let transport = crate::transport::BlockingTransportHandle::Tcp(tcp);
        let camera = crate::camera::Camera::new_blocking(transport)?;
        Ok(crate::BlockingClient::from_camera(camera))
    }

    /// Open a UDP blocking camera connection.
    ///
    /// Connects to the camera using UDP with the profile's default protocol style.
    /// Returns a camera using BlockingTransportHandle for zero-cost operation.
    ///
    /// If no port is specified in the address, the profile's DEFAULT_UDP_PORT will be used.
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:1259` becomes `[2001:db8::1]:1259`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::GenericVisca};
    ///
    /// // With explicit port
    /// let cam = Camera::open_udp_blocking::<GenericVisca>("192.168.0.110:1259")?;
    ///
    /// // Without port - uses GenericVisca::DEFAULT_UDP_PORT (1259)
    /// let cam = Camera::open_udp_blocking::<GenericVisca>("192.168.0.110")?;
    ///
    /// // IPv6 with explicit port (canonicalized automatically)
    /// let cam = Camera::open_udp_blocking::<GenericVisca>("[::1]:1259")?;
    /// ```
    pub fn open_udp_blocking<P>(
        addr: impl Into<String>,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        // Canonicalize the address (handles IPv6 bracketing and default port)
        let canonical_addr = crate::transport::address::canonicalize_endpoint(
            &addr.into(),
            Some(P::DEFAULT_UDP_PORT),
        )?;

        let udp = crate::transport::blocking::udp::Udp::connect(&canonical_addr)?;
        let transport = crate::transport::BlockingTransportHandle::Udp(udp);
        let camera = crate::camera::Camera::new_blocking(transport)?;
        Ok(crate::BlockingClient::from_camera(camera))
    }

    /// Open a serial blocking camera connection.
    ///
    /// Connects to the camera using serial port with the profile's default protocol style.
    ///
    /// # Arguments
    ///
    /// * `port` - The serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows)
    /// * `baud_rate` - The baud rate (typically 9600 or 38400 for VISCA)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Camera, profiles::PtzOpticsG2};
    ///
    /// let cam = Camera::open_serial_blocking::<PtzOpticsG2>("/dev/ttyUSB0", 9600)?;
    /// cam.power().on()?;
    /// cam.close()?;
    /// ```
    #[cfg(feature = "transport-serial")]
    pub fn open_serial_blocking<P>(
        port: impl Into<String>,
        baud_rate: u32,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        CameraConfig::<P>::new()
            .serial(port, baud_rate)
            .open_serial_blocking()
    }
}

// Builder implementation
impl Connect {
    /// Create a new connection builder for flexible camera setup.
    ///
    /// The builder provides a runtime-neutral API for configuring camera connections
    /// with support for default ports and multiple transport types.
    ///
    /// # Example
    /// ```rust,ignore
    /// use grafton_visca::Connect;
    /// use grafton_visca::camera::profiles::PtzOpticsG2;
    ///
    /// // Build a connection with default settings
    /// let cam = Connect::builder()
    ///     .tcp("192.168.0.10")
    ///     .with_default_port()
    ///     .open::<PtzOpticsG2>()
    ///     .await?;
    /// ```
    pub fn builder() -> ConnectBuilder {
        ConnectBuilder {
            transport_type: None,
            address: None,
            serial_port: None,
            baud_rate: None,
            use_default_port: false,
        }
    }
}

impl ConnectBuilder {
    /// Configure TCP transport.
    ///
    /// # Arguments
    /// * `addr` - The address to connect to. Can be:
    ///   - IP address: "192.168.0.10"
    ///   - IP with port: "192.168.0.10:5678"
    ///   - Hostname: "camera.local"
    ///   - Hostname with port: "camera.local:5678"
    pub fn tcp(mut self, addr: impl Into<String>) -> Self {
        self.transport_type = Some(TransportType::Tcp);
        self.address = Some(addr.into());
        self
    }

    /// Configure UDP transport.
    ///
    /// # Arguments
    /// * `addr` - The address to connect to. Can be:
    ///   - IP address: "192.168.0.10"
    ///   - IP with port: "192.168.0.10:1259"
    ///   - Hostname: "camera.local"
    ///   - Hostname with port: "camera.local:1259"
    pub fn udp(mut self, addr: impl Into<String>) -> Self {
        self.transport_type = Some(TransportType::Udp);
        self.address = Some(addr.into());
        self
    }

    /// Configure serial transport.
    ///
    /// # Arguments
    /// * `port` - The serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows)
    /// * `baud_rate` - The baud rate (typically 9600 or 38400 for VISCA)
    pub fn serial(mut self, port: impl Into<String>, baud_rate: u32) -> Self {
        self.transport_type = Some(TransportType::Serial);
        self.serial_port = Some(port.into());
        self.baud_rate = Some(baud_rate);
        self
    }

    /// Use the profile's default port if no port was specified in the address.
    ///
    /// This uses the camera profile's DEFAULT_TCP_PORT or DEFAULT_UDP_PORT
    /// constant depending on the transport type.
    ///
    /// # Example
    /// ```rust,ignore
    /// use grafton_visca::Connect;
    /// use grafton_visca::camera::profiles::PtzOpticsG2;
    ///
    /// // Will use PtzOpticsG2::DEFAULT_TCP_PORT (5678)
    /// let cam = Connect::builder()
    ///     .tcp("192.168.0.10")
    ///     .with_default_port()
    ///     .open::<PtzOpticsG2>()?;
    /// ```
    pub fn with_default_port(mut self) -> Self {
        self.use_default_port = true;
        self
    }

    /// Open a blocking camera connection with the configured settings.
    ///
    /// This method connects to the camera using the specified transport
    /// and profile, returning a blocking camera client.
    ///
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:5678` becomes `[2001:db8::1]:5678`).
    ///
    /// # Type Parameters
    /// * `P` - The camera profile to use
    ///
    /// # Errors
    /// Returns an error if:
    /// - No transport was configured
    /// - The connection fails
    /// - The address is invalid
    #[cfg(not(feature = "mode-async"))]
    pub fn open<P>(
        self,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + Default,
    {
        let transport_type = self.transport_type.ok_or_else(|| {
            Error::InvalidState(
                "No transport configured. Use .tcp(), .udp(), or .serial() first.".into(),
            )
        })?;

        match transport_type {
            TransportType::Tcp | TransportType::Udp => {
                let address = self
                    .address
                    .ok_or_else(|| Error::InvalidState("No address configured".into()))?;

                // Determine default port based on transport type and use_default_port flag
                let default_port = if self.use_default_port {
                    Some(match transport_type {
                        TransportType::Tcp => P::DEFAULT_TCP_PORT,
                        TransportType::Udp => P::DEFAULT_UDP_PORT,
                        _ => unreachable!(),
                    })
                } else {
                    None
                };

                // Canonicalize the address (handles IPv6 bracketing and default port)
                let canonical_addr =
                    crate::transport::address::canonicalize_endpoint(&address, default_port)?;

                let transport = match transport_type {
                    TransportType::Tcp => {
                        let tcp = crate::transport::blocking::tcp::Tcp::connect(&canonical_addr)?;
                        crate::transport::BlockingTransportHandle::Tcp(tcp)
                    }
                    TransportType::Udp => {
                        let udp = crate::transport::blocking::udp::Udp::connect(&canonical_addr)?;
                        crate::transport::BlockingTransportHandle::Udp(udp)
                    }
                    _ => unreachable!(),
                };

                let camera = crate::camera::Camera::new_blocking(transport)?;
                Ok(crate::BlockingClient::from_camera(camera))
            }
            #[cfg(feature = "transport-serial")]
            TransportType::Serial => {
                let port = self
                    .serial_port
                    .ok_or_else(|| Error::InvalidState("No serial port configured".into()))?;
                let baud_rate = self
                    .baud_rate
                    .ok_or_else(|| Error::InvalidState("No baud rate configured".into()))?;

                CameraConfig::<P>::new()
                    .serial(port, baud_rate)
                    .open_serial_blocking()
            }
            #[cfg(not(feature = "transport-serial"))]
            TransportType::Serial => Err(Error::NotSupported),
        }
    }

    /// Open an async camera connection with the configured settings.
    ///
    /// This method connects to the camera using the specified transport,
    /// profile, and runtime, returning an async camera session.
    ///
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:5678` becomes `[2001:db8::1]:5678`).
    ///
    /// Note: For serial transport, the runtime must also implement `RuntimeSerial`.
    ///
    /// # Type Parameters
    /// * `P` - The camera profile to use
    /// * `R` - The runtime to use
    ///
    /// # Errors
    /// Returns an error if:
    /// - No transport was configured
    /// - The connection fails
    /// - The address is invalid
    /// - Serial transport is selected but runtime doesn't implement RuntimeSerial
    #[cfg(feature = "mode-async")]
    pub async fn open<P, R>(
        self,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime,
    {
        let transport_type = self.transport_type.ok_or_else(|| {
            Error::InvalidState(
                "No transport configured. Use .tcp(), .udp(), or .serial() first.".into(),
            )
        })?;

        match transport_type {
            TransportType::Tcp | TransportType::Udp => {
                let address = self
                    .address
                    .ok_or_else(|| Error::InvalidState("No address configured".into()))?;

                // Determine default port based on transport type and use_default_port flag
                let default_port = if self.use_default_port {
                    Some(match transport_type {
                        TransportType::Tcp => P::DEFAULT_TCP_PORT,
                        TransportType::Udp => P::DEFAULT_UDP_PORT,
                        _ => unreachable!(),
                    })
                } else {
                    None
                };

                // Canonicalize the address (handles IPv6 bracketing and default port)
                let canonical_addr =
                    crate::transport::address::canonicalize_endpoint(&address, default_port)?;

                // Use CameraConfig for consistency
                let config = match transport_type {
                    TransportType::Tcp => CameraConfig::<P>::new().tcp().address(canonical_addr),
                    TransportType::Udp => CameraConfig::<P>::new().udp().address(canonical_addr),
                    _ => unreachable!(),
                };

                config.open_async(runtime).await
            }
            TransportType::Serial => {
                // Serial transport requires RuntimeSerial trait which we can't enforce here
                // Users should use open_serial_async directly for serial connections
                Err(Error::InvalidState(
                    "Serial transport requires RuntimeSerial trait. Use open_serial_async() method instead.".into()
                ))
            }
        }
    }

    /// Open an async serial camera connection with the configured settings.
    ///
    /// This is a specialized method for serial connections that requires
    /// the runtime to implement `RuntimeSerial`.
    ///
    /// # Type Parameters
    /// * `P` - The camera profile to use
    /// * `R` - The runtime to use (must implement RuntimeSerial)
    ///
    /// # Errors
    /// Returns an error if:
    /// - Transport type is not Serial
    /// - Serial port or baud rate was not configured
    /// - The connection fails
    #[cfg(all(feature = "mode-async", feature = "transport-serial-tokio"))]
    pub async fn open_serial_async<P, R>(
        self,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + Default,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        if !matches!(self.transport_type, Some(TransportType::Serial)) {
            return Err(Error::InvalidState(
                "open_serial_async() requires serial transport. Use .serial() first.".into(),
            ));
        }

        let port = self
            .serial_port
            .ok_or_else(|| Error::InvalidState("No serial port configured".into()))?;
        let baud_rate = self
            .baud_rate
            .ok_or_else(|| Error::InvalidState("No baud rate configured".into()))?;

        CameraConfig::<P>::new()
            .serial(port, baud_rate)
            .open_serial_async(runtime)
            .await
    }
}
