//! Issue #631: runtime timeout/tuning reconfiguration through the async facade.
//!
//! The blocking twin is `tests/issue_631_runtime_tuning_blocking.rs`. The two
//! assert the same observable scope against the two owners, because a
//! reconfiguration path that only holds on one of them is not a capability.
//!
//! What is specific to this side is the boundary itself: the update travels as
//! a control message to the single actor, so two session clones reconfiguring
//! concurrently must resolve last-writer-wins with no torn value in between.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    camera::CameraConfig,
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::SonyFR7,
    request,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    AffectedAxes, CameraId, CameraSession, ControlClass, Error, Executor, Inquiry, InquiryRoute,
    OperationCommand, OperationalTuning, Request, ResponseDecoder, RetryClass, Session,
    SessionConfig, TimeoutClass,
};

/// The fixture profile's own acknowledgement deadline (the restored 1.x
/// 500 ms default; see issue #689).
const PROFILE_ACK_TIMEOUT: Duration = Duration::from_millis(500);
/// A deliberately wide acknowledgement deadline, four times the profile's.
const WIDE_ACK_TIMEOUT: Duration = Duration::from_secs(2);

/// A never-retried operation, so exactly one acknowledgement deadline decides
/// when a silent camera fails the request.
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

/// A camera that accepts every frame and answers none of them.
///
/// Deliberate silence is what makes the deadline itself observable: nothing
/// else can end the request, so the wall-clock gap between submission and
/// failure *is* the deadline that fired.
#[derive(Debug)]
struct SilentTransport {
    config: TransportConfig,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    replies: flume::Receiver<Vec<u8>>,
    // Retained so the receive side never disconnects and reports the peer gone.
    _reads: flume::Sender<Vec<u8>>,
}

impl SilentTransport {
    fn new() -> Self {
        let (reads, replies) = flume::unbounded();
        Self {
            config: TransportConfig::default(),
            writes: Arc::new(Mutex::new(Vec::new())),
            replies,
            _reads: reads,
        }
    }

    fn probe(&self) -> Arc<Mutex<Vec<Vec<u8>>>> {
        Arc::clone(&self.writes)
    }
}

impl HasTransportConfig for SilentTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for SilentTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        async { Ok(()) }
    }

    #[allow(clippy::manual_async_fn)]
    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send {
        async move {
            let bytes = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn session_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile"))
}

/// Submits one never-retried operation into silence and reports how long the
/// owner took to fail it.
async fn time_to_ack_timeout(session: &Session) -> Duration {
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let started = Instant::now();
    let error = camera
        .submit::<AppliedOnly, _>(&SilentOperation)
        .await
        .expect("submission")
        // Far past any acknowledgement deadline under test, so the observer
        // budget can never be what fired.
        .applied_with_timeout(Duration::from_secs(30))
        .await
        .expect_err("a silent camera cannot acknowledge");
    assert!(
        matches!(error, Error::Timeout),
        "expected the owner's own deadline, got {error:?}"
    );
    started.elapsed()
}

/// The headline restoration: widen the acknowledgement deadline on a live
/// session and the next submission waits the new, longer time. Narrowing back
/// is a real update too, not a one-way ratchet.
async fn a_reconfigured_ack_timeout_governs_the_next_submission<E: Executor>(executor: E) {
    let transport = SilentTransport::new();
    let writes = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");

    let before = time_to_ack_timeout(&session).await;
    assert!(
        before < WIDE_ACK_TIMEOUT / 2,
        "the profile's own {PROFILE_ACK_TIMEOUT:?} deadline should fire well \
         inside {WIDE_ACK_TIMEOUT:?}, took {before:?}"
    );

    session
        .set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT))
        .await
        .expect("widening an acknowledgement deadline is accepted");
    let widened = time_to_ack_timeout(&session).await;
    assert!(
        widened >= WIDE_ACK_TIMEOUT,
        "the submission after the update must wait the new {WIDE_ACK_TIMEOUT:?} \
         deadline, took {widened:?}"
    );

    session
        .set_tuning(OperationalTuning::new())
        .await
        .expect("returning to the profile defaults is accepted");
    let narrowed = time_to_ack_timeout(&session).await;
    assert!(
        narrowed < WIDE_ACK_TIMEOUT / 2,
        "clearing the override must return to the profile's {PROFILE_ACK_TIMEOUT:?} \
         deadline, took {narrowed:?}"
    );

    assert_eq!(
        writes.lock().expect("writes lock").len(),
        3,
        "no submission was retried, so each measures exactly one deadline"
    );
    session.shutdown().await.expect("owner shutdown");
}

