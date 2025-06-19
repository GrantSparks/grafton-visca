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

#[cfg(feature = "blocking-client")]
use grafton_visca::{Command, Error};

#[cfg(feature = "blocking-client")]
use grafton_visca::{InquiryResponse, Response};

#[cfg(feature = "blocking-client")]
use grafton_visca::transport::blocking::{Transport as BlockingTransport, ViscaTransport};

// Import parse_response from the command module
#[cfg(feature = "blocking-client")]
use grafton_visca::command::parse_response;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A flexible mock transport for testing various scenarios.
#[derive(Clone)]
#[allow(dead_code)] // Complete testing API - not all methods used in every test
pub struct MockTransport {
    /// Queue of responses to return
    pub responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    /// Commands that have been sent
    pub commands_sent: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Whether to fail on send
    pub fail_send: bool,
    /// Whether to fail on receive
    pub fail_receive: bool,
    /// Fail after N commands (for testing error scenarios)
    pub fail_after: Option<usize>,
}

#[allow(dead_code)] // Complete testing API - not all methods used in every test
#[cfg(feature = "blocking-client")]
impl MockTransport {
    /// Create a new mock transport with no responses queued.
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            commands_sent: Arc::new(Mutex::new(Vec::new())),
            fail_send: false,
            fail_receive: false,
            fail_after: None,
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

    /// Add an inquiry response.
    pub fn add_inquiry_response(&self, response: Vec<u8>) {
        self.add_response(response);
    }

    /// Get the commands that were sent.
    pub fn commands_sent(&self) -> Vec<Vec<u8>> {
        self.commands_sent.lock().unwrap().clone()
    }

    /// Get the last command that was sent.
    pub fn last_command(&self) -> Option<Vec<u8>> {
        self.commands_sent.lock().unwrap().last().cloned()
    }

    /// Clear all sent commands.
    pub fn clear_commands(&self) {
        self.commands_sent.lock().unwrap().clear();
    }

    /// Create a mock that expects a specific command and returns a response.
    pub fn expecting(command: &[u8], response: &[u8]) -> Self {
        let mut mock = Self::new();
        mock.expect_command(command, response);
        mock
    }

    /// Add an expectation for a command and its response.
    pub fn expect_command(&mut self, _expected_command: &[u8], response: &[u8]) {
        // For now, we just add the response. In the future, we could verify
        // that the expected command matches what was sent.
        self.add_response(response.to_vec());
    }

    /// Verify that all expected commands were sent.
    /// This is a placeholder for future enhancement where we track expectations.
    pub fn verify(&self) {
        // Currently, this is a no-op. In the future, we could track
        // expected vs actual commands and panic if they don't match.
    }

    /// Create a mock that returns an error response.
    pub fn with_error(error_code: u8) -> Self {
        let mock = Self::new();
        mock.add_response(vec![0x90, 0x60, error_code, 0xFF]);
        mock
    }

    /// Create a mock that times out (returns no response).
    pub fn with_timeout() -> Self {
        Self::new() // No responses queued means timeout
    }
}

#[cfg(feature = "blocking-client")]
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

// Helper to create a ViscaTransport for testing
#[cfg(feature = "blocking-client")]
impl MockTransport {
    pub fn into_visca_transport(self) -> ViscaTransport<Self> {
        ViscaTransport::new(self)
    }
}

/// A mock device implementation that properly handles VISCA protocol.
#[cfg(feature = "blocking-client")]
#[allow(dead_code)] // Complete testing API - not all methods used in every test
pub struct MockDevice {
    transport: BlockingAdapter<MockTransport>,
}

#[cfg(feature = "blocking-client")]
#[allow(dead_code)] // Complete testing API - not all methods used in every test
impl MockDevice {
    /// Create a new mock device.
    pub fn new() -> Self {
        Self {
            transport: BlockingAdapter(MockTransport::new()),
        }
    }

    /// Create a mock device that returns completion for all commands.
    pub fn with_completion() -> Self {
        Self {
            transport: BlockingAdapter(MockTransport::with_ack_completion()),
        }
    }

    /// Create from a custom mock transport.
    pub const fn from_transport(transport: MockTransport) -> Self {
        Self {
            transport: BlockingAdapter(transport),
        }
    }

