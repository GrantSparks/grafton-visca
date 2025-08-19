//! Tokio UDP transport implementation with zero-cost async and IPv6 support.

use bytes::Bytes;
use tokio::net::UdpSocket;

use std::sync::Arc;

use crate::transport::address::AddressResolver;
use crate::transport::buffer::{BufferConfig, BufferManager};
use crate::transport::retry::RetryExecutor;
use crate::transport::{builder::TransportConfig, AsyncTransport, RetryConfig};
use crate::Error;

/// UDP transport for async VISCA communication using tokio.
///
/// This transport uses native async functions without boxing and supports
/// both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Udp {
    socket: Arc<UdpSocket>,
    retry_executor: Arc<RetryExecutor>,
    buffer_manager: Arc<BufferManager>,
}

impl Udp {
    /// Connect to a UDP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The socket will bind to the appropriate unspecified address based on the
    /// target address family.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        // Use the common address resolver
        let resolver = AddressResolver::new();
        let target_addr = resolver.resolve_first(address)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = resolver.bind_address_for(&target_addr);

        let socket = UdpSocket::bind(bind_addr).await?;
        socket.connect(target_addr).await?;

        Ok(Self {
            socket: Arc::new(socket),
            retry_executor: Arc::new(RetryExecutor::with_defaults()),
            buffer_manager: Arc::new(BufferManager::new(BufferConfig::for_udp())),
        })
    }

    /// Create a new UDP transport with custom retry configuration.
    pub async fn connect_with_retry(address: &str, config: RetryConfig) -> Result<Self, Error> {
        // Use the common address resolver
        let resolver = AddressResolver::new();
        let target_addr = resolver.resolve_first(address)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = resolver.bind_address_for(&target_addr);

        let socket = UdpSocket::bind(bind_addr).await?;
        socket.connect(target_addr).await?;

        Ok(Self {
            socket: Arc::new(socket),
            retry_executor: Arc::new(RetryExecutor::new(config)),
            buffer_manager: Arc::new(BufferManager::new(BufferConfig::for_udp())),
        })
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
            socket: Arc::new(socket),
            retry_executor: Arc::new(RetryExecutor::new(config.retry_config)),
            buffer_manager: Arc::new(BufferManager::new(BufferConfig::for_udp())),
        })
    }
}

impl AsyncTransport for Udp {
    async fn send(&self, data: &[u8]) -> Result<(), Error> {
        // Clone data and socket for retry closure
        let data_vec = data.to_vec();
        let socket = self.socket.clone();

        self.retry_executor
            .execute_async(|| {
                let socket = socket.clone();
                let data = data_vec.clone();
                async move {
                    socket.send(&data).await?;
                    Ok(())
                }
            })
            .await
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        // Receiving is typically not retried to avoid protocol confusion
        let mut buffer = self.buffer_manager.alloc_vec_buffer();
        let n = self.socket.recv(&mut buffer).await?;
        Ok(self.buffer_manager.process_recv_data(&mut buffer, n))
    }
}
