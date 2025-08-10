//! Tokio UDP transport implementation using GAT.

use crate::transport::core::Transport;
use crate::transport::UnifiedTransport;
use crate::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

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

use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for UDP send operations.
pub struct UdpSendFut<'a> {
    fut: Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>,
}

impl<'a> std::future::Future for UdpSendFut<'a> {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.as_mut().poll(cx)
    }
}

impl<'a> std::fmt::Debug for UdpSendFut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdpSendFut").finish()
    }
}

/// Future for UDP receive operations.
pub struct UdpRecvFut<'a> {
    fut: Pin<Box<dyn std::future::Future<Output = Result<bytes::Bytes, Error>> + Send + 'a>>,
}

impl<'a> std::future::Future for UdpRecvFut<'a> {
    type Output = Result<bytes::Bytes, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.as_mut().poll(cx)
    }
}

impl<'a> std::fmt::Debug for UdpRecvFut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdpRecvFut").finish()
    }
}

impl Transport for Udp {
    type Error = Error;
    type SendFut<'a> = UdpSendFut<'a>;
    type RecvFut<'a> = UdpRecvFut<'a>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        let fut = Box::pin(async move {
            self.socket.send(data).await?;
            Ok(())
        });
        UdpSendFut { fut }
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        let fut = Box::pin(async move {
            let mut buffer = vec![0u8; 1024];
            let n = self.socket.recv(&mut buffer).await?;
            buffer.truncate(n);
            Ok(bytes::Bytes::from(buffer))
        });
        UdpRecvFut { fut }
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
