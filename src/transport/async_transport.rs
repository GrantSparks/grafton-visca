//! Async transport trait for zero-cost async VISCA communication.
//!
//! This trait provides zero-overhead async transport via RPITIT methods that
//! return unboxed futures with explicit `Send` guarantees.

use std::future::Future;

use crate::{
    transport::{AddressingMode, SendSemantics},
    Error,
};

/// Async transport for VISCA communication.
///
/// This trait uses return-position impl trait in traits (RPITIT) with explicit
/// `+ Send` bounds to ensure futures can be safely sent across threads.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::AsyncTransport;
/// struct MyAsyncTransport { /* ... */ }
///
/// impl AsyncTransport for MyAsyncTransport {
///     fn send(&mut self, _bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
///         async move { Ok(()) }
///     }
///
///     fn recv_into<'a>(&'a mut self, dst: &'a mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send {
///         async move { Ok(0) }
///     }
/// }
/// ```
pub trait AsyncTransport: Send {
    /// Send raw bytes to the device.
    ///
    /// This method sends the provided bytes over the transport and returns
    /// when the bytes have been written to the underlying transport.
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;

    /// Receive raw bytes from the device into the provided buffer.
    ///
    /// This method reads the next available chunk of bytes from the transport
    /// into the provided buffer and returns the number of bytes read.
    /// It may return partial frames, complete frames, or multiple frames.
    /// The runtime is responsible for aggregating chunks and extracting frames.
    ///
    /// Returns `Ok(0)` when the connection is closed.
    /// Stream-based transports should return `Err(Error::ConnectionClosed)` when encountering EOF.
    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send;

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
    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }
}
