//! Issue #629: a blocking pump that ends the session must report the session's
//! boundary error, never the raw transport cause.
//!
//! The escape sites were the observation paths that pump without a receipt to
//! consult — settlement polling above all. A raw `Error::Io` classifies as
//! survivable (`requires_new_session() == false`), so an auto-reconnect loop
//! keyed on that predicate kept using a session the owner had already closed.

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
    request::builtin::{ZoomStop, ZoomTarget},
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    types::ZoomPosition,
    CameraId, Error, OperationalTuning,
};

use profile_fixtures::DirectZoomOnlyTypedSupport;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
/// `90 50 0p 0q 0r 0s FF`: the zoom position the settlement baseline reads.
const ZOOM_POSITION_REPLY: &[u8] = &[0x90, 0x50, 0x00, 0x01, 0x00, 0x00, 0xff];

/// A stream camera whose reads are scripted per write.
#[derive(Debug)]
struct ProbeTransport {
    config: TransportConfig,
    /// What the camera queues for reading after the n-th write.
    script: VecDeque<Vec<Result<Vec<u8>, Error>>>,
    reads: VecDeque<Result<Vec<u8>, Error>>,
    writes: Arc<Mutex<usize>>,
    sends: usize,
    fail_on_send: Option<(usize, Error)>,
    wait_when_idle: bool,
}

impl ProbeTransport {
    fn new(script: Vec<Vec<Result<Vec<u8>, Error>>>) -> Self {
        Self {
            config: TransportConfig::default(),
            script: script.into(),
            reads: VecDeque::new(),
            writes: Arc::new(Mutex::new(0)),
            sends: 0,
            fail_on_send: None,
            wait_when_idle: false,
        }
    }

    fn with_serial_addressing(mut self) -> Self {
        self.config.addressing = AddressingMode::Serial;
        self
    }

    fn with_stream_write_failure(mut self, send: usize, error: Error) -> Self {
        self.fail_on_send = Some((send, error));
        self
    }

    fn wait_when_idle(mut self) -> Self {
        self.wait_when_idle = true;
        self
    }
}

impl HasTransportConfig for ProbeTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ProbeTransport {
    fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.sends = self.sends.saturating_add(1);
        *self.writes.lock().expect("write count lock") += 1;
        if self
            .fail_on_send
            .as_ref()
            .is_some_and(|(send, _)| *send == self.sends)
        {
            return Err(self
                .fail_on_send
                .take()
                .expect("matching configured send failure")
                .1);
        }
        for read in self.script.pop_front().unwrap_or_default() {
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
        timeout: Duration,
    ) -> Result<usize, Error> {
        let Some(next) = self.reads.pop_front() else {
            if self.wait_when_idle && !timeout.is_zero() {
                std::thread::sleep(timeout);
            }
            return Err(Error::Timeout);
        };
        let bytes = next?;
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }
}

/// The review's probe: the operation applies, the settlement baseline is read,
/// and the connection then dies inside the interval pump between two position
/// samples. The caller must be told the session is gone.
#[test]
fn connection_reset_during_settlement_polling_classifies_as_session_death() {
    let transport = ProbeTransport::new(vec![
        // The zoom target is acknowledged and applied.
        vec![
            Ok(ACK_SOCKET_ONE.to_vec()),
            Ok(COMPLETE_SOCKET_ONE.to_vec()),
        ],
        // The settlement baseline inquiry is answered, and the peer then
        // resets the connection before the next sample can be taken.
        vec![
            Ok(ZOOM_POSITION_REPLY.to_vec()),
            Err(Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionReset,
            )))),
        ],
    ]);
    let writes = Arc::clone(&transport.writes);
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()
            .expect("settlement-polling runtime profile"),
    );
    let session = Session::open(transport, config).expect("owner session");
    let camera = session
        .camera::<DirectZoomOnlyTypedSupport>()
        .expect("camera view");

    let error = camera
        .submit::<Targeted, _>(&ZoomTarget::new(
            ZoomPosition::new(0x0100).expect("zoom position"),
        ))
        .expect("submission")
        .settled()
        .expect_err("a reset connection cannot settle the operation");

    let Error::ConnectionClosed { reason } = &error else {
        panic!("settlement must report the session close, got {error:?}");
    };
    let reason = reason.as_ref().expect("the read fault names the close");
    assert!(
        reason.contains("connection reset"),
        "the transport cause must survive in the close reason: {reason}"
    );
    assert!(
        error.requires_new_session(),
        "a settlement wait interrupted by a dead transport must ask for a new session"
    );
    assert_eq!(
        *writes.lock().expect("write count lock"),
        2,
        "one operation write and one settlement baseline inquiry"
    );
}

/// A targeted operation can have already applied when another target retries.
/// Its retry write is still a stream boundary: the public settlement wait must
/// surface `StreamPoisoned`, not consume the remainder of its own timeout.
#[test]
fn stream_retry_write_failure_during_settlement_is_not_reported_as_timeout() {
    let profile = ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()
        .expect("settlement-polling runtime profile");
    let config = SessionConfig::new(profile.clone())
        .with_target(CameraId::CAMERA_2, profile)
        .expect("second serial target")
        .with_tuning(OperationalTuning::new().retry_timing(
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_secs(1),
        ))
        .expect("short bounded retry timing");
    let transport = ProbeTransport::new(vec![
        vec![
            Ok(ACK_SOCKET_ONE.to_vec()),
            Ok(COMPLETE_SOCKET_ONE.to_vec()),
        ],
        vec![Ok(vec![0xa0, 0x60, 0x03, 0xff])],
        vec![Ok(ZOOM_POSITION_REPLY.to_vec())],
    ])
    .with_serial_addressing()
    .wait_when_idle()
    .with_stream_write_failure(
        4,
        Error::TransportError("public settlement retry write failed".into()),
    );
    let session = Session::open(transport, config).expect("owner session");
    let first_camera = session
        .camera_for::<DirectZoomOnlyTypedSupport>(CameraId::CAMERA_1)
        .expect("camera one view");
    let second_camera = session
        .camera_for::<DirectZoomOnlyTypedSupport>(CameraId::CAMERA_2)
        .expect("camera two view");

    let current = first_camera
        .submit::<Targeted, _>(&ZoomTarget::new(
            ZoomPosition::new(0x0100).expect("zoom position"),
        ))
        .expect("targeted operation write");
    let peer = second_camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("peer operation write");

    let error = current
        .settled()
        .expect_err("a stream retry write must end the settlement");
    let Error::StreamPoisoned { reason } = &error else {
        panic!("settlement must report stream poison, got {error:?}");
    };
    assert!(reason.contains("public settlement retry write failed"));
    assert!(error.requires_new_session());
    assert!(matches!(peer.applied(), Err(Error::StreamPoisoned { .. })));
}
