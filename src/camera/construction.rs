//! Canonical owner-backed async construction.
//!
//! Standard connections resolve configuration before creating a transport and
//! end in [`crate::Session::open`].

#![cfg(feature = "async")]

#[cfg(feature = "transport-serial-tokio")]
use crate::capabilities::SupportsSerial;
use crate::{camera::CameraConfig, profile::CompileTimeProfile};

/// One-line canonical async connection constructor.
#[derive(Debug, Clone, Copy)]
pub struct Connect;

/// Runtime-neutral typed connection builder.
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

/// Tokio serial-selected connection builder.
#[derive(Debug, Clone)]
#[cfg(feature = "transport-serial-tokio")]
pub struct SerialConnectBuilder {
    port: String,
    baud_rate: u32,
}

impl Connect {
    /// Open one canonical owner-backed TCP session.
    pub async fn open_tcp<P, R>(
        address: impl Into<String>,
        runtime: R,
    ) -> crate::Result<crate::Session>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::tcp(address).open_async(runtime).await
    }

    /// Open one canonical owner-backed UDP session.
    pub async fn open_udp<P, R>(
        address: impl Into<String>,
        runtime: R,
    ) -> crate::Result<crate::Session>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::udp(address).open_async(runtime).await
    }

    /// Open one canonical owner-backed TCP session for a single camera.
    ///
    /// The profile is named once and bound at compile time: the returned
    /// [`CameraSession`](crate::CameraSession) owns the `P` camera view
    /// directly, with no second, runtime-checked profile naming.
    pub async fn open_tcp_camera<P, R>(
        address: impl Into<String>,
        runtime: R,
    ) -> crate::Result<crate::CameraSession<P>>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::tcp(address)
            .open_camera_async(runtime)
            .await
    }

    /// Open one canonical owner-backed UDP session for a single camera.
    ///
    /// The profile is named once and bound at compile time: the returned
    /// [`CameraSession`](crate::CameraSession) owns the `P` camera view
    /// directly, with no second, runtime-checked profile naming.
    pub async fn open_udp_camera<P, R>(
        address: impl Into<String>,
        runtime: R,
    ) -> crate::Result<crate::CameraSession<P>>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::udp(address)
            .open_camera_async(runtime)
            .await
    }

    /// Open one canonical owner-backed Tokio serial session.
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial<P, R>(
        port: impl Into<String>,
        baud_rate: u32,
        runtime: R,
    ) -> crate::Result<crate::Session>
    where
        P: CompileTimeProfile + SupportsSerial,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        CameraConfig::<P>::serial(port, baud_rate)
            .open_serial_async(runtime)
            .await
    }

    /// Open one canonical owner-backed Tokio serial session for a single
    /// camera.
    ///
    /// The profile is named once and bound at compile time: the returned
    /// [`CameraSession`](crate::CameraSession) owns the `P` camera view
    /// directly, with no second, runtime-checked profile naming.
    ///
    /// Async serial is Tokio-only, so this constructor carries the same
    /// [`RuntimeSerial`](crate::runtime::RuntimeSerial) bound as
    /// [`Self::open_serial`].
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial_camera<P, R>(
        port: impl Into<String>,
        baud_rate: u32,
        runtime: R,
    ) -> crate::Result<crate::CameraSession<P>>
    where
        P: CompileTimeProfile + SupportsSerial,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        CameraConfig::<P>::serial(port, baud_rate)
            .open_serial_camera_async(runtime)
            .await
    }

    /// Creates a typed standard transport builder.
    pub fn builder() -> ConnectBuilder {
        ConnectBuilder
    }
}

impl ConnectBuilder {
    /// Select TCP without applying a profile default port.
    pub fn tcp(self, address: impl Into<String>) -> TcpConnectBuilder {
        TcpConnectBuilder {
            address: address.into(),
            use_default_port: false,
        }
    }

    /// Select UDP without applying a profile default port.
    pub fn udp(self, address: impl Into<String>) -> UdpConnectBuilder {
        UdpConnectBuilder {
            address: address.into(),
            use_default_port: false,
        }
    }

    /// Select Tokio serial.
    #[cfg(feature = "transport-serial-tokio")]
    pub fn serial(self, port: impl Into<String>, baud_rate: u32) -> SerialConnectBuilder {
        SerialConnectBuilder {
            port: port.into(),
            baud_rate,
        }
    }
}

impl TcpConnectBuilder {
    /// Apply the selected profile's compile-time TCP default port.
    pub fn with_default_port(mut self) -> Self {
        self.use_default_port = true;
        self
    }

    /// Open the owner-backed TCP session.
    pub async fn open<P, R>(self, runtime: R) -> crate::Result<crate::Session>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        R: crate::runtime::Runtime,
    {
        let config = if self.use_default_port {
            CameraConfig::<P>::tcp(self.address)
        } else {
            CameraConfig::<P>::new()
                .transport(crate::camera::TransportOptions::tcp(self.address))
                .without_network_default_port()
        };
        config.open_async(runtime).await
    }
}

impl UdpConnectBuilder {
    /// Apply the selected profile's compile-time UDP default port.
    pub fn with_default_port(mut self) -> Self {
        self.use_default_port = true;
        self
    }

    /// Open the owner-backed UDP session.
    pub async fn open<P, R>(self, runtime: R) -> crate::Result<crate::Session>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        R: crate::runtime::Runtime,
    {
        let config = if self.use_default_port {
            CameraConfig::<P>::udp(self.address)
        } else {
            CameraConfig::<P>::new()
                .transport(crate::camera::TransportOptions::udp(self.address))
                .without_network_default_port()
        };
        config.open_async(runtime).await
    }
}

#[cfg(feature = "transport-serial-tokio")]
impl SerialConnectBuilder {
    /// Open the owner-backed Tokio serial session.
    pub async fn open<P, R>(self, runtime: R) -> crate::Result<crate::Session>
    where
        P: CompileTimeProfile + SupportsSerial,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        CameraConfig::<P>::serial(self.port, self.baud_rate)
            .open_serial_async(runtime)
            .await
    }
}
