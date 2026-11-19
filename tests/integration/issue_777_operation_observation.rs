//! Operation handles observe their operation through borrowing waits (#777).
//!
//! Every wait takes `&mut self` and the handle caches what it observes, so a
//! wait that times out or is dropped releases only that wait, a later wait
//! continues from what was already observed, and cancellation is one
//! idempotent intent per handle. The scenarios drive a raw two-socket profile
//! over a scripted camera whose wire transcript is the assertion surface.
//!
//! Every scenario in `facade_matrix!` runs on the blocking facade and on the
//! async facade under each enabled runtime. The scenarios in [`async_facade`]
//! run on the async facade only, because each one needs a mechanism the
//! blocking facade does not have:
//!
//! * `abandoned_waits_keep_the_handle_observing`,
//!   `dropped_and_concurrent_cancels_observe_the_one_intent`,
//!   `dropping_a_handle_whose_cancel_future_was_dropped` and
//!   `tokio_select_losing_branch_keeps_the_handle_observing` (Tokio only)
//!   drop an in-flight wait or cancel future, or poll a wait concurrently
//!   with the reply that concludes it. A blocking wait is a synchronous call
//!   that returns only once it concludes or times out, so there is no
//!   in-flight wait to drop; its timed-out form is covered by the matrix
//!   scenarios.
//! * `shutdown_ends_pending_waits` polls a wait before shutting the owner
//!   down, so the wait is provably pending when shutdown arrives. A blocking
//!   wait registers nothing a second thread can observe, so the interleaving
//!   cannot be forced and a port would not prove the wait was pending.
//! * `runtime_profile_handles_share_the_contract` covers concrete
//!   `Operation<AppliedOnly>` and `Operation<Targeted>` handles submitted
//!   through the async `Camera<AnyProfile>` view.

#![cfg(any(
    all(feature = "blocking", feature = "test-utils"),
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
#![allow(clippy::expect_used)]

use grafton_visca_test_support::{facade_matrix, fake_camera, profile_fixtures};

#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grafton_visca::{
    config::SessionConfig,
    operation::{AppliedOnly, CancellationOutcome},
    profiles::ProfileSpec,
    Error,
};

#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
use grafton_visca::{
    error::{Certainty, FailureContext, FailureStage},
    operation::Targeted,
};
#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
use std::time::Instant;

use fake_camera::{frames, FakeCamera, WAIT_BUDGET, ZOOM_STOP, ZOOM_TELE};
use profile_fixtures::NonDefaultCompileTimeProfile as Raw;

#[cfg(any(
    feature = "runtime-tokio",
    feature = "runtime-smol",
    feature = "test-utils"
))]
const PAN_TILT_HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xff];
const CANCEL_SOCKET_ONE: &[u8] = &[0x81, 0x21, 0xff];
const PAN_TILT_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xff];

/// A short wait that the scripted camera never answers in time.
const SHORT: Duration = Duration::from_millis(20);

#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
/// Whether position inquiries are answered, and how many written while they
/// were not are still owed a reply.
#[derive(Debug, Default)]
struct PositionReplies {
    answering: bool,
    owed: usize,
}

#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
/// The switch that makes the scripted camera answer position inquiries.
#[derive(Debug)]
struct Positions {
    camera: FakeCamera,
    replies: Arc<Mutex<PositionReplies>>,
}

#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
impl Positions {
    /// Answers every position inquiry from now on, including those already
    /// written and unanswered.
    fn answer(&self) {
        let mut replies = self.replies.lock().expect("positions lock");
        replies.answering = true;
        for _ in 0..std::mem::take(&mut replies.owed) {
            self.camera.push(pan_tilt_position());
        }
    }
}

/// A fixed pan-tilt position reply.
fn pan_tilt_position() -> Vec<u8> {
    frames::inquiry_reply(&[0x00; 8])
}

#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
/// A camera that replays exactly the frames a test pushes, except pan-tilt
/// position inquiries, which it answers with a fixed position once the test
/// enables that through [`Positions::answer`].
fn scripted_camera() -> (FakeCamera, Positions) {
    let replies = Arc::new(Mutex::new(PositionReplies::default()));
    let responder_replies = Arc::clone(&replies);
    let camera = FakeCamera::new(move |write, answer| {
        if write == PAN_TILT_POSITION_INQUIRY {
            let mut replies = responder_replies.lock().expect("positions lock");
            if replies.answering {
                answer.reply(pan_tilt_position());
            } else {
                replies.owed += 1;
            }
        }
    });
    let positions = Positions {
        camera: camera.clone(),
        replies,
    };
    (camera, positions)
}

fn config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<Raw>().expect("raw two-socket profile"))
}

