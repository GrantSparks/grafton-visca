//! Async transport trait for zero-cost async VISCA communication.
//!
//! This trait provides zero-overhead async transport via RPITIT methods that
//! return unboxed futures with explicit `Send` guarantees.

use std::future::Future;

use crate::Error;

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
}
