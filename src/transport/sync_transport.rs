//! Synchronous transport trait for VISCA communication.
//!
//! This trait provides synchronous methods for blocking transports,
//! with built-in timeout support using OS-level socket timeouts.

use bytes::Bytes;

use core::time::Duration;

use crate::{command::CommandKind, Error};

/// Synchronous transport for VISCA communication.
///
/// This trait provides synchronous methods for transports that block
/// the current thread. It includes built-in timeout support that should
/// be implemented using OS-level socket timeouts where possible.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::SyncTransport;
/// use core::time::Duration;
///
/// struct MySyncTransport { /* ... */ }
///
/// impl SyncTransport for MySyncTransport {
///     fn send_with_kind(&mut self, bytes: &[u8], kind: CommandKind) -> Result<(), Error> {
///         // Send implementation with command kind for proper framing
///         Ok(())
///     }
///
///     fn recv(&mut self) -> Result<Bytes, Error> {
///         // Receive implementation without timeout
///         Ok(Bytes::new())
///     }
///
///     fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes, Error> {
///         // Receive implementation with timeout
///         Ok(Bytes::new())
///     }
/// }
/// ```
pub trait SyncTransport: Send {
    /// Send raw bytes to the device with command kind (synchronous).
    ///
    /// This method blocks until the bytes have been written to the
    /// underlying transport. The CommandKind is used for proper protocol
    /// framing (e.g., Sony encapsulation).
    ///
    /// # Arguments
    ///
    /// * `bytes` - The raw VISCA command bytes to send
    /// * `kind` - Whether this is a command or inquiry for proper framing
    fn send_with_kind(&mut self, bytes: &[u8], kind: CommandKind) -> Result<(), Error>;

    /// Receive raw bytes from the device (synchronous).
    ///
    /// This method blocks until a complete VISCA frame is received.
    /// There is no timeout - it will block indefinitely.
    fn recv(&mut self) -> Result<Bytes, Error>;

    /// Receive raw bytes with a timeout (synchronous).
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
    fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes, Error>;
}

// Implement SyncTransport for Box<dyn SyncTransport> to enable nested boxing
impl SyncTransport for Box<dyn SyncTransport> {
    fn send_with_kind(&mut self, bytes: &[u8], kind: CommandKind) -> Result<(), Error> {
        (**self).send_with_kind(bytes, kind)
    }

    fn recv(&mut self) -> Result<Bytes, Error> {
        (**self).recv()
    }

    fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes, Error> {
        (**self).recv_with_timeout(timeout)
    }
}
