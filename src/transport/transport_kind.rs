//! Enum-based transport wrapper for zero-cost abstraction.
//!
//! This module provides `TransportKind` for cases where you need to store
//! different transport types in a single type without dynamic dispatch.

use crate::{transport::core::Transport, Error};
use bytes::Bytes;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

#[cfg(feature = "tokio")]
use crate::transport::tokio::{tcp::Tcp as TokioTcp, udp::Udp as TokioUdp};

use crate::transport::blocking::{Tcp as BlockingTcp, Udp as BlockingUdp};

/// Future type for TransportKind send operations.
#[derive(Debug)]
pub enum TransportKindSendFut<'a> {
    /// Blocking TCP send future
    BlockingTcp(<BlockingTcp as Transport>::SendFut<'a>),
    /// Blocking UDP send future
    BlockingUdp(<BlockingUdp as Transport>::SendFut<'a>),
    #[cfg(feature = "tokio")]
    /// Tokio TCP send future
    TokioTcp(<TokioTcp as Transport>::SendFut<'a>),
    #[cfg(feature = "tokio")]
    /// Tokio UDP send future
    TokioUdp(<TokioUdp as Transport>::SendFut<'a>),
}

// Safety: The inner futures are Send, so the enum is Send
#[allow(unsafe_code)]
unsafe impl Send for TransportKindSendFut<'_> {}

impl Future for TransportKindSendFut<'_> {
    type Output = Result<(), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Safety: We're not moving the inner futures, just polling them
        #[allow(unsafe_code)]
        unsafe {
            match self.get_unchecked_mut() {
                TransportKindSendFut::BlockingTcp(f) => Pin::new_unchecked(f).poll(cx),
                TransportKindSendFut::BlockingUdp(f) => Pin::new_unchecked(f).poll(cx),
                #[cfg(feature = "tokio")]
                TransportKindSendFut::TokioTcp(f) => Pin::new_unchecked(f).poll(cx),
                #[cfg(feature = "tokio")]
                TransportKindSendFut::TokioUdp(f) => Pin::new_unchecked(f).poll(cx),
            }
        }
    }
}

/// Future type for TransportKind receive operations.
#[derive(Debug)]
pub enum TransportKindRecvFut<'a> {
    /// Blocking TCP receive future
    BlockingTcp(<BlockingTcp as Transport>::RecvFut<'a>),
    /// Blocking UDP receive future
    BlockingUdp(<BlockingUdp as Transport>::RecvFut<'a>),
    #[cfg(feature = "tokio")]
    /// Tokio TCP receive future
    TokioTcp(<TokioTcp as Transport>::RecvFut<'a>),
    #[cfg(feature = "tokio")]
    /// Tokio UDP receive future
    TokioUdp(<TokioUdp as Transport>::RecvFut<'a>),
}

// Safety: The inner futures are Send, so the enum is Send
#[allow(unsafe_code)]
unsafe impl Send for TransportKindRecvFut<'_> {}

impl Future for TransportKindRecvFut<'_> {
    type Output = Result<Bytes, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Safety: We're not moving the inner futures, just polling them
        #[allow(unsafe_code)]
        unsafe {
            match self.get_unchecked_mut() {
                TransportKindRecvFut::BlockingTcp(f) => Pin::new_unchecked(f).poll(cx),
                TransportKindRecvFut::BlockingUdp(f) => Pin::new_unchecked(f).poll(cx),
                #[cfg(feature = "tokio")]
                TransportKindRecvFut::TokioTcp(f) => Pin::new_unchecked(f).poll(cx),
                #[cfg(feature = "tokio")]
                TransportKindRecvFut::TokioUdp(f) => Pin::new_unchecked(f).poll(cx),
            }
        }
    }
}

/// Enum representing different transport implementations.
///
/// This type allows runtime transport selection with zero-cost abstraction,
/// avoiding the overhead of dynamic dispatch.
///
/// # Example
///
/// ```ignore
/// // Example of using TransportKind enum
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let use_tcp = true;
/// // Can store different transport types in the same enum
/// let transport = if use_tcp {
///     TransportKind::BlockingTcp(Tcp::connect("192.168.1.100:5678")?)
/// } else {
///     TransportKind::BlockingUdp(Udp::connect("192.168.1.100:5678")?)
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub enum TransportKind {
    /// Blocking TCP transport
    BlockingTcp(BlockingTcp),
    /// Blocking UDP transport
    BlockingUdp(BlockingUdp),
    #[cfg(feature = "tokio")]
    /// Tokio TCP transport
    TokioTcp(TokioTcp),
    #[cfg(feature = "tokio")]
    /// Tokio UDP transport
    TokioUdp(TokioUdp),
}

/// Implementation of Transport for TransportKind.
impl Transport for TransportKind {
    type Error = Error;

    type SendFut<'a>
        = TransportKindSendFut<'a>
    where
        Self: 'a;

    type RecvFut<'a>
        = TransportKindRecvFut<'a>
    where
        Self: 'a;

    fn send<'a>(&'a self, bytes: &'a [u8]) -> Self::SendFut<'a> {
        match self {
            TransportKind::BlockingTcp(t) => TransportKindSendFut::BlockingTcp(t.send(bytes)),
            TransportKind::BlockingUdp(t) => TransportKindSendFut::BlockingUdp(t.send(bytes)),
            #[cfg(feature = "tokio")]
            TransportKind::TokioTcp(t) => TransportKindSendFut::TokioTcp(t.send(bytes)),
            #[cfg(feature = "tokio")]
            TransportKind::TokioUdp(t) => TransportKindSendFut::TokioUdp(t.send(bytes)),
        }
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        match self {
            TransportKind::BlockingTcp(t) => TransportKindRecvFut::BlockingTcp(t.recv()),
            TransportKind::BlockingUdp(t) => TransportKindRecvFut::BlockingUdp(t.recv()),
            #[cfg(feature = "tokio")]
            TransportKind::TokioTcp(t) => TransportKindRecvFut::TokioTcp(t.recv()),
            #[cfg(feature = "tokio")]
            TransportKind::TokioUdp(t) => TransportKindRecvFut::TokioUdp(t.recv()),
        }
    }
}

impl TransportKind {
    /// Create a TransportKind from a blocking TCP transport.
    pub fn from_blocking_tcp(transport: BlockingTcp) -> Self {
        Self::BlockingTcp(transport)
    }

    /// Create a TransportKind from a blocking UDP transport.
    pub fn from_blocking_udp(transport: BlockingUdp) -> Self {
        Self::BlockingUdp(transport)
    }

    #[cfg(feature = "tokio")]
    /// Create a TransportKind from a Tokio TCP transport.
    #[must_use]
    pub fn from_tokio_tcp(transport: TokioTcp) -> Self {
        Self::TokioTcp(transport)
    }

    #[cfg(feature = "tokio")]
    /// Create a TransportKind from a Tokio UDP transport.
    #[must_use]
    pub fn from_tokio_udp(transport: TokioUdp) -> Self {
        Self::TokioUdp(transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_kind_size() {
        // Ensure the enum doesn't get too large
        let size = size_of::<TransportKind>();
        // This will vary based on the largest variant, but should be reasonable
        assert!(size < 256, "TransportKind size is {size} bytes");
    }
}
