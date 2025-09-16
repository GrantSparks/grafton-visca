//! Scripted transport implementations for deterministic testing.
//!
//! This module provides transport implementations that follow a predetermined script
//! of responses, allowing for deterministic and repeatable test behavior.

#![cfg(feature = "test-utils")]
// Expects in test utilities are intentional for detecting test failures
#![allow(clippy::expect_used)]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(feature = "mode-async")]
use super::deterministic_executor::ExecutorExt;
#[cfg(feature = "mode-async")]
use crate::transport::{builder::TransportConfig, HasTransportConfig};
#[cfg(not(feature = "mode-async"))]
use crate::{
    command::CommandKind,
    transport::{builder::TransportConfig, HasTransportConfig, SyncTransport},
};
#[cfg(feature = "mode-async")]
use crate::{executor::Executor, transport::AsyncTransport};
use crate::{Error, Result};

/// Function type for dynamic response generation.
pub type DynamicResponseFn = Box<dyn Fn(&[u8]) -> Vec<Vec<u8>> + Send + Sync>;

/// A step in a transport script defining what should happen when commands are sent.
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

    /// Generate responses dynamically based on the sent command bytes.
    /// The function receives the sent bytes and returns responses to send back.
    DynamicResponse(DynamicResponseFn),
}

impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::OnSend { matches, responses } => f
                .debug_struct("OnSend")
                .field("matches", matches)
                .field("responses", responses)
                .finish(),
            Step::After { delay, responses } => f
                .debug_struct("After")
                .field("delay", delay)
                .field("responses", responses)
                .finish(),
            Step::InjectError(err) => f.debug_tuple("InjectError").field(err).finish(),
            Step::DynamicResponse(_) => f.write_str("DynamicResponse(<function>)"),
        }
    }
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
                Error::ConnectionClosed { reason } => Error::ConnectionClosed {
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
                _ => Error::TransportError(format!("Mock error: {err}").into()),
            }),
            Step::DynamicResponse(_) => Step::InjectError(Error::InvalidState(
                "DynamicResponse steps cannot be cloned".into(),
            )),
        }
    }
}

/// Async transport that follows a predetermined script of responses.
///
/// This transport allows tests to define exactly what responses should be sent
/// and when, without relying on real network behavior or timing.
#[cfg(feature = "mode-async")]
#[derive(Debug)]
pub struct ScriptedTransport<E = ()> {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    steps: Arc<Mutex<VecDeque<Step>>>,
    response_tx: flume::Sender<Result<Vec<u8>>>,
    response_rx: flume::Receiver<Result<Vec<u8>>>,
    executor: Option<Arc<E>>,
    shutdown_rx: Option<flume::Receiver<()>>,
}

#[cfg(feature = "mode-async")]
impl<E> Clone for ScriptedTransport<E> {
    fn clone(&self) -> Self {
        Self {
            sent: self.sent.clone(),
            steps: self.steps.clone(),
            response_tx: self.response_tx.clone(),
            response_rx: self.response_rx.clone(),
            executor: self.executor.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
        }
    }
}

