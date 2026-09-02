//! Issue #630: typed control-class selection through the blocking API.
//!
//! The owner has four control-class lanes, but the blocking operation API now
//! requires each returned handle's first write to succeed. These tests keep
//! the handle/default and derived-view class surface covered at the
//! admission boundary while asserting that no class can make a blocked public
//! operation queue behind occupied sockets. Actual queued class ordering
//! remains covered by the async counterpart (and the engine tests).

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
    profiles::{PtzOpticsG2, SonyFR7},
    request::builtin::{FocusDrive, FocusModeCommand, ZoomDrive, ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, SubmissionClass,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const FOCUS_FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff];

/// A two-socket camera: every accepted command is answered with an ACK and a
/// completion on alternating sockets, in the order the commands were written.
/// Sony mode wraps each reply with the exact sequence from its request so the
/// two pre-ACK writes are correlated by the same evidence as the real camera.
#[derive(Debug)]
struct TwoSocketTransport {
    config: TransportConfig,
    responses: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    next_socket: u8,
    sony: bool,
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
                sony: false,
            },
            writes,
        )
    }

    fn with_sony(mut self) -> Self {
        self.sony = true;
        self
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
        let ack = vec![0x90, 0x40 | socket, 0xff];
        let completion = vec![0x90, 0x50 | socket, 0xff];
        if self.sony {
            self.responses.push_back(sony_reply(bytes, &ack));
            self.responses.push_back(sony_reply(bytes, &completion));
        } else {
            self.responses.push_back(ack);
            self.responses.push_back(completion);
        }
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

fn sony_session_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile"))
}

fn sony_reply(request: &[u8], payload: &[u8]) -> Vec<u8> {
    assert!(
        request.len() >= 8,
        "Sony request must carry its sequence header"
    );
    let mut response = Vec::with_capacity(8 + payload.len());
    response.extend_from_slice(&[0x01, 0x11]);
    response.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("Sony reply payload length")
            .to_be_bytes(),
    );
    response.extend_from_slice(&request[4..8]);
    response.extend_from_slice(payload);
    response
}

fn sony_payload(frame: &[u8]) -> &[u8] {
    assert!(
        frame.len() >= 8,
        "Sony write must carry its sequence header"
    );
    &frame[8..]
}

fn written(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<Vec<u8>> {
    writes.lock().expect("writes lock").clone()
}

/// Fills both command sockets so that a public operation submitted afterwards
/// must satisfy the blocking first-write contract, and returns the handles
/// that free them again.
fn occupy_both_sockets<'session>(
    camera: &Camera<'session, SonyFR7>,
    writes: &Arc<Mutex<Vec<Vec<u8>>>>,
) -> [grafton_visca::blocking::Operation<'session, AppliedOnly>; 2] {
    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("first socket");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .expect("second socket");
    let written = written(writes);
    assert_eq!(written.len(), 2);
    assert_eq!(sony_payload(&written[0]), ZOOM_TELE);
    assert_eq!(sony_payload(&written[1]), FOCUS_FAR);
    [first, second]
}

/// A background-class public operation cannot wait in a blocking queue for a
/// freed socket. Queued class ordering is covered by the async counterpart.
#[test]
fn a_background_operation_cannot_queue_behind_busy_sockets() {
    let (transport, writes) = TwoSocketTransport::new();
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let background = camera
        .with_submission_class(SubmissionClass::Background)
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .expect_err("a blocking operation cannot queue behind busy sockets");
    assert!(matches!(background, Error::TransportBusy));
    let user = camera
        .with_submission_class(SubmissionClass::User)
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .expect_err("a blocking operation cannot queue behind busy sockets");
    assert!(matches!(user, Error::TransportBusy));
    assert_eq!(
        written(&writes).len(),
        2,
        "neither rejected operation is written while both sockets are busy",
    );

    first.applied().expect("first socket freed");
    second.applied().expect("second socket freed");
    assert_eq!(written(&writes).len(), 2);
    session.shutdown().expect("owner shutdown");
}

