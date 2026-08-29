//! Issue #631: runtime timeout/tuning reconfiguration through the blocking
//! facade.
//!
//! 1.2.0 let a caller widen `ack_timeout` on a congested venue network without
//! dropping the session (`Camera::set_timeout_config`, #575). 2.0 made tuning
//! construction-time only, so the same caller had to `close()` and reopen.
//! `Session::set_tuning` restores the capability; what these tests pin is the
//! exact scope it restores it with:
//!
//! * a request prepared *after* the update uses the new deadline;
//! * a request already admitted keeps the deadline it was admitted with;
//! * the update is validated on the same grounds construction validates on;
//! * the installed value is readable back through `Session::tuning`.
//!
//! The async twin is `tests/issue_631_runtime_tuning_async.rs`.

#![cfg(feature = "blocking")]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{CameraSession, Session, SessionConfig},
    camera::CameraConfig,
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::SonyFR7,
    request, AffectedAxes, CameraId, ControlClass, Error, Inquiry, InquiryRoute, OperationCommand,
    OperationalTuning, Request, ResponseDecoder, RetryClass, TimeoutClass,
};

use grafton_visca::transport::{
    BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
};

/// The Sony FR7 profile's own acknowledgement deadline.
const PROFILE_ACK_TIMEOUT: Duration = Duration::from_millis(200);
/// A deliberately wide acknowledgement deadline, five times the profile's.
const WIDE_ACK_TIMEOUT: Duration = Duration::from_secs(1);

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

/// A camera that accepts every frame and answers none of them.
///
/// Deliberate silence is what makes the deadline itself observable: nothing
/// else can end the request, so the wall-clock gap between submission and
/// failure *is* the deadline that fired.
#[derive(Debug)]
struct SilentTransport {
    config: TransportConfig,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    replies: VecDeque<Vec<u8>>,
}

impl SilentTransport {
    fn new() -> Self {
        Self {
            config: TransportConfig::default(),
            writes: Arc::new(Mutex::new(Vec::new())),
            replies: VecDeque::new(),
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

impl BlockingTransport for SilentTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
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
        let Some(bytes) = self.replies.pop_front() else {
            // A silent camera must not spin the owner's pump; a short pause is
            // what a real socket read would do while its deadline runs down.
            std::thread::sleep(timeout.min(Duration::from_millis(2)));
            return Err(Error::Timeout);
        };
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
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
fn time_to_ack_timeout(session: &Session) -> Duration {
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let started = Instant::now();
    let error = camera
        .submit::<AppliedOnly, _>(&SilentOperation)
        .expect("submission")
        // Far past any acknowledgement deadline under test, so the observer
        // budget can never be what fired.
        .applied_with_timeout(Duration::from_secs(30))
        .expect_err("a silent camera cannot acknowledge");
    assert!(
        matches!(error, Error::Timeout),
        "expected the owner's own deadline, got {error:?}"
    );
    started.elapsed()
}

/// The headline restoration: widen the acknowledgement deadline on a live
/// session and the next submission waits the new, longer time.
#[test]
fn a_widened_ack_timeout_governs_the_next_submission() {
    let transport = SilentTransport::new();
    let writes = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");

    let before = time_to_ack_timeout(&session);
    assert!(
        before < WIDE_ACK_TIMEOUT / 2,
        "the profile's own {PROFILE_ACK_TIMEOUT:?} deadline should fire well \
         inside {WIDE_ACK_TIMEOUT:?}, took {before:?}"
    );

    session
        .set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT))
        .expect("widening an acknowledgement deadline is accepted");

    let after = time_to_ack_timeout(&session);
    assert!(
        after >= WIDE_ACK_TIMEOUT,
        "the submission after the update must wait the new {WIDE_ACK_TIMEOUT:?} \
         deadline, took {after:?}"
    );

    assert_eq!(
        writes.lock().expect("writes lock").len(),
        2,
        "neither submission was retried, so each measures exactly one deadline"
    );
    session.shutdown().expect("owner shutdown");
}

/// The other direction: narrowing back is a real update, not a one-way ratchet.
///
/// An empty tuning is a complete replacement, so every deadline returns to its
/// profile default rather than keeping the widened value.
#[test]
fn a_narrowed_ack_timeout_governs_the_next_submission() {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");

    session
        .set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT))
        .expect("widening an acknowledgement deadline is accepted");
    let widened = time_to_ack_timeout(&session);
    assert!(
        widened >= WIDE_ACK_TIMEOUT,
        "the widened deadline must govern first, took {widened:?}"
    );

    session
        .set_tuning(OperationalTuning::new())
        .expect("returning to the profile defaults is accepted");
    let narrowed = time_to_ack_timeout(&session);
    assert!(
        narrowed < WIDE_ACK_TIMEOUT / 2,
        "clearing the override must return to the profile's {PROFILE_ACK_TIMEOUT:?} \
         deadline, took {narrowed:?}"
    );

    session.shutdown().expect("owner shutdown");
}

/// The inquiry lane follows the same update, so this is session tuning rather
/// than a command-only knob.
#[test]
fn a_widened_inquiry_timeout_governs_the_next_inquiry() {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    session
        .set_tuning(OperationalTuning::new().inquiry_timeout(Duration::from_secs(2)))
        .expect("widening an inquiry deadline is accepted");

    let started = Instant::now();
    let error = camera
        .inquire(&SilentInquiry)
        .expect_err("a silent camera cannot answer an inquiry");
    let elapsed = started.elapsed();
    assert!(
        matches!(error, Error::Timeout),
        "expected the owner's own deadline, got {error:?}"
    );
    assert!(
        elapsed >= Duration::from_secs(2),
        "the inquiry after the update must wait the new two-second deadline, \
         took {elapsed:?}"
    );

    session.shutdown().expect("owner shutdown");
}

