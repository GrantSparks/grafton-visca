//! Worker tests (D24, #780).
//!
//! Every test runs the real worker thread over the production blocking
//! adapter and a channel-backed transport, so the selection loop, read
//! slicing, pacing and lifecycle are exercised as callers see them. Turn
//! semantics shared with the async actor are pinned by the shell core and
//! coordinator tests; these tests pin what is specific to the worker.
//!
//! Tests whose assertions are about elapsed owner time run the worker on a
//! [`ManualClock`]: the test thread enters the clock, the transport and the
//! peer wait on it, and virtual time advances only when every participant is
//! blocked, so those deadlines pass without a real sleep.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use std::ops::ControlFlow;

use super::super::receipt::OperationWait;
use super::super::{
    canonical_owner_trace, BlockingTransportAdapter, DiagnosticEvent, OperationObservation,
    RuntimeOutcome, RuntimeRequest, CANONICAL_OWNER_TRACE,
};
use super::super::{CommandOutcome, InquiryOutcome};
use super::*;
use crate::testing::manual_clock::ManualClock;
use crate::{
    command::PanTiltLimitCorner,
    completion::AppliedOnly,
    prepared::{
        test_support::{prepare_command, prepare_operation},
        ClassSelection,
    },
    profiles::ProfileSpec,
    profiles::{GenericVisca, SonyFr7},
    protocol::sony::SonyHeader,
    request::builtin::{FocusModeCommand, PanTiltLimitClear, ZoomStop},
    runtime::engine::{
        CancellationPolicy, ControlPolicy, EncodedMessage, ReplyShape, RequestContext, RetryPolicy,
        TimeoutPolicy,
    },
    transport::{
        BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
    },
    OperationalTuning,
};

mod cancellation_and_budget;
mod position_sampling;
mod reentry;

