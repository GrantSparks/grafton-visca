//! Tokio UDP transport implementation using the generic async_udp module.

use tokio::net::UdpSocket;

use crate::{
    transport::{
        async_io::UdpSocketConfig,
        buffer::BufferConfig,
        tokio::connectors::connect_udp,
        {builder::TransportConfig, RetryConfig},
    },
    Error,
};

/// UDP transport for async VISCA communication using tokio.
///
/// This is a type alias for the generic UDP transport specialized for tokio's UdpSocket.
pub type Udp = crate::transport::async_udp::Udp<UdpSocket>;

/// Helper methods for creating tokio UDP transports.
impl Udp {
    /// Connect to a UDP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The socket will bind to the appropriate unspecified address based on the
    /// target address family.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        let config = TransportConfig {
            buffer_config: BufferConfig::for_udp(),
            ..Default::default()
        };
        Self::connect_with_config(address, config).await
    }

    /// Create a new UDP transport with custom retry configuration.
    pub async fn connect_with_retry(
        address: &str,
        retry_config: RetryConfig,
    ) -> Result<Self, Error> {
        let config = TransportConfig {
            retry_config,
            buffer_config: BufferConfig::for_udp(),
            ..Default::default()
        };
        Self::connect_with_config(address, config).await
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    pub async fn connect_with_config(
        address: &str,
        config: TransportConfig,
    ) -> Result<Self, Error> {
        let udp_config = UdpSocketConfig::from(config);
        let socket = connect_udp(address, udp_config).await?;

        Ok(Self::new(socket, config))
    }
}
