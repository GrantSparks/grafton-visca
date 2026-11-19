//! Final, profile-generic async owner facade.
//!
//! This module is intentionally small. A [`Session`] owns exactly one
//! serialized [`AsyncOwnerActor`](crate::runtime::owner::AsyncOwnerActor) and
//! the transport moved into that actor. [`Camera`] values are inexpensive
//! target views: cloning one only clones the owner handle and its immutable
//! target/profile facts. Dropping a non-final handle leaves that shared owner
//! running; dropping the final owner handle may release its actor and transport,
//! but never emits a protocol STOP. [`Session::close`] is the deterministic
//! release barrier.

#![cfg(feature = "async")]

use crate::observability::DiagnosticCapacity;
use std::{fmt, marker::PhantomData, sync::Arc};

use crate::{
    completion,
    config::OperationalTuning,
    executor::Executor,
    prepared::ClassSelection,
    profiles::{AnyProfile, CameraProfile, CompileTimeProfile},
    runtime::{
        owner::AsyncTransportAdapter,
        owner::{AsyncOwnerActor, AsyncOwnerHandle},
    },
    target::TargetSlot,
    transport::{AsyncTransport, HasTransportConfig},
    Inquiry, MetricsSnapshot, OperationCommand, PlainCommand, Result, SessionConfig,
    SubmissionClass,
};

pub use crate::async_operation::Operation;

/// A bounded asynchronous diagnostic stream from one session.
///
/// Dropping this stream releases its subscription without keeping the owner
/// alive. Queued events remain readable before the owner's original terminal cause.
pub struct Diagnostics {
    inner: crate::runtime::owner::DiagnosticSubscription,
    wait: crate::observability::DiagnosticWait,
}

impl Diagnostics {
    fn from_owner(
        inner: crate::runtime::owner::DiagnosticSubscription,
        clock: crate::runtime::owner::BoundClock,
    ) -> Self {
        Self {
            inner,
            wait: crate::observability::DiagnosticWait::new(clock),
        }
    }

    /// Receives one queued event without waiting; an empty or closed queue returns None.
    #[must_use]
    pub fn try_recv(&self) -> Option<crate::observability::DiagnosticEvent> {
        self.inner.try_recv()
    }

    /// Waits for an event, preserving the owner's terminal cause after the queue drains.
    pub async fn recv(&self) -> Result<crate::observability::DiagnosticEvent> {
        self.inner.recv_async().await
    }

