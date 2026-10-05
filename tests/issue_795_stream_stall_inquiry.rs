//! Regressions for #795, observed on a PTZOptics G2 bench (2026-10-04,
//! `5a3e81d1`): a raw-VISCA TCP (stream) session whose outbound bytes are
//! silently queued during a network fault and delivered together once it
//! lifts.
//!
//! On a stream nothing written is lost, so the camera eventually answers every
//! copy of a timed-out inquiry. Raw inquiry replies carry no identity, so a
//! late reply must never be bound to a later inquiry on the same camera. The
//! engine therefore writes a raw stream inquiry once, fails it at its reply
//! deadline, and keeps that camera's inquiry lane closed until the owed reply
//! has been discarded. If it has not arrived within the profile's ambiguity
//! window, only that camera's inquiries fail (promptly, unwritten) until it
//! does; commands, other cameras, and the session keep running. Datagram
//! behaviour and raw command handling are unchanged. The async-facade twin is
//! `issue_795_stream_stall_inquiry_async.rs`.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::{
    collections::VecDeque,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    camera::profiles::PtzOpticsG2,
    command::{CommandKind, FocusMode},
    profile::ProfileSpec,
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics,
        TransportConfig,
    },
    CameraId, Certainty, Error, FailureContext, FailureStage,
};

const POWER_INQUIRY: [u8; 4] = [0x09, 0x04, 0x00, 0xff];
const FOCUS_MODE_INQUIRY: [u8; 4] = [0x09, 0x04, 0x38, 0xff];
const PAN_TILT_STOP: [u8; 8] = [0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];

/// How long after the fault lifts the queued bytes are retransmitted when no
/// new write pushes them out first. Longer than the G2 reply skew (150 ms), so
/// stale replies arrive after the pre-fix skew hold had already expired.
const RETRANSMIT_AFTER: Duration = Duration::from_millis(300);

/// Camera truth for every address: power on (`02`), focus MANUAL (`03`). A
/// stale power reply decoded as focus mode reads as AUTO (`02`). A camera
/// listed in `mute_power` never answers the power inquiry.
fn camera_answer(request: &[u8], mute_power: &[u8]) -> Vec<Vec<u8>> {
    let Some((&address, body)) = request.split_first() else {
        return Vec::new();
    };
    let camera = address & 0x0f;
    let reply = 0x80 | (camera << 4);
    match body {
        b if b == POWER_INQUIRY && !mute_power.contains(&camera) => {
            vec![vec![reply, 0x50, 0x02, 0xff]]
        }
        b if b == FOCUS_MODE_INQUIRY => vec![vec![reply, 0x50, 0x03, 0xff]],
        b if b == PAN_TILT_STOP => vec![vec![reply, 0x41, 0xff], vec![reply, 0x51, 0xff]],
        _ => Vec::new(),
    }
}

fn addressed(camera: u8, body: &[u8]) -> Vec<u8> {
    let mut frame = vec![0x80 | camera];
    frame.extend_from_slice(body);
    frame
}

/// A raw-VISCA transport with an injectable silent outbound fault.
///
/// Stream: while `fault` is set every write "succeeds" into a kernel queue and
/// nothing reaches the camera. Once the fault lifts, the queue is delivered in
/// order either by the next write or by a retransmission timer
/// ([`RETRANSMIT_AFTER`]), as TCP retransmission does, and the camera answers
/// every queued copy. Datagram: writes during the fault are lost.
struct StallingTransport {
    config: TransportConfig,
    semantics: SendSemantics,
    fault: Arc<AtomicBool>,
    mute_power: Vec<u8>,
    queued: Vec<Vec<u8>>,
    clear_since: Option<Instant>,
    replies: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl fmt::Debug for StallingTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StallingTransport").finish_non_exhaustive()
    }
}

impl StallingTransport {
    fn deliver_queued(&mut self) {
        for request in std::mem::take(&mut self.queued) {
            self.replies
                .extend(camera_answer(&request, &self.mute_power));
        }
        self.clear_since = None;
    }
}

impl HasTransportConfig for StallingTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for StallingTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.writes.lock().unwrap().push(bytes.to_vec());
        let faulted = self.fault.load(Ordering::SeqCst);
        if faulted && self.semantics == SendSemantics::Datagram {
            return Ok(());
        }
        self.queued.push(bytes.to_vec());
        if !faulted {
            self.deliver_queued();
        }
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        destination: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        if !self.queued.is_empty() && !self.fault.load(Ordering::SeqCst) {
            let since = *self.clear_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= RETRANSMIT_AFTER {
                self.deliver_queued();
            }
        }
        match self.replies.pop_front() {
            Some(reply) => Ok(ReceiveOutcome::copy_message(&reply, destination)),
            None => {
                thread::sleep(timeout.min(Duration::from_millis(5)));
                Err(Error::io_timeout())
            }
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        self.semantics
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }
}

