//! Issue #630: the engine's four control-class lanes, through the blocking API.
//!
//! The blocking twin of `tests/issue_630_submission_class_async.rs`. A blocking
//! submission is admitted into the owner worker's queue (#780), so the same
//! class ordering is observable at the transport boundary: while both command
//! sockets are busy, the queued requests wait, and each freed socket goes to
//! the highest class waiting for it.

#![cfg(feature = "blocking")]

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    request::builtin::{FocusDrive, FocusModeCommand, ZoomDrive, ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, SubmissionClass,
};

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff];
const FOCUS_NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, 0xff];

/// A two-socket camera that acknowledges every write immediately and only
/// completes a socket when the test says so, which is what makes "the socket
/// freed, and this is what the owner wrote next" observable.
#[derive(Debug)]
struct LaneTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    send_count: usize,
}

impl LaneTransport {
    fn new() -> (Self, LaneProbe) {
        let (response_tx, responses) = flume::unbounded();
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx: response_tx.clone(),
                writes: Arc::clone(&writes),
                send_count: 0,
            },
            LaneProbe {
                response_tx,
                writes,
            },
        )
    }
}

#[derive(Clone, Debug)]
struct LaneProbe {
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl LaneProbe {
    /// Completes one command socket, freeing it for the next dispatch.
    fn complete_socket(&self, socket: u8) {
        self.response_tx
            .send(vec![0x90, 0x50 | socket, 0xff])
            .expect("owner response channel remains connected");
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }

    fn wait_for_writes(&self, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.writes().len() < count {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {count} writes; saw {:?}",
                self.writes(),
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    /// Asserts that no further write appears while the queue is deliberately
    /// starved of sockets.
    fn assert_stable_write_count(&self, count: usize) {
        thread::sleep(Duration::from_millis(10));
        assert_eq!(
            self.writes().len(),
            count,
            "no queued request may be written while every socket is busy",
        );
    }
}

impl HasTransportConfig for LaneTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for LaneTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.send_count = self.send_count.saturating_add(1);
        // Two sockets, assigned round-robin exactly as a real camera would as
        // each one becomes free again.
        let socket = u8::try_from(self.send_count % 2).unwrap_or(1) + 1;
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        self.response_tx
            .send(vec![0x90, 0x40 | socket, 0xff])
            .map_err(|_| Error::connection_closed(None))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        let bytes = self
            .responses
            .recv_timeout(timeout)
            .map_err(|error| match error {
                flume::RecvTimeoutError::Timeout => Error::io_timeout(),
                flume::RecvTimeoutError::Disconnected => Error::connection_closed(None),
            })?;
        let length = bytes.len();
        dst[..length].copy_from_slice(&bytes);
        Ok(length)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn g2_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile"),
    )
}

/// A background-class submission yields the freed socket to a user-class
/// submission made after it.
#[test]
fn background_yields_to_a_later_user_submission() {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("first socket");
    probe.wait_for_writes(1);
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .expect("second socket");
    probe.wait_for_writes(2);

    let background = camera
        .with_submission_class(SubmissionClass::Background)
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .expect("background submission is admitted and queued");
    let user = camera
        .with_submission_class(SubmissionClass::User)
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect("user submission is admitted and queued");
    probe.assert_stable_write_count(2);

    probe.complete_socket(2);
    probe.wait_for_writes(3);
    assert_eq!(
        probe.writes()[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: the later user-class request takes the freed socket",
    );

    probe.complete_socket(1);
    probe.wait_for_writes(4);
    assert_eq!(
        probe.writes()[3],
        ZOOM_WIDE.to_vec(),
        "the background request is written once nothing outranks it",
    );
    assert_eq!(
        probe.writes()[0..2].to_vec(),
        vec![ZOOM_TELE.to_vec(), FOCUS_FAR.to_vec()],
    );

    first.detach();
    second.detach();
    user.detach();
    background.detach();
    session.shutdown().expect("owner shutdown");
}

/// The handle default demotes one handle's traffic; a sibling clone keeps the
/// built-in classification, and a derived view can outrank the default.
#[test]
fn handle_default_and_derived_view_override() {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("first socket");
    probe.wait_for_writes(1);
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .expect("second socket");
    probe.wait_for_writes(2);

    let mut poller = camera.clone();
    assert_eq!(poller.submission_class(), None);
    poller.set_submission_class(Some(SubmissionClass::Background));
    assert_eq!(poller.submission_class(), Some(SubmissionClass::Background));
    assert_eq!(
        camera.submission_class(),
        None,
        "a clone diverges rather than sharing the default",
    );

    // Both are `SubmissionClass::User` built-ins, so only the handle they came
    // from can separate them.
    let demoted = poller
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .expect("demoted submission");
    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect("ordinary submission");
    probe.assert_stable_write_count(2);

    probe.complete_socket(2);
    probe.wait_for_writes(3);
    assert_eq!(
        probe.writes()[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: the demoted handle's earlier request yields the socket",
    );

    probe.complete_socket(1);
    probe.wait_for_writes(4);
    assert_eq!(probe.writes()[3], ZOOM_WIDE.to_vec());

    // Now the override: the demoted handle submits first and still wins.
    let raised = poller
        .with_submission_class(SubmissionClass::User)
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("raised submission");
    let later = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .expect("later ordinary submission");
    probe.assert_stable_write_count(4);
    assert_eq!(
        poller.submission_class(),
        Some(SubmissionClass::Background),
        "a derived view does not change the source handle default",
    );

    probe.complete_socket(2);
    probe.wait_for_writes(5);
    assert_eq!(
        probe.writes()[4],
        ZOOM_TELE.to_vec(),
        "issue #630: the derived view's class wins over the handle default",
    );

    first.detach();
    second.detach();
    demoted.detach();
    ordinary.detach();
    raised.detach();
    later.detach();
    session.shutdown().expect("owner shutdown");
}

/// A typed stop retains its urgent safety class under both QoS override forms.
#[test]
fn urgent_cannot_be_demoted_by_any_submission_override() {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("first socket");
    probe.wait_for_writes(1);
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .expect("second socket");
    probe.wait_for_writes(2);

    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect("ordinary user-class submission");
    let mut poller = camera.clone();
    poller.set_submission_class(Some(SubmissionClass::Background));
    let stop = poller
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("stop from a demoted handle");
    probe.assert_stable_write_count(2);

    probe.complete_socket(2);
    probe.wait_for_writes(3);
    assert_eq!(
        probe.writes()[2],
        ZOOM_STOP.to_vec(),
        "issue #630: an urgent stop keeps its class under a demoted handle",
    );

    probe.complete_socket(1);
    probe.wait_for_writes(4);
    assert_eq!(probe.writes()[3], FOCUS_NEAR.to_vec());

    // A derived view's QoS value is also unable to weaken the stop's intrinsic
    // urgent safety floor.
    let protected_stop = camera
        .with_submission_class(SubmissionClass::Background)
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("stop with background ordinary-traffic QoS");
    let later = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .expect("later user-class submission");
    probe.assert_stable_write_count(4);

    probe.complete_socket(2);
    probe.wait_for_writes(5);
    assert_eq!(
        probe.writes()[4],
        ZOOM_STOP.to_vec(),
        "issue #630: a derived view cannot demote an urgent stop",
    );

    first.detach();
    second.detach();
    ordinary.detach();
    stop.detach();
    protected_stop.detach();
    later.detach();
    session.shutdown().expect("owner shutdown");
}

/// The single-camera session carries the default into every view it hands out.
#[test]
fn a_camera_session_default_reaches_the_views_it_hands_out() {
    let (transport, _probe) = LaneTransport::new();
    let mut session = grafton_visca::blocking::CameraSession::<PtzOpticsG2>::open(
        transport,
        &grafton_visca::blocking::CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1"),
    )
    .expect("single-camera session");

    assert_eq!(session.submission_class(), None);
    assert_eq!(session.camera().submission_class(), None);

    session.set_submission_class(Some(SubmissionClass::Background));
    assert_eq!(
        session.submission_class(),
        Some(SubmissionClass::Background)
    );
    assert_eq!(
        session.camera().submission_class(),
        Some(SubmissionClass::Background),
        "issue #630: a view taken after the call carries the session default",
    );

    session.set_submission_class(None);
    assert_eq!(session.camera().submission_class(), None);

    session.close().expect("owner shutdown");
}

/// A class-selected view's `execute` reaches the same owner path as its
/// source-view twin.
#[test]
fn classified_execute_reaches_the_wire() {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let mut camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    thread::scope(|scope| {
        let completion = scope.spawn(|| {
            probe.wait_for_writes(1);
            probe.complete_socket(2);
        });
        camera
            .with_submission_class(SubmissionClass::Background)
            .execute(&FocusModeCommand::Manual)
            .expect("background plain command still executes");
        completion.join().expect("completion thread");
    });

    camera.set_submission_class(Some(SubmissionClass::User));
    thread::scope(|scope| {
        let completion = scope.spawn(|| {
            probe.wait_for_writes(2);
            probe.complete_socket(1);
        });
        camera
            .execute(&FocusModeCommand::Auto)
            .expect("handle default plain command still executes");
        completion.join().expect("completion thread");
    });

    assert_eq!(probe.writes().len(), 2);
    session.shutdown().expect("owner shutdown");
}
