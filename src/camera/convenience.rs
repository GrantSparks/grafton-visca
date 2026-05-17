//! One-liner convenience methods for quick camera setup.
//!
//! This module provides simple, one-line methods to quickly connect to cameras
//! with sensible defaults.

use crate::{
    capabilities::{Profile, SupportsTcp, SupportsUdp},
    error::Error,
};

#[cfg(any(feature = "mode-async", feature = "transport-serial"))]
use crate::camera::config::CameraConfig;
#[cfg(feature = "mode-async")]
use crate::camera::CameraSession;
#[cfg(any(
    all(not(feature = "mode-async"), feature = "transport-serial"),
    all(feature = "mode-async", feature = "transport-serial-tokio")
))]
use crate::capabilities::SupportsSerial;

/// Convenience methods for connecting to cameras with one-liner setup.
#[derive(Debug, Clone, Copy)]
pub struct Connect;

/// Initial builder for creating high-level camera connections.
#[derive(Debug, Clone, Copy)]
pub struct ConnectBuilder;

/// TCP-selected connection builder.
#[derive(Debug, Clone)]
pub struct TcpConnectBuilder {
    address: String,
    use_default_port: bool,
}

/// UDP-selected connection builder.
#[derive(Debug, Clone)]
pub struct UdpConnectBuilder {
    address: String,
    use_default_port: bool,
}

/// Serial-selected connection builder.
#[derive(Debug, Clone)]
#[cfg_attr(
    not(any(
        all(not(feature = "mode-async"), feature = "transport-serial"),
        all(feature = "mode-async", feature = "transport-serial-tokio")
    )),
    allow(dead_code)
)]
pub struct SerialConnectBuilder {
    port: String,
    baud_rate: u32,
}

#[cfg(feature = "mode-async")]
impl Connect {
    /// Open a TCP async camera connection.
    ///
    /// Connects to the camera using TCP with the profile's default protocol style.
    ///
    /// Note: The async runtime implementation handles adding the profile's
    /// default TCP port if no port is specified in the address.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Connect, profiles::PtzOpticsG2};
    /// use grafton_visca::runtime::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    ///
    /// // With explicit port
    /// let cam = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110:5678", runtime.clone()).await?;
    ///
    /// // Without port - runtime adds PtzOpticsG2's TCP default (5678)
    /// let cam = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
    /// ```
    pub async fn open_tcp_async<P, R>(
        addr: impl Into<String>,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + SupportsTcp + Default,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::tcp(addr).open_async(runtime).await
    }