struct Bench {
    session: Session,
    fault: Arc<AtomicBool>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Bench {
    fn open(semantics: SendSemantics) -> Self {
        Self::open_with(semantics, &[CameraId::CAMERA_1], Vec::new())
    }

    fn open_with(semantics: SendSemantics, targets: &[CameraId], mute_power: Vec<u8>) -> Self {
        let fault = Arc::new(AtomicBool::new(false));
        let writes = Arc::new(Mutex::new(Vec::new()));
        let mut transport_config = TransportConfig::default();
        if targets.len() > 1 {
            // Several cameras share one byte stream only with serial-style
            // addressing (a daisy chain behind one TCP bridge).
            transport_config.addressing = AddressingMode::Serial;
        }
        let transport = StallingTransport {
            config: transport_config,
            semantics,
            fault: Arc::clone(&fault),
            mute_power,
            queued: Vec::new(),
            clear_since: None,
            replies: VecDeque::new(),
            writes: Arc::clone(&writes),
        };
        let profile = || ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2");
        let mut config = SessionConfig::new(profile());
        for target in targets.iter().skip(1) {
            config
                .register_target(*target, profile())
                .expect("second target");
        }
        let session = Session::open(transport, config).expect("session");
        Self {
            session,
            fault,
            writes,
        }
    }

    fn set_fault(&self, on: bool) {
        self.fault.store(on, Ordering::SeqCst);
    }

    fn count(&self, camera: u8, body: &[u8]) -> usize {
        let frame = addressed(camera, body);
        self.writes
            .lock()
            .unwrap()
            .iter()
            .filter(|w| **w == frame)
            .count()
    }
}

fn context_of<T>(result: &Result<T, Error>) -> Option<FailureContext> {
    result.as_ref().err().and_then(Error::failure_context)
}

fn conclusive_timeout<T>(result: &Result<T, Error>) -> bool {
    context_of(result)
        == Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively,
        ))
}

/// The latched-lane failure: nothing written, not retryable on this
/// session, and the session itself still live.
fn latched_inquiry_failure<T: fmt::Debug>(result: &Result<T, Error>) {
    let Err(error) = result else {
        panic!("expected InquiryCorrelationLost, got {result:?}");
    };
    assert!(
        matches!(error, Error::InquiryCorrelationLost { camera, .. } if *camera == CameraId::CAMERA_1),
        "{error:?}"
    );
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::NotAccepted
        ))
    );
    assert!(!error.is_retryable());
    assert!(!error.requires_new_session(), "{error}");
}

/// The #795 defect: after the stall lifts, the camera's late `90 50 02 FF`
/// power reply must not be bound to the next same-target inquiry (focus mode,
/// one-byte reply). The camera is in MANUAL focus; on 5a3e81d1 the session
/// silently reported AUTO.
#[test]
fn stale_stream_inquiry_reply_never_binds_a_later_inquiry() {
    let bench = Bench::open(SendSemantics::Stream);
    let camera = bench.session.camera::<PtzOpticsG2>().expect("camera");

    bench.set_fault(true);
    let power = camera.power().state();
    assert!(conclusive_timeout(&power), "{power:?}");
    bench.set_fault(false);

    let mode = camera.focus().mode();
    assert!(
        matches!(mode, Ok(FocusMode::Manual)),
        "a stale power reply must never be returned as the focus mode: {mode:?}"
    );
    // The owed reply was absorbed; the session keeps serving correct data.
    assert!(camera.power().state().expect("power after recovery"));
    assert!(matches!(camera.focus().mode(), Ok(FocusMode::Manual)));
    bench.session.shutdown().expect("shutdown");
}

/// A raw stream inquiry is written once and fails at its own reply deadline:
/// a resend cannot recover bytes a stream has not lost, it only multiplies
/// the late replies. On 5a3e81d1 it was written six times over ~7 s.
#[test]
fn stream_inquiry_reply_timeout_is_not_resent() {
    let bench = Bench::open(SendSemantics::Stream);
    let camera = bench.session.camera::<PtzOpticsG2>().expect("camera");
    assert!(camera.power().state().expect("pre-fault inquiry"));

    bench.set_fault(true);
    let started = Instant::now();
    let power = camera.power().state();
    let elapsed = started.elapsed();
    assert!(conclusive_timeout(&power), "{power:?}");
    assert_eq!(
        bench.count(1, &POWER_INQUIRY),
        1 + 1,
        "pre-fault write plus one"
    );
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    bench.set_fault(false);
    bench.session.shutdown().expect("shutdown");
}

