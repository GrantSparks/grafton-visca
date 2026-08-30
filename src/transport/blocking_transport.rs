//! Blocking transport trait for VISCA communication.
//!
//! This trait provides blocking methods for transports,
//! with built-in timeout support using OS-level socket timeouts.

use core::time::Duration;

use crate::{
    command::CommandKind,
    transport::{builder::TransportConfig, AddressingMode, SendSemantics},
    Error,
};

/// Transport handle for blocking VISCA communication.
///
/// This enum provides a typed alternative to `Box<dyn BlockingTransport>` that avoids heap
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
#[cfg(feature = "blocking")]
#[derive(Debug)]
#[non_exhaustive]
pub enum BlockingTransportHandle {
    /// TCP transport for blocking mode.
    Tcp(crate::transport::blocking::Tcp),
    /// UDP transport for blocking mode.
    Udp(crate::transport::blocking::Udp),
    /// Serial transport for blocking mode.
    #[cfg(feature = "transport-serial")]
    Serial(crate::transport::serial_blocking::SerialTransport),
}

#[cfg(feature = "blocking")]
impl BlockingTransport for BlockingTransportHandle {
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

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.addressing_mode_hint(),
            BlockingTransportHandle::Udp(transport) => transport.addressing_mode_hint(),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.addressing_mode_hint(),
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.send_semantics(),
            BlockingTransportHandle::Udp(transport) => transport.send_semantics(),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.send_semantics(),
        }
    }
}

#[cfg(feature = "blocking")]
impl HasTransportConfig for BlockingTransportHandle {
    fn transport_config(&self) -> &TransportConfig {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.transport_config(),
            BlockingTransportHandle::Udp(transport) => transport.transport_config(),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.transport_config(),
        }
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(match self {
            BlockingTransportHandle::Tcp(_) => crate::camera::TransportKind::Tcp,
            BlockingTransportHandle::Udp(_) => crate::camera::TransportKind::Udp,
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(_) => crate::camera::TransportKind::Serial,
        })
    }
}

/// Blocking transport for VISCA communication.
///
/// This trait provides blocking methods for transports that block
/// the current thread. It includes built-in timeout support that should
/// be implemented using OS-level socket timeouts where possible.
///
/// Transports are stream/datagram-level adapters that read/write raw bytes.
/// Framing and retry logic are handled by the runtime layer.
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
pub trait BlockingTransport: Send {
    /// Send raw bytes to the device with command kind (blocking).
    ///
    /// This method blocks until the bytes have been written to the
    /// underlying transport. The CommandKind is used for proper protocol
    /// framing (e.g., Sony encapsulation).
    ///
    /// # Arguments
    ///
    /// * `bytes` - The raw VISCA command bytes to send
    /// * `kind` - Whether this is a command or inquiry for proper framing
    ///
    /// # Error contract
    ///
    /// What a send failure costs is decided by
    /// [`BlockingTransport::send_semantics`], not by the error value:
    ///
    /// - [`SendSemantics::Stream`]: the byte-stream position is now unknowable,
    ///   so the runtime poisons the session. Every request in flight fails with
    ///   [`Error::StreamPoisoned`] carrying this error's text.
    /// - [`SendSemantics::Datagram`]: only the request being written fails and
    ///   the session keeps running. Because the session survives, the runtime
    ///   normalizes a session-fatal error value (any error for which
    ///   [`Error::requires_new_session`] is true, such as
    ///   [`Error::ConnectionClosed`]) into a plain per-request
    ///   [`Error::TransportError`]: a caller must never be told to open a new
    ///   session by an error raised on one that is still running. Report a
    ///   genuinely dead socket from the receive side, which is the side the
    ///   runtime treats as authoritative about session death.
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
    ///
    /// The error contract is the same as
    /// [`BlockingTransport::recv_into_with_timeout`].
    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error>;

