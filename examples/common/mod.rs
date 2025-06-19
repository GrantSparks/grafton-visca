//! Common utilities for examples using the new clean transport API.
//!
//! This module provides simple helper functions for creating transports
//! that examples can use consistently.

use std::time::Duration;

#[cfg(feature = "async-client")]
pub use grafton_visca::transport::{
    create, ChannelTransport, SerialTransport, TcpTransport, UdpTransport, ViscaTransport,
};

#[cfg(feature = "async-client")]
pub mod r#async {
    use super::*;

    /// Create a TCP transport for examples
    pub async fn tcp_transport(address: &str) -> std::io::Result<ViscaTransport<TcpTransport>> {
        create::tcp(address).await
    }

    /// Create a TCP transport with timeout for examples
    pub async fn tcp_transport_timeout(
        address: &str,
        timeout: Duration,
    ) -> std::io::Result<ViscaTransport<TcpTransport>> {
        create::tcp_timeout(address, timeout).await
    }

    /// Create a UDP transport for examples
    pub async fn udp_transport(address: &str) -> std::io::Result<ViscaTransport<UdpTransport>> {
        create::udp(address).await
    }

    /// Create a serial transport for examples
    pub fn serial_transport(camera_address: u8) -> ViscaTransport<SerialTransport> {
        create::serial(camera_address)
    }

    /// Create a channel transport for examples
    pub fn channel_transport<T: grafton_visca::transport::RawTransport + 'static>(
        transport: ViscaTransport<T>,
    ) -> ChannelTransport {
        create::channel(transport)
    }
}

#[cfg(feature = "blocking-client")]
pub mod blocking {
    use super::*;

    /// For blocking examples, we provide the same interface but note that
    /// the new API is async-first. Blocking examples should use the async
    /// transports with appropriate runtime handling.
    pub type TcpTransport = ViscaTransport<grafton_visca::transport::TcpTransport>;
    pub type UdpTransport = ViscaTransport<grafton_visca::transport::UdpTransport>;
}