/// A camera that never answers an inquiry (no fault at all): its owed reply
/// never arrives, so after the ambiguity window only that camera's inquiries
/// fail, promptly and unwritten. A STOP to the same camera and inquiries to
/// another camera on the same session still succeed; nothing is poisoned. On
/// 5a3e81d1 the power inquiry was resent for ~7 s and later inquiries were
/// sent into an uncorrelatable lane.
#[test]
fn unanswered_stream_inquiry_latches_only_that_cameras_inquiries() {
    let bench = Bench::open_with(
        SendSemantics::Stream,
        &[CameraId::CAMERA_1, CameraId::CAMERA_2],
        vec![1],
    );
    let first = bench
        .session
        .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
        .expect("camera 1");
    let second = bench
        .session
        .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
        .expect("camera 2");

    let power = first.power().state();
    assert!(conclusive_timeout(&power), "{power:?}");

    let started = Instant::now();
    let queued = first.focus().mode();
    assert!(started.elapsed() < Duration::from_secs(3));
    latched_inquiry_failure(&queued);

    let started = Instant::now();
    let rejected = first.focus().mode();
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "a latched lane fails promptly"
    );
    latched_inquiry_failure(&rejected);
    assert_eq!(bench.count(1, &FOCUS_MODE_INQUIRY), 0);
    assert_eq!(bench.count(1, &POWER_INQUIRY), 1);

    first
        .pan_tilt()
        .stop()
        .expect("STOP admitted")
        .applied()
        .expect("STOP to the latched camera still applies");
    assert!(second.power().state().expect("other camera unaffected"));
    assert!(matches!(second.focus().mode(), Ok(FocusMode::Manual)));
    bench
        .session
        .shutdown()
        .expect("session was never poisoned");
}

/// An owed reply that arrives only after the window (a long stall) still
/// settles: the latched lane reopens and later inquiries return correct data,
/// never the stale value. On 5a3e81d1 the stale `02` was read as AUTO.
#[test]
fn owed_reply_arriving_after_the_window_reopens_the_lane_with_correct_data() {
    let bench = Bench::open(SendSemantics::Stream);
    let camera = bench.session.camera::<PtzOpticsG2>().expect("camera");

    bench.set_fault(true);
    let power = camera.power().state();
    assert!(conclusive_timeout(&power), "{power:?}");
    let during = camera.focus().mode();
    latched_inquiry_failure(&during);

    bench.set_fault(false);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mode = loop {
        let mode = camera.focus().mode();
        assert!(
            !matches!(mode, Ok(FocusMode::Auto)),
            "a stale power reply was returned as the focus mode"
        );
        if mode.is_ok() || Instant::now() > deadline {
            break mode;
        }
        thread::sleep(Duration::from_millis(50));
    };
    assert!(matches!(mode, Ok(FocusMode::Manual)), "{mode:?}");
    assert!(camera.power().state().expect("power after recovery"));
    bench.session.shutdown().expect("shutdown");
}

/// Datagram behaviour is unchanged: a lost UDP inquiry is still resent under
/// the Inquiry retry policy (1 + 5 writes) and fails conclusively.
#[test]
fn datagram_inquiry_reply_timeout_still_retries() {
    let bench = Bench::open(SendSemantics::Datagram);
    let camera = bench.session.camera::<PtzOpticsG2>().expect("camera");

    bench.set_fault(true);
    let power = camera.power().state();
    assert!(conclusive_timeout(&power), "{power:?}");
    assert_eq!(bench.count(1, &POWER_INQUIRY), 6);
    bench.set_fault(false);
    assert!(camera.power().state().expect("datagram recovers"));
    bench.session.shutdown().expect("shutdown");
}

/// Raw commands under the same stall are unchanged: the STOP is written once,
/// fails `UnsequencedCommandUnconfirmed` at its ACK deadline, and is never
/// replayed.
#[test]
fn stalled_stream_command_is_unconfirmed_without_replay() {
    let bench = Bench::open(SendSemantics::Stream);
    let camera = bench.session.camera::<PtzOpticsG2>().expect("camera");

    bench.set_fault(true);
    let result = camera.pan_tilt().stop().expect("admitted").applied();
    assert!(
        matches!(result, Err(Error::UnsequencedCommandUnconfirmed)),
        "{result:?}"
    );
    assert_eq!(bench.count(1, &PAN_TILT_STOP), 1);
    bench.set_fault(false);
    bench.session.shutdown().expect("shutdown");
}
