//! Unified transport trait that works for both async and blocking contexts.
//!
//! This module provides a single transport abstraction that eliminates the need
//! for separate async and sync traits, reducing code duplication and complexity.

use std::future::Future;
use std::pin::Pin;

use crate::{error::Error, Command};

/// Type alias for boxed futures used in transport operations.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Unified transport trait that supports both async and blocking operations.
///
/// This trait uses associated types to allow implementations to provide
/// either truly async futures or immediately-ready futures for blocking operations.
pub trait UnifiedTransport: Send + Sync {
    /// The future type returned by send operations.
    type SendFuture<'a>: Future<Output = Result<(), Error>> + Send + 'a
    where
        Self: 'a;

    /// The future type returned by receive operations.
    type ReceiveFuture<'a>: Future<Output = Result<Vec<Vec<u8>>, Error>> + Send + 'a
    where
        Self: 'a;

    /// Send a VISCA command to the camera.
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> Self::SendFuture<'a>;

    /// Receive response frames from the camera.
    fn receive_response(&mut self) -> Self::ReceiveFuture<'_>;
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
    /// Send a command synchronously.
    ///
    /// # Errors
    /// Returns `Error` if the command cannot be sent to the camera.
    fn send_blocking(&mut self, command: &dyn Command) -> Result<(), Error>;

    /// Receive responses synchronously.
    ///
    /// # Errors
    /// Returns `Error` if responses cannot be received from the camera.
    fn receive_blocking(&mut self) -> Result<Vec<Vec<u8>>, Error>;
}

impl<T: BlockingTransport> UnifiedTransport for BlockingTransportAdapter<T> {
    type SendFuture<'a>
        = ReadyFuture<Result<(), Error>>
    where
        Self: 'a;

    type ReceiveFuture<'a>
        = ReadyFuture<Result<Vec<Vec<u8>>, Error>>
    where
        Self: 'a;

    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> Self::SendFuture<'a> {
        ready(self.inner.send_blocking(command))
    }

    fn receive_response(&mut self) -> Self::ReceiveFuture<'_> {
        ready(self.inner.receive_blocking())
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    struct MockBlockingTransport;

    impl BlockingTransport for MockBlockingTransport {
        fn send_blocking(&mut self, _: &dyn Command) -> Result<(), Error> {
            Ok(())
        }

        fn receive_blocking(&mut self) -> Result<Vec<Vec<u8>>, Error> {
            Ok(vec![vec![0x90, 0x50, 0xFF]])
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
        let mut send_fut = adapter.send_command(&cmd);

        match Pin::new(&mut send_fut).poll(&mut cx) {
            Poll::Ready(Ok(())) => {}
            _ => panic!("Expected immediate ready result"),
        }

        // Test receive
        let mut recv_fut = adapter.receive_response();

        match Pin::new(&mut recv_fut).poll(&mut cx) {
            Poll::Ready(Ok(responses)) => {
                assert_eq!(responses.len(), 1);
                assert_eq!(responses[0], vec![0x90, 0x50, 0xFF]);
            }
            _ => panic!("Expected immediate ready result"),
        }
    }
}