    /// Waits at most `timeout` on the session's selected runtime clock.
    ///
    /// Returns `Ok(None)` when the queue remains empty. Events and terminal
    /// causes ready at the deadline take precedence. Cancelling this future
    /// leaves unread events queued and does not end the subscription.
    pub async fn recv_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<crate::observability::DiagnosticEvent>> {
        self.wait.recv_with_timeout(&self.inner, timeout).await
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
    pub async fn connect(
        config: &SessionConfig,
        runtime: impl crate::runtime::Runtime,
    ) -> Result<Self> {
        crate::camera::construction::connect_async(config.connection_plan()?, runtime).await
    }

    /// Starts one owner actor over a caller-owned async transport.
    ///
    /// Profile compatibility is checked against the actual opened transport.
    /// The standard connection selection stored in `config` is not used here;
    /// [`CameraSession::open`] additionally validates its camera configuration's selection.
    ///
    /// Startup performs profile, tuning, transport-buffer, and immutable owner
    /// policy validation before spawning exactly one actor task. The future
    /// returns after the actor is detached; request methods return after their
    /// own admission or terminal observation as documented below. If the executor
    /// drops the actor during spawning, opening returns its terminal error and
    /// drops the transport.
    pub async fn open(
        transport: impl AsyncTransport + HasTransportConfig + 'static,
        config: &SessionConfig,
        executor: impl Executor,
    ) -> Result<Self> {
        Self::open_validated(transport, config.clone().validated()?, executor).await
    }

    /// [`Self::open`] for a configuration whose session policy is already
    /// validated, so each open validates it exactly once.
    // The explicit bound avoids rustc issue #100013 across downstream spawn boundaries.
    #[allow(clippy::manual_async_fn)]
    pub(crate) fn open_validated<E, T>(
        transport: T,
        config: crate::session_config::ValidatedSessionConfig,
        executor: E,
    ) -> impl std::future::Future<Output = Result<Self>> + Send
    where
        E: Executor,
        T: AsyncTransport + HasTransportConfig + 'static,
    {
        async move {
            let routing = config.validate_opened_transport(&transport)?;
            let tuning = config.validated_tuning();
            let profiles = config.profile_registry();
            let (mut adapter, policy) = AsyncTransportAdapter::new_with_routing(
                transport,
                &profiles,
                routing,
                tuning,
                config.admission_capacity(),
                config.strict_unconfirmed_poison(),
            );

            if config.sony_sequence_reset_on_connect() {
                crate::timeout::within(
                    adapter.send_sony_sequence_reset(),
                    executor.sleep(policy.write_timeout.get()),
                )
                .await
                .ok_or_else(crate::Error::io_timeout)??;
            }
            let (owner, actor) = AsyncOwnerActor::new(policy, executor.clone())?;
            // This is the only task spawned by this facade. The owner handle is
            // cloneable; all public camera views share its one serialized actor.
            executor.spawn_detached(async move {
                actor.run(adapter).await;
            });
            if let Some(error) = owner.terminal_error() {
                return Err(error);
            }
            Ok(Self {
                owner,
                config: Arc::new(config),
            })
        }
    }

    #[doc = crate::camera_facade::session_contract!(close)]
    /// This awaits release of the detached actor's driver and transport. It
    /// does not promise an executor-specific task join after that release point.
    pub async fn close(self) -> Result<()> {
        let _ = self.shutdown();
        self.owner.wait_closed().await
    }

    #[doc = crate::camera_facade::session_contract!(diagnostics)]
    pub async fn subscribe_diagnostics(&self, capacity: DiagnosticCapacity) -> Result<Diagnostics> {
        let subscription = self.owner.subscribe_diagnostics(capacity).await?;
        Ok(Diagnostics::from_owner(
            subscription,
            self.owner.diagnostic_clock(),
        ))
    }
}

impl<P: CompileTimeProfile> CameraSession<P> {
    /// Connects a typed camera using its documented TCP port for host-only addresses.
    pub async fn connect_tcp(
        address: impl Into<String>,
        runtime: impl crate::runtime::Runtime,
    ) -> Result<Self>
    where
        P: crate::capabilities::SupportsTcp,
    {
        Self::connect(&crate::camera::CameraConfig::<P>::tcp(address), runtime).await
    }

    /// Connects a typed camera using its documented UDP port for host-only addresses.
    pub async fn connect_udp(
        address: impl Into<String>,
        runtime: impl crate::runtime::Runtime,
    ) -> Result<Self>
    where
        P: crate::capabilities::SupportsUdp,
    {
        Self::connect(&crate::camera::CameraConfig::<P>::udp(address), runtime).await
    }

