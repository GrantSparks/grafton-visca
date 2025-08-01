//! Unified transport trait for abstracting over async and blocking transports.

use crate::error::Error;
use std::time::Duration;

/// Type-erased transport trait for unified camera.
///
/// This trait provides a unified interface for both async and blocking transports,
/// using async as the base and providing a blocking wrapper.
#[async_trait::async_trait]
pub trait UnifiedTransport: Send + Sync {
    /// Send raw bytes to the device.
    async fn send(&self, bytes: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device.
    async fn recv(&self) -> Result<bytes::Bytes, Error>;

    /// Send raw bytes to the device (blocking).
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device with timeout (blocking).
    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<bytes::Bytes, Error>;
}

/// Wrapper for the new async-trait based transports.
#[derive(Debug)]
pub struct NewAsyncTransportWrapper<T> {
    pub(crate) transport: T,
}

#[async_trait::async_trait]
impl<T> UnifiedTransport for NewAsyncTransportWrapper<T>
where
    T: crate::transport::Transport + Send + Sync,
{
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.transport.send(bytes).await
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        self.transport.recv().await
    }

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        // For async transports, use futures::executor to block
        futures::executor::block_on(self.send(bytes))
    }

    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<bytes::Bytes, Error> {
        // For async transports, use futures::executor with timeout
        #[cfg(feature = "tokio")]
        {
            futures::executor::block_on(async {
                tokio::time::timeout(timeout, self.recv())
                    .await
                    .map_err(|_| Error::Timeout)?
            })
        }

        #[cfg(not(feature = "tokio"))]
        {
            // Without tokio, use a simpler timeout approach
            log::warn!(
                "Timeout ({timeout:?}) not supported without tokio feature - blocking on recv"
            );
            futures::executor::block_on(self.recv())
        }
    }
}

/// Wrapper for blocking transports that implement both async and blocking traits.
#[derive(Debug)]
pub struct BlockingTransportWrapper<T> {
    pub(crate) transport: T,
}

#[async_trait::async_trait]
impl<T> UnifiedTransport for BlockingTransportWrapper<T>
where
    T: crate::transport::Transport + crate::transport::BlockingTransport + Send + Sync,
{
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.transport.send(bytes).await
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        self.transport.recv().await
    }

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        self.transport.send_blocking(bytes)
    }

    fn recv_blocking_timeout(&self, _timeout: Duration) -> Result<bytes::Bytes, Error> {
        // Blocking transports don't support timeout in recv_blocking
        self.transport.recv_blocking()
    }
}
