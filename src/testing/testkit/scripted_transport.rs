//! Scripted transport implementations for deterministic testing.
//!
//! This module provides transport implementations that follow a predetermined script
//! of responses, allowing for deterministic and repeatable test behavior. Both
//! transports run one step queue; only their native I/O differs.

#![cfg(feature = "test-utils")]
// Panics and expects in test utilities are intentional for detecting test failures
#![allow(clippy::panic, clippy::expect_used)]

use std::time::Duration;

#[cfg(any(feature = "async", feature = "blocking"))]
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

#[cfg(feature = "async")]
use super::deterministic_executor::ExecutorExt;
#[cfg(any(feature = "async", feature = "blocking"))]
use crate::transport::{builder::TransportConfig, HasTransportConfig, ReceiveOutcome};
use crate::Error;
#[cfg(any(feature = "async", feature = "blocking"))]
use crate::Result;
#[cfg(feature = "blocking")]
use crate::{command::CommandKind, transport::BlockingTransport};
#[cfg(feature = "async")]
use crate::{executor::Executor, transport::AsyncTransport};

/// Function type for dynamic response generation.
pub type DynamicResponseFn = Box<dyn Fn(&[u8]) -> Vec<Vec<u8>> + Send + Sync>;

/// A step in a transport script defining what should happen when commands are sent.
#[non_exhaustive]
pub enum Step {
    /// Respond immediately when the next send occurs.
    /// If `matches` is provided, only respond if the sent bytes start with those bytes.
    #[non_exhaustive]
    OnSend {
        /// Optional bytes to match against sent data
        matches: Option<Vec<u8>>,
        /// Responses to send when matched
        responses: Vec<Vec<u8>>,
    },

    /// Schedule responses after a virtual delay (uses the provided Executor in async mode).
    #[non_exhaustive]
    After {
        /// Delay before sending responses
        delay: Duration,
        /// Responses to send after delay
        responses: Vec<Vec<u8>>,
    },

    /// Inject a transport-level error on the next recv attempt.
    InjectError(Error),

    /// Generate responses dynamically based on the sent command bytes.
    /// The function receives the sent bytes and returns responses to send back.
    DynamicResponse(DynamicResponseFn),
}

impl Step {
    /// Responds on the next send, optionally matching a byte prefix.
    #[must_use]
    pub fn on_send(matches: Option<Vec<u8>>, responses: Vec<Vec<u8>>) -> Self {
        Self::OnSend { matches, responses }
    }
    /// Schedules responses after a virtual delay.
    #[must_use]
    pub fn after(delay: Duration, responses: Vec<Vec<u8>>) -> Self {
        Self::After { delay, responses }
    }
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
            Step::InjectError(err) => Step::InjectError(err.clone()),
            Step::DynamicResponse(_) => Step::InjectError(Error::InvalidState(
                "DynamicResponse steps cannot be cloned".into(),
            )),
        }
    }
}

/// One reply batch a send releases, in script order.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
// Only the async transport reads the delay; the blocking one delivers
// immediately.
#[cfg_attr(not(feature = "async"), allow(dead_code))]
enum Delivery {
    /// Deliver now.
    Now(Vec<u8>),
    /// Deliver after a virtual delay (a [`Step::After`]).
    After(Duration, Vec<Vec<u8>>),
}

/// The answer a consumed step gives a send.
#[cfg(any(feature = "async", feature = "blocking"))]
enum Answer {
    /// Scripted frames.
    Frames(Vec<Vec<u8>>),
    /// Frames computed from the sent bytes by a [`Step::DynamicResponse`].
    Dynamic(DynamicResponseFn),
}

