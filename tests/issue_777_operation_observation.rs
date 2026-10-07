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
//! * `dyn_handles_share_the_contract` (with `dyn-api`) covers the dynamic
//!   projection's completion-erased handles (`submit_applied`,
//!   `submit_targeted`). The blocking dynamic projection has no such handles:
//!   its `submit` returns the typed blocking handle the matrix already covers.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    completion::{AppliedOnly, Targeted},
    profile::ProfileSpec,
    request::builtin::{PanTiltHome, ZoomDrive, ZoomStop},
    CancellationOutcome, Certainty, Error, FailureContext, FailureStage, SessionConfig,
};

use fake_camera::{frames, FakeCamera, WAIT_BUDGET, ZOOM_STOP, ZOOM_TELE};
use profile_fixtures::NonDefaultCompileTimeProfile as Raw;

const PAN_TILT_HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xff];
const CANCEL_SOCKET_ONE: &[u8] = &[0x81, 0x21, 0xff];
const PAN_TILT_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xff];

/// A short wait that the scripted camera never answers in time.
const SHORT: Duration = Duration::from_millis(20);

/// Whether position inquiries are answered, and how many written while they
/// were not are still owed a reply.
#[derive(Debug, Default)]
struct PositionReplies {
    answering: bool,
    owed: usize,
}

/// The switch that makes the scripted camera answer position inquiries.
#[derive(Debug)]
struct Positions {
    camera: FakeCamera,
    replies: Arc<Mutex<PositionReplies>>,
}

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
macro_rules! running_zoom {
    ($session:expr, $camera:expr) => {{
        let view = $session.camera::<Raw>().expect("raw camera");
        let writes = $camera.write_count();
        let reads = $camera.read_count();
        let operation = wait!(view.submit::<AppliedOnly, _>(&ZoomDrive::Tele))
            .expect("continuous zoom admitted");
        wait_for_writes!($camera, writes + 1);
        $camera.push(frames::ack(1));
        wait_for_reads!($camera, reads + 1);
        operation
    }};
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
        assert!(!expired.is_retryable());
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
        let view = session.camera::<Raw>().expect("raw camera");
        let mut home = wait!(view.submit::<Targeted, _>(&PanTiltHome)).expect("home admitted");
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
            matches!(evidence, grafton_visca::Settlement::ObservedStable { axes, window, .. }
            if axes == grafton_visca::AffectedAxes::PAN_TILT && !window.is_zero())
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
        assert!(matches!(wait!(moving.applied()), Err(Error::CommandCanceled)));
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

        let view = session.camera::<Raw>().expect("raw camera");
        let mut stop = wait!(view.submit::<AppliedOnly, _>(&ZoomStop)).expect("stop admitted");
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
        let (camera, _) = scripted_camera();
        let session = open!(camera, config()).expect("owner session");
        let view = session.camera::<Raw>().expect("raw camera");
        let mut first = wait!(view.submit::<Targeted, _>(&PanTiltHome)).expect("first home");
        wait_for_writes!(camera, 1);
        camera.push(frames::ack(1));
        camera.push(frames::complete(1));
        wait!(first.applied()).expect("first applied");
        let later = wait!(view.submit::<Targeted, _>(&PanTiltHome)).expect("later admitted");
        assert!(
            matches!(wait!(first.settled()), Err(Error::SettlementSuperseded { operation, .. }) if operation == first.id())
        );
        wait!(first.applied()).expect("supersession preserves application");
        later.detach();
        session.shutdown().expect("shutdown");
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
    use grafton_visca::{Executor, Operation, Session};

    use super::*;

    async fn open<E: Executor>(executor: &E) -> (Session, FakeCamera, Positions) {
        let (camera, positions) = scripted_camera();
        let session = Session::open(camera.async_wire(), config(), executor.clone())
            .await
            .expect("owner session");
        (session, camera, positions)
    }

    /// Admits a continuous zoom and acknowledges it on socket one, so the
    /// operation is written, accepted, and still running.
    async fn running_zoom<E: Executor>(
        executor: &E,
        session: &Session,
        camera: &FakeCamera,
    ) -> Operation<AppliedOnly> {
        let view = session.camera::<Raw>().expect("raw camera");
        let writes = camera.write_count();
        let reads = camera.read_count();
        let operation = view
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
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
        let (session, camera, _) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;

        // The expired wait names the still-running operation and is never
        // retryable: resubmitting would duplicate it (D20, #783).
        let id = moving.id();
        let expired = moving
            .applied_with_timeout(SHORT)
            .await
            .expect_err("nothing concluded the zoom");
        assert!(matches!(expired, Error::ObservationTimeout { operation, .. } if operation == id));
        assert!(!expired.is_retryable());
        let lost = future::or(async { Some(moving.applied().await) }, async {
            executor.sleep(SHORT).await;
            None
        })
        .await;
        assert!(lost.is_none(), "the timer wins the race");
        assert!(
            executor.timeout(SHORT, moving.applied()).await.is_err(),
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
        let (session, camera, _) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;

        assert!(matches!(
            moving.cancel_with_timeout(SHORT).await,
            Err(Error::ObservationTimeout { .. })
        ));
        camera.wait_for_writes_async(&executor, 2).await;
        assert!(
            executor.timeout(SHORT, moving.cancel()).await.is_err(),
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
            Err(Error::CommandCanceled)
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
        let (session, camera, _) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;

        let (waited, shutdown) = executor
            .timeout(
                WAIT_BUDGET,
                future::zip(moving.applied(), async { session.shutdown() }),
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
        let (session, camera, _) = open(&executor).await;
        let mut moving = running_zoom(&executor, &session, &camera).await;
        assert!(executor.timeout(SHORT, moving.cancel()).await.is_err());
        camera.wait_for_writes_async(&executor, 2).await;
        drop(moving);
        camera.push(frames::canceled(1));
        camera.wait_for_reads_async(&executor, 2).await;

        let view = session.camera::<Raw>().expect("raw camera");
        let mut stop = view
            .submit::<AppliedOnly, _>(&ZoomStop)
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
    #[cfg(feature = "dyn-api")]
    async fn dyn_handles_share_the_contract<E: Executor>(executor: E) {
        let (session, camera, positions) = open(&executor).await;
        let view = session.camera_dyn().expect("dynamic camera");
        let mut moving = view
            .submit_applied(&ZoomDrive::Tele)
            .await
            .expect("continuous zoom admitted");
        camera.wait_for_writes_async(&executor, 1).await;
        camera.push(frames::ack(1));

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
            Err(Error::CommandCanceled)
        ));
        assert_eq!(
            camera.writes(),
            vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()]
        );

        positions.answer();
        let mut home = view
            .submit_targeted(&PanTiltHome)
            .await
            .expect("home admitted");
        camera.wait_for_writes_async(&executor, 3).await;
        camera.push(frames::ack(1));
        camera.push(frames::complete(1));
        home.applied().await.expect("home applied");
        assert!(
            matches!(home.settled().await.expect("home settled"), grafton_visca::Settlement::ObservedStable { axes, .. } if axes == grafton_visca::AffectedAxes::PAN_TILT)
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

    #[cfg(feature = "dyn-api")]
    runtime_matrix!(dyn_handles_share_the_contract);

    /// The losing branch of a real `tokio::select!` drops its wait future.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_select_losing_branch_keeps_the_handle_observing() {
        let executor = grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
        let (session, camera, _) = open(&executor).await;
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
}
