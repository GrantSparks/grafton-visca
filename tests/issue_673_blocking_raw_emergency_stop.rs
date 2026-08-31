//! Issue #673: a blocking emergency stop must reach the wire on a raw profile
//! even while the caller still holds an un-awaited operation handle.
//!
//! On raw VISCA the engine keeps at most one *unacknowledged* command in flight
//! (the single-candidate gate), so a socketless ACK can never be misattributed.
//! Before the fix, a second operation submitted while a first raw command was
//! still awaiting its ACK lost the first-dispatch race and — under the
//! operation handle's `RequireFirstWrite` contract — was rejected
//! `Error::TransportBusy` with **zero** bytes on the wire, so
//! `motion().stop_all_motion()` and typed `Urgent` stops (`ZoomStop`,
//! `FocusStop`, `PanTiltStop`) could not reach a moving camera. The blocking
//! owner now drains that pre-ACK gate by pumping the peer's ACK (bounded by the
//! submitting request's own ACK budget), so a command socket frees and the
//! stop's first write wins — while genuine socket-capacity contention still
//! fails fast, as it must.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::{AppliedOnly, Targeted},
    profile::ProfileSpec,
    raw::{self, RawReplyShape},
    request::builtin::{FocusStop, ZoomDrive, ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    AffectedAxes, ControlClass, Error, RetryClass, TimeoutClass,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];
const RAW_ZOOM_STOP: [u8; 6] = [0x81, 0x01, 0x04, 0x07, 0x00, 0xff];

/// A raw datagram camera whose reads are scripted per write. The reads a send
/// queues are only consumed by a *later* pump, so a command written but never
/// awaited stays `AwaitingAck` with its ACK waiting on the wire — exactly the
/// state that arms the single-candidate pre-ACK gate.
#[derive(Debug)]
struct RawProbeTransport {
    config: TransportConfig,
    per_send: VecDeque<Vec<Vec<u8>>>,
    reads: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<usize>>,
    read_count: Arc<Mutex<usize>>,
}

impl RawProbeTransport {
    fn new(per_send: Vec<Vec<Vec<u8>>>) -> Self {
        Self {
            config: TransportConfig::default(),
            per_send: per_send.into(),
            reads: VecDeque::new(),
            writes: Arc::new(Mutex::new(0)),
            read_count: Arc::new(Mutex::new(0)),
        }
    }

    fn write_counter(&self) -> Arc<Mutex<usize>> {
        Arc::clone(&self.writes)
    }

    fn read_counter(&self) -> Arc<Mutex<usize>> {
        Arc::clone(&self.read_count)
    }
}

