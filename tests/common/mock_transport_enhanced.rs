//! Enhanced mock transport implementation for testing.
//!
//! Provides a highly configurable mock transport that can simulate various
//! camera behaviors and network conditions for comprehensive testing.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use grafton_visca::transport::{BlockingTransport, Transport};
use grafton_visca::{Error, Result};
use std::future::Ready;

/// A mock transport for testing VISCA communication.
///
/// This transport records all sent commands and provides configurable responses,
/// making it easy to test camera control logic without real hardware.
#[derive(Clone)]
pub struct MockTransport {
    inner: Arc<Mutex<MockTransportInner>>,
}

struct MockTransportInner {
    /// Expected command sequences
    expectations: VecDeque<TransportExpectation>,

    /// History of all sent commands
    sent_history: Vec<(Instant, Vec<u8>)>,

    /// History of all responses sent
    response_history: Vec<Vec<u8>>,

    /// Queued responses to return
    response_queue: VecDeque<MockResponse>,

    /// Whether the transport is "connected"
    connected: bool,

    /// Optional latency to simulate
    latency: Option<Duration>,

    /// Whether to validate VISCA frame format
    validate_frames: bool,

    /// Current expectation index for better error messages
    current_expectation: usize,
}

/// A single transport expectation
pub struct TransportExpectation {
    /// Expected command bytes
    pub command: Vec<u8>,

    /// Responses to queue when this command is received
    pub responses: Vec<MockResponse>,

    /// Whether this expectation has been met
    pub met: bool,

    /// Optional description for better error messages
    pub description: Option<String>,
}

/// A mock response to return
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum MockResponse {
    /// Return these bytes immediately
    Immediate(Vec<u8>),

    /// Return these bytes after a delay
    Delayed(Vec<u8>, Duration),

    /// Return an error code (as VISCA error response)
    ErrorCode(u8),

    /// Timeout (no response)
    Timeout,
}

#[allow(dead_code)]
impl MockTransport {
    /// Create a new mock transport
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MockTransportInner {
                expectations: VecDeque::new(),
                sent_history: Vec::new(),
                response_history: Vec::new(),
                response_queue: VecDeque::new(),
                connected: true,
                latency: None,
                validate_frames: true,
                current_expectation: 0,
            })),
        }
    }

    /// Convenience method for tests - send data synchronously
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        // Use futures::executor to block on the future
        futures::executor::block_on(Transport::send(self, data))
    }

    /// Convenience method for tests - receive data with timeout
    pub fn receive(&mut self, _timeout: Duration) -> Result<Vec<u8>> {
        // Just use the recv method which already handles everything
        match futures::executor::block_on(Transport::recv(self)) {
            Ok(bytes) => Ok(bytes.to_vec()),
            Err(e) => Err(e),
        }
    }

    /// Check if transport is connected
    pub fn is_connected(&self) -> bool {
        self.inner.lock().unwrap().connected
    }

    /// Create a mock transport using the builder pattern
    pub fn builder() -> MockTransportBuilder {
        MockTransportBuilder::new()
    }

    /// Expect a specific command to be sent
    pub fn expect_command(&mut self, command: &[u8]) -> &mut TransportExpectation {
        let mut inner = self.inner.lock().unwrap();
        inner.expectations.push_back(TransportExpectation {
            command: command.to_vec(),
            responses: Vec::new(),
            met: false,
            description: None,
        });

        // Return a mutable reference through unsafe pointer manipulation
        // This is safe because we hold the mutex lock
        let expectation_ptr = inner.expectations.back_mut().unwrap() as *mut TransportExpectation;
        drop(inner); // Release the lock
        unsafe { &mut *expectation_ptr }
    }

    /// Queue a response to return (without expectation)
    pub fn queue_response(&mut self, response: MockResponse) {
        let mut inner = self.inner.lock().unwrap();
        inner.response_queue.push_back(response);
    }

    /// Set simulated network latency
    pub fn set_latency(&mut self, latency: Duration) {
        self.inner.lock().unwrap().latency = Some(latency);
    }

    /// Simulate a disconnection
    pub fn disconnect(&mut self) {
        self.inner.lock().unwrap().connected = false;
    }

    /// Get the history of sent commands
    pub fn sent_history(&self) -> Vec<Vec<u8>> {
        self.inner
            .lock()
            .unwrap()
            .sent_history
            .iter()
            .map(|(_, cmd)| cmd.clone())
            .collect()
    }

    /// Get the history of responses sent
    pub fn response_history(&self) -> Vec<Vec<u8>> {
        self.inner.lock().unwrap().response_history.clone()
    }

    /// Verify all expectations were met
    pub fn verify(&self) -> Result<()> {
        let inner = self.inner.lock().unwrap();

        for (i, expectation) in inner.expectations.iter().enumerate() {
            if !expectation.met {
                let desc = expectation.description.as_deref().unwrap_or("");
                return Err(Error::InvalidState(format!(
                    "Expectation {} not met: expected command {:02X?} {}",
                    i + 1,
                    expectation.command,
                    desc
                )));
            }
        }

        Ok(())
    }
}

#[allow(dead_code)]
impl TransportExpectation {
    /// Expect an ACK response with the given socket number
    pub fn will_ack(&mut self, socket: u8) -> &mut Self {
        self.responses
            .push(MockResponse::Immediate(vec![0x90, 0x40 | socket, 0xFF]));
        self
    }

    /// Expect a completion response after ACK
    pub fn then_complete(&mut self, socket: u8) -> &mut Self {
        self.responses
            .push(MockResponse::Immediate(vec![0x90, 0x50 | socket, 0xFF]));
        self
    }

