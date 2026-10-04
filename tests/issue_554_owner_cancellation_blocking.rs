//! Blocking owner cancellation acceptance for the built-in PTZOptics G2 profile.
//!
//! Admission returns before the native worker necessarily writes the command.
//! The transport probe confirms the first send before testing transmitted
//! cancellation; terminal progression is observed through the same operation
//! handle while the worker owns all protocol progress.

#![cfg(feature = "blocking")]

use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    request::builtin::FocusStop,
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error,
};

const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xff];
const ACK_AND_COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff, 0x90, 0x51, 0xff];

#[derive(Debug)]
struct CancellationTransport {
    config: TransportConfig,
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    writes_ready: Arc<Condvar>,
    send_count: usize,
    auto_complete_after_first: bool,
}

impl CancellationTransport {
    fn new(auto_complete_after_first: bool) -> (Self, CancellationProbe) {
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        let writes = Arc::new(Mutex::new(Vec::new()));
        let writes_ready = Arc::new(Condvar::new());
        (
            Self {
                config: TransportConfig::default(),
                responses: Arc::clone(&responses),
                writes: Arc::clone(&writes),
                writes_ready: Arc::clone(&writes_ready),
                send_count: 0,
                auto_complete_after_first,
            },
            CancellationProbe {
                responses,
                writes,
                writes_ready,
            },
        )
    }
}

#[derive(Clone, Debug)]
struct CancellationProbe {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    writes_ready: Arc<Condvar>,
}

impl CancellationProbe {
    fn push(&self, bytes: Vec<u8>) {
        self.responses
            .lock()
            .expect("responses lock")
            .push_back(bytes);
    }

    fn wait_for_first_write(&self) {
        let (writes, _) = self
            .writes_ready
            .wait_timeout_while(
                self.writes.lock().expect("writes lock"),
                Duration::from_secs(5),
                |writes| writes.is_empty(),
            )
            .expect("write notification lock");
        assert!(
            !writes.is_empty(),
            "owner must transmit the admitted operation"
        );
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl HasTransportConfig for CancellationTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for CancellationTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: grafton_visca::command::CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.send_count = self.send_count.saturating_add(1);
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        self.writes_ready.notify_all();
        if self.auto_complete_after_first && self.send_count > 1 {
            self.responses
                .lock()
                .expect("responses lock")
                .push_back(ACK_AND_COMPLETE_SOCKET_ONE.to_vec());
        }
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
fn blocking_g2_transmitted_cancel_is_not_supported_without_cancel_frame() {
    let (transport, probe) = CancellationTransport::new(true);
    let session = Session::open(transport, g2_config()).expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let mut original = camera.zoom().stop().expect("transmitted operation");
    probe.wait_for_first_write();
    assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);

    let refused = original
        .cancel()
        .expect_err("G2 sent cancellation must be rejected by profile policy");
    assert!(matches!(refused, Error::NotSupported));
    // A refusal leaves the handle observing the original operation (#777).
    assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);

    // The original frames are delivered before the next operation's frames.
    // The next applied wait observes the same worker-owned protocol progression,
    // proving that a rejected cancel left the original operation live until
    // its protocol terminal state and emitted no cancellation transmission.
    probe.push(ACK_AND_COMPLETE_SOCKET_ONE.to_vec());
    original
        .applied()
        .expect("the handle still observes the original operation");
    let mut next = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("next operation");
    next.applied().expect("next operation applied");
    assert_eq!(
        probe.writes(),
        vec![ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]
    );

    session.shutdown().expect("owner shutdown");
}
