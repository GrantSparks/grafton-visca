//! Worker tests (D24, #780).
//!
//! Every test runs the real worker thread over the production blocking
//! adapter and a channel-backed transport, so the selection loop, read
//! slicing, pacing and lifecycle are exercised as callers see them. Turn
//! semantics shared with the async actor are pinned by the shell core and
//! coordinator tests; these tests pin what is specific to the worker.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::super::{canonical_owner_trace, BlockingTransportAdapter, CANONICAL_OWNER_TRACE};
use super::*;
use crate::{
    command::CommandKind,
    command::PanTiltLimitCorner,
    completion::AppliedOnly,
    prepared::{prepare_command, prepare_operation, ClassSelection},
    profile::ProfileSpec,
    profiles::{GenericVisca, SonyFR7},
    protocol::sony::SonyHeader,
    request::builtin::{FocusModeCommand, PanTiltLimitClear, ZoomStop},
    runtime::engine::{
        CancellationPolicy, ControlPolicy, EncodedMessage, InquiryRoute, ReplyShape,
        RequestContext, RetryPolicy, TimeoutPolicy,
    },
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    CameraId, OperationalTuning,
};

const ACK: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETION: &[u8] = &[0x90, 0x51, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
/// How long a test waits for something the worker should do promptly.
const PROMPTLY: Duration = Duration::from_secs(2);

/// One scripted transport read.
enum Read {
    Bytes(Vec<u8>),
    EndOfStream,
    Fail(Error),
    Panic,
    Callback(Box<dyn FnOnce() + Send>),
}

/// A transport whose reads arrive from the test thread and whose writes are
/// reported back to it. A read honors its timeout, as the trait requires,
/// unless the transport is deliberately eager.
struct ChannelTransport {
    config: TransportConfig,
    semantics: SendSemantics,
    reads: flume::Receiver<Read>,
    writes: flume::Sender<Vec<u8>>,
    fail_writes: bool,
    eager_idle: bool,
    read_calls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
}

impl Drop for ChannelTransport {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

impl HasTransportConfig for ChannelTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ChannelTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        if self.fail_writes {
            return Err(Error::TransportError("injected write failure".into()));
        }
        let _ = self.writes.send(bytes.to_vec());
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        self.read_calls.fetch_add(1, Ordering::Relaxed);
        if self.eager_idle {
            return Err(Error::io_timeout());
        }
        match self.reads.recv_timeout(timeout) {
            Ok(Read::Bytes(bytes)) => {
                dst[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Ok(Read::EndOfStream) => Ok(0),
            Ok(Read::Fail(error)) => Err(error),
            Ok(Read::Panic) => panic!("injected transport panic"),
            Ok(Read::Callback(callback)) => {
                callback();
                Err(Error::io_timeout())
            }
            Err(flume::RecvTimeoutError::Timeout) => Err(Error::io_timeout()),
            Err(flume::RecvTimeoutError::Disconnected) => {
                thread::sleep(timeout);
                Err(Error::io_timeout())
            }
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        self.semantics
    }
}

/// The test's end of a [`ChannelTransport`].
struct Peer {
    reads: flume::Sender<Read>,
    writes: flume::Receiver<Vec<u8>>,
    read_calls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
}

impl Peer {
    fn next_write(&self) -> Vec<u8> {
        self.writes
            .recv_timeout(PROMPTLY)
            .expect("the worker writes promptly")
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
    semantics: SendSemantics,
    fail_writes: bool,
    eager_idle: bool,
    profile: ProfileSpec,
    raw_inquiry_release_hold: Option<Duration>,
}

impl Setup {
    fn datagram() -> Self {
        Self::datagram_for(generic_profile())
    }

    fn datagram_for(profile: ProfileSpec) -> Self {
        Self {
            semantics: SendSemantics::Datagram,
            fail_writes: false,
            eager_idle: false,
            profile,
            raw_inquiry_release_hold: None,
        }
    }

    fn stream() -> Self {
        Self {
            semantics: SendSemantics::Stream,
            ..Self::datagram()
        }
    }

    fn transport(self) -> (ChannelTransport, Peer, ProfileSpec, Option<Duration>) {
        let (read_tx, reads) = flume::unbounded();
        let (writes, write_rx) = flume::unbounded();
        let read_calls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let transport = ChannelTransport {
            config: TransportConfig {
                addressing: AddressingMode::Ip,
                read_timeout: Duration::from_millis(200),
                ..TransportConfig::default()
            },
            semantics: self.semantics,
            reads,
            writes,
            fail_writes: self.fail_writes,
            eager_idle: self.eager_idle,
            read_calls: Arc::clone(&read_calls),
            dropped: Arc::clone(&dropped),
        };
        let peer = Peer {
            reads: read_tx,
            writes: write_rx,
            read_calls,
            dropped,
        };
        (transport, peer, self.profile, self.raw_inquiry_release_hold)
    }

    fn spawn(self) -> (BlockingOwnerHandle, Peer) {
        let (transport, peer, profile, hold) = self.transport();
        let adapter =
            BlockingTransportAdapter::new(transport, &profile, CameraId::CAMERA_1).unwrap();
        let mut policy = adapter.policy().clone();
        if let Some(hold) = hold {
            policy.protocol.raw_inquiry_release_hold = hold;
        }
        (BlockingOwnerHandle::spawn(policy, adapter).unwrap(), peer)
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
        CameraId::CAMERA_1,
        profile,
        OperationalTuning::new(),
        ClassSelection::Request,
    )
    .unwrap()
}

/// A command whose application updates the target's applied-state cache.
fn limit_clear_for(profile: &ProfileSpec) -> crate::prepared::PreparedCommand {
    prepare_command(
        &PanTiltLimitClear::for_profile(PanTiltLimitCorner::DownLeft, profile).unwrap(),
        CameraId::CAMERA_1,
        profile,
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
        .state_cache(CameraId::CAMERA_1)
        .pan_tilt_limits()
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
        CameraId::CAMERA_1,
        &generic_profile(),
        OperationalTuning::new(),
        ClassSelection::Request,
    )
    .unwrap()
}

/// A raw inquiry whose own reply deadline is `inquiry`.
fn raw_inquiry(inquiry: Duration) -> RuntimeRequest {
    RuntimeRequest::Inquiry {
        wire: Arc::new(EncodedMessage::new(&[0x81, 0x09, 0x04, 0x47, 0xff]).unwrap()),
        context: RequestContext {
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: CameraId::CAMERA_1,
            timeout: TimeoutPolicy {
                ack: Duration::from_secs(1),
                completion: Duration::from_secs(1),
                inquiry,
                cancellation: Duration::from_secs(1),
                ambiguity: Duration::from_secs(1),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        },
        route: InquiryRoute::UNKNOWN,
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
    assert!(matches!(receipt.wait(), Err(Error::TransportError(_))));
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

/// Out-of-order peer-receipt retention on the Sony sequence-bearing envelope
/// (#542 §4): several in-flight blocking receipts each keep their own exact
/// terminal outcome, whatever order the camera answers in and the callers
/// wait in. The worker applies every reply as it arrives, so both outcomes are
/// retained before either caller waits.
#[test]
fn sony_blocking_waits_retain_out_of_order_peer_results() {
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().unwrap();
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
    assert!(matches!(second.wait(), Err(Error::SyntaxError)));
    assert_eq!(owner.metrics().unwrap().active, 0);
    owner.close().unwrap();
}

/// Issue #565: on the Sony sequence-bearing envelope a transient read failure
/// retries the in-flight command with its own sequence and leaves the session
/// running. The raw envelope has no request identity after a successful
/// write, so this case is Sony-only by name.
#[test]
fn sony_transient_blocking_read_fault_retries_and_keeps_the_session() {
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().unwrap();
    let (owner, peer) = Setup::datagram_for(profile.clone()).spawn();
    let receipt = owner.submit_command(focus_manual_for(&profile)).unwrap();
    let written = peer.next_write();

    peer.send(Read::Fail(Error::Io(Arc::new(std::io::Error::from(
        std::io::ErrorKind::ConnectionRefused,
    )))));
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
        wait_core_until(&receipt.core, &owner, Instant::now()),
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

/// #625: a read that keeps failing transiently for long enough is a broken
/// transport. The worker paces the run and then ends the session with the
/// cause rather than retrying forever.
#[test]
fn a_persistent_transient_fault_run_closes_the_session() {
    let (owner, peer) = Setup::datagram().spawn();
    for _ in 0..64 {
        peer.send(Read::Fail(Error::TransportError("transient".into())));
    }
    let closed = Instant::now();
    let deadline = closed + Duration::from_secs(5);
    let error = loop {
        match owner.metrics() {
            Err(error) => break error,
            Ok(_) => {
                assert!(Instant::now() < deadline, "the fault run never ended");
                thread::sleep(Duration::from_millis(10));
            }
        }
    };
    assert!(
        matches!(&error, Error::ConnectionClosed { reason: Some(reason) }
            if reason.contains("consecutive receive faults")),
        "{error:?}"
    );
    assert!(
        closed.elapsed() >= Duration::from_secs(1),
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
    peer.send(Read::Fail(Error::ResponseTooLarge { max_size: 3 }));
    peer.reply(ACK);
    peer.reply(COMPLETION);

    receipt.wait().unwrap();
    assert_eq!(owner.metrics().unwrap().ignored_malformed_frames, 1);
    owner.close().unwrap();
}

/// #675: a transport that answers "no data" at once, ignoring its timeout,
/// is paced instead of spinning the worker.
#[test]
fn an_eager_idle_transport_is_paced() {
    let (owner, peer) = Setup {
        eager_idle: true,
        ..Setup::datagram()
    }
    .spawn();
    thread::sleep(Duration::from_millis(500));
    let reads = peer.read_calls.load(Ordering::Relaxed);
    owner.close().unwrap();
    assert!(reads < 100, "eager idle reads hot-spun: {reads} in 500 ms");
}

/// #713/#775: a stale reply to a timed-out raw inquiry arrives inside its
/// release hold. The successor is written only after the hold, and the stale
/// reply cannot resolve it.
#[test]
fn a_stale_raw_reply_is_consumed_before_the_successor_is_written() {
    const HOLD: Duration = Duration::from_millis(150);
    let (owner, peer) = Setup {
        raw_inquiry_release_hold: Some(HOLD),
        ..Setup::datagram()
    }
    .spawn();

    let first = owner
        .submit_with_timeout(raw_inquiry(Duration::from_millis(20)), PROMPTLY)
        .unwrap();
    peer.next_write();
    let deadline = owner.deadline_after(PROMPTLY).unwrap();
    assert!(matches!(
        wait_core_until(&first, &owner, deadline),
        Ok(RuntimeOutcome::Failed(Error::Timeout { .. }))
    ));
    let timed_out = Instant::now();

    let second = owner
        .submit_with_timeout(raw_inquiry(Duration::from_secs(1)), PROMPTLY)
        .unwrap();
    peer.reply(&[0x90, 0x50, 0x01, 0xff]);
    peer.next_write();
    assert!(
        timed_out.elapsed() >= HOLD.saturating_sub(Duration::from_millis(20)),
        "the successor was written inside the release hold"
    );
    peer.reply(&[0x90, 0x50, 0x03, 0xff]);
    let deadline = owner.deadline_after(PROMPTLY).unwrap();
    assert!(matches!(
        wait_core_until(&second, &owner, deadline),
        Ok(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x03]
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
        assert!(matches!(result, Err(Error::InvalidState(message))
            if message == "blocking owner worker cannot close its own session"));
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

    assert!(matches!(receipt.wait(), Err(Error::InvalidState(_))));
    assert!(matches!(owner.close(), Err(Error::InvalidState(_))));
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
    let config = crate::SessionConfig::from_compile_time::<SonyFR7>()
        .unwrap()
        .with_sony_sequence_reset_on_connect(true);
    assert!(matches!(
        crate::blocking::Session::open(transport, config),
        Err(Error::TransportError(_))
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
    assert!(
        matches!(owner.reply(&receiver).wait(), Err(Error::InvalidState(reason))
        if reason == "owner actor disconnected without publishing a terminal result")
    );
    owner.close().unwrap();
    assert!(owner.core.actor_alive.is_disconnected());
    assert!(matches!(
        owner.reply(&receiver).wait(),
        Err(Error::RuntimeShutdown)
    ));
    assert!(matches!(
        owner
            .reply(&receiver)
            .wait_deadline(Instant::now() + PROMPTLY),
        Ok(Err(Error::RuntimeShutdown))
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
            value: RuntimeOutcome::Applied,
        })
        .unwrap();
    drop(sender);
    let event = resolve_selected_receive(&receiver, Err(flume::RecvError::Disconnected));
    assert!(
        matches!(
            ObservationWake::Terminal(event.clone().ok()).conclude(
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
