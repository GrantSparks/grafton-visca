//! Issue #795: raw-VISCA correlation and certainty regressions from the
//! PTZOptics G2 bench (three firmware builds, 2026-10-04).
//!
//! Each scenario replays the captured wire exchange through a scripted
//! transport on the real `PtzOpticsG2` profile, on both public facades:
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

#![cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
};

const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xff];
const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];

const ACK_S1: &[u8] = &[0x90, 0x41, 0xff];
const ACK_S2: &[u8] = &[0x90, 0x42, 0xff];
const COMPLETE_S1: &[u8] = &[0x90, 0x51, 0xff];
const COMPLETE_S2: &[u8] = &[0x90, 0x52, 0xff];
const NOT_EXECUTABLE_S1: &[u8] = &[0x90, 0x61, 0x41, 0xff];
const NOT_EXECUTABLE_S2: &[u8] = &[0x90, 0x62, 0x41, 0xff];

type Script = Box<dyn FnMut(&[u8]) -> Vec<&'static [u8]> + Send>;

/// The camera side of one scripted connection.
///
/// While `stalled` is set, every write is accepted locally but held back, as
/// a TCP connection does during a retransmission stall. Once the stall lifts,
/// the camera answers every held write in order before anything newer.
struct Wire {
    stalled: bool,
    held: Vec<Vec<u8>>,
    replies: VecDeque<Vec<u8>>,
    writes: Vec<Vec<u8>>,
    script: Script,
}

impl Wire {
    fn write(&mut self, bytes: &[u8]) {
        self.writes.push(bytes.to_vec());
        if self.stalled {
            self.held.push(bytes.to_vec());
        } else {
            let replies = (self.script)(bytes);
            self.replies.extend(replies.into_iter().map(<[u8]>::to_vec));
        }
    }

    fn read(&mut self) -> Option<Vec<u8>> {
        if !self.stalled {
            for request in std::mem::take(&mut self.held) {
                let replies = (self.script)(&request);
                self.replies.extend(replies.into_iter().map(<[u8]>::to_vec));
            }
        }
        self.replies.pop_front()
    }
}

/// Shared handle to the scripted camera, held by the test and the transport.
#[derive(Clone)]
struct Link(Arc<Mutex<Wire>>);

impl fmt::Debug for Link {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Link").finish_non_exhaustive()
    }
}

impl Link {
    fn new(script: Script) -> Self {
        Self(Arc::new(Mutex::new(Wire {
            stalled: false,
            held: Vec::new(),
            replies: VecDeque::new(),
            writes: Vec::new(),
            script,
        })))
    }

    fn stall(&self) {
        self.0.lock().unwrap().stalled = true;
    }

    fn lift(&self) {
        self.0.lock().unwrap().stalled = false;
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.0.lock().unwrap().writes.clone()
    }

    fn count(&self, frame: &[u8]) -> usize {
        self.0
            .lock()
            .unwrap()
            .writes
            .iter()
            .filter(|write| write.as_slice() == frame)
            .count()
    }

    fn write(&self, bytes: &[u8]) {
        self.0.lock().unwrap().write(bytes);
    }

    fn read(&self) -> Option<Vec<u8>> {
        self.0.lock().unwrap().read()
    }
}

/// The PTZOptics G2 bench halt transcript: the pan/tilt STOP completes on S1, the zoom
/// STOP on S2, and the focus STOP (auto-focus mode) is rejected without an
/// ACK by `90 61 41 FF`, naming S1, the next free socket in rotation.
fn halt_transcript() -> Script {
    Box::new(|bytes: &[u8]| match bytes {
        PAN_TILT_STOP => vec![ACK_S1, COMPLETE_S1],
        ZOOM_STOP => vec![ACK_S2, COMPLETE_S2],
        FOCUS_STOP => vec![NOT_EXECUTABLE_S1],
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
            return vec![ACK_S2, COMPLETE_S2];
        }
        if bytes == FOCUS_STOP {
            return vec![NOT_EXECUTABLE_S1, COMPLETE_S1];
        }
        match &relative {
            None => {
                relative = Some(bytes.to_vec());
                vec![ACK_S1]
            }
            Some(_) => vec![ACK_S1, COMPLETE_S1],
        }
    })
}

/// The camera accepts the relative move on S1, then reports on S1 that it
/// cannot complete it. Any replay would be acknowledged and completed.
fn post_ack_rejection() -> Script {
    let mut first = true;
    Box::new(move |_bytes: &[u8]| {
        if std::mem::take(&mut first) {
            vec![ACK_S1, NOT_EXECUTABLE_S1]
        } else {
            vec![ACK_S1, COMPLETE_S1]
        }
    })
}

