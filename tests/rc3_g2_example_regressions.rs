//! Hardware-free regressions for two PTZOptics G2 observations that drove
//! `examples/cancellation.rs --g2-unsupported` and `examples/motion_safety.rs`.
//!
//! The examples are binaries and cannot be linked into a test, so these
//! characterization tests pin the library behaviour the example fixes rely on.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    camera::profiles::PtzOpticsG2,
    command::CommandKind,
    profile::ProfileSpec,
    transport::{
        BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
    },
    CancellationOutcome, Error, HaltOutcome,
};

const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xff];
const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];

type Script = Box<dyn FnMut(&[u8]) -> Vec<Vec<u8>> + Send>;

struct ScriptedG2 {
    config: TransportConfig,
    replies: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    script: Script,
}

impl fmt::Debug for ScriptedG2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptedG2").finish_non_exhaustive()
    }
}

impl ScriptedG2 {
    fn new(script: Script) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                replies: VecDeque::new(),
                writes: Arc::clone(&writes),
                script,
            },
            writes,
        )
    }
}

impl HasTransportConfig for ScriptedG2 {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ScriptedG2 {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.writes.lock().unwrap().push(bytes.to_vec());
        let replies = (self.script)(bytes);
        self.replies.extend(replies);
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        destination: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        match self.replies.pop_front() {
            Some(reply) => Ok(ReceiveOutcome::copy_message(&reply, destination)),
            None => {
                thread::sleep(timeout.min(Duration::from_millis(5)));
                Err(Error::io_timeout())
            }
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }
}

fn g2_session(script: Script) -> (Session, Arc<Mutex<Vec<Vec<u8>>>>) {
    let (transport, writes) = ScriptedG2::new(script);
    let session = Session::open(
        transport,
        SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile")),
    )
    .expect("session");
    (session, writes)
}

const FOCUS_STOP_MANUAL_REPLY: [[u8; 3]; 2] = [[0x90, 0x42, 0xff], [0x90, 0x52, 0xff]];

fn halt_script(focus: Vec<Vec<u8>>) -> Script {
    Box::new(move |bytes: &[u8]| match bytes {
        b if b == PAN_TILT_STOP => vec![vec![0x90, 0x41, 0xff], vec![0x90, 0x51, 0xff]],
        b if b == ZOOM_STOP => vec![vec![0x90, 0x42, 0xff], vec![0x90, 0x52, 0xff]],
        b if b == FOCUS_STOP => focus.clone(),
        _ => Vec::new(),
    })
}

fn all_writes() -> Vec<Vec<u8>> {
    vec![
        PAN_TILT_STOP.to_vec(),
        ZOOM_STOP.to_vec(),
        FOCUS_STOP.to_vec(),
    ]
}

/// Hardware observation: a continuous zoom drive (`81 01 04 07 02 FF`) on a
/// real PTZOptics G2 gets ACK and completion together about 50 ms after the
/// write while the zoom keeps moving (0x0EFC to 0x344B over 3 s). `cancel()`
/// then returns `Ok(Completed)` and writes nothing, so the example must send
/// its zoom STOP unconditionally rather than only after a refused cancel.
#[test]
fn g2_cancel_after_immediate_completion_is_completed_and_sends_nothing() {
    let (session, writes) = g2_session(Box::new(|bytes: &[u8]| {
        if bytes == ZOOM_TELE {
            vec![vec![0x90, 0x41, 0xff], vec![0x90, 0x51, 0xff]]
        } else {
            Vec::new()
        }
    }));
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    let mut operation = camera.zoom().tele().expect("tele admitted");
    thread::sleep(Duration::from_millis(100));
    let outcome = operation.cancel();
    assert!(
        matches!(outcome, Ok(CancellationOutcome::Completed)),
        "{outcome:?}"
    );
    assert_eq!(writes.lock().unwrap().as_slice(), &[ZOOM_TELE.to_vec()]);
    session.shutdown().expect("shutdown");
}

/// Hardware observation (rc.3 motion_safety, PTZOptics G2 cam4 in auto focus,
/// the factory default): focus STOP is rejected with `90 61 41 FF` (command not
/// executable on a free socket) while pan/tilt and zoom STOP succeed. The
/// report is pan/tilt Applied, zoom Applied, focus `Failed(_)` (rc.3 reports
/// `UnsequencedCommandUnconfirmed`; the exact classification is deliberately
/// not pinned, so a later conclusive not-executable mapping does not break
/// this); `into_result()` collapses it to an error, which is why the example
/// must inspect each axis instead.
#[test]
fn g2_auto_focus_halt_report_keeps_pan_tilt_and_zoom_applied() {
    let (session, writes) = g2_session(halt_script(vec![vec![0x90, 0x61, 0x41, 0xff]]));
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    let report = camera.motion().stop_all_motion().expect("halt accepted");

    assert!(
        matches!(report.pan_tilt, HaltOutcome::Applied),
        "{report:?}"
    );
    assert!(matches!(report.zoom, HaltOutcome::Applied), "{report:?}");
    assert!(matches!(report.focus, HaltOutcome::Failed(_)), "{report:?}");
    assert!(report.into_result().is_err());
    assert_eq!(writes.lock().unwrap().as_slice(), all_writes());
    session.shutdown().expect("shutdown");
}

/// Hardware observation: with manual focus the same PTZOptics G2 accepts the
/// focus STOP (`90 42 FF`, `90 52 FF`), so all three axes are Applied and the
/// example exits 0.
#[test]
fn g2_manual_focus_halt_report_is_all_applied() {
    let (session, writes) = g2_session(halt_script(
        FOCUS_STOP_MANUAL_REPLY.iter().map(|r| r.to_vec()).collect(),
    ));
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    let report = camera.motion().stop_all_motion().expect("halt accepted");

    assert!(
        matches!(report.pan_tilt, HaltOutcome::Applied),
        "{report:?}"
    );
    assert!(matches!(report.zoom, HaltOutcome::Applied), "{report:?}");
    assert!(matches!(report.focus, HaltOutcome::Applied), "{report:?}");
    assert!(report.into_result().is_ok());
    assert_eq!(writes.lock().unwrap().as_slice(), all_writes());
    session.shutdown().expect("shutdown");
}