    /// Add a response to the queue.
    pub fn add_response(&self, response: Vec<u8>) {
        self.transport.0.add_response(response);
    }

    /// Add an inquiry response.
    pub fn queue_inquiry_response(&self, response: InquiryResponse) {
        // Convert the inquiry response to bytes based on its type
        let bytes = match response {
            InquiryResponse::Power { on } => {
                vec![0x90, 0x50, if on { 0x02 } else { 0x03 }, 0xFF]
            }
            InquiryResponse::PanTiltPosition { pan, tilt } => {
                // Extract 4-bit nibbles from 16-bit signed values
                // Using transmute for protocol-level bit manipulation
                let pan_u16 = u16::from_ne_bytes(pan.to_ne_bytes());
                let tilt_u16 = u16::from_ne_bytes(tilt.to_ne_bytes());
                vec![
                    0x90,
                    0x50,
                    ((pan_u16 >> 12) & 0x0F) as u8,
                    ((pan_u16 >> 8) & 0x0F) as u8,
                    ((pan_u16 >> 4) & 0x0F) as u8,
                    (pan_u16 & 0x0F) as u8,
                    ((tilt_u16 >> 12) & 0x0F) as u8,
                    ((tilt_u16 >> 8) & 0x0F) as u8,
                    ((tilt_u16 >> 4) & 0x0F) as u8,
                    (tilt_u16 & 0x0F) as u8,
                    0xFF,
                ]
            }
            InquiryResponse::ZoomPosition { position } => {
                vec![
                    0x90,
                    0x50,
                    ((position >> 12) & 0x0F) as u8,
                    ((position >> 8) & 0x0F) as u8,
                    ((position >> 4) & 0x0F) as u8,
                    (position & 0x0F) as u8,
                    0xFF,
                ]
            }
            _ => vec![0x90, 0x50, 0xFF], // Generic completion for unsupported types
        };
        self.add_response(bytes);
    }

    /// Get the commands that were sent.
    pub fn commands_sent(&self) -> Vec<Vec<u8>> {
        self.transport.0.commands_sent()
    }

    /// Get the last command that was sent.
    pub fn last_command(&self) -> Option<Vec<u8>> {
        self.transport.0.last_command()
    }

    /// Clear the command history.
    pub fn clear_commands(&self) {
        self.transport.0.clear_commands();
    }
}

// MockDevice no longer implements Transport directly since the new API
// uses Camera<P> for high-level operations. Tests should use Camera<P>
// with MockTransport for testing.
#[cfg(feature = "blocking-client")]
impl MockDevice {
    /// Execute a command and handle the response according to VISCA protocol
    #[allow(dead_code)]
    pub fn execute_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // For blocking transport, we don't need futures
        use grafton_visca::transport::Transport;
        use std::future::Future;
        use std::pin::Pin;
        use std::task::{Context, Poll, Waker};

        // Simple executor for our blocking adapter
        fn block_on<F: Future>(mut fut: F) -> F::Output {
            let waker = unsafe {
                Waker::from_raw(std::task::RawWaker::new(
                    std::ptr::null(),
                    &std::task::RawWakerVTable::new(
                        |_| std::task::RawWaker::new(std::ptr::null(), &VTABLE),
                        |_| {},
                        |_| {},
                        |_| {},
                    ),
                ))
            };
            let mut cx = Context::from_waker(&waker);

            loop {
                match unsafe { Pin::new_unchecked(&mut fut) }.poll(&mut cx) {
                    Poll::Ready(val) => return val,
                    Poll::Pending => {}
                }
            }
        }

        const VTABLE: std::task::RawWakerVTable = std::task::RawWakerVTable::new(
            |_| std::task::RawWaker::new(std::ptr::null(), &VTABLE),
            |_| {},
            |_| {},
            |_| {},
        );

        // Send the command and get response using the new unified API
        block_on(self.transport.send_command(command))
    }
}

// Re-export async mock types when async-client feature is enabled
#[cfg(feature = "async-client")]
#[allow(unused_imports)] // Used by async tests when feature is enabled
pub use async_mock::MockAsyncTransport;

#[cfg(feature = "async-client")]
mod async_mock {
    use super::*;
    use grafton_visca::{Command, Error};
    use std::time::Duration;
    use tokio::time::sleep;

