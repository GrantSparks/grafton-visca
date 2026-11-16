//! Scripted transport implementations for deterministic testing.
//!
//! This module provides transport implementations that follow a predetermined script
//! of responses, allowing for deterministic and repeatable test behavior. Both
//! transports run one step queue; only their native I/O differs.

#![cfg(feature = "test-utils")]
// Panics and expects in test utilities are intentional for detecting test failures
#![allow(clippy::panic, clippy::expect_used)]

use std::time::Duration;

#[cfg(feature = "blocking")]
use crate::testing::manual_clock::ManualClock;
use std::sync::Arc;
#[cfg(any(feature = "async", feature = "blocking"))]
use std::{
    collections::VecDeque,
    sync::{Mutex, MutexGuard},
};

#[cfg(feature = "blocking")]
use crate::transport::BlockingTransport;
#[cfg(any(feature = "async", feature = "blocking"))]
use crate::transport::{builder::TransportConfig, HasTransportConfig, ReceiveOutcome};
use crate::Error;
#[cfg(any(feature = "async", feature = "blocking"))]
use crate::Result;
#[cfg(feature = "async")]
use crate::{executor::Executor, transport::AsyncTransport};

/// Function type for dynamic response generation.
pub type DynamicResponseFn = Arc<dyn Fn(&[u8]) -> Vec<Vec<u8>> + Send + Sync>;

/// A step in a transport script defining what should happen when commands are sent.
#[non_exhaustive]
#[derive(Clone)]
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

    /// Persistently answer every subsequent send with this response function.
    /// This step stays at the front of the script until the transport is dropped.
    #[non_exhaustive]
    EverySend {
        /// Produces the reply batch for each command.
        respond: DynamicResponseFn,
    },
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
            Step::EverySend { .. } => f.write_str("EverySend(<function>)"),
        }
    }
}

