//! Issue #631: runtime timeout/tuning reconfiguration on both facades.
//!
//! `Session::set_tuning` replaces a live session's runtime-mutable tuning
//! without dropping the session. These tests pin the scope of that update:
//!
//! * a request prepared *after* the update uses the new deadline;
//! * a request already admitted keeps the deadline it was admitted with;
//! * the update is validated on the same grounds construction validates on;
//! * the installed value is readable back through `Session::tuning`.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime, because a reconfiguration path that only holds on one
//! owner is not a capability. The one scenario written for a single facade is
//! `concurrent_updates_from_two_handles_are_last_writer_wins`: it polls two
//! `set_tuning` futures together on one task, which only the async facade has,
//! so it runs once per runtime instead.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::time::{Duration, Instant};

use grafton_visca::{
    camera::CameraConfig, completion::AppliedOnly, profiles::SonyFR7, request, AffectedAxes,
    CameraId, ControlClass, Error, FailureStage, Inquiry, InquiryRoute, OperationCommand,
    OperationalTuning, Request, ResponseDecoder, RetryClass, TimeoutClass,
};

use fake_camera::FakeCamera;
use profile_fixtures::sony_session_config;

/// The Sony FR7 profile's acknowledgement deadline, 500 ms on every built-in
/// profile (see issue #689).
const PROFILE_ACK_TIMEOUT: Duration = Duration::from_millis(500);
/// A deliberately wide acknowledgement deadline, four times the profile's.
const WIDE_ACK_TIMEOUT: Duration = Duration::from_secs(2);

/// A never-retried operation, so exactly one acknowledgement deadline decides
/// when a silent camera fails the request.
///
/// A retried request would fail on the sum of several deadlines plus backoff,
/// which measures the retry budget rather than the acknowledgement deadline
/// this file is about.
#[derive(Debug)]
struct SilentOperation;

impl Request for SilentOperation {
    type Class = request::Operation<AppliedOnly>;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
        out[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
        Ok(3)
    }
}

impl OperationCommand<AppliedOnly> for SilentOperation {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::ZOOM
    }
}

/// A never-retried inquiry, used to prove the *inquiry* deadline follows the
/// same update.
#[derive(Debug)]
struct SilentInquiry;

impl Request for SilentInquiry {
    type Class = request::Inquiry;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
        out[..3].copy_from_slice(&[target.to_address_byte(), 0x09, 0xff]);
        Ok(3)
    }
}

impl Inquiry for SilentInquiry {
    type Response = Vec<u8>;

    fn route(&self) -> InquiryRoute {
        InquiryRoute::RAW
    }

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
    }
}

/// Submits one never-retried operation into silence and evaluates to how long
/// the owner took to fail it. The camera accepts every frame and answers none
/// of them, so nothing else can end the request: the wall-clock gap between
/// submission and failure *is* the deadline that fired. Usable only inside a
/// `facade_matrix!` body.
macro_rules! time_to_ack_timeout {
    ($session:expr) => {{
        let camera = $session.camera::<SonyFR7>().expect("camera view");
        let started = Instant::now();
        let error = wait!(wait!(camera.submit::<AppliedOnly, _>(&SilentOperation))
            .expect("submission")
            // Far past any acknowledgement deadline under test, so the
            // observer budget can never be what fired.
            .applied_with_timeout(Duration::from_secs(30)))
        .expect_err("a silent camera cannot acknowledge");
        assert!(
            matches!(&error, Error::Timeout { context, .. } if context.stage == FailureStage::Terminal),
            "expected the owner's own deadline, got {error:?}"
        );
        started.elapsed()
    }};
}

