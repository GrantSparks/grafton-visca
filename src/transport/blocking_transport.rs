//! Blocking transport trait for synchronous VISCA communication.
//!
//! This trait provides synchronous methods for blocking transports,
//! with built-in timeout support using OS-level socket timeouts.

use bytes::Bytes;

use core::time::Duration;

use crate::Error;

/// Blocking transport for VISCA communication.
///
/// This trait provides synchronous methods for transports that block
/// the current thread. It includes built-in timeout support that should
/// be implemented using OS-level socket timeouts where possible.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::BlockingTransport;
/// use core::time::Duration;
///
/// struct MyBlockingTransport { /* ... */ }
///
/// impl BlockingTransport for MyBlockingTransport {
///     fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
///         // Send implementation
///         Ok(())
///     }
///
///     fn recv_blocking(&self) -> Result<Bytes, Error> {
///         // Receive implementation without timeout
///         Ok(Bytes::new())
///     }
///
///     fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes, Error> {
///         // Receive implementation with timeout
///         Ok(Bytes::new())
///     }
/// }
/// ```
pub trait BlockingTransport: Send + Sync {
    /// Send raw bytes to the device (blocking).
    ///
    /// This method blocks until the bytes have been written to the
    /// underlying transport.
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device (blocking).
    ///
    /// This method blocks until a complete VISCA frame is received.
    /// There is no timeout - it will block indefinitely.
    fn recv_blocking(&self) -> Result<Bytes, Error>;

    /// Receive raw bytes with a timeout (blocking).
    ///
    /// This method blocks until a complete VISCA frame is received or
    /// the timeout expires. Implementations should use OS-level socket
    /// timeouts where possible for efficiency.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for data
    ///
    /// # Returns
    ///
    /// * `Ok(bytes)` - A complete VISCA frame
    /// * `Err(Error::Timeout)` - If the timeout expires
    /// * `Err(_)` - For other transport errors
    fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes, Error>;
}

// Implement BlockingTransport for Box<dyn BlockingTransport> to enable nested boxing
impl BlockingTransport for Box<dyn BlockingTransport> {
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        (**self).send_blocking(bytes)
    }

    fn recv_blocking(&self) -> Result<Bytes, Error> {
        (**self).recv_blocking()
    }

    fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes, Error> {
        (**self).recv_blocking_with_timeout(timeout)
    }
}