/// Camera truth for the stalled-stream scenarios: a zoom drive is accepted on
/// S1 and completes, a focus STOP is rejected (auto-focus) naming S2, and a
/// zoom wide is accepted on S2 and completes.
fn stalled_stream_camera() -> Script {
    Box::new(|bytes: &[u8]| match bytes {
        ZOOM_TELE => vec![ACK_S1, COMPLETE_S1],
        FOCUS_STOP => vec![NOT_EXECUTABLE_S2],
        ZOOM_WIDE => vec![ACK_S2, COMPLETE_S2],
        _ => Vec::new(),
    })
}

/// The first command is never answered (it stays unacknowledged), so later
/// ordinary motion queues behind it. Every halt STOP completes normally.
fn unanswered_first_command() -> Script {
    Box::new(|bytes: &[u8]| match bytes {
        PAN_TILT_STOP | ZOOM_STOP | FOCUS_STOP => vec![ACK_S2, COMPLETE_S2],
        _ => Vec::new(),
    })
}

#[cfg(feature = "blocking")]
mod blocking {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use grafton_visca::{
        blocking::{Session, SessionConfig},
        camera::profiles::PtzOpticsG2,
        command::CommandKind,
        profile::ProfileSpec,
        transport::{
            BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
        },
        units::Degrees,
        Certainty, Error, FailureContext, FailureStage, HaltOutcome, SpeedLevel,
    };