#[cfg(feature = "mode-async")]
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
            shutdown_rx: None,
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

    /// Add a shutdown receiver to cleanly exit on shutdown signal.
    pub fn with_shutdown(mut self, shutdown_rx: flume::Receiver<()>) -> Self {
        self.shutdown_rx = Some(shutdown_rx);
        self
    }

    /// Get all commands that were sent to this transport.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.sent
            .lock()
            .expect("ScriptedSyncTransport mutex poisoned")
            .clone()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        let _ = self.response_tx.send(Ok(response));
    }

    /// Schedule a response to be delivered after `delay`.
    /// This provides a convenient way to add delayed responses without
    /// having to pre-script them in the constructor.
    #[cfg(feature = "mode-async")]
    pub fn add_after(&self, delay: Duration, response: Vec<u8>)
    where
        E: Executor + ExecutorExt + 'static,
    {
        if let Some(executor) = &self.executor {
            let tx = self.response_tx.clone();
            let exec = executor.clone();
            let exec_clone = exec.clone();
            exec.spawn_detached(async move {
                exec_clone.sleep(delay).await;
                let _ = tx.send_async(Ok(response)).await;
            });
        } else {
            // Without an executor, deliver immediately (consistent with Step::After fallback).
            let _ = self.response_tx.send(Ok(response));
        }
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
                let mut steps_guard = steps.lock().expect("ScriptedSyncTransport mutex poisoned");
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
                    // Use spawn_detached for true fire-and-forget semantics
                    executor.spawn_detached(async move {
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
}

#[cfg(feature = "mode-async")]
impl<E> AsyncTransport for ScriptedTransport<E>
where
    E: Executor + ExecutorExt + 'static,
{
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        let bytes_vec = bytes.to_vec();
        let sent = self.sent.clone();
        let steps = self.steps.clone();
        let response_tx = self.response_tx.clone();
        let executor = self.executor.clone();

        // Record the sent command
        sent.lock()
            .expect("ScriptedSyncTransport mutex poisoned")
            .push(bytes_vec.clone());

        // First, drain any leading After steps
        ScriptedTransport::<E>::process_after_steps_static(
            steps.clone(),
            response_tx.clone(),
            executor.clone(),
        );

        // Only act on the *front* of the queue. Never reorder past InjectError (leave for recv*())
        let step = {
            let mut guard = steps.lock().expect("ScriptedSyncTransport mutex poisoned");
            match guard.front() {
                Some(Step::InjectError(_)) => None, // leave it for recv*()
                Some(Step::After { .. }) => None,   // already handled by process_after_steps_static
                Some(Step::OnSend { .. }) | Some(Step::DynamicResponse(_)) => guard.pop_front(),
                None => None,
            }
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
                        // Process any After steps that may now be at the front
                        ScriptedTransport::<E>::process_after_steps_static(
                            steps.clone(),
                            response_tx.clone(),
                            executor.clone(),
                        );
                    } else {
                        steps
                            .lock()
                            .expect("ScriptedSyncTransport mutex poisoned")
                            .push_front(Step::OnSend { matches, responses });
                    }
                }
                Step::After { .. } => {
                    unreachable!("After steps are handled by process_after_steps_static");
                }
                Step::InjectError(_) => {
                    unreachable!("InjectError is never popped in send()");
                }
                Step::DynamicResponse(func) => {
                    let responses = func(&bytes_vec);
                    for response in responses {
                        let _ = response_tx.send(Ok(response));
                    }
                    // After a dynamic response, After steps may now lead.
                    ScriptedTransport::<E>::process_after_steps_static(
                        steps.clone(),
                        response_tx.clone(),
                        executor.clone(),
                    );
                }
            }
        }

        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize> {
        let steps = self.steps.clone();
        let response_rx = self.response_rx.clone();

        // Check for injected errors first
        {
            let mut steps_guard = steps.lock().expect("ScriptedSyncTransport mutex poisoned");
            if let Some(Step::InjectError(_)) = steps_guard.front() {
                let error = match steps_guard
                    .pop_front()
                    .expect("No error step available in scripted transport")
                {
                    Step::InjectError(e) => e,
                    _ => unreachable!(),
                };
                return Err(error);
            }
        }

        // Wait for response using recv_async, with optional shutdown handling
        use futures_lite::future;

        let next = async {
            match response_rx.recv_async().await {
                Ok(Ok(bytes)) => Ok(bytes),
                Ok(Err(_)) => Err(Error::Timeout),
                Err(_) => Err(Error::Timeout),
            }
        };

        let outcome = if let Some(shutdown_rx) = &self.shutdown_rx {
            // Race between data reception and shutdown signal
            future::race(
                async {
                    let _ = shutdown_rx.recv_async().await;
                    Err(Error::ConnectionClosed {
                        reason: Some("Shutdown signal received".into()),
                    })
                },
                next,
            )
            .await
        } else {
            next.await
        }?;

        // Copy response data into the provided buffer
        let len = outcome.len().min(dst.len());
        dst[..len].copy_from_slice(&outcome[..len]);
        Ok(len)
    }
}

