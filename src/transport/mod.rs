//! Unified transport layer for VISCA communication.
//!
//! This module provides the core transport abstractions for the v0.4.0 API,
//! featuring an async-first design with optional blocking adapters.
//!
//! The transport layer only defines traits - concrete implementations
//! (TCP, UDP, serial, etc.) should be implemented by users or provided
//! in example code.

// Standard library imports
use std::{future::Future, pin::Pin};

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{error::Error, Command, Response};

// Submodules
/// Channel-based transport for thread-safe sharing
#[cfg(feature = "async-client")]
pub mod channel;
pub mod common;
/// Core transport traits and types
pub mod traits;

// Channel transport re-exports
#[cfg(feature = "async-client")]
pub use self::channel::{ChannelTransport, ChannelTransportBuilder, ChannelTransportConfig};

/// Type alias for the future returned by async transport methods.
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// Primary async transport trait for VISCA communication.
///
/// This is the main transport abstraction in v0.4.0, designed to be async-first
/// while supporting blocking operations through adapters. The transport now handles
/// the complete command lifecycle including socket management and response matching.
pub trait Transport: Send + Sync {
    /// Send a VISCA command and wait for its complete response.
    ///
    /// This method handles the entire command lifecycle:
    /// 1. Automatically assigns an available socket (0 or 1)
    /// 2. Sends the command with the appropriate socket ID
    /// 3. Waits for and processes all responses (ACK, completion, or error)
    /// 4. Returns the final response
    ///
    /// # Arguments
    /// * `command` - The VISCA command to send
    ///
    /// # Returns
    /// The final response from the camera (completion with data for inquiries,
    /// or simple completion for control commands)
    ///
    /// # Errors
    /// - `Error::CommandBufferFull` if both sockets are in use
    /// - Transport errors if communication fails
    /// - VISCA errors if the camera rejects the command
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response>;
}

/// Implementation of Transport for `Box<dyn Transport>` to allow dynamic dispatch.
impl Transport for Box<dyn Transport> {
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response> {
        (**self).send_command(command)
    }
}

/// Trait for blocking transport implementations.
///
/// This trait provides the blocking interface that can be adapted to the async Transport trait.
#[doc(hidden)]
#[cfg(feature = "blocking-client")]
pub trait BlockingTransport {
    /// Send a VISCA command and wait for its complete response (blocking).
    fn send_command_blocking(&mut self, command: &dyn Command) -> Result<Response, Error>;
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
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response> {
        Box::pin(async move { self.0.send_command_blocking(command) })
    }
}

#[cfg(all(test, feature = "blocking-client"))]
mod tests {
    use super::*;
    use crate::Response as ViscaResponse;

    struct MockBlockingTransport;

    impl BlockingTransport for MockBlockingTransport {
        fn send_command_blocking(&mut self, _command: &dyn Command) -> Result<Response, Error> {
            Ok(ViscaResponse::Completion)
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

        let response = adapter.send_command(&cmd).await?;
        assert!(matches!(response, ViscaResponse::Completion));
        Ok(())
    }
}
