//! Common utilities for examples using the new clean transport API.
//!
//! This module provides simple helper functions for creating transports
//! that examples can use consistently.

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
    use grafton_visca::transport::blocking::{
        create, TcpTransport as BlockingTcp, UdpTransport as BlockingUdp, ViscaTransport,
    };

    /// Blocking transport types
    pub type TcpTransport = BlockingTcp;
    pub type UdpTransport = BlockingUdp;

    /// Create a TCP transport for blocking examples
    pub fn tcp_transport(address: &str) -> std::io::Result<ViscaTransport<TcpTransport>> {
        create::tcp(address)
    }

    /// Create a UDP transport for blocking examples  
    pub fn udp_transport(address: &str) -> std::io::Result<ViscaTransport<UdpTransport>> {
        create::udp(address)
    }
}
