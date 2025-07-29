//! Tokio UDP transport implementation using GAT.

use crate::transport::core::Transport;
use crate::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
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

impl Transport for Udp {
    type Error = Error;
    type SendFut<'a> = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>;
    type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<bytes::Bytes, Self::Error>> + Send + 'a>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        Box::pin(async move {
            self.socket.send(data).await?;
            Ok(())
        })
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        Box::pin(async move {
            let mut buffer = vec![0u8; 1024];
            let n = self.socket.recv(&mut buffer).await?;
            buffer.truncate(n);
            Ok(bytes::Bytes::from(buffer))
        })
    }
}