const ACK: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETION: &[u8] = &[0x90, 0x51, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
/// How long a test waits for something the worker should do promptly.
const PROMPTLY: Duration = Duration::from_secs(2);

type TransportCallback = Box<dyn FnOnce() + Send>;

/// One scripted transport read.
enum Read {
    Bytes(Vec<u8>),
    /// Bytes become available on the transport at an exact virtual instant.
    At(Instant, Vec<u8>),
    /// An oversized datagram: the prefix that fits is copied and the read
    /// reports the rest as lost.
    Truncated(Vec<u8>),
    Overreported,
    EndOfStream,
    Fail(Error),
    Panic,
    Callback(TransportCallback),
}

/// Receive from `receiver` within `timeout` on `clock`.
fn recv_within<T>(
    clock: &Clock,
    receiver: &flume::Receiver<T>,
    timeout: Duration,
) -> Result<T, flume::RecvTimeoutError> {
    let deadline = clock.now() + timeout;
    match clock
        .select()
        .recv(receiver, |read| read)
        .wait_until(deadline)
    {
        Some(Ok(value)) => Ok(value),
        Some(Err(flume::RecvError::Disconnected)) => Err(flume::RecvTimeoutError::Disconnected),
        None => Err(flume::RecvTimeoutError::Timeout),
    }
}

/// A transport whose reads arrive from the test thread and whose writes are
/// reported back to it. A read honors its timeout on the owner's clock, as
/// the trait requires, unless the transport is deliberately eager.
struct ChannelTransport {
    clock: Clock,
    config: TransportConfig,
    semantics: SendSemantics,
    reads: flume::Receiver<Read>,
    writes: flume::Sender<Vec<u8>>,
    fail_writes: bool,
    eager_idle: bool,
    read_calls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
    scheduled: Option<(Instant, Vec<u8>)>,
    write_callback: Arc<Mutex<Option<TransportCallback>>>,
}

impl Drop for ChannelTransport {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

impl HasTransportConfig for ChannelTransport {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Custom {
            semantics: self.semantics,
            addressing: crate::transport::AddressingMode::Ip,
        }
    }

    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ChannelTransport {
    fn send_with_timeout(&mut self, bytes: &[u8], _timeout: Duration) -> Result<(), Error> {
        let callback = self.write_callback.lock().unwrap().take();
        if let Some(callback) = callback {
            callback();
        }
        if self.fail_writes {
            return Err(Error::transport_error(std::io::Error::other(
                "injected write failure",
            )));
        }
        let _ = self.writes.send(bytes.to_vec());
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        self.read_calls.fetch_add(1, Ordering::Relaxed);
        if self.eager_idle {
            return Err(Error::io_timeout());
        }
        if let Some((at, bytes)) = self.scheduled.take() {
            self.clock
                .sleep(at.saturating_duration_since(self.clock.now()).min(timeout));
            return if self.clock.now() >= at {
                Ok(ReceiveOutcome::copy_message(&bytes, dst))
            } else {
                self.scheduled = Some((at, bytes));
                Err(Error::io_timeout())
            };
        }
        let deadline = self.clock.now() + timeout;
        match recv_within(&self.clock, &self.reads, timeout) {
            Ok(Read::Bytes(bytes)) => Ok(ReceiveOutcome::copy_message(&bytes, dst)),
            Ok(Read::At(at, bytes)) => {
                self.scheduled = Some((at, bytes));
                self.recv_into_with_timeout(
                    dst,
                    deadline.saturating_duration_since(self.clock.now()),
                )
            }
            Ok(Read::Truncated(prefix)) => {
                let copied = prefix.len().min(dst.len());
                dst[..copied].copy_from_slice(&prefix[..copied]);
                Ok(ReceiveOutcome::truncated(copied))
            }
            Ok(Read::Overreported) => Ok(ReceiveOutcome::complete(dst.len() + 1)),
            Ok(Read::EndOfStream) => Ok(ReceiveOutcome::complete(0)),
            Ok(Read::Fail(error)) => Err(error),
            Ok(Read::Panic) => panic!("injected transport panic"),
            Ok(Read::Callback(callback)) => {
                callback();
                Err(Error::io_timeout())
            }
            Err(flume::RecvTimeoutError::Timeout) => Err(Error::io_timeout()),
            Err(flume::RecvTimeoutError::Disconnected) => {
                self.clock.sleep(timeout);
                Err(Error::io_timeout())
            }
        }
    }
}

/// The test's end of a [`ChannelTransport`].
struct Peer {
    clock: Clock,
    reads: flume::Sender<Read>,
    writes: flume::Receiver<Vec<u8>>,
    read_calls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
    write_callback: Arc<Mutex<Option<TransportCallback>>>,
}

impl Peer {
    fn next_write(&self) -> Vec<u8> {
        recv_within(&self.clock, &self.writes, PROMPTLY).expect("the worker writes promptly")
    }

    fn send(&self, read: Read) {
        self.reads.send(read).unwrap();
    }

    fn reply(&self, bytes: &[u8]) {
        self.send(Read::Bytes(bytes.to_vec()));
    }

    fn transport_dropped(&self) -> bool {
        self.dropped.load(Ordering::Acquire)
    }

    /// Wait until the worker has dropped its transport.
    fn await_transport_drop(&self) {
        let deadline = Instant::now() + PROMPTLY;
        while !self.transport_dropped() {
            assert!(Instant::now() < deadline, "the worker kept its transport");
            thread::sleep(Duration::from_millis(1));
        }
    }
}

struct Setup {
    clock: Clock,
    semantics: SendSemantics,
    fail_writes: bool,
    eager_idle: bool,
    profile: ProfileSpec,
    raw_inquiry_release_hold: Option<Duration>,
    read_timeout: Duration,
}

impl Setup {
    fn datagram() -> Self {
        Self::datagram_for(generic_profile())
    }

    fn datagram_for(profile: ProfileSpec) -> Self {
        Self {
            clock: Clock::System,
            semantics: SendSemantics::Datagram,
            fail_writes: false,
            eager_idle: false,
            profile,
            raw_inquiry_release_hold: None,
            read_timeout: Duration::from_millis(200),
        }
    }

    fn stream() -> Self {
        Self {
            semantics: SendSemantics::Stream,
            ..Self::datagram()
        }
    }

    /// Run the worker and the channel transport on `clock`.
    fn on(self, clock: &ManualClock) -> Self {
        Self {
            clock: Clock::Manual(clock.clone()),
            ..self
        }
    }

    fn transport(self) -> (ChannelTransport, Peer, ProfileSpec, Option<Duration>) {
        let (read_tx, reads) = flume::unbounded();
        let (writes, write_rx) = flume::unbounded();
        let read_calls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let write_callback = Arc::new(Mutex::new(None));
        let transport = ChannelTransport {
            clock: self.clock.clone(),
            config: TransportConfig {
                read_timeout: self.read_timeout,
                ..TransportConfig::default()
            },
            semantics: self.semantics,
            reads,
            writes,
            fail_writes: self.fail_writes,
            eager_idle: self.eager_idle,
            read_calls: Arc::clone(&read_calls),
            dropped: Arc::clone(&dropped),
            scheduled: None,
            write_callback: Arc::clone(&write_callback),
        };
        let peer = Peer {
            clock: self.clock.clone(),
            reads: read_tx,
            writes: write_rx,
            read_calls,
            dropped,
            write_callback,
        };
        (transport, peer, self.profile, self.raw_inquiry_release_hold)
    }

    fn spawn(self) -> (BlockingOwnerHandle, Peer) {
        let clock = self.clock.clone();
        let (transport, peer, profile, hold) = self.transport();
        let (adapter, adapter_policy) =
            BlockingTransportAdapter::new(transport, &profile, crate::target::TargetSlot::CAMERA_1)
                .unwrap();
        let mut policy = adapter_policy.clone();
        if let Some(hold) = hold {
            policy.protocol.raw_inquiry_release_hold = hold;
        }
        (
            BlockingOwnerHandle::spawn(policy, adapter, clock).unwrap(),
            peer,
        )
    }
}

/// The raw-envelope profile most worker tests run under.
fn generic_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<GenericVisca>().unwrap()
}

fn focus_manual() -> crate::prepared::PreparedCommand {
    focus_manual_for(&generic_profile())
}

fn focus_manual_for(profile: &ProfileSpec) -> crate::prepared::PreparedCommand {
    prepare_command(
        &FocusModeCommand::Manual,
        &crate::TargetRegistration::new(
            (crate::target::TargetSlot::CAMERA_1).individual_camera(),
            profile.clone(),
        ),
        OperationalTuning::new(),
        ClassSelection::Request,
    )
    .unwrap()
}

/// A command whose application updates the target's applied-state cache.
fn limit_clear_for(profile: &ProfileSpec) -> crate::prepared::PreparedCommand {
    prepare_command(
        &PanTiltLimitClear::for_profile(PanTiltLimitCorner::DownLeft, profile).unwrap(),
        &crate::TargetRegistration::new(
            (crate::target::TargetSlot::CAMERA_1).individual_camera(),
            profile.clone(),
        ),
        OperationalTuning::new(),
        ClassSelection::Request,
    )
    .unwrap()
}

/// Wraps `payload` as the Sony reply to the encapsulated `request`, carrying
/// the request's own sequence number.
fn sony_reply_to(request: &[u8], payload: &[u8]) -> Vec<u8> {
    let sequence = u32::from_be_bytes(request[4..8].try_into().unwrap());
    let mut reply = SonyHeader::new_reply(payload.len(), sequence)
        .encode()
        .to_vec();
    reply.extend_from_slice(payload);
    reply
}

/// Wait until camera 1's applied-state cache records a pan/tilt limit.
fn await_pan_tilt_limits(owner: &BlockingOwnerHandle) {
    let deadline = Instant::now() + PROMPTLY;
    while owner
        .state_cache(crate::target::TargetSlot::CAMERA_1)
        .pan_tilt()
        .limit()
        .is_none()
    {
        assert!(
            Instant::now() < deadline,
            "the cache never recorded the limit"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn zoom_stop() -> crate::prepared::PreparedOperation<AppliedOnly> {
    prepare_operation::<AppliedOnly, _>(
        &ZoomStop,
        &crate::TargetRegistration::new(
            (crate::target::TargetSlot::CAMERA_1).individual_camera(),
            generic_profile().clone(),
        ),
        OperationalTuning::new(),
        ClassSelection::Request,
    )
    .unwrap()
}

/// A raw inquiry whose own reply deadline is `inquiry`.
fn raw_inquiry(inquiry: Duration) -> RuntimeRequest {
    RuntimeRequest::Inquiry {
        payload_shape: crate::PayloadShape::Unknown,
        wire: Arc::new(EncodedMessage::new(&[0x81, 0x09, 0x04, 0x47, 0xff]).unwrap()),
        context: RequestContext {
            requires_extended_pan_tilt_speed: false,
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: crate::target::TargetSlot::CAMERA_1,
            timeout: TimeoutPolicy {
                ack: crate::runtime::engine::NonZeroDuration::new(Duration::from_secs(1)).unwrap(),
                completion: crate::runtime::engine::NonZeroDuration::new(Duration::from_secs(1))
                    .unwrap(),
                inquiry: crate::runtime::engine::NonZeroDuration::new(inquiry).unwrap(),
                cancellation: crate::runtime::engine::NonZeroDuration::new(Duration::from_secs(1))
                    .unwrap(),
                ambiguity: crate::runtime::engine::NonZeroDuration::new(Duration::from_secs(1))
                    .unwrap(),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
        },
    }
}

/// D24: submission success means admission. A transport write failure is
/// reported through the operation's outcome, not by submission.
#[test]
fn submission_succeeds_at_admission_and_a_failed_write_is_the_outcome() {
    let (owner, _peer) = Setup {
        fail_writes: true,
        ..Setup::datagram()
    }
    .spawn();
    let receipt = owner
        .submit_command(focus_manual())
        .expect("admission does not wait for the write");
    assert!(matches!(
        receipt.wait(),
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    owner.close().unwrap();
}

/// D24/#779: while one thread waits on an operation that will not complete
/// soon, another thread's STOP is admitted and written within a read slice,
/// rather than failing because the session is busy.
#[test]
fn a_stop_from_another_thread_is_written_while_a_wait_is_in_progress() {
    let (owner, peer) = Setup::datagram().spawn();
    let waiting = owner.submit_command(focus_manual()).unwrap();
    assert_eq!(peer.next_write()[..2], [0x81, 0x01]);
    let waiter = thread::spawn(move || waiting.wait());

    let submitted = Instant::now();
    let mut stop = owner.submit_operation(zoom_stop()).unwrap();
    assert_eq!(peer.next_write(), ZOOM_STOP);
    assert!(
        submitted.elapsed() < Duration::from_millis(500),
        "the STOP waited {:?} behind another caller",
        submitted.elapsed()
    );

    // The STOP bypasses the raw single-candidate gate (#714), so an ACK can
    // bind to neither request; each completes on its own deadline instead.
    owner.close().unwrap();
    assert!(matches!(stop.applied(None), Err(Error::RuntimeShutdown)));
    assert!(matches!(
        waiter.join().unwrap(),
        Err(Error::RuntimeShutdown)
    ));
}

/// D24: a detached operation reaches its terminal outcome with no caller
/// inside the session.
#[test]
fn a_detached_operation_progresses_without_a_caller() {
    let (owner, peer) = Setup::datagram().spawn();
    drop(owner.submit_command(focus_manual()).unwrap());
    peer.next_write();
    peer.reply(ACK);
    peer.reply(COMPLETION);

    let deadline = Instant::now() + PROMPTLY;
    while owner.metrics().unwrap().terminal == 0 {
        assert!(
            Instant::now() < deadline,
            "the detached command never ended"
        );
        thread::sleep(Duration::from_millis(1));
    }
    owner.close().unwrap();
}

#[test]
fn terminal_metrics_stay_identical_while_a_blocking_caller_races_cancellations_with_close() {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let (owner, peer) = Setup::datagram().on(&clock).spawn();
    let operation = owner.submit_operation(zoom_stop()).unwrap();
    peer.next_write();
    peer.reply(ACK);
    let id = operation.observation.id();
    let cancelling = owner.clone();
    let (started, ready) = flume::bounded(1);
    let registration = clock.register();
    let caller = thread::spawn(move || {
        let _participant = registration.enter();
        let mut snapshots = Vec::new();
        for index in 0..128 {
            let (_observer, cell) = CancellationObserver::pair();
            let request = super::super::CancellationRequest { id, observer: cell };
            let deadline = cancelling.deadline_after(Duration::from_secs(1)).unwrap();
            assert!(matches!(
                cancelling.request_cancellation(request, deadline),
                Ok(()) | Err(Error::CancellationNotSupported | Error::RuntimeShutdown)
            ));
            if index == 0 {
                started.send(()).unwrap();
            }
            if let Some(snapshot) = cancelling.core.terminal_metrics() {
                snapshots.push(snapshot.unwrap());
            }
        }
        snapshots
    });
    owner
        .clock
        .select()
        .recv(&ready, |value| value.unwrap())
        .wait();
    owner.close().unwrap();
    let published = owner.metrics().unwrap();
    assert_eq!(published.session, crate::SessionStatus::Shutdown);
    for snapshot in caller.join().unwrap() {
        assert_eq!(snapshot, published);
    }
    for _ in 0..8 {
        let (_observer, cell) = CancellationObserver::pair();
        let request = super::super::CancellationRequest { id, observer: cell };
        assert!(matches!(
            owner.request_cancellation(
                request,
                owner.deadline_after(Duration::from_secs(1)).unwrap()
            ),
            Err(Error::RuntimeShutdown)
        ));
        assert!(matches!(
            owner.subscribe_diagnostics(1),
            Err(Error::RuntimeShutdown)
        ));
        assert!(owner.core.cancellations.is_empty());
        assert!(owner.core.control.is_empty());
        assert_eq!(owner.metrics().unwrap(), published);
    }
}

/// D25 lifecycle order: the worker emits the same canonical owner trace as
/// the async actor for an applied raw command.
#[test]
fn blocking_owner_matches_canonical_lifecycle_trace() {
    let (owner, peer) = Setup::datagram_for(generic_profile()).spawn();
    let diagnostics = owner.subscribe_diagnostics(128).unwrap();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    peer.reply(ACK);
    peer.reply(COMPLETION);
    receipt.wait().unwrap();

    // The worker resolves the receipt before it publishes the `Terminal`
    // diagnostic for the same effect, and the stream is best-effort, so a
    // returned wait does not mean the trace is complete. `close` joins the
    // worker; every event it published is queued once that returns.
    owner.close().unwrap();
    let events = std::iter::from_fn(|| diagnostics.try_recv());
    assert_eq!(canonical_owner_trace(events), CANONICAL_OWNER_TRACE);
}

/// Diagnostic subscription deadlines participate in the session's virtual clock.
#[test]
fn diagnostic_receive_timeout_uses_the_session_clock() {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let (owner, _peer) = Setup::datagram().on(&clock).spawn();
    let diagnostics = owner.subscribe_diagnostics(128).unwrap();
    while diagnostics.try_recv().is_some() {}
    let start = clock.now();
    // This registered thread keeps virtual time still during an untracked
    // wall-clock wait. Reaching the exact deadline therefore proves that the
    // subscription waits on the session clock, regardless of host CPU load.
    assert!(diagnostics
        .recv_timeout(Duration::from_secs(60))
        .unwrap()
        .is_none());
    assert_eq!(clock.now() - start, Duration::from_secs(60));
    owner.close().unwrap();
}

/// A diagnostic emitted at an owner deadline wakes the virtual subscription wait.
#[test]
fn diagnostic_receive_observes_an_owner_deadline_without_wall_time() {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let (owner, peer) = Setup::datagram().on(&clock).spawn();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    let ack_deadline = clock.now() + generic_profile().timing().ack_timeout();
    let diagnostics = owner.subscribe_diagnostics(128).unwrap();
    while diagnostics.try_recv().is_some() {}
    let start = clock.now();
    let event = diagnostics
        .recv_timeout(Duration::from_secs(60))
        .unwrap()
        .expect("the owner ACK deadline emits a diagnostic before the subscription deadline");
    assert!(matches!(event, DiagnosticEvent::DeadlineExpired {
        id,
        deadline: crate::observability::DiagnosticDeadline::Ack,
        ..
    } if id == crate::observability::DiagnosticId::from_owner(receipt.core.id.get())));
    assert!(start < ack_deadline);
    assert_eq!(clock.now(), ack_deadline);
    drop(receipt);
    owner.close().unwrap();
}

/// A caller outside the worker can still block until the worker publishes an event.
#[test]
fn diagnostic_blocking_receive_observes_an_owner_deadline_without_wall_time() {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let (owner, peer) = Setup::datagram().on(&clock).spawn();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    let ack_deadline = clock.now() + generic_profile().timing().ack_timeout();
    let diagnostics = owner.subscribe_diagnostics(128).unwrap();
    while diagnostics.try_recv().is_some() {}

    let event = diagnostics.recv_blocking().unwrap();
    assert!(matches!(event, DiagnosticEvent::DeadlineExpired {
        id,
        deadline: crate::observability::DiagnosticDeadline::Ack,
        ..
    } if id == crate::observability::DiagnosticId::from_owner(receipt.core.id.get())));
    assert_eq!(clock.now(), ack_deadline);
    drop(receipt);
    owner.close().unwrap();
}

/// Out-of-order peer-receipt retention on the Sony sequence-bearing envelope
/// (#542 §4): several in-flight blocking receipts each keep their own exact
/// terminal outcome, whatever order the camera answers in and the callers
/// wait in. The worker applies every reply as it arrives, so both outcomes are
/// retained before either caller waits.
#[test]
fn sony_blocking_waits_retain_out_of_order_peer_results() {
    let profile = ProfileSpec::from_compile_time::<SonyFr7>().unwrap();
    let (owner, peer) = Setup::datagram_for(profile.clone()).spawn();
    let first = owner.submit_command(focus_manual_for(&profile)).unwrap();
    let first_wire = peer.next_write();
    let second = owner.submit_command(focus_manual_for(&profile)).unwrap();
    let second_wire = peer.next_write();

    peer.reply(&sony_reply_to(&first_wire, &[0x90, 0x41, 0xff]));
    peer.reply(&sony_reply_to(&second_wire, &[0x90, 0x42, 0xff]));
    peer.reply(&sony_reply_to(&second_wire, &[0x90, 0x62, 0x02, 0xff]));
    peer.reply(&sony_reply_to(&first_wire, &[0x90, 0x51, 0xff]));

    first
        .wait()
        .expect("the first command keeps its own completion");
    // The error followed the second command's ACK, so it is unconfirmed with
    // the camera's exact error as its source (#795).
    assert!(matches!(
        second.wait(),
        Err(Error::CommandFailedAfterAck { source, .. }) if matches!(*source, Error::SyntaxError)
    ));
    assert_eq!(owner.metrics().unwrap().active, 0);
    owner.close().unwrap();
}

/// Issue #565: on the Sony sequence-bearing envelope a transient read failure
/// retries the in-flight command with its own sequence and leaves the session
/// running. The raw envelope has no request identity after a successful
/// write, so this case is Sony-only by name.
#[test]
fn sony_transient_blocking_read_fault_retries_and_keeps_the_session() {
    let profile = ProfileSpec::from_compile_time::<SonyFr7>().unwrap();
    let (owner, peer) = Setup::datagram_for(profile.clone()).spawn();
    let receipt = owner.submit_command(focus_manual_for(&profile)).unwrap();
    let written = peer.next_write();

    peer.send(Read::Fail(Error::Io {
        source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
    }));
    assert_eq!(
        peer.next_write(),
        written,
        "the same request is written again"
    );
    assert_eq!(
        owner.metrics().unwrap().session,
        crate::SessionStatus::Running
    );

    peer.reply(&sony_reply_to(&written, ACK));
    peer.reply(&sony_reply_to(&written, COMPLETION));
    receipt.wait().expect("the retried command completes");
    owner.close().unwrap();
}

/// Lifecycle (#542 §3): an observer whose wait times out detaches without
/// cancelling anything. The late application still reaches the target's
/// applied-state cache.
#[test]
fn observer_timeout_detaches_without_cancel_and_late_applied_still_caches() {
    let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
    let (owner, peer) = Setup::datagram_for(profile.clone()).spawn();
    let receipt = owner.submit_command(limit_clear_for(&profile)).unwrap();
    peer.next_write();

    // A deadline that has already passed: the wait times out at once.
    assert!(matches!(
        owner.wait_core_until(&receipt.core, Instant::now()),
        Err(Error::ObservationTimeout { .. })
    ));
    drop(receipt);

    peer.reply(ACK);
    peer.reply(COMPLETION);
    await_pan_tilt_limits(&owner);
    assert!(
        peer.writes.try_recv().is_err(),
        "an observer timeout never writes a cancellation"
    );
    owner.close().unwrap();
}

/// Lifecycle (#542 §3): dropping a receipt detaches its observer and nothing
/// else. The late completion still updates the applied-state cache, and only
/// the detached observation itself is lost.
#[test]
fn detached_late_completion_still_updates_the_state_cache() {
    let (owner, peer) = Setup::datagram_for(generic_profile()).spawn();
    let receipt = owner
        .submit_command(limit_clear_for(&generic_profile()))
        .unwrap();
    peer.next_write();
    drop(receipt);

    peer.reply(ACK);
    peer.reply(COMPLETION);
    await_pan_tilt_limits(&owner);
    let metrics = owner.metrics().unwrap();
    assert_eq!(
        metrics.dropped_observer_events, 1,
        "the detached receipt loses exactly its terminal observation"
    );
    assert_eq!(metrics.active, 0);
    owner.close().unwrap();
}

/// Only a zero-length read means the peer closed a stream; it ends the
/// session with that cause for waiters and for `close`.
#[test]
fn a_zero_byte_stream_read_closes_the_session() {
    let (owner, peer) = Setup::stream().spawn();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    peer.send(Read::EndOfStream);

    assert!(matches!(
        receipt.wait(),
        Err(Error::ConnectionClosed { .. })
    ));
    assert!(matches!(owner.close(), Err(Error::ConnectionClosed { .. })));
    assert!(peer.transport_dropped());
}

/// A fatal read retains the precise OS error, including keepalive expiry.
#[test]
fn fatal_stream_reads_retain_the_typed_os_cause() {
    for kind in [std::io::ErrorKind::BrokenPipe, std::io::ErrorKind::TimedOut] {
        let (owner, peer) = Setup::stream().spawn();
        let receipt = owner.submit_command(focus_manual()).unwrap();
        peer.next_write();
        peer.send(Read::Fail(Error::from(std::io::Error::from(kind))));
        let error = receipt.wait().expect_err("fatal read closes the stream");
        assert!(
            matches!(&error, Error::ConnectionClosed { source: Some(source) }
            if matches!(source.as_ref(), Error::Io { source: io } if io.kind() == kind))
        );
        assert!(std::error::Error::source(&error).is_some());
        assert!(matches!(owner.close(), Err(Error::ConnectionClosed { .. })));
    }
}

/// A caller transport that violates the supplied buffer bound ends the
/// stream session and preserves the typed reported and available counts.
#[test]
fn an_overreported_stream_read_ends_the_session_with_typed_counts() {
    let (owner, peer) = Setup::stream().spawn();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    peer.send(Read::Overreported);
    let error = receipt.wait().expect_err("invalid read ends the stream");
    let capacity = TransportConfig::default().buffer_config.recv_buffer_size;
    assert!(
        matches!(&error, Error::StreamPoisoned { source } if matches!(source.as_ref(), Error::BufferTooSmall { required, actual } if *required == capacity + 1 && *actual == capacity))
    );
    assert!(matches!(owner.close(), Err(Error::StreamPoisoned { .. })));
}

/// #625: a read that keeps failing transiently for long enough is a broken
/// transport. The worker paces the run and then ends the session with the
/// cause rather than retrying forever.
#[test]
fn a_persistent_transient_fault_run_closes_the_session() {
    let clock = ManualClock::new();
    let _driver = clock.enter();
    let (owner, peer) = Setup::datagram().on(&clock).spawn();
    for _ in 0..64 {
        peer.send(Read::Fail(Error::transport_error(std::io::Error::other(
            "transient",
        ))));
    }
    let closed = clock.now();
    let deadline = closed + Duration::from_secs(5);
    // Each snapshot blocks this thread on the clock, which lets the paced
    // worker's virtual time move on.
    let error = loop {
        match owner.metrics().unwrap().session {
            crate::SessionStatus::Running => {
                assert!(clock.now() < deadline, "the fault run never ended");
            }
            crate::SessionStatus::Closed => break owner.shutdown().unwrap_err(),
            status => panic!("unexpected terminal status: {status:?}"),
        }
    };
    assert!(
        matches!(&error, Error::ConnectionClosed { source: Some(source) }
            if matches!(source.as_ref(), Error::WithContext { context, source }
                if context.contains("consecutive receive faults") && matches!(source.as_ref(), Error::TransportError { .. }))),
        "{error:?}"
    );
    assert!(
        clock.now() - closed >= Duration::from_secs(1),
        "the run was paced out to its minimum span"
    );
}

/// A consumed datagram that does not decode is discarded; the session keeps
/// running and a later reply still completes the request.
#[test]
fn an_undecodable_datagram_is_discarded_and_the_session_continues() {
    let (owner, peer) = Setup::datagram().spawn();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    peer.send(Read::Truncated(ACK.to_vec()));
    peer.reply(ACK);
    peer.reply(COMPLETION);

    receipt.wait().unwrap();
    assert_eq!(owner.metrics().unwrap().ignored_malformed_frames, 1);
    owner.close().unwrap();
}

/// #979: exhausting the read budget cannot pre-empt ready admission, control,
/// cancellation, shutdown or engine deadlines, even when one probe spends it all.
#[test]
fn one_millisecond_reads_keep_boundary_lanes_and_deadlines_live() {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let (owner, peer) = Setup {
        read_timeout: READY_PROBE,
        ..Setup::datagram()
    }
    .on(&clock)
    .spawn();
    let command = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    peer.reply(ACK);
    peer.reply(COMPLETION);
    command.wait().unwrap();

    let mut stop = owner.submit_operation(zoom_stop()).unwrap();
    assert_eq!(peer.next_write(), ZOOM_STOP);
    peer.reply(ACK);
    peer.reply(COMPLETION);
    stop.applied(None).unwrap();

    let request = owner
        .submit_with_timeout(raw_inquiry(Duration::from_millis(20)).into(), PROMPTLY)
        .unwrap();
    peer.next_write();
    let deadline = owner.deadline_after(PROMPTLY).unwrap();
    assert!(matches!(
        owner.wait_core_until(&request, deadline),
        Ok(RuntimeOutcome::Inquiry(InquiryOutcome::Failed(Error::InquiryFailed { source })))
            if matches!(source.as_ref(), Error::Timeout { .. })
    ));
    owner.close().unwrap();
    assert!(peer.transport_dropped());
}

/// The system-clock path also makes progress when OS UDP read rounding makes
/// one probe exceed the configured idle budget.
#[test]
fn one_millisecond_native_udp_reads_allow_admission_and_close() {
    let silent_peer = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let config = TransportConfig {
        read_timeout: READY_PROBE,
        ..TransportConfig::default()
    };
    let transport = crate::transport::blocking::Udp::connect_with_config(
        &silent_peer.local_addr().unwrap().to_string(),
        config,
    )
    .unwrap();
    let (adapter, policy) = BlockingTransportAdapter::new(
        transport,
        &generic_profile(),
        crate::target::TargetSlot::CAMERA_1,
    )
    .unwrap();
    let owner = BlockingOwnerHandle::spawn(policy, adapter, Clock::System).unwrap();
    let command = owner.submit_command(focus_manual()).unwrap();
    owner.metrics().unwrap();
    owner.close().unwrap();
    assert!(matches!(command.wait(), Err(Error::RuntimeShutdown)));
}

/// #675: a transport that answers "no data" at once, ignoring its timeout,
/// is paced instead of spinning the worker.
#[test]
fn an_eager_idle_transport_is_paced() {
    let clock = ManualClock::new();
    let _driver = clock.enter();
    let (owner, peer) = Setup {
        eager_idle: true,
        ..Setup::datagram()
    }
    .on(&clock)
    .spawn();
    clock.sleep(Duration::from_millis(500));
    let reads = peer.read_calls.load(Ordering::Relaxed);
    owner.close().unwrap();
    assert!(reads < 100, "eager idle reads hot-spun: {reads} in 500 ms");
}

/// Local fake: run the production bounded-read helper against a serial
/// backend that truncates its device timeout to whole milliseconds.
#[cfg(feature = "transport-serial")]
#[test]
fn fractional_serial_idle_reads_do_not_escalate_receive_pauses() {
    struct TruncatingDevice {
        clock: Clock,
        timeout: Duration,
    }
    impl TruncatingDevice {
        fn arm(&mut self, timeout: Duration) -> std::io::Result<()> {
            self.timeout = crate::transport::serial::device_timeout(timeout);
            Ok(())
        }
    }
    impl std::io::Read for TruncatingDevice {
        fn read(&mut self, _dst: &mut [u8]) -> std::io::Result<usize> {
            let millis = self.timeout.as_millis().try_into().unwrap();
            self.clock.sleep(Duration::from_millis(millis));
            Err(std::io::ErrorKind::TimedOut.into())
        }
    }
    struct IdleSerial {
        device: TruncatingDevice,
        config: TransportConfig,
        read_starts: Arc<Mutex<Vec<Instant>>>,
    }
    impl HasTransportConfig for IdleSerial {
        fn identity(&self) -> crate::transport::TransportIdentity<'_> {
            crate::transport::TransportIdentity::Serial { bus: None }
        }
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }
    impl BlockingTransport for IdleSerial {
        fn send_with_timeout(&mut self, _bytes: &[u8], _timeout: Duration) -> Result<(), Error> {
            Ok(())
        }
        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            timeout: Duration,
        ) -> Result<ReceiveOutcome, Error> {
            use crate::{
                timeout::Deadline,
                transport::blocking::{read_before, TimedIo},
            };
            let clock = self.device.clock.clone();
            self.read_starts.lock().unwrap().push(clock.now());
            let deadline = Deadline::after(clock.now(), timeout, "read_timeout")?;
            read_before(
                &mut self.device,
                TruncatingDevice::arm,
                dst,
                deadline,
                TimedIo::SerialDevice,
                || clock.now(),
            )
            .map(ReceiveOutcome::complete)
        }
    }

    let clock = ManualClock::new();
    let _participant = clock.enter();
    let read_timeout = Duration::from_micros(9_600);
    let starts = Arc::new(Mutex::new(Vec::new()));
    let transport = IdleSerial {
        device: TruncatingDevice {
            clock: Clock::Manual(clock.clone()),
            timeout: Duration::ZERO,
        },
        config: TransportConfig::for_serial().with_read_timeout(read_timeout),
        read_starts: Arc::clone(&starts),
    };
    let (adapter, policy) = BlockingTransportAdapter::new(
        transport,
        &generic_profile(),
        crate::target::TargetSlot::CAMERA_1,
    )
    .unwrap();
    let owner = BlockingOwnerHandle::spawn(policy, adapter, Clock::Manual(clock.clone())).unwrap();
    clock.sleep(read_timeout * 10);
    owner.close().unwrap();
    let starts = starts.lock().unwrap();
    assert!(starts.len() >= 10, "idle reads were paused: {starts:?}");
    assert!(
        starts
            .windows(2)
            .all(|pair| pair[1].duration_since(pair[0]) <= Duration::from_millis(10)),
        "a full-budget serial idle read gained an extra receive pause: {starts:?}"
    );
}

/// #713/#775: a stale reply to a timed-out raw inquiry arrives inside its
/// release hold. The successor is written only after the hold, and the stale
/// reply cannot resolve it.
#[test]
fn a_stale_raw_reply_is_consumed_before_the_successor_is_written() {
    const HOLD: Duration = Duration::from_millis(150);
    let clock = ManualClock::new();
    let _driver = clock.enter();
    let (owner, peer) = Setup {
        raw_inquiry_release_hold: Some(HOLD),
        ..Setup::datagram()
    }
    .on(&clock)
    .spawn();

    let first = owner
        .submit_with_timeout(raw_inquiry(Duration::from_millis(20)).into(), PROMPTLY)
        .unwrap();
    peer.next_write();
    let deadline = owner.deadline_after(PROMPTLY).unwrap();
    assert!(matches!(
        owner.wait_core_until(&first, deadline),
        Ok(RuntimeOutcome::Inquiry(InquiryOutcome::Failed(Error::InquiryFailed { source }))) if matches!(source.as_ref(), Error::Timeout { .. })
    ));
    let timed_out = clock.now();

    let second = owner
        .submit_with_timeout(raw_inquiry(Duration::from_secs(1)).into(), PROMPTLY)
        .unwrap();
    peer.reply(&[0x90, 0x50, 0x01, 0xff]);
    peer.next_write();
    assert!(
        clock.now() - timed_out >= HOLD,
        "the successor was written inside the release hold"
    );
    peer.reply(&[0x90, 0x50, 0x03, 0xff]);
    let deadline = owner.deadline_after(PROMPTLY).unwrap();
    assert!(matches!(
        owner.wait_core_until(&second, deadline),
        Ok(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x03]
    ));
    owner.close().unwrap();
}

/// `close` joins the worker after it has dropped its transport, and a second
/// clone's close waits for the same release.
#[test]
fn close_joins_the_worker_after_it_releases_the_transport() {
    let (owner, peer) = Setup::datagram().spawn();
    let other = owner.clone();
    owner.close().unwrap();
    assert!(peer.transport_dropped());
    other.close().unwrap();
}

/// Exercise a transport calling close on its own worker both before and
/// after an external closer has taken the join handle. Keep the callback
/// parked until the join is in progress, so teardown cannot hide either race.
fn self_close_with_external_join(external_first: bool) {
    let (owner, peer) = Setup::datagram().spawn();
    let callback_owner = owner.clone();
    let (entered_tx, entered) = flume::bounded(1);
    let (proceed, proceed_rx) = flume::bounded(1);
    let (results_tx, results) = flume::bounded(1);
    let (return_tx, return_rx) = flume::bounded(1);
    peer.send(Read::Callback(Box::new(move || {
        entered_tx.send(()).unwrap();
        proceed_rx.recv_timeout(PROMPTLY).unwrap();
        // Repeated self-close must also fail without consuming the join.
        let first = callback_owner.close();
        let second = callback_owner.close();
        results_tx.send([first, second]).unwrap();
        return_rx.recv_timeout(PROMPTLY).unwrap();
    })));
    entered.recv_timeout(PROMPTLY).unwrap();

    let start_close = || {
        let closer = owner.clone();
        let (done_tx, done) = flume::bounded(1);
        let join = thread::spawn(move || {
            let result = closer.close();
            done_tx.send(result).unwrap();
        });
        let deadline = Instant::now() + PROMPTLY;
        while owner.worker.lock().unwrap().is_some() {
            assert!(
                Instant::now() < deadline,
                "external close did not take the join"
            );
            thread::yield_now();
        }
        (join, done)
    };
    let mut external = external_first.then(start_close);
    proceed.send(()).unwrap();
    for result in results
        .recv_timeout(PROMPTLY)
        .expect("self-close must not wait")
    {
        assert!(
            matches!(result, Err(Error::InvalidState { reason: message })
            if message == "blocking owner worker cannot close its own session")
        );
    }
    if !external_first {
        assert!(
            owner.worker.lock().unwrap().is_some(),
            "self-close kept the join handle"
        );
        external = Some(start_close());
    }
    let (join, done) = external.unwrap();
    assert!(matches!(done.try_recv(), Err(flume::TryRecvError::Empty)));
    assert!(
        !peer.transport_dropped(),
        "callback still owns the transport"
    );
    return_tx.send(()).unwrap();
    done.recv_timeout(PROMPTLY)
        .expect("external close must join")
        .unwrap();
    join.join().unwrap();
    assert!(peer.transport_dropped());
    owner.close().unwrap();
}

#[test]
fn worker_self_close_preserves_the_external_join() {
    self_close_with_external_join(false);
}

#[test]
fn worker_self_close_does_not_wait_for_an_external_join_in_progress() {
    self_close_with_external_join(true);
}

/// Dropping the last handle stops the worker and releases the transport
/// without a join.
#[test]
fn dropping_the_last_handle_stops_the_worker() {
    let (owner, peer) = Setup::datagram().spawn();
    drop(owner);
    peer.await_transport_drop();
}

/// An operation keeps the owner alive after every session handle is gone,
/// and remains observable.
#[test]
fn an_operation_keeps_the_worker_alive_after_its_session_handle_drops() {
    let (owner, peer) = Setup::datagram().spawn();
    let mut operation = owner.submit_operation(zoom_stop()).unwrap();
    drop(owner);
    peer.next_write();
    peer.reply(ACK);
    peer.reply(COMPLETION);
    operation.applied(None).unwrap();
    assert!(!peer.transport_dropped());
    drop(operation);
    peer.await_transport_drop();
}

/// A worker that panics fails closed: waiters and `close` see a terminal
/// error instead of hanging.
#[test]
fn a_worker_panic_is_a_terminal_error_for_waiters_and_close() {
    let (owner, peer) = Setup::datagram().spawn();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    peer.send(Read::Panic);

    assert!(matches!(receipt.wait(), Err(Error::OwnerTerminated)));
    assert!(matches!(owner.close(), Err(Error::OwnerTerminated)));
    assert!(peer.transport_dropped());
}

/// Constructor failure leaves no worker: a failed Sony reset drops the
/// transport before any thread starts.
#[test]
fn a_failed_startup_leaves_no_worker_and_drops_the_transport() {
    let (transport, peer, _, _) = Setup {
        fail_writes: true,
        ..Setup::datagram()
    }
    .transport();
    let config = crate::SessionConfig::from_compile_time::<SonyFr7>()
        .unwrap()
        .with_sony_sequence_reset_on_connect(true);
    assert!(matches!(
        crate::blocking::Session::open(transport, &config),
        Err(Error::TransportError { .. })
    ));
    assert!(peer.transport_dropped());
    assert_eq!(peer.read_calls.load(Ordering::Relaxed), 0);
}

/// Every blocking handle can be shared and moved between threads.
#[test]
fn blocking_handles_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BlockingOwnerHandle>();
    assert_send_sync::<crate::blocking::Session>();
    assert_send_sync::<crate::blocking::Camera<GenericVisca>>();
    assert_send_sync::<crate::blocking::Operation<AppliedOnly>>();
}

/// Pin the dependency race without relying on a scheduler hitting its tiny
/// window: the selector already observed Empty, then the last sender queued a
/// value and dropped, and the selected result is Disconnected.
#[test]
fn selected_disconnect_recovers_queued_success_and_admission_rejection_with_owner_live() {
    let (owner, _peer) = Setup::datagram().spawn();
    for answer in [Ok(17_u64), Err(Error::runtime_queue_full(1))] {
        let (sender, receiver) = flume::bounded(1);
        let success = answer.is_ok();
        sender.send(answer).unwrap();
        drop(sender);
        assert!(!owner.core.actor_alive.is_disconnected());
        let recovered = resolve_selected_receive(&receiver, Err(flume::RecvError::Disconnected))
            .map_err(|_| owner.core.disconnected_error())
            .expect("queued admission answer survives selected disconnect");
        if success {
            assert_eq!(recovered.unwrap(), 17);
        } else {
            assert!(matches!(
                recovered,
                Err(Error::RuntimeQueueFull { capacity: 1, .. })
            ));
        }
        assert!(receiver.try_recv().is_err(), "the reply is consumed once");
        assert!(!owner.core.actor_alive.is_disconnected());
    }
    owner.close().unwrap();
}

#[test]
fn selected_empty_disconnect_preserves_missing_and_published_owner_errors() {
    let (owner, _peer) = Setup::datagram().spawn();
    let (sender, receiver) = flume::bounded::<u64>(1);
    drop(sender);
    assert!(!owner.core.actor_alive.is_disconnected());
    assert!(matches!(
        owner.reply(&receiver).wait(),
        Err(Error::OwnerTerminated)
    ));
    owner.close().unwrap();
    assert!(owner.core.actor_alive.is_disconnected());
    assert!(matches!(
        owner.reply(&receiver).wait(),
        Err(Error::RuntimeShutdown)
    ));
    assert!(matches!(
        owner.reply(&receiver).wait_until(Instant::now() + PROMPTLY),
        Some(Err(Error::RuntimeShutdown))
    ));
}

#[test]
fn selected_disconnect_preserves_late_terminal_timeout_and_borrowing_cache() {
    use super::super::Observed;
    let (owner, peer) = Setup::datagram().spawn();
    let receipt = owner.submit_command(focus_manual()).unwrap();
    peer.next_write();
    let deadline = Instant::now() + PROMPTLY;
    let late = deadline + Duration::from_nanos(1);
    let (sender, receiver) = flume::bounded(1);
    sender
        .send(Observed {
            at: late,
            value: CommandOutcome::Applied,
        })
        .unwrap();
    drop(sender);
    let event = resolve_selected_receive(&receiver, Err(flume::RecvError::Disconnected));
    assert!(
        matches!(
            TerminalWake::Terminal(event.clone().ok()).conclude(
                &receipt.core,
                &owner.core,
                deadline
            ),
            Err(Error::ObservationTimeout { .. })
        ),
        "a linear command receipt also keeps the observer deadline"
    );
    let mut observation = OperationObservation::new(receipt.core, PROMPTLY);
    {
        let mut wait =
            OperationWait::new(&mut observation, OperationObservation::applied, deadline);
        assert!(matches!(
            wait.absorb(ObservationWake::Terminal(event.ok()), &owner.core),
            ControlFlow::Break(Err(Error::ObservationTimeout { .. }))
        ));
    }
    assert!(
        matches!(observation.applied(late), Some(Ok(()))),
        "late delivery stays cached for the next wait"
    );
    assert!(!owner.core.actor_alive.is_disconnected());
    owner.close().unwrap();
}

/// #838: a full read timeout already paced the worker. A reply arriving in
/// the following read must not sit behind another idle pause until its due pass.
#[test]
fn replies_after_a_full_idle_read_win_before_their_deadlines() {
    for semantics in [SendSemantics::Datagram, SendSemantics::Stream] {
        for (deadline_ms, reply_ms) in [(205, 202), (400, 210), (205, 205)] {
            let clock = ManualClock::new();
            let _driver = clock.enter();
            let (owner, peer) = Setup {
                semantics,
                ..Setup::datagram()
            }
            .on(&clock)
            .spawn();
            let receipt = owner
                .submit_with_timeout(
                    raw_inquiry(Duration::from_millis(deadline_ms)).into(),
                    PROMPTLY,
                )
                .unwrap();
            peer.next_write();
            peer.send(Read::At(
                clock.now() + Duration::from_millis(reply_ms),
                vec![0x90, 0x50, 0x03, 0xff],
            ));
            let deadline = owner.deadline_after(PROMPTLY).unwrap();
            assert!(
                matches!(owner.wait_core_until(&receipt, deadline), Ok(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [3])
            );
            assert_eq!(owner.metrics().unwrap().inquiry_timeouts, 0);
            owner.close().unwrap();
        }
    }
}

fn subscription_terminal(subscription: &super::super::DiagnosticSubscription) -> Error {
    loop {
        match subscription.recv_timeout(PROMPTLY) {
            Ok(Some(_)) => {}
            Ok(None) => panic!("the ended owner must disconnect its subscription"),
            Err(error) => return error,
        }
    }
}

#[test]
fn diagnostic_subscriptions_keep_the_blocking_owners_terminal_cause() {
    for failure in [0, 1, 2] {
        let setup = if failure == 2 {
            Setup {
                fail_writes: true,
                ..Setup::stream()
            }
        } else {
            Setup::datagram()
        };
        let (owner, peer) = setup.spawn();
        let subscription = owner.subscribe_diagnostics(1).unwrap();
        let receipt = owner.submit_command(focus_manual()).unwrap();
        if failure != 2 {
            peer.next_write();
            peer.send(if failure == 0 {
                Read::EndOfStream
            } else {
                Read::Panic
            });
        }
        let terminal = receipt.wait().unwrap_err();
        assert!(terminal.requires_new_session());
        if failure == 1 {
            assert!(matches!(owner.metrics(), Err(Error::OwnerTerminated)));
        } else {
            let metrics = owner.metrics().unwrap();
            assert_eq!(metrics.active, 0);
            assert_eq!(metrics.pending, 0);
            assert_eq!(
                metrics.session,
                if failure == 0 {
                    crate::SessionStatus::Closed
                } else {
                    crate::SessionStatus::Poisoned
                }
            );
        }
        assert!(owner.close().unwrap_err().requires_new_session());
        let reported = subscription_terminal(&subscription);
        assert!(reported.requires_new_session());
        assert_eq!(reported.to_string(), terminal.to_string());
    }
}

#[test]
fn diagnostic_subscriber_capacity_has_its_own_nonretryable_error() {
    let (owner, _peer) = Setup::datagram().spawn();
    let mut subscriptions = Vec::new();
    for _ in 0..4 {
        subscriptions.push(owner.subscribe_diagnostics(1).unwrap());
    }
    let error = owner.subscribe_diagnostics(1).unwrap_err();
    assert!(matches!(
        error,
        Error::DiagnosticSubscriberLimit { capacity: 4 }
    ));
    assert!(!matches!(
        error.recovery(),
        crate::Recovery::Resubmit { .. }
    ));
    subscriptions.pop();
    subscriptions.push(owner.subscribe_diagnostics(1).unwrap());
    owner.close().unwrap();
}

/// #838: the final part of a long stream command remains readable after a
/// full five-second idle read; the last millisecond is still inside its window.
#[test]
fn a_stream_completion_after_a_long_idle_read_wins_before_its_deadline() {
    let clock = ManualClock::new();
    let _driver = clock.enter();
    let (mut transport, peer, profile, _) = Setup::stream().on(&clock).transport();
    transport.config.read_timeout = Duration::from_secs(5);
    let (adapter, adapter_policy) =
        BlockingTransportAdapter::new(transport, &profile, crate::target::TargetSlot::CAMERA_1)
            .unwrap();
    let owner = BlockingOwnerHandle::spawn(
        adapter_policy.clone(),
        adapter,
        Clock::Manual(clock.clone()),
    )
    .unwrap();
    let mut context = *raw_inquiry(Duration::from_secs(1)).context();
    context.timeout.completion =
        crate::runtime::engine::NonZeroDuration::new(Duration::from_secs(6)).unwrap();
    let request = RuntimeRequest::Command {
        reply_shape: ReplyShape::AckThenCompletion,
        wire: Arc::new(EncodedMessage::new(ZOOM_STOP).unwrap()),
        context,
        applied_state: None,
    };
    let receipt = owner
        .submit_with_timeout(request.into(), Duration::from_secs(10))
        .unwrap();
    peer.next_write();
    let begun = clock.now();
    peer.reply(ACK);
    assert_eq!(owner.metrics().unwrap().active, 1);
    peer.send(Read::At(
        begun + Duration::from_millis(5_999),
        COMPLETION.to_vec(),
    ));
    let deadline = owner.deadline_after(Duration::from_secs(10)).unwrap();
    assert!(matches!(
        owner.wait_core_until(&receipt, deadline),
        Ok(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert_eq!(owner.metrics().unwrap().completion_timeouts, 0);
    owner.close().unwrap();
}

/// Park the serialized shell while calling the real blocking handle. This
/// deterministically exercises caller-owned expiry, both before enqueue and
/// while waiting for the owner to claim the queued halt.
#[test]
fn caller_halt_timeouts_record_each_stop_before_enqueue_or_owner_claim() {
    use super::super::{
        boundary::{AdmissionValidity, CancellationBoundary, OwnerHandleCore},
        halt::HaltBoundary,
        shell::{OwnerEvent, OwnerShellCore, TurnStep},
        OwnerState, RetainedStreamInput,
    };
    struct Idle;
    impl RetainedStreamInput for Idle {}

    for saturated in [false, true] {
        let clock = ManualClock::new();
        let _participant = clock.enter();
        let now = clock.now();
        let profile = generic_profile();
        let policy = super::super::adapter::owner_policy_for_targets(
            &[(crate::target::TargetSlot::CAMERA_1, &profile)],
            &TransportConfig::default(),
            SendSemantics::Stream,
        )
        .unwrap();
        let mut state = OwnerState::new(policy, now).unwrap();
        let diagnostics = super::super::tests::DiagnosticRecorder::new(&mut state);
        let (core, ends) = OwnerHandleCore::new(&state);
        let mut shell = OwnerShellCore::new(state, ends);
        let worker_id = thread::spawn(|| thread::current().id()).join().unwrap();
        let owner = BlockingOwnerHandle {
            core,
            clock: Clock::Manual(clock.clone()),
            worker_id,
            worker: Arc::new(Mutex::new(None)),
        };
        if saturated {
            for _ in 0..owner.core.cancellations.capacity().unwrap() {
                let (reply, _receiver) = flume::bounded(1);
                owner
                    .core
                    .cancellations
                    .try_send(CancellationBoundary::Halt(HaltBoundary {
                        prepared: crate::prepared::PreparedHalt {
                            target: crate::target::TargetSlot::CAMERA_1,
                            axes: None,
                            requests: [None, None, None],
                            budget: PROMPTLY,
                        },
                        validity: AdmissionValidity::until(now + PROMPTLY),
                        reply,
                    }))
                    .unwrap();
            }
        }
        let tuning =
            crate::session_config::ValidatedTuning::validate(OperationalTuning::new(), [&profile])
                .unwrap();
        let mut prepared = crate::prepared::prepare_halt(
            &crate::TargetRegistration::from_profile(crate::target::TargetSlot::CAMERA_1, profile),
            tuning,
        );
        prepared.budget = Duration::from_millis(10);
        let stop_count = prepared.requests.iter().flatten().count();
        assert!(stop_count > 0);
        let error = owner.halt(prepared, now).unwrap_err();
        assert_eq!(error.kind(), Error::admission_timeout().kind());
        assert_eq!(clock.now() - now, Duration::from_millis(10));
        shell.flush_pre_admission_rejections(true);
        if !saturated {
            let halt = shell.receivers.cancellations.recv().unwrap();
            assert!(matches!(
                shell.handle(
                    OwnerEvent::Cancellation(halt),
                    &mut Idle,
                    clock.now(),
                    false,
                ),
                TurnStep::Done(_)
            ));
        }
        assert_eq!(shell.state.metrics().admission_rejected, stop_count as u64);
        assert_eq!(shell.state.metrics().admitted, 0);
        assert_eq!(shell.state.metrics().writes, 0);
        assert_eq!(
            diagnostics
                .events()
                .iter()
                .filter(|event| matches!(event,
                    DiagnosticEvent::AdmissionRejected { error, .. }
                        if *error == Error::admission_timeout().kind()
                ))
                .count(),
            stop_count,
        );
    }
}