/// A handle-local default is independent of its sibling views and cannot
/// bypass the blocking first-write boundary.
#[test]
fn a_handle_default_does_not_bypass_blocking_first_write() {
    let (transport, writes) = TwoSocketTransport::new();
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let mut poller = session.camera::<SonyFR7>().expect("second camera view");
    assert_eq!(
        poller.submission_class(),
        None,
        "a fresh view submits in each request's own class",
    );
    poller.set_submission_class(Some(SubmissionClass::Background));
    assert_eq!(poller.submission_class(), Some(SubmissionClass::Background));
    assert_eq!(
        camera.submission_class(),
        None,
        "the default belongs to one handle, not to the session",
    );

    // `ZoomDrive` is a `SubmissionClass::User` built-in. The handle-local default
    // is configured successfully, but cannot make an unwritten handle escape.
    let demoted = poller
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .expect_err("a demoted blocking operation cannot queue");
    assert!(matches!(demoted, Error::TransportBusy));
    assert_eq!(written(&writes).len(), 2);

    first.applied().expect("first socket freed");
    second.applied().expect("second socket freed");
    assert_eq!(written(&writes).len(), 2);
    session.shutdown().expect("owner shutdown");
}

/// A class-selected derived view does not alter the source handle default and
/// cannot bypass the first-write boundary.
#[test]
fn a_derived_view_class_does_not_bypass_blocking_first_write() {
    let (transport, writes) = TwoSocketTransport::new();
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let mut poller = session.camera::<SonyFR7>().expect("second camera view");
    poller.set_submission_class(Some(SubmissionClass::Background));

    // Submitted first from a derived view: the class is accepted as view
    // configuration, but cannot bypass first-write rejection.
    let raised = poller
        .with_submission_class(SubmissionClass::User)
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .expect_err("a raised blocking operation cannot queue");
    assert!(matches!(raised, Error::TransportBusy));
    assert_eq!(written(&writes).len(), 2);
    assert_eq!(
        poller.submission_class(),
        Some(SubmissionClass::Background),
        "a derived view does not change the source handle default",
    );

    first.applied().expect("first socket freed");
    second.applied().expect("second socket freed");
    assert_eq!(written(&writes).len(), 2);
    session.shutdown().expect("owner shutdown");
}

/// A handle default cannot bypass the blocking first-write boundary for an
/// urgent operation.
#[test]
fn a_handle_default_never_changes_the_first_write_boundary_for_an_urgent_stop() {
    let (transport, writes) = TwoSocketTransport::new();
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let mut poller = session.camera::<SonyFR7>().expect("second camera view");
    poller.set_submission_class(Some(SubmissionClass::Background));
    let stop = poller
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("an urgent blocking operation cannot queue");
    assert!(matches!(stop, Error::TransportBusy));
    assert_eq!(written(&writes).len(), 2);

    first.applied().expect("first socket freed");
    second.applied().expect("second socket freed");
    assert_eq!(written(&writes).len(), 2);
    session.shutdown().expect("owner shutdown");
}

/// A class-selected derived view cannot bypass the blocking first-write
/// invariant when both sockets are occupied.
#[test]
fn a_derived_view_class_cannot_bypass_the_first_write_boundary() {
    let (transport, writes) = TwoSocketTransport::new();
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let [first, second] = occupy_both_sockets(&camera, &writes);

    let stop = camera
        .with_submission_class(SubmissionClass::Background)
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("an urgent blocking stop cannot queue behind occupied sockets");
    assert!(matches!(stop, Error::TransportBusy));
    assert_eq!(written(&writes).len(), 2);

    first.applied().expect("first socket freed");
    second.applied().expect("second socket freed");
    assert_eq!(written(&writes).len(), 2);
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

/// A class-selected view's `execute` and `inquire` reach the same owner path
/// as their source-view twins.
#[test]
fn classified_execute_reaches_the_wire() {
    let (transport, writes) = TwoSocketTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let mut camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .with_submission_class(SubmissionClass::Background)
        .execute(&FocusModeCommand::Manual)
        .expect("background plain command still executes");
    camera.set_submission_class(Some(SubmissionClass::User));
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
