//! Unified transport layer for VISCA communication.
//!
//! This module provides the core transport abstractions for the v0.4.0 API,
//! featuring an async-first design with optional blocking adapters.

// Standard library imports
use std::{future::Future, pin::Pin};

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{ViscaCommand, ViscaError};

// Submodules
pub mod adapters;
mod tcp;
mod udp;

// Public re-exports
#[cfg(feature = "blocking-client")]
pub use self::{tcp::TcpTransport, udp::UdpTransport};

#[cfg(feature = "async-client")]
pub use self::{tcp::AsyncTcpTransport, udp::AsyncUdpTransport};

pub use self::adapters::{IntoTransport, LegacyTransportAdapter};

/// Type alias for the future returned by async transport methods.
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ViscaError>> + Send + 'a>>;

/// Primary async transport trait for VISCA communication.
///
/// This is the main transport abstraction in v0.4.0, designed to be async-first
/// while supporting blocking operations through adapters.
pub trait Transport: Send + Sync {
    /// Send a VISCA command to the camera asynchronously.
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()>;

    /// Receive response frames from the camera asynchronously.
    ///
    /// Returns a vector of response frames that have been received.
    /// Each frame is a complete VISCA response (starts with 0x90 and ends with 0xFF).
    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>>;
}

/// Trait for blocking transport implementations.
///
/// This trait provides the blocking interface that can be adapted to the async Transport trait.
#[doc(hidden)]
#[cfg(feature = "blocking-client")]
pub trait BlockingTransport {
    /// Send a VISCA command to the camera synchronously.
    fn send_command_blocking(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError>;

    /// Receive response frames from the camera synchronously.
    fn receive_response_blocking(&mut self) -> Result<Vec<Vec<u8>>, ViscaError>;
}

/// Adapter that implements the async Transport trait for any BlockingTransport.
///
/// This allows blocking implementations to be used through the async interface.
#[doc(hidden)]
#[cfg(feature = "blocking-client")]
pub struct BlockingAdapter<T: BlockingTransport>(pub T);

#[cfg(feature = "blocking-client")]
impl<T: BlockingTransport + Send + Sync> Transport for BlockingAdapter<T> {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move { self.0.send_command_blocking(command) })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move { self.0.receive_response_blocking() })
    }
}

#[cfg(all(test, feature = "blocking-client"))]
mod tests {
    use super::*;

    struct MockBlockingTransport;

    impl BlockingTransport for MockBlockingTransport {
        fn send_command_blocking(&mut self, _command: &dyn ViscaCommand) -> Result<(), ViscaError> {
            Ok(())
        }

        fn receive_response_blocking(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
            Ok(vec![vec![0x90, 0x50, 0xFF]])
        }
    }

    #[tokio::test]
    async fn test_blocking_adapter() {
        struct DummyCommand;

        impl ViscaCommand for DummyCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
                Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
            }

            fn response_type(&self) -> Option<crate::command::ViscaResponseType> {
                None
            }

            fn command_category(&self) -> crate::timeout::CommandCategory {
                crate::timeout::CommandCategory::Quick
            }
        }

        let mut adapter = BlockingAdapter(MockBlockingTransport);
        let cmd = DummyCommand;

        adapter.send_command(&cmd).await.unwrap();

        let responses = adapter.receive_response().await.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0], vec![0x90, 0x50, 0xFF]);
    }
}