    /// An async mock transport for testing async functionality.
    pub struct MockAsyncTransport {
        pub sent_commands: Arc<tokio::sync::Mutex<Vec<Vec<u8>>>>,
        pub responses: Arc<tokio::sync::Mutex<Vec<Vec<u8>>>>,
        pub delay_ms: u64,
        pub fail_after: Option<usize>,
    }

    #[allow(dead_code)] // Complete testing API - not all methods used in every test
    impl MockAsyncTransport {
        pub fn new() -> Self {
            Self {
                sent_commands: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                responses: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                delay_ms: 10,
                fail_after: None,
            }
        }

        pub const fn with_delay(mut self, delay_ms: u64) -> Self {
            self.delay_ms = delay_ms;
            self
        }

        pub const fn fail_after_n_commands(mut self, n: usize) -> Self {
            self.fail_after = Some(n);
            self
        }

        pub async fn add_response(&self, response: Vec<u8>) {
            let mut responses = self.responses.lock().await;
            responses.push(response);
        }

        pub async fn add_ack_completion(&self, socket: u8) {
            self.add_response(vec![0x90, 0x40 | socket, 0xFF]).await;
            self.add_response(vec![0x90, 0x50 | socket, 0xFF]).await;
        }

        pub async fn command_count(&self) -> usize {
            self.sent_commands.lock().await.len()
        }
    }

    impl AsyncTransport for MockAsyncTransport {
        fn send_command<'a>(
            &'a mut self,
            command: &'a dyn Command,
        ) -> TransportFuture<'a, grafton_visca::Response> {
            Box::pin(async move {
                let count = self.sent_commands.lock().await.len();

                if let Some(fail_after) = self.fail_after {
                    if count >= fail_after {
                        return Err(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::ConnectionAborted,
                            "Simulated connection failure",
                        )));
                    }
                }

                let bytes = command.to_bytes()?;
                self.sent_commands.lock().await.push(bytes);

                sleep(Duration::from_millis(self.delay_ms)).await;

                // Simulate receiving responses
                let mut responses = self.responses.lock().await;

                // If this is an inquiry command, return the first response directly
                if let Some(resp_type) = command.response_type() {
                    if !responses.is_empty() {
                        let response_bytes = responses.remove(0);
                        // Parse the response based on type
                        use grafton_visca::command::parse_response;
                        return parse_response(&response_bytes, &resp_type);
                    }
                    return Err(Error::Timeout);
                }

                // For control commands, simulate ACK then completion/error
                if !responses.is_empty() {
                    let first_response = responses[0].clone();

                    // Check if it's an error response
                    if first_response.len() >= 4
                        && first_response[0] == 0x90
                        && first_response[1] == 0x60
                    {
                        let error_code = first_response[2];
                        responses.remove(0);
                        return Ok(grafton_visca::Response::Error(Error::from_code(error_code)));
                    }

                    // For testing, if we get an ACK, look for completion
                    if first_response.len() == 3
                        && first_response[0] == 0x90
                        && (first_response[1] & 0xF0) == 0x40
                    {
                        // Remove ACK
                        responses.remove(0);

                        // Simulate delay between ACK and completion
                        sleep(Duration::from_millis(self.delay_ms)).await;

                        // Now get completion
                        if !responses.is_empty() {
                            let second_response = responses[0].clone();
                            if second_response.len() >= 4
                                && second_response[0] == 0x90
                                && second_response[1] == 0x60
                            {
                                let error_code = second_response[2];
                                responses.remove(0);
                                return Ok(grafton_visca::Response::Error(Error::from_code(
                                    error_code,
                                )));
                            }
                            if second_response.len() == 3
                                && second_response[0] == 0x90
                                && (second_response[1] & 0xF0) == 0x50
                            {
                                responses.remove(0);
                                return Ok(grafton_visca::Response::Completion);
                            }
                        }
                    }

                    // Direct completion response
                    if first_response.len() == 3
                        && first_response[0] == 0x90
                        && (first_response[1] & 0xF0) == 0x50
                    {
                        responses.remove(0);
                        return Ok(grafton_visca::Response::Completion);
                    }
                }

                Err(Error::Timeout)
            })
        }
    }
}
