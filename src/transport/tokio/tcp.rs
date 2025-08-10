//! Tokio TCP transport implementation using GAT.

use crate::transport::core::Transport;
use crate::transport::UnifiedTransport;
use crate::Error;
use std::borrow::Cow;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// TCP transport for async VISCA communication using tokio.
#[derive(Debug)]
pub struct Tcp {
    reader: Mutex<BufReader<OwnedReadHalf>>,
    writer: Mutex<OwnedWriteHalf>,
}

impl Tcp {
    /// Connect to a TCP endpoint.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5)).await
    }

    /// Connect with a custom timeout.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(address))
            .await
            .map_err(|_| Error::Timeout)??;

        // Set TCP nodelay
        stream.set_nodelay(true)?;

        // Split into read and write halves
        let (read_half, write_half) = stream.into_split();

        Ok(Self {
            reader: Mutex::new(BufReader::new(read_half)),
            writer: Mutex::new(write_half),
        })
    }
}

use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for TCP send operations.
pub struct TcpSendFut<'a> {
    fut: Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>,
}

impl<'a> std::future::Future for TcpSendFut<'a> {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.as_mut().poll(cx)
    }
}

impl<'a> std::fmt::Debug for TcpSendFut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpSendFut").finish()
    }
}

/// Future for TCP receive operations.
pub struct TcpRecvFut<'a> {
    fut: Pin<Box<dyn std::future::Future<Output = Result<bytes::Bytes, Error>> + Send + 'a>>,
}

impl<'a> std::future::Future for TcpRecvFut<'a> {
    type Output = Result<bytes::Bytes, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.as_mut().poll(cx)
    }
}

impl<'a> std::fmt::Debug for TcpRecvFut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpRecvFut").finish()
    }
}

impl Transport for Tcp {
    type Error = Error;
    type SendFut<'a> = TcpSendFut<'a>;
    type RecvFut<'a> = TcpRecvFut<'a>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        let fut = Box::pin(async move {
            let mut writer = self.writer.lock().await;
            writer.write_all(data).await?;
            writer.flush().await?;
            Ok(())
        });
        TcpSendFut { fut }
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        let fut = Box::pin(async move {
            let mut reader = self.reader.lock().await;
            let mut buf = Vec::with_capacity(64);
            
            // Use buffered read_until to find VISCA terminator
            let n = reader.read_until(0xFF, &mut buf).await?;
            
            if n == 0 {
                return Err(Error::ConnectionLost {
                    reason: Cow::Borrowed("peer closed connection"),
                });
            }
            
            Ok(bytes::Bytes::from(buf))
        });
        TcpRecvFut { fut }
    }
}

#[async_trait::async_trait]
impl UnifiedTransport for Tcp {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        let mut writer = self.writer.lock().await;
        writer.write_all(bytes).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        let mut reader = self.reader.lock().await;
        let mut buf = Vec::with_capacity(64);
        
        // Use buffered read_until to find VISCA terminator
        let n = reader.read_until(0xFF, &mut buf).await?;
        
        if n == 0 {
            return Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            });
        }
        
        Ok(bytes::Bytes::from(buf))
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