    /// Open a UDP async camera connection.
    ///
    /// Connects to the camera using UDP with the profile's default protocol style.
    ///
    /// Note: The async runtime implementation handles adding the profile's
    /// default UDP port if no port is specified in the address.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Connect, profiles::GenericVisca};
    /// use grafton_visca::runtime::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    ///
    /// // With explicit port
    /// let cam = Connect::open_udp_async::<GenericVisca, _>("192.168.0.110:1259", runtime.clone()).await?;
    ///
    /// // Without port - runtime adds GenericVisca's UDP default (1259)
    /// let cam = Connect::open_udp_async::<GenericVisca, _>("192.168.0.110", runtime).await?;
    /// ```
    pub async fn open_udp_async<P, R>(
        addr: impl Into<String>,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + SupportsUdp + Default,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::udp(addr).open_async(runtime).await
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
    /// use grafton_visca::camera::{Connect, profiles::PtzOpticsG2};
    /// use grafton_visca::runtime::TokioRuntime;
    ///
    /// let runtime = TokioRuntime::from_current()?;
    /// let cam = Connect::open_serial_async::<PtzOpticsG2, _>("/dev/ttyUSB0", 9600, runtime).await?;
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
        P: Profile + SupportsSerial + Default,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        CameraConfig::<P>::serial(port, baud_rate)
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
    /// If no port is specified in the address, the profile's TCP default will be used.
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:5678` becomes `[2001:db8::1]:5678`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Connect, profiles::PtzOpticsG2};
    ///
    /// // With explicit port
    /// let cam = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
    ///
    /// // Without port - uses PtzOpticsG2's TCP default (5678)
    /// let cam = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
    ///
    /// // IPv6 with explicit port (canonicalized automatically)
    /// let cam = Connect::open_tcp_blocking::<PtzOpticsG2>("[::1]:5678")?;
    /// ```
    pub fn open_tcp_blocking<P>(
        addr: impl Into<String>,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + SupportsTcp + Default,
    {
        // Canonicalize the address (handles IPv6 bracketing and default port)
        let canonical_addr = crate::transport::address::canonicalize_endpoint(
            &addr.into(),
            Some(<P as SupportsTcp>::DEFAULT_TCP_PORT),
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
    /// If no port is specified in the address, the profile's UDP default will be used.
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:1259` becomes `[2001:db8::1]:1259`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use grafton_visca::camera::{Connect, profiles::GenericVisca};
    ///
    /// // With explicit port
    /// let cam = Connect::open_udp_blocking::<GenericVisca>("192.168.0.110:1259")?;
    ///
    /// // Without port - uses GenericVisca's UDP default (1259)
    /// let cam = Connect::open_udp_blocking::<GenericVisca>("192.168.0.110")?;
    ///
    /// // IPv6 with explicit port (canonicalized automatically)
    /// let cam = Connect::open_udp_blocking::<GenericVisca>("[::1]:1259")?;
    /// ```
    pub fn open_udp_blocking<P>(
        addr: impl Into<String>,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + SupportsUdp + Default,
    {
        // Canonicalize the address (handles IPv6 bracketing and default port)
        let canonical_addr = crate::transport::address::canonicalize_endpoint(
            &addr.into(),
            Some(<P as SupportsUdp>::DEFAULT_UDP_PORT),
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
    /// use grafton_visca::camera::{Connect, profiles::PtzOpticsG2};
    ///
    /// let cam = Connect::open_serial_blocking::<PtzOpticsG2>("/dev/ttyUSB0", 9600)?;
    /// cam.power().on()?;
    /// cam.close()?;
    /// ```
    #[cfg(feature = "transport-serial")]
    pub fn open_serial_blocking<P>(
        port: impl Into<String>,
        baud_rate: u32,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + SupportsSerial + Default,
    {
        CameraConfig::<P>::serial(port, baud_rate).open_serial_blocking()
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
    /// use grafton_visca::camera::{Connect, profiles::PtzOpticsG2};
    ///
    /// // Build a connection with default settings
    /// let cam = Connect::builder()
    ///     .tcp("192.168.0.10")
    ///     .with_default_port()
    ///     .open::<PtzOpticsG2>()
    ///     .await?;
    /// ```
    pub fn builder() -> ConnectBuilder {
        ConnectBuilder
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
    pub fn tcp(self, addr: impl Into<String>) -> TcpConnectBuilder {
        TcpConnectBuilder {
            address: addr.into(),
            use_default_port: false,
        }
    }

    /// Configure UDP transport.
    ///
    /// # Arguments
    /// * `addr` - The address to connect to. Can be:
    ///   - IP address: "192.168.0.10"
    ///   - IP with port: "192.168.0.10:1259"
    ///   - Hostname: "camera.local"
    ///   - Hostname with port: "camera.local:1259"
    pub fn udp(self, addr: impl Into<String>) -> UdpConnectBuilder {
        UdpConnectBuilder {
            address: addr.into(),
            use_default_port: false,
        }
    }

    /// Configure serial transport.
    ///
    /// # Arguments
    /// * `port` - The serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows)
    /// * `baud_rate` - The baud rate (typically 9600 or 38400 for VISCA)
    pub fn serial(self, port: impl Into<String>, baud_rate: u32) -> SerialConnectBuilder {
        SerialConnectBuilder {
            port: port.into(),
            baud_rate,
        }
    }
}

impl TcpConnectBuilder {
    /// Use the selected profile's default TCP port if no port was specified.
    pub fn with_default_port(mut self) -> Self {
        self.use_default_port = true;
        self
    }

    fn canonical_address<P>(&self) -> Result<String, Error>
    where
        P: SupportsTcp,
    {
        let default_port = self
            .use_default_port
            .then_some(<P as SupportsTcp>::DEFAULT_TCP_PORT);
        crate::transport::address::canonicalize_endpoint(&self.address, default_port)
    }

    /// Open a blocking TCP camera connection with the configured settings.
    #[cfg(not(feature = "mode-async"))]
    pub fn open<P>(
        self,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + SupportsTcp + Default,
    {
        let canonical_addr = self.canonical_address::<P>()?;
        let tcp = crate::transport::blocking::tcp::Tcp::connect(&canonical_addr)?;
        let transport = crate::transport::BlockingTransportHandle::Tcp(tcp);
        let camera = crate::camera::Camera::new_blocking(transport)?;
        Ok(crate::BlockingClient::from_camera(camera))
    }

    /// Open an async TCP camera connection with the configured settings.
    #[cfg(feature = "mode-async")]
    pub async fn open<P, R>(
        self,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + SupportsTcp + Default,
        R: crate::runtime::Runtime,
    {
        let canonical_addr = self.canonical_address::<P>()?;
        CameraConfig::<P>::tcp(canonical_addr)
            .open_async(runtime)
            .await
    }
}

impl UdpConnectBuilder {
    /// Use the selected profile's default UDP port if no port was specified.
    pub fn with_default_port(mut self) -> Self {
        self.use_default_port = true;
        self
    }

    fn canonical_address<P>(&self) -> Result<String, Error>
    where
        P: SupportsUdp,
    {
        let default_port = self
            .use_default_port
            .then_some(<P as SupportsUdp>::DEFAULT_UDP_PORT);
        crate::transport::address::canonicalize_endpoint(&self.address, default_port)
    }

    /// Open a blocking UDP camera connection with the configured settings.
    #[cfg(not(feature = "mode-async"))]
    pub fn open<P>(
        self,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + SupportsUdp + Default,
    {
        let canonical_addr = self.canonical_address::<P>()?;
        let udp = crate::transport::blocking::udp::Udp::connect(&canonical_addr)?;
        let transport = crate::transport::BlockingTransportHandle::Udp(udp);
        let camera = crate::camera::Camera::new_blocking(transport)?;
        Ok(crate::BlockingClient::from_camera(camera))
    }

    /// Open an async UDP camera connection with the configured settings.
    #[cfg(feature = "mode-async")]
    pub async fn open<P, R>(
        self,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + SupportsUdp + Default,
        R: crate::runtime::Runtime,
    {
        let canonical_addr = self.canonical_address::<P>()?;
        CameraConfig::<P>::udp(canonical_addr)
            .open_async(runtime)
            .await
    }
}

impl SerialConnectBuilder {
    /// Open a blocking serial camera connection with the configured settings.
    #[cfg(all(not(feature = "mode-async"), feature = "transport-serial"))]
    pub fn open<P>(
        self,
    ) -> Result<crate::BlockingClient<P, crate::transport::BlockingTransportHandle>, Error>
    where
        P: Profile + SupportsSerial + Default,
    {
        CameraConfig::<P>::serial(self.port, self.baud_rate).open_serial_blocking()
    }

    /// Open an async serial camera connection with the configured settings.
    #[cfg(all(feature = "mode-async", feature = "transport-serial-tokio"))]
    pub async fn open<P, R>(
        self,
        runtime: R,
    ) -> Result<CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>, Error>
    where
        P: Profile + SupportsSerial + Default,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        CameraConfig::<P>::serial(self.port, self.baud_rate)
            .open_serial_async(runtime)
            .await
    }
}