/// The inquiry lane follows the same update, so this is session tuning rather
/// than a command-only knob. A camera clone taken before the update is used on
/// purpose: views read the owner's live tuning instead of a copy.
async fn a_widened_inquiry_timeout_governs_a_pre_existing_view<E: Executor>(executor: E) {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<SonyFR7>()
        .expect("camera view taken before the update");

    session
        .set_tuning(OperationalTuning::new().inquiry_timeout(Duration::from_secs(2)))
        .await
        .expect("widening an inquiry deadline is accepted");

    let started = Instant::now();
    let error = camera
        .inquire(&SilentInquiry)
        .await
        .expect_err("a silent camera cannot answer an inquiry");
    let elapsed = started.elapsed();
    assert!(
        matches!(error, Error::Timeout),
        "expected the owner's own deadline, got {error:?}"
    );
    assert!(
        elapsed >= Duration::from_secs(2),
        "the pre-existing view must prepare under the new deadline, took {elapsed:?}"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// An update taken while an operation is in flight applies to the next prepared
/// request and never re-times the handle already being awaited.
async fn an_update_mid_flight_leaves_the_live_operation_alone<E: Executor>(executor: E) {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let operation = camera
        .submit::<AppliedOnly, _>(&SilentOperation)
        .await
        .expect("submission");

    // The request is admitted under the profile's 500 ms deadline. Widening to
    // two seconds now must not extend it.
    session
        .set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT))
        .await
        .expect("an update mid-flight is accepted");

    let started = Instant::now();
    let error = operation
        .applied_with_timeout(Duration::from_secs(30))
        .await
        .expect_err("a silent camera cannot acknowledge");
    let elapsed = started.elapsed();
    assert!(
        matches!(error, Error::Timeout),
        "the in-flight handle must still resolve on its own deadline, got {error:?}"
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
    let after = time_to_ack_timeout(&session).await;
    assert!(
        after >= WIDE_ACK_TIMEOUT,
        "the request prepared after the update must use it, took {after:?}"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// The installed value reads back through every clone, and a rejected update
/// changes nothing.
async fn the_installed_tuning_reads_back_and_invalid_updates_are_rejected<E: Executor>(
    executor: E,
) {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let clone = session.clone();

    assert_eq!(
        session.tuning(),
        OperationalTuning::new(),
        "a session opened with no tuning reports none"
    );

    let accepted = OperationalTuning::new()
        .ack_timeout(WIDE_ACK_TIMEOUT)
        .quick_timeout(Duration::from_secs(9))
        .retry_limit(5);
    session.set_tuning(accepted).await.expect("accepted");
    assert_eq!(
        session.tuning(),
        accepted,
        "the getter reports what was set"
    );
    assert_eq!(
        clone.tuning(),
        accepted,
        "a clone shares the one owner, so it reports the same live value"
    );

    // Every rejection below is one `ProfileSpec::validate_tuning` rule, checked
    // against the same profile that would have rejected it at construction.
    let rejected = [
        OperationalTuning::new().ack_timeout(Duration::from_millis(1)),
        OperationalTuning::new().quick_timeout(Duration::ZERO),
        OperationalTuning::new().maximum_command_sockets(3),
        OperationalTuning::new().maximum_command_sockets(0),
        OperationalTuning::new().retry_limit(33),
        OperationalTuning::new().retry_timing(
            Duration::from_millis(50),
            Duration::from_millis(10),
            Duration::from_millis(10),
        ),
    ];
    for tuning in rejected {
        let error = session
            .set_tuning(tuning)
            .await
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

    session.shutdown().await.expect("owner shutdown");
}

/// `strict_unconfirmed_poison` selects the engine's raw-command recovery
/// policy while the session is built. Runtime reconfiguration must therefore
/// never make the readback claim a different policy: a default session cannot
/// turn strict at runtime, while a strict session retains its construction
/// policy when it replaces its mutable tuning or leaves it entirely unset.
async fn strict_recovery_policy_is_construction_only_and_remains_visible_in_readback<
    E: Executor,
>(
    executor: E,
) {
    let default_session = Session::open(SilentTransport::new(), session_config(), executor.clone())
        .await
        .expect("default owner session");
    let requested_strict = OperationalTuning::new().strict_unconfirmed_poison(true);
    let error = default_session
        .set_tuning(requested_strict)
        .await
        .expect_err("runtime tuning cannot enable construction-only strict recovery");
    assert!(matches!(error, Error::InvalidRequest(_)), "got {error:?}");
    assert_eq!(
        default_session.tuning(),
        OperationalTuning::new(),
        "a rejected default-to-strict update must not make readback claim strict recovery"
    );
    default_session.shutdown().await.expect("owner shutdown");

    let strict = OperationalTuning::new().strict_unconfirmed_poison(true);
    let strict_session = Session::open(
        SilentTransport::new(),
        session_config()
            .with_tuning(strict)
            .expect("strict tuning is valid"),
        executor,
    )
    .await
    .expect("strict owner session");

    let error = strict_session
        .set_tuning(OperationalTuning::new().strict_unconfirmed_poison(false))
        .await
        .expect_err("runtime tuning cannot disable construction-only strict recovery");
    assert!(matches!(error, Error::InvalidRequest(_)), "got {error:?}");
    assert_eq!(
        strict_session.tuning(),
        strict,
        "the rejected strict-to-default update leaves the strict readback intact"
    );

    let mutable_update = OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT);
    strict_session
        .set_tuning(mutable_update)
        .await
        .expect("runtime-mutable tuning is still replaceable on a strict session");
    assert_eq!(
        strict_session.tuning(),
        mutable_update.strict_unconfirmed_poison(true),
        "a mutable update preserves the effective construction-time strict policy"
    );

    strict_session
        .set_tuning(OperationalTuning::new())
        .await
        .expect("an unset strict field is not a policy change");
    assert_eq!(
        strict_session.tuning(),
        strict,
        "clearing mutable overrides must not make a strict session read back as default"
    );
    strict_session.shutdown().await.expect("owner shutdown");
}

/// Two handles reconfiguring at once: the boundary serializes them, so the
/// result is one of the two whole values and never a mixture of both.
async fn concurrent_updates_from_two_handles_are_last_writer_wins<E: Executor>(executor: E) {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let other = session.clone();

    // Two updates that differ in every field they set. A torn result would mix
    // one field from each; the boundary makes that unrepresentable.
    let first = OperationalTuning::new()
        .ack_timeout(Duration::from_millis(500))
        .quick_timeout(Duration::from_secs(9))
        .retry_limit(1);
    let second = OperationalTuning::new()
        .ack_timeout(Duration::from_millis(900))
        .quick_timeout(Duration::from_secs(11))
        .retry_limit(9);

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

    session.shutdown().await.expect("owner shutdown");
}

/// The single-camera session exposes the same capability without reaching for
/// its inner `Session`.
async fn a_camera_session_reconfigures_its_own_session<E: Executor>(executor: E) {
    let transport = SilentTransport::new();
    let config = CameraConfig::<SonyFR7>::new();
    let session = CameraSession::open(transport, &config, executor)
        .await
        .expect("single-camera session");

    let widened = session.tuning().ack_timeout(WIDE_ACK_TIMEOUT);
    session.set_tuning(widened).await.expect("accepted");
    assert_eq!(session.tuning(), widened);
    assert_eq!(
        session.session().tuning(),
        widened,
        "the owned session reports the same live value"
    );

    let started = Instant::now();
    let error = session
        .camera()
        .submit::<AppliedOnly, _>(&SilentOperation)
        .await
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(30))
        .await
        .expect_err("a silent camera cannot acknowledge");
    let elapsed = started.elapsed();
    assert!(matches!(error, Error::Timeout), "got {error:?}");
    assert!(
        elapsed >= WIDE_ACK_TIMEOUT,
        "the camera session's next submission must use the new deadline, \
         took {elapsed:?}"
    );

    session.close().await.expect("owner shutdown");
}

async fn run_matrix<E: Executor>(executor: E) {
    a_reconfigured_ack_timeout_governs_the_next_submission(executor.clone()).await;
    a_widened_inquiry_timeout_governs_a_pre_existing_view(executor.clone()).await;
    an_update_mid_flight_leaves_the_live_operation_alone(executor.clone()).await;
    the_installed_tuning_reads_back_and_invalid_updates_are_rejected(executor.clone()).await;
    strict_recovery_policy_is_construction_only_and_remains_visible_in_readback(executor.clone())
        .await;
    concurrent_updates_from_two_handles_are_last_writer_wins(executor.clone()).await;
    a_camera_session_reconfigures_its_own_session(executor).await;
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_owner_reconfigures_tuning_at_runtime() {
    let executor = grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
    run_matrix(executor).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_owner_reconfigures_tuning_at_runtime() {
    smol::block_on(run_matrix(grafton_visca::SmolRuntime::new()));
}
