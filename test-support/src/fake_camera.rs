//! The integration suite's one fake camera.
//!
//! Compiled under grafton-visca's `blocking` or `async` feature. It must not
//! depend on `test-utils`: most behavioural tests run in CI legs that do not
//! enable it.
//!
//! * [`frames`] — the reply frames (ACK, completion, error, inquiry data, the
//!   Sony envelope). It is the same source file the crate's testkit uses,
//!   `src/testing/frames.rs`, compiled here through `#[path]`, so the suite
//!   and the testkit cannot drift. This module supplies the
//!   `VISCA_TERMINATOR` that file reads from its parent.
//! * [`FakeCamera`] — a sans-I/O camera: a responder decides the [`Answer`] to
//!   each write (replies, queued receive faults, or a failed send), tests may
//!   push replies at any time, and every write and delivered read is recorded.
//! * [`BlockingWire`] / [`AsyncWire`] — the transport adapters. Both record
//!   writes and deliver reads through the camera; only their native receive
//!   differs.
//! * Wait helpers — [`FakeCamera::wait_for_writes`] /
//!   [`FakeCamera::wait_for_reads`] (blocking) and their `_async` forms
//!   (runtime-neutral, polling with [`Executor::sleep`]), all bounded by
//!   [`WAIT_BUDGET`].
//!
//! # Silent camera
//!
//! A read with nothing queued behaves as a socket read does. The blocking
//! wire waits up to the timeout the owner passed for a reply to arrive, then
//! reports [`Error::io_timeout`]; a zero timeout polls. The async wire waits
//! until a reply arrives, leaving timeouts to the owner. Every blocking fake
//! in the suite shares this one model, so the owner's pump timing is the same
//! in every test for the same scripted silence.
//!
//! # Virtual time
//!
//! With `test-utils`, a blocking wire built with [`BlockingWire::on_clock`]
//! waits for replies on a `ManualClock` instead, and
//! [`FakeCamera::wait_for_writes_on`] / [`FakeCamera::wait_for_reads_on`]
//! wait on it too, so a blocking session opened on that clock never blocks
//! in real time.

use std::{
    fmt,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    time::Duration,
};

#[cfg(feature = "test-utils")]
use grafton_visca::testing::testkit::ManualClock;
#[cfg(feature = "async")]
use grafton_visca::Executor;
use grafton_visca::{
    camera::TransportKind,
    command::VISCA_TERMINATOR,
    transport::{
        AddressedBus, AddressingMode, HasTransportConfig, ReceiveOutcome, SendSemantics,
        TransportConfig,
    },
    Error,
};

#[path = "../../src/testing/frames.rs"]
pub mod frames;

/// The budget every wait helper allows before it fails the test.
pub const WAIT_BUDGET: Duration = Duration::from_secs(5);

/// How often the async wait helpers re-check their condition.
#[cfg(feature = "async")]
const ASYNC_POLL: Duration = Duration::from_millis(1);

/// Zoom tele (standard speed), camera address 1.
pub const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
/// Zoom stop, camera address 1.
pub const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
/// Focus stop, camera address 1.
pub const FOCUS_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xff];

/// ACK and completion for `socket` in one read, as a camera that finishes a
/// command at once may send them.
pub fn ack_and_complete(socket: u8) -> Vec<u8> {
    [frames::ack(socket), frames::complete(socket)].concat()
}

/// The VISCA message inside `frame`: the payload of a Sony envelope, or the
/// frame itself when it is raw VISCA.
///
/// # Panics
///
/// Panics on a malformed envelope, as [`frames::sony_split`] does.
#[must_use]
fn visca_payload(frame: &[u8]) -> &[u8] {
    frames::sony_split(frame).map_or(frame, |(_, payload)| payload)
}

/// Wraps `reply` in the framing of `request`: the Sony envelope echoing the
/// request's sequence number when the request was enveloped, raw otherwise.
///
/// # Panics
///
/// Panics on a malformed request envelope, as [`frames::sony_split`] does.
#[must_use]
fn reply_like(request: &[u8], reply: &[u8]) -> Vec<u8> {
    match frames::sony_sequence(request) {
        Some(sequence) => frames::sony_reply(sequence, reply),
        None => reply.to_vec(),
    }
}