    /// Connects one typed serial camera; opening writes only the configured startup.
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn connect_serial(
        port: impl Into<String>,
        baud_rate: u32,
        runtime: impl crate::runtime::Runtime,
    ) -> Result<Self>
    where
        P: crate::capabilities::SupportsSerial,
    {
        Self::connect(
            &crate::camera::CameraConfig::<P>::serial(port, baud_rate),
            runtime,
        )
        .await
    }
}

crate::camera_core::camera_core! {
    core: AsyncCameraCore,
    owner: AsyncOwnerHandle,
    async: [async],
    await: [.await],
}

crate::camera_facade::camera_facade! {
    core: AsyncCameraCore,
    owner: AsyncOwnerHandle,
    async: [async],
    await: [.await],
    error: [],
    session_mode_doc: "Construction moves the transport into one detached actor task. Constructors take their runtime or executor last; `close` waits for the actor to release its owned transport.",
    camera_session_constructors: {
        /// Connects a typed or runtime-selected camera configuration to one owner.
        ///
        /// TCP, UDP and serial return the same single-camera session shape.
        pub async fn connect(
            config: &crate::camera::CameraConfig<P>,
            runtime: impl crate::runtime::Runtime,
        ) -> Result<Self> {
            let plan = config.connection_plan()?;
            let session = crate::camera::construction::connect_async(plan.connection, runtime).await?;
            Ok(Self::from_registration(session, plan.registration))
        }

        /// Starts one single-camera owner session over a caller-owned transport.
        ///
        /// Both the transport selected in `config` and the actual opened transport
        /// must support the profile, as for [`CameraConfig::validate`](crate::config::CameraConfig::validate).
        /// Use `TransportOptions::Custom` in the configuration when only the opened
        /// transport should select the transport kind.
        ///
        /// This is the [`Session::open`] counterpart for callers that already own
        /// a transport. The profile comes from `config`, so the returned camera is
        /// bound to the same `P` the configuration was written for.
        pub async fn open(
            transport: impl AsyncTransport + HasTransportConfig + 'static,
            config: &crate::camera::CameraConfig<P>,
            executor: impl Executor,
        ) -> Result<Self> {
            let (session_config, registration) = config.validated_camera_config()?;
            let session = Session::open_validated(transport, session_config, executor).await?;
            Ok(Self::from_registration(session, registration))
        }
    },
}

#[cfg(all(test, feature = "test-utils"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Error;
    use std::{
        future::Future,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use crate::{
        capabilities::Capabilities,
        profiles::{ProfileEnvelope, ProfileTiming, TransportCompatibility},
        testing::testkit::{helpers, ManualClock, ManualClockExecutor, ScriptedTransport, Step},
        transport::{AsyncTransport, HasTransportConfig},
        CommandTimeouts, ProfileSpec,
    };

    const CUSTOM_IP_STREAM: crate::testing::testkit::ScriptedTransportIdentity =
        crate::testing::testkit::ScriptedTransportIdentity::Custom {
            semantics: crate::transport::SendSemantics::Stream,
            addressing: crate::transport::AddressingMode::Ip,
        };

    struct FailFirstZoomStopSend {
        inner: ScriptedTransport<Arc<ManualClockExecutor>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        failed: bool,
    }

    impl FailFirstZoomStopSend {
        fn new(
            steps: impl Into<Vec<Step>>,
            runtime: Arc<ManualClockExecutor>,
        ) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    inner: ScriptedTransport::new(CUSTOM_IP_STREAM, steps).with_executor(runtime),
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

        fn io_bounds(&self) -> &crate::transport::IoBounds {
            self.inner.io_bounds()
        }
    }

    impl AsyncTransport for FailFirstZoomStopSend {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            let should_fail = !self.failed && bytes.starts_with(&[0x81, 0x01, 0x04, 0x07]);
            self.failed |= should_fail;
            async move {
                if should_fail {
                    Err(Error::transport_error(std::io::Error::other(
                        "injected zoom stop send failure",
                    )))
                } else {
                    self.inner.send(bytes).await
                }
            }
        }

        fn recv_into<'a>(
            &'a mut self,
            dst: &'a mut [u8],
        ) -> impl Future<Output = Result<crate::transport::ReceiveOutcome, Error>> + Send {
            self.inner.recv_into(dst)
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

    fn partial_motion_core(session: &Session) -> AsyncCameraCore {
        let profile = session
            .config
            .registered_target(TargetSlot::CAMERA_1)
            .expect("registered partial motion profile");
        session.core(profile)
    }

    /// Issue #805: an operation's `Debug` output is its identity only, the
    /// same on both facades (this facade used to print its whole receipt).
    #[test]
    fn operation_debug_shows_only_its_identity() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, [helpers::auto_respond_step()])
            .with_executor(runtime.clone());
        let session = runtime
            .run_until(Session::open(
                transport,
                &SessionConfig::new(partial_motion_profile(true, false)),
                runtime.clone(),
            ))
            .expect("zoom session");
        let mut operation = runtime
            .run_until(
                partial_motion_core(&session)
                    .submit::<completion::AppliedOnly, _>(&crate::request::builtin::ZoomStop),
            )
            .expect("admitted");
        assert_eq!(
            format!("{operation:?}"),
            format!("Operation {{ id: {:?} }}", operation.id())
        );
        runtime.run_until(operation.applied()).expect("applied");
        runtime
            .run_until(session.close())
            .expect("session shutdown");
    }

    /// Issue #805: tuning is validated once, when the session opens, before
    /// any write: an open that would send the Sony RESET writes nothing.
    #[test]
    fn session_open_rejects_invalid_tuning_before_any_write() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, Vec::<Step>::new())
            .with_executor(runtime.clone());
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true)
            .with_tuning(OperationalTuning::new().with_ack_timeout(Duration::ZERO));
        assert!(matches!(
            runtime.run_until(Session::open(transport, &config, runtime.clone(),)),
            Err(Error::InvalidRequest { .. } | Error::InvalidParameter { .. })
        ));
        assert!(probe.sent().is_empty());
    }

    #[test]
    fn session_open_sends_opt_in_sony_sequence_reset_before_owner_work() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, Vec::<Step>::new())
            .with_executor(runtime.clone());
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);

        let session = runtime
            .run_until(Session::open(transport, &config, runtime.clone()))
            .expect("Sony session opens after RESET");

        assert_eq!(
            probe.sent(),
            vec![vec![0x02, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0x01]]
        );
        runtime
            .run_until(session.close())
            .expect("session shutdown");
    }

    #[derive(Clone)]
    struct ConstructionExecutor<E> {
        inner: E,
        drop_actor: bool,
        ready_sleep: bool,
    }

    impl<E: Executor> Executor for ConstructionExecutor<E> {
        type Timer = E::Timer;

        fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
            self.inner.timer(deadline)
        }
        fn spawn_detached<F>(&self, future: F)
        where
            F: Future<Output = ()> + Send + 'static,
        {
            if !self.drop_actor {
                self.inner.spawn_detached(future);
            }
        }
        async fn sleep(&self, duration: Duration) {
            if !self.ready_sleep {
                self.inner.sleep(duration).await;
            }
        }
        fn now(&self) -> std::time::Instant {
            self.inner.now()
        }
    }

    struct DropTrackedTransport<T> {
        inner: T,
        dropped: Arc<std::sync::atomic::AtomicBool>,
    }
    impl<T> Drop for DropTrackedTransport<T> {
        fn drop(&mut self) {
            self.dropped
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    impl<T: HasTransportConfig> HasTransportConfig for DropTrackedTransport<T> {
        fn identity(&self) -> crate::transport::TransportIdentity<'_> {
            self.inner.identity()
        }
        fn io_bounds(&self) -> &crate::transport::IoBounds {
            self.inner.io_bounds()
        }
    }
    impl<T: AsyncTransport> AsyncTransport for DropTrackedTransport<T> {
        async fn send(&mut self, bytes: &[u8]) -> Result<()> {
            self.inner.send(bytes).await
        }
        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<crate::transport::ReceiveOutcome> {
            self.inner.recv_into(dst).await
        }
    }

    #[test]
    fn session_open_refuses_an_executor_that_drops_the_actor_and_releases_transport() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let transport = DropTrackedTransport {
            inner: ScriptedTransport::new(CUSTOM_IP_STREAM, Vec::<Step>::new())
                .with_executor(runtime.clone()),
            dropped: Arc::clone(&dropped),
        };
        let executor = ConstructionExecutor {
            inner: runtime.clone(),
            drop_actor: true,
            ready_sleep: false,
        };
        let config = SessionConfig::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        assert!(matches!(
            runtime.run_until(Session::open(transport, &config, executor)),
            Err(Error::OwnerTerminated)
        ));
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn session_open_ready_sony_reset_wins_an_already_ready_timer() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, Vec::<Step>::new())
            .with_executor(runtime.clone());
        let probe = transport.clone();
        let executor = ConstructionExecutor {
            inner: runtime.clone(),
            drop_actor: false,
            ready_sleep: true,
        };
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);
        let session = runtime
            .run_until(Session::open(transport, &config, executor))
            .expect("ready RESET wins the elapsed timer");
        assert_eq!(
            probe.sent(),
            [vec![0x02, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0x01]]
        );
        runtime.run_until(session.close()).expect("actor closes");
    }

    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn session_open_refuses_a_shutdown_tokio_runtime() {
        let selected = tokio::runtime::Runtime::new().unwrap();
        let executor = crate::executor::TokioExecutor::from_handle(selected.handle().clone());
        // This plain test can wait for worker teardown before opening. Background
        // shutdown could instead drop a spawned actor after open's synchronous
        // terminal check, which the construction contract cannot detect.
        drop(selected);
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let transport = DropTrackedTransport {
            inner: ScriptedTransport::new(CUSTOM_IP_STREAM, Vec::<Step>::new())
                .with_executor(runtime),
            dropped: Arc::clone(&dropped),
        };
        let config = SessionConfig::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        assert!(matches!(
            pollster::block_on(Session::open(transport, &config, executor)),
            Err(Error::OwnerTerminated)
        ));
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn stop_all_motion_on_a_zoom_only_runtime_profile_writes_only_zoom_stop() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, [helpers::auto_respond_step()])
            .with_executor(runtime.clone());
        let probe = transport.clone();
        let session = runtime
            .run_until(Session::open(
                transport,
                &SessionConfig::new(partial_motion_profile(true, false)),
                runtime.clone(),
            ))
            .expect("zoom-only session");

        runtime
            .run_until(partial_motion_core(&session).stop_all_motion())
            .expect("zoom-only stop succeeds")
            .into_result()
            .expect("supported STOP applied");

        assert_eq!(
            probe.sent(),
            vec![vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff]],
            "unsupported pan/tilt and focus stops must never reach the wire"
        );
        runtime
            .run_until(session.close())
            .expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_on_a_profile_without_motion_axes_is_a_successful_no_op() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, Vec::<Step>::new())
            .with_executor(runtime.clone());
        let probe = transport.clone();
        let session = runtime
            .run_until(Session::open(
                transport,
                &SessionConfig::new(partial_motion_profile(false, false)),
                runtime.clone(),
            ))
            .expect("no-axis session");

        runtime
            .run_until(partial_motion_core(&session).stop_all_motion())
            .expect("no-axis stop is a successful no-op")
            .into_result()
            .expect("no supported axes");

        assert!(
            probe.sent().is_empty(),
            "a profile without motion axes must not emit a stop frame"
        );
        runtime
            .run_until(session.close())
            .expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_reports_each_supported_axis_after_a_partial_failure() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let (transport, writes) =
            FailFirstZoomStopSend::new([helpers::auto_respond_step()], runtime.clone());
        let session = runtime
            .run_until(Session::open(
                transport,
                &SessionConfig::new(partial_motion_profile(true, true)),
                runtime.clone(),
            ))
            .expect("zoom-focus session");

        let report = runtime
            .run_until(partial_motion_core(&session).stop_all_motion())
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
        runtime
            .run_until(session.close())
            .expect("session shutdown");
    }
    #[test]
    fn profile_erasure_and_typed_conversion_preserve_one_actor_and_view_settings() {
        use crate::profiles::{PtzOpticsG2, SonyFr7};
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, [helpers::auto_respond_step()])
            .with_executor(runtime.clone());
        let probe = transport.clone();
        let config =
            SessionConfig::from_compile_time::<PtzOpticsG2>().expect("typed configuration");
        let session = runtime
            .run_until(Session::open(transport, &config, runtime.clone()))
            .expect("owner session");
        let selected = session
            .camera::<PtzOpticsG2>(crate::IndividualCameraId::CAMERA_1)
            .expect("typed camera")
            .with_submission_class(SubmissionClass::Background);
        let erased: Camera = selected.into_any();
        assert_eq!(erased.submission_class(), Some(SubmissionClass::Background));
        assert!(erased.clone().into_typed::<SonyFr7>().is_err());
        assert!(matches!(
            runtime.run_until(erased.nd_filter().set_auto(true)),
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
        let mut operation = runtime
            .run_until(typed.zoom().stop())
            .expect("stop admission");
        runtime
            .run_until(operation.applied())
            .expect("applied stop");
        assert_eq!(probe.sent(), [vec![0x81, 1, 4, 7, 0, 0xff]]);
        runtime
            .run_until(session.close())
            .expect("one actor closes");
    }
    #[derive(Clone)]
    struct DiagnosticExecutor<E: Executor> {
        inner: Arc<E>,
        timers: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl<E: Executor> Executor for DiagnosticExecutor<E> {
        type Timer = E::Timer;
        fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
            self.timers
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.inner.timer(deadline)
        }
        fn spawn_detached<F>(&self, future: F)
        where
            F: Future<Output = ()> + Send + 'static,
        {
            self.inner.spawn_detached(future);
        }

        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            self.inner.sleep(duration)
        }

        fn now(&self) -> std::time::Instant {
            self.inner.now()
        }
    }

    async fn assert_diagnostics_reuse_one_timer(clock: &ManualClock, executor: impl Executor) {
        use crate::profiles::GenericVisca;
        use std::{
            pin::pin,
            sync::atomic::{AtomicUsize, Ordering},
        };
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Diagnostics>();
        let runtime = DiagnosticExecutor {
            inner: Arc::new(executor),
            timers: Arc::new(AtomicUsize::new(0)),
        };
        let transport = ScriptedTransport::new(CUSTOM_IP_STREAM, Vec::<Step>::new())
            .with_executor(runtime.inner.clone());
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("configuration");
        let session = Session::open(transport, &config, runtime.clone())
            .await
            .expect("session");
        let before = runtime.timers.load(Ordering::Relaxed);
        let mut diagnostics = session
            .subscribe_diagnostics(DiagnosticCapacity::MAX)
            .await
            .expect("subscription");
        assert_eq!(
            runtime.timers.load(Ordering::Relaxed),
            before + 1,
            "one bounded timer allocation at subscription creation"
        );
        let created = runtime.timers.load(Ordering::Relaxed);
        // Drop a pending receive, then reuse the same timer and subscription.
        {
            let mut receive = pin!(diagnostics.recv_with_timeout(Duration::from_secs(60)));
            assert!(futures_lite::future::poll_once(receive.as_mut())
                .await
                .is_none());
        }
        for _ in 0..3 {
            let start = clock.now();
            assert!(diagnostics
                .recv_with_timeout(Duration::from_secs(2))
                .await
                .expect("empty timed observation")
                .is_none());
            assert_eq!(clock.now() - start, Duration::from_secs(2));
        }
        assert_eq!(
            runtime.timers.load(Ordering::Relaxed),
            created,
            "receives reset the original timer"
        );
        session
            .close()
            .await
            .expect("owner closes while subscription exists");
        assert!(diagnostics
            .recv_with_timeout(Duration::ZERO)
            .await
            .expect("queued ending event")
            .is_some());
        while diagnostics.try_recv().is_some() {}
        assert!(matches!(
            diagnostics.recv().await,
            Err(Error::RuntimeShutdown)
        ));
        assert!(matches!(
            diagnostics.recv_with_timeout(Duration::ZERO).await,
            Err(Error::RuntimeShutdown)
        ));
    }

    #[test]
    fn diagnostics_reuse_one_timer_on_the_selected_manual_clock() {
        let clock = ManualClock::new();
        let executor = ManualClockExecutor::local(&clock);
        executor.run_until(assert_diagnostics_reuse_one_timer(&clock, executor.clone()));
    }

    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn diagnostics_reuse_the_selected_tokio_manual_clock() {
        let selected = tokio::runtime::Runtime::new().expect("Tokio runtime");
        let clock = ManualClock::new();
        let executor = ManualClockExecutor::new(
            crate::runtime::TokioExecutor::from_handle(selected.handle().clone()),
            &clock,
        );
        executor.run_until(assert_diagnostics_reuse_one_timer(&clock, executor.clone()));
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn diagnostics_reuse_the_selected_smol_manual_clock() {
        let clock = ManualClock::new();
        let executor = ManualClockExecutor::new(crate::runtime::SmolExecutor::new(), &clock);
        executor.run_until(assert_diagnostics_reuse_one_timer(&clock, executor.clone()));
    }

    #[test]
    fn diagnostics_keep_the_transport_terminal_cause_after_async_close() {
        use crate::{profiles::GenericVisca, types::FocusMode};
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new(
            CUSTOM_IP_STREAM,
            [
                Step::on_send(None, vec![]),
                helpers::errors::connection_lost(),
            ],
        )
        .with_executor(runtime.clone());
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("configuration");
        let session = runtime
            .run_until(Session::open(transport, &config, runtime.clone()))
            .expect("session");
        let diagnostics = runtime
            .run_until(session.subscribe_diagnostics(DiagnosticCapacity::MAX))
            .expect("subscription");
        assert!(runtime
            .run_until(
                session
                    .camera::<GenericVisca>(crate::IndividualCameraId::CAMERA_1)
                    .expect("camera")
                    .focus()
                    .set_mode(FocusMode::Manual)
            )
            .is_err());
        let terminal = runtime
            .run_until(session.close())
            .expect_err("transport terminal cause");
        while diagnostics.try_recv().is_some() {}
        let observed = runtime
            .run_until(diagnostics.recv())
            .expect_err("diagnostic terminal cause");
        assert_eq!(observed.kind(), terminal.kind());
        assert_eq!(observed.to_string(), terminal.to_string());
    }
}
