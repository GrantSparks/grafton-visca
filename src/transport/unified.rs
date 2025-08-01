//! Unified transport trait for abstracting over async and blocking transports.

use crate::error::Error;
use std::time::Duration;

// Removed unused imports

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

// UnifiedTransportWrapper has been removed. Transports now implement UnifiedTransport directly.
