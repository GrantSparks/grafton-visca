//! TCP transport implementation for VISCA over IP.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::error::Error;

use super::BlockingTransport;

/// TCP transport for VISCA over IP communication.
#[derive(Debug)]
pub struct Tcp {
    stream: TcpStream,
    remote_addr: SocketAddr,
}

impl Tcp {
    /// Create a new TCP transport connected to the given address.
    pub fn connect<A: ToSocketAddrs>(address: A) -> io::Result<Self> {
        let addrs: Vec<_> = address.to_socket_addrs()?.collect();
        if addrs.is_empty() {
            return Err(io::Error::new(ErrorKind::InvalidInput, "Invalid address"));
        }

        // Try each address until one succeeds
        let mut last_error = None;
        for addr in addrs {
            match TcpStream::connect(addr) {
                Ok(stream) => {
                    // Set reasonable default timeouts
                    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

                    return Ok(Self {
                        stream,
                        remote_addr: addr,
                    });
                }
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error
            .unwrap_or_else(|| io::Error::new(ErrorKind::NotConnected, "Failed to connect")))
    }

    /// Create a TCP transport with custom connection timeout.
    pub fn connect_timeout<A: ToSocketAddrs>(address: A, timeout: Duration) -> io::Result<Self> {
        let addrs: Vec<_> = address.to_socket_addrs()?.collect();
        if addrs.is_empty() {
            return Err(io::Error::new(ErrorKind::InvalidInput, "Invalid address"));
        }

        // Try each address until one succeeds
        let mut last_error = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, timeout) {
                Ok(stream) => {
                    // Set reasonable default timeouts for read/write
                    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

                    return Ok(Self {
                        stream,
                        remote_addr: addr,
                    });
                }
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error
            .unwrap_or_else(|| io::Error::new(ErrorKind::NotConnected, "Failed to connect")))
    }

    /// Set read timeout for the socket.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    /// Set write timeout for the socket.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }

    /// Set TCP nodelay option.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.stream.set_nodelay(nodelay)
    }
}

impl BlockingTransport for Tcp {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        log::debug!("TCP sending {} bytes to {}", data.len(), self.remote_addr);
        log::trace!("TCP TX: {:02X?}", data);

        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    fn receive(&mut self, timeout: Duration) -> Result<Vec<u8>, Error> {
        // Update timeout if different from current
        self.stream.set_read_timeout(Some(timeout))?;

        let mut buffer = vec![0u8; 1024];
        match self.stream.read(&mut buffer) {
            Ok(0) => {
                // Zero bytes read means connection closed
                Err(Error::Io(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "Connection closed",
                )))
            }
            Ok(len) => {
                buffer.truncate(len);
                log::debug!("TCP received {} bytes", len);
                log::trace!("TCP RX: {:02X?}", buffer);
                Ok(buffer)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                Err(Error::Timeout)
            }
            Err(e) => Err(e.into()),
        }
    }

    fn is_connected(&self) -> bool {
        // Check if we can get peer address (indicates connection is still valid)
        self.stream.peer_addr().is_ok()
    }

    fn description(&self) -> &str {
        "TCP Transport"
    }
}
