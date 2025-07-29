//! Enum-based transport wrapper for zero-cost abstraction.
//!
//! This module provides `TransportKind` for cases where you need to store
//! different transport types in a single type without dynamic dispatch.

use crate::{transport::core::Transport, Error};
use bytes::Bytes;
use core::future::Future;

#[cfg(feature = "tokio")]
use crate::transport::tokio::{tcp::Tcp as TokioTcp, udp::Udp as TokioUdp};

use crate::transport::blocking::{Tcp as BlockingTcp, Udp as BlockingUdp};

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
        = core::pin::Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>
    where
        Self: 'a;

    type RecvFut<'a>
        = core::pin::Pin<Box<dyn Future<Output = Result<Bytes, Self::Error>> + Send + 'a>>
    where
        Self: 'a;

    fn send<'a>(&'a self, bytes: &'a [u8]) -> Self::SendFut<'a> {
        Box::pin(async move {
            match self {
                TransportKind::BlockingTcp(t) => t.send(bytes).await,
                TransportKind::BlockingUdp(t) => t.send(bytes).await,
                #[cfg(feature = "tokio")]
                TransportKind::TokioTcp(t) => t.send(bytes).await,
                #[cfg(feature = "tokio")]
                TransportKind::TokioUdp(t) => t.send(bytes).await,
            }
        })
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        Box::pin(async move {
            match self {
                TransportKind::BlockingTcp(t) => t.recv().await,
                TransportKind::BlockingUdp(t) => t.recv().await,
                #[cfg(feature = "tokio")]
                TransportKind::TokioTcp(t) => t.recv().await,
                #[cfg(feature = "tokio")]
                TransportKind::TokioUdp(t) => t.recv().await,
            }
        })
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
        assert!(size < 256, "TransportKind size is {} bytes", size);
    }
}
