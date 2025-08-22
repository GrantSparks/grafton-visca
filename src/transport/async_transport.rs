//! Async transport trait for zero-cost async VISCA communication.
//!
//! This trait provides native async functions without boxing, enabling
//! zero-overhead async transport implementations.

use bytes::Bytes;
use std::future::Future;

use crate::Error;

/// Async transport for VISCA communication.
///
/// This trait uses native async functions in traits (AFIT) with explicit Send bounds
/// to ensure futures can be safely sent across threads.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::AsyncTransport;
/// use std::future::Future;
///
/// struct MyAsyncTransport { /* ... */ }
///
/// impl AsyncTransport for MyAsyncTransport {
///     fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
///         async move {
///             // Send implementation
///             Ok(())
///         }
///     }
///
///     fn recv(&mut self) -> impl Future<Output = Result<Bytes, Error>> + Send {
///         async move {
///             // Receive implementation
///             Ok(Bytes::new())
///         }
///     }
/// }
/// ```
pub trait AsyncTransport: Send {
    /// Send raw bytes to the device.
    ///
    /// This method sends the provided bytes over the transport and returns
    /// when the bytes have been written to the underlying transport.
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;

    /// Receive raw bytes from the device.
    ///
    /// This method receives exactly one VISCA frame from the device.
    /// It should handle framing internally and return complete frames.
    fn recv(&mut self) -> impl Future<Output = Result<Bytes, Error>> + Send;
}