/// One read the camera delivers: reply bytes or a receive fault.
pub type Read = Result<Vec<u8>, Error>;

/// What the camera does in answer to one write.
#[derive(Debug, Default)]
pub struct Answer {
    reads: Vec<Read>,
    send_error: Option<Error>,
}

impl Answer {
    /// Queue a reply frame (or several frames in one read).
    pub fn reply(&mut self, frame: impl Into<Vec<u8>>) -> &mut Self {
        self.reads.push(Ok(frame.into()));
        self
    }

    /// Queue a receive fault: the read that takes it fails with `error`.
    pub fn fault(&mut self, error: Error) -> &mut Self {
        self.reads.push(Err(error));
        self
    }

    /// Fail the write itself with `error`. The write is still recorded, and
    /// any reads queued with it are still delivered.
    pub fn fail_send(&mut self, error: Error) -> &mut Self {
        self.send_error = Some(error);
        self
    }
}

type Responder = Box<dyn FnMut(&[u8], &mut Answer) + Send>;

struct State {
    writes: Vec<Vec<u8>>,
    reads: usize,
    responder: Responder,
}

struct Shared {
    state: Mutex<State>,
    /// Signalled after every write and every delivered read.
    progress: Condvar,
    reads_tx: flume::Sender<Read>,
    reads_rx: flume::Receiver<Read>,
    receive_calls: AtomicUsize,
    config_reads: AtomicUsize,
    wires_dropped: AtomicUsize,
}

/// A scripted camera shared by a test and the wire it hands to a session.
///
/// Clones share one camera.
#[derive(Clone)]
pub struct FakeCamera {
    shared: Arc<Shared>,
}

impl fmt::Debug for FakeCamera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeCamera")
            .field("writes", &self.write_count())
            .field("reads", &self.read_count())
            .finish_non_exhaustive()
    }
}

