//! Unified transport trait that works for both async and blocking contexts.
//!
//! This module provides a single transport abstraction that eliminates the need
//! for separate async and sync traits, reducing code duplication and complexity.

use std::future::Future;
use std::pin::Pin;

use crate::{error::Error, Command, Response};

/// Type alias for boxed futures used in transport operations.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Unified transport trait that supports both async and blocking operations.
///
/// This trait uses associated types to allow implementations to provide
/// either truly async futures or immediately-ready futures for blocking operations.
/// The transport now handles the complete command lifecycle including socket management.
pub trait UnifiedTransport: Send + Sync {
    /// The future type returned by send_command operations.
    type CommandFuture<'a>: Future<Output = Result<Response, Error>> + Send + 'a
    where
        Self: 'a;

    /// Send a VISCA command and wait for its complete response.
    ///
    /// This method handles the entire command lifecycle:
    /// 1. Automatically assigns an available socket (0 or 1)
    /// 2. Sends the command with the appropriate socket ID
    /// 3. Waits for and processes all responses (ACK, completion, or error)
    /// 4. Returns the final response
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> Self::CommandFuture<'a>;
}

/// Blocking transport adapter that implements `UnifiedTransport` using ready futures.
///
/// This adapter allows blocking transports to be used through the unified interface
/// without requiring actual async runtime support.
#[derive(Debug)]
pub struct BlockingTransportAdapter<T> {
    inner: T,
}

impl<T> BlockingTransportAdapter<T> {
    /// Create a new blocking transport adapter.
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

/// Ready future that immediately returns a result.
///
/// Used by blocking implementations to satisfy the async interface.
use std::future::ready;

/// Wrapper around `std::future::Ready` for compatibility.
pub type ReadyFuture<T> = std::future::Ready<T>;

/// Trait for blocking transport implementations.
pub trait BlockingTransport: Send + Sync {
    /// Send a VISCA command and wait for its complete response (blocking).
    ///
    /// # Errors
    /// Returns `Error` if the command cannot be sent to the camera or if the response is invalid.
    fn send_command_blocking(&mut self, command: &dyn Command) -> Result<Response, Error>;
}

impl<T: BlockingTransport> UnifiedTransport for BlockingTransportAdapter<T> {
    type CommandFuture<'a>
        = ReadyFuture<Result<Response, Error>>
    where
        Self: 'a;

    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> Self::CommandFuture<'a> {
        ready(self.inner.send_command_blocking(command))
    }
}

/// Async transport adapter that implements `UnifiedTransport` for async transports.
///
/// This provides a convenient way to use the main Transport trait through the unified interface.
#[cfg(feature = "async-client")]
#[derive(Debug)]
pub struct AsyncTransportAdapter<T> {
    inner: T,
}

#[cfg(feature = "async-client")]
impl<T> AsyncTransportAdapter<T> {
    /// Create a new async transport adapter.
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "async-client")]
impl<T: crate::transport::Transport> UnifiedTransport for AsyncTransportAdapter<T> {
    type CommandFuture<'a>
        = crate::transport::TransportFuture<'a, Response>
    where
        Self: 'a;

    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> Self::CommandFuture<'a> {
        self.inner.send_command(command)
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    struct MockBlockingTransport;

    impl BlockingTransport for MockBlockingTransport {
        fn send_command_blocking(&mut self, _: &dyn Command) -> Result<Response, Error> {
            Ok(Response::Completion)
        }
    }

    #[test]
    fn test_blocking_adapter() {
        use std::sync::Arc;
        use std::task::{Context, Poll};

        struct NoopWaker;

        impl std::task::Wake for NoopWaker {
            fn wake(self: Arc<Self>) {}
        }

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

        let waker = Arc::new(NoopWaker).into();
        let mut cx = Context::from_waker(&waker);

        let mut adapter = BlockingTransportAdapter::new(MockBlockingTransport);

        let cmd = DummyCommand;
        let mut cmd_fut = adapter.send_command(&cmd);

        match Pin::new(&mut cmd_fut).poll(&mut cx) {
            Poll::Ready(Ok(Response::Completion)) => {}
            _ => panic!("Expected immediate ready result with Completion response"),
        }
    }
}