impl HasTransportConfig for RawProbeTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for RawProbeTransport {
    fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        *self.writes.lock().expect("write count lock") += 1;
        for read in self.per_send.pop_front().unwrap_or_default() {
            self.reads.push_back(read);
        }
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_millis(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        *self.read_count.lock().expect("read count lock") += 1;
        let bytes = self.reads.pop_front().ok_or(Error::Timeout)?;
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn session_with_counters(
    per_send: Vec<Vec<Vec<u8>>>,
) -> (Session, Arc<Mutex<usize>>, Arc<Mutex<usize>>) {
    let transport = RawProbeTransport::new(per_send);
    let writes = transport.write_counter();
    let reads = transport.read_counter();
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket raw runtime profile"),
    );
    let session = Session::open(transport, config).expect("owner session");
    (session, writes, reads)
}

fn session(per_send: Vec<Vec<Vec<u8>>>) -> (Session, Arc<Mutex<usize>>) {
    let (session, writes, _reads) = session_with_counters(per_send);
    (session, writes)
}

fn write_count(writes: &Arc<Mutex<usize>>) -> usize {
    *writes.lock().expect("write count lock")
}

fn read_count(reads: &Arc<Mutex<usize>>) -> usize {
    *reads.lock().expect("read count lock")
}

fn raw_policy(reply_shape: RawReplyShape) -> raw::Policy {
    raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
        .expect("raw policy")
        .with_reply_shape(reply_shape)
}

fn raw_applied_only(reply_shape: RawReplyShape) -> raw::AppliedOnly {
    raw::AppliedOnly::with_policy(RAW_ZOOM_STOP, AffectedAxes::ZOOM, raw_policy(reply_shape))
        .expect("raw applied-only operation")
}

fn raw_targeted(reply_shape: RawReplyShape) -> raw::Targeted {
    raw::Targeted::with_policy(RAW_ZOOM_STOP, AffectedAxes::ZOOM, raw_policy(reply_shape))
        .expect("raw targeted operation")
}

/// The core defect: a typed `Urgent` stop submitted behind a live, un-awaited
/// raw operation handle must reach the wire, not fail `TransportBusy` with no
/// bytes written.
#[test]
fn urgent_stop_reaches_the_wire_behind_a_live_raw_operation_handle() {
    // send #1 (drive) queues only its ACK — read later, by the stop's drain.
    // send #2 (stop) queues the stop's own ACK + completion.
    let (session, writes) = session(vec![
        vec![ACK_SOCKET_ONE.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    // A continuous zoom drive is submitted and its handle held un-awaited: its
    // command is on the wire but still awaiting its ACK, arming the gate.
    let _drive = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("drive submitted");
    assert_eq!(
        write_count(&writes),
        1,
        "the drive command is on the wire and awaiting its ACK"
    );

    // Before the fix this returned Err(TransportBusy) with the write count
    // still at 1 while the camera kept moving.
    let stop = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the emergency stop must reach the wire behind the live drive handle");
    assert_eq!(
        write_count(&writes),
        2,
        "the stop's first write wins once the drive's ACK frees a socket"
    );

    // The session is still fully usable: the stop settles on its own frames.
    stop.applied()
        .expect("the stop settles on the live session");

    session.shutdown().expect("owner shutdown");
}

/// Two un-awaited operation handles can be held at once: the second wins its
/// first write by pumping the first's ACK, and both then settle.
#[test]
fn two_unawaited_operation_handles_both_win_their_first_write() {
    let (session, writes) = session(vec![
        // send #1 (first op): its ACK, drained by the second submit.
        vec![ACK_SOCKET_ONE.to_vec()],
        // send #2 (second op): both operations' completions and the second's
        // ACK, consumed when the handles are awaited.
        vec![
            ACK_SOCKET_TWO.to_vec(),
            COMPLETE_SOCKET_ONE.to_vec(),
            COMPLETE_SOCKET_TWO.to_vec(),
        ],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission wins its first write via the pump");
    assert_eq!(
        write_count(&writes),
        2,
        "both operation handles named a request whose first write succeeded"
    );

    first.applied().expect("first operation settles");
    second.applied().expect("second operation settles");

    session.shutdown().expect("owner shutdown");
}

/// The #673 drain is valid only when the submitting operation can become
/// eligible from the peer ACK alone. CompletionOnly still requires total
/// target idleness after that ACK, so the architecture's "sole obstacle"
/// contract requires a fail-fast `TransportBusy` with no peer read.
#[test]
fn completion_only_raw_operations_do_not_drain_an_unacknowledged_peer() {
    macro_rules! assert_completion_only_busy {
        ($kind:ty, $operation:expr, $label:literal) => {{
            let (session, writes, reads) =
                session_with_counters(vec![vec![ACK_SOCKET_ONE.to_vec()]]);
            let camera = session
                .camera::<NonDefaultCompileTimeProfile>()
                .expect("camera view");
            let predecessor = raw_applied_only(RawReplyShape::AckThenCompletion);
            let _predecessor = camera
                .submit::<AppliedOnly, _>(&predecessor)
                .expect("raw predecessor is written and awaits its ACK");
            assert_eq!(write_count(&writes), 1);
            assert_eq!(read_count(&reads), 0);

            let operation = $operation;
            let error = camera
                .submit::<$kind, _>(&operation)
                .expect_err("completion-only successor cannot reach first write");
            assert!(matches!(error, Error::TransportBusy), "{error:?}");
            assert_eq!(
                read_count(&reads),
                0,
                concat!($label, " must not drain the predecessor ACK")
            );
            assert_eq!(
                write_count(&writes),
                1,
                concat!($label, " must fail before its first write")
            );
            session.shutdown().expect("owner shutdown");
        }};
    }

    assert_completion_only_busy!(
        Targeted,
        raw_targeted(RawReplyShape::CompletionOnly),
        "targeted completion-only operation"
    );
    assert_completion_only_busy!(
        AppliedOnly,
        raw_applied_only(RawReplyShape::CompletionOnly),
        "applied-only completion-only operation"
    );
}

/// The issue's exact scenario: `motion().stop_all_motion()` reaches the wire
/// while a drive handle is live. It submits and settles pan/tilt, zoom, and
/// focus stops in turn; the first drains the drive's ACK.
#[test]
fn stop_all_motion_reaches_the_wire_while_a_drive_handle_is_live() {
    let (session, writes) = session(vec![
        // send #1 (drive): its ACK, drained by the pan/tilt stop.
        vec![ACK_SOCKET_ONE.to_vec()],
        // send #2/#3/#4 (the three stops): each settles on socket two, which is
        // free while the drive holds socket one.
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let _drive = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("drive submitted");
    assert_eq!(write_count(&writes), 1, "the drive is on the wire");

    camera
        .motion()
        .stop_all_motion()
        .expect("stop_all_motion reaches the wire and settles behind the live drive handle");
    assert_eq!(
        write_count(&writes),
        4,
        "the drive plus the three stop commands are all on the wire"
    );

    session.shutdown().expect("owner shutdown");
}

/// Genuine socket-capacity contention is *not* the pre-ACK gate and must still
/// fail fast: with both command sockets occupied, pumping the pending ACK would
/// not free one, so the fail-fast rejection stands with no extra write.
#[test]
fn genuine_socket_capacity_contention_still_fails_fast() {
    let (session, writes) = session(vec![
        // send #1 (op one): its ACK, drained by op two so op one becomes
        // Executing and holds socket one.
        vec![ACK_SOCKET_ONE.to_vec()],
        // send #2 (op two): its ACK is left on the wire so op two stays
        // AwaitingAck and holds socket two — both sockets are now occupied.
        vec![ACK_SOCKET_TWO.to_vec()],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let _first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let _second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission drains the first ACK and takes the other socket");
    assert_eq!(write_count(&writes), 2, "both sockets are now occupied");

    // Both sockets are genuinely full: even pumping the pending ACK only moves a
    // command from awaiting-ACK to executing without releasing a socket. The
    // third submit must fail fast, immediately, with no third write.
    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("a third command under genuine contention must fail fast");
    assert!(
        matches!(error, Error::TransportBusy),
        "genuine socket contention fails fast as TransportBusy, got {error:?}"
    );
    assert_eq!(
        write_count(&writes),
        2,
        "the fail-fast rejection writes nothing"
    );

    session.shutdown().expect("owner shutdown");
}