/// One reply batch a send releases, in script order.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
enum Delivery {
    /// Deliver now.
    Now(Vec<u8>),
    /// Deliver after a virtual delay (a [`Step::After`]).
    After {
        /// Delay on the selected executor or blocking manual clock; immediate
        /// delivery modes ignore it.
        delay: Duration,
        /// The frames released together.
        batch: Vec<Vec<u8>>,
    },
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
        if let Some(Step::EverySend { respond }) = self.steps.front() {
            return (deliveries, Some(Answer::Dynamic(Arc::clone(respond))));
        }
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
            if let Some(Step::After {
                delay, responses, ..
            }) = self.steps.pop_front()
            {
                deliveries.push(Delivery::After {
                    delay,
                    batch: responses,
                });
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

/// Immediate reply delivery for a scripted transport with no executor.
#[cfg(feature = "async")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Immediate;

/// Async transport that follows a predetermined script of responses.
///
/// This transport allows tests to define exactly what responses should be sent
/// and when, without relying on real network behavior or timing. A
/// [`Step::After`] batch is delivered after its delay on the executor given to
/// [`with_executor`](Self::with_executor), or immediately without one.
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct ScriptedTransport<E = Immediate> {
    script: Script,
    executor: E,
    shutdown_rx: Option<flume::Receiver<()>>,
    config: TransportConfig,
}

#[cfg(feature = "async")]
impl<E: Clone> Clone for ScriptedTransport<E> {
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
impl ScriptedTransport<Immediate> {
    /// Create a new scripted transport with the given steps.
    pub fn new(steps: impl Into<Vec<Step>>) -> Self {
        Self {
            script: Script::new("ScriptedTransport", steps.into()),
            executor: Immediate,
            shutdown_rx: None,
            config: TransportConfig::default(),
        }
    }

    /// A delayed batch is delivered immediately when no executor is selected.
    pub fn add_after(&self, _delay: Duration, response: Vec<u8>) {
        self.script.deliver(response);
    }

    fn deliver(&self, delivery: Delivery) {
        match delivery {
            Delivery::Now(reply) => self.script.deliver(reply),
            Delivery::After { batch, .. } => {
                for reply in batch {
                    self.script.deliver(reply);
                }
            }
        }
    }
}

#[cfg(feature = "async")]
impl<E> ScriptedTransport<E> {
    /// Select an executor for delayed replies, preserving the current script.
    pub fn with_executor<X: Executor>(self, executor: X) -> ScriptedTransport<X> {
        ScriptedTransport {
            script: self.script,
            executor,
            shutdown_rx: self.shutdown_rx,
            config: self.config,
        }
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
    /// on [`crate::config::SessionConfig`], not on the transport.
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

#[cfg(feature = "async")]
impl<E: Executor> ScriptedTransport<E> {
    /// Schedule a response to be delivered after `delay` on the selected executor.
    pub fn add_after(&self, delay: Duration, response: Vec<u8>) {
        self.deliver(Delivery::After {
            delay,
            batch: vec![response],
        });
    }

    fn deliver(&self, delivery: Delivery)
    where
        E: Executor,
    {
        match delivery {
            Delivery::Now(reply) => self.script.deliver(reply),
            Delivery::After { delay, batch } => {
                let replies_tx = self.script.replies_tx.clone();
                let sleeper = self.executor.clone();
                let deadline = sleeper.now().checked_add(delay);
                self.executor.spawn_bg(async move {
                    match deadline {
                        Some(deadline) => sleeper.timer(deadline).await,
                        None => std::future::pending::<()>().await,
                    }
                    for reply in batch {
                        let _ = replies_tx.send_async(reply).await;
                    }
                });
            }
        }
    }
}

#[cfg(feature = "async")]
impl<E> AsyncTransport for ScriptedTransport<E>
where
    E: Executor,
{
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        for delivery in self.script.on_send(bytes) {
            self.deliver(delivery);
        }
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome> {
        self.receive(dst).await
    }
}

#[cfg(feature = "async")]
impl<E> ScriptedTransport<E> {
    async fn receive(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome> {
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
            // A signalled shutdown wins over a queued reply on the same poll.
            future::or(
                async {
                    let _ = shutdown_rx.recv_async().await;
                    Err(Error::ConnectionClosed {
                        source: Some(Arc::new(Error::from(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "Shutdown signal received",
                        )))),
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
impl AsyncTransport for ScriptedTransport<Immediate> {
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        for delivery in self.script.on_send(bytes) {
            self.deliver(delivery);
        }
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome> {
        self.receive(dst).await
    }
}

#[cfg(feature = "async")]
impl<E> HasTransportConfig for ScriptedTransport<E> {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Custom {
            semantics: crate::transport::SendSemantics::Stream,
            addressing: crate::transport::AddressingMode::Ip,
        }
    }

    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

/// Blocking transport that follows a predetermined script of responses.
///
/// This is the blocking counterpart of the async `ScriptedTransport` and runs
/// the same step queue. With [`Self::on_clock`], reads and [`Step::After`]
/// batches wait on virtual time. Without a clock, delayed batches are delivered
/// immediately.
#[cfg(feature = "blocking")]
#[derive(Clone, Debug)]
pub struct ScriptedBlockingTransport {
    script: Script,
    config: TransportConfig,
    clock: Option<ManualClock>,
    scheduled: Arc<Mutex<Vec<ScheduledBatch>>>,
}

#[cfg(feature = "blocking")]
#[derive(Debug)]
struct ScheduledBatch {
    at: Option<std::time::Instant>,
    replies: Vec<Vec<u8>>,
}

#[cfg(feature = "blocking")]
impl ScriptedBlockingTransport {
    /// Create a new scripted blocking transport with the given steps.
    pub fn new(steps: impl Into<Vec<Step>>) -> Self {
        Self {
            script: Script::new("ScriptedBlockingTransport", steps.into()),
            config: TransportConfig::default(),
            clock: None,
            scheduled: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Measure reads and delayed replies on `clock` instead of wall time.
    #[must_use]
    pub fn on_clock(mut self, clock: &ManualClock) -> Self {
        self.clock = Some(clock.clone());
        self
    }

    fn release_due(&self, now: std::time::Instant) {
        let mut scheduled = self
            .scheduled
            .lock()
            .expect("scripted reply schedule mutex poisoned");
        // Stable ordering preserves script order for equal deadlines.
        scheduled.sort_by_key(|batch| (batch.at.is_none(), batch.at));
        let ready = scheduled.partition_point(|batch| batch.at.is_some_and(|at| at <= now));
        for batch in scheduled.drain(..ready) {
            for reply in batch.replies {
                self.script.deliver(reply);
            }
        }
    }

    fn receive_on_clock(&self, clock: &ManualClock, timeout: Duration) -> Option<Vec<u8>> {
        let expires = clock.now().checked_add(timeout);
        loop {
            let now = clock.now();
            self.release_due(now);
            if let Ok(reply) = self.script.replies_rx.try_recv() {
                return Some(reply);
            }
            if expires.is_some_and(|deadline| now >= deadline) {
                return None;
            }
            let next = self
                .scheduled
                .lock()
                .expect("scripted reply schedule mutex poisoned")
                .iter()
                .filter_map(|batch| batch.at)
                .min();
            let deadline = expires.into_iter().chain(next).min();
            let received = match deadline {
                Some(deadline) => clock.wait_until(deadline, || {
                    self.release_due(clock.now());
                    self.script.replies_rx.try_recv().ok()
                }),
                None => clock.wait_on(None, &mut BlockingReplies(self)),
            };
            if received.is_some() {
                return received;
            }
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
    fn send_with_timeout(&mut self, bytes: &[u8], _timeout: Duration) -> Result<()> {
        for delivery in self.script.on_send(bytes) {
            match delivery {
                Delivery::Now(reply) => self.script.deliver(reply),
                Delivery::After { delay, batch } => {
                    if let Some(clock) = &self.clock {
                        self.scheduled
                            .lock()
                            .expect("scripted reply schedule mutex poisoned")
                            .push(ScheduledBatch {
                                at: clock.now().checked_add(delay),
                                replies: batch,
                            });
                    } else {
                        for reply in batch {
                            self.script.deliver(reply);
                        }
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

        if let Some(clock) = &self.clock {
            return self
                .receive_on_clock(clock, timeout)
                .map(|reply| ReceiveOutcome::copy_message(&reply, dst))
                .ok_or_else(Error::io_timeout);
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
struct BlockingReplies<'a>(&'a ScriptedBlockingTransport);
#[cfg(feature = "blocking")]
impl crate::testing::manual_clock::Source<Vec<u8>> for BlockingReplies<'_> {
    fn poll(&mut self) -> Option<Vec<u8>> {
        self.0.script.replies_rx.try_recv().ok()
    }
    fn block(
        &mut self,
        bell: &flume::Receiver<crate::testing::manual_clock::Bell>,
    ) -> Result<Vec<u8>, crate::testing::manual_clock::Bell> {
        flume::Selector::new()
            .recv(&self.0.script.replies_rx, |reply| {
                reply.map_err(|_| crate::testing::manual_clock::Bell::Expired)
            })
            .recv(bell, |bell| {
                Err(bell.unwrap_or(crate::testing::manual_clock::Bell::Expired))
            })
            .wait()
    }
}

#[cfg(feature = "blocking")]
impl HasTransportConfig for ScriptedBlockingTransport {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Custom {
            semantics: crate::transport::SendSemantics::Stream,
            addressing: crate::transport::AddressingMode::Ip,
        }
    }

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
        Step::EverySend {
            respond: std::sync::Arc::new(|_| vec![ack(1), complete(1)]),
        }
    }

    /// Auto-response mode for Sony cameras: every send is answered with an ACK
    /// and a completion on socket 1, each wrapped in the Sony envelope that
    /// echoes the request's sequence number (sequence 0 for a request that is
    /// not enveloped).
    pub fn sony_auto_respond_step() -> Step {
        Step::EverySend {
            respond: std::sync::Arc::new(|sent_bytes| {
                let sequence = sony_sequence(sent_bytes).unwrap_or(0);
                vec![
                    sony_reply(sequence, &ack(1)),
                    sony_reply(sequence, &complete(1)),
                ]
            }),
        }
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
                source: Some(std::sync::Arc::new(Error::from(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Test connection lost",
                )))),
            })
        }
    }
}

#[cfg(all(test, any(feature = "async", feature = "blocking")))]
mod tests {
    use super::*;

    use crate::command::bytes::VISCA_TERMINATOR;
    #[cfg(feature = "async")]
    use crate::testing::testkit::{ManualClock, ManualClockExecutor};
    use crate::transport::ReceiveOutcome;

    #[cfg(feature = "async")]
    fn assert_executor_delivers_at_its_deadline(executor: impl Executor) {
        let start = executor.now();
        let mut transport = ScriptedTransport::new(vec![Step::after(
            Duration::from_secs(30),
            vec![helpers::ack(1)],
        )])
        .with_executor(executor.clone());
        executor.block_on(async {
            transport.send(&[0x81]).await.expect("send");
            let mut buffer = [0; 16];
            let outcome = transport.recv_into(&mut buffer).await.expect("reply");
            assert_eq!(&buffer[..outcome.copied_len()], helpers::ack(1));
        });
        assert_eq!(executor.now() - start, Duration::from_secs(30));
    }

    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn tokio_scripted_transport_uses_manual_clock_deadlines() {
        let runtime = tokio::runtime::Runtime::new().expect("Tokio");
        let clock = ManualClock::new();
        assert_executor_delivers_at_its_deadline(ManualClockExecutor::new(
            crate::TokioExecutor::from_handle(runtime.handle().clone()),
            &clock,
        ));
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_scripted_transport_uses_manual_clock_deadlines() {
        let clock = ManualClock::new();
        assert_executor_delivers_at_its_deadline(ManualClockExecutor::new(
            crate::SmolExecutor::new(),
            &clock,
        ));
    }

    #[cfg(feature = "blocking")]
    #[test]
    fn a_silent_scripted_camera_times_out_without_real_sleep() {
        let clock = ManualClock::new();
        let participant = clock.enter();
        let real = std::time::Instant::now();
        let session = clock
            .open_blocking_session(
                &participant,
                ScriptedBlockingTransport::new(vec![]).on_clock(&clock),
                crate::SessionConfig::from_compile_time::<crate::profiles::GenericVisca>()
                    .expect("profile"),
            )
            .expect("session");
        let camera = session
            .camera::<crate::profiles::GenericVisca>(crate::types::CameraId::CAMERA_1)
            .expect("camera");
        let start = clock.now();
        assert!(matches!(
            camera.focus().set_mode(crate::types::FocusMode::Manual),
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        assert!(clock.now() > start);
        session.close().expect("close");
        assert!(real.elapsed() < Duration::from_secs(1));
    }

    #[cfg(feature = "async")]
    #[test]
    fn delayed_batch_deadlines_start_at_send_before_the_task_is_polled() {
        let clock = ManualClock::new();
        let executor = ManualClockExecutor::local(&clock);
        let start = clock.now();
        let mut transport = ScriptedTransport::new(vec![Step::after(
            Duration::from_secs(30),
            vec![helpers::ack(1)],
        )])
        .with_executor(executor.clone());
        executor.run_until(async {
            transport.send(&[0x81]).await.expect("send");
            clock.advance(Duration::from_secs(10));
            let mut buffer = [0; 16];
            let outcome = transport.recv_into(&mut buffer).await.expect("reply");
            assert_eq!(&buffer[..outcome.copied_len()], helpers::ack(1));
        });
        assert_eq!(clock.now() - start, Duration::from_secs(30));
    }

    #[cfg(feature = "async")]
    #[test]
    fn executor_free_transport_clones_dynamic_steps_and_prefers_shutdown() {
        futures_lite::future::block_on(async {
            let original = Step::DynamicResponse(Arc::new(|sent| vec![sent.to_vec()]));
            let mut transport = ScriptedTransport::new(vec![original.clone()]);
            transport.send(&[0x81, 1]).await.expect("send");
            let mut buffer = [0; 16];
            let count = transport
                .recv_into(&mut buffer)
                .await
                .expect("cloned dynamic reply")
                .copied_len();
            assert_eq!(&buffer[..count], &[0x81, 1]);
            for _ in 0..200 {
                let (signal, shutdown) = flume::bounded(1);
                let mut transport = ScriptedTransport::new(vec![helpers::auto_respond_step()])
                    .with_shutdown(shutdown);
                transport.send(&[0x81, 1]).await.expect("send");
                signal.send(()).expect("shutdown signal");
                assert!(matches!(
                    transport.recv_into(&mut buffer).await,
                    Err(Error::ConnectionClosed { .. })
                ));
            }
        });
    }

    #[cfg(feature = "blocking")]
    #[test]
    fn automatic_reply_steps_answer_every_send() {
        for step in [
            helpers::auto_respond_step(),
            helpers::sony_auto_respond_step(),
        ] {
            let mut transport = ScriptedBlockingTransport::new(vec![step]);
            for _ in 0..2 {
                transport
                    .send_with_timeout(&[0x81, 1], Duration::ZERO)
                    .expect("send");
                let mut buffer = [0; 32];
                for _ in 0..2 {
                    let outcome = transport
                        .recv_into_with_timeout(&mut buffer, Duration::ZERO)
                        .expect("every send gets both replies");
                    assert!(outcome.copied_len() > 0);
                }
            }
        }
    }

    #[cfg(feature = "blocking")]
    #[test]
    fn blocking_reads_and_delayed_batches_wait_on_virtual_time() {
        let clock = ManualClock::new();
        let _entered = clock.enter();
        let mut transport = ScriptedBlockingTransport::new(vec![Step::after(
            Duration::from_secs(30),
            vec![helpers::ack(1)],
        )])
        .on_clock(&clock);
        let start = clock.now();
        let real = std::time::Instant::now();
        transport
            .send_with_timeout(&[0x81], Duration::ZERO)
            .expect("send");
        let mut buffer = [0; 16];
        let outcome = transport
            .recv_into_with_timeout(&mut buffer, Duration::from_secs(60))
            .expect("delayed reply");
        assert_eq!(&buffer[..outcome.copied_len()], helpers::ack(1));
        assert_eq!(clock.now() - start, Duration::from_secs(30));
        assert!(transport
            .recv_into_with_timeout(&mut buffer, Duration::from_secs(60))
            .is_err());
        assert_eq!(clock.now() - start, Duration::from_secs(90));
        assert!(real.elapsed() < Duration::from_secs(1));
    }

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
        let clock = ManualClock::new();
        let executor = Arc::new(ManualClockExecutor::local(&clock));

        // Immediate replies use the same transport over the local executor.
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
        let mut transport: ScriptedTransport<Arc<TokioExecutor>> =
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

    #[cfg(feature = "async")]
    #[test]
    fn delayed_response_uses_the_one_manual_clock() {
        let clock = ManualClock::new();
        assert_executor_delivers_at_its_deadline(ManualClockExecutor::local(&clock));
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
            Err(Error::ConnectionClosed { source }) => {
                assert!(
                    matches!(source.as_deref(), Some(Error::Io { source: io }) if io.kind() == std::io::ErrorKind::UnexpectedEof)
                );
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
            .send_with_timeout(&[0x81, 0x01, VISCA_TERMINATOR], Duration::ZERO)
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
        let clock = ManualClock::new();
        let executor = Arc::new(ManualClockExecutor::local(&clock));
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
            .send_with_timeout(&[0x81], Duration::ZERO)
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
            ScriptedBlockingTransport::new(vec![Step::DynamicResponse(Arc::new(|_| {
                panic!("dynamic response panics")
            }))]);
        let mut sender = transport.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sender.send_with_timeout(&[0x81], Duration::ZERO)
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
        let transport = ScriptedTransport::new(vec![]);
        poison(&transport.script);
        let _ = transport.sent();
    }
}
