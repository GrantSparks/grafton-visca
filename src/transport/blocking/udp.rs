//! Blocking UDP transport implementation with IPv6 support.

use crate::transport::BlockingTransport;
use crate::Error;
use bytes::Bytes;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::Mutex;
use std::time::Duration;

/// UDP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Udp {
    socket: Mutex<UdpSocket>,
}

impl Udp {
    /// Connect to a UDP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The socket will bind to the appropriate unspecified address based on the
    /// target address family.
    pub fn connect(address: &str) -> Result<Self, Error> {
        // Resolve the target address to determine address family
        let target_addr = Self::resolve_address(address)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = if target_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };

        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(target_addr)?;

        // Set timeouts
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self {
            socket: Mutex::new(socket),
        })
    }

    /// Resolve an address string to a SocketAddr.
    ///
    /// This handles DNS resolution and returns the first resolved address.
    fn resolve_address(address: &str) -> Result<SocketAddr, Error> {
        // Use ToSocketAddrs to resolve the address
        let addrs: Vec<SocketAddr> = address
            .to_socket_addrs()
            .map_err(|e| Error::InvalidAddress {
                reason: format!("Failed to resolve '{}': {}", address, e).into(),
            })?
            .collect();

        addrs
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidAddress {
                reason: format!("No addresses resolved for '{}'", address).into(),
            })
    }
}

impl BlockingTransport for Udp {
    fn send_blocking(&self, data: &[u8]) -> Result<(), Error> {
        let socket = self
            .socket
            .lock()
            .map_err(|_| Error::LockPoisoned("socket"))?;
        socket.send(data)?;
        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes, Error> {
        let socket = self
            .socket
            .lock()
            .map_err(|_| Error::LockPoisoned("socket"))?;
        let mut buffer = vec![0u8; 1024];

        match socket.recv(&mut buffer) {
            Ok(n) => {
                buffer.truncate(n);
                Ok(Bytes::from(buffer))
            }
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
        let mut buffer = vec![0u8; 1024];
        let result = socket.recv(&mut buffer);

        // Restore the original timeout
        socket.set_read_timeout(original_timeout)?;

        // Handle the result
        match result {
            Ok(n) => {
                buffer.truncate(n);
                Ok(Bytes::from(buffer))
            }
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
