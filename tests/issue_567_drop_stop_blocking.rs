//! Dropping an unobserved blocking movement handle stops the camera (#567).
//!
//! Blocking admission returns after the operation's first write, and the
//! caller thread is the only thing that can put further bytes on the wire, so
//! the drop STOP is written by the dropping thread and is visible in the
//! transcript immediately.

#![cfg(feature = "blocking")]

use std::{
    collections::VecDeque,
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::{PanTiltDirection, PresetNumber},
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    request::builtin::ZoomStop,
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    types::{PanSpeed, TiltSpeed},
    Error,
};

const PAN_TILT_DRIVE_UP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x01, 0xff];
const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];
const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_STOP_FRAME: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const PAN_TILT_HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xff];
const FOCUS_AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x02, 0xff];
const ACK_AND_COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff, 0x90, 0x51, 0xff];

#[derive(Debug)]
struct DropStopTransport {
    config: TransportConfig,
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl DropStopTransport {
    fn new() -> (Self, DropStopProbe) {
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses: Arc::clone(&responses),
                writes: Arc::clone(&writes),
            },
            DropStopProbe { writes },
        )
    }
}

#[derive(Clone, Debug)]
struct DropStopProbe {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl DropStopProbe {
    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl HasTransportConfig for DropStopTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for DropStopTransport {
    fn send_with_kind(
        &mut self,
        bytes: &[u8],
        _kind: grafton_visca::command::CommandKind,
    ) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        // Every write is answered, so any caller-thread pump makes progress
        // and no operation is left waiting on a socket that never frees.
        self.responses
            .lock()
            .expect("responses lock")
            .push_back(ACK_AND_COMPLETE_SOCKET_ONE.to_vec());
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
        let bytes = self
            .responses
            .lock()
            .expect("responses lock")
            .pop_front()
            .ok_or(Error::Timeout)?;
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn g2_config() -> SessionConfig {
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile");
    SessionConfig::new(profile)
}

#[test]
fn dropping_an_unobserved_pan_tilt_drive_stops_the_axis() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let operation = camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(12).expect("pan speed"),
            TiltSpeed::new(10).expect("tilt speed"),
        )
        .expect("pan/tilt drive");
    assert_eq!(probe.writes(), vec![PAN_TILT_DRIVE_UP.to_vec()]);

    drop(operation);

    assert_eq!(
        probe.writes(),
        vec![PAN_TILT_DRIVE_UP.to_vec(), PAN_TILT_STOP.to_vec()],
        "dropping an unobserved pan/tilt drive must stop the axis"
    );

    session.shutdown().expect("owner shutdown");
}

#[test]
fn dropping_an_unobserved_zoom_drive_stops_the_axis() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    drop(camera.zoom().tele().expect("zoom tele"));

    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), ZOOM_STOP_FRAME.to_vec()],
        "dropping an unobserved zoom drive must stop the axis"
    );

    session.shutdown().expect("owner shutdown");
}

#[test]
fn detach_suppresses_the_drop_stop() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(12).expect("pan speed"),
            TiltSpeed::new(10).expect("tilt speed"),
        )
        .expect("pan/tilt drive")
        .detach();

    assert_eq!(
        probe.writes(),
        vec![PAN_TILT_DRIVE_UP.to_vec()],
        "detach must not emit a STOP"
    );

    session.shutdown().expect("owner shutdown");
}

#[test]
fn observed_operations_add_no_stop() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    camera
        .zoom()
        .tele()
        .expect("zoom tele")
        .applied()
        .expect("zoom tele applied");
    camera
        .pan_tilt()
        .home()
        .expect("pan/tilt home")
        .settled()
        .expect("pan/tilt home settled");

    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), PAN_TILT_HOME.to_vec()],
        "observed operations must not emit a STOP"
    );

    session.shutdown().expect("owner shutdown");
}

#[test]
fn a_cancel_attempt_consumes_the_handle_without_a_stop() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // G2 has no VISCA socket-cancel support, so this is the rejected-cancel
    // path.  Either way the caller made an explicit terminal decision and the
    // handle is consumed, so no drop STOP follows it.
    let error = camera
        .zoom()
        .tele()
        .expect("zoom tele")
        .cancel()
        .expect_err("G2 sent cancellation must be rejected by profile policy");
    assert!(matches!(error, Error::NotSupported));

    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec()],
        "an explicitly cancelled operation must not emit a STOP"
    );

    session.shutdown().expect("owner shutdown");
}

#[test]
fn stops_and_plain_commands_are_unaffected() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // Dropping an unobserved STOP must not enqueue another STOP.
    drop(
        camera
            .submit::<AppliedOnly, _>(&ZoomStop)
            .expect("zoom stop"),
    );
    // A plain configuration command has no operation handle at all.
    camera.focus().auto().expect("focus auto");

    assert_eq!(
        probe.writes(),
        vec![ZOOM_STOP_FRAME.to_vec(), FOCUS_AUTO.to_vec()],
        "an abandoned STOP and plain commands add nothing"
    );

    session.shutdown().expect("owner shutdown");
}

#[test]
fn dropping_a_preset_recall_stops_every_affected_axis() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    drop(
        camera
            .presets()
            .recall(PresetNumber::new(3).expect("preset number"))
            .expect("preset recall"),
    );

    let writes = probe.writes();
    let stops = &writes[1..];
    assert!(
        stops.contains(&PAN_TILT_STOP.to_vec()),
        "preset recall drives pan/tilt; saw {writes:?}"
    );
    assert!(
        stops.contains(&ZOOM_STOP_FRAME.to_vec()),
        "preset recall drives zoom; saw {writes:?}"
    );

    session.shutdown().expect("owner shutdown");
}

#[test]
fn a_panic_unwinding_past_a_live_handle_stops_the_camera() {
    let (transport, probe) = DropStopTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let operation = camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(12).expect("pan speed"),
            TiltSpeed::new(10).expect("tilt speed"),
        )
        .expect("pan/tilt drive");
    assert_eq!(probe.writes(), vec![PAN_TILT_DRIVE_UP.to_vec()]);

    let unwound = std::panic::catch_unwind(AssertUnwindSafe(move || {
        let _live_movement = operation;
        panic!("issue 567: simulated failure while the camera is moving");
    }));
    assert!(unwound.is_err(), "the test panic must have unwound");

    assert_eq!(
        probe.writes(),
        vec![PAN_TILT_DRIVE_UP.to_vec(), PAN_TILT_STOP.to_vec()],
        "a panic must not leave hardware moving"
    );

    session.shutdown().expect("owner shutdown");
}
