//! Tokio transport implementations for VISCA communication.
//!
//! Tokio contributes only its I/O primitives; the TCP and UDP transports are
//! the generic implementations in `async_tcp` and `async_udp`.

pub(crate) mod connectors;
#[cfg(feature = "transport-serial-tokio")]
pub(crate) mod serial;

/// TCP transport over Tokio.
pub(crate) mod tcp {
    /// TCP transport for async VISCA communication using Tokio.
    pub type Tcp =
        crate::transport::async_tcp::Tcp<crate::transport::tokio::connectors::TokioTcpStream>;
}

/// UDP transport over Tokio.
pub(crate) mod udp {
    /// UDP transport for async VISCA communication using Tokio.
    pub type Udp = crate::transport::async_udp::Udp<tokio::net::UdpSocket>;
}

pub use tcp::Tcp as TcpTransport;
pub use udp::Udp as UdpTransport;

#[cfg(feature = "transport-serial-tokio")]
pub use serial::Serial as SerialTransport;
