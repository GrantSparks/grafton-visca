//! Blocking UDP transport implementation with IPv6 support.

use bytes::Bytes;

use std::net::UdpSocket;
use std::sync::Mutex;
use std::time::Duration;

use crate::transport::address::AddressResolver;
use crate::transport::buffer::{BufferConfig, BufferManager};
use crate::transport::retry::RetryExecutor;
use crate::transport::{BlockingTransport, RetryConfig};
use crate::Error;

/// UDP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Udp {
    socket: Mutex<UdpSocket>,
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
        // Use the common address resolver
        let resolver = AddressResolver::new();
        let target_addr = resolver.resolve_first(address)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = resolver.bind_address_for(&target_addr);

        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(target_addr)?;

        // Set timeouts
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self {
            socket: Mutex::new(socket),
            retry_executor: RetryExecutor::with_defaults(),
            buffer_manager: BufferManager::new(BufferConfig::for_udp()),
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
        let socket = self
            .socket
            .lock()
            .map_err(|_| Error::LockPoisoned("socket"))?;
        socket.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set the write timeout for send operations.
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        let socket = self
            .socket
            .lock()
            .map_err(|_| Error::LockPoisoned("socket"))?;
        socket.set_write_timeout(timeout)?;
        Ok(())
    }

    /// Set TTL (Time To Live) for packets.
    pub fn set_ttl(&mut self, ttl: u32) -> Result<(), Error> {
        let socket = self
            .socket
            .lock()
            .map_err(|_| Error::LockPoisoned("socket"))?;
        socket.set_ttl(ttl)?;
        Ok(())
    }
}

impl BlockingTransport for Udp {
    fn send_blocking(&self, data: &[u8]) -> Result<(), Error> {
        // Clone data for retry closure
        let data_vec = data.to_vec();

        self.retry_executor.execute(|| {
            let socket = self
                .socket
                .lock()
                .map_err(|_| Error::LockPoisoned("socket"))?;
            socket.send(&data_vec)?;
            Ok(())
        })
    }

    fn recv_blocking(&self) -> Result<Bytes, Error> {
        let socket = self
            .socket
            .lock()
            .map_err(|_| Error::LockPoisoned("socket"))?;
        let mut buffer = self.buffer_manager.alloc_vec_buffer();

        match socket.recv(&mut buffer) {
            Ok(n) => Ok(self.buffer_manager.process_recv_data(&mut buffer, n)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(Error::Timeout),
            Err(e) => Err(e.into()),
        }
    }

    fn recv_blocking_with_timeout(&self, duration: Duration) -> Result<Bytes, Error> {
        // Get the socket
        let socket = self
            .socket
            .lock()
            .map_err(|_| Error::LockPoisoned("socket"))?;

        // Save the current timeout
        let original_timeout = socket.read_timeout()?;

        // Set the new timeout for this operation
        socket.set_read_timeout(Some(duration))?;

        // Perform the receive operation
        let mut buffer = self.buffer_manager.alloc_vec_buffer();
        let result = socket.recv(&mut buffer);

        // Restore the original timeout
        socket.set_read_timeout(original_timeout)?;

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
