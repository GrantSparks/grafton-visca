//! Blocking owner facade.
//!
//! A [`Session`] owns exactly one native owner worker thread and the transport
//! moved into it (D24). [`Camera`] values are inexpensive target views:
//! cloning one only clones the owner handle and its immutable target/profile
//! facts. Session and camera handles are `Clone + Send + Sync`; operation
//! handles are `Send + Sync` and may move between threads. Calls from any
//! number of threads are serialized by the worker rather than by the caller.
//! These
//! handles are deliberately mode-native: they expose direct [`Result`]s and
//! never create or return a future. Dropping a non-final handle leaves the
//! shared owner running; dropping the final one stops the worker and releases
//! its transport, but never emits a protocol STOP. [`Session::close`] is the
//! deterministic release barrier.

use std::{fmt, marker::PhantomData, sync::Arc, time::Duration};

use crate::{
    completion,
    prepared::ClassSelection,
    profiles::{AnyProfile, CameraProfile, CompileTimeProfile},
    runtime::owner::{
        BlockingOperationReceipt, BlockingOwnerHandle, BlockingTransportAdapter, Clock,
    },
    target::TargetSlot,
    CancellationOutcome, Error, Inquiry, MetricsSnapshot, OperationCommand, OperationalTuning,
    PlainCommand, Result, SubmissionClass,
};

use crate::OperationId;

crate::operation_handle::operation_handle! {
    receipt: BlockingOperationReceipt,
    async: [],
    await: [],
    description: "A blocking handle on one admitted operation.",
    wait_doc: "Waits block the calling thread while the owner worker continues processing other requests.",
    must_use: "observe, cancel, or explicitly detach this operation",
}

use crate::{camera::CameraConfig, SessionConfig};

/// A bounded blocking diagnostic stream from one session.
///
/// Dropping this stream releases its subscription without keeping the owner
/// alive. Queued events remain readable before the owner's original terminal cause.
pub struct Diagnostics {
    inner: crate::observability::DiagnosticSubscription,
}

impl Diagnostics {
    fn from_owner(inner: crate::runtime::owner::DiagnosticSubscription) -> Self {
        Self {
            inner: crate::observability::DiagnosticSubscription::from_owner(inner),
        }
    }

    /// Receives one queued event without waiting; an empty or closed queue returns None.
    #[must_use]
    pub fn try_recv(&self) -> Option<crate::observability::DiagnosticEvent> {
        self.inner.try_recv()
    }

    /// Waits for an event, preserving the owner's terminal cause after the queue drains.
    /// Returns [`Error::InvalidState`] when called from this session's owner worker.
    pub fn recv(&self) -> Result<crate::observability::DiagnosticEvent> {
        self.inner.recv_blocking()
    }

    /// Waits at most `timeout` on the owner's clock for an event.
    ///
    /// Returns `Ok(None)` when the queue remains empty, or the owner's
    /// original terminal cause once every queued event has been received.
    /// A non-zero timeout returns [`Error::InvalidState`] when called from this
    /// session's owner worker.
    pub fn recv_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<crate::observability::DiagnosticEvent>> {
        self.inner.recv_timeout(timeout)
    }
}

