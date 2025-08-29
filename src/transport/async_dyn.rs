//! Object-safe dynamic async transport trait.
//!
//! This module provides [`DynAsyncTransport`] and [`BoxAsyncTransport`] for
//! object-safe, runtime-agnostic dynamic usage. This complements the zero-cost
//! generic [`AsyncTransport`] trait.
//!
//! # Usage
//!
//! ```rust,no_run
//! # #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
//! # {
//! use grafton_visca::transport::async_dyn::BoxAsyncTransport;
//! use grafton_visca::transport::Transport;
//!
//! # async fn example() -> Result<(), grafton_visca::Error> {
//! // Get a stable dynamic transport handle
//! let transport: BoxAsyncTransport = Transport::tcp()
//!     .address("192.168.0.110:5678")
//!     .connect_dyn()
//!     .await?;
//!
//! // Use the transport - each call allocates but type is stable across features
//! // transport.send(b"some data").await?;
//! # Ok(())
//! # }
//! # }
//! ```

use core::{future::Future, pin::Pin};

use bytes::Bytes;

use crate::{error::Error, transport::AsyncTransport};

/// Object-safe dynamic async transport trait.
///
/// This trait provides object safety for async transport operations by using
/// pinned boxed futures. Unlike [`AsyncTransport`], this trait can be
/// used with `dyn` and provides a stable type across feature configurations.
///
/// Each method call allocates a boxed future, making this unsuitable for hot paths.
/// Use the zero-cost generic [`AsyncTransport`] trait for performance-critical code.
///
/// # Send Safety
///
/// All returned futures are guaranteed to be `Send`, ensuring spawn-safety across
/// all async runtimes.
pub trait DynAsyncTransport: Send {
    /// Send raw bytes to the device.
    ///
    /// This method sends the provided bytes over the transport and returns
    /// a boxed future that resolves when the bytes have been written.
    fn send<'a>(
        &'a mut self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;

    /// Receive raw bytes from the device.
    ///
    /// This method receives exactly one VISCA frame from the device.
    /// It should handle framing internally and return complete frames.
    fn recv<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = Result<Bytes, Error>> + Send + 'a>>;
}

/// Stable dynamic handle for async transports.
///
/// This type alias provides a convenient way to work with boxed dynamic transports.
/// Unlike [`AnyTransport`](super::builder::AnyTransport), this type is stable across
/// feature configurations and does not change shape based on enabled runtimes.
pub type BoxAsyncTransport = Box<dyn DynAsyncTransport>;

/// Blanket implementation to convert any [`AsyncTransport`] into [`DynAsyncTransport`].
///
/// This allows zero-cost generic transports to be used in dynamic contexts when needed.
impl<T> DynAsyncTransport for T
where
    T: AsyncTransport + Send + 'static,
{
    fn send<'a>(
        &'a mut self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(<Self as AsyncTransport>::send(self, bytes))
    }

    fn recv<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = Result<Bytes, Error>> + Send + 'a>> {
        Box::pin(<Self as AsyncTransport>::recv(self))
    }
}

/// Convenience conversion from generic transports to boxed dynamic transports.
impl<T> From<T> for BoxAsyncTransport
where
    T: AsyncTransport + Send + 'static,
{
    fn from(transport: T) -> Self {
        Box::new(transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time test to ensure futures are Send.
    ///
    /// This test verifies that futures returned by DynAsyncTransport methods
    /// implement Send, ensuring spawn-safety.
    fn assert_send<T: Send>(_: T) {}

    #[test]
    fn test_dyn_async_transport_futures_are_send() {
        // Create a mock transport for testing
        struct MockTransport;

        impl AsyncTransport for MockTransport {
            async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
                Ok(())
            }

            async fn recv(&mut self) -> Result<Bytes, Error> {
                Ok(Bytes::new())
            }
        }

        let mut transport: BoxAsyncTransport = Box::new(MockTransport);

        // Test that futures are Send (test one at a time to avoid borrow checker issues)
        let send_fut = transport.send(b"test");
        assert_send(send_fut);

        let recv_fut = transport.recv();
        assert_send(recv_fut);
    }
}