facade_matrix! {
    /// Widen the acknowledgement deadline on a live session and the next
    /// submission waits the new, longer time.
    fn a_widened_ack_timeout_governs_the_next_submission() {
        let fake = FakeCamera::silent();
        let session = open!(fake, sony_session_config()).expect("owner session");

        let before = time_to_ack_timeout!(session);
        assert!(
            before < WIDE_ACK_TIMEOUT / 2,
            "the profile's own {PROFILE_ACK_TIMEOUT:?} deadline should fire well \
             inside {WIDE_ACK_TIMEOUT:?}, took {before:?}"
        );

        wait!(session.set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT)))
            .expect("widening an acknowledgement deadline is accepted");

        let after = time_to_ack_timeout!(session);
        assert!(
            after >= WIDE_ACK_TIMEOUT,
            "the submission after the update must wait the new {WIDE_ACK_TIMEOUT:?} \
             deadline, took {after:?}"
        );

        assert_eq!(
            fake.write_count(),
            2,
            "neither submission was retried, so each measures exactly one deadline"
        );
        session.shutdown().expect("owner shutdown");
    }

    /// The other direction: narrowing back is a real update, not a one-way
    /// ratchet.
    ///
    /// An empty tuning completely replaces the runtime-mutable overrides, so
    /// every deadline returns to its profile default rather than keeping the
    /// widened value.
    fn a_narrowed_ack_timeout_governs_the_next_submission() {
        let fake = FakeCamera::silent();
        let session = open!(fake, sony_session_config()).expect("owner session");

        wait!(session.set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT)))
            .expect("widening an acknowledgement deadline is accepted");
        let widened = time_to_ack_timeout!(session);
        assert!(
            widened >= WIDE_ACK_TIMEOUT,
            "the widened deadline must govern first, took {widened:?}"
        );

        wait!(session.set_tuning(OperationalTuning::new()))
            .expect("returning to the profile defaults is accepted");
        let narrowed = time_to_ack_timeout!(session);
        assert!(
            narrowed < WIDE_ACK_TIMEOUT / 2,
            "clearing the override must return to the profile's {PROFILE_ACK_TIMEOUT:?} \
             deadline, took {narrowed:?}"
        );

        assert_eq!(
            fake.write_count(),
            2,
            "neither submission was retried, so each measures exactly one deadline"
        );
        session.shutdown().expect("owner shutdown");
    }

    /// The inquiry lane follows the same update, so this is session tuning
    /// rather than a command-only knob. A camera view taken before the update
    /// is used on purpose: views read the owner's live tuning instead of a
    /// copy.
    fn a_widened_inquiry_timeout_governs_a_pre_existing_view() {
        let session = open!(FakeCamera::silent(), sony_session_config()).expect("owner session");
        let camera = session
            .camera::<SonyFR7>()
            .expect("camera view taken before the update");

        wait!(session.set_tuning(OperationalTuning::new().inquiry_timeout(Duration::from_secs(2))))
            .expect("widening an inquiry deadline is accepted");

        let started = Instant::now();
        let error = wait!(camera.inquire(&SilentInquiry))
            .expect_err("a silent camera cannot answer an inquiry");
        let elapsed = started.elapsed();
        assert!(
            matches!(&error, Error::Timeout { context, .. } if context.stage == FailureStage::Terminal),
            "expected the owner's own deadline, got {error:?}"
        );
        assert!(
            elapsed >= Duration::from_secs(2),
            "the pre-existing view must prepare under the new two-second deadline, \
             took {elapsed:?}"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// A camera view taken *before* the update still prepares its next request
    /// under the new tuning: the view reads the owner's live value rather than
    /// a copy taken when it was created.
    fn a_view_taken_before_the_update_prepares_under_the_new_tuning() {
        let session = open!(FakeCamera::silent(), sony_session_config()).expect("owner session");
        let camera = session
            .camera::<SonyFR7>()
            .expect("camera view taken before the update");

        wait!(session.set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT)))
            .expect("widening an acknowledgement deadline is accepted");

        let started = Instant::now();
        let error = wait!(wait!(camera.submit::<AppliedOnly, _>(&SilentOperation))
            .expect("submission")
            .applied_with_timeout(Duration::from_secs(30)))
        .expect_err("a silent camera cannot acknowledge");
        let elapsed = started.elapsed();
        assert!(
            matches!(&error, Error::Timeout { context, .. } if context.stage == FailureStage::Terminal),
            "got {error:?}"
        );
        assert!(
            elapsed >= WIDE_ACK_TIMEOUT,
            "the pre-existing view must pick up the new deadline, took {elapsed:?}"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// An update taken while an operation is outstanding applies to the next
    /// prepared request and never re-times the handle already being awaited.
    ///
    /// The owner writes the admitted frame and stamps its acknowledgement
    /// deadline; the update is a later owner turn while that deadline runs. The
    /// operation must resolve on its unchanged deadline, and the caller's next
    /// preparation must use the new one.
    fn an_update_while_an_operation_is_outstanding_leaves_it_alone() {
        let fake = FakeCamera::silent();
        let session = open!(fake, sony_session_config()).expect("owner session");
        let camera = session.camera::<SonyFR7>().expect("camera view");

        let mut operation =
            wait!(camera.submit::<AppliedOnly, _>(&SilentOperation)).expect("submission");
        // Submission returns at admission; the owner writes afterwards. Once
        // the frame is on the wire its acknowledgement deadline is stamped as
        // an absolute instant.
        assert_eq!(
            wait_for_writes!(fake, 1).len(),
            1,
            "exactly the admitted frame is on the wire"
        );

        // The request is admitted under the profile's 500 ms deadline.
        // Widening to two seconds now must not extend it.
        wait!(session.set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT)))
            .expect("an update mid-flight is accepted");

        let started = Instant::now();
        let error = wait!(operation.applied_with_timeout(Duration::from_secs(30)))
            .expect_err("a silent camera cannot acknowledge");
        let elapsed = started.elapsed();
        assert!(
            matches!(&error, Error::Timeout { context, .. } if context.stage == FailureStage::Terminal),
            "the in-flight operation must still resolve on its own deadline, got {error:?}"
        );
        assert!(
            elapsed < WIDE_ACK_TIMEOUT,
            "an admitted request keeps the deadline it was admitted with; \
             waiting {elapsed:?} means it was re-timed"
        );

        // The update did land, though: the next submission uses it.
        assert_eq!(
            session.tuning(),
            OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT)
        );
        let after = time_to_ack_timeout!(session);
        assert!(
            after >= WIDE_ACK_TIMEOUT,
            "the request prepared after the update must use it, took {after:?}"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// The installed value is readable back through every clone, so a caller
    /// can confirm what the session is running under without re-deriving it.
    fn the_installed_tuning_reads_back_through_the_session() {
        let session = open!(FakeCamera::silent(), sony_session_config()).expect("owner session");
        let clone = session.clone();

        assert_eq!(
            session.tuning(),
            OperationalTuning::new(),
            "a session opened with no tuning reports none"
        );

        let tuning = OperationalTuning::new()
            .ack_timeout(WIDE_ACK_TIMEOUT)
            .quick_timeout(Duration::from_secs(9))
            .retry_limit(2);
        wait!(session.set_tuning(tuning)).expect("accepted");
        assert_eq!(session.tuning(), tuning, "the getter reports what was set");
        assert_eq!(
            clone.tuning(),
            tuning,
            "a clone shares the one owner, so it reports the same live value"
        );

        wait!(session.set_tuning(OperationalTuning::new())).expect("accepted");
        assert_eq!(
            session.tuning(),
            OperationalTuning::new(),
            "the runtime-mutable update replaces the previous value whole rather than merging"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// Strict raw-command recovery is immutable session policy, not a tuning
    /// override. A strict session therefore exposes only its genuinely mutable
    /// tuning through `tuning()` and can replace that value normally.
    fn strict_recovery_policy_is_separate_from_runtime_tuning() {
        let default_config = sony_session_config();
        assert!(!default_config.strict_unconfirmed_poison());
        let strict_config = default_config.with_strict_unconfirmed_poison(true);
        assert!(strict_config.strict_unconfirmed_poison());
        let strict_session =
            open!(FakeCamera::silent(), strict_config).expect("strict owner session");
        assert_eq!(strict_session.tuning(), OperationalTuning::new());

        let mutable_update = OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT);
        wait!(strict_session.set_tuning(mutable_update))
            .expect("runtime-mutable tuning is still replaceable on a strict session");
        assert_eq!(
            strict_session.tuning(),
            mutable_update,
            "construction policy must not leak into tuning readback"
        );

        wait!(strict_session.set_tuning(OperationalTuning::new()))
            .expect("all tuning fields remain runtime-mutable");
        assert_eq!(
            strict_session.tuning(),
            OperationalTuning::new(),
            "clearing tuning does not claim to change immutable session policy"
        );
        strict_session.shutdown().expect("owner shutdown");
    }

    /// Runtime updates are validated on exactly the grounds construction
    /// validates on, and a rejected update leaves the live tuning untouched.
    fn an_invalid_update_is_rejected_and_changes_nothing() {
        let session = open!(FakeCamera::silent(), sony_session_config()).expect("owner session");

        let accepted = OperationalTuning::new()
            .ack_timeout(WIDE_ACK_TIMEOUT)
            .quick_timeout(Duration::from_secs(9))
            .retry_limit(2);
        wait!(session.set_tuning(accepted)).expect("accepted");

        // Every rejection below is one `ProfileSpec::validate_tuning` rule, and
        // each is checked against the same profile that would have rejected it
        // in `SessionConfig::with_tuning`.
        let rejected = [
            // Undercuts the profile's own acknowledgement deadline.
            OperationalTuning::new().ack_timeout(Duration::from_millis(1)),
            // A zero deadline is never a deadline.
            OperationalTuning::new().quick_timeout(Duration::ZERO),
            // Raises the profile's socket limit.
            OperationalTuning::new().maximum_command_sockets(3),
            // Zero sockets is not a capacity.
            OperationalTuning::new().maximum_command_sockets(0),
            // Tuning may only lower the default base retry count of 3.
            OperationalTuning::new().retry_limit(4),
            // Backoff that is not ordered, with a budget that cannot hold it.
            OperationalTuning::new().retry_timing(
                Duration::from_millis(50),
                Duration::from_millis(10),
                Duration::from_millis(10),
            ),
        ];
        for tuning in rejected {
            let error = wait!(session.set_tuning(tuning))
                .expect_err("construction would have rejected this tuning");
            assert!(
                matches!(error, Error::InvalidRequest(_)),
                "expected the construction-time rejection, got {error:?}"
            );
            assert_eq!(
                session.tuning(),
                accepted,
                "a rejected update must leave the live tuning untouched"
            );
        }

        // And the session still works afterwards.
        assert_eq!(session.tuning(), accepted);
        session.shutdown().expect("owner shutdown");
    }

    /// The single-camera session exposes the same capability without reaching
    /// for its inner `Session`.
    fn a_camera_session_reconfigures_its_own_session() {
        let config = CameraConfig::<SonyFR7>::new();
        let session =
            open_camera!(FakeCamera::silent(), &config).expect("single-camera session");

        let widened = session.tuning().ack_timeout(WIDE_ACK_TIMEOUT);
        wait!(session.set_tuning(widened)).expect("accepted");
        assert_eq!(session.tuning(), widened);
        assert_eq!(
            session.session().tuning(),
            widened,
            "the owned session reports the same live value"
        );

        let started = Instant::now();
        let error = wait!(wait!(session
            .camera()
            .submit::<AppliedOnly, _>(&SilentOperation))
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(30)))
        .expect_err("a silent camera cannot acknowledge");
        let elapsed = started.elapsed();
        assert!(
            matches!(&error, Error::Timeout { context, .. } if context.stage == FailureStage::Terminal),
            "got {error:?}"
        );
        assert!(
            elapsed >= WIDE_ACK_TIMEOUT,
            "the camera session's next submission must use the new deadline, \
             took {elapsed:?}"
        );

        wait!(session.close()).expect("owner shutdown");
    }
}

