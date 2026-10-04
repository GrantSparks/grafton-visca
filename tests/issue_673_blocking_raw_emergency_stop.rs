//! Issue #673: a blocking emergency stop must reach the wire on a raw profile
//! even while the caller still holds an un-awaited operation handle.
//!
//! On raw VISCA the engine normally keeps one *unacknowledged* command in flight
//! (the single-candidate gate), so a socketless ACK is never guessed. Before
//! the fix, a second operation submitted while a first raw command was still
//! awaiting its ACK was rejected with zero bytes on the wire, so
//! `motion().stop_all_motion()` and typed `Urgent` stops (`ZoomStop`,
//! `FocusStop`, `PanTiltStop`) could not reach a moving camera.
//!
//! The blocking owner is now a native worker thread (#780): a submission is
//! admitted into its queue and written when the scheduler allows, exactly as
//! on the async owner. Ordinary work waits for the gate (and for #714's
//! bounded lost-ACK quarantine); intrinsic `Urgent` stops bypass it and write
//! within physical pacing, and if two raw candidates are then open an ACK
//! binds to neither. Socket-capacity contention queues instead of failing.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::{AppliedOnly, Targeted},
    profile::ProfileSpec,
    raw::{self, RawReplyShape},
    request::builtin::{FocusStop, ZoomDrive},
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    AffectedAxes, CameraId, ControlClass, Error, OperationalTuning, RetryClass, TimeoutClass,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
const CAMERA_TWO_ACK_SOCKET_ONE: &[u8] = &[0xa0, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];
const RAW_ZOOM_STOP: [u8; 6] = [0x81, 0x01, 0x04, 0x07, 0x00, 0xff];

#[derive(Debug)]
struct Script {
    steps: VecDeque<Vec<Vec<u8>>>,
    writes: Vec<Vec<u8>>,
}

/// A raw datagram camera whose replies are scripted per write: each send
/// queues its step's replies, which the owner worker reads as soon as they
/// are on the wire. The probe can also deliver a late reply on its own.
#[derive(Debug)]
struct RawProbeTransport {
    config: TransportConfig,
    script: Arc<Mutex<Script>>,
    reply_tx: flume::Sender<Vec<u8>>,
    replies: flume::Receiver<Vec<u8>>,
}

#[derive(Clone, Debug)]
struct Probe {
    script: Arc<Mutex<Script>>,
    reply_tx: flume::Sender<Vec<u8>>,
}

impl Probe {
    fn writes(&self) -> Vec<Vec<u8>> {
        self.script.lock().expect("script lock").writes.clone()
    }

    fn write_count(&self) -> usize {
        self.script.lock().expect("script lock").writes.len()
    }