impl FakeCamera {
    /// A camera whose `responder` answers each write.
    pub fn new(responder: impl FnMut(&[u8], &mut Answer) + Send + 'static) -> Self {
        let (reads_tx, reads_rx) = flume::unbounded();
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    writes: Vec::new(),
                    reads: 0,
                    responder: Box::new(responder),
                }),
                progress: Condvar::new(),
                reads_tx,
                reads_rx,
                receive_calls: AtomicUsize::new(0),
                config_reads: AtomicUsize::new(0),
                wires_dropped: AtomicUsize::new(0),
            }),
        }
    }

    /// A camera that answers nothing on its own; the test pushes every reply.
    pub fn silent() -> Self {
        Self::new(|_, _| {})
    }

    /// A camera that answers every write with ACK and completion on `socket`.
    pub fn acking(socket: u8) -> Self {
        Self::new(move |_, answer| {
            answer
                .reply(frames::ack(socket))
                .reply(frames::complete(socket));
        })
    }

    /// A camera that answers in the framing of each request. `responder`
    /// sees the VISCA message inside the request; every reply it queues is
    /// wrapped like the request — in the Sony envelope echoing the request's
    /// sequence number when the request was enveloped, raw otherwise. Faults
    /// and send failures pass through unchanged.
    pub fn visca(mut responder: impl FnMut(&[u8], &mut Answer) + Send + 'static) -> Self {
        Self::new(move |write, answer| {
            let mut inner = Answer::default();
            responder(visca_payload(write), &mut inner);
            for read in inner.reads {
                answer
                    .reads
                    .push(read.map(|reply| reply_like(write, &reply)));
            }
            answer.send_error = inner.send_error;
        })
    }

    /// Queue a reply for the next read, outside any write.
    pub fn push(&self, frame: impl Into<Vec<u8>>) {
        self.deliver(Ok(frame.into()));
    }

    /// Queue a receive fault for the next read, outside any write.
    pub fn push_fault(&self, error: Error) {
        self.deliver(Err(error));
    }

    /// Every write so far, in order.
    pub fn writes(&self) -> Vec<Vec<u8>> {
        self.state().writes.clone()
    }

    /// Remove and return every write so far, as VISCA messages (a Sony
    /// envelope is stripped). Later counts start again from zero.
    pub fn take_payloads(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.state().writes)
            .iter()
            .map(|write| visca_payload(write).to_vec())
            .collect()
    }

    /// Remove the writes so far and return the one VISCA message among them.
    ///
    /// # Panics
    ///
    /// Panics unless exactly one write was made.
    pub fn take_only_payload(&self) -> Vec<u8> {
        let mut payloads = self.take_payloads();
        assert_eq!(payloads.len(), 1, "expected exactly one written frame");
        payloads.remove(0)
    }

    /// The number of writes so far.
    pub fn write_count(&self) -> usize {
        self.state().writes.len()
    }

    /// The number of reads delivered to the owner so far (replies and faults).
    pub fn read_count(&self) -> usize {
        self.state().reads
    }

    /// The number of receive calls so far, including those that found nothing.
    pub fn receive_calls(&self) -> usize {
        self.shared.receive_calls.load(Ordering::Acquire)
    }

    /// The number of times a session read a wire's transport configuration.
    pub fn config_reads(&self) -> usize {
        self.shared.config_reads.load(Ordering::Acquire)
    }

    /// The number of wires built from this camera that have been dropped.
    pub fn wires_dropped(&self) -> usize {
        self.shared.wires_dropped.load(Ordering::Acquire)
    }

    /// Block until at least `count` writes have been made, then return them.
    ///
    /// # Panics
    ///
    /// Panics when [`WAIT_BUDGET`] elapses first.
    pub fn wait_for_writes(&self, count: usize) -> Vec<Vec<u8>> {
        self.wait_until(&format!("{count} writes"), |state| {
            state.writes.len() >= count
        })
        .writes
        .clone()
    }

    /// Block until at least `count` reads have been delivered.
    ///
    /// # Panics
    ///
    /// Panics when [`WAIT_BUDGET`] elapses first.
    pub fn wait_for_reads(&self, count: usize) {
        drop(self.wait_until(&format!("{count} reads"), |state| state.reads >= count));
    }

    /// [`Self::wait_for_writes`] on `clock`'s virtual time.
    ///
    /// # Panics
    ///
    /// Panics when [`WAIT_BUDGET`] of virtual time elapses first.
    #[cfg(feature = "test-utils")]
    pub fn wait_for_writes_on(&self, clock: &ManualClock, count: usize) -> Vec<Vec<u8>> {
        self.wait_on(clock, &format!("{count} writes"), |state| {
            (state.writes.len() >= count).then(|| state.writes.clone())
        })
    }

    /// [`Self::wait_for_reads`] on `clock`'s virtual time.
    ///
    /// # Panics
    ///
    /// Panics when [`WAIT_BUDGET`] of virtual time elapses first.
    #[cfg(feature = "test-utils")]
    pub fn wait_for_reads_on(&self, clock: &ManualClock, count: usize) {
        self.wait_on(clock, &format!("{count} reads"), |state| {
            (state.reads >= count).then_some(())
        });
    }

    /// Wait on `executor` until at least `count` writes have been made, then
    /// return them.
    ///
    /// # Panics
    ///
    /// Panics when [`WAIT_BUDGET`] elapses first.
    #[cfg(feature = "async")]
    pub async fn wait_for_writes_async<E: Executor>(
        &self,
        executor: &E,
        count: usize,
    ) -> Vec<Vec<u8>> {
        self.poll_until(executor, &format!("{count} writes"), |camera| {
            camera.write_count() >= count
        })
        .await;
        self.writes()
    }

    /// Wait on `executor` until at least `count` reads have been delivered.
    ///
    /// # Panics
    ///
    /// Panics when [`WAIT_BUDGET`] elapses first.
    #[cfg(feature = "async")]
    pub async fn wait_for_reads_async<E: Executor>(&self, executor: &E, count: usize) {
        self.poll_until(executor, &format!("{count} reads"), |camera| {
            camera.read_count() >= count
        })
        .await;
    }

    /// A blocking transport onto this camera.
    pub fn blocking_wire(&self) -> BlockingWire {
        BlockingWire(Wire::new(self.clone()))
    }

    /// An async transport onto this camera.
    pub fn async_wire(&self) -> AsyncWire {
        AsyncWire(Wire::new(self.clone()))
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.state.lock().expect("fake camera state lock")
    }

    fn deliver(&self, read: Read) {
        // The camera holds the receiver, so the channel cannot disconnect.
        let _ = self.shared.reads_tx.send(read);
    }

    fn on_write(&self, bytes: &[u8]) -> Result<(), Error> {
        let mut answer = Answer::default();
        {
            let mut state = self.state();
            state.writes.push(bytes.to_vec());
            (state.responder)(bytes, &mut answer);
        }
        for read in answer.reads {
            self.deliver(read);
        }
        self.shared.progress.notify_all();
        answer.send_error.map_or(Ok(()), Err)
    }

    fn begin_receive(&self) -> &flume::Receiver<Read> {
        self.shared.receive_calls.fetch_add(1, Ordering::AcqRel);
        &self.shared.reads_rx
    }

    fn on_read(&self, read: Read, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        self.state().reads += 1;
        self.shared.progress.notify_all();
        read.map(|bytes| ReceiveOutcome::copy_message(&bytes, dst))
    }

    fn wait_until(
        &self,
        what: &str,
        mut done: impl FnMut(&State) -> bool,
    ) -> MutexGuard<'_, State> {
        let (state, timeout) = self
            .shared
            .progress
            .wait_timeout_while(self.state(), WAIT_BUDGET, |state| !done(state))
            .expect("fake camera state lock");
        assert!(
            !timeout.timed_out(),
            "timed out after {WAIT_BUDGET:?} waiting for {what}"
        );
        state
    }

    #[cfg(feature = "test-utils")]
    fn wait_on<T>(
        &self,
        clock: &ManualClock,
        what: &str,
        mut done: impl FnMut(&State) -> Option<T>,
    ) -> T {
        clock
            .wait_until(clock.now() + WAIT_BUDGET, || done(&self.state()))
            .unwrap_or_else(|| panic!("timed out after {WAIT_BUDGET:?} waiting for {what}"))
    }

    #[cfg(feature = "async")]
    async fn poll_until<E: Executor>(
        &self,
        executor: &E,
        what: &str,
        done: impl Fn(&Self) -> bool,
    ) {
        let deadline = std::time::Instant::now() + WAIT_BUDGET;
        while !done(self) {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out after {WAIT_BUDGET:?} waiting for {what}"
            );
            executor.sleep(ASYNC_POLL).await;
        }
    }
}

