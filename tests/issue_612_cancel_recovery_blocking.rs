//! A refused cancellation must never strand the blocking caller (#612, #777).
//!
//! The blocking twin of `issue_612_cancel_recovery.rs`. 1.x's blocking
//! `BlockingInFlight::cancel` consumed the handle on the `NotSupported` path
//! and discarded the retained result with it. In 2.0 `cancel` borrows the
//! handle on both facades, so a refusal is a plain error and the same handle
//! keeps observing the original operation.

#![cfg(feature = "blocking")]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error,
};

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const ACK_AND_COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff, 0x90, 0x51, 0xff];

/// Replays exactly the frames a test queues and records every write, so the
/// wire transcript is the assertion surface.
#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ScriptedTransport {
    fn new() -> (Self, TransportProbe) {
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses: Arc::clone(&responses),
                writes: Arc::clone(&writes),
            },
            TransportProbe { responses, writes },
        )
    }
}

#[derive(Clone, Debug)]
struct TransportProbe {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl TransportProbe {
    fn push(&self, bytes: &[u8]) {
        self.responses
            .lock()
            .expect("responses lock")
            .push_back(bytes.to_vec());
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ScriptedTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: grafton_visca::command::CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        Ok(())
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
            .ok_or(Error::io_timeout())?;
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
fn blocking_refused_cancellation_leaves_the_handle_observing_and_stop_available() {
    let (transport, probe) = ScriptedTransport::new();
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // A continuous zoom: the axis keeps moving until something stops it.
    let mut moving = camera.zoom().tele().expect("continuous zoom admitted");
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);

    // The G2 has no socket-cancel, so the owner refuses. `cancel` borrows the
    // handle, so the refusal leaves it in the caller's hands.
    let refused = moving
        .cancel()
        .expect_err("G2 sent cancellation must be rejected by profile policy");
    assert!(matches!(refused, Error::NotSupported));

    // A refusal installs no intent, so a retry is refused the same way.
    let refused = moving
        .cancel()
        .expect_err("the retry is refused on the same profile grounds");
    assert!(matches!(refused, Error::NotSupported));

    // No cancellation frame was ever written.
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);

    // The handle still reports the original operation's own terminal state,
    // which is exactly the state 1.x's blocking facade discarded on this path,
    // and a cancel after it concluded answers from that state.
    probe.push(ACK_AND_COMPLETE_SOCKET_ONE);
    moving
        .applied()
        .expect("the handle still observes the original operation");
    assert!(matches!(
        moving.cancel(),
        Ok(grafton_visca::CancellationOutcome::Completed)
    ));

    // The documented recourse for a profile without socket-cancel: an
    // explicit typed STOP, which still reaches the wire.
    let mut stop = camera.zoom().stop().expect("typed stop admitted");
    probe.push(ACK_AND_COMPLETE_SOCKET_ONE);
    stop.applied().expect("the stop applies");
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()],
        "the STOP that ends physical movement must reach the wire"
    );

    session.shutdown().expect("owner shutdown");
}
