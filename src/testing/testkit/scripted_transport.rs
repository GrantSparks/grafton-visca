//! Scripted transport implementations for deterministic testing.
//!
//! This module provides transport implementations that follow a predetermined script
//! of responses, allowing for deterministic and repeatable test behavior.

#![cfg(feature = "test-utils")]
// Expects in test utilities are intentional for detecting test failures
#![allow(clippy::expect_used)]

use bytes::Bytes;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{transport::BlockingTransport, Error, Result};

#[cfg(feature = "async")]
use crate::{executor_unified::Executor, transport::AsyncTransport};

#[cfg(feature = "async")]
use super::deterministic_executor::ExecutorExt;

/// A step in a transport script defining what should happen when commands are sent.
#[derive(Debug)]
pub enum Step {
    /// Respond immediately when the next send occurs.
    /// If `matches` is provided, only respond if the sent bytes start with those bytes.
    OnSend {
        matches: Option<Vec<u8>>,
        responses: Vec<Vec<u8>>,
    },

    /// Schedule responses after a virtual delay (uses the provided Executor in async mode).
    After {
        delay: Duration,
        responses: Vec<Vec<u8>>,
    },

    /// Inject a transport-level error on the next recv attempt.
    InjectError(Error),
}

impl Clone for Step {
    fn clone(&self) -> Self {
        match self {
            Step::OnSend { matches, responses } => Step::OnSend {
                matches: matches.clone(),
                responses: responses.clone(),
            },
            Step::After { delay, responses } => Step::After {
                delay: *delay,
                responses: responses.clone(),
            },
            Step::InjectError(err) => Step::InjectError(match err {
                Error::Timeout => Error::Timeout,
                Error::ConnectionLost { reason } => Error::ConnectionLost {
                    reason: reason.clone(),
                },
                Error::InvalidState(msg) => Error::InvalidState(msg.clone()),
                Error::TransportError(msg) => Error::TransportError(msg.clone()),
                Error::ParseError(msg) => Error::ParseError(msg.clone()),
                Error::InvalidRequest(msg) => Error::InvalidRequest(msg.clone()),
                Error::MissingRuntime => Error::MissingRuntime,
                Error::CameraBusy => Error::CameraBusy,
                Error::CommandPending => Error::CommandPending,
                Error::CameraNotReady => Error::CameraNotReady,
                Error::SyntaxError => Error::SyntaxError,
                Error::CommandBufferFull => Error::CommandBufferFull,
                Error::CommandCanceled => Error::CommandCanceled,
                Error::NoSocket => Error::NoSocket,
                Error::CommandNotExecutable => Error::CommandNotExecutable,
                Error::Unsupported => Error::Unsupported,
                // For other variants, just create a generic transport error with the display representation
                _ => Error::TransportError(format!("Mock error: {}", err).into()),
            }),
        }
    }
}

/// Async transport that follows a predetermined script of responses.
///
/// This transport allows tests to define exactly what responses should be sent
/// and when, without relying on real network behavior or timing.
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct ScriptedTransport<E = ()> {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    steps: Arc<Mutex<VecDeque<Step>>>,
    response_tx: flume::Sender<Result<Vec<u8>>>,
    response_rx: flume::Receiver<Result<Vec<u8>>>,
    executor: Option<Arc<E>>,
}

#[cfg(feature = "async")]
impl<E> Clone for ScriptedTransport<E> {
    fn clone(&self) -> Self {
        Self {
            sent: self.sent.clone(),
            steps: self.steps.clone(),
            response_tx: self.response_tx.clone(),
            response_rx: self.response_rx.clone(),
            executor: self.executor.clone(),
        }
    }
}

#[cfg(feature = "async")]
impl<E> ScriptedTransport<E> {
    /// Create a new scripted transport with the given steps.
    pub fn new(steps: impl Into<Vec<Step>>) -> Self {
        let (response_tx, response_rx) = flume::unbounded();
        Self {
            sent: Arc::new(Mutex::new(Vec::new())),
            steps: Arc::new(Mutex::new(steps.into().into())),
            response_tx,
            response_rx,
            executor: None,
        }
    }