/// What both wires share: the camera, the transport configuration, and the
/// transport facts a session reads.
#[derive(Debug)]
struct Wire {
    camera: FakeCamera,
    /// The clock a blocking read waits on; real time without one.
    #[cfg(feature = "test-utils")]
    clock: Option<ManualClock>,
    config: TransportConfig,
    semantics: SendSemantics,
    addressing: Option<AddressingMode>,
    kind: Option<TransportKind>,
    bus: Option<AddressedBus>,
}

impl Wire {
    fn new(camera: FakeCamera) -> Self {
        Self {
            camera,
            #[cfg(feature = "test-utils")]
            clock: None,
            config: TransportConfig::default(),
            semantics: SendSemantics::Datagram,
            addressing: None,
            kind: None,
            bus: None,
        }
    }
}

#[cfg(feature = "blocking")]
impl Wire {
    /// A blocking read: a poll for a zero `timeout`, otherwise a wait of up
    /// to `timeout` on the wire's clock, or in real time without one.
    fn read_within(&self, timeout: Duration) -> Option<Read> {
        let reads = self.camera.begin_receive();
        if timeout.is_zero() {
            return reads.try_recv().ok();
        }
        #[cfg(feature = "test-utils")]
        if let Some(clock) = &self.clock {
            if let Some(deadline) = clock.now().checked_add(timeout) {
                return clock.wait_until(deadline, || reads.try_recv().ok());
            }
        }
        match std::time::Instant::now().checked_add(timeout) {
            Some(deadline) => reads.recv_deadline(deadline).ok(),
            None => reads.recv().ok(),
        }
    }
}

impl Drop for Wire {
    fn drop(&mut self) {
        self.camera
            .shared
            .wires_dropped
            .fetch_add(1, Ordering::AcqRel);
    }
}