    use super::*;

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        link: Link,
        semantics: SendSemantics,
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
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            self.link.write(bytes);
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            destination: &mut [u8],
            timeout: Duration,
        ) -> Result<ReceiveOutcome, Error> {
            if let Some(reply) = self.link.read() {
                return Ok(ReceiveOutcome::copy_message(&reply, destination));
            }
            thread::sleep(timeout.min(Duration::from_millis(2)));
            Err(Error::io_timeout())
        }

        fn send_semantics(&self) -> SendSemantics {
            self.semantics
        }
    }

    fn open_with(script: Script, semantics: SendSemantics) -> (Session, Link) {
        let link = Link::new(script);
        let session = Session::open(
            ScriptedTransport {
                config: TransportConfig::default(),
                link: link.clone(),
                semantics,
            },
            SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2")),
        )
        .expect("session");
        (session, link)
    }

    fn open(script: Script) -> (Session, Link) {
        open_with(script, SendSemantics::Stream)
    }

    #[test]
    fn g2_focus_stop_rejected_on_a_free_socket_fails_conclusively_without_retry() {
        let (session, link) = open(halt_transcript());
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let started = Instant::now();
        let report = camera.motion().stop_all_motion().expect("halt accepted");
        let elapsed = started.elapsed();

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
        assert_eq!(
            link.writes(),
            [PAN_TILT_STOP, ZOOM_STOP, FOCUS_STOP].map(<[u8]>::to_vec)
        );
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn error_naming_a_busy_socket_ends_the_executing_move_without_replay() {
        let (session, link) = open(busy_socket_rejection());
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        camera
            .zoom()
            .stop()
            .expect("zoom stop admitted")
            .applied()
            .expect("the zoom stop completes naming its socket");

        let mut relative = camera
            .pan_tilt()
            .relative(Degrees(10.0), Degrees(0.0), SpeedLevel::Fastest)
            .expect("relative move admitted");
        thread::sleep(Duration::from_millis(150));
        let relative_frame = link.writes()[1].clone();
        let focus = camera
            .focus()
            .stop()
            .expect("focus stop admitted")
            .applied();

        assert!(
            matches!(focus, Err(Error::UnsequencedCommandUnconfirmed)),
            "the error named the executing S1, not the STOP: {focus:?}"
        );
        let moved = relative.applied_with_timeout(Duration::from_secs(5));
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

    #[test]
    fn post_ack_rejection_of_the_executing_move_is_terminal_and_never_replayed() {
        let (session, link) = open(post_ack_rejection());
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let result = camera
            .pan_tilt()
            .relative(Degrees(10.0), Degrees(0.0), SpeedLevel::Fastest)
            .expect("relative move admitted")
            .applied_with_timeout(Duration::from_secs(5));

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
        thread::sleep(Duration::from_millis(300));
        assert_eq!(
            link.writes().len(),
            1,
            "an acknowledged command is never written again"
        );
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn stale_stream_ack_never_completes_a_later_stop() {
        let (session, link) = open(stalled_stream_camera());
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = camera.zoom().tele().expect("tele admitted").applied();
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );
        // Outlive the one-second ambiguity hold.
        thread::sleep(Duration::from_millis(1_100));

        let mut stop = camera.focus().stop().expect("focus stop admitted");
        thread::sleep(Duration::from_millis(150));
        assert_eq!(
            link.count(FOCUS_STOP),
            1,
            "a STOP is still written while the response is owed"
        );
        link.lift();
        let stop = stop.applied_with_timeout(Duration::from_secs(5));

        // The stalled tele's ACK and completion arrive first. They are owed to
        // the tele and must not make the rejected STOP look applied.
        assert!(matches!(stop, Err(Error::CommandNotExecutable)), "{stop:?}");
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn ordinary_motion_waits_for_the_owed_stream_response() {
        let (session, link) = open(stalled_stream_camera());
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = camera.zoom().tele().expect("tele admitted").applied();
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );

        // Inside the owing command's one-second window, ordinary motion
        // queues rather than being piled into the stalled stream.
        let mut wide = camera.zoom().wide().expect("wide admitted");
        thread::sleep(Duration::from_millis(300));
        assert_eq!(link.writes(), [ZOOM_TELE.to_vec()]);

        link.lift();
        wide.applied_with_timeout(Duration::from_secs(5))
            .expect("wide is written and settled once the owed ACK arrived");
        assert_eq!(link.writes(), [ZOOM_TELE, ZOOM_WIDE].map(<[u8]>::to_vec));
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn a_long_stall_latches_only_ordinary_commands_until_the_owed_answer() {
        let (session, link) = open(stalled_stream_camera());
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = camera.zoom().tele().expect("tele admitted").applied();
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );
        thread::sleep(Duration::from_millis(1_100));

        let error = camera
            .zoom()
            .wide()
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
        let _stop = camera.focus().stop().expect("focus stop admitted");
        thread::sleep(Duration::from_millis(150));
        assert_eq!(link.count(FOCUS_STOP), 1);
        assert_eq!(link.count(ZOOM_WIDE), 0);

        // The owed answer arrives and reopens the lane.
        link.lift();
        thread::sleep(Duration::from_millis(300));
        camera
            .zoom()
            .wide()
            .expect("wide admitted")
            .applied_with_timeout(Duration::from_secs(5))
            .expect("the lane reopened");
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn halt_superseding_unsent_motion_reports_not_accepted() {
        let (session, link) = open_with(unanswered_first_command(), SendSemantics::Datagram);
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let _first = camera.zoom().tele().expect("tele admitted");
        thread::sleep(Duration::from_millis(50));
        let mut queued = camera.zoom().wide().expect("wide admitted");
        let _report = camera.motion().stop_all_motion().expect("halt accepted");

        let error = queued.applied().expect_err("the halt superseded it");
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

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod asynchronous {
    use std::{
        future::Future,
        time::{Duration, Instant},
    };

    use grafton_visca::{
        camera::profiles::PtzOpticsG2,
        profile::ProfileSpec,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, ReceiveOutcome, SendSemantics,
            TransportConfig,
        },
        units::Degrees,
        Certainty, Error, FailureContext, FailureStage, HaltOutcome, Session, SessionConfig,
        SpeedLevel, TokioRuntime,
    };

    use super::*;

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        link: Link,
        semantics: SendSemantics,
    }

    impl HasTransportConfig for ScriptedTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for ScriptedTransport {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            self.link.write(bytes);
            async { Ok(()) }
        }

        async fn recv_into(&mut self, destination: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            loop {
                if let Some(reply) = self.link.read() {
                    return Ok(ReceiveOutcome::copy_message(&reply, destination));
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Ip)
        }

        fn send_semantics(&self) -> SendSemantics {
            self.semantics
        }
    }

    async fn open_with(script: Script, semantics: SendSemantics) -> (Session, Link) {
        let link = Link::new(script);
        let session = Session::open(
            ScriptedTransport {
                config: TransportConfig::default(),
                link: link.clone(),
                semantics,
            },
            SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2")),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session");
        (session, link)
    }

    async fn open(script: Script) -> (Session, Link) {
        open_with(script, SendSemantics::Stream).await
    }

    #[tokio::test]
    async fn g2_focus_stop_rejected_on_a_free_socket_fails_conclusively_without_retry() {
        let (session, link) = open(halt_transcript()).await;
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let started = Instant::now();
        let report = camera
            .motion()
            .stop_all_motion()
            .await
            .expect("halt accepted");
        let elapsed = started.elapsed();

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
            "{report:?}"
        );
        assert!(elapsed < Duration::from_millis(450), "{elapsed:?}");
        assert_eq!(link.count(FOCUS_STOP), 1);
        assert_eq!(link.writes().len(), 3);
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn error_naming_a_busy_socket_ends_the_executing_move_without_replay() {
        let (session, link) = open(busy_socket_rejection()).await;
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        camera
            .zoom()
            .stop()
            .await
            .expect("zoom stop admitted")
            .applied()
            .await
            .expect("the zoom stop completes naming its socket");

        let mut relative = camera
            .pan_tilt()
            .relative(Degrees(10.0), Degrees(0.0), SpeedLevel::Fastest)
            .await
            .expect("relative move admitted");
        tokio::time::sleep(Duration::from_millis(150)).await;
        let relative_frame = link.writes()[1].clone();
        let focus = camera
            .focus()
            .stop()
            .await
            .expect("focus stop admitted")
            .applied()
            .await;

        assert!(
            matches!(focus, Err(Error::UnsequencedCommandUnconfirmed)),
            "{focus:?}"
        );
        let moved = relative.applied_with_timeout(Duration::from_secs(5)).await;
        assert!(
            matches!(moved, Err(Error::CommandFailedAfterAck { .. })),
            "{moved:?}"
        );
        assert_eq!(link.count(&relative_frame), 1);
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn post_ack_rejection_of_the_executing_move_is_terminal_and_never_replayed() {
        let (session, link) = open(post_ack_rejection()).await;
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let result = camera
            .pan_tilt()
            .relative(Degrees(10.0), Degrees(0.0), SpeedLevel::Fastest)
            .await
            .expect("relative move admitted")
            .applied_with_timeout(Duration::from_secs(5))
            .await;

        assert!(
            matches!(
                &result,
                Err(Error::CommandFailedAfterAck { source, .. })
                    if matches!(**source, Error::CommandNotExecutable)
            ),
            "{result:?}"
        );
        assert_eq!(
            result.unwrap_err().failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::Unconfirmed
            ))
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(link.writes().len(), 1);
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn stale_stream_ack_never_completes_a_later_stop() {
        let (session, link) = open(stalled_stream_camera()).await;
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = camera
            .zoom()
            .tele()
            .await
            .expect("tele admitted")
            .applied()
            .await;
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );
        tokio::time::sleep(Duration::from_millis(1_100)).await;

        let mut stop = camera.focus().stop().await.expect("focus stop admitted");
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(link.count(FOCUS_STOP), 1);
        link.lift();
        let stop = stop.applied_with_timeout(Duration::from_secs(5)).await;
        assert!(matches!(stop, Err(Error::CommandNotExecutable)), "{stop:?}");
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn ordinary_motion_waits_for_the_owed_stream_response() {
        let (session, link) = open(stalled_stream_camera()).await;
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = camera
            .zoom()
            .tele()
            .await
            .expect("tele admitted")
            .applied()
            .await;
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );

        let mut wide = camera.zoom().wide().await.expect("wide admitted");
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(link.writes(), [ZOOM_TELE.to_vec()]);

        link.lift();
        wide.applied_with_timeout(Duration::from_secs(5))
            .await
            .expect("wide is written and settled once the owed ACK arrived");
        assert_eq!(link.writes(), [ZOOM_TELE, ZOOM_WIDE].map(<[u8]>::to_vec));
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn a_long_stall_latches_only_ordinary_commands_until_the_owed_answer() {
        let (session, link) = open(stalled_stream_camera()).await;
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        link.stall();
        let tele = camera
            .zoom()
            .tele()
            .await
            .expect("tele admitted")
            .applied()
            .await;
        assert!(
            matches!(tele, Err(Error::UnsequencedCommandUnconfirmed)),
            "{tele:?}"
        );
        tokio::time::sleep(Duration::from_millis(1_100)).await;

        let error = camera
            .zoom()
            .wide()
            .await
            .expect_err("the latched command lane rejects ordinary motion unwritten");
        assert!(
            matches!(error, Error::CommandCorrelationLost { .. }),
            "{error:?}"
        );
        let _stop = camera.focus().stop().await.expect("focus stop admitted");
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(link.count(FOCUS_STOP), 1);
        assert_eq!(link.count(ZOOM_WIDE), 0);

        link.lift();
        tokio::time::sleep(Duration::from_millis(300)).await;
        camera
            .zoom()
            .wide()
            .await
            .expect("wide admitted")
            .applied_with_timeout(Duration::from_secs(5))
            .await
            .expect("the lane reopened");
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn halt_superseding_unsent_motion_reports_not_accepted() {
        let (session, link) = open_with(unanswered_first_command(), SendSemantics::Datagram).await;
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        let _first = camera.zoom().tele().await.expect("tele admitted");
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut queued = camera.zoom().wide().await.expect("wide admitted");
        let _report = camera
            .motion()
            .stop_all_motion()
            .await
            .expect("halt accepted");

        let error = queued.applied().await.expect_err("the halt superseded it");
        assert!(matches!(error, Error::MotionSuperseded { .. }), "{error:?}");
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::NotAccepted
            ))
        );
        assert_eq!(link.count(ZOOM_WIDE), 0);
        session.shutdown().expect("shutdown");
    }
}