    /// Add an executor for handling delayed responses.
    /// This will immediately schedule any Step::After steps that exist in the queue.
    pub fn with_executor(mut self, executor: Arc<E>) -> Self
    where
        E: Executor + ExecutorExt + 'static,
    {
        self.executor = Some(executor.clone());

        // Process any Step::After steps immediately
        let mut steps_to_process = Vec::new();
        {
            let mut steps = self.steps.lock().expect("ScriptedBlockingTransport mutex poisoned");
            let mut remaining_steps = VecDeque::new();

            while let Some(step) = steps.pop_front() {
                if let Step::After { delay, responses } = step {
                    steps_to_process.push((delay, responses));
                } else {
                    remaining_steps.push_back(step);
                }
            }

            *steps = remaining_steps;
        }

        // Schedule all After steps
        for (delay, responses) in steps_to_process {
            let response_tx = self.response_tx.clone();
            let executor_clone = executor.clone();

            // Use spawn_bg for true fire-and-forget semantics
            executor.spawn_bg(async move {
                executor_clone.sleep(delay).await;
                for response in responses {
                    let _ = response_tx.send_async(Ok(response)).await;
                }
            });
        }

        self
    }

    /// Get all commands that were sent to this transport.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().expect("ScriptedBlockingTransport mutex poisoned").clone()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        let _ = self.response_tx.send(Ok(response));
    }
}

#[cfg(feature = "async")]
impl<E> AsyncTransport for ScriptedTransport<E>
where
    E: Executor + ExecutorExt + 'static,
{
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        // eprintln!("[ScriptedTransport::send] Sending: {:02X?}", bytes);
        // Record the sent command
        self.sent.lock().expect("ScriptedBlockingTransport mutex poisoned").push(bytes.to_vec());

        // Process any applicable steps
        let step = {
            let mut steps = self.steps.lock().expect("ScriptedBlockingTransport mutex poisoned");
            steps.pop_front()
        };

        if let Some(step) = step {
            match step {
                Step::OnSend { matches, responses } => {
                    // Check if this send matches the expected pattern
                    let should_respond = matches
                        .as_ref()
                        .map_or(true, |pattern| bytes.starts_with(pattern));

                    if should_respond {
                        // Send responses to the channel immediately
                        for response in responses {
                            let _ = self.response_tx.send(Ok(response));
                        }
                    } else {
                        // Put the step back if it didn't match
                        self.steps
                            .lock()
                            .expect("ScriptedBlockingTransport mutex poisoned")
                            .push_front(Step::OnSend { matches, responses });
                    }
                }
                Step::After { delay, responses } => {
                    // Step::After should have been processed in with_executor()
                    // If we encounter it here, it means no executor was set, so put it back
                    self.steps
                        .lock()
                        .expect("ScriptedBlockingTransport mutex poisoned")
                        .push_front(Step::After { delay, responses });
                }
                Step::InjectError(error) => {
                    // Put the error step back to be handled on recv
                    self.steps
                        .lock()
                        .expect("ScriptedBlockingTransport mutex poisoned")
                        .push_front(Step::InjectError(error));
                }
            }
        }

        Ok(())
    }

    async fn recv(&self) -> Result<Bytes> {
        // eprintln!("[ScriptedTransport::recv] Called");
        // Check for injected errors first
        {
            let mut steps = self.steps.lock().expect("ScriptedBlockingTransport mutex poisoned");
            if let Some(Step::InjectError(_)) = steps.front() {
                let error = match steps.pop_front().expect("No error step available in scripted transport") {
                    Step::InjectError(e) => e,
                    _ => unreachable!(),
                };
                eprintln!(
                    "[ScriptedTransport::recv] Returning injected error: {:?}",
                    error
                );
                return Err(error);
            }
        }

        // Use event-driven channel receive with timeout
        if let Some(executor) = &self.executor {
            // With executor, use its timeout mechanism
            match executor
                .timeout(Duration::from_secs(10), self.response_rx.recv_async())
                .await
            {
                Ok(Ok(Ok(response))) => {
                    eprintln!(
                        "[ScriptedTransport::recv] Returning response: {:02X?}",
                        response
                    );
                    Ok(Bytes::from(response))
                }
                Ok(Ok(Err(_))) => {
                    eprintln!("[ScriptedTransport::recv] Channel closed, returning timeout");
                    Err(Error::Timeout)
                } // Channel closed
                Ok(Err(_)) => {
                    eprintln!("[ScriptedTransport::recv] Recv error, returning timeout");
                    Err(Error::Timeout)
                } // Recv error
                Err(_) => {
                    eprintln!("[ScriptedTransport::recv] Timeout from executor");
                    Err(Error::Timeout)
                } // Timeout from executor
            }
        } else {
            // Without executor, use tokio timeout
            #[cfg(feature = "rt-tokio")]
            match tokio::time::timeout(Duration::from_secs(10), self.response_rx.recv_async()).await
            {
                Ok(Ok(Ok(response))) => Ok(Bytes::from(response)),
                Ok(Ok(Err(_))) => Err(Error::Timeout), // Channel closed
                Ok(Err(_)) => Err(Error::Timeout),     // Recv error
                Err(_) => Err(Error::Timeout),         // Timeout
            }

            #[cfg(not(feature = "rt-tokio"))]
            {
                // Fallback for tests without tokio - just try receive
                match self.response_rx.recv_async().await {
                    Ok(Ok(response)) => Ok(Bytes::from(response)),
                    Ok(Err(_)) => Err(Error::Timeout),
                    Err(_) => Err(Error::Timeout),
                }
            }
        }
    }
}

/// Blocking transport that follows a predetermined script of responses.
///
/// This is the blocking version of ScriptedTransport, designed for testing
/// blocking transport implementations.
#[derive(Clone, Debug)]
pub struct ScriptedBlockingTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    steps: Arc<Mutex<VecDeque<Step>>>,
    response_tx: flume::Sender<Result<Vec<u8>>>,
    response_rx: flume::Receiver<Result<Vec<u8>>>,
}

impl ScriptedBlockingTransport {
    /// Create a new scripted blocking transport with the given steps.
    pub fn new(steps: impl Into<Vec<Step>>) -> Self {
        let (response_tx, response_rx) = flume::unbounded();
        Self {
            sent: Arc::new(Mutex::new(Vec::new())),
            steps: Arc::new(Mutex::new(steps.into().into())),
            response_tx,
            response_rx,
        }
    }

    /// Get all commands that were sent to this transport.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().expect("ScriptedBlockingTransport mutex poisoned").clone()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        let _ = self.response_tx.send(Ok(response));
    }
}

impl BlockingTransport for ScriptedBlockingTransport {
    fn send_blocking(&self, bytes: &[u8]) -> Result<()> {
        // Record the sent command
        self.sent.lock().expect("ScriptedBlockingTransport mutex poisoned").push(bytes.to_vec());

        // Process any applicable steps
        let step = {
            let mut steps = self.steps.lock().expect("ScriptedBlockingTransport mutex poisoned");
            steps.pop_front()
        };

        if let Some(step) = step {
            match step {
                Step::OnSend { matches, responses } => {
                    // Check if this send matches the expected pattern
                    let should_respond = matches
                        .as_ref()
                        .map_or(true, |pattern| bytes.starts_with(pattern));

                    if should_respond {
                        // Send responses to the channel immediately
                        for response in responses {
                            let _ = self.response_tx.send(Ok(response));
                        }
                    } else {
                        // Put the step back if it didn't match
                        self.steps
                            .lock()
                            .expect("ScriptedBlockingTransport mutex poisoned")
                            .push_front(Step::OnSend { matches, responses });
                    }
                }
                Step::After { responses, .. } => {
                    // In blocking mode, we can't wait for delays, so just queue responses immediately
                    // The delay behavior will be handled by the calling code's timeout logic
                    for response in responses {
                        let _ = self.response_tx.send(Ok(response));
                    }
                }
                Step::InjectError(error) => {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        // Try to get a response from the channel (blocking)
        match self.response_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(response)) => Ok(Bytes::from(response)),
            Ok(Err(e)) => Err(e),          // Error from the channel
            Err(_) => Err(Error::Timeout), // Channel timeout or closed
        }
    }

    fn recv_blocking_with_timeout(&self, _timeout: Duration) -> Result<Bytes> {
        // For the scripted transport, we just use the same logic as recv_blocking
        // The timeout is handled by the script itself
        self.recv_blocking()
    }
}

/// Helper functions for creating common VISCA response patterns
pub mod helpers {
    use super::Step;

    /// VISCA terminator byte
    pub const VISCA_TERMINATOR: u8 = 0xFF;

    /// Create an ACK response for the given socket number (0-7)
    pub fn ack(socket: u8) -> Vec<u8> {
        vec![0x90, 0x40 | (socket & 0x0F), VISCA_TERMINATOR]
    }

    /// Create a completion response for the given socket number (0-7)
    pub fn complete(socket: u8) -> Vec<u8> {
        vec![0x90, 0x50 | (socket & 0x0F), VISCA_TERMINATOR]
    }

    /// Create a BUSY response for the given socket number (0-7)
    /// Returns error code 0x41 (Command Not Executable - camera busy, per VISCA spec)
    pub fn busy(socket: u8) -> Vec<u8> {
        vec![0x90, 0x60 | (socket & 0x0F), 0x41, VISCA_TERMINATOR]
    }

    /// Create a standard command response (ACK followed by completion)
    pub fn standard_command_response(socket: u8) -> Step {
        Step::OnSend {
            matches: None,
            responses: vec![ack(socket), complete(socket)],
        }
    }

    /// Create a standard command response matching a specific command pattern
    pub fn command_response(pattern: Vec<u8>, socket: u8) -> Step {
        Step::OnSend {
            matches: Some(pattern),
            responses: vec![ack(socket), complete(socket)],
        }
    }

    /// Create an inquiry response (ACK followed by data)
    pub fn inquiry_response(pattern: Vec<u8>, socket: u8, data: Vec<u8>) -> Step {
        Step::OnSend {
            matches: Some(pattern),
            responses: vec![ack(socket), data],
        }
    }

    /// Create a BUSY response followed by successful completion after retry
    pub fn busy_then_success(socket: u8) -> Vec<Step> {
        vec![
            Step::OnSend {
                matches: None,
                responses: vec![busy(socket)],
            },
            Step::OnSend {
                matches: None,
                responses: vec![ack(socket), complete(socket)],
            },
        ]
    }

    /// Create a sequence of BUSY responses followed by success
    pub fn busy_sequence_then_success(socket: u8, busy_count: usize) -> Vec<Step> {
        let mut steps = Vec::new();

        // Add BUSY responses
        for _ in 0..busy_count {
            steps.push(Step::OnSend {
                matches: None,
                responses: vec![busy(socket)],
            });
        }

        // Add final success response
        steps.push(Step::OnSend {
            matches: None,
            responses: vec![ack(socket), complete(socket)],
        });

        steps
    }

    /// Auto-response mode: generate appropriate response for any VISCA command
    /// This is useful for tests that don't care about specific command details
    pub fn auto_respond_step() -> Step {
        Step::OnSend {
            matches: None,
            responses: vec![ack(1), complete(1)], // Default to socket 1
        }
    }

    /// Create an ACK response with Sony envelope for the given socket number (0-7)
    pub fn sony_ack(socket: u8) -> Vec<u8> {
        vec![
            0x01,
            0x11,
            0x00,
            0x03,
            0x00,
            0x00,
            0x00,
            0x00, // Sony header
            0x90,
            0x40 | (socket & 0x0F),
            VISCA_TERMINATOR,
        ]
    }

    /// Create a completion response with Sony envelope for the given socket number (0-7)
    pub fn sony_complete(socket: u8) -> Vec<u8> {
        vec![
            0x01,
            0x11,
            0x00,
            0x03,
            0x00,
            0x00,
            0x00,
            0x00, // Sony header
            0x90,
            0x50 | (socket & 0x0F),
            VISCA_TERMINATOR,
        ]
    }