/// Two handles reconfiguring at once through the async owner's control
/// boundary.
#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod concurrent_async_updates {
    use grafton_visca::Executor;

    use super::{sony_session_config, Duration, FakeCamera, OperationalTuning};

    /// The boundary serializes the two updates, so the result is one of the
    /// two whole values and never a mixture of both.
    async fn concurrent_updates_from_two_handles_are_last_writer_wins<E: Executor>(executor: E) {
        let session = grafton_visca::Session::open(
            FakeCamera::silent().async_wire(),
            sony_session_config(),
            executor,
        )
        .await
        .expect("owner session");
        let other = session.clone();

        // Two updates that differ in every field they set. A torn result would
        // mix one field from each; the boundary makes that unrepresentable.
        let first = OperationalTuning::new()
            .ack_timeout(Duration::from_millis(500))
            .quick_timeout(Duration::from_secs(9))
            .retry_limit(1);
        let second = OperationalTuning::new()
            .ack_timeout(Duration::from_millis(900))
            .quick_timeout(Duration::from_secs(11))
            .retry_limit(3);

        for _ in 0..16 {
            let left = session.set_tuning(first);
            let right = other.set_tuning(second);
            let (left, right) = futures_lite::future::zip(left, right).await;
            left.expect("accepted");
            right.expect("accepted");

            let observed = session.tuning();
            assert!(
                observed == first || observed == second,
                "the boundary serializes updates, so the live value is one whole \
                 update and never a mixture; observed {observed:?}"
            );
            assert_eq!(
                observed,
                other.tuning(),
                "both handles read the same one owner value"
            );
        }

        session.shutdown().expect("owner shutdown");
    }

    runtime_matrix!(concurrent_updates_from_two_handles_are_last_writer_wins);
}
