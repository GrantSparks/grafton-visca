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

/// Wrapper for async transports.
#[derive(Debug)]
pub struct AsyncTransportWrapper<T: crate::transport::Transport> {
    pub(crate) transport: T,
}

#[async_trait::async_trait]
impl<T> UnifiedTransport for AsyncTransportWrapper<T>
where
    T: crate::transport::Transport + Send + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.transport.send(bytes).await.map_err(Into::into)
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        self.transport.recv().await.map_err(Into::into)
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
            use std::time::Instant;
            let _start = Instant::now();

            // For non-tokio async transports, we just block on recv without timeout
            // This is a limitation when not using tokio
            log::warn!(
                "Timeout ({:?}) not supported without tokio feature - blocking on recv",
                timeout
            );
            futures::executor::block_on(self.recv())
        }
    }
}