    fn wait_for_writes(&self, count: usize, timeout: Duration) {
        let started = Instant::now();
        while self.write_count() < count {
            assert!(
                started.elapsed() < timeout,
                "timed out waiting for write {count}; saw {:?}",
                self.writes()
            );
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Asserts that no further write appears while the scheduler deliberately
    /// holds the queued work.
    fn assert_stable_write_count(&self, count: usize) {
        thread::sleep(Duration::from_millis(20));
        assert_eq!(self.write_count(), count, "queued work must stay unwritten");
    }

    /// Waits until the worker has read every reply on the wire. The worker
    /// handles a read before it selects its next boundary, so work submitted
    /// afterwards is scheduled against the replies already applied.
    fn wait_until_replies_read(&self) {
        let started = Instant::now();
        while !self.reply_tx.is_empty() {
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "the owner worker never read the queued replies"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Delivers one reply that no write scripted.
    fn reply(&self, bytes: &[u8]) {
        self.reply_tx
            .send(bytes.to_vec())
            .expect("owner reply channel remains connected");
    }
}

impl RawProbeTransport {
    fn new(steps: Vec<Vec<Vec<u8>>>) -> (Self, Probe) {
        let script = Arc::new(Mutex::new(Script {
            steps: steps.into(),
            writes: Vec::new(),
        }));
        let (reply_tx, replies) = flume::unbounded();
        let config = {
            let mut config = TransportConfig::default();
            config.addressing = AddressingMode::Serial;
            config
        };
        (
            Self {
                config,
                script: Arc::clone(&script),
                reply_tx: reply_tx.clone(),
                replies,
            },
            Probe { script, reply_tx },
        )
    }
}

impl HasTransportConfig for RawProbeTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for RawProbeTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        let replies = {
            let mut script = self.script.lock().expect("script lock");
            script.writes.push(bytes.to_vec());
            script.steps.pop_front().unwrap_or_default()
        };
        for reply in replies {
            self.reply_tx
                .send(reply)
                .map_err(|_| Error::connection_closed(None))?;
        }
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        let reply = self
            .replies
            .recv_timeout(timeout)
            .map_err(|error| match error {
                flume::RecvTimeoutError::Timeout => Error::io_timeout(),
                flume::RecvTimeoutError::Disconnected => Error::connection_closed(None),
            })?;
        dst[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket raw runtime profile"),
    )
}

fn two_camera_session_config(command_spacing: Duration) -> SessionConfig {
    let mut config = session_config();
    config
        .register_target(
            CameraId::CAMERA_2,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                .expect("two-socket raw runtime profile"),
        )
        .expect("second serial target");
    config
        .with_tuning(OperationalTuning::new().command_spacing(command_spacing))
        .expect("test command spacing")
}

fn session_with_config(steps: Vec<Vec<Vec<u8>>>, config: SessionConfig) -> (Session, Probe) {
    let (transport, probe) = RawProbeTransport::new(steps);
    let session = Session::open(transport, config).expect("owner session");
    (session, probe)
}

fn session(steps: Vec<Vec<Vec<u8>>>) -> (Session, Probe) {
    session_with_config(steps, session_config())
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

fn raw_applied_only_for(target: CameraId, reply_shape: RawReplyShape) -> raw::AppliedOnly {
    let mut wire = RAW_ZOOM_STOP;
    wire[0] = target.to_address_byte();
    raw::AppliedOnly::with_policy(wire, AffectedAxes::ZOOM, raw_policy(reply_shape))
        .expect("targeted raw applied-only operation")
}

fn raw_targeted(reply_shape: RawReplyShape) -> raw::Targeted {
    raw::Targeted::with_policy(RAW_ZOOM_STOP, AffectedAxes::ZOOM, raw_policy(reply_shape))
        .expect("raw targeted operation")
}

/// The core defect: a typed `Urgent` stop submitted behind a live, un-awaited
/// raw operation handle reaches the wire and applies.
#[test]
fn urgent_stop_reaches_the_wire_behind_a_live_raw_operation_handle() {
    let (session, probe) = session(vec![
        vec![ACK_SOCKET_ONE.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    // A continuous zoom drive is submitted and its handle held un-awaited: its
    // command is on the wire and executing on socket one.
    let drive = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("drive submitted");
    probe.wait_for_writes(1, Duration::from_secs(1));
    probe.wait_until_replies_read();

    let mut stop = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the emergency stop is admitted behind the live drive handle");
    stop.applied()
        .expect("the stop reaches the wire and applies on the free socket");
    assert_eq!(
        probe.write_count(),
        2,
        "the drive and the stop are on the wire"
    );

    drive.detach();
    session.shutdown().expect("owner shutdown");
}

/// Two un-awaited operation handles can be held at once: the second is
/// written once the first's ACK clears the gate, and both then settle.
#[test]
fn two_unawaited_operation_handles_both_reach_the_wire() {
    let (session, probe) = session(vec![
        vec![ACK_SOCKET_ONE.to_vec()],
        vec![
            ACK_SOCKET_TWO.to_vec(),
            COMPLETE_SOCKET_ONE.to_vec(),
            COMPLETE_SOCKET_TWO.to_vec(),
        ],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let first_request = raw_applied_only(RawReplyShape::AckThenCompletion);
    let second_request = raw_applied_only(RawReplyShape::AckThenCompletion);
    let mut first = camera
        .submit::<AppliedOnly, _>(&first_request)
        .expect("first submission");
    let mut second = camera
        .submit::<AppliedOnly, _>(&second_request)
        .expect("second submission");

    first.applied().expect("first operation settles");
    second.applied().expect("second operation settles");
    assert_eq!(probe.write_count(), 2, "both operations reached the wire");

    session.shutdown().expect("owner shutdown");
}

/// A completion-only raw operation cannot become eligible from a peer ACK
/// alone: it requires total target idleness, so it stays queued while an
/// acknowledged raw predecessor is still executing and is written once that
/// predecessor completes.
#[test]
fn completion_only_raw_operations_wait_for_an_idle_target() {
    macro_rules! assert_completion_only_waits {
        ($kind:ty, $operation:expr, $label:literal) => {{
            let (session, probe) = session(vec![vec![ACK_SOCKET_ONE.to_vec()]]);
            let camera = session
                .camera::<NonDefaultCompileTimeProfile>()
                .expect("camera view");
            let predecessor = raw_applied_only(RawReplyShape::AckThenCompletion);
            let mut predecessor = camera
                .submit::<AppliedOnly, _>(&predecessor)
                .expect("raw predecessor is admitted");
            probe.wait_for_writes(1, Duration::from_secs(1));

            let operation = $operation;
            let successor = camera
                .submit::<$kind, _>(&operation)
                .expect("completion-only successor is admitted");
            probe.assert_stable_write_count(1);

            probe.reply(COMPLETE_SOCKET_ONE);
            predecessor.applied().expect("the predecessor completes");
            probe.wait_for_writes(2, Duration::from_secs(1));
            assert_eq!(
                probe.writes()[1],
                RAW_ZOOM_STOP.to_vec(),
                concat!($label, " is written once the target is idle")
            );
            successor.detach();
            session.shutdown().expect("owner shutdown");
        }};
    }

    assert_completion_only_waits!(
        Targeted,
        raw_targeted(RawReplyShape::CompletionOnly),
        "targeted completion-only operation"
    );
    assert_completion_only_waits!(
        AppliedOnly,
        raw_applied_only(RawReplyShape::CompletionOnly),
        "applied-only completion-only operation"
    );
}

/// The issue's exact scenario: `motion().stop_all_motion()` reaches the wire
/// while a drive handle is live. It submits pan/tilt, zoom, and focus stops in
/// turn, each of which applies on the socket the drive leaves free.
#[test]
fn stop_all_motion_reaches_the_wire_while_a_drive_handle_is_live() {
    let (session, probe) = session(vec![
        vec![ACK_SOCKET_ONE.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec(), COMPLETE_SOCKET_TWO.to_vec()],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let drive = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("drive submitted");
    probe.wait_for_writes(1, Duration::from_secs(1));
    probe.wait_until_replies_read();

    camera
        .motion()
        .stop_all_motion()
        .expect("every stop applies while the drive is live")
        .into_result()
        .expect("each supported STOP applied");
    assert_eq!(
        probe.write_count(),
        4,
        "the drive plus the three stop commands are all on the wire"
    );

    drive.detach();
    session.shutdown().expect("owner shutdown");
}

/// #714: after the predecessor's ACK deadline, its raw ambiguity quarantine is
/// a bounded wait, not generic contention. An ordinary successor is admitted
/// at once and written when the quarantine releases.
#[test]
fn ordinary_successor_waits_for_lost_ack_quarantine() {
    let (session, probe) = session(vec![
        vec![],
        vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let mut predecessor = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("predecessor admission");
    probe.wait_for_writes(1, Duration::from_millis(50));
    let first_written_at = Instant::now();
    thread::sleep(Duration::from_millis(150));
    let ordinary = raw_applied_only(RawReplyShape::AckThenCompletion);
    let mut successor = camera
        .submit::<AppliedOnly, _>(&ordinary)
        .expect("ordinary successor admission");
    probe.wait_for_writes(2, Duration::from_secs(2));
    let release_elapsed = first_written_at.elapsed();

    assert!(
        release_elapsed >= Duration::from_millis(950),
        "ordinary successor wrote before quarantine release: {release_elapsed:?}"
    );
    assert!(
        release_elapsed < Duration::from_secs(2),
        "ordinary successor missed quarantine release: {release_elapsed:?}"
    );
    assert!(matches!(
        predecessor.applied(),
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    successor
        .applied()
        .expect("ordinary successor settles after release");

    session.shutdown().expect("owner shutdown");
}

/// #714: an intrinsic Urgent stop never waits on the pre-ACK gate. It writes
/// within physical pacing even with a lost predecessor ACK; ACKs observed
/// while both positional candidates are open bind to neither.
#[test]
fn urgent_stop_bypasses_lost_ack_gate_and_ambiguous_ack_binds_to_neither() {
    let (session, probe) = session(vec![
        vec![],
        vec![
            ACK_SOCKET_ONE.to_vec(),
            ACK_SOCKET_TWO.to_vec(),
            COMPLETE_SOCKET_TWO.to_vec(),
        ],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let mut predecessor = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("predecessor admission");
    probe.wait_for_writes(1, Duration::from_millis(50));
    let urgent_started = Instant::now();
    let mut urgent = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("urgent admission");
    probe.wait_for_writes(2, Duration::from_millis(50));
    assert!(
        urgent_started.elapsed() < Duration::from_millis(50),
        "urgent first write exceeded its pacing bound"
    );
    assert!(matches!(
        urgent.applied(),
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(matches!(
        predecessor.applied(),
        Err(Error::UnsequencedCommandUnconfirmed)
    ));

    session.shutdown().expect("owner shutdown");
}

/// #744: a cancellation that has an exact socket on camera 2 is local to
/// that camera. Camera 1's ordinary raw submission waits through physical
/// pacing, but is not held behind camera 2's pending cancellation.
#[test]
fn pending_camera_two_cancel_does_not_hold_camera_one_work() {
    const SPACING: Duration = Duration::from_millis(10);

    // Camera 2's first ACK lets its successor establish the predecessor's
    // socket. Cancelling that predecessor then leaves a paced socket-cancel
    // pending exactly while camera 1 submits ordinary raw work.
    let (session, probe) = session_with_config(
        vec![
            vec![CAMERA_TWO_ACK_SOCKET_ONE.to_vec()],
            vec![],
            vec![],
            vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()],
        ],
        two_camera_session_config(SPACING),
    );
    let camera_one = session
        .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
        .expect("camera one view");
    let camera_two = session
        .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
        .expect("camera two view");

    let mut predecessor = camera_two
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("camera two predecessor admission");
    probe.wait_for_writes(1, Duration::from_secs(1));
    let socket_successor = camera_two
        .submit::<AppliedOnly, _>(&raw_applied_only_for(
            CameraId::CAMERA_2,
            RawReplyShape::AckThenCompletion,
        ))
        .expect("camera two successor admission");
    probe.wait_for_writes(2, Duration::from_secs(1));
    // A zero-length wait records the intent and returns before the paced
    // socket-cancel is written; the camera never answers it.
    assert!(matches!(
        predecessor.cancel_with_timeout(Duration::ZERO),
        Err(Error::ObservationTimeout { .. })
    ));

    let camera_one_request = raw_applied_only(RawReplyShape::AckThenCompletion);
    let mut camera_one_operation = camera_one
        .submit::<AppliedOnly, _>(&camera_one_request)
        .expect("camera one admission");
    camera_one_operation
        .applied()
        .expect("camera one operation settles after its first write");

    assert_eq!(
        probe.writes(),
        [
            vec![0x82, 0x01, 0x04, 0x07, 0x02, 0xff],
            vec![0x82, 0x01, 0x04, 0x07, 0x00, 0xff],
            vec![0x82, 0x21, 0xff],
            RAW_ZOOM_STOP.to_vec(),
        ],
        "the camera two cancel consumes the shared pacing slot, but camera one writes next"
    );

    socket_successor.detach();
    session.shutdown().expect("owner shutdown");
}

/// Socket-capacity contention is not the pre-ACK gate: with both command
/// sockets occupied, a third request is admitted and waits for a socket,
/// writing nothing until one frees.
#[test]
fn socket_capacity_contention_queues_until_a_socket_frees() {
    let (session, probe) = session(vec![
        vec![ACK_SOCKET_ONE.to_vec()],
        vec![ACK_SOCKET_TWO.to_vec()],
        vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()],
    ]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let first_request = raw_applied_only(RawReplyShape::AckThenCompletion);
    let second_request = raw_applied_only(RawReplyShape::AckThenCompletion);
    let third_request = raw_applied_only(RawReplyShape::AckThenCompletion);
    let mut first = camera
        .submit::<AppliedOnly, _>(&first_request)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&second_request)
        .expect("second submission");
    probe.wait_for_writes(2, Duration::from_secs(1));
    probe.wait_until_replies_read();

    let mut third = camera
        .submit::<AppliedOnly, _>(&third_request)
        .expect("a third command under socket contention is admitted");
    probe.assert_stable_write_count(2);

    probe.reply(COMPLETE_SOCKET_ONE);
    first.applied().expect("the first command completes");
    third
        .applied()
        .expect("the third command is written on the freed socket and settles");
    assert_eq!(probe.write_count(), 3);

    second.detach();
    session.shutdown().expect("owner shutdown");
}
