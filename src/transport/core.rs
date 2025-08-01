//! Generic Associated Type (GAT) based transport trait.
//!
//! This module provides a unified transport abstraction that works for both
//! blocking and async implementations using GAT futures.

use crate::Error;
use core::future::Future;

#[cfg(feature = "async")]
use crate::executor::Sleep;

/// Low-level VISCA byte transport - one frame at a time.
///
/// This trait unifies blocking and async transports using GAT futures.
/// Blocking implementations use `std::future::Ready`, while async
/// implementations use actual async futures.
pub trait Transport {
    /// Error type for transport operations.
    type Error: Into<Error>;

    /// Future produced by `send()` - completes when bytes are on the wire.
    type SendFut<'a>: Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    /// Future produced by `recv()` - resolves to exactly one VISCA frame.
    type RecvFut<'a>: Future<Output = Result<bytes::Bytes, Self::Error>>
    where
        Self: 'a;

    /// Send raw bytes to the device.
    fn send<'a>(&'a self, bytes: &'a [u8]) -> Self::SendFut<'a>;

    /// Receive raw bytes from the device.
    /// Returns exactly one VISCA frame.
    fn recv(&self) -> Self::RecvFut<'_>;
}

/// Helper module for blocking implementations.
pub mod blocking {
    use core::future::Ready;

    /// Create a ready future from a result for blocking implementations.
    pub fn ready<T, E>(result: Result<T, E>) -> Ready<Result<T, E>> {
        core::future::ready(result)
    }
}

/// Marker trait for blocking transports.
///
/// This trait is implemented by transports that return immediately-ready futures
/// (using `std::future::Ready`). It's used to enforce at compile time that
/// `CameraBlocking` can only be used with blocking transports.
pub trait BlockingTransport: Transport {}

/// Extension trait for timeout operations.
pub trait TransportExt: Transport {
    /// Receive with timeout using a runtime-specific Sleep implementation.
    ///
    /// When `sleep_impl` is provided, it will be used for timeout.
    /// When `sleep_impl` is None and tokio feature is enabled, tokio::time::timeout is used.
    /// Otherwise, no timeout is applied.
    #[cfg(feature = "async")]
    fn recv_with_timeout<'a>(
        &'a self,
        duration: core::time::Duration,
        sleep_impl: Option<&'a dyn Sleep>,
    ) -> impl Future<Output = Result<bytes::Bytes, Error>> + 'a
    where
        Self: 'a,
    {
        async move {
            if let Some(sleep) = sleep_impl {
                crate::executor::timeout_with_sleep(sleep, duration, self.recv())
                    .await
                    .map_err(Into::into)
            } else {
                #[cfg(feature = "tokio")]
                {
                    tokio::time::timeout(duration, self.recv())
                        .await
                        .map_err(|_| Error::Timeout)?
                        .map_err(Into::into)
                }
                #[cfg(not(feature = "tokio"))]
                {
                    // Without a specific runtime, we can't implement timeout.
                    log::debug!("Timeout requested but no Sleep implementation provided and tokio feature not enabled");
                    self.recv().await.map_err(Into::into)
                }
            }
        }
    }

    /// Blocking timeout helper for non-async transports.
    #[cfg(not(feature = "async"))]
    fn recv_with_timeout_blocking(
        &self,
        duration: core::time::Duration,
    ) -> Result<bytes::Bytes, Error> {
        crate::executor::timeout(duration, self.recv()).and_then(|r| r.map_err(Into::into))
    }
}

impl<T: Transport> TransportExt for T {}