/// Writes other than position inquiries.
#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
fn commands(camera: &FakeCamera) -> Vec<Vec<u8>> {
    camera
        .writes()
        .into_iter()
        .filter(|bytes| bytes != PAN_TILT_POSITION_INQUIRY)
        .collect()
}

fn debug<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

/// Admits a continuous zoom and acknowledges it on socket one, so the
/// operation is written, accepted, and still running. Expands inside a
/// `facade_matrix!` body.
#[cfg(any(feature = "runtime-tokio", feature = "test-utils"))]
macro_rules! running_zoom {
    ($session:expr, $camera:expr) => {{
        let view = $session
            .camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("raw camera");
        let writes = $camera.write_count();
        let reads = $camera.read_count();
        let operation = wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele))
            .expect("continuous zoom admitted");
        wait_for_writes!($camera, writes + 1);
        $camera.push(frames::ack(1));
        wait_for_reads!($camera, reads + 1);
        operation
    }};
}

/// Use the EVI-H100's raw VISCA framing with a serial stream identity.
#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
struct SerialCamera(FakeCamera);

#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
impl SerialCamera {
    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_transport_kind(grafton_visca::config::TransportKind::Serial)
    }

    #[cfg(any(
        feature = "runtime-tokio",
        all(feature = "runtime-smol", feature = "test-utils")
    ))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.0
            .async_wire()
            .with_transport_kind(grafton_visca::config::TransportKind::Serial)
    }
}

