//! async-std UDP transport implementation with zero-cost async and IPv6 support.

use bytes::Bytes;
use async_std::net::UdpSocket;

use crate::transport::address::AddressResolver;
use crate::transport::buffer::{BufferConfig, BufferManager};
use crate::transport::retry::RetryExecutor;
use crate::transport::{builder::TransportConfig, AsyncTransport, RetryConfig};
use crate::Error;

/// UDP transport for async VISCA communication using async-std.
///
/// This transport uses native async functions without boxing and supports
/// both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Udp {
    socket: UdpSocket,
    retry_executor: RetryExecutor,
    buffer_manager: BufferManager,
}

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

    /// Get the current retry configuration.
    pub fn retry_config(&self) -> &RetryConfig {
        self.retry_executor.config()
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    pub async fn connect_with_config(
        address: &str,
        config: TransportConfig,
    ) -> Result<Self, Error> {
        // Use the common address resolver
        let resolver = AddressResolver::new();
        let target_addr = resolver.resolve_first(address)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = resolver.bind_address_for(&target_addr);

        let socket = UdpSocket::bind(bind_addr).await?;
        socket.connect(target_addr).await?;

        // Apply socket options
        if let Some(ttl) = config.ttl {
            socket.set_ttl(ttl)?;
        }

        Ok(Self {
            socket,
            retry_executor: RetryExecutor::new(config.retry_config),
            buffer_manager: BufferManager::new(config.buffer_config),
        })
    }
}

#[allow(clippy::manual_async_fn)]
impl AsyncTransport for Udp {
    fn send(&mut self, data: &[u8]) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            // Note: Retry logic for async UDP would require more complex refactoring
            // For now, send directly without retry
            self.socket.send(data).await?;
            Ok(())
        }
    }

    fn recv(&mut self) -> impl std::future::Future<Output = Result<Bytes, Error>> + Send {
        async move {
            // Receiving is typically not retried to avoid protocol confusion
            let mut buffer = self.buffer_manager.alloc_vec_buffer();
            let n = self.socket.recv(&mut buffer).await?;
            Ok(self.buffer_manager.process_recv_data(&mut buffer, n))
        }
    }
}