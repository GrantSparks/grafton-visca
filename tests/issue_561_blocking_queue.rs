//! Issue #561: blocking submissions queue behind busy sockets.
//!
//! A blocking caller may hold more un-awaited operation handles than the
//! target has command sockets. The excess work is queued by the owner and
//! drained as sockets free; only genuine admission capacity
//! (`max_pending_queue_depth`) rejects a submission.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::builtin::{FocusStop, PanTiltStop, ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    types::{PanSpeed, TiltSpeed},
    Error,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xff];
const PAN_TILT_STOP_PREFIX: &[u8] = &[0x81, 0x01, 0x06, 0x01];

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
    fn new(config: TransportConfig) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config,
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

fn pan_tilt_stop() -> PanTiltStop {
    PanTiltStop::new(
        PanSpeed::new(1).expect("valid pan speed"),
        TiltSpeed::new(1).expect("valid tilt speed"),
    )
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

/// Three concurrently held operation handles over a two-socket target: none
/// fails with `TransportBusy`, the third is written once a socket frees, and
/// every operation completes in submit order.
#[test]
fn three_concurrent_blocking_submits_queue_and_complete_in_order() {
    let (transport, writes) = TwoSocketTransport::new(TransportConfig::default());
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");
    let third = camera
        .submit::<AppliedOnly, _>(&pan_tilt_stop())
        .expect("issue #561: the third submission queues instead of failing busy");

    let before = written(&writes);
    assert_eq!(
        before.len(),
        2,
        "only two command sockets exist, so the third request stays queued"
    );
    assert_eq!(before[0], ZOOM_STOP);
    assert_eq!(before[1], FOCUS_STOP);

    first.applied().expect("first operation applied");
    let drained = written(&writes);
    assert_eq!(
        drained.len(),
        3,
        "the queued request is written once a socket frees"
    );
    assert!(drained[2].starts_with(PAN_TILT_STOP_PREFIX));

    second.applied().expect("second operation applied");
    third.applied().expect("third operation applied");

    assert_eq!(written(&writes).len(), 3, "nothing is written twice");
    session.shutdown().expect("owner shutdown");
}

/// Queue depth is bounded by admission capacity, not by socket availability:
/// once `max_pending_queue_depth` is genuinely exhausted the submission is
/// still rejected, and it is rejected before any wire write.
#[test]
fn blocking_submits_beyond_the_queue_depth_are_still_rejected() {
    let config = TransportConfig {
        max_pending_queue_depth: NonZeroUsize::new(3).expect("non-zero queue depth"),
        ..TransportConfig::default()
    };
    let (transport, writes) = TwoSocketTransport::new(config);
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");
    let third = camera
        .submit::<AppliedOnly, _>(&pan_tilt_stop())
        .expect("the third submission fills the bounded queue");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("the fourth submission exceeds the configured queue depth");
    assert!(
        matches!(error, Error::RuntimeQueueFull { capacity: 3 }),
        "expected a queue-depth rejection, got {error:?}"
    );
    assert_eq!(
        written(&writes).len(),
        2,
        "a rejected submission never writes"
    );

    first.applied().expect("first operation applied");
    second.applied().expect("second operation applied");
    third.applied().expect("third operation applied");

    // With the queue drained the same submission is admitted again.
    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("capacity is released by terminal operations")
        .applied()
        .expect("re-admitted operation applied");

    session.shutdown().expect("owner shutdown");
}
