//! smol runtime transport implementations.
//!
//! This module provides transport implementations for the smol runtime.

pub(crate) mod connectors;

/// TCP transport over smol.
pub(crate) mod tcp {
    /// TCP transport for async VISCA communication using smol.
    pub type Tcp =
        crate::transport::async_tcp::Tcp<crate::transport::smol::connectors::SmolTcpStream>;
}

/// UDP transport over smol.
pub(crate) mod udp {
    /// UDP transport for async VISCA communication using smol.
    pub type Udp = crate::transport::async_udp::Udp<smol::net::UdpSocket>;
}

pub use tcp::Tcp as TcpTransport;
pub use udp::Udp as UdpTransport;
