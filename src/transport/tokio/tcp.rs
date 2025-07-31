//! Tokio TCP transport implementation using GAT.

use crate::transport::core::Transport;
use crate::Error;
use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// Future type for TCP send operations.
#[derive(Debug)]
pub struct TcpSendFut<'a> {
    stream: &'a Arc<Mutex<TcpStream>>,
    data: &'a [u8],
}

impl Future for TcpSendFut<'_> {
    type Output = Result<(), Error>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        let fut = async {
            let mut stream = this.stream.lock().await;
            stream.write_all(this.data).await?;
            stream.flush().await?;
            Ok(())
        };
        // Create a pinned future and poll it
        let mut pinned = Box::pin(fut);
        Future::poll(Pin::new(&mut pinned), cx)
    }
}

/// Future type for TCP receive operations.
#[derive(Debug)]
pub struct TcpRecvFut<'a> {
    stream: &'a Arc<Mutex<TcpStream>>,
}

impl Future for TcpRecvFut<'_> {
    type Output = Result<bytes::Bytes, Error>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let stream = self.get_mut().stream;
        let fut = async {
            let mut stream = stream.lock().await;
            let mut buffer = vec![0u8; 1024];
            let mut total_read = 0;

            // Read until we find a VISCA terminator (0xFF)
            loop {
                if total_read >= buffer.len() {
                    return Err(Error::TransportError(Cow::Borrowed("Response too large")));
                }

                match stream.read(&mut buffer[total_read..total_read + 1]).await {
                    Ok(0) => return Err(Error::TransportError(Cow::Borrowed("Connection closed"))),
                    Ok(1) => {
                        total_read += 1;
                        if buffer[total_read - 1] == 0xFF {
                            // Found terminator
                            buffer.truncate(total_read);
                            return Ok(bytes::Bytes::from(buffer));
                        }
                    }
                    Ok(_) => unreachable!(),
                    Err(e) => return Err(e.into()),
                }
            }
        };
        // Create a pinned future and poll it
        let mut pinned = Box::pin(fut);
        Future::poll(Pin::new(&mut pinned), cx)
    }
}

/// TCP transport for async VISCA communication using tokio.
#[derive(Debug)]
pub struct Tcp {
    stream: Arc<Mutex<TcpStream>>,
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

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
        })
    }
}

impl Transport for Tcp {
    type Error = Error;
    type SendFut<'a> = TcpSendFut<'a>;
    type RecvFut<'a> = TcpRecvFut<'a>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        TcpSendFut {
            stream: &self.stream,
            data,
        }
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        TcpRecvFut {
            stream: &self.stream,
        }
    }
}
