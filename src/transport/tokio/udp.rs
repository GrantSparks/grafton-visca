//! Tokio UDP transport implementation using GAT.

use crate::transport::core::Transport;
use crate::transport::UnifiedTransport;
use crate::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

/// Future type for UDP send operations.
#[derive(Debug)]
pub struct UdpSendFut<'a> {
    socket: &'a Arc<UdpSocket>,
    data: &'a [u8],
}

impl Future for UdpSendFut<'_> {
    type Output = Result<(), Error>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        let fut = async {
            this.socket.send(this.data).await?;
            Ok(())
        };
        // Create a pinned future and poll it
        let mut pinned = Box::pin(fut);
        Future::poll(Pin::new(&mut pinned), cx)
    }
}

/// Future type for UDP receive operations.
#[derive(Debug)]
pub struct UdpRecvFut<'a> {
    socket: &'a Arc<UdpSocket>,
}

impl Future for UdpRecvFut<'_> {
    type Output = Result<bytes::Bytes, Error>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let socket = self.get_mut().socket;
        let fut = async {
            let mut buffer = vec![0u8; 1024];
            let n = socket.recv(&mut buffer).await?;
            buffer.truncate(n);
            Ok(bytes::Bytes::from(buffer))
        };
        // Create a pinned future and poll it
        let mut pinned = Box::pin(fut);
        Future::poll(Pin::new(&mut pinned), cx)
    }
}

/// UDP transport for async VISCA communication using tokio.
#[derive(Debug)]
pub struct Udp {
    socket: Arc<UdpSocket>,
}

impl Udp {
    /// Connect to a UDP endpoint.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(address).await?;

        Ok(Self {
            socket: Arc::new(socket),
        })
    }
}

impl Transport for Udp {
    type Error = Error;
    type SendFut<'a> = UdpSendFut<'a>;
    type RecvFut<'a> = UdpRecvFut<'a>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        UdpSendFut {
            socket: &self.socket,
            data,
        }
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        UdpRecvFut {
            socket: &self.socket,
        }
    }
}

#[async_trait::async_trait]
impl UnifiedTransport for Udp {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.socket.send(bytes).await?;
        Ok(())
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        let mut buffer = vec![0u8; 1024];
        let n = self.socket.recv(&mut buffer).await?;
        buffer.truncate(n);
        Ok(bytes::Bytes::from(buffer))
    }

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        // Block on the async version
        futures::executor::block_on(UnifiedTransport::send(self, bytes))
    }

    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<bytes::Bytes, Error> {
        // Use tokio's block_on with timeout
        futures::executor::block_on(async {
            tokio::time::timeout(timeout, UnifiedTransport::recv(self))
                .await
                .map_err(|_| Error::Timeout)?
        })
    }
}
