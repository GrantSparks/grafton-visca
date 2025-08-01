//! Unified transport trait for abstracting over async and blocking transports.

use crate::error::Error;
use std::time::Duration;

#[cfg(feature = "async")]
use crate::executor::Sleep;

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
pub struct UnifiedTransportWrapper<T: crate::transport::Transport> {
    pub(crate) transport: T,
    #[cfg(feature = "async")]
    pub(crate) sleep_impl: Option<std::sync::Arc<dyn Sleep>>,
}

impl<T: crate::transport::Transport> std::fmt::Debug for UnifiedTransportWrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("UnifiedTransportWrapper");
        debug.field("transport", &"<Transport>");
        #[cfg(feature = "async")]
        debug.field("has_sleep_impl", &self.sleep_impl.is_some());
        debug.finish()
    }
}

#[async_trait::async_trait]
impl<T> UnifiedTransport for UnifiedTransportWrapper<T>
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
        #[cfg(feature = "async")]
        {
            // For async transports, use futures::executor to block
            futures::executor::block_on(self.send(bytes))
        }
        #[cfg(not(feature = "async"))]
        {
            // In blocking mode, use the transport directly
            futures::executor::block_on(self.transport.send(bytes)).map_err(Into::into)
        }
    }

    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<bytes::Bytes, Error> {
        // For async transports, use futures::executor with timeout
        #[cfg(feature = "async")]
        {
            if let Some(sleep_impl) = &self.sleep_impl {
                // Use the provided Sleep implementation
                futures::executor::block_on(async {
                    crate::executor::timeout_with_sleep(sleep_impl.as_ref(), timeout, self.recv())
                        .await
                })?
            } else {
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
                    // Without tokio and no Sleep impl, use a simpler timeout approach
                    use std::time::Instant;
                    let _start = Instant::now();

                    // For non-tokio async transports, we just block on recv without timeout
                    // This is a limitation when not using tokio
                    log::warn!(
                        "Timeout ({timeout:?}) not supported without tokio feature or Sleep implementation - blocking on recv"
                    );
                    futures::executor::block_on(self.recv())
                }
            }
        }
        #[cfg(not(feature = "async"))]
        {
            // In blocking mode, blocking transports return Ready futures
            // So block_on just extracts the value immediately
            use crate::transport::core::TransportExt;
            self.transport.recv_with_timeout_blocking(timeout)
        }
    }
}
