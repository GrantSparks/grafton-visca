#![allow(missing_docs)]
//! Common test utilities for grafton-visca tests.
//!
//! This module provides shared mock implementations and utilities
//! to avoid code duplication across test files.

// Allow unsafe in tests for creating mock wakers
#![allow(unsafe_code)]

// Re-export submodules
pub mod builders;
pub mod helpers;
pub mod macros;

#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::BlockingTransport;

#[cfg(any(not(feature = "async"), all(feature = "async", feature = "tokio")))]
use grafton_visca::Error;

#[cfg(any(not(feature = "async"), all(feature = "async", feature = "tokio")))]
use std::collections::VecDeque;

#[cfg(any(not(feature = "async"), all(feature = "async", feature = "tokio")))]
use std::sync::Arc;

#[cfg(not(feature = "async"))]
use std::sync::Mutex;

/// A flexible mock transport for testing various scenarios.
#[cfg(not(feature = "async"))]
#[derive(Clone, Debug)]
pub struct MockTransport {
    /// Queue of responses to return
    pub responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    /// Commands that have been sent
    pub commands_sent: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Whether to fail on send
    pub fail_send: bool,
    /// Whether to fail on receive
    pub fail_receive: bool,
}

#[cfg(not(feature = "async"))]
impl MockTransport {
    /// Create a new mock transport with no responses queued.
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            commands_sent: Arc::new(Mutex::new(Vec::new())),
            fail_send: false,
            fail_receive: false,
        }
    }

    /// Create a mock that returns ACK followed by completion.
    pub fn with_ack_completion() -> Self {
        let mock = Self::new();
        mock.add_ack_completion(0);
        mock
    }

    /// Add a response to the queue.
    pub fn add_response(&self, response: Vec<u8>) {
        self.responses.lock().unwrap().push_back(response);
    }

    /// Add an ACK followed by completion response.
    pub fn add_ack_completion(&self, socket: u8) {
        self.add_response(vec![0x90, 0x40 | socket, 0xFF]); // ACK
        self.add_response(vec![0x90, 0x50 | socket, 0xFF]); // Completion
    }
}

#[cfg(not(feature = "async"))]
impl BlockingTransport for MockTransport {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        if self.fail_send {
            return Err(Error::Io(std::io::Error::other("Mock send error")));
        }
        self.commands_sent.lock().unwrap().push(data.to_vec());
        Ok(())
    }

    fn receive(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, Error> {
        if self.fail_receive {
            return Err(Error::Io(std::io::Error::other("Mock receive error")));
        }
        self.responses.lock().unwrap().pop_front().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Mock timeout - no response queued",
            ))
        })
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Mock transport for testing"
    }
}

// Async version of MockTransport for feature parity
#[cfg(all(feature = "async", feature = "tokio"))]
use grafton_visca::transport::AsyncTransport;
#[cfg(all(feature = "async", feature = "tokio"))]
use std::time::Duration;
#[cfg(all(feature = "async", feature = "tokio"))]
use tokio::sync::Mutex as AsyncMutex;

/// Async mock transport for testing - feature parity with MockTransport
#[cfg(all(feature = "async", feature = "tokio"))]
#[derive(Clone)]
pub struct MockAsyncTransport {
    /// Queue of responses to return
    pub responses: Arc<AsyncMutex<VecDeque<Vec<u8>>>>,
    /// Commands that have been sent
    pub sent_commands: Arc<AsyncMutex<Vec<Vec<u8>>>>,
    /// Whether to fail on send
    pub fail_send: bool,
    /// Whether to fail on receive
    pub fail_receive: bool,
    /// Fail after N commands (for testing error scenarios)
    pub fail_after: Option<usize>,
    /// Command counter
    pub command_counter: Arc<AsyncMutex<usize>>,
    /// Optional delay in milliseconds
    pub delay_ms: Option<u64>,
}

#[cfg(all(feature = "async", feature = "tokio"))]
impl MockAsyncTransport {
    /// Create a new mock transport with no responses queued.
    pub fn new() -> Self {
        Self {
            responses: Arc::new(AsyncMutex::new(VecDeque::new())),
            sent_commands: Arc::new(AsyncMutex::new(Vec::new())),
            fail_send: false,
            fail_receive: false,
            fail_after: None,
            command_counter: Arc::new(AsyncMutex::new(0)),
            delay_ms: None,
        }
    }

    /// Create with a delay (in milliseconds).
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = Some(delay_ms);
        self
    }

    /// Add a response to the queue.
    pub async fn add_response(&self, response: Vec<u8>) {
        self.responses.lock().await.push_back(response);
    }

    /// Add an ACK followed by completion response.
    pub async fn add_ack_completion(&self, socket: u8) {
        self.add_response(vec![0x90, 0x40 | socket, 0xFF]).await; // ACK
        self.add_response(vec![0x90, 0x50 | socket, 0xFF]).await; // Completion
    }
}

#[cfg(all(feature = "async", feature = "tokio"))]
impl std::fmt::Debug for MockAsyncTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockAsyncTransport")
            .field("fail_send", &self.fail_send)
            .field("fail_receive", &self.fail_receive)
            .field("delay_ms", &self.delay_ms)
            .finish()
    }
}

#[cfg(all(feature = "async", feature = "tokio"))]
impl AsyncTransport for MockAsyncTransport {
    type SendFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
        Box::pin(async move {
            // Check if we should fail after N commands
            let mut counter = self.command_counter.lock().await;
            *counter += 1;
            if let Some(fail_after) = self.fail_after {
                if *counter > fail_after {
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        "Mock transport configured to fail",
                    )));
                }
            }
            drop(counter);

            if self.fail_send {
                return Err(Error::Io(std::io::Error::other("Mock send error")));
            }

            self.sent_commands.lock().await.push(data.to_vec());

            if let Some(delay) = self.delay_ms {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            Ok(())
        })
    }

    fn receive(&self) -> Self::ReceiveFuture<'_> {
        Box::pin(async move {
            if self.fail_receive {
                return Err(Error::Io(std::io::Error::other("Mock receive error")));
            }

            if let Some(delay) = self.delay_ms {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            self.responses
                .lock()
                .await
                .pop_front()
                .ok_or_else(|| Error::CommandTimeout {
                    duration: Duration::from_millis(self.delay_ms.unwrap_or(100)),
                    command: "mock_receive".to_string(),
                })
        })
    }
}
