//! Unified transport layer for VISCA communication.
//!
//! This module provides the core transport abstractions for the v0.4.0 API,
//! featuring an async-first design with optional blocking adapters.

// Standard library imports
use std::{future::Future, pin::Pin};

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{error::Error, Command};

// Submodules
pub mod common;
pub mod resilient;
mod tcp;
mod tcp_unified;
/// Core transport traits and types
pub mod traits;
mod udp;
pub mod unified;

// Public re-exports
#[cfg(feature = "blocking-client")]
pub use self::{tcp::TcpTransport, udp::UdpTransport};

#[cfg(feature = "async-client")]
pub use self::{tcp::AsyncTcpTransport, udp::AsyncUdpTransport};

// Unified transport re-exports
pub use self::tcp_unified::UnifiedTcpTransport;
pub use self::unified::UnifiedTransport;

/// Type alias for the future returned by async transport methods.
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// Primary async transport trait for VISCA communication.
///
/// This is the main transport abstraction in v0.4.0, designed to be async-first
/// while supporting blocking operations through adapters.
pub trait Transport: Send + Sync {
    /// Send a VISCA command to the camera asynchronously with a specific socket ID.
    ///
    /// The socket ID is used for tracking concurrent commands. VISCA cameras typically
    /// support Socket 0 and Socket 1 for up to 2 concurrent commands.
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: crate::types::SocketId,
    ) -> TransportFuture<'a, ()>;

    /// Receive a response frame from the camera asynchronously.
    ///
    /// Returns a tuple of (socket_id, response_data) where:
    /// - socket_id identifies which command this response belongs to
    /// - response_data is the complete VISCA response (starts with 0x90 and ends with 0xFF)
    fn receive_response(&mut self) -> TransportFuture<'_, (crate::types::SocketId, Vec<u8>)>;
}

/// Implementation of Transport for `Box<dyn Transport>` to allow dynamic dispatch.
impl Transport for Box<dyn Transport> {
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: crate::types::SocketId,
    ) -> TransportFuture<'a, ()> {
        (**self).send_command(command, socket_id)
    }

    fn receive_response(&mut self) -> TransportFuture<'_, (crate::types::SocketId, Vec<u8>)> {
        (**self).receive_response()
    }
}

/// Trait for blocking transport implementations.
///
/// This trait provides the blocking interface that can be adapted to the async Transport trait.
#[doc(hidden)]
#[cfg(feature = "blocking-client")]
pub trait BlockingTransport {
    /// Send a VISCA command to the camera synchronously with a specific socket ID.
    fn send_command_blocking(
        &mut self,
        command: &dyn Command,
        socket_id: crate::types::SocketId,
    ) -> Result<(), Error>;

    /// Receive a response frame from the camera synchronously.
    fn receive_response_blocking(&mut self) -> Result<(crate::types::SocketId, Vec<u8>), Error>;
}

/// Adapter that implements the async Transport trait for any BlockingTransport.
///
/// This allows blocking implementations to be used through the async interface.
#[doc(hidden)]
#[cfg(feature = "blocking-client")]
#[derive(Debug)]
pub struct BlockingAdapter<T: BlockingTransport>(pub T);

#[cfg(feature = "blocking-client")]
impl<T: BlockingTransport + Send + Sync> Transport for BlockingAdapter<T> {
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: crate::types::SocketId,
    ) -> TransportFuture<'a, ()> {
        Box::pin(async move { self.0.send_command_blocking(command, socket_id) })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, (crate::types::SocketId, Vec<u8>)> {
        Box::pin(async move { self.0.receive_response_blocking() })
    }
}

#[cfg(all(test, feature = "blocking-client"))]
mod tests {
    use super::*;

    struct MockBlockingTransport;

    impl BlockingTransport for MockBlockingTransport {
        fn send_command_blocking(
            &mut self,
            _command: &dyn Command,
            _socket_id: crate::types::SocketId,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn receive_response_blocking(
            &mut self,
        ) -> Result<(crate::types::SocketId, Vec<u8>), Error> {
            Ok((crate::types::SocketId::SOCKET_0, vec![0x90, 0x50, 0xFF]))
        }
    }

    #[tokio::test]
    async fn test_blocking_adapter() -> Result<(), Error> {
        struct DummyCommand;

        impl Command for DummyCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
            }

            fn response_type(&self) -> Option<crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> crate::timeout::CommandCategory {
                crate::timeout::CommandCategory::Quick
            }
        }

        let mut adapter = BlockingAdapter(MockBlockingTransport);
        let cmd = DummyCommand;

        adapter
            .send_command(&cmd, crate::types::SocketId::SOCKET_0)
            .await?;

        let (socket_id, response) = adapter.receive_response().await?;
        assert_eq!(socket_id, crate::types::SocketId::SOCKET_0);
        assert_eq!(response, vec![0x90, 0x50, 0xFF]);
        Ok(())
    }
}