/// Blocking transport that follows a predetermined script of responses.
///
/// This is the blocking version of ScriptedTransport, designed for testing
/// blocking transport implementations.
#[cfg(not(feature = "mode-async"))]
#[derive(Clone, Debug)]
pub struct ScriptedSyncTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    steps: Arc<Mutex<VecDeque<Step>>>,
    response_tx: flume::Sender<Result<Vec<u8>>>,
    response_rx: flume::Receiver<Result<Vec<u8>>>,
    transport_config: TransportConfig,
}

#[cfg(not(feature = "mode-async"))]
impl ScriptedSyncTransport {
    /// Create a new scripted blocking transport with the given steps.
    pub fn new(steps: impl Into<Vec<Step>>) -> Self {
        let (response_tx, response_rx) = flume::unbounded();
        Self {
            sent: Arc::new(Mutex::new(Vec::new())),
            steps: Arc::new(Mutex::new(steps.into().into())),
            response_tx,
            response_rx,
            transport_config: TransportConfig::default(),
        }
    }

    /// Get all commands that were sent to this transport.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.sent
            .lock()
            .expect("ScriptedSyncTransport mutex poisoned")
            .clone()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        let _ = self.response_tx.send(Ok(response));
    }
}

#[cfg(not(feature = "mode-async"))]
impl SyncTransport for ScriptedSyncTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<()> {
        // Record the sent command
        self.sent
            .lock()
            .expect("ScriptedSyncTransport mutex poisoned")
            .push(bytes.to_vec());

        // Drain *leading* After steps first (sync flavor ignores delay)
        loop {
            let next = {
                let mut steps = self
                    .steps
                    .lock()
                    .expect("ScriptedSyncTransport mutex poisoned");
                match steps.front() {
                    Some(Step::After { .. }) => steps.pop_front(),
                    _ => None,
                }
            };
            match next {
                Some(Step::After { responses, .. }) => {
                    for response in responses {
                        let _ = self.response_tx.send(Ok(response));
                    }
                }
                _ => break,
            }
        }

        // Only look at the front; never pop InjectError here
        let step = {
            let mut steps = self
                .steps
                .lock()
                .expect("ScriptedSyncTransport mutex poisoned");
            match steps.front() {
                Some(Step::InjectError(_)) => None,
                Some(Step::After { .. }) => None,
                Some(Step::OnSend { .. }) | Some(Step::DynamicResponse(_)) => steps.pop_front(),
                None => None,
            }
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
                        // After consuming OnSend, drain any newly-leading After steps
                        loop {
                            let next = {
                                let mut steps = self
                                    .steps
                                    .lock()
                                    .expect("ScriptedSyncTransport mutex poisoned");
                                match steps.front() {
                                    Some(Step::After { .. }) => steps.pop_front(),
                                    _ => None,
                                }
                            };
                            match next {
                                Some(Step::After { responses, .. }) => {
                                    for response in responses {
                                        let _ = self.response_tx.send(Ok(response));
                                    }
                                }
                                _ => break,
                            }
                        }
                    } else {
                        self.steps
                            .lock()
                            .expect("ScriptedSyncTransport mutex poisoned")
                            .push_front(Step::OnSend { matches, responses });
                    }
                }
                Step::DynamicResponse(func) => {
                    let responses = func(bytes);
                    for response in responses {
                        let _ = self.response_tx.send(Ok(response));
                    }
                    // Drain any leading After now visible
                    loop {
                        let next = {
                            let mut steps = self
                                .steps
                                .lock()
                                .expect("ScriptedSyncTransport mutex poisoned");
                            match steps.front() {
                                Some(Step::After { .. }) => steps.pop_front(),
                                _ => None,
                            }
                        };
                        match next {
                            Some(Step::After { responses, .. }) => {
                                for response in responses {
                                    let _ = self.response_tx.send(Ok(response));
                                }
                            }
                            _ => break,
                        }
                    }
                }
                Step::After { .. } => {
                    unreachable!("After steps are handled before main match block");
                }
                Step::InjectError(_) => {
                    unreachable!("InjectError is never popped in send_with_kind()");
                }
            }
        }

        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize> {
        // Check for injected errors first
        {
            let mut steps = self
                .steps
                .lock()
                .expect("ScriptedSyncTransport mutex poisoned");
            if let Some(Step::InjectError(_)) = steps.front() {
                let error = match steps
                    .pop_front()
                    .expect("No error step available in scripted transport")
                {
                    Step::InjectError(e) => e,
                    _ => unreachable!(),
                };
                return Err(error);
            }
        }

        // Try to get a response from the channel (blocking)
        match self.response_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(response)) => {
                let len = response.len().min(dst.len());
                dst[..len].copy_from_slice(&response[..len]);
                Ok(len)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Error::Timeout),
        }
    }

    fn recv_into_with_timeout(&mut self, dst: &mut [u8], _timeout: Duration) -> Result<usize> {
        // For the scripted transport, we just use the same logic as recv_into
        // The timeout is handled by the script itself
        self.recv_into(dst)
    }
}

