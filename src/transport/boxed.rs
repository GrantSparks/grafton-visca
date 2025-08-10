//! Boxed transport wrapper for dynamic dispatch.
//!
//! This module provides a trait-object-safe wrapper around GAT-based transports,
//! allowing them to be used with dynamic dispatch in places like the socket manager.
//!
//! ## Architecture
//!
//! Since the `Transport` trait uses GATs (Generic Associated Types), it cannot be made
//! into a trait object directly. This module provides:
//!
//! 1. `TransportDyn` - An object-safe trait that returns boxed futures
//! 2. `BoxedTransport` - A wrapper type that implements `Transport` using boxed futures
//! 3. Blanket implementation to convert any `Transport` to `TransportDyn`
//!
//! This allows us to use dynamic dispatch where needed (e.g., SocketManager) while
//! keeping the zero-cost abstraction for concrete types.

use crate::Error;
use core::future::Future;
use core::pin::Pin;
use std::sync::Arc;

/// Object-safe transport trait that returns boxed futures.
///
/// This trait is automatically implemented for all types that implement `Transport`.
/// It provides the same functionality but with boxed futures, making it suitable
/// for use with dynamic dispatch.
pub trait TransportDyn: Send + Sync {
    /// Send raw bytes to the device (returns boxed future).
    fn send_dyn<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;

    /// Receive raw bytes from the device (returns boxed future).
    fn recv_dyn(&self) -> Pin<Box<dyn Future<Output = Result<bytes::Bytes, Error>> + Send + '_>>;
}

/// Blanket implementation: any Transport automatically implements TransportDyn.
///
/// This allows all concrete transport types to be used with dynamic dispatch
/// by boxing their futures.
impl<T> TransportDyn for T
where
    T: super::core::Transport + Send + Sync,
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn send_dyn<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            super::core::Transport::send(self, bytes)
                .await
                .map_err(Into::into)
        })
    }

    fn recv_dyn(&self) -> Pin<Box<dyn Future<Output = Result<bytes::Bytes, Error>> + Send + '_>> {
        Box::pin(async move { super::core::Transport::recv(self).await.map_err(Into::into) })
    }
}

/// Boxed transport wrapper that implements the Transport trait using boxed futures.
///
/// This type wraps `Arc<dyn TransportDyn>` and implements the `Transport` trait,
/// allowing it to be used anywhere a `T: Transport + Send + Sync` is required while still
/// supporting dynamic dispatch.
///
/// ## Usage
///
/// ```rust,no_run
/// # use grafton_visca::transport::{BoxedTransport, Transport};
/// # use std::sync::Arc;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(feature = "tokio")]
/// # {
/// use grafton_visca::transport::tokio::Tcp;
///
/// // Create a concrete transport
/// let tcp = Tcp::connect("192.168.0.110:5678").await?;
///
/// // Box it for dynamic dispatch
/// let boxed = BoxedTransport::new(Arc::new(tcp));
///
/// // boxed can now be used with SocketManager or stored in collections
/// # }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct BoxedTransport {
    inner: Arc<dyn TransportDyn>,
}

impl core::fmt::Debug for BoxedTransport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BoxedTransport").finish_non_exhaustive()
    }
}

impl BoxedTransport {
    /// Create a new boxed transport from any type that implements TransportDyn.
    ///
    /// Since all Transport types automatically implement TransportDyn through
    /// the blanket implementation, this can accept any Arc'd transport.
    pub fn new(inner: Arc<dyn TransportDyn>) -> Self {
        Self { inner }
    }

    /// Create a boxed transport from a concrete transport type.
    ///
    /// This is a convenience method that wraps the transport in an Arc.
    pub fn from_transport<T>(transport: T) -> Self
    where
        T: super::core::Transport + Send + Sync + 'static,
        T::Error: Into<Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::new(Arc::new(transport))
    }
}

/// BoxedTransport implements Transport with boxed futures.
///
/// This allows BoxedTransport to be used anywhere a `T: Transport + Send + Sync` is required,
/// while internally using dynamic dispatch.
impl super::core::Transport for BoxedTransport {
    type Error = Error;

    type SendFut<'a>
        = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>
    where
        Self: 'a;

    type RecvFut<'a>
        = Pin<Box<dyn Future<Output = Result<bytes::Bytes, Self::Error>> + Send + 'a>>
    where
        Self: 'a;

    fn send<'a>(&'a self, bytes: &'a [u8]) -> Self::SendFut<'a> {
        self.inner.send_dyn(bytes)
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        self.inner.recv_dyn()
    }
}