    /// Expect an error response
    pub fn will_error(&mut self, error_code: u8) -> &mut Self {
        self.responses.push(MockResponse::ErrorCode(error_code));
        self
    }

    /// Expect an inquiry data response
    pub fn will_return_data(&mut self, data: &[u8]) -> &mut Self {
        let mut response = vec![0x90, 0x50];
        response.extend_from_slice(data);
        response.push(0xFF);
        self.responses.push(MockResponse::Immediate(response));
        self
    }

    /// Add a custom response
    pub fn will_respond(&mut self, response: MockResponse) -> &mut Self {
        self.responses.push(response);
        self
    }

    /// Add a description for better error messages
    pub fn described_as(&mut self, description: &str) -> &mut Self {
        self.description = Some(description.to_string());
        self
    }
}

impl Transport for MockTransport {
    type Error = Error;
    type SendFut<'a>
        = Ready<Result<(), Self::Error>>
    where
        Self: 'a;
    type RecvFut<'a>
        = Ready<Result<Bytes, Self::Error>>
    where
        Self: 'a;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        let mut inner = self.inner.lock().unwrap();

        if !inner.connected {
            return std::future::ready(Err(Error::ConnectionLost {
                reason: "Mock transport disconnected".to_string(),
            }));
        }

        // Validate frame format if enabled
        if inner.validate_frames && data.len() >= 3 {
            if data[0] & 0xF0 != 0x80 {
                return std::future::ready(Err(Error::InvalidState(format!(
                    "Invalid address byte: {:02X}",
                    data[0]
                ))));
            }
            if data[data.len() - 1] != 0xFF {
                return std::future::ready(Err(Error::InvalidState(
                    "Missing terminator FF".to_string(),
                )));
            }
        }

        // Record the command
        inner.sent_history.push((Instant::now(), data.to_vec()));

        // Check expectations
        let current_idx = inner.current_expectation;
        let mut responses_to_queue = Vec::new();

        if let Some(expectation) = inner.expectations.get_mut(current_idx) {
            if expectation.command == data {
                expectation.met = true;

                // Collect responses to queue
                responses_to_queue = expectation.responses.clone();
            }
        }

        // Queue the responses after releasing the borrow
        if !responses_to_queue.is_empty() {
            for response in responses_to_queue {
                inner.response_queue.push_back(response);
            }
            inner.current_expectation += 1;
        }

        // Simulate latency
        if let Some(latency) = inner.latency {
            std::thread::sleep(latency);
        }

        std::future::ready(Ok(()))
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        let mut inner = self.inner.lock().unwrap();

        if !inner.connected {
            return std::future::ready(Err(Error::ConnectionLost {
                reason: "Mock transport disconnected".to_string(),
            }));
        }

        if let Some(response) = inner.response_queue.pop_front() {
            let result = match response {
                MockResponse::Immediate(data) => {
                    inner.response_history.push(data.clone());
                    Ok(Bytes::from(data))
                }
                MockResponse::Delayed(data, _delay) => {
                    // For blocking transport, we ignore delay in recv
                    inner.response_history.push(data.clone());
                    Ok(Bytes::from(data))
                }
                MockResponse::ErrorCode(code) => {
                    let error_response = vec![0x90, 0x60, code, 0xFF];
                    inner.response_history.push(error_response.clone());
                    Ok(Bytes::from(error_response))
                }
                MockResponse::Timeout => Err(Error::Timeout),
            };

            return std::future::ready(result);
        }

        // No response queued
        std::future::ready(Err(Error::Timeout))
    }
}

impl BlockingTransport for MockTransport {}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.inner.lock().unwrap();
        f.debug_struct("MockTransport")
            .field("connected", &inner.connected)
            .field("expectations", &inner.expectations.len())
            .field("sent_count", &inner.sent_history.len())
            .field("queued_responses", &inner.response_queue.len())
            .finish()
    }
}

/// Builder for creating configured mock transports
pub struct MockTransportBuilder {
    transport: MockTransport,
}

#[allow(dead_code)]
impl MockTransportBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            transport: MockTransport::new(),
        }
    }

    /// Set the initial connection state
    pub fn connected(self, connected: bool) -> Self {
        self.transport.inner.lock().unwrap().connected = connected;
        self
    }

    /// Set simulated network latency
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.transport.set_latency(latency);
        self
    }

    /// Disable VISCA frame validation
    pub fn no_validation(self) -> Self {
        self.transport.inner.lock().unwrap().validate_frames = false;
        self
    }

    /// Add an expectation
    pub fn expect(self, command: &[u8], responses: Vec<MockResponse>) -> Self {
        {
            let mut inner = self.transport.inner.lock().unwrap();
            inner.expectations.push_back(TransportExpectation {
                command: command.to_vec(),
                responses,
                met: false,
                description: None,
            });
        }
        self
    }

    /// Build the mock transport
    pub fn build(self) -> MockTransport {
        self.transport
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_mock_transport() {
        let mut mock = MockTransport::new();

        // Set up expectation
        mock.expect_command(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
            .will_ack(1)
            .then_complete(1);

        // Send the expected command
        futures::executor::block_on(Transport::send(
            &mock,
            &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
        ))
        .unwrap();

        // Receive the responses
        let ack = mock.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(ack, vec![0x90, 0x41, 0xFF]);

        let complete = mock.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(complete, vec![0x90, 0x51, 0xFF]);

        // Verify expectations met
        mock.verify().unwrap();
    }

    #[test]
    fn test_unmet_expectation() {
        let mut mock = MockTransport::new();

        mock.expect_command(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
            .described_as("power on command");

        // Don't send the command

        // Verify should fail
        assert!(mock.verify().is_err());
    }
}