/// The step queue both scripted transports run, free of any I/O.
///
/// A send records its bytes, releases every leading [`Step::After`], consumes
/// the front [`Step::OnSend`] (when it matches) or [`Step::DynamicResponse`],
/// and then releases any [`Step::After`] that now leads. A front
/// [`Step::InjectError`] is never consumed by a send: it waits for the next
/// receive. The transports differ only in how they deliver a delayed batch.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
struct ScriptCore {
    steps: VecDeque<Step>,
    sent: Vec<Vec<u8>>,
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl ScriptCore {
    /// Records a send and consumes the step it answers.
    ///
    /// Returns the batches released so far and the answer to deliver next. A
    /// dynamic answer is returned unrun: the caller runs it with the script
    /// unlocked and then calls [`Self::release_leading_after`].
    fn begin_send(&mut self, bytes: &[u8]) -> (Vec<Delivery>, Option<Answer>) {
        self.sent.push(bytes.to_vec());
        let mut deliveries = Vec::new();
        self.release_leading_after(&mut deliveries);
        let answer = match self.steps.pop_front() {
            Some(Step::OnSend { matches, responses })
                if matches
                    .as_ref()
                    .is_none_or(|pattern| bytes.starts_with(pattern)) =>
            {
                Some(Answer::Frames(responses))
            }
            Some(Step::DynamicResponse(respond)) => Some(Answer::Dynamic(respond)),
            // An unmatched `OnSend` stays for a later send, and an
            // `InjectError` stays for the next receive.
            Some(step) => {
                self.steps.push_front(step);
                None
            }
            None => None,
        };
        (deliveries, answer)
    }

    fn release_leading_after(&mut self, deliveries: &mut Vec<Delivery>) {
        while matches!(self.steps.front(), Some(Step::After { .. })) {
            if let Some(Step::After { delay, responses }) = self.steps.pop_front() {
                deliveries.push(Delivery::After(delay, responses));
            }
        }
    }

    fn take_injected_error(&mut self) -> Option<Error> {
        match self.steps.front() {
            Some(Step::InjectError(_)) => match self.steps.pop_front() {
                Some(Step::InjectError(error)) => Some(error),
                _ => None,
            },
            _ => None,
        }
    }
}

/// A shared [`ScriptCore`] plus the reply channel both transports read.
///
/// Replies are plain frames: a scripted receive error is only ever a
/// [`Step::InjectError`], so both transports surface it the same way.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Clone, Debug)]
struct Script {
    /// The transport type, named in lock-poisoning panics.
    owner: &'static str,
    core: Arc<Mutex<ScriptCore>>,
    replies_tx: flume::Sender<Vec<u8>>,
    replies_rx: flume::Receiver<Vec<u8>>,
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl Script {
    fn new(owner: &'static str, steps: Vec<Step>) -> Self {
        let (replies_tx, replies_rx) = flume::unbounded();
        Self {
            owner,
            core: Arc::new(Mutex::new(ScriptCore {
                steps: steps.into(),
                sent: Vec::new(),
            })),
            replies_tx,
            replies_rx,
        }
    }

    fn core(&self) -> MutexGuard<'_, ScriptCore> {
        self.core
            .lock()
            .unwrap_or_else(|_| panic!("{} script mutex poisoned", self.owner))
    }

    fn sent(&self) -> Vec<Vec<u8>> {
        self.core().sent.clone()
    }

    fn deliver(&self, reply: Vec<u8>) {
        // The script holds the receiver, so the channel cannot disconnect.
        let _ = self.replies_tx.send(reply);
    }

    fn on_send(&self, bytes: &[u8]) -> Vec<Delivery> {
        let (mut deliveries, answer) = self.core().begin_send(bytes);
        let responses = match answer {
            Some(Answer::Frames(responses)) => responses,
            // A dynamic response runs with the script unlocked, so a closure
            // that panics cannot poison the script and one that calls back
            // into the transport cannot deadlock it.
            Some(Answer::Dynamic(respond)) => respond(bytes),
            None => return deliveries,
        };
        deliveries.extend(responses.into_iter().map(Delivery::Now));
        self.core().release_leading_after(&mut deliveries);
        deliveries
    }

