//! Synchronous transport trait for VISCA communication.
//!
//! This trait provides synchronous methods for blocking transports,
//! with built-in timeout support using OS-level socket timeouts.

use core::time::Duration;

use crate::{command::CommandKind, transport::builder::TransportConfig, Error};

/// Transport handle for blocking VISCA communication.
///
/// This enum provides a typed alternative to `Box<dyn SyncTransport>` that avoids heap
/// allocation and dynamic dispatch. It mirrors the design of the async `TransportHandle<R>`
/// but for blocking transports.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::BlockingTransportHandle;
/// use grafton_visca::transport::blocking::{Tcp, Udp};
///
/// // Create a handle from either TCP or UDP transport
/// let handle = BlockingTransportHandle::Tcp(
///     Tcp::connect_with_config("192.168.0.110:5678", Default::default())?
/// );
/// ```
#[cfg(not(feature = "mode-async"))]
#[derive(Debug)]
pub enum BlockingTransportHandle {
    /// TCP transport for blocking mode.
    Tcp(crate::transport::blocking::Tcp),
    /// UDP transport for blocking mode.
    Udp(crate::transport::blocking::Udp),
    /// Serial transport for blocking mode.
    #[cfg(feature = "transport-serial")]
    Serial(crate::transport::serial_blocking::SerialTransport),
}

#[cfg(not(feature = "mode-async"))]
impl SyncTransport for BlockingTransportHandle {
    fn send_with_kind(&mut self, bytes: &[u8], kind: CommandKind) -> Result<(), Error> {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.send_with_kind(bytes, kind),
            BlockingTransportHandle::Udp(transport) => transport.send_with_kind(bytes, kind),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.send_with_kind(bytes, kind),
        }
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.recv_into(dst),
            BlockingTransportHandle::Udp(transport) => transport.recv_into(dst),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.recv_into(dst),
        }
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        match self {
            BlockingTransportHandle::Tcp(transport) => {
                transport.recv_into_with_timeout(dst, timeout)
            }
            BlockingTransportHandle::Udp(transport) => {
                transport.recv_into_with_timeout(dst, timeout)
            }
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => {
                transport.recv_into_with_timeout(dst, timeout)
            }
        }
    }
}

#[cfg(not(feature = "mode-async"))]
impl HasTransportConfig for BlockingTransportHandle {
    fn transport_config(&self) -> &TransportConfig {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.transport_config(),
            BlockingTransportHandle::Udp(transport) => transport.transport_config(),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.transport_config(),
        }
    }
}

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

/// Trait for types that carry transport configuration.
///
/// This trait provides a unified interface for accessing the transport configuration
/// from various transport types, enabling the runtime to apply consistent settings
/// (buffers, timeouts) across all transport implementations.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::{HasTransportConfig, TransportConfig};
///
/// // Access configuration from any transport
/// let config = transport.transport_config();
/// let read_timeout = config.read_timeout;
/// let buffer_config = &config.buffer_config;
/// ```
pub trait HasTransportConfig {
    /// Get the transport configuration.
    fn transport_config(&self) -> &TransportConfig;
}

// Blanket implementation for references, enabling HRTB bounds like `for<'a> &'a T: HasTransportConfig`
impl<T: HasTransportConfig + ?Sized> HasTransportConfig for &T {
    #[inline]
    fn transport_config(&self) -> &TransportConfig {
        (*self).transport_config()
    }
}
