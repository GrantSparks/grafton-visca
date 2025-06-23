//! UDP transport implementation for VISCA over IP.

use std::io::{self, ErrorKind};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use crate::error::Error;

use super::BlockingTransport;

/// UDP transport for VISCA over IP communication.
#[derive(Debug)]
pub struct Udp {
    socket: UdpSocket,
    remote_addr: SocketAddr,
}

impl Udp {
    /// Create a new UDP transport connected to the given address.
    pub fn connect<A: ToSocketAddrs>(address: A) -> io::Result<Self> {
        let remote_addr = address
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "Invalid address"))?;

        // Bind to any available port
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(remote_addr)?;

        // Set reasonable default timeout
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self {
            socket,
            remote_addr,
        })
    }

    /// Create a UDP transport bound to a specific local address.
    pub fn bind<L: ToSocketAddrs, R: ToSocketAddrs>(
        local_addr: L,
        remote_addr: R,
    ) -> io::Result<Self> {
        let remote_addr = remote_addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "Invalid remote address"))?;

        let socket = UdpSocket::bind(local_addr)?;
        socket.connect(remote_addr)?;

        // Set reasonable default timeout
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self {
            socket,
            remote_addr,
        })
    }

    /// Set read timeout for the socket.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Set write timeout for the socket.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_write_timeout(timeout)
    }
}

impl BlockingTransport for Udp {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        log::debug!("UDP sending {} bytes to {}", data.len(), self.remote_addr);
        log::trace!("UDP TX: {:02X?}", data);

        self.socket.send(data)?;
        Ok(())
    }

    fn receive(&mut self, timeout: Duration) -> Result<Vec<u8>, Error> {
        // Update timeout if different from current
        self.socket.set_read_timeout(Some(timeout))?;

        let mut buffer = vec![0u8; 1024];
        match self.socket.recv(&mut buffer) {
            Ok(len) => {
                buffer.truncate(len);
                log::debug!("UDP received {} bytes", len);
                log::trace!("UDP RX: {:02X?}", buffer);
                Ok(buffer)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                Err(Error::Timeout)
            }
            Err(e) => Err(e.into()),
        }
    }

    fn is_connected(&self) -> bool {
        // UDP is "connectionless" but we can check if socket is valid
        self.socket.peer_addr().is_ok()
    }

    fn description(&self) -> &str {
        "UDP Transport"
    }
}