    fn take_injected_error(&self) -> Option<Error> {
        self.core().take_injected_error()
    }
}

/// Async transport that follows a predetermined script of responses.
///
/// This transport allows tests to define exactly what responses should be sent
/// and when, without relying on real network behavior or timing. A
/// [`Step::After`] batch is delivered after its delay on the executor given to
/// [`with_executor`](Self::with_executor), or immediately without one.
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct ScriptedTransport<E = ()> {
    script: Script,
    executor: Option<Arc<E>>,
    shutdown_rx: Option<flume::Receiver<()>>,
    config: TransportConfig,
}

#[cfg(feature = "async")]
impl<E> Clone for ScriptedTransport<E> {
    fn clone(&self) -> Self {
        Self {
            script: self.script.clone(),
            executor: self.executor.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
            config: self.config,
        }
    }
}

#[cfg(feature = "async")]
impl<E> ScriptedTransport<E> {
    /// Create a new scripted transport with the given steps.
    pub fn new(steps: impl Into<Vec<Step>>) -> Self {
        Self {
            script: Script::new("ScriptedTransport", steps.into()),
            executor: None,
            shutdown_rx: None,
            config: TransportConfig::default(),
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

    /// Set a custom transport configuration.
    ///
    /// This allows tests to configure transport parameters such as socket,
    /// timeout, and buffer settings. Session admission capacity is configured
    /// on [`SessionConfig`](crate::SessionConfig), not on the transport.
    pub fn with_config(mut self, config: TransportConfig) -> Self {
        self.config = config;
        self
    }

    /// Get all commands that were sent to this transport.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.script.sent()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        self.script.deliver(response);
    }

    /// Schedule a response to be delivered after `delay`.
    ///
    /// Without an executor the response is delivered immediately, as an
    /// unscheduled [`Step::After`] is.
    pub fn add_after(&self, delay: Duration, response: Vec<u8>)
    where
        E: Executor + ExecutorExt + 'static,
    {
        self.deliver(Delivery::After(delay, vec![response]));
    }

    fn deliver(&self, delivery: Delivery)
    where
        E: Executor + ExecutorExt + 'static,
    {
        match (delivery, &self.executor) {
            (Delivery::Now(reply), _) => self.script.deliver(reply),
            (Delivery::After(delay, batch), Some(executor)) => {
                let replies_tx = self.script.replies_tx.clone();
                let sleeper = Arc::clone(executor);
                executor.spawn_detached(async move {
                    sleeper.sleep(delay).await;
                    for reply in batch {
                        let _ = replies_tx.send_async(reply).await;
                    }
                });
            }
            (Delivery::After(_, batch), None) => {
                for reply in batch {
                    self.script.deliver(reply);
                }
            }
        }
    }
}

#[cfg(feature = "async")]
impl<E> AsyncTransport for ScriptedTransport<E>
where
    E: Executor + ExecutorExt + 'static,
{
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        for delivery in self.script.on_send(bytes) {
            self.deliver(delivery);
        }
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome> {
        use futures_lite::future;

        if let Some(error) = self.script.take_injected_error() {
            return Err(error);
        }

        let replies_rx = self.script.replies_rx.clone();
        let next = async {
            replies_rx
                .recv_async()
                .await
                .map_err(|_| Error::io_timeout())
        };

        let reply = if let Some(shutdown_rx) = &self.shutdown_rx {
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

        Ok(ReceiveOutcome::copy_message(&reply, dst))
    }
}

#[cfg(feature = "async")]
impl<E> HasTransportConfig for ScriptedTransport<E> {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

/// Blocking transport that follows a predetermined script of responses.
///
/// This is the blocking counterpart of the async `ScriptedTransport` and runs
/// the same step queue. It has no executor, so a [`Step::After`] batch is
/// delivered immediately and its delay is ignored.
#[cfg(feature = "blocking")]
#[derive(Clone, Debug)]
pub struct ScriptedBlockingTransport {
    script: Script,
    config: TransportConfig,
}

#[cfg(feature = "blocking")]
impl ScriptedBlockingTransport {
    /// Create a new scripted blocking transport with the given steps.
    pub fn new(steps: impl Into<Vec<Step>>) -> Self {
        Self {
            script: Script::new("ScriptedBlockingTransport", steps.into()),
            config: TransportConfig::default(),
        }
    }

    /// Set a custom transport configuration.
    pub fn with_config(mut self, config: TransportConfig) -> Self {
        self.config = config;
        self
    }

    /// Get all commands that were sent to this transport.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.script.sent()
    }

    /// Add a response to be returned immediately.
    pub fn add_response(&self, response: Vec<u8>) {
        self.script.deliver(response);
    }
}

#[cfg(feature = "blocking")]
impl BlockingTransport for ScriptedBlockingTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<()> {
        for delivery in self.script.on_send(bytes) {
            match delivery {
                Delivery::Now(reply) => self.script.deliver(reply),
                Delivery::After(_, batch) => {
                    for reply in batch {
                        self.script.deliver(reply);
                    }
                }
            }
        }
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome> {
        if let Some(error) = self.script.take_injected_error() {
            return Err(error);
        }

        // A zero timeout polls: check for a queued reply without blocking.
        // A timeout too long to express as a deadline waits indefinitely.
        let timeout = timeout.max(Duration::from_micros(1));
        let reply = match std::time::Instant::now().checked_add(timeout) {
            Some(deadline) => self.script.replies_rx.recv_deadline(deadline).ok(),
            None => self.script.replies_rx.recv().ok(),
        };
        reply
            .map(|reply| ReceiveOutcome::copy_message(&reply, dst))
            .ok_or_else(Error::io_timeout)
    }
}

#[cfg(feature = "blocking")]
impl HasTransportConfig for ScriptedBlockingTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

/// Script steps for common camera behaviours.
///
/// The reply frames come from one module, `src/testing/frames.rs`, which the
/// integration suite also compiles; see its documentation for the reply
/// grammar and the one Buffer Full shape.
pub mod helpers {
    use super::Step;

    // The frames a downstream script needs to build its own steps; the error
    // replies are reachable as ready-made steps in [`errors`].
    pub use crate::testing::frames::{
        ack, buffer_full, complete, inquiry_reply, not_executable, sony_reply, sony_sequence,
    };
    use crate::testing::frames::{canceled, no_socket, syntax_error};

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
        buffer_full_sequence_then_success(socket, 1)
    }

    /// Create a NOT EXECUTABLE response followed by successful completion after retry
    pub fn not_executable_then_success(socket: u8) -> Vec<Step> {
        not_executable_sequence_then_success(socket, 1)
    }

    /// Create a transient inquiry SYNTAX ERROR (0x02) followed by a successful
    /// data reply on resend (issue #536).
    ///
    /// The first send of an inquiry matching `pattern` receives a `0x02` syntax
    /// error (no socket assignment, per VISCA immediate-error framing); the
    /// resend receives `data`. This models a camera that briefly reports `0x02`
    /// for an inquiry while overloaded and then answers correctly.
    pub fn syntax_error_then_inquiry_success(pattern: Vec<u8>, data: Vec<u8>) -> Vec<Step> {
        vec![
            Step::OnSend {
                matches: Some(pattern.clone()),
                responses: vec![syntax_error()],
            },
            Step::OnSend {
                matches: Some(pattern),
                responses: vec![data], // No ACK for inquiries per VISCA spec
            },
        ]
    }

    /// Create `full_count` BUFFER FULL refusals followed by ACK and completion
    /// on `socket`. Buffer Full carries no socket: the camera never accepted
    /// the refused sends.
    pub fn buffer_full_sequence_then_success(socket: u8, full_count: usize) -> Vec<Step> {
        refusals_then_success(buffer_full(), full_count, socket)
    }

    /// Create a sequence of NOT EXECUTABLE responses followed by success
    pub fn not_executable_sequence_then_success(socket: u8, error_count: usize) -> Vec<Step> {
        refusals_then_success(not_executable(socket), error_count, socket)
    }

    fn refusals_then_success(refusal: Vec<u8>, count: usize, socket: u8) -> Vec<Step> {
        std::iter::repeat_n(refusal, count)
            .map(|frame| Step::OnSend {
                matches: None,
                responses: vec![frame],
            })
            .chain(std::iter::once(standard_command_response(socket)))
            .collect()
    }

    /// Auto-response mode: generate appropriate response for any VISCA command
    /// This is useful for tests that don't care about specific command details
    pub fn auto_respond_step() -> Step {
        standard_command_response(1) // Default to socket 1
    }

    /// Auto-response mode for Sony cameras: every send is answered with an ACK
    /// and a completion on socket 1, each wrapped in the Sony envelope that
    /// echoes the request's sequence number (sequence 0 for a request that is
    /// not enveloped).
    pub fn sony_auto_respond_step() -> Step {
        Step::DynamicResponse(Box::new(|sent_bytes| {
            let sequence = sony_sequence(sent_bytes).unwrap_or(0);
            vec![
                sony_reply(sequence, &ack(1)),
                sony_reply(sequence, &complete(1)),
            ]
        }))
    }

    /// Create a power inquiry response
    pub fn power_inquiry_response(power_on: bool) -> Step {
        let state = if power_on { 0x02 } else { 0x03 };
        inquiry_response(
            vec![0x81, 0x09, 0x04, 0x00, crate::command::VISCA_TERMINATOR],
            1,
            inquiry_reply(&[state]),
        )
    }

    /// Common error responses
    pub mod errors {
        use super::Step;
        use crate::Error;

        fn reply(frame: Vec<u8>) -> Step {
            Step::OnSend {
                matches: None,
                responses: vec![frame],
            }
        }

        /// Generate a syntax error response (no socket: the message was never
        /// accepted).
        pub fn syntax_error() -> Step {
            reply(super::syntax_error())
        }

        /// Generate a command buffer full error response (no socket: the
        /// command was never accepted).
        pub fn command_buffer_full() -> Step {
            reply(super::buffer_full())
        }

        /// Generate a command canceled response
        pub fn command_canceled(socket: u8) -> Step {
            reply(super::canceled(socket))
        }

        /// Generate a no socket available error response
        pub fn no_socket() -> Step {
            reply(super::no_socket(0))
        }

        /// Generate a transport timeout error
        pub fn transport_timeout() -> Step {
            Step::InjectError(Error::io_timeout())
        }

        /// Generate a connection lost error
        pub fn connection_lost() -> Step {
            Step::InjectError(Error::ConnectionClosed {
                reason: Some("Test connection lost".into()),
            })
        }
    }
}

#[cfg(all(test, any(feature = "async", feature = "blocking")))]
mod tests {
    use super::*;