macro_rules! wire_builders {
    ($wire:ident) => {
        impl $wire {
            /// Use `config` as the transport configuration.
            pub fn with_config(mut self, config: TransportConfig) -> Self {
                self.0.config = config;
                self
            }

            /// Report `semantics` (the default is [`SendSemantics::Datagram`]).
            pub fn with_semantics(mut self, semantics: SendSemantics) -> Self {
                self.0.semantics = semantics;
                self
            }

            /// Report `mode` as the addressing hint (the default is none).
            pub fn with_addressing(mut self, mode: AddressingMode) -> Self {
                self.0.addressing = Some(mode);
                self
            }

            /// Report `kind` as the standard transport kind (the default is
            /// none, a custom transport).
            pub fn with_transport_kind(mut self, kind: TransportKind) -> Self {
                self.0.kind = Some(kind);
                self
            }

            /// Report `bus` as the serial bus this transport addressed while
            /// opening (the default is none).
            pub fn with_addressed_bus(mut self, bus: AddressedBus) -> Self {
                self.0.bus = Some(bus);
                self
            }

            /// The camera behind this wire.
            pub fn camera(&self) -> &FakeCamera {
                &self.0.camera
            }
        }

        impl HasTransportConfig for $wire {
            fn transport_config(&self) -> &TransportConfig {
                self.0
                    .camera
                    .shared
                    .config_reads
                    .fetch_add(1, Ordering::AcqRel);
                &self.0.config
            }

            fn standard_transport_kind(&self) -> Option<TransportKind> {
                self.0.kind
            }

            fn addressed_bus(&self) -> Option<&AddressedBus> {
                self.0.bus.as_ref()
            }
        }
    };
}

/// A blocking transport onto a [`FakeCamera`].
#[derive(Debug)]
pub struct BlockingWire(Wire);

wire_builders!(BlockingWire);

#[cfg(feature = "test-utils")]
impl BlockingWire {
    /// Wait for replies on `clock`'s virtual time.
    pub fn on_clock(mut self, clock: &ManualClock) -> Self {
        self.0.clock = Some(clock.clone());
        self
    }
}

#[cfg(feature = "blocking")]
impl grafton_visca::transport::BlockingTransport for BlockingWire {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: grafton_visca::command::CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.0.camera.on_write(bytes)
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        match self.0.read_within(timeout) {
            Some(read) => self.0.camera.on_read(read, dst),
            None => Err(Error::io_timeout()),
        }
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        self.0.addressing
    }

    fn send_semantics(&self) -> SendSemantics {
        self.0.semantics
    }
}

/// An async transport onto a [`FakeCamera`]. Its receive is runtime-neutral.
#[derive(Debug)]
pub struct AsyncWire(Wire);

wire_builders!(AsyncWire);

#[cfg(feature = "async")]
impl grafton_visca::transport::AsyncTransport for AsyncWire {
    fn send(
        &mut self,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        std::future::ready(self.0.camera.on_write(bytes))
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        let read = self
            .0
            .camera
            .begin_receive()
            .recv_async()
            .await
            .map_err(|_| Error::connection_closed(None))?;
        self.0.camera.on_read(read, dst)
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        self.0.addressing
    }

    fn send_semantics(&self) -> SendSemantics {
        self.0.semantics
    }
}

#[cfg(test)]
mod tests {
    use super::{frames, reply_like, visca_payload};

    #[test]
    fn replies_take_the_framing_of_their_request() {
        let payload = frames::complete(1);
        let enveloped = frames::sony_reply(0x0102_0304, &payload);
        assert_eq!(visca_payload(&enveloped), payload.as_slice());
        assert_eq!(reply_like(&enveloped, &payload), enveloped);

        let raw = [0x81, 0x09, 0x04, 0x00, 0xFF];
        assert_eq!(visca_payload(&raw), raw);
        assert_eq!(reply_like(&raw, &payload), payload);
    }

    #[test]
    #[should_panic(expected = "Sony length field disagrees with the payload")]
    fn a_malformed_envelope_is_never_stripped() {
        let mut frame = frames::sony_reply(1, &frames::ack(1));
        frame.push(0xFF);
        let _ = visca_payload(&frame);
    }
}