#[cfg(not(feature = "mode-async"))]
impl HasTransportConfig for ScriptedSyncTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.transport_config
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
        sony_complete_with_sequence(socket, 0)
    }

    /// Create an ACK response with Sony envelope and specific sequence number
    pub fn sony_ack_with_sequence(socket: u8, sequence: u32) -> Vec<u8> {
        let seq_bytes = sequence.to_be_bytes();
        vec![
            0x01,
            0x11,
            0x00,
            0x03,
            seq_bytes[0],
            seq_bytes[1],
            seq_bytes[2],
            seq_bytes[3], // Sony header with sequence
            0x90,
            0x40 | (socket & 0x0F),
            VISCA_TERMINATOR,
        ]
    }

    /// Create a completion response with Sony envelope and specific sequence number
    pub fn sony_complete_with_sequence(socket: u8, sequence: u32) -> Vec<u8> {
        let seq_bytes = sequence.to_be_bytes();
        vec![
            0x01,
            0x11,
            0x00,
            0x03,
            seq_bytes[0],
            seq_bytes[1],
            seq_bytes[2],
            seq_bytes[3], // Sony header with sequence
            0x90,
            0x50 | (socket & 0x0F),
            VISCA_TERMINATOR,
        ]
    }

    /// Auto-response mode for Sony cameras: generate Sony envelope responses
    /// This function returns a Step that dynamically extracts the sequence number
    /// from the incoming command and echoes it back in the responses.
    pub fn sony_auto_respond_step() -> Step {
        Step::DynamicResponse(Box::new(|sent_bytes| {
            // Extract sequence number from the Sony header (bytes 4-7)
            let sequence = if sent_bytes.len() >= 8 {
                u32::from_be_bytes([sent_bytes[4], sent_bytes[5], sent_bytes[6], sent_bytes[7]])
            } else {
                0
            };

            // Generate ACK and completion with matching sequence
            vec![
                sony_ack_with_sequence(1, sequence),
                sony_complete_with_sequence(1, sequence),
            ]
        }))
    }

    /// Create a power inquiry response
    pub fn power_inquiry_response(power_on: bool) -> Step {
        let data = if power_on {
            vec![0x90, 0x50, 0x02, VISCA_TERMINATOR]
        } else {
            vec![0x90, 0x50, 0x03, VISCA_TERMINATOR]
        };

        inquiry_response(vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR], 1, data)
    }

    /// Common error responses
    pub mod errors {
        use super::{Step, VISCA_TERMINATOR};
        use crate::Error;

        pub fn syntax_error(_socket: u8) -> Step {
            // Syntax errors come without ACK, so they don't have socket assignment
            // Per VISCA spec, immediate errors use 0x60 without socket bits
            Step::OnSend {
                matches: None,
                responses: vec![vec![0x90, 0x60, 0x02, VISCA_TERMINATOR]],
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
            Step::InjectError(Error::ConnectionClosed {
                reason: Some("Test connection lost".into()),
            })
        }
    }
}

