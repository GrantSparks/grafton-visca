//! Issue #795: raw-VISCA correlation and certainty regressions from the
//! PTZOptics G2 bench (three firmware builds, 2026-10-04).
//!
//! Each scenario replays the captured wire exchange through a scripted
//! camera on the real `PtzOpticsG2` profile, on the blocking facade and on the
//! async facade under each enabled runtime:
//!
//! - A focus STOP sent in auto-focus mode is answered, without an ACK, by
//!   `90 6y 41 FF` naming the camera's *next free* socket. That rejection is
//!   the STOP's conclusive answer and is reported promptly, without retry.
//! - An error naming a socket an executing command holds is that command's
//!   execution error, never a reason to write it again: an acknowledged
//!   command is never written again, whatever error arrives later.
//! - On a byte stream, a command that went unconfirmed still owes its ACK. A
//!   late ACK delivered after the stall never completes a later command;
//!   ordinary motion waits through the owing command's window, then fails
//!   `CommandCorrelationLost` until the owed answer arrives, while STOPs are
//!   always written.
//! - A camera error after an ACK is `CommandFailedAfterAck`: unconfirmed and
//!   never retryable.
//! - A halt that supersedes motion before it was ever written reports
//!   `Certainty::NotAccepted`.
//!
//! The Tokio cases run on paused (virtual) time, so the G2 ACK deadline, the
//! ambiguity hold and every `pause!` lapse at once and exactly. A
//! step that waits for a write waits for that write (`wait_for_writes!`), not
//! for a fixed time; the remaining real-time pauses on the blocking and smol
//! facades either outlive a production window or prove that nothing happens
//! in one, which no event can signal.

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
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    camera::profiles::PtzOpticsG2, profile::ProfileSpec, transport::SendSemantics, units::Degrees,
    Certainty, Error, FailureContext, FailureStage, HaltOutcome, SessionConfig, SpeedLevel,
};

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use fake_camera::AsyncWire;
#[cfg(feature = "blocking")]
use fake_camera::BlockingWire;
use fake_camera::{frames, FakeCamera, FOCUS_STOP, ZOOM_STOP, ZOOM_TELE};
#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use grafton_visca::transport::AddressingMode;

const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];
const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];

type Script = Box<dyn FnMut(&[u8]) -> Vec<Vec<u8>> + Send>;

/// The stall state of one scripted connection.
///
/// While `stalled` is set, every write is accepted locally but held back, as
/// a TCP connection does during a retransmission stall. Once the stall lifts,
/// the camera answers every held write in order before anything newer.
struct Stall {
    stalled: bool,
    held: Vec<Vec<u8>>,
    script: Script,
}

/// The scripted camera behind one connection, held by the test and the wire.
struct Link {
    camera: FakeCamera,
    stall: Arc<Mutex<Stall>>,
    semantics: SendSemantics,
}

impl Link {
    /// A byte-stream connection, as the G2 serves.
    fn new(script: Script) -> Self {
        Self::with_semantics(script, SendSemantics::Stream)
    }

    fn with_semantics(script: Script, semantics: SendSemantics) -> Self {
        let stall = Arc::new(Mutex::new(Stall {
            stalled: false,
            held: Vec::new(),
            script,
        }));
        let shared = Arc::clone(&stall);
        let camera = FakeCamera::new(move |bytes, answer| {
            let mut stall = shared.lock().unwrap();
            if stall.stalled {
                stall.held.push(bytes.to_vec());
            } else {
                for reply in (stall.script)(bytes) {
                    answer.reply(reply);
                }
            }
        });
        Self {
            camera,
            stall,
            semantics,
        }
    }

    /// The blocking wire, in the shape `open!` asks its camera for.
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> BlockingWire {
        self.camera.blocking_wire().with_semantics(self.semantics)
    }