impl fmt::Debug for Diagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Diagnostics")
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Connects the configured standard transport and starts one owner session.
    ///
    /// Profile, policy, transport and endpoint validation complete before DNS,
    /// device opening or owner startup. A serial session may register several targets.
    pub fn connect(config: &SessionConfig) -> Result<Self> {
        crate::camera::construction::connect_blocking(config.connection_plan()?)
    }

    /// Opens a blocking session over a configured transport and profile.
    ///
    /// Profile compatibility is checked against the actual opened transport.
    /// The standard connection selection stored in `config` is not used here;
    /// [`CameraSession::open`] additionally validates its camera configuration's selection.
    ///
    /// Startup validates the profiles, tuning, transport buffers, and the
    /// immutable owner policy, and sends the optional Sony sequence RESET on
    /// the calling thread, before it starts exactly one worker thread. Any
    /// failure, including a failed thread spawn, drops the transport and
    /// returns the error; no worker is left running.
    pub fn open<T>(transport: T, config: &SessionConfig) -> Result<Self, Error>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        Self::open_validated(transport, config.clone().validated()?)
    }

    /// [`Self::open`] for a configuration whose session policy is already
    /// validated, so each open validates it exactly once.
    pub(crate) fn open_validated<T>(
        transport: T,
        config: crate::session_config::ValidatedSessionConfig,
    ) -> Result<Self, Error>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        Self::open_on(transport, config, Clock::System)
    }

    /// [`Self::open_validated`] with the owner keeping time on `clock`.
    pub(crate) fn open_on<T>(
        transport: T,
        config: crate::session_config::ValidatedSessionConfig,
        clock: Clock,
    ) -> Result<Self, Error>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        config.validate_opened_transport(&transport)?;
        let profiles = config.profile_registry();
        let (mut adapter, policy) = BlockingTransportAdapter::new_with_targets(
            transport,
            &profiles,
            config.validated_tuning(),
            config.admission_capacity(),
            config.strict_unconfirmed_poison(),
        )?;
        if config.sony_sequence_reset_on_connect() {
            adapter.send_sony_sequence_reset(policy.write_timeout.get())?;
        }
        Ok(Self {
            owner: BlockingOwnerHandle::spawn(policy, adapter, clock)?,
            config: Arc::new(config),
        })
    }

    #[doc = crate::camera_facade::session_contract!(close)]
    /// A custom transport calling this from its own worker requests shutdown
    /// but receives [`Error::InvalidState`] immediately: it cannot wait for
    /// its own teardown. A caller on another thread can still close and join.
    /// Other calls that would wait for this worker also fail immediately with
    /// [`Error::InvalidState`], without enqueuing work.
    pub fn close(self) -> Result<(), Error> {
        self.owner.close()
    }

    #[doc = crate::camera_facade::session_contract!(diagnostics)]
    pub fn subscribe_diagnostics(&self, capacity: usize) -> Result<Diagnostics, Error> {
        self.owner
            .subscribe_diagnostics(capacity)
            .map(Diagnostics::from_owner)
    }
}

impl<P: CompileTimeProfile> CameraSession<P> {
    /// Connects a typed camera using its documented TCP port for host-only addresses.
    pub fn connect_tcp(address: impl Into<String>) -> Result<Self>
    where
        P: crate::capabilities::SupportsTcp,
    {
        Self::connect(&CameraConfig::<P>::tcp(address))
    }

    /// Connects a typed camera using its documented UDP port for host-only addresses.
    pub fn connect_udp(address: impl Into<String>) -> Result<Self>
    where
        P: crate::capabilities::SupportsUdp,
    {
        Self::connect(&CameraConfig::<P>::udp(address))
    }

    /// Connects one typed serial camera; opening writes only the configured startup.
    #[cfg(feature = "transport-serial")]
    pub fn connect_serial(port: impl Into<String>, baud_rate: u32) -> Result<Self>
    where
        P: crate::capabilities::SupportsSerial,
    {
        Self::connect(&CameraConfig::<P>::serial(port, baud_rate))
    }
}

crate::camera_core::camera_core! {
    core: BlockingCameraCore,
    owner: BlockingOwnerHandle,
    async: [],
    await: [],
}