#[cfg(feature = "mode-async")]
impl<E> HasTransportConfig for ScriptedTransport<E> {
    fn transport_config(&self) -> &TransportConfig {
        // Return a static default config for test transports
        // This is safe because TransportConfig is Copy and we're returning a reference to a static
        static DEFAULT_CONFIG: std::sync::OnceLock<TransportConfig> = std::sync::OnceLock::new();
        DEFAULT_CONFIG.get_or_init(TransportConfig::default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;

    #[cfg(feature = "mode-async")]
    use crate::testing::testkit::DeterministicExecutor;

    #[test]
    #[cfg(not(feature = "mode-async"))]
    #[allow(clippy::unwrap_used)]
    fn test_scripted_blocking_transport_basic() {
        let mut transport = ScriptedSyncTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]],
        }]);

        // Send a command
        transport
            .send_with_kind(
                &[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR],
                CommandKind::Command,
            )
            .unwrap();

        // Should receive the scripted response
        let mut buffer = vec![0u8; 256];
        let n = transport.recv_into(&mut buffer).unwrap();
        assert_eq!(&buffer[..n], &[0x90, 0x41, VISCA_TERMINATOR]);

        // Verify the command was recorded
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR]);
    }

    #[test]
    #[cfg(not(feature = "mode-async"))]
    #[allow(clippy::unwrap_used)]
    fn test_scripted_blocking_transport_no_response() {
        let mut transport = ScriptedSyncTransport::new(vec![]);

        // Send a command
        transport
            .send_with_kind(
                &[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR],
                CommandKind::Command,
            )
            .unwrap();

        // Should timeout since no response is scripted
        let mut buffer = vec![0u8; 256];
        let result = transport.recv_into(&mut buffer);
        assert!(matches!(result, Err(Error::Timeout)));
    }

    #[cfg(feature = "mode-async")]
    #[tokio::test]
    #[allow(clippy::unwrap_used)]
    async fn test_scripted_async_transport_with_executor() {
        let (executor, _clock) = DeterministicExecutor::new();

        // Test basic functionality without delayed responses for now
        // The After step with spawned tasks requires more complex integration
        // between DeterministicExecutor and async-executor
        let mut transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]],
        }])
        .with_executor(executor.clone());

        // Send a command
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .await
            .unwrap();

        // Should receive the response immediately
        let mut buffer = vec![0u8; 256];
        let n = transport.recv_into(&mut buffer).await.unwrap();
        assert_eq!(&buffer[..n], &[0x90, 0x41, VISCA_TERMINATOR]);

        // Verify the command was recorded
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR]);
    }

    #[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
    #[tokio::test(start_paused = true)]
    #[allow(clippy::unwrap_used)]
    async fn test_scripted_transport_immediate_timeout_via_injected_error() {
        use crate::testing::testkit::helpers::errors;
        use crate::{Error, TokioExecutor};
        use std::sync::Arc;

        let exec = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Arrange: first recv() should see a transport-level timeout error
        let mut transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![errors::transport_timeout()]).with_executor(exec);

        // Act: send anything (no response will be produced)
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .await
            .unwrap();

        // Assert: recv yields Err(Timeout) immediately (no hangs)
        let mut buffer = vec![0u8; 256];
        let err = transport.recv_into(&mut buffer).await.unwrap_err();
        assert!(matches!(err, Error::Timeout));
    }

    #[cfg(all(test, feature = "test-utils", feature = "mode-async"))]
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
                    responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]],
                },
            ];

            let mut transport = ScriptedTransport::new(steps).with_executor(executor.clone());

            // Kick off send, then concurrently wait for recv
            transport.send(&cmd).await.unwrap();

            // Spawn the recv future and advance time to deliver the delayed response
            let mut buffer = vec![0u8; 256];
            let recv_fut = transport.recv_into(&mut buffer);
            executor.drive_until_idle();
            clock.advance(Duration::from_millis(100));
            executor.drive_until_idle();

            let n = recv_fut.await.expect("Delayed response should arrive");
            assert_eq!(&buffer[..n], &[0x90, 0x41, VISCA_TERMINATOR]);
        });
    }
}
