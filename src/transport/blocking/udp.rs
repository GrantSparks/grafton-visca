//! Blocking UDP transport implementation with IPv6 support.

use bytes::Bytes;
use std::{net::UdpSocket, time::Duration};

use crate::{
    transport::{
        address::AddressResolver,
        buffer::{BufferConfig, BufferManager},
        builder::TransportConfig,
        retry::RetryExecutor,
        RetryConfig, SyncTransport,
    },
    Error,
};

/// UDP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
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
    pub fn connect(address: &str) -> Result<Self, Error> {
        let config = TransportConfig {
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            buffer_config: BufferConfig::for_udp(),
            ..Default::default()
        };
        Self::connect_with_config(address, config)
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        // Use the common address resolver
        let resolver = AddressResolver::new();
        let target_addr = resolver.resolve_first(address)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = resolver.bind_address_for(&target_addr);

        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(target_addr)?;

        // Apply socket options from config
        socket.set_read_timeout(Some(config.read_timeout))?;
        socket.set_write_timeout(Some(config.write_timeout))?;
        if let Some(ttl) = config.ttl {
            socket.set_ttl(ttl)?;
        }

        // Create buffer manager with config
        let buffer_manager = BufferManager::new(config.buffer_config);

        // Create retry executor with config
        let retry_executor = RetryExecutor::new(config.retry_config);

        Ok(Self {
            socket,
            retry_executor,
            buffer_manager,
        })
    }

    /// Set the retry configuration for this transport.
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_executor.set_config(config);
    }

    /// Get the current retry configuration.
    pub fn retry_config(&self) -> &RetryConfig {
        self.retry_executor.config()
    }

    /// Set the read timeout for receive operations.
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.socket.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set the write timeout for send operations.
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.socket.set_write_timeout(timeout)?;
        Ok(())
    }

    /// Set TTL (Time To Live) for packets.
    pub fn set_ttl(&mut self, ttl: u32) -> Result<(), Error> {
        self.socket.set_ttl(ttl)?;
        Ok(())
    }
}

impl SyncTransport for Udp {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Clone data for retry closure
        let data_vec = data.to_vec();

        self.retry_executor.execute(|| {
            self.socket.send(&data_vec)?;
            Ok(())
        })
    }

    fn recv(&mut self) -> Result<Bytes, Error> {
        let mut buffer = self.buffer_manager.alloc_vec_buffer();

        match self.socket.recv(&mut buffer) {
            Ok(n) => Ok(self.buffer_manager.process_recv_data(&mut buffer, n)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(Error::Timeout),
            Err(e) => Err(e.into()),
        }
    }

    fn recv_with_timeout(&mut self, duration: Duration) -> Result<Bytes, Error> {
        // Save the current timeout
        let original_timeout = self.socket.read_timeout()?;

        // Set the new timeout for this operation
        self.socket.set_read_timeout(Some(duration))?;

        // Perform the receive operation
        let mut buffer = self.buffer_manager.alloc_vec_buffer();
        let result = self.socket.recv(&mut buffer);

        // Restore the original timeout
        self.socket.set_read_timeout(original_timeout)?;

        // Handle the result
        match result {
            Ok(n) => Ok(self.buffer_manager.process_recv_data(&mut buffer, n)),
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Err(Error::Timeout)
            }
            Err(e) => Err(e.into()),
        }
    }
}
