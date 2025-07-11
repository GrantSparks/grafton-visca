//! Tokio TCP transport implementation using GAT.

use crate::transport::gat_transport::Transport;
use crate::Error;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use std::pin::Pin;
use std::future::Future;

/// TCP transport for async VISCA communication using tokio.
#[derive(Debug)]
pub struct TcpGat {
    stream: Arc<Mutex<TcpStream>>,
    _address: String,
}

impl TcpGat {
    /// Connect to a TCP endpoint.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5)).await
    }

    /// Connect with a custom timeout.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let stream = tokio::time::timeout(
            timeout,
            TcpStream::connect(address)
        ).await
        .map_err(|_| Error::Timeout)??;

        // Set TCP nodelay
        stream.set_nodelay(true)?;

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            _address: address.to_string(),
        })
    }
}

impl Transport for TcpGat {
    type Error = Error;
    type SendFut<'a> = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>;
    type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<bytes::Bytes, Self::Error>> + Send + 'a>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        Box::pin(async move {
            let mut stream = self.stream.lock().await;
            stream.write_all(data).await?;
            stream.flush().await?;
            Ok(())
        })
    }

    fn recv<'a>(&'a self) -> Self::RecvFut<'a> {
        Box::pin(async move {
            let mut stream = self.stream.lock().await;
            let mut buffer = vec![0u8; 1024];
            let mut total_read = 0;
            
            // Read until we find a VISCA terminator (0xFF)
            loop {
                if total_read >= buffer.len() {
                    return Err(Error::TransportError("Response too large".to_string()));
                }
                
                match stream.read(&mut buffer[total_read..total_read + 1]).await {
                    Ok(0) => return Err(Error::TransportError("Connection closed".to_string())),
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
        })
    }
}