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
use crate::{executor::Executor, transport::AsyncTransport};

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
    pub fn with_executor(mut self, executor: Arc<E>) -> Self
    where
        E: Executor + ExecutorExt + 'static,
    {
        self.executor = Some(executor);
        self
    }

    /// Get all commands that were sent to this transport.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.sent
            .lock()
            .expect("ScriptedBlockingTransport mutex poisoned")
            .clone()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        let _ = self.response_tx.send(Ok(response));
    }

    /// Process any pending Step::After steps (static version for use in async blocks).
    fn process_after_steps_static(
        steps: Arc<Mutex<VecDeque<Step>>>,
        response_tx: flume::Sender<Result<Vec<u8>>>,
        executor: Option<Arc<E>>,
    ) where
        E: Executor + ExecutorExt + 'static,
    {
        loop {
            let step = {
                let mut steps_guard = steps
                    .lock()
                    .expect("ScriptedBlockingTransport mutex poisoned");
                // Only process Step::After, leave others alone
                match steps_guard.front() {
                    Some(Step::After { .. }) => steps_guard.pop_front(),
                    _ => None,
                }
            };

            if let Some(Step::After { delay, responses }) = step {
                // Schedule delayed responses if we have an executor
                if let Some(executor) = &executor {
                    let response_tx_clone = response_tx.clone();
                    let executor_clone = executor.clone();
                    // Use spawn_bg for true fire-and-forget semantics
                    executor.spawn_bg(async move {
                        executor_clone.sleep(delay).await;
                        for response in responses {
                            let _ = response_tx_clone.send_async(Ok(response)).await;
                        }
                    });
                } else {
                    // No executor available, send responses immediately
                    for response in responses {
                        let _ = response_tx.send(Ok(response));
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Process any pending Step::After steps.
    /// This is called after a successful Step::OnSend to handle any delayed responses.
    #[allow(dead_code)]
    fn process_after_steps(&self)
    where
        E: Executor + ExecutorExt,
    {
        loop {
            let step = {
                let mut steps = self
                    .steps
                    .lock()
                    .expect("ScriptedBlockingTransport mutex poisoned");

                // Only process Step::After, leave others alone
                match steps.front() {
                    Some(Step::After { .. }) => steps.pop_front(),
                    _ => None,
                }
            };

            if let Some(Step::After { delay, responses }) = step {
                // Schedule delayed responses if we have an executor
                if let Some(executor) = &self.executor {
                    let response_tx = self.response_tx.clone();
                    let executor_clone = executor.clone();

                    // Use spawn_bg for true fire-and-forget semantics
                    executor.spawn_bg(async move {
                        executor_clone.sleep(delay).await;
                        for response in responses {
                            let _ = response_tx.send_async(Ok(response)).await;
                        }
                    });
                } else {
                    // No executor available, send responses immediately
                    for response in responses {
                        let _ = self.response_tx.send(Ok(response));
                    }
                }
            } else {
                break;
            }
        }
    }
}

#[cfg(feature = "async")]
impl<E> AsyncTransport for ScriptedTransport<E>
where
    E: Executor + ExecutorExt + 'static,
{
    fn send(&mut self, bytes: &[u8]) -> impl std::future::Future<Output = Result<()>> + Send {
        let bytes_vec = bytes.to_vec();
        let sent = self.sent.clone();
        let steps = self.steps.clone();
        let response_tx = self.response_tx.clone();
        let executor = self.executor.clone();

        async move {
            // Record the sent command
            sent.lock()
                .expect("ScriptedBlockingTransport mutex poisoned")
                .push(bytes_vec.clone());

            // Process any applicable steps
            let step = {
                let mut steps_guard = steps
                    .lock()
                    .expect("ScriptedBlockingTransport mutex poisoned");
                steps_guard.pop_front()
            };

            if let Some(step) = step {
                match step {
                    Step::OnSend { matches, responses } => {
                        // Check if this send matches the expected pattern
                        let should_respond = matches
                            .as_ref()
                            .map_or(true, |pattern| bytes_vec.starts_with(pattern));

                        if should_respond {
                            // Send responses to the channel immediately
                            for response in responses {
                                let _ = response_tx.send(Ok(response));
                            }
                            // Process any Step::After that follows immediately
                            ScriptedTransport::<E>::process_after_steps_static(
                                steps.clone(),
                                response_tx.clone(),
                                executor.clone(),
                            );
                        } else {
                            // Put the step back if it didn't match
                            steps
                                .lock()
                                .expect("ScriptedBlockingTransport mutex poisoned")
                                .push_front(Step::OnSend { matches, responses });
                        }
                    }
                    Step::After { delay, responses } => {
                        // Put it back to be processed by process_after_steps
                        steps
                            .lock()
                            .expect("ScriptedBlockingTransport mutex poisoned")
                            .push_front(Step::After { delay, responses });
                        // Process it now
                        ScriptedTransport::<E>::process_after_steps_static(
                            steps.clone(),
                            response_tx.clone(),
                            executor.clone(),
                        );
                    }
                    Step::InjectError(error) => {
                        // Put the error step back to be handled on recv
                        steps
                            .lock()
                            .expect("ScriptedBlockingTransport mutex poisoned")
                            .push_front(Step::InjectError(error));
                    }
                }
            }

            Ok(())
        }
    }

    fn recv(&mut self) -> impl std::future::Future<Output = Result<Bytes>> + Send {
        let steps = self.steps.clone();
        let response_rx = self.response_rx.clone();

        async move {
            // Check for injected errors first
            {
                let mut steps_guard = steps
                    .lock()
                    .expect("ScriptedBlockingTransport mutex poisoned");
                if let Some(Step::InjectError(_)) = steps_guard.front() {
                    let error = match steps_guard
                        .pop_front()
                        .expect("No error step available in scripted transport")
                    {
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

            // ARCHITECTURAL NOTE: This implementation has a fundamental issue:
            // - For DeterministicExecutor tests, we need non-blocking behavior (try_recv)
            // - For real async runtime tests, we need blocking behavior (recv_async)
            // 
            // The current implementation uses recv_async which works for real runtimes
            // but cannot be controlled by DeterministicExecutor's virtual time.
            // This means timeout testing with DeterministicExecutor is not possible.
            //
            // Potential solutions:
            // 1. Create separate test transports for deterministic vs real async
            // 2. Add a runtime-aware timeout mechanism using the Executor trait
            // 3. Accept that timeout testing requires real time
            
            // Receive response from channel
            match response_rx.recv_async().await {
                Ok(Ok(response)) => {
                    eprintln!(
                        "[ScriptedTransport::recv] Returning response: {:02X?}",
                        response
                    );
                    Ok(Bytes::from(response))
                }
                Ok(Err(_)) => {
                    eprintln!("[ScriptedTransport::recv] Recv error, returning timeout");
                    Err(Error::Timeout)
                }
                Err(_) => {
                    eprintln!("[ScriptedTransport::recv] Channel closed, returning timeout");
                    Err(Error::Timeout)
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
        self.sent
            .lock()
            .expect("ScriptedBlockingTransport mutex poisoned")
            .clone()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        let _ = self.response_tx.send(Ok(response));
    }
}

impl BlockingTransport for ScriptedBlockingTransport {
    fn send_blocking(&mut self, bytes: &[u8]) -> Result<()> {
        // Record the sent command
        self.sent
            .lock()
            .expect("ScriptedBlockingTransport mutex poisoned")
            .push(bytes.to_vec());

        // Process any applicable steps
        let step = {
            let mut steps = self
                .steps
                .lock()
                .expect("ScriptedBlockingTransport mutex poisoned");
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

    fn recv_blocking(&mut self) -> Result<Bytes> {
        // Try to get a response from the channel (blocking)
        match self.response_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(response)) => Ok(Bytes::from(response)),
            Ok(Err(e)) => Err(e),          // Error from the channel
            Err(_) => Err(Error::Timeout), // Channel timeout or closed
        }
    }

    fn recv_blocking_with_timeout(&mut self, _timeout: Duration) -> Result<Bytes> {
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

    /// Create a NOT EXECUTABLE response for the given socket number (0-7)
    /// Returns error code 0x41 (Command Not Executable, per VISCA spec)
    pub fn not_executable(socket: u8) -> Vec<u8> {
        vec![0x90, 0x60 | (socket & 0x0F), 0x41, VISCA_TERMINATOR]
    }

    /// Create a BUFFER FULL response
    /// Returns error code 0x03 (Command Buffer Full, per VISCA spec)
    /// Note: BufferFull errors don't have a socket assignment as the command wasn't accepted
    pub fn buffer_full(_socket: u8) -> Vec<u8> {
        vec![0x90, 0x60, 0x03, VISCA_TERMINATOR] // No socket bits in 0x60 for buffer full
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

    /// Create an inquiry response (data only, no ACK per VISCA spec)
    pub fn inquiry_response(pattern: Vec<u8>, _socket: u8, data: Vec<u8>) -> Step {
        Step::OnSend {
            matches: Some(pattern),
            responses: vec![data], // No ACK for inquiries per VISCA spec
        }
    }

    /// Create a BUFFER FULL response followed by successful completion after retry
    pub fn buffer_full_then_success(socket: u8) -> Vec<Step> {
        vec![
            Step::OnSend {
                matches: None,
                responses: vec![buffer_full(0)], // BufferFull has no socket assignment
            },
            Step::OnSend {
                matches: None,
                responses: vec![ack(socket), complete(socket)], // Retry succeeds with socket assignment
            },
        ]
    }

    /// Create a NOT EXECUTABLE response followed by successful completion after retry
    pub fn not_executable_then_success(socket: u8) -> Vec<Step> {
        vec![
            Step::OnSend {
                matches: None,
                responses: vec![not_executable(socket)],
            },
            Step::OnSend {
                matches: None,
                responses: vec![ack(socket), complete(socket)],
            },
        ]
    }

    /// Create a sequence of BUFFER FULL responses followed by success
    pub fn buffer_full_sequence_then_success(socket: u8, full_count: usize) -> Vec<Step> {
        let mut steps = Vec::new();

        // Add BUFFER FULL responses
        for _ in 0..full_count {
            steps.push(Step::OnSend {
                matches: None,
                responses: vec![buffer_full(socket)],
            });
        }

        // Add final success response
        steps.push(Step::OnSend {
            matches: None,
            responses: vec![ack(socket), complete(socket)],
        });

        steps
    }

    /// Create a sequence of NOT EXECUTABLE responses followed by success
    pub fn not_executable_sequence_then_success(socket: u8, error_count: usize) -> Vec<Step> {
        let mut steps = Vec::new();

        // Add NOT EXECUTABLE responses
        for _ in 0..error_count {
            steps.push(Step::OnSend {
                matches: None,
                responses: vec![not_executable(socket)],
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
    #[allow(clippy::unwrap_used)]
    fn test_scripted_blocking_transport_basic() {
        let mut transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
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
    #[allow(clippy::unwrap_used)]
    fn test_scripted_blocking_transport_no_response() {
        let mut transport = ScriptedBlockingTransport::new(vec![]);

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
    #[allow(clippy::unwrap_used)]
    async fn test_scripted_async_transport_with_executor() {
        let (executor, _clock) = DeterministicExecutor::new();

        // Test basic functionality without delayed responses for now
        // The After step with spawned tasks requires more complex integration
        // between DeterministicExecutor and async-executor
        let mut transport = ScriptedTransport::new(vec![Step::OnSend {
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

    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
    #[tokio::test(start_paused = true)]
    #[allow(clippy::unwrap_used)]
    async fn test_scripted_transport_immediate_timeout_via_injected_error() {
        use crate::testing::testkit::helpers::errors;
        use crate::{Error, TokioExecutor};
        use std::sync::Arc;

        let exec = Arc::new(TokioExecutor::from_handle(
            tokio::runtime::Handle::current(),
        ));

        // Arrange: first recv() should see a transport-level timeout error
        let mut transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![errors::transport_timeout()]).with_executor(exec);

        // Act: send anything (no response will be produced)
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .await
            .unwrap();

        // Assert: recv yields Err(Timeout) immediately (no hangs)
        let err = transport.recv().await.unwrap_err();
        assert!(matches!(err, Error::Timeout));
    }

    #[cfg(all(test, feature = "test-utils", feature = "async"))]
    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_scripted_transport_delayed_response_with_deterministic_executor() {
        use crate::testing::testkit::deterministic_executor::DeterministicExecutor;
        use crate::testing::testkit::Step;
        use std::time::Duration;

        let (executor, clock) = DeterministicExecutor::new();

        executor.clone().block_on_bg(async move {
            let cmd = vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR];

            let steps = vec![
                Step::OnSend {
                    matches: None,
                    responses: vec![],
                },
                Step::After {
                    delay: Duration::from_millis(100),
                    responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]], // ACK after 100ms
                },
            ];

            let mut transport = ScriptedTransport::new(steps).with_executor(executor.clone());

            // Kick off send, then concurrently wait for recv
            transport.send(&cmd).await.unwrap();

            // Spawn the recv future and advance time to deliver the delayed response
            let recv_fut = transport.recv();
            executor.drive_until_idle();
            clock.advance(Duration::from_millis(100));
            executor.drive_until_idle();

            let bytes = recv_fut.await.expect("Delayed response should arrive");
            assert_eq!(bytes.as_ref(), &[0x90, 0x41, VISCA_TERMINATOR]);
        });
    }
}