crate::camera_facade::camera_facade! {
    core: BlockingCameraCore,
    owner: BlockingOwnerHandle,
    async: [],
    await: [],
    error: [, Error],
    session_mode_doc: "Construction moves the transport into one native worker thread. A blocking wait does not block another thread’s submit, cancel, or STOP; `close` joins the worker after transport teardown.",
    camera_session_constructors: {
        /// Connects a typed or runtime-selected camera configuration to one owner.
        ///
        /// TCP, UDP and serial return the same single-camera session shape.
        pub fn connect(config: &CameraConfig<P>) -> Result<Self> {
            let plan = config.connection_plan()?;
            let session = crate::camera::construction::connect_blocking(plan.connection)?;
            Ok(Self::from_registration(session, plan.registration))
        }

        /// Opens one single-camera owner session over a caller-owned transport.
        ///
        /// Both the transport selected in `config` and the actual opened transport
        /// must support the profile, as for [`CameraConfig::validate`](crate::config::CameraConfig::validate).
        /// Use `TransportOptions::Custom` in the configuration when only the opened
        /// transport should select the transport kind.
        ///
        /// This is the [`Session::open`] counterpart for callers that already own
        /// a transport. The profile comes from `config`, so the camera this
        /// session hands out is bound to the same `P` the configuration was
        /// written for.
        pub fn open<T>(transport: T, config: &CameraConfig<P>) -> Result<Self, Error>
        where
            T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
        {
            let (session_config, registration) = config.validated_camera_config()?;
            let session = Session::open_validated(transport, session_config)?;
            Ok(Self::from_registration(session, registration))
        }
    },
}

#[path = "blocking_nouns.rs"]
mod blocking_nouns;

use crate::noun_table::reexport_nouns;
crate::noun_table::noun_table!(reexport_nouns, [blocking_nouns]);
crate::noun_table::motion_table!(reexport_nouns, [blocking_nouns]);