    use crate::command::bytes::VISCA_TERMINATOR;
    #[cfg(feature = "async")]
    use crate::testing::testkit::DeterministicExecutor;
    use crate::transport::ReceiveOutcome;

    #[test]
    #[cfg(feature = "blocking")]
    #[allow(clippy::unwrap_used)]
    fn test_scripted_blocking_transport_basic() {
        let mut transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]],
        }]);

        transport
            .send_with_timeout(
                &[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR],
                CommandKind::Command,
                Duration::from_secs(1),
            )
            .unwrap();
        let mut buffer = vec![0u8; 256];
        let n = transport
            .recv_into_with_timeout(&mut buffer, Duration::from_secs(1))
            .unwrap()
            .copied_len();
        assert_eq!(&buffer[..n], &[0x90, 0x41, VISCA_TERMINATOR]);

        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR]);
    }

    #[test]
    #[cfg(feature = "blocking")]
    #[allow(clippy::unwrap_used)]
    fn test_scripted_blocking_transport_no_response() {
        let mut transport = ScriptedBlockingTransport::new(vec![]);

        transport
            .send_with_timeout(
                &[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR],
                CommandKind::Command,
                Duration::from_secs(1),
            )
            .unwrap();
        let mut buffer = vec![0u8; 256];
        let result = transport.recv_into_with_timeout(&mut buffer, Duration::from_millis(1));
        assert!(matches!(result, Err(Error::Timeout { .. })));
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
            responses: vec![vec![0x90, 0x41, VISCA_TERMINATOR]],
        }])
        .with_executor(executor.clone());

        transport
            .send(&[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR])
            .await
            .unwrap();
        let mut buffer = vec![0u8; 256];
        let n = transport.recv_into(&mut buffer).await.unwrap().copied_len();
        assert_eq!(&buffer[..n], &[0x90, 0x41, VISCA_TERMINATOR]);

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
        assert!(matches!(err, Error::Timeout { .. }));
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

            let n = recv_fut
                .await
                .expect("Delayed response should arrive")
                .copied_len();
            assert_eq!(&buffer[..n], &[0x90, 0x41, VISCA_TERMINATOR]);
        });
    }

    /// A scripted receive error is surfaced unchanged, not as a timeout.
    fn scripted_connection_loss() -> Vec<Step> {
        vec![
            helpers::errors::connection_lost(),
            helpers::standard_command_response(1),
        ]
    }

    fn assert_connection_lost(result: Result<ReceiveOutcome>) {
        match result {
            Err(Error::ConnectionClosed { reason }) => {
                assert_eq!(reason.as_deref(), Some("Test connection lost"));
            }
            other => panic!("expected the scripted ConnectionClosed, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "blocking")]
    #[allow(clippy::unwrap_used)]
    fn blocking_receive_surfaces_the_scripted_error_then_the_script_resumes() {
        let mut transport = ScriptedBlockingTransport::new(scripted_connection_loss());
        let mut buffer = [0u8; 16];
        assert_connection_lost(transport.recv_into_with_timeout(&mut buffer, Duration::ZERO));
        transport
            .send_with_timeout(
                &[0x81, 0x01, VISCA_TERMINATOR],
                CommandKind::Command,
                Duration::ZERO,
            )
            .unwrap();
        let n = transport
            .recv_into_with_timeout(&mut buffer, Duration::ZERO)
            .unwrap()
            .copied_len();
        assert_eq!(&buffer[..n], helpers::ack(1).as_slice());
    }

    #[test]
    #[cfg(feature = "async")]
    #[allow(clippy::unwrap_used)]
    fn async_receive_surfaces_the_scripted_error_then_the_script_resumes() {
        let (executor, _clock) = DeterministicExecutor::new();
        let mut transport =
            ScriptedTransport::new(scripted_connection_loss()).with_executor(Arc::clone(&executor));
        executor.run_until(async {
            let mut buffer = [0u8; 16];
            assert_connection_lost(transport.recv_into(&mut buffer).await);
            transport
                .send(&[0x81, 0x01, VISCA_TERMINATOR])
                .await
                .unwrap();
            let n = transport.recv_into(&mut buffer).await.unwrap().copied_len();
            assert_eq!(&buffer[..n], helpers::ack(1).as_slice());
        });
    }

    /// `After` batches are released in script order around the consumed step.
    #[test]
    #[cfg(feature = "blocking")]
    #[allow(clippy::unwrap_used)]
    fn blocking_send_releases_after_batches_in_script_order() {
        let mut transport = ScriptedBlockingTransport::new(vec![
            Step::after(Duration::from_secs(5), vec![vec![0x01]]),
            Step::on_send(None, vec![vec![0x02]]),
            Step::after(Duration::from_secs(5), vec![vec![0x03], vec![0x04]]),
            Step::on_send(None, vec![vec![0x05]]),
        ]);
        transport
            .send_with_timeout(&[0x81], CommandKind::Command, Duration::ZERO)
            .unwrap();
        let mut buffer = [0u8; 4];
        let mut received = Vec::new();
        while let Ok(outcome) = transport.recv_into_with_timeout(&mut buffer, Duration::ZERO) {
            received.push(buffer[..outcome.copied_len()].to_vec());
        }
        assert_eq!(received, [[0x01], [0x02], [0x03], [0x04]]);
    }

    /// Poisons a script's lock: a thread panics while holding it.
    fn poison(script: &Script) {
        let core = Arc::clone(&script.core);
        let _ = std::thread::spawn(move || {
            let _held = core.lock();
            panic!("a thread panics while holding the script lock");
        })
        .join();
    }

    /// A panicking dynamic response runs outside the script lock, so the
    /// script stays usable after it.
    #[test]
    #[cfg(feature = "blocking")]
    fn a_panicking_dynamic_response_does_not_poison_the_script() {
        let transport =
            ScriptedBlockingTransport::new(vec![Step::DynamicResponse(Box::new(|_| {
                panic!("dynamic response panics")
            }))]);
        let mut sender = transport.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sender.send_with_timeout(&[0x81], CommandKind::Command, Duration::ZERO)
        }));
        assert!(result.is_err(), "the dynamic response panicked");
        assert_eq!(transport.sent(), vec![vec![0x81]]);
    }

    #[test]
    #[cfg(feature = "blocking")]
    #[should_panic(expected = "ScriptedBlockingTransport script mutex poisoned")]
    fn blocking_poison_panic_names_the_blocking_transport() {
        let transport = ScriptedBlockingTransport::new(vec![]);
        poison(&transport.script);
        let _ = transport.sent();
    }

    #[test]
    #[cfg(feature = "async")]
    #[should_panic(expected = "ScriptedTransport script mutex poisoned")]
    fn async_poison_panic_names_the_async_transport() {
        let transport = ScriptedTransport::<DeterministicExecutor>::new(vec![]);
        poison(&transport.script);
        let _ = transport.sent();
    }
}
