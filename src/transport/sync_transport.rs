//! Synchronous transport trait for VISCA communication.
//!
//! This trait provides synchronous methods for blocking transports,
//! with built-in timeout support using OS-level socket timeouts.

use core::time::Duration;

use crate::{command::CommandKind, Error};

/// Synchronous transport for VISCA communication.
///
/// This trait provides synchronous methods for transports that block
/// the current thread. It includes built-in timeout support that should
/// be implemented using OS-level socket timeouts where possible.
///
/// Transports are stream/datagram-level adapters that read/write raw bytes.
/// Framing and retry logic are handled by the runtime layer.
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
///     fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
///         // Receive implementation without timeout
///         Ok(0)
///     }
///
///     fn recv_into_with_timeout(&mut self, dst: &mut [u8], timeout: Duration) -> Result<usize, Error> {
///         // Receive implementation with timeout
///         Ok(0)
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

    /// Read raw bytes into caller-provided buffer.
    ///
    /// This method blocks until some data is available and reads it into
    /// the provided buffer. It returns the number of bytes read.
    /// Returns 0 to indicate EOF (peer closed connection).
    ///
    /// # Arguments
    ///
    /// * `dst` - The buffer to read data into
    ///
    /// # Returns
    ///
    /// * `Ok(n)` - Number of bytes read (0 = EOF)
    /// * `Err(_)` - For transport errors
    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error>;

    /// Read raw bytes with a timeout (synchronous).
    ///
    /// This method blocks until some data is available or the timeout expires.
    /// Implementations should use OS-level socket timeouts where possible for efficiency.
    ///
    /// # Arguments
    ///
    /// * `dst` - The buffer to read data into
    /// * `timeout` - Maximum time to wait for data
    ///
    /// # Returns
    ///
    /// * `Ok(n)` - Number of bytes read (0 = EOF)
    /// * `Err(Error::Timeout)` - If the timeout expires
    /// * `Err(_)` - For other transport errors
    fn recv_into_with_timeout(&mut self, dst: &mut [u8], timeout: Duration)
        -> Result<usize, Error>;
}

// Implement SyncTransport for Box<dyn SyncTransport> to enable nested boxing
impl SyncTransport for Box<dyn SyncTransport> {
    fn send_with_kind(&mut self, bytes: &[u8], kind: CommandKind) -> Result<(), Error> {
        (**self).send_with_kind(bytes, kind)
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        (**self).recv_into(dst)
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        (**self).recv_into_with_timeout(dst, timeout)
    }
}