    /// The async wire, in the shape `open!` asks its camera for.
    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> AsyncWire {
        self.camera
            .async_wire()
            .with_semantics(self.semantics)
            .with_addressing(AddressingMode::Ip)
    }

    fn stall(&self) {
        self.stall.lock().unwrap().stalled = true;
    }

    fn lift(&self) {
        let mut stall = self.stall.lock().unwrap();
        stall.stalled = false;
        for request in std::mem::take(&mut stall.held) {
            for reply in (stall.script)(&request) {
                self.camera.push(reply);
            }
        }
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.camera.writes()
    }

    fn count(&self, frame: &[u8]) -> usize {
        self.camera
            .writes()
            .iter()
            .filter(|write| write.as_slice() == frame)
            .count()
    }
}

/// The G2 profile's minimum spacing between two command writes, read from the
/// profile itself.
fn g2_command_spacing() -> Duration {
    ProfileSpec::from_compile_time::<PtzOpticsG2>()
        .expect("G2")
        .timing()
        .minimum_command_spacing()
}

fn g2_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"))
}

/// The PTZOptics G2 bench halt transcript: the pan/tilt STOP completes on S1, the zoom
/// STOP on S2, and the focus STOP (auto-focus mode) is rejected without an
/// ACK by `90 61 41 FF`, naming S1, the next free socket in rotation.
fn halt_transcript() -> Script {
    Box::new(|bytes: &[u8]| match bytes {
        PAN_TILT_STOP => vec![frames::ack(1), frames::complete(1)],
        ZOOM_STOP => vec![frames::ack(2), frames::complete(2)],
        FOCUS_STOP => vec![frames::not_executable(1)],
        _ => Vec::new(),
    })
}

/// A relative pan/tilt move executes on S1. The camera then reports an error
/// naming S1 (busy) while a focus STOP awaits its answer, and later sends S1's
/// completion. On a byte stream an error naming an executing socket is that
/// command's execution error (a rejection names the free socket it was
/// allocated). Any second write of the move frame would be a physical second
/// move.
fn busy_socket_rejection() -> Script {
    let mut relative: Option<Vec<u8>> = None;
    Box::new(move |bytes: &[u8]| {
        // A first zoom stop completes naming its socket: the camera names
        // its sockets in completions, so S1 is known busy below.
        if bytes == ZOOM_STOP {
            return vec![frames::ack(2), frames::complete(2)];
        }
        if bytes == FOCUS_STOP {
            return vec![frames::not_executable(1), frames::complete(1)];
        }
        match &relative {
            None => {
                relative = Some(bytes.to_vec());
                vec![frames::ack(1)]
            }
            Some(_) => vec![frames::ack(1), frames::complete(1)],
        }
    })
}

/// The camera accepts the relative move on S1, then reports on S1 that it
/// cannot complete it. Any replay would be acknowledged and completed.
fn post_ack_rejection() -> Script {
    let mut first = true;
    Box::new(move |_bytes: &[u8]| {
        if std::mem::take(&mut first) {
            vec![frames::ack(1), frames::not_executable(1)]
        } else {
            vec![frames::ack(1), frames::complete(1)]
        }
    })
}

/// Camera truth for the stalled-stream scenarios: a zoom drive is accepted on
/// S1 and completes, a focus STOP is rejected (auto-focus) naming S2, and a
/// zoom wide is accepted on S2 and completes.
fn stalled_stream_camera() -> Script {
    Box::new(|bytes: &[u8]| match bytes {
        ZOOM_TELE => vec![frames::ack(1), frames::complete(1)],
        FOCUS_STOP => vec![frames::not_executable(2)],
        ZOOM_WIDE => vec![frames::ack(2), frames::complete(2)],
        _ => Vec::new(),
    })
}

/// The first command is never answered (it stays unacknowledged), so later
/// ordinary motion queues behind it. Every halt STOP completes normally.
fn unanswered_first_command() -> Script {
    Box::new(|bytes: &[u8]| match bytes {
        PAN_TILT_STOP | ZOOM_STOP | FOCUS_STOP => vec![frames::ack(2), frames::complete(2)],
        _ => Vec::new(),
    })
}

