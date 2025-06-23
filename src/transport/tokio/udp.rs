//! Clean UDP transport implementation for tokio.

use crate::error::Error;
use crate::transport::AsyncTransport;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// UDP transport implementation for tokio.
///
/// This is a clean implementation that works directly with AsyncTransport
/// without requiring adapters or complex trait hierarchies.
#[derive(Debug)]
pub struct Udp {
    socket: std::sync::Arc<UdpSocket>,
    remote_addr: SocketAddr,
    description: String,
}

impl Udp {
    /// Create a new UDP transport.
    ///
    /// Binds to a local address and sets the remote address for sending.
    pub async fn new(local_addr: &str, remote_addr: &str) -> Result<Self, Error> {
        let socket = UdpSocket::bind(local_addr).await?;
        let remote_addr: SocketAddr = remote_addr.parse().map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid remote address: {}", e),
            ))
        })?;

        Ok(Self {
            socket: std::sync::Arc::new(socket),
            remote_addr,
            description: format!("UDP {} -> {}", local_addr, remote_addr),
        })
    }

    /// Create a UDP transport that connects to a remote address.
    ///
    /// This binds to 0.0.0.0:0 (any available port) and connects to the remote address.
    pub async fn connect(remote_addr: &str) -> Result<Self, Error> {
        Self::new("0.0.0.0:0", remote_addr).await
    }

    /// Get a description of this transport.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Check if the socket is still connected.
    pub fn is_connected(&self) -> bool {
        // For UDP, we assume the socket is always "connected" since it's connectionless
        true
    }

    /// Get the local address this socket is bound to.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Get the remote address this socket is connected to.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

impl AsyncTransport for Udp {
    type SendFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
        Box::pin(async move {
            self.socket.send_to(data, self.remote_addr).await?;
            Ok(())
        })
    }

    fn receive(&self) -> Self::ReceiveFuture<'_> {
        Box::pin(async move {
            let mut buf = vec![0; 1024];
            let (n, _addr) = self.socket.recv_from(&mut buf).await?;
            buf.truncate(n);
            Ok(buf)
        })
    }
}
