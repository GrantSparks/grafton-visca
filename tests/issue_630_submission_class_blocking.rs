//! Issue #630: the engine's four control-class lanes, through blocking API.
//!
//! The owner has always dispatched ready work from the highest occupied
//! control class first; before this issue the only public route into that
//! choice was `raw::Policy` on hand-assembled bytes. These tests observe the
//! restored typed route at the transport boundary: nothing is asserted about
//! the private queues, only about which request is written when a socket
//! frees.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Camera, Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    request::builtin::{FocusDrive, FocusModeCommand, ZoomDrive, ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    ControlClass, Error,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff];
const FOCUS_NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, 0xff];

/// A two-socket camera: every accepted command is answered with an ACK and a
/// completion on alternating sockets, in the order the commands were written.
#[derive(Debug)]
struct TwoSocketTransport {
    config: TransportConfig,
    responses: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    next_socket: u8,
}

impl TwoSocketTransport {
    fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses: VecDeque::new(),
                writes: Arc::clone(&writes),
                next_socket: 0,
            },
            writes,
        )
    }
}

impl HasTransportConfig for TwoSocketTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for TwoSocketTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        let socket = (self.next_socket % 2) + 1;
        self.next_socket = self.next_socket.wrapping_add(1);
        self.responses.push_back(vec![0x90, 0x40 | socket, 0xff]);
        self.responses.push_back(vec![0x90, 0x50 | socket, 0xff]);
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_secs(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        let response = self.responses.pop_front().ok_or(Error::Timeout)?;
        dst[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn written(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<Vec<u8>> {
    writes.lock().expect("writes lock").clone()
}

/// Fills both command sockets so that everything submitted afterwards is
/// unambiguously queued, and returns the handles that free them again.
fn occupy_both_sockets<'session>(
    camera: &Camera<'session, NonDefaultCompileTimeProfile>,
    writes: &Arc<Mutex<Vec<Vec<u8>>>>,
) -> [grafton_visca::blocking::Operation<'session, AppliedOnly>; 2] {
    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("first socket");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .expect("second socket");
    assert_eq!(
        written(writes),
        vec![ZOOM_TELE.to_vec(), FOCUS_FAR.to_vec()],
        "both sockets are occupied before anything is queued",
    );
    [first, second]
}

/// A background-class submission yields the freed socket to a user-class
/// submission made *after* it.
#[test]
fn a_background_submission_yields_the_socket_to_a_later_user_submission() {
    let (transport, writes) = TwoSocketTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let background = camera
        .submit_with_class::<AppliedOnly, _>(&ZoomDrive::Wide, ControlClass::Background)
        .expect("background submission is admitted and queued");
    let user = camera
        .submit_with_class::<AppliedOnly, _>(&FocusDrive::Near, ControlClass::User)
        .expect("user submission is admitted and queued");
    assert_eq!(
        written(&writes).len(),
        2,
        "neither queued request is written while both sockets are busy",
    );

    first.applied().expect("first socket freed");
    assert_eq!(
        written(&writes),
        vec![ZOOM_TELE.to_vec(), FOCUS_FAR.to_vec(), FOCUS_NEAR.to_vec(),],
        "issue #630: the later user-class request takes the freed socket",
    );

    second.applied().expect("second socket freed");
    assert_eq!(
        written(&writes),
        vec![
            ZOOM_TELE.to_vec(),
            FOCUS_FAR.to_vec(),
            FOCUS_NEAR.to_vec(),
            ZOOM_WIDE.to_vec(),
        ],
        "the background request is written once nothing outranks it",
    );

    user.applied().expect("user operation applied");
    background.applied().expect("background operation applied");
    session.shutdown().expect("owner shutdown");
}

/// A handle default demotes everything that handle submits, including through
/// its noun accessors, while a sibling view keeps the built-in classification.
#[test]
fn a_handle_default_demotes_that_handles_traffic() {
    let (transport, writes) = TwoSocketTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let mut poller = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("second camera view");
    assert_eq!(
        poller.command_class(),
        None,
        "a fresh view submits in each request's own class",
    );
    poller.set_command_class(Some(ControlClass::Background));
    assert_eq!(poller.command_class(), Some(ControlClass::Background));
    assert_eq!(
        camera.command_class(),
        None,
        "the default belongs to one handle, not to the session",
    );

    // `ZoomDrive` is a `ControlClass::User` built-in, so the two submissions
    // below differ only in which handle made them.
    let demoted = poller
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .expect("demoted submission");
    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect("ordinary submission");
    assert_eq!(written(&writes).len(), 2);

    first.applied().expect("first socket freed");
    assert_eq!(
        written(&writes)[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: the demoted handle's earlier request yields the socket",
    );

    second.applied().expect("second socket freed");
    assert_eq!(written(&writes)[3], ZOOM_WIDE.to_vec());

    demoted.applied().expect("demoted operation applied");
    ordinary.applied().expect("ordinary operation applied");
    session.shutdown().expect("owner shutdown");
}

/// A per-submission class outranks the handle default for that one request.
#[test]
fn a_per_submission_class_overrides_the_handle_default() {
    let (transport, writes) = TwoSocketTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let mut poller = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("second camera view");
    poller.set_command_class(Some(ControlClass::Background));

    // Submitted first, and from the demoted handle: it can only be written
    // first if the per-submission class replaced that handle's default.
    let raised = poller
        .submit_with_class::<AppliedOnly, _>(&ZoomDrive::Wide, ControlClass::User)
        .expect("raised submission");
    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect("ordinary submission");
    assert_eq!(written(&writes).len(), 2);
    assert_eq!(
        poller.command_class(),
        Some(ControlClass::Background),
        "a per-submission class does not change the handle default",
    );

    first.applied().expect("first socket freed");
    assert_eq!(
        written(&writes)[2],
        ZOOM_WIDE.to_vec(),
        "issue #630: the per-submission class wins over the handle default",
    );

    second.applied().expect("second socket freed");
    assert_eq!(written(&writes)[3], FOCUS_NEAR.to_vec());

    raised.applied().expect("raised operation applied");
    ordinary.applied().expect("ordinary operation applied");
    session.shutdown().expect("owner shutdown");
}

/// The safety rule: a handle demoted to background still preempts with a stop.
#[test]
fn a_handle_default_never_demotes_an_urgent_stop() {
    let (transport, writes) = TwoSocketTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect("ordinary user-class submission");

    let mut poller = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("second camera view");
    poller.set_command_class(Some(ControlClass::Background));
    let stop = poller
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("stop from a demoted handle");
    assert_eq!(written(&writes).len(), 2);

    first.applied().expect("first socket freed");
    assert_eq!(
        written(&writes)[2],
        ZOOM_STOP.to_vec(),
        "issue #630: an urgent stop keeps its class under a demoted handle",
    );

    second.applied().expect("second socket freed");
    assert_eq!(written(&writes)[3], FOCUS_NEAR.to_vec());

    stop.applied().expect("stop applied");
    ordinary.applied().expect("ordinary operation applied");
    session.shutdown().expect("owner shutdown");
}

/// The documented escape hatch: an explicit per-submission class may demote a
/// stop, which nothing else in the crate does.
#[test]
fn an_explicit_per_submission_class_may_demote_an_urgent_stop() {
    let (transport, writes) = TwoSocketTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let stop = camera
        .submit_with_class::<AppliedOnly, _>(&ZoomStop, ControlClass::Background)
        .expect("deliberately demoted stop");
    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect("ordinary user-class submission");
    assert_eq!(written(&writes).len(), 2);

    first.applied().expect("first socket freed");
    assert_eq!(
        written(&writes)[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: an explicit background class demotes even a stop",
    );

    second.applied().expect("second socket freed");
    assert_eq!(written(&writes)[3], ZOOM_STOP.to_vec());

    stop.applied().expect("stop applied");
    ordinary.applied().expect("ordinary operation applied");
    session.shutdown().expect("owner shutdown");
}

/// The single-camera session carries the default into every view it hands out.
#[test]
fn a_camera_session_default_reaches_the_views_it_hands_out() {
    let (transport, _writes) = TwoSocketTransport::new();
    let mut session = grafton_visca::blocking::CameraSession::<PtzOpticsG2>::open(
        transport,
        &grafton_visca::blocking::CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1"),
    )
    .expect("single-camera session");

    assert_eq!(session.command_class(), None);
    assert_eq!(session.camera().command_class(), None);

    session.set_command_class(Some(ControlClass::Background));
    assert_eq!(session.command_class(), Some(ControlClass::Background));
    assert_eq!(
        session.camera().command_class(),
        Some(ControlClass::Background),
        "issue #630: a view taken after the call carries the session default",
    );

    session.set_command_class(None);
    assert_eq!(session.camera().command_class(), None);

    session.close().expect("owner shutdown");
}

/// `execute_with_class` and `inquire_with_class` reach the wire on the same
/// path as their unclassified twins.
#[test]
fn classified_execute_reaches_the_wire() {
    let (transport, writes) = TwoSocketTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let mut camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .execute_with_class(&FocusModeCommand::Manual, ControlClass::Background)
        .expect("background plain command still executes");
    camera.set_command_class(Some(ControlClass::Urgent));
    camera
        .execute(&FocusModeCommand::Auto)
        .expect("handle default plain command still executes");

    assert_eq!(
        written(&writes).len(),
        2,
        "both plain commands reached the transport",
    );
    session.shutdown().expect("owner shutdown");
}