#[cfg(all(test, feature = "test-utils"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::{
        capabilities::Capabilities,
        profiles::{ProfileEnvelope, ProfileTiming, TransportCompatibility},
        testing::testkit::{helpers, ScriptedBlockingTransport, Step},
        transport::{BlockingTransport, HasTransportConfig},
        CommandTimeouts, ProfileSpec,
    };

    const CUSTOM_IP_STREAM: crate::testing::testkit::ScriptedTransportIdentity =
        crate::testing::testkit::ScriptedTransportIdentity::Custom {
            semantics: crate::transport::SendSemantics::Stream,
            addressing: crate::transport::AddressingMode::Ip,
        };

    struct FailFirstZoomStopSend {
        inner: ScriptedBlockingTransport,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        failed: bool,
    }

    impl FailFirstZoomStopSend {
        fn new(steps: impl Into<Vec<Step>>) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    inner: ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, steps),
                    writes: Arc::clone(&writes),
                    failed: false,
                },
                writes,
            )
        }
    }

    impl HasTransportConfig for FailFirstZoomStopSend {
        fn identity(&self) -> crate::transport::TransportIdentity<'_> {
            crate::transport::TransportIdentity::Custom {
                semantics: crate::transport::SendSemantics::Datagram,
                addressing: crate::transport::AddressingMode::Ip,
            }
        }

        fn transport_config(&self) -> &crate::transport::ValidatedTransportConfig {
            self.inner.transport_config()
        }
    }

    impl BlockingTransport for FailFirstZoomStopSend {
        fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            if !self.failed && bytes.starts_with(&[0x81, 0x01, 0x04, 0x07]) {
                self.failed = true;
                return Err(Error::transport_error(std::io::Error::other(
                    "injected zoom stop send failure",
                )));
            }
            self.inner.send_with_timeout(bytes, timeout)
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            timeout: Duration,
        ) -> Result<crate::transport::ReceiveOutcome, Error> {
            self.inner.recv_into_with_timeout(dst, timeout)
        }
    }

    fn partial_motion_profile(has_zoom: bool, has_focus: bool) -> ProfileSpec {
        let mut capabilities =
            Capabilities::runtime_baseline("Partial motion test camera", 1).expect("baseline");
        let domains = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("valid built-in profile")
            .capabilities()
            .clone();
        capabilities.zoom = has_zoom.then(|| domains.zoom.expect("generic zoom domain"));
        capabilities.focus = has_focus.then(|| domains.focus.expect("generic focus domain"));

        ProfileSpec::builder(capabilities)
            .transports(TransportCompatibility::new().with_tcp(5678))
            .envelope(ProfileEnvelope::RawVisca)
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(CommandTimeouts::default())
                    .inquiry_timeout(Duration::from_secs(1))
                    .cancellation_timeout(Duration::from_secs(1))
                    .ambiguity_timeout(Duration::from_secs(1))
                    .busy_timeout(Duration::ZERO)
                    .raw_inquiry_reply_skew(Duration::ZERO)
                    .inquiry_spacing(Duration::ZERO)
                    .command_spacing(Duration::ZERO)
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(crate::types::CommandSocketCount::ONE)
            .supports_operation_complete(true)
            .supports_command_cancel(false)
            .position_inquiries(None)
            .build()
            .expect("valid partial motion profile")
    }

    fn partial_motion_core(session: &Session) -> BlockingCameraCore {
        let profile = session
            .config
            .registered_target(TargetSlot::CAMERA_1)
            .expect("registered partial motion profile");
        session.core(profile)
    }

    #[test]
    fn selected_noun_calls_validate_their_value_once() {
        use crate::{
            request::builtin::{
                request_validation_count, request_value_validation_count,
                reset_request_validation_count,
            },
            types::{
                Kelvin, PanTiltDirection, PanTiltLimitCorner, PanTiltPosition, PresetNumber,
                PresetSpeed, SpeedLevel, UnitInterval, ZoomDomain,
            },
        };

        for profile in [
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap(),
            ProfileSpec::from_compile_time::<crate::profiles::SonyFr7>().unwrap(),
        ] {
            let supports_temperature = profile
                .capabilities()
                .supports(crate::capabilities::TypedSupportSurface::ColorTemperature);
            let step = Step::every_send(Arc::new(|frame| {
                let (sequence, payload) = crate::testing::frames::sony_split(frame)
                    .map_or((None, frame), |(sequence, payload)| {
                        (Some(sequence), payload)
                    });
                let replies = if payload == [0x81, 0x09, 0x04, 0x47, 0xff] {
                    vec![helpers::inquiry_reply(&[0, 0, 0, 0])]
                } else {
                    vec![helpers::ack(1), helpers::complete(1)]
                };
                if let Some(sequence) = sequence {
                    replies
                        .iter()
                        .map(|reply| helpers::sony_reply(sequence, reply))
                        .collect()
                } else {
                    replies
                }
            }));
            let transport = ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, [step]);
            let session = Session::open(transport, &SessionConfig::new(profile)).unwrap();
            let camera = session
                .camera::<AnyProfile>(crate::IndividualCameraId::CAMERA_1)
                .unwrap();
            macro_rules! once {
                ($method:literal, $call:expr) => {{
                    reset_request_validation_count();
                    let result = $call;
                    assert!(result.is_ok(), "{}: {:?}", $method, result.err());
                    assert_eq!(request_validation_count(), 1, "{}", $method);
                    assert_eq!(request_value_validation_count(), 1, "{}", $method);
                }};
            }
            let speed = SpeedLevel::Medium;
            let origin = PanTiltPosition::from_degrees(0.0, 0.0).unwrap();
            let corner = PanTiltLimitCorner::UpRight;

            once!(
                "tele_variable",
                camera
                    .zoom()
                    .tele_variable(speed)
                    .and_then(|mut op| op.applied())
            );
            once!(
                "wide_variable",
                camera
                    .zoom()
                    .wide_variable(speed)
                    .and_then(|mut op| op.applied())
            );
            once!(
                "far_variable",
                camera
                    .focus()
                    .far_variable(speed)
                    .and_then(|mut op| op.applied())
            );
            once!(
                "near_variable",
                camera
                    .focus()
                    .near_variable(speed)
                    .and_then(|mut op| op.applied())
            );
            once!(
                "drive",
                camera
                    .pan_tilt()
                    .drive(PanTiltDirection::Right, speed)
                    .and_then(|mut op| op.applied())
            );
            once!(
                "stop",
                camera.pan_tilt().stop().and_then(|mut op| op.applied())
            );
            once!(
                "absolute",
                camera
                    .pan_tilt()
                    .absolute(origin, speed)
                    .and_then(|mut op| op.applied())
            );
            once!(
                "relative",
                camera
                    .pan_tilt()
                    .relative(origin, speed)
                    .and_then(|mut op| op.applied())
            );
            once!("set_limit", camera.pan_tilt().set_limit(corner, origin));
            once!("clear_limit", camera.pan_tilt().clear_limit(corner));
            once!(
                "set_recall_speed",
                camera
                    .presets()
                    .set_recall_speed(PresetSpeed::new(12).unwrap())
            );

            once!(
                "recall",
                camera
                    .presets()
                    .recall(PresetNumber::new(1).unwrap())
                    .and_then(|mut op| op.applied())
            );
            once!(
                "set_normalized",
                camera
                    .zoom()
                    .set_normalized(UnitInterval::MIN, ZoomDomain::Optical)
                    .and_then(|mut op| op.applied())
            );
            once!("normalized", camera.zoom().normalized(ZoomDomain::Optical));
            if supports_temperature {
                once!(
                    "set_color_temperature",
                    camera
                        .white_balance()
                        .set_color_temperature(Kelvin::new(3200).unwrap())
                );
            }

            reset_request_validation_count();
            camera.white_balance().one_push().unwrap();
            assert_eq!(request_validation_count(), 1, "one_push");
            session.shutdown().unwrap();
        }
    }

    /// Two views taken from the same session drive its one owned transport,
    /// and an operation submitted through either of them is applied rather
    /// than merely being constructed.
    #[test]
    fn camera_views_share_the_session_owner() {
        use crate::testing::testkit::{helpers, ScriptedBlockingTransport};

        let transport = ScriptedBlockingTransport::new(
            CUSTOM_IP_STREAM,
            [helpers::auto_respond_step(), helpers::auto_respond_step()],
        );
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap();
        let session = Session::open(transport, &config).unwrap();

        let first = session
            .camera::<crate::profiles::PtzOpticsG2>(crate::IndividualCameraId::CAMERA_1)
            .unwrap();
        let second = session
            .camera::<crate::profiles::PtzOpticsG2>(crate::IndividualCameraId::CAMERA_1)
            .unwrap();
        assert_eq!(first.target(), second.target());

        first.zoom().stop().unwrap().applied().unwrap();
        second.zoom().stop().unwrap().applied().unwrap();

        let sent = probe.sent();
        assert_eq!(
            sent,
            vec![
                vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff],
                vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff],
            ],
            "both views wrote their zoom stop through the one owned transport"
        );
        session.shutdown().unwrap();
    }

    /// Issue #805: an operation's `Debug` output is its identity only, the
    /// same on both facades (the async facade used to print its receipt).
    #[test]
    fn operation_debug_shows_only_its_identity() {
        let transport =
            ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, [helpers::auto_respond_step()]);
        let session = Session::open(
            transport,
            &SessionConfig::new(partial_motion_profile(true, false)),
        )
        .expect("zoom session");
        let mut operation = partial_motion_core(&session)
            .submit::<completion::AppliedOnly, _>(&crate::request::builtin::ZoomStop)
            .expect("admitted");
        assert_eq!(
            format!("{operation:?}"),
            format!("Operation {{ id: {:?} }}", operation.id())
        );
        operation.applied().expect("applied");
        session.shutdown().expect("session shutdown");
    }

    /// A session's `Debug` output lists exactly its registered targets in
    /// camera-number order, not every addressable slot.
    #[test]
    fn session_debug_lists_only_registered_targets() {
        let session = Session::open(
            ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, []),
            &SessionConfig::for_target(
                crate::IndividualCameraId::CAMERA_3,
                ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
                    .expect("generic profile"),
            ),
        )
        .expect("single-target session");
        assert_eq!(
            format!("{session:?}"),
            format!(
                "Session {{ targets: {:?}, .. }}",
                [crate::IndividualCameraId::CAMERA_3]
            )
        );
        session.shutdown().expect("session shutdown");
    }

    /// Issue #805: tuning is validated once, when the session opens, before
    /// any write: an open that would send the Sony RESET writes nothing.
    #[test]
    fn session_open_rejects_invalid_tuning_before_any_write() {
        let transport = ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, []);
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true)
            .with_tuning(OperationalTuning::new().with_ack_timeout(Duration::ZERO));
        assert!(matches!(
            Session::open(transport, &config),
            Err(Error::InvalidRequest { .. } | Error::InvalidParameter { .. })
        ));
        assert!(probe.sent().is_empty());
    }

    #[test]
    fn session_open_sends_opt_in_sony_sequence_reset_before_owner_work() {
        let transport = ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, []);
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);

        let session = Session::open(transport, &config).expect("Sony session opens after RESET");

        assert_eq!(
            probe.sent(),
            vec![vec![0x02, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0x01]]
        );
        session.shutdown().expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_on_a_zoom_only_runtime_profile_writes_only_zoom_stop() {
        let transport =
            ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, [helpers::auto_respond_step()]);
        let probe = transport.clone();
        let session = Session::open(
            transport,
            &SessionConfig::new(partial_motion_profile(true, false)),
        )
        .expect("zoom-only session");

        partial_motion_core(&session)
            .stop_all_motion()
            .expect("zoom-only stop succeeds")
            .into_result()
            .expect("supported STOP applied");

        assert_eq!(
            probe.sent(),
            vec![vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff]],
            "unsupported pan/tilt and focus stops must never reach the wire"
        );
        session.shutdown().expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_on_a_profile_without_motion_axes_is_a_successful_no_op() {
        let transport = ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, []);
        let probe = transport.clone();
        let session = Session::open(
            transport,
            &SessionConfig::new(partial_motion_profile(false, false)),
        )
        .expect("no-axis session");

        partial_motion_core(&session)
            .stop_all_motion()
            .expect("no-axis stop is a successful no-op")
            .into_result()
            .expect("no supported axes");

        assert!(
            probe.sent().is_empty(),
            "a profile without motion axes must not emit a stop frame"
        );
        session.shutdown().expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_reports_each_supported_axis_after_a_partial_failure() {
        let (transport, writes) = FailFirstZoomStopSend::new([helpers::auto_respond_step()]);
        let session = Session::open(
            transport,
            &SessionConfig::new(partial_motion_profile(true, true)),
        )
        .expect("zoom-focus session");

        let report = partial_motion_core(&session)
            .stop_all_motion()
            .expect("halt was accepted");
        assert!(matches!(report.pan_tilt, crate::HaltOutcome::Unsupported));
        assert!(matches!(
            report.zoom,
            crate::HaltOutcome::Failed(Error::UnsequencedCommandUnconfirmed)
        ));
        assert!(matches!(report.focus, crate::HaltOutcome::Applied));
        let failure = report
            .clone()
            .into_result()
            .expect_err("zoom STOP may have applied");
        assert_eq!(failure.recovery(), crate::Recovery::Reconcile);
        assert_eq!(
            failure.failure_context(),
            Some(crate::FailureContext::new(
                crate::FailureStage::Terminal,
                crate::Certainty::Unconfirmed,
            ))
        );
        assert_eq!(
            writes.lock().expect("writes lock").clone(),
            vec![
                vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff],
                vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xff],
            ],
            "the focus stop must follow the failed zoom stop, while unsupported pan/tilt stays absent"
        );
        session.shutdown().expect("session shutdown");
    }
    #[test]
    fn profile_erasure_and_typed_conversion_preserve_one_owner_and_view_settings() {
        use crate::{
            profiles::{PtzOpticsG2, SonyFr7},
            testing::testkit::{helpers, ScriptedBlockingTransport},
        };
        let transport =
            ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, [helpers::auto_respond_step()]);
        let probe = transport.clone();
        let config =
            SessionConfig::from_compile_time::<PtzOpticsG2>().expect("typed configuration");
        let session = Session::open(transport, &config).expect("owner session");
        let selected = session
            .camera::<PtzOpticsG2>(crate::IndividualCameraId::CAMERA_1)
            .expect("typed camera")
            .with_submission_class(SubmissionClass::Background);
        let erased: Camera = selected.into_any();
        assert_eq!(erased.submission_class(), Some(SubmissionClass::Background));
        assert!(erased.clone().into_typed::<SonyFr7>().is_err());
        assert!(matches!(
            erased.nd_filter().set_auto(true),
            Err(Error::FeatureNotSupported { .. })
        ));
        assert!(
            probe.sent().is_empty(),
            "conversion and unsupported gates perform no I/O"
        );
        let typed = erased
            .into_typed::<PtzOpticsG2>()
            .expect("matching complete profile");
        assert_eq!(typed.target(), crate::IndividualCameraId::CAMERA_1);
        assert_eq!(typed.submission_class(), Some(SubmissionClass::Background));
        let mut single = CameraSession::<PtzOpticsG2>::from_registration(
            session.clone(),
            session
                .config
                .registered_target(TargetSlot::CAMERA_1)
                .expect("registered target"),
        );
        single.set_submission_class(Some(SubmissionClass::User));
        let single: CameraSession = single.into_any();
        let single = single
            .into_typed::<PtzOpticsG2>()
            .expect("typed session conversion");
        assert_eq!(single.submission_class(), Some(SubmissionClass::User));
        typed
            .zoom()
            .stop()
            .expect("stop admission")
            .applied()
            .expect("applied stop");
        assert_eq!(probe.sent(), [vec![0x81, 1, 4, 7, 0, 0xff]]);
        session.close().expect("one owner closes");
    }
    #[test]
    fn diagnostics_wait_on_the_manual_clock_and_preserve_shutdown_after_queued_events() {
        use crate::{
            profiles::GenericVisca,
            testing::testkit::{helpers, ManualClock, ScriptedBlockingTransport},
        };
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Diagnostics>();
        let clock = ManualClock::new();
        let participant = clock.enter();
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("configuration");
        let session = clock
            .open_blocking_session(
                &participant,
                ScriptedBlockingTransport::new(CUSTOM_IP_STREAM, [helpers::auto_respond_step()]),
                config,
            )
            .expect("manual-clock session");
        let diagnostics = session.subscribe_diagnostics(128).expect("subscription");
        let start = clock.now();
        assert!(diagnostics
            .recv_with_timeout(Duration::from_secs(3))
            .expect("empty timed observation")
            .is_none());
        assert_eq!(clock.now() - start, Duration::from_secs(3));
        let sender = session.clone();
        let work = clock.spawn(move || {
            sender
                .camera::<GenericVisca>(crate::IndividualCameraId::CAMERA_1)
                .expect("camera")
                .zoom()
                .stop()
                .expect("stop")
                .applied()
                .expect("applied stop");
        });
        let _event = diagnostics
            .recv()
            .expect("unbounded receive waits for the worker");
        work.join().expect("command worker");
        session
            .close()
            .expect("owner closes while subscription exists");
        // Ready events win even when the observation budget is zero.
        assert!(diagnostics
            .recv_with_timeout(Duration::ZERO)
            .expect("queued event")
            .is_some());
        while diagnostics.try_recv().is_some() {}
        assert!(matches!(diagnostics.recv(), Err(Error::RuntimeShutdown)));
        assert!(matches!(
            diagnostics.recv_with_timeout(Duration::ZERO),
            Err(Error::RuntimeShutdown)
        ));
    }

    #[test]
    fn diagnostics_keep_the_transport_terminal_cause_after_close() {
        use crate::{
            profiles::GenericVisca,
            testing::testkit::{helpers, ScriptedBlockingTransport, Step},
            types::FocusMode,
        };
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("configuration");
        let transport = ScriptedBlockingTransport::new(
            CUSTOM_IP_STREAM,
            [
                Step::on_send(None, vec![]),
                helpers::errors::connection_lost(),
            ],
        );
        let session = Session::open(transport, &config).expect("session");
        let diagnostics = session.subscribe_diagnostics(128).expect("subscription");
        assert!(session
            .camera::<GenericVisca>(crate::IndividualCameraId::CAMERA_1)
            .expect("camera")
            .focus()
            .set_mode(FocusMode::Manual)
            .is_err());
        let terminal = session.close().expect_err("transport terminal cause");
        while diagnostics.try_recv().is_some() {}
        let observed = diagnostics.recv().expect_err("diagnostic terminal cause");
        assert_eq!(observed.kind(), terminal.kind());
        assert_eq!(observed.to_string(), terminal.to_string());
    }
}