    /// Read raw bytes with a timeout (blocking).
    ///
    /// This method blocks until some data is available or the timeout expires.
    /// Implementations should use OS-level socket timeouts where possible for efficiency.
    ///
    /// # Arguments
    ///
    /// * `dst` - The buffer to read data into
    /// * `timeout` - Maximum time to wait for data
    ///
    /// # Error contract
    ///
    /// The runtime never guesses: the value this method returns decides whether
    /// the session lives, and whether every command still waiting for its ACK is
    /// retransmitted. A failed read must consume nothing, so that the runtime's
    /// framing state stays intact.
    ///
    /// - `Ok(n)` with `n > 0` — bytes were read. Returning fewer bytes than a
    ///   whole frame is normal and is not an error; the runtime buffers the
    ///   remainder until a later read completes the frame.
    /// - `Ok(0)` — end of stream: the peer closed. The runtime ends the session
    ///   with [`Error::ConnectionClosed`]. Never return `Ok(0)` to mean "no data
    ///   yet"; that is the one signal reserved for EOF.
    /// - `Err(Error::Timeout)` — `timeout` expired and no bytes arrived. The
    ///   runtime treats it as "no data": the session lives, framing state is
    ///   untouched, and no request's retry budget is spent. The raw I/O
    ///   spellings [`std::io::ErrorKind::TimedOut`],
    ///   [`std::io::ErrorKind::WouldBlock`] and
    ///   [`std::io::ErrorKind::Interrupted`] wrapped in [`Error::Io`] are
    ///   normalized to the same meaning.
    /// - A session-fatal error — any error for which
    ///   [`Error::requires_new_session`] is true, plus [`Error::Io`] carrying
    ///   `ConnectionReset`, `ConnectionAborted`, `BrokenPipe`, `UnexpectedEof`
    ///   or `NotConnected`. The runtime ends the session as
    ///   [`Error::ConnectionClosed`] and retains the transport cause's text in
    ///   its reason. A failed read consumed nothing, so it is not a stream
    ///   framing poison; [`Error::StreamPoisoned`] is reserved for an
    ///   unknowable stream position caused by framing loss or a failed write.
    /// - Any other error is a *transient* fault: the read failed but the
    ///   connection may still be usable — a UDP `recv` reporting ECONNREFUSED
    ///   after an ICMP port-unreachable is the canonical case. The engine may
    ///   retransmit sequence-correlated Sony commands still waiting for their
    ///   ACK under each command's retry policy. A raw command awaiting ACK has
    ///   no sequence key, so it is left to its own ACK deadline and is never
    ///   replayed merely because of this fault. Do not use this class for idle
    ///   timeouts.
    ///
    /// # Returns
    ///
    /// * `Ok(n)` - Number of bytes read (0 = EOF)
    /// * `Err(Error::Timeout)` - If the timeout expires
    /// * `Err(_)` - For other transport errors
    fn recv_into_with_timeout(&mut self, dst: &mut [u8], timeout: Duration)
        -> Result<usize, Error>;

    /// Return a side-effect-free hint for the transport's VISCA addressing mode.
    ///
    /// Multi-target sessions require an explicit [`AddressingMode::Serial`]
    /// declaration because serial replies carry a camera source while IP
    /// replies do not. Custom transports default to `None`, which conservatively
    /// rejects multi-target startup before configuration is read. Single-target
    /// sessions do not require a hint. The hint only authorizes preflight;
    /// `TransportConfig::addressing` remains the runtime's framing source.
    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        None
    }

    /// Query the transport's send semantics.
    ///
    /// This method returns whether the transport uses stream or datagram semantics,
    /// which determines how the runtime handles send failures:
    ///
    /// - [`SendSemantics::Stream`]: On send failure/timeout, the transport is
    ///   "poisoned" and the runtime stops using it to prevent protocol desync.
    ///
    /// - [`SendSemantics::Datagram`]: On send failure, only the affected command
    ///   fails; the transport continues operating for subsequent commands.
    ///
    /// # Default Implementation
    ///
    /// Returns [`SendSemantics::Stream`] by default, which is the safer choice
    /// for unknown transport types. Datagram transports (UDP) should override
    /// this to return [`SendSemantics::Datagram`].
    ///
    /// # Forwarding wrappers
    ///
    /// A transport that wraps another (such as [`BlockingTransportHandle`], which
    /// dispatches to a TCP, UDP, or serial inner transport) **must** forward this
    /// method to the inner transport, exactly as it forwards
    /// [`BlockingTransport::send_with_kind`] and [`BlockingTransport::recv_into`].
    /// Unlike a missing `match` arm, an unforwarded `send_semantics` does not fail
    /// to compile — it silently falls back to this `Stream` default, which is
    /// wrong for any datagram inner transport. The inner transport is the
    /// authority on its own semantics.
    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }
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

    /// Return the standard transport kind when this is a built-in TCP, UDP, or
    /// serial transport. Custom transports leave this as `None` so profile
    /// compatibility remains an explicit BYO escape hatch. Multi-target
    /// routing additionally requires an explicit serial
    /// [`BlockingTransport::addressing_mode_hint`] implementation.
    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        None
    }
}

// Blanket implementation for references, enabling HRTB bounds like `for<'a> &'a T: HasTransportConfig`
impl<T: HasTransportConfig + ?Sized> HasTransportConfig for &T {
    #[inline]
    fn transport_config(&self) -> &TransportConfig {
        (*self).transport_config()
    }

    #[inline]
    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        (*self).standard_transport_kind()
    }
}
