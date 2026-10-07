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
//! behaviour and raw command handling are unchanged.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime. The Tokio cases run on paused (virtual) time: the G2
//! deadlines (inquiry reply, ambiguity window, the datagram retry schedule) and the bench's retransmission timer are all runtime timers
//! there, so they lapse at once and in exactly the order real time would
//! give them. The blocking and smol cases wait them out on the real clock.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;

use std::{
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    command::FocusMode,
    profile::{ProfileSpec, ProfileTiming},
    transport::{AddressingMode, SendSemantics, TransportConfig},
    CameraId, Certainty, Error, FailureContext, FailureStage, SessionConfig,
};

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use fake_camera::AsyncWire;
#[cfg(feature = "blocking")]
use fake_camera::BlockingWire;
use fake_camera::FakeCamera;

const POWER_INQUIRY: [u8; 4] = [0x09, 0x04, 0x00, 0xff];
const FOCUS_MODE_INQUIRY: [u8; 4] = [0x09, 0x04, 0x38, 0xff];
const PAN_TILT_STOP: [u8; 8] = [0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];

/// How long after the fault lifts the queued bytes are retransmitted when no
/// new write pushes them out first. Longer than the G2 reply skew (150 ms), so
/// stale replies arrive after the reply-skew hold has already expired.
const RETRANSMIT_AFTER: Duration = Duration::from_millis(300);

/// The G2 profile's timing, read from the profile itself.
fn g2_timing() -> ProfileTiming {
    ProfileSpec::from_compile_time::<PtzOpticsG2>()
        .expect("G2")
        .timing()
}

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

/// The silent outbound fault on a raw-VISCA connection.
///
/// Stream: while `fault` is set every write "succeeds" into a kernel queue and
/// nothing reaches the camera. Once the fault lifts, the queue is delivered in
/// order either by the next write or by a retransmission timer
/// ([`RETRANSMIT_AFTER`]), as TCP retransmission does, and the camera answers
/// every queued copy. Datagram: writes during the fault are lost.
struct Path {
    fault: bool,
    mute_power: Vec<u8>,
    queued: Vec<Vec<u8>>,
}

/// A scripted camera behind a [`Path`], and what a session over it needs.
struct Bench {
    camera: FakeCamera,
    path: Arc<Mutex<Path>>,
    semantics: SendSemantics,
    targets: Vec<CameraId>,
}

impl Bench {
    fn new(semantics: SendSemantics) -> Self {
        Self::with_targets(semantics, vec![CameraId::CAMERA_1], Vec::new())
    }

    fn with_targets(semantics: SendSemantics, targets: Vec<CameraId>, mute_power: Vec<u8>) -> Self {
        let path = Arc::new(Mutex::new(Path {
            fault: false,
            mute_power,
            queued: Vec::new(),
        }));
        let shared = Arc::clone(&path);
        let camera = FakeCamera::new(move |bytes, answer| {
            let mut path = shared.lock().unwrap();
            let faulted = path.fault;
            if faulted && semantics == SendSemantics::Datagram {
                return;
            }
            path.queued.push(bytes.to_vec());
            if !faulted {
                for request in std::mem::take(&mut path.queued) {
                    for reply in camera_answer(&request, &path.mute_power) {
                        answer.reply(reply);
                    }
                }
            }
        });
        Self {
            camera,
            path,
            semantics,
            targets,
        }
    }

    fn transport_config(&self) -> TransportConfig {
        let mut config = TransportConfig::default();
        if self.targets.len() > 1 {
            // Several cameras share one byte stream only with serial-style
            // addressing (a daisy chain behind one TCP bridge).
            config.addressing = AddressingMode::Serial;
        }
        config
    }

    fn addressing(&self) -> AddressingMode {
        self.transport_config().addressing
    }

    /// The blocking wire, in the shape `open!` asks its camera for.
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> BlockingWire {
        self.camera
            .blocking_wire()
            .with_config(self.transport_config())
            .with_semantics(self.semantics)
            .with_addressing(self.addressing())
    }

    /// The async wire, in the shape `open!` asks its camera for.
    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> AsyncWire {
        self.camera
            .async_wire()
            .with_config(self.transport_config())
            .with_semantics(self.semantics)
            .with_addressing(self.addressing())
    }

    fn config(&self) -> SessionConfig {
        let profile = || ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2");
        let mut config = SessionConfig::new(profile());
        for target in self.targets.iter().skip(1) {
            config
                .register_target(*target, profile())
                .expect("second target");
        }
        config
    }

    /// Starts the fault.
    fn start_fault(&self) {
        self.path.lock().unwrap().fault = true;
    }

