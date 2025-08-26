//! Async transport trait for zero-cost async VISCA communication.
//!
//! This trait provides zero-overhead async transport via RPITIT methods that
//! return unboxed futures with explicit `Send` guarantees.

use bytes::Bytes;

use crate::Error;
use std::future::Future;

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
///     fn recv(&mut self) -> impl Future<Output = Result<Bytes, Error>> + Send {
///         async move { Ok(Bytes::new()) }
///     }
/// }
/// ```
pub trait AsyncTransport: Send {
    /// Send raw bytes to the device.
    ///
    /// This method sends the provided bytes over the transport and returns
    /// when the bytes have been written to the underlying transport. TODO: Fix this to be async fn when return type notation is stable in Rust.
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;

    /// Receive raw bytes from the device.
    ///
    /// This method receives exactly one VISCA frame from the device.
    /// It should handle framing internally and return complete frames. TODO: Fix this to be async fn when return type notation is stable in Rust.
    fn recv(&mut self) -> impl Future<Output = Result<Bytes, Error>> + Send;
}
