//! Async transport trait for zero-cost async VISCA communication.
//!
//! This trait provides native async functions without boxing, enabling
//! zero-overhead async transport implementations.

use crate::Error;
use bytes::Bytes;

/// Async transport for VISCA communication.
///
/// This trait uses native `async fn` in traits (stable since Rust 1.75)
/// to provide zero-cost async transport without heap allocations.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::AsyncTransport;
///
/// struct MyAsyncTransport { /* ... */ }
///
/// impl AsyncTransport for MyAsyncTransport {
///     async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
///         // Send implementation
///         Ok(())
///     }
///
///     async fn recv(&self) -> Result<Bytes, Error> {
///         // Receive implementation
///         Ok(Bytes::new())
///     }
/// }
/// ```
pub trait AsyncTransport: Send + Sync {
    /// Send raw bytes to the device.
    ///
    /// This method sends the provided bytes over the transport and returns
    /// when the bytes have been written to the underlying transport.
    fn send(&self, bytes: &[u8]) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Receive raw bytes from the device.
    ///
    /// This method receives exactly one VISCA frame from the device.
    /// It should handle framing internally and return complete frames.
    fn recv(&self) -> impl std::future::Future<Output = Result<Bytes, Error>> + Send;
}