/// A camera view taken *before* the update still prepares its next request
/// under the new tuning: the view reads the owner's live value rather than a
/// copy taken when it was created.
#[test]
fn a_view_taken_before_the_update_prepares_under_the_new_tuning() {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<SonyFR7>()
        .expect("camera view taken before the update");

    session
        .set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT))
        .expect("widening an acknowledgement deadline is accepted");

    let started = Instant::now();
    let error = camera
        .submit::<AppliedOnly, _>(&SilentOperation)
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(30))
        .expect_err("a silent camera cannot acknowledge");
    let elapsed = started.elapsed();
    assert!(matches!(error, Error::Timeout), "got {error:?}");
    assert!(
        elapsed >= WIDE_ACK_TIMEOUT,
        "the pre-existing view must pick up the new deadline, took {elapsed:?}"
    );

    session.shutdown().expect("owner shutdown");
}

/// The installed value is readable back, so a caller can confirm what the
/// session is running under without re-deriving it.
#[test]
fn the_installed_tuning_reads_back_through_the_session() {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");

    assert_eq!(
        session.tuning(),
        OperationalTuning::new(),
        "a session opened with no tuning reports none"
    );

    let tuning = OperationalTuning::new()
        .ack_timeout(WIDE_ACK_TIMEOUT)
        .quick_timeout(Duration::from_secs(9))
        .retry_limit(5);
    session.set_tuning(tuning).expect("accepted");
    assert_eq!(session.tuning(), tuning, "the getter reports what was set");

    session
        .set_tuning(OperationalTuning::new())
        .expect("accepted");
    assert_eq!(
        session.tuning(),
        OperationalTuning::new(),
        "the update replaces the previous value whole rather than merging"
    );

    session.shutdown().expect("owner shutdown");
}

/// Runtime updates are validated on exactly the grounds construction validates
/// on, and a rejected update leaves the live tuning untouched.
#[test]
fn an_invalid_update_is_rejected_and_changes_nothing() {
    let transport = SilentTransport::new();
    let session = Session::open(transport, session_config()).expect("owner session");

    let accepted = OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT);
    session.set_tuning(accepted).expect("accepted");

    // Every rejection below is one `ProfileSpec::validate_tuning` rule, and
    // each is checked against the same profile that would have rejected it in
    // `SessionConfig::with_tuning`.
    let rejected = [
        // Undercuts the profile's own acknowledgement deadline.
        OperationalTuning::new().ack_timeout(Duration::from_millis(1)),
        // A zero deadline is never a deadline.
        OperationalTuning::new().quick_timeout(Duration::ZERO),
        // Raises the profile's socket limit.
        OperationalTuning::new().maximum_command_sockets(3),
        // Zero sockets is not a capacity.
        OperationalTuning::new().maximum_command_sockets(0),
        // Beyond the bounded retry maximum.
        OperationalTuning::new().retry_limit(33),
        // Backoff that is not ordered, with a budget that cannot hold it.
        OperationalTuning::new().retry_timing(
            Duration::from_millis(50),
            Duration::from_millis(10),
            Duration::from_millis(10),
        ),
    ];
    for tuning in rejected {
        let error = session
            .set_tuning(tuning)
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

/// An update taken while a receipt is outstanding applies to the next prepared
/// request and never disturbs the receipt already being observed.
///
/// The blocking owner runs on the caller's thread, so this is the only ordering
/// an update can take: it occupies one owner turn between the turns that a
/// receipt's own wait drives. The receipt must resume on its unchanged
/// deadline, and the caller's next preparation must use the new one.
#[test]
fn an_update_while_a_receipt_is_outstanding_leaves_it_alone() {
    let transport = SilentTransport::new();
    let writes = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let operation = camera
        .submit::<AppliedOnly, _>(&SilentOperation)
        .expect("submission");
    assert_eq!(
        writes.lock().expect("writes lock").len(),
        1,
        "the frame is already on the wire, so its acknowledgement deadline is \
         already stamped as an absolute instant"
    );

    // The request is admitted under the profile's 200 ms deadline. Widening to
    // a full second now must not extend it.
    session
        .set_tuning(OperationalTuning::new().ack_timeout(WIDE_ACK_TIMEOUT))
        .expect("an update mid-flight is accepted");

    let started = Instant::now();
    let error = operation
        .applied_with_timeout(Duration::from_secs(30))
        .expect_err("a silent camera cannot acknowledge");
    let elapsed = started.elapsed();
    assert!(
        matches!(error, Error::Timeout),
        "the in-flight receipt must still resolve on its own deadline, got {error:?}"
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
    let after = time_to_ack_timeout(&session);
    assert!(
        after >= WIDE_ACK_TIMEOUT,
        "the request prepared after the update must use it, took {after:?}"
    );

    session.shutdown().expect("owner shutdown");
}

/// The single-camera session exposes the same capability without reaching for
/// its inner `Session`.
#[test]
fn a_camera_session_reconfigures_its_own_session() {
    let transport = SilentTransport::new();
    let config = CameraConfig::<SonyFR7>::new();
    let session = CameraSession::open(transport, &config).expect("single-camera session");

    let opened = session.tuning();
    let widened = opened.ack_timeout(WIDE_ACK_TIMEOUT);
    session.set_tuning(widened).expect("accepted");
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
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(30))
        .expect_err("a silent camera cannot acknowledge");
    let elapsed = started.elapsed();
    assert!(matches!(error, Error::Timeout), "got {error:?}");
    assert!(
        elapsed >= WIDE_ACK_TIMEOUT,
        "the camera session's next submission must use the new deadline, \
         took {elapsed:?}"
    );

    session.close().expect("owner shutdown");
}