facade_matrix! {
    paused:

    /// A timed-out wait releases only itself: the handle observes the later
    /// outcome and then answers repeat waits from its cache.
    fn a_timed_out_wait_keeps_the_handle_observing() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);

        // The expired wait names the still-running operation and is never
        // retryable: resubmitting would duplicate it (D20, #783).
        let id = moving.id();
        let expired = wait!(moving.applied_with_timeout(SHORT))
            .expect_err("nothing concluded the zoom");
        assert!(matches!(expired, Error::ObservationTimeout { operation, .. } if operation == id));
        assert!(!matches!(expired.recovery(), grafton_visca::error::Recovery::Resubmit { .. }));
        camera.push(frames::complete(1));
        wait!(moving.applied()).expect("the handle observes the completion");
        wait!(moving.applied_with_timeout(Duration::ZERO))
            .expect("a cached outcome needs no wait");
        assert_eq!(camera.writes(), vec![ZOOM_TELE.to_vec()]);
        session.shutdown().expect("owner shutdown");
    }

    /// Application and settlement are observed on one handle, in sequence,
    /// and a settlement wait abandoned during position polling restarts with
    /// a fresh proof instead of losing the cached application.
    fn application_then_settlement_restarts_an_abandoned_proof() {
        let (camera, positions) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let view = session.camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("raw camera");
        let mut home = wait!(view.submit::<Targeted, _>(&grafton_visca::command::pan_tilt::Home)).expect("home admitted");
        wait_for_writes!(camera, 1);
        camera.push(frames::ack(1));

        // Abandoned while awaiting application.
        assert!(matches!(
            wait!(home.settled_with_timeout(SHORT)),
            Err(Error::ObservationTimeout { .. })
        ));
        camera.push(frames::complete(1));
        wait!(home.applied()).expect("home applied");

        // Abandoned while polling: the camera does not answer position
        // inquiries. On the async facade the first abandoned wait has already
        // polled. On the blocking facade a short observer deadline can expire
        // before inquiry admission, so it keeps borrowing until an actual
        // inquiry write proves polling began; the silent camera ensures that
        // wait still abandons an unfinished settlement proof.
        let witness_deadline = Instant::now() + WAIT_BUDGET;
        loop {
            assert!(matches!(
                wait!(home.settled_with_timeout(SHORT)),
                Err(Error::ObservationTimeout { .. })
            ));
            if camera
                .writes()
                .iter()
                .any(|bytes| bytes == PAN_TILT_POSITION_INQUIRY)
            {
                break;
            }
            assert_eq!(FACADE, "blocking", "settlement polls the pan-tilt position");
            assert!(
                Instant::now() < witness_deadline,
                "settlement inquiry was never written"
            );
        }

        positions.answer();
        // The blocking case keeps the untimed `settled()` path; the async
        // cases bound the same wait so a settlement that never concludes
        // fails the case instead of hanging it.
        let evidence = if FACADE == "blocking" {
            wait!(home.settled())
        } else {
            wait!(home.settled_with_timeout(WAIT_BUDGET))
        }
        .expect("two equal samples settle home");
        assert!(
            matches!(evidence, grafton_visca::operation::Settlement::ObservedStable { axes, window, .. }
            if axes == grafton_visca::types::AffectedAxes::PAN_TILT && !window.is_zero())
        );
        let polled = camera.write_count();
        wait!(home.settled_with_timeout(Duration::ZERO)).expect("settlement is cached");
        wait!(home.applied()).expect("application is cached");
        assert_eq!(camera.write_count(), polled, "cached waits poll nothing");
        assert_eq!(commands(&camera), vec![PAN_TILT_HOME.to_vec()]);
        session.shutdown().expect("owner shutdown");
    }

    /// A failed outcome is cached like a success, and `cancel` after it
    /// reports that failure without writing anything.
    fn a_failed_outcome_is_cached_and_answers_cancel() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);
        camera.push(frames::error(1, frames::SYNTAX_ERROR));

        let first = wait!(moving.applied()).expect_err("the camera rejects it");
        let again = wait!(moving.applied()).expect_err("the failure is cached");
        assert_eq!(debug(&first), debug(&again));
        let cancelled = wait!(moving.cancel()).expect_err("the failure decides");
        assert_eq!(debug(&first), debug(&cancelled));
        assert_eq!(camera.writes(), vec![ZOOM_TELE.to_vec()]);
        session.shutdown().expect("owner shutdown");
    }

    /// One handle has one cancellation intent: two timed-out cancels and a
    /// repeat cancel all observe the single socket cancel written for the
    /// first, and the conclusion is cached.
    fn cancellation_is_one_idempotent_intent() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);

        assert!(matches!(
            wait!(moving.cancel_with_timeout(SHORT)),
            Err(Error::ObservationTimeout { .. })
        ));
        assert!(matches!(
            wait!(moving.cancel_with_timeout(SHORT)),
            Err(Error::ObservationTimeout { .. })
        ));
        // Cancellation intent survives both observer timeouts. Await its
        // actual write instead of treating elapsed observer time as worker
        // progress.
        wait_for_writes!(camera, 2);
        assert_eq!(
            camera.writes(),
            vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()],
            "the repeat observed the one intent"
        );

        camera.push(frames::canceled(1));
        assert_eq!(
            wait!(moving.cancel()).expect("cancellation won"),
            CancellationOutcome::Cancelled
        );
        assert!(matches!(wait!(moving.applied()), Err(Error::CommandCancelled)));
        assert_eq!(
            wait!(moving.cancel()).expect("cached"),
            CancellationOutcome::Cancelled
        );
        assert_eq!(camera.write_count(), 2);
        session.shutdown().expect("owner shutdown");
    }

    /// An outcome delivered before `cancel` answers it, whether or not the
    /// handle has observed it yet: nothing is written.
    fn cancel_after_the_outcome_sends_nothing() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);
        camera.push(frames::complete(1));
        wait_for_reads!(camera, 2);

        assert_eq!(
            wait!(moving.cancel()).expect("completed first"),
            CancellationOutcome::Completed
        );
        wait!(moving.applied()).expect("completed");
        assert_eq!(
            wait!(moving.cancel()).expect("the observed outcome decides"),
            CancellationOutcome::Completed
        );
        assert_eq!(camera.writes(), vec![ZOOM_TELE.to_vec()]);
        session.shutdown().expect("owner shutdown");
    }

    /// A first cancel on a handle that has already observed its successful
    /// outcome is answered from that outcome and writes nothing.
    fn cancel_after_the_observed_outcome_sends_nothing() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);
        camera.push(frames::complete(1));
        wait!(moving.applied()).expect("completed");

        assert_eq!(
            wait!(moving.cancel()).expect("completed first"),
            CancellationOutcome::Completed
        );
        assert_eq!(camera.writes(), vec![ZOOM_TELE.to_vec()]);
        drop(moving);
        session.shutdown().expect("owner shutdown");
    }

    /// A cancellation the owner accepted but could not conclude is an error
    /// from `cancel`, cached as the intent's answer; the operation keeps
    /// running and its own outcome then decides every later answer.
    fn a_failed_cancellation_leaves_the_outcome_observable() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);

        // The camera never answers the socket cancel, so the owner's own
        // cancellation observation deadline ends the intent.
        let failed = wait!(moving.cancel()).expect_err("unanswered cancel fails");
        assert_eq!(
            failed.failure_context(),
            Some(FailureContext::new(
                FailureStage::CancellationAttempt,
                Certainty::StillLive
            )),
            "{failed:?}"
        );
        let again = wait!(moving.cancel()).expect_err("the failure is cached");
        assert!(!matches!(failed.recovery(), grafton_visca::error::Recovery::Resubmit { .. }));
        assert!(!matches!(again.recovery(), grafton_visca::error::Recovery::Resubmit { .. }));
        assert_eq!(again.failure_context(), failed.failure_context());
        assert_eq!(
            camera.writes(),
            vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()],
            "a failed intent is not retried"
        );

        camera.push(frames::complete(1));
        wait!(moving.applied()).expect("the operation's own outcome is still observed");
        assert_eq!(
            wait!(moving.cancel()).expect("the outcome decides"),
            CancellationOutcome::Completed
        );
        session.shutdown().expect("owner shutdown");
    }

    /// #841: a failed datagram cancellation write is a cached observation,
    /// never an invitation to resend the handle's single cancellation intent.
    fn a_failed_cancel_write_is_nonretryable_and_keeps_its_source() {
        let camera = FakeCamera::new(|write, answer| {
            if write == CANCEL_SOCKET_ONE {
                answer.fail_send(Error::transport_error(std::io::Error::other("cancel send failed")));
            }
        });
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);
        let first = wait!(moving.cancel()).expect_err("cancel write fails");
        assert!(!matches!(first.recovery(), grafton_visca::error::Recovery::Resubmit { .. }));
        let source = std::error::Error::source(&first).expect("transport source retained");
        assert!(source.to_string().contains("cancel send failed"));
        let again = wait!(moving.cancel()).expect_err("cached cancellation failure");
        assert!(!matches!(again.recovery(), grafton_visca::error::Recovery::Resubmit { .. }));
        assert_eq!(debug(&first), debug(&again));
        assert_eq!(camera.writes(), vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()]);
        assert!(matches!(wait!(moving.applied_with_timeout(SHORT)),
            Err(Error::ObservationTimeout { .. })));
        camera.push(frames::complete(1));
        wait!(moving.applied()).expect("the operation's outcome remains observable");
        session.shutdown().expect("owner shutdown");
    }

    /// An outcome delivered before the owner stopped still answers `cancel`,
    /// even though the cancellation request can no longer be sent.
    fn cancel_after_shutdown_reports_the_delivered_outcome() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);
        camera.push(frames::complete(1));
        wait_for_reads!(camera, 2);
        session.shutdown().expect("owner shutdown");

        assert_eq!(
            wait!(moving.cancel()).expect("the delivered outcome decides"),
            CancellationOutcome::Completed
        );
        wait!(moving.applied()).expect("and is still observable");
    }

    /// Dropping a handle while its cancellation is in flight relinquishes
    /// only observation; the owner concludes the cancellation and keeps
    /// serving.
    fn dropping_a_handle_mid_cancellation_keeps_the_owner_serving() {
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let mut moving = running_zoom!(session, camera);
        assert!(matches!(
            wait!(moving.cancel_with_timeout(SHORT)),
            Err(Error::ObservationTimeout { .. })
        ));
        drop(moving);
        wait_for_writes!(camera, 2);
        camera.push(frames::canceled(1));
        wait_for_reads!(camera, 2);

        let view = session.camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("raw camera");
        let mut stop = wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).expect("stop admitted");
        wait_for_writes!(camera, 3);
        camera.push(frames::ack(1));
        camera.push(frames::complete(1));
        wait!(stop.applied()).expect("the owner keeps serving");
        assert_eq!(
            camera.writes(),
            vec![
                ZOOM_TELE.to_vec(),
                CANCEL_SOCKET_ONE.to_vec(),
                ZOOM_STOP.to_vec()
            ]
        );
        session.shutdown().expect("owner shutdown");
    }

    /// A later conflicting admission supersedes unfinished polled settlement
    /// evidence and preserves the cached application.
    fn later_admission_supersedes_polled_settlement() {
        use grafton_visca::types::{PanSpeed, PanTiltDirection, PanTiltSpeed, TiltSpeed};
        for drive in [false, true] {
            let (camera, _) = scripted_camera();
            let session = open!(camera, config()).expect("owner session");
            let view = session.camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("raw camera");
            let mut first = wait!(view.submit::<Targeted, _>(&grafton_visca::command::pan_tilt::Home)).expect("first home");
            wait_for_writes!(camera, 1);
            camera.push(frames::ack(1));
            camera.push(frames::complete(1));
            wait!(first.applied()).expect("first applied");
            if drive {
                let speed = PanTiltSpeed::new(
                    PanSpeed::new(1).expect("valid pan speed"),
                    TiltSpeed::new(1).expect("valid tilt speed"),
                );
                let drive = grafton_visca::command::pan_tilt::Drive::new(PanTiltDirection::Right, speed);
                wait!(view.submit::<AppliedOnly, _>(&drive)).expect("drive admitted").detach();
            } else {
                wait!(view.submit::<Targeted, _>(&grafton_visca::command::pan_tilt::Home)).expect("later home admitted").detach();
            }
            assert!(
                matches!(wait!(first.settled()), Err(Error::SettlementSuperseded { operation, .. }) if operation == first.id())
            );
            wait!(first.applied()).expect("supersession preserves application");
            session.shutdown().expect("shutdown");
        }
    }

    /// A confirmed move cannot inherit the position produced by an older
    /// BufferFull retry, including when the retry's write reports a failure.
    fn out_of_order_zoom_retry_cannot_prove_newer_move_settlement() {
        use grafton_visca::{
            config::OperationalTuning,
            operation::Settlement,
            profiles::SonyEviH100,
            types::{AffectedAxes, IndividualCameraId, ZoomPosition},
        };

        const OLD_ZOOM: [u8; 9] = [0x81, 0x01, 0x04, 0x47, 1, 0, 0, 0, 0xff];
        const NEW_ZOOM: [u8; 9] = [0x81, 0x01, 0x04, 0x47, 2, 0, 0, 0, 0xff];
        const READ_ZOOM: [u8; 5] = [0x81, 0x09, 0x04, 0x47, 0xff];
        for (observe_before_retry, report_send_error) in [(false, false), (true, false), (false, true)] {
            let mut oldest_attempts = 0;
            let mut position_nibbles = [0; 4];
            let fake = SerialCamera(FakeCamera::new(move |write, answer| {
                if write == READ_ZOOM {
                    answer.reply(frames::inquiry_reply(&position_nibbles));
                    return;
                }
                if write == OLD_ZOOM {
                    oldest_attempts += 1;
                    if oldest_attempts == 1 {
                        // Both camera sockets were occupied when the first
                        // command arrived, and one is free by the next write.
                        answer.reply(frames::buffer_full());
                        return;
                    }
                } else {
                    assert_eq!(write, NEW_ZOOM);
                }
                position_nibbles.copy_from_slice(&write[4..8]);
                if write == OLD_ZOOM && report_send_error {
                    answer.fail_send(std::io::Error::other("retry delivery uncertain").into());
                } else {
                    answer.reply(frames::ack(1)).reply(frames::complete(1));
                }
            }));

            let config = SessionConfig::from_compile_time::<SonyEviH100>()
                .expect("EVI-H100 profile")
                .with_tuning(OperationalTuning::new().with_initial_retry_backoff(Duration::from_millis(300)));
            let session = open!(fake, config).expect("EVI-H100 session");
            let camera = session
                .camera::<SonyEviH100>(IndividualCameraId::CAMERA_1)
                .expect("camera");
            let mut older = wait!(camera.zoom().set_position(ZoomPosition::new(0x1000).expect("position")))
                .expect("older zoom admitted");
            wait_for_reads!(fake.0, 1);
            let mut newer = wait!(camera.zoom().set_position(ZoomPosition::new(0x2000).expect("position")))
                .expect("newer zoom admitted");
            wait!(newer.applied()).expect("newer zoom applied before retry");

            let mut expected = vec![OLD_ZOOM.to_vec(), NEW_ZOOM.to_vec()];
            let cached = if observe_before_retry {
                let settled = wait!(newer.settled()).expect("two samples of the newer move");
                assert!(matches!(settled, Settlement::ObservedStable { axes, .. } if axes == AffectedAxes::ZOOM));
                expected.extend([READ_ZOOM.to_vec(), READ_ZOOM.to_vec()]);
                Some(settled)
            } else {
                None
            };

            let original_result = wait!(older.applied());
            if report_send_error {
                assert!(matches!(original_result, Err(Error::StreamPoisoned { .. })), "{original_result:?}");
            } else {
                original_result.expect("older zoom retry applied");
            }
            expected.push(OLD_ZOOM.to_vec());
            assert_eq!(fake.0.writes(), expected, "the older move wrote last");

            if let Some(evidence) = cached {
                assert_eq!(wait!(newer.settled_with_timeout(Duration::ZERO)).expect("cached evidence"), evidence);
            } else {
                let id = newer.id();
                for _ in 0..2 {
                    assert!(matches!(wait!(newer.settled()),
                        Err(Error::SettlementSuperseded { operation, axes, .. })
                            if operation == id && axes == AffectedAxes::ZOOM));
                }
                wait!(newer.applied()).expect("the camera confirmed the newer move");
            }
            assert_eq!(fake.0.writes(), expected, "repeated waits do not sample the older move");
            let close = wait!(session.close());
            if report_send_error {
                assert!(matches!(close, Err(Error::StreamPoisoned { .. })), "{close:?}");
            } else {
                close.expect("close EVI-H100 session");
            }
        }
    }

    /// Supersession stops a running wait before its next axis or snapshot.
    /// A conflict during the final in-flight query also prevents the commit.
    fn conflicting_admission_stops_an_in_progress_settlement_wait() {
        use grafton_visca::{
            profiles::SonyEviH100,
            types::{AffectedAxes, IndividualCameraId, PresetNumber},
        };

        let position_reply = |request: &[u8]| {
            let payload: &[u8] = match request {
                PAN_TILT_POSITION_INQUIRY => &[0; 8],
                [0x81, 0x09, 0x04, 0x48, 0xff] => &[1, 0, 0, 0],
                _ => &[0; 4],
            };
            frames::inquiry_reply(payload)
        };

        // Hold an answer during the first snapshot, at the end of the
        // baseline, or at the end of the final stable snapshot.
        for held_sample in [1, 3, 6] {
            let mut inquiries = 0;
            let camera = FakeCamera::new(move |write, answer| {
                if write.get(1) == Some(&0x09) {
                    inquiries += 1;
                    if inquiries != held_sample {
                        answer.reply(position_reply(write));
                    }
                } else {
                    answer.reply(frames::ack(1)).reply(frames::complete(1));
                }
            });
            let config = SessionConfig::from_compile_time::<SonyEviH100>().expect("polling profile");
            let session = open!(camera, config).expect("session");
            let view = session.camera::<SonyEviH100>(IndividualCameraId::CAMERA_1)
                .expect("camera");
            let mut original = wait!(view.presets().recall(PresetNumber::new(1).expect("preset")))
                .expect("recall");
            wait!(original.applied()).expect("recall completion");

            let id = original.id();
            let started = now!();
            let (error, ()) = together!({
                let error = wait!(original.settled_with_timeout(WAIT_BUDGET))
                    .expect_err("admission supersedes the active wait");
                let cached = wait!(original.settled_with_timeout(Duration::ZERO))
                    .expect_err("supersession is cached before another wait starts");
                assert_eq!(debug(&error), debug(&cached));
                error
            }, {
                let writes = wait_for_writes!(camera, 1 + held_sample);
                wait!(view.pan_tilt().home()).expect("conflicting admission").detach();
                camera.push(position_reply(&writes[held_sample]));
            });
            assert!(matches!(error, Error::SettlementSuperseded { operation, axes, .. }
                if operation == id && axes == AffectedAxes::MOVEMENT));
            assert!(now!().duration_since(started) < WAIT_BUDGET);
            wait!(session.close()).expect("close");
            assert_eq!(
                camera.writes().iter().filter(|write| write.get(1) == Some(&0x09)).count(),
                held_sample,
                "only the already-submitted queries can reach the wire"
            );
        }
    }
}

