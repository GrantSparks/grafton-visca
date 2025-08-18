//! Tokio UDP transport implementation with zero-cost async and IPv6 support.

use bytes::Bytes;
use tokio::net::UdpSocket;

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::transport::AsyncTransport;
use crate::Error;

/// UDP transport for async VISCA communication using tokio.
///
/// This transport uses native async functions without boxing and supports
/// both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Udp {
    socket: Arc<UdpSocket>,
}

impl Udp {
    /// Connect to a UDP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The socket will bind to the appropriate unspecified address based on the
    /// target address family.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        // Resolve the target address to determine address family
        let target_addr = Self::resolve_address(address)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = if target_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };

        let socket = UdpSocket::bind(bind_addr).await?;
        socket.connect(target_addr).await?;

        Ok(Self {
            socket: Arc::new(socket),
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

impl AsyncTransport for Udp {
    async fn send(&self, data: &[u8]) -> Result<(), Error> {
        self.socket.send(data).await?;
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        let mut buffer = vec![0u8; 1024];
        let n = self.socket.recv(&mut buffer).await?;
        buffer.truncate(n);
        Ok(Bytes::from(buffer))
    }
}
