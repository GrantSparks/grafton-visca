//! Trait-object wrapper for dynamic transport dispatch.
//!
//! This module provides `AnyTransport` for rare cases where dynamic dispatch
//! is needed instead of the zero-cost generic approach.

use crate::{transport::gat_transport::Transport, Error};
use bytes::Bytes;
use core::future::Future;
use core::pin::Pin;

/// Type-erased transport trait for dynamic dispatch.
///
/// This trait mirrors the Transport trait but boxes its futures,
/// making it object-safe at the cost of an extra allocation.
trait ErasedTransport: Send + Sync {
    fn send<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
    fn recv<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Bytes, Error>> + Send + 'a>>;
}

/// Wrapper to make any Transport implementation work with ErasedTransport.
struct ErasedTransportWrapper<T: Transport>(T);

impl<T> ErasedTransport for ErasedTransportWrapper<T>
where
    T: Transport + Send + Sync,
    T::Error: Into<Error>,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn send<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move { self.0.send(bytes).await.map_err(Into::into) })
    }

    fn recv<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Bytes, Error>> + Send + 'a>> {
        Box::pin(async move { self.0.recv().await.map_err(Into::into) })
    }
}

/// Dynamic transport wrapper for trait-object use cases.
///
/// This type allows runtime transport selection at the cost of dynamic dispatch.
/// For most use cases, the generic Transport trait should be preferred.
///
/// # Example
///
/// ```no_run
/// # use grafton_visca::transport::AnyTransport;
/// # use grafton_visca::transport::blocking::{Tcp, Udp};
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let use_tcp = true;
/// // Can store different transport types in the same variable
/// let transport: AnyTransport = if use_tcp {
///     AnyTransport::new(TcpGat::connect("192.168.1.100:5678")?)
/// } else {
///     AnyTransport::new(UdpGat::connect("192.168.1.100:5678")?)
/// };
/// # Ok(())
/// # }
/// ```
pub struct AnyTransport(Box<dyn ErasedTransport>);

impl AnyTransport {
    /// Create a new AnyTransport from any Transport implementation.
    pub fn new<T>(transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        T::Error: Into<Error>,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self(Box::new(ErasedTransportWrapper(transport)))
    }
}

/// Implementation of Transport for AnyTransport.
impl Transport for AnyTransport {
    type Error = Error;
    type SendFut<'a>
        = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>
    where
        Self: 'a;
    type RecvFut<'a>
        = Pin<Box<dyn Future<Output = Result<Bytes, Self::Error>> + Send + 'a>>
    where
        Self: 'a;

    fn send<'a>(&'a self, bytes: &'a [u8]) -> Self::SendFut<'a> {
        self.0.send(bytes)
    }

    fn recv<'a>(&'a self) -> Self::RecvFut<'a> {
        self.0.recv()
    }
}

impl std::fmt::Debug for AnyTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyTransport").finish()
    }
}