facade_matrix! {
    paused:

    fn g2_focus_stop_rejected_on_a_free_socket_fails_conclusively_without_retry() {
        let link = Link::new(halt_transcript());
        let session = open!(link, g2_config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let started = now!();
        let report = wait!(camera.motion().stop_all_motion()).expect("halt accepted");
        let elapsed = now!().duration_since(started);

        assert!(
            matches!(report.pan_tilt, HaltOutcome::Applied),
            "{report:?}"
        );
        assert!(matches!(report.zoom, HaltOutcome::Applied), "{report:?}");
        assert!(
            matches!(
                report.focus,
                HaltOutcome::Failed(Error::CommandNotExecutable)
            ),
            "the camera's rejection is the focus STOP's conclusive answer: {report:?}"
        );
        // The verdict comes from the camera's frame, not the 500 ms G2 ACK
        // deadline, and a not-executable STOP is never rewritten.
        assert!(elapsed < Duration::from_millis(450), "{elapsed:?}");
        if virtual_clock!() {
            assert_eq!(
                elapsed,
                2 * g2_command_spacing(),
                "the three STOPs are only paced; no deadline is waited out"
            );
        }
        assert_eq!(
            link.writes(),
            [PAN_TILT_STOP, ZOOM_STOP, FOCUS_STOP].map(<[u8]>::to_vec)
        );
        session.shutdown().expect("shutdown");
    }

    fn error_naming_a_busy_socket_ends_the_executing_move_without_replay() {
        let link = Link::new(busy_socket_rejection());
        let session = open!(link, g2_config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        wait!(wait!(camera.zoom().stop())
            .expect("zoom stop admitted")
            .applied())
        .expect("the zoom stop completes naming its socket");

        let mut relative = wait!(camera
            .pan_tilt()
            .relative(Degrees(10.0), Degrees(0.0), SpeedLevel::Fastest))
        .expect("relative move admitted");
        pause!(Duration::from_millis(150));
        let relative_frame = link.writes()[1].clone();
        let focus = wait!(wait!(camera.focus().stop())
            .expect("focus stop admitted")
            .applied());

        assert!(
            matches!(focus, Err(Error::UnsequencedCommandUnconfirmed)),
            "the error named the executing S1, not the STOP: {focus:?}"
        );
        let moved = wait!(relative.applied_with_timeout(Duration::from_secs(5)));
        assert!(
            matches!(moved, Err(Error::CommandFailedAfterAck { .. })),
            "{moved:?}"
        );
        assert_eq!(
            link.count(&relative_frame),
            1,
            "the executing relative move must never be written twice"
        );
        session.shutdown().expect("shutdown");
    }

    fn post_ack_rejection_of_the_executing_move_is_terminal_and_never_replayed() {
        let link = Link::new(post_ack_rejection());
        let session = open!(link, g2_config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let result = wait!(wait!(camera
            .pan_tilt()
            .relative(Degrees(10.0), Degrees(0.0), SpeedLevel::Fastest))
        .expect("relative move admitted")
        .applied_with_timeout(Duration::from_secs(5)));

        assert!(
            matches!(
                &result,
                Err(Error::CommandFailedAfterAck { source, .. })
                    if matches!(**source, Error::CommandNotExecutable)
            ),
            "{result:?}"
        );
        let error = result.unwrap_err();
        assert!(!error.is_retryable());
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::Unconfirmed
            ))
        );
        pause!(Duration::from_millis(300));
        assert_eq!(
            link.writes().len(),
            1,
            "an acknowledged command is never written again"
        );
        session.shutdown().expect("shutdown");
    }

    fn stale_stream_ack_never_completes_a_later_stop() {
        let link = Link::new(stalled_stream_camera());
        let session = open!(link, g2_config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = wait!(wait!(camera.zoom().tele())
            .expect("tele admitted")
            .applied());
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );
        // Outlive the one-second ambiguity hold.
        pause!(Duration::from_millis(1_100));

        let mut stop = wait!(camera.focus().stop()).expect("focus stop admitted");
        // The stalled tele and then the STOP.
        wait_for_writes!(link.camera, 2);
        assert_eq!(
            link.count(FOCUS_STOP),
            1,
            "a STOP is still written while the response is owed"
        );
        link.lift();
        let stop = wait!(stop.applied_with_timeout(Duration::from_secs(5)));

        // The stalled tele's ACK and completion arrive first. They are owed to
        // the tele and must not make the rejected STOP look applied.
        assert!(matches!(stop, Err(Error::CommandNotExecutable)), "{stop:?}");
        session.shutdown().expect("shutdown");
    }

    fn ordinary_motion_waits_for_the_owed_stream_response() {
        let link = Link::new(stalled_stream_camera());
        let session = open!(link, g2_config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = wait!(wait!(camera.zoom().tele())
            .expect("tele admitted")
            .applied());
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );

        // Inside the owing command's one-second window, ordinary motion
        // queues rather than being piled into the stalled stream.
        let mut wide = wait!(camera.zoom().wide()).expect("wide admitted");
        pause!(Duration::from_millis(300));
        assert_eq!(link.writes(), [ZOOM_TELE.to_vec()]);

        link.lift();
        wait!(wide.applied_with_timeout(Duration::from_secs(5)))
            .expect("wide is written and settled once the owed ACK arrived");
        assert_eq!(link.writes(), [ZOOM_TELE, ZOOM_WIDE].map(<[u8]>::to_vec));
        session.shutdown().expect("shutdown");
    }

    fn a_long_stall_latches_only_ordinary_commands_until_the_owed_answer() {
        let link = Link::new(stalled_stream_camera());
        let session = open!(link, g2_config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = wait!(wait!(camera.zoom().tele())
            .expect("tele admitted")
            .applied());
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );
        pause!(Duration::from_millis(1_100));

        let error = wait!(camera.zoom().wide())
            .expect_err("the latched command lane rejects ordinary motion unwritten");
        assert!(
            matches!(error, Error::CommandCorrelationLost { .. }),
            "{error:?}"
        );
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::NotAccepted
            ))
        );
        // A STOP is still written.
        let _stop = wait!(camera.focus().stop()).expect("focus stop admitted");
        // The stalled tele and then the STOP; the rejected wide was never
        // written.
        wait_for_writes!(link.camera, 2);
        assert_eq!(link.count(FOCUS_STOP), 1);
        assert_eq!(link.count(ZOOM_WIDE), 0);

        // The owed answer arrives and reopens the lane.
        link.lift();
        pause!(Duration::from_millis(300));
        wait!(wait!(camera.zoom().wide())
            .expect("wide admitted")
            .applied_with_timeout(Duration::from_secs(5)))
        .expect("the lane reopened");
        session.shutdown().expect("shutdown");
    }

    fn halt_superseding_unsent_motion_reports_not_accepted() {
        let link = Link::with_semantics(unanswered_first_command(), SendSemantics::Datagram);
        let session = open!(link, g2_config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let _first = wait!(camera.zoom().tele()).expect("tele admitted");
        pause!(Duration::from_millis(50));
        let mut queued = wait!(camera.zoom().wide()).expect("wide admitted");
        let _report = wait!(camera.motion().stop_all_motion()).expect("halt accepted");

        let error = wait!(queued.applied()).expect_err("the halt superseded it");
        assert!(matches!(error, Error::MotionSuperseded { .. }), "{error:?}");
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::NotAccepted
            )),
            "the superseded motion was never written"
        );
        assert_eq!(link.count(ZOOM_WIDE), 0);
        session.shutdown().expect("shutdown");
    }
}