    /// Ends the fault and returns its retransmission: run
    /// [`RETRANSMIT_AFTER`] later (`after!`), it delivers the queued bytes if
    /// no write has delivered them by then, and the camera answers every
    /// copy. The caller arms it on the facade's own clock, so a paused-time
    /// case fires it at an exact virtual instant instead of racing a real
    /// sleep against auto-advanced time.
    fn lift_fault(&self) -> impl FnOnce() + Send + 'static {
        self.path.lock().unwrap().fault = false;
        let path = Arc::clone(&self.path);
        let camera = self.camera.clone();
        move || {
            let mut path = path.lock().unwrap();
            if !path.fault {
                for request in std::mem::take(&mut path.queued) {
                    for reply in camera_answer(&request, &path.mute_power) {
                        camera.push(reply);
                    }
                }
            }
        }
    }

    fn count(&self, camera: u8, body: &[u8]) -> usize {
        let frame = addressed(camera, body);
        self.camera.writes().iter().filter(|w| **w == frame).count()
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

facade_matrix! {
    paused:

    /// The #795 defect: after the stall lifts, the camera's late `90 50 02 FF`
    /// power reply must not be bound to the next same-target inquiry (focus mode,
    /// one-byte reply). The camera is in MANUAL focus; on 5a3e81d1 the session
    /// silently reported AUTO.
    fn stale_stream_inquiry_reply_never_binds_a_later_inquiry() {
        let bench = Bench::new(SendSemantics::Stream);
        let session = open!(bench, bench.config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        bench.start_fault();
        let power = wait!(camera.power().state());
        assert!(conclusive_timeout(&power), "{power:?}");
        after!(RETRANSMIT_AFTER, bench.lift_fault());

        let mode = wait!(camera.focus().mode());
        assert!(
            matches!(mode, Ok(FocusMode::Manual)),
            "a stale power reply must never be returned as the focus mode: {mode:?}"
        );
        // The owed reply was absorbed; the session keeps serving correct data.
        assert!(wait!(camera.power().state()).expect("power after recovery"));
        assert!(matches!(wait!(camera.focus().mode()), Ok(FocusMode::Manual)));
        session.shutdown().expect("shutdown");
    }

    /// A raw stream inquiry is written once and fails at its own reply deadline:
    /// a resend cannot recover bytes a stream has not lost, it only multiplies
    /// the late replies. On 5a3e81d1 it was written six times over ~7 s.
    fn stream_inquiry_reply_timeout_is_not_resent() {
        let bench = Bench::new(SendSemantics::Stream);
        let session = open!(bench, bench.config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        assert!(wait!(camera.power().state()).expect("pre-fault inquiry"));

        bench.start_fault();
        let started = now!();
        let power = wait!(camera.power().state());
        let elapsed = now!().duration_since(started);
        assert!(conclusive_timeout(&power), "{power:?}");
        assert_eq!(
            bench.count(1, &POWER_INQUIRY),
            1 + 1,
            "pre-fault write plus one"
        );
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
        if virtual_clock!() {
            assert_eq!(
                elapsed,
                g2_timing().minimum_inquiry_spacing() + g2_timing().inquiry_timeout(),
                "paced behind the pre-fault inquiry, then one write failed at its own \
                 reply deadline"
            );
        }
        after!(RETRANSMIT_AFTER, bench.lift_fault());
        session.shutdown().expect("shutdown");
    }

    /// A camera that never answers an inquiry (no fault at all): its owed reply
    /// never arrives, so after the ambiguity window only that camera's inquiries
    /// fail, promptly and unwritten. A STOP to the same camera and inquiries to
    /// another camera on the same session still succeed; nothing is poisoned. On
    /// 5a3e81d1 the power inquiry was resent for ~7 s and later inquiries were
    /// sent into an uncorrelatable lane.
    fn unanswered_stream_inquiry_latches_only_that_cameras_inquiries() {
        let bench = Bench::with_targets(
            SendSemantics::Stream,
            vec![CameraId::CAMERA_1, CameraId::CAMERA_2],
            vec![1],
        );
        let session = open!(bench, bench.config()).expect("session");
        let first = session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
            .expect("camera 1");
        let second = session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
            .expect("camera 2");

        let power = wait!(first.power().state());
        assert!(conclusive_timeout(&power), "{power:?}");

        let started = now!();
        let queued = wait!(first.focus().mode());
        let elapsed = now!().duration_since(started);
        assert!(elapsed < Duration::from_secs(3));
        if virtual_clock!() {
            assert_eq!(
                elapsed,
                g2_timing().ambiguity_timeout(),
                "the queued inquiry fails when the ambiguity window closes"
            );
        }
        latched_inquiry_failure(&queued);

        let started = now!();
        let rejected = wait!(first.focus().mode());
        let elapsed = now!().duration_since(started);
        assert!(
            elapsed < Duration::from_millis(200),
            "a latched lane fails promptly"
        );
        if virtual_clock!() {
            assert_eq!(elapsed, Duration::ZERO, "a latched lane fails without waiting");
        }
        latched_inquiry_failure(&rejected);
        assert_eq!(bench.count(1, &FOCUS_MODE_INQUIRY), 0);
        assert_eq!(bench.count(1, &POWER_INQUIRY), 1);

        wait!(wait!(first.pan_tilt().stop())
            .expect("STOP admitted")
            .applied())
        .expect("STOP to the latched camera still applies");
        assert!(wait!(second.power().state()).expect("other camera unaffected"));
        assert!(matches!(wait!(second.focus().mode()), Ok(FocusMode::Manual)));
        session.shutdown().expect("session was never poisoned");
    }

    /// The same latch on a single IP-addressed camera: once the ambiguity
    /// window passes, the queued inquiry and every later one fail with
    /// `InquiryCorrelationLost` (the later ones at admission, promptly) without
    /// being written, while a STOP to the camera still applies.
    fn unanswered_stream_inquiry_latches_a_single_cameras_inquiries() {
        let bench = Bench::with_targets(SendSemantics::Stream, vec![CameraId::CAMERA_1], vec![1]);
        let session = open!(bench, bench.config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let power = wait!(camera.power().state());
        assert!(conclusive_timeout(&power), "{power:?}");

        let queued = wait!(camera.focus().mode());
        latched_inquiry_failure(&queued);
        let started = now!();
        let rejected = wait!(camera.focus().mode());
        let elapsed = now!().duration_since(started);
        assert!(
            elapsed < Duration::from_millis(200),
            "a latched lane fails promptly"
        );
        if virtual_clock!() {
            assert_eq!(elapsed, Duration::ZERO, "a latched lane fails without waiting");
        }
        latched_inquiry_failure(&rejected);
        assert_eq!(bench.count(1, &FOCUS_MODE_INQUIRY), 0);

        wait!(wait!(camera.pan_tilt().stop())
            .expect("STOP admitted")
            .applied())
        .expect("STOP to the latched camera applies");
        session.shutdown().expect("shutdown");
    }

    /// An owed reply that arrives only after the window (a long stall) still
    /// settles: the latched lane reopens and later inquiries return correct data,
    /// never the stale value. On 5a3e81d1 the stale `02` was read as AUTO.
    fn owed_reply_arriving_after_the_window_reopens_the_lane_with_correct_data() {
        let bench = Bench::new(SendSemantics::Stream);
        let session = open!(bench, bench.config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        bench.start_fault();
        let power = wait!(camera.power().state());
        assert!(conclusive_timeout(&power), "{power:?}");
        let during = wait!(camera.focus().mode());
        latched_inquiry_failure(&during);

        after!(RETRANSMIT_AFTER, bench.lift_fault());
        let deadline = now!() + Duration::from_secs(3);
        let mode = loop {
            let mode = wait!(camera.focus().mode());
            assert!(
                !matches!(mode, Ok(FocusMode::Auto)),
                "a stale power reply was returned as the focus mode"
            );
            if mode.is_ok() || now!() > deadline {
                break mode;
            }
            pause!(Duration::from_millis(50));
        };
        assert!(matches!(mode, Ok(FocusMode::Manual)), "{mode:?}");
        assert!(wait!(camera.power().state()).expect("power after recovery"));
        session.shutdown().expect("shutdown");
    }

    /// Datagram behaviour is unchanged: a lost UDP inquiry is still resent under
    /// the Inquiry retry policy (1 + 5 writes) and fails conclusively.
    fn datagram_inquiry_reply_timeout_still_retries() {
        let bench = Bench::new(SendSemantics::Datagram);
        let session = open!(bench, bench.config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        bench.start_fault();
        let power = wait!(camera.power().state());
        assert!(conclusive_timeout(&power), "{power:?}");
        assert_eq!(bench.count(1, &POWER_INQUIRY), 6);
        after!(RETRANSMIT_AFTER, bench.lift_fault());
        assert!(wait!(camera.power().state()).expect("datagram recovers"));
        session.shutdown().expect("shutdown");
    }

    /// Raw commands under the same stall are unchanged: the STOP is written once,
    /// fails `UnsequencedCommandUnconfirmed` at its ACK deadline, and is never
    /// replayed.
    fn stalled_stream_command_is_unconfirmed_without_replay() {
        let bench = Bench::new(SendSemantics::Stream);
        let session = open!(bench, bench.config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        bench.start_fault();
        let result = wait!(wait!(camera.pan_tilt().stop()).expect("admitted").applied());
        assert!(
            matches!(result, Err(Error::UnsequencedCommandUnconfirmed)),
            "{result:?}"
        );
        assert_eq!(bench.count(1, &PAN_TILT_STOP), 1);
        after!(RETRANSMIT_AFTER, bench.lift_fault());
        session.shutdown().expect("shutdown");
    }
}