/// Scenarios that only the async facade can express; the file header gives
/// each one's reason.
#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_facade {
    use futures_lite::future;
    use grafton_visca::{
        r#async::{Operation, Session},
        runtime::Executor,
    };
    use grafton_visca_test_support::runtime_matrix;

    use super::*;

    async fn open<E: Executor>(executor: &E) -> (Session, FakeCamera) {
        let camera = FakeCamera::silent();
        let session = Session::open(camera.async_wire(), &config(), executor.clone())
            .await
            .expect("owner session");
        (session, camera)
    }

    /// Admits a continuous zoom and acknowledges it on socket one, so the
    /// operation is written, accepted, and still running.
    async fn running_zoom<E: Executor>(
        executor: &E,
        session: &Session,
        camera: &FakeCamera,
    ) -> Operation<AppliedOnly> {
        let view = session
            .camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("raw camera");
        let writes = camera.write_count();
        let reads = camera.read_count();
        let operation = view
            .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)
            .await
            .expect("continuous zoom admitted");
        camera.wait_for_writes_async(executor, writes + 1).await;
        camera.push(frames::ack(1));
        camera.wait_for_reads_async(executor, reads + 1).await;
        operation
    }

    /// A wait that times out, and one dropped as the losing branch of a race,
    /// release only themselves: the handle observes the later outcome and
    /// then answers repeat waits from its cache.
    async fn abandoned_waits_keep_the_handle_observing<E: Executor>(executor: E) {
        let (session, camera) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;

        // The expired wait names the still-running operation and is never
        // retryable: resubmitting would duplicate it (D20, #783).
        let id = moving.id();
        let expired = moving
            .applied_with_timeout(SHORT)
            .await
            .expect_err("nothing concluded the zoom");
        assert!(matches!(expired, Error::ObservationTimeout { operation, .. } if operation == id));
        assert!(!matches!(
            expired.recovery(),
            grafton_visca::error::Recovery::Resubmit { .. }
        ));
        let lost = future::or(async { Some(moving.applied().await) }, async {
            executor.sleep(SHORT).await;
            None
        })
        .await;
        assert!(lost.is_none(), "the timer wins the race");
        assert!(
            futures_lite::future::or(async { Ok(moving.applied().await) }, async {
                executor.sleep(SHORT).await;
                Err(grafton_visca::Error::io_timeout())
            })
            .await
            .is_err(),
            "an executor timeout drops the wait future"
        );

        camera.push(frames::complete(1));
        moving
            .applied_with_timeout(WAIT_BUDGET)
            .await
            .expect("the handle observes the completion");

        // The cached outcome answers without the owner: even a zero timeout
        // succeeds, and after shutdown the handle still reports it.
        moving
            .applied_with_timeout(Duration::ZERO)
            .await
            .expect("a cached outcome needs no wait");
        session.shutdown().expect("owner shutdown");
        moving
            .applied()
            .await
            .expect("the cache outlives the owner");
        assert_eq!(camera.writes(), vec![ZOOM_TELE.to_vec()]);
    }

    /// One handle has one cancellation intent: a timed-out cancel, a dropped
    /// cancel, and a repeat cancel polled concurrently with the reply all
    /// observe the single socket cancel written for the first, and the
    /// conclusion is cached.
    async fn dropped_and_concurrent_cancels_observe_the_one_intent<E: Executor>(executor: E) {
        let (session, camera) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;

        assert!(matches!(
            moving.cancel_with_timeout(SHORT).await,
            Err(Error::ObservationTimeout { .. })
        ));
        camera.wait_for_writes_async(&executor, 2).await;
        assert!(
            futures_lite::future::or(async { Ok(moving.cancel().await) }, async {
                executor.sleep(SHORT).await;
                Err(grafton_visca::Error::io_timeout())
            })
            .await
            .is_err(),
            "an executor timeout drops the cancel future"
        );

        let reply = async {
            executor.sleep(SHORT).await;
            camera.push(frames::canceled(1));
        };
        let (outcome, ()) = future::zip(moving.cancel_with_timeout(WAIT_BUDGET), reply).await;
        assert_eq!(
            outcome.expect("cancellation won"),
            CancellationOutcome::Cancelled
        );
        assert_eq!(
            camera.writes(),
            vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()],
            "every cancel observed the one intent"
        );

        assert!(matches!(
            moving.applied().await,
            Err(Error::CommandCancelled)
        ));
        assert_eq!(
            moving.cancel().await.expect("cached"),
            CancellationOutcome::Cancelled
        );
        assert_eq!(camera.write_count(), 2);
        session.shutdown().expect("owner shutdown");
    }

    /// Shutdown ends a pending wait instead of stranding it.
    async fn shutdown_ends_pending_waits<E: Executor>(executor: E) {
        let (session, camera) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;

        let (waited, shutdown) = futures_lite::future::or(
            async { Ok(future::zip(moving.applied(), async { session.shutdown() }).await) },
            async {
                executor.sleep(WAIT_BUDGET).await;
                Err(grafton_visca::Error::io_timeout())
            },
        )
        .await
        .expect("neither the wait nor shutdown hangs");
        shutdown.expect("owner shutdown");
        let error = waited.expect_err("the operation never concluded");
        let again = moving.applied().await.expect_err("still unconcluded");
        assert_eq!(debug(&error), debug(&again));
    }

    /// Dropping a handle whose cancellation was started by a dropped cancel
    /// future relinquishes only observation; the owner concludes the
    /// cancellation and keeps serving.
    async fn dropping_a_handle_whose_cancel_future_was_dropped<E: Executor>(executor: E) {
        let (session, camera) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;
        assert!(
            futures_lite::future::or(async { Ok(moving.cancel().await) }, async {
                executor.sleep(SHORT).await;
                Err(grafton_visca::Error::io_timeout())
            })
            .await
            .is_err()
        );
        camera.wait_for_writes_async(&executor, 2).await;
        drop(moving);
        camera.push(frames::canceled(1));
        camera.wait_for_reads_async(&executor, 2).await;

        let view = session
            .camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("raw camera");
        let mut stop = view
            .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)
            .await
            .expect("stop admitted");
        camera.wait_for_writes_async(&executor, 3).await;
        camera.push(frames::ack(1));
        camera.push(frames::complete(1));
        stop.applied().await.expect("the owner keeps serving");
        assert_eq!(
            camera.writes(),
            vec![
                ZOOM_TELE.to_vec(),
                CANCEL_SOCKET_ONE.to_vec(),
                ZOOM_STOP.to_vec()
            ]
        );
        session.shutdown().expect("owner shutdown");
    }

    /// The dynamic projection erases the completion marker, not the contract.
    #[cfg(any(feature = "async", feature = "blocking"))]
    async fn runtime_profile_handles_share_the_contract<E: Executor>(executor: E) {
        // Reply as part of the simulated write when no delayed answer is
        // under test. The 100 ms protocol ACK clock starts when the write
        // completes, regardless of when this test task is scheduled again.
        let camera = FakeCamera::new(|write, answer| match write {
            ZOOM_TELE => {
                answer.reply(frames::ack(1));
            }
            PAN_TILT_HOME => {
                answer.reply(frames::ack(1)).reply(frames::complete(1));
            }
            PAN_TILT_POSITION_INQUIRY => {
                answer.reply(pan_tilt_position());
            }
            _ => {}
        });
        let session = Session::open(camera.async_wire(), &config(), executor.clone())
            .await
            .expect("owner session");
        let view = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("dynamic camera");
        let mut moving = view
            .submit(&grafton_visca::command::zoom::Tele)
            .await
            .expect("continuous zoom admitted");
        camera.wait_for_writes_async(&executor, 1).await;

        assert!(matches!(
            moving.applied_with_timeout(SHORT).await,
            Err(Error::ObservationTimeout { .. })
        ));
        assert!(matches!(
            moving.cancel_with_timeout(SHORT).await,
            Err(Error::ObservationTimeout { .. })
        ));
        camera.wait_for_writes_async(&executor, 2).await;
        camera.push(frames::canceled(1));
        assert_eq!(
            moving.cancel().await.expect("cancellation won"),
            CancellationOutcome::Cancelled
        );
        assert!(matches!(
            moving.applied().await,
            Err(Error::CommandCancelled)
        ));
        assert_eq!(
            camera.writes(),
            vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()]
        );

        let mut home = view
            .submit(&grafton_visca::command::pan_tilt::Home)
            .await
            .expect("home admitted");
        home.applied().await.expect("home applied");
        assert_eq!(
            camera.writes(),
            vec![
                ZOOM_TELE.to_vec(),
                CANCEL_SOCKET_ONE.to_vec(),
                PAN_TILT_HOME.to_vec()
            ]
        );
        assert!(
            matches!(home.settled().await.expect("home settled"), grafton_visca::operation::Settlement::ObservedStable { axes, .. } if axes == grafton_visca::types::AffectedAxes::PAN_TILT)
        );
        home.settled_with_timeout(Duration::ZERO)
            .await
            .expect("settlement is cached");
        session.shutdown().expect("owner shutdown");
    }

    runtime_matrix!(
        abandoned_waits_keep_the_handle_observing,
        dropped_and_concurrent_cancels_observe_the_one_intent,
        shutdown_ends_pending_waits,
        dropping_a_handle_whose_cancel_future_was_dropped,
    );

    #[cfg(any(feature = "async", feature = "blocking"))]
    runtime_matrix!(runtime_profile_handles_share_the_contract);

    /// The losing branch of a real `tokio::select!` drops its wait future.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_select_losing_branch_keeps_the_handle_observing() {
        let executor = grafton_visca::runtime::TokioRuntime::from_current().expect("Tokio runtime");
        let (session, camera) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;

        tokio::select! {
            biased;
            () = tokio::time::sleep(SHORT) => {}
            result = moving.applied() => panic!("nothing concluded the zoom: {result:?}"),
        }
        camera.push(frames::complete(1));
        tokio::select! {
            result = moving.applied() => result.expect("the handle observes the completion"),
            () = tokio::time::sleep(WAIT_BUDGET) => panic!("the completion was lost"),
        }
        session.shutdown().expect("owner shutdown");
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn late_completion_while_cancel_caller_is_paused() {
        use grafton_visca::{
            error::Recovery,
            testing::testkit::{ManualClock, ManualClockExecutor},
        };

        let clock = ManualClock::new();
        let executor = ManualClockExecutor::local(&clock);
        executor.run_until(async {
            let (session, camera) = open(&executor).await;
            let mut moving = running_zoom(&executor, &session, &camera).await;
            let id = moving.id();
            let budget = Duration::from_millis(10);
            let deadline = clock.now() + budget;
            let error = {
                let mut cancel = std::pin::pin!(moving.cancel_with_timeout(budget));
                assert!(future::poll_once(&mut cancel).await.is_none());
                camera.wait_for_writes_async(&executor, 2).await;
                assert!(clock.now() <= deadline, "owner accepted the cancel in time");

                // Keep the caller suspended until the owner has delivered the
                // original command's completion and released its terminal sender.
                clock.advance(deadline + Duration::from_millis(1) - clock.now());
                camera.push(frames::complete(1));
                camera.wait_for_reads_async(&executor, 2).await;
                assert_eq!(session.metrics().await.expect("owner metrics").active, 0);
                cancel.await.expect_err("completion missed this wait")
            };
            assert!(
                matches!(error, Error::ObservationTimeout { operation, .. } if operation == id),
                "observed {error:?}"
            );
            assert_eq!(error.recovery(), Recovery::WaitAgain);
            moving
                .applied_with_timeout(Duration::ZERO)
                .await
                .expect("fresh wait recovers the cached completion");
            assert_eq!(
                camera.writes(),
                vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()]
            );
            session.close().await.expect("owner remains healthy");
        });
    }
}