    /// Auto-response mode for Sony cameras: generate Sony envelope responses
    pub fn sony_auto_respond_step() -> Step {
        Step::OnSend {
            matches: None,
            responses: vec![sony_ack(1), sony_complete(1)], // Default to socket 1
        }
    }

    /// Create a power inquiry response
    pub fn power_inquiry_response(power_on: bool) -> Step {
        let data = if power_on {
            vec![0x90, 0x50, 0x02, VISCA_TERMINATOR] // Power on
        } else {
            vec![0x90, 0x50, 0x03, VISCA_TERMINATOR] // Power off
        };

        inquiry_response(
            vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR], // Power inquiry command
            1,
            data,
        )
    }

    /// Common error responses
    pub mod errors {
        use super::{Step, VISCA_TERMINATOR};
        use crate::Error;

        pub fn syntax_error(socket: u8) -> Step {
            Step::OnSend {
                matches: None,
                responses: vec![vec![0x90, 0x60 | socket, 0x02, VISCA_TERMINATOR]],
            }
        }

        pub fn command_buffer_full(socket: u8) -> Step {
            Step::OnSend {
                matches: None,
                responses: vec![vec![0x90, 0x60 | socket, 0x03, VISCA_TERMINATOR]],
            }
        }

        pub fn command_canceled(socket: u8) -> Step {
            Step::OnSend {
                matches: None,
                responses: vec![vec![0x90, 0x60 | socket, 0x04, VISCA_TERMINATOR]],
            }
        }

        pub fn no_socket() -> Step {
            Step::OnSend {
                matches: None,
                responses: vec![vec![0x90, 0x60, 0x05, VISCA_TERMINATOR]],
            }
        }

        pub fn transport_timeout() -> Step {
            Step::InjectError(Error::Timeout)
        }

        pub fn connection_lost() -> Step {
            Step::InjectError(Error::ConnectionLost {
                reason: "Test connection lost".into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::const_encoding::VISCA_TERMINATOR;

    #[cfg(feature = "async")]
    use crate::testing::testkit::DeterministicExecutor;

    #[test]
    fn test_scripted_blocking_transport_basic() {
        let transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]], // ACK
        }]);

        // Send a command
        transport
            .send_blocking(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .unwrap();

        // Should receive the scripted response
        let response = transport.recv_blocking().unwrap();
        assert_eq!(response.as_ref(), &[0x90, 0x41, VISCA_TERMINATOR]);

        // Verify the command was recorded
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR]);
    }

    #[test]
    fn test_scripted_blocking_transport_no_response() {
        let transport = ScriptedBlockingTransport::new(vec![]);

        // Send a command
        transport
            .send_blocking(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .unwrap();

        // Should timeout since no response is scripted
        let result = transport.recv_blocking();
        assert!(matches!(result, Err(Error::Timeout)));
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_scripted_async_transport_with_executor() {
        let (executor, _clock) = DeterministicExecutor::new();

        // Test basic functionality without delayed responses for now
        // The After step with spawned tasks requires more complex integration
        // between DeterministicExecutor and async-executor
        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]], // ACK immediately
        }])
        .with_executor(executor.clone());

        // Send a command
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .await
            .unwrap();

        // Should receive the response immediately
        let response = transport.recv().await.unwrap();
        assert_eq!(response.as_ref(), &[0x90, 0x41, VISCA_TERMINATOR]);

        // Verify the command was recorded
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR]);
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_scripted_transport_delayed_response_manual() {
        let (executor, _clock) = DeterministicExecutor::new();
        let transport = ScriptedTransport::new(vec![]).with_executor(executor.clone());

        // Send a command without any scripted responses
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .await
            .unwrap();

        // Initially no response should be available
        let result = transport.recv().await;
        assert!(matches!(result, Err(Error::Timeout)));

        // Manually add a response (simulating a delayed response)
        transport.add_response(vec![0x90, 0x41, VISCA_TERMINATOR]);

        // Now the response should be available
        let response = transport.recv().await.unwrap();
        assert_eq!(response.as_ref(), &[0x90, 0x41, VISCA_TERMINATOR]);
    }
}
