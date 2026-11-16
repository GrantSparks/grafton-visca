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
    CameraId, Inquiry, MetricsSnapshot, OperationCommand, PlainCommand, Result, SessionConfig,
    SubmissionClass,
};

pub use crate::async_operation::Operation;

/// A bounded asynchronous diagnostic stream from one session.
///
/// Dropping this stream releases its subscription without keeping the owner
/// alive. Queued events remain readable before the owner's original terminal cause.
pub struct Diagnostics {
    inner: crate::observability::DiagnosticSubscription,
    wait: crate::observability::DiagnosticWait,
}

impl Diagnostics {
    fn from_owner(
        inner: crate::runtime::owner::DiagnosticSubscription,
        clock: crate::runtime::owner::BoundClock,
    ) -> Self {
        Self {
            inner: crate::observability::DiagnosticSubscription::from_owner(inner),
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
        self.inner.recv().await
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

/// One async serialized owner session.
#[derive(Clone)]
pub struct Session {
    owner: AsyncOwnerHandle,
    config: Arc<crate::session_config::ValidatedSessionConfig>,
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("targets", &self.config.target_count())
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
    /// own admission or terminal observation as documented below.
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
            config.validate_opened_transport(&transport)?;
            let tuning = config.validated_tuning();
            let profiles = config.profile_registry();
            let mut adapter = AsyncTransportAdapter::new_with_targets(
                transport,
                &profiles,
                tuning,
                config.admission_capacity(),
                config.strict_unconfirmed_poison(),
            )?;
            let policy = adapter.policy().clone();

            if config.sony_sequence_reset_on_connect() {
                executor
                    .timeout(policy.write_timeout, adapter.send_sony_sequence_reset())
                    .await??;
            }
            let (owner, actor) = AsyncOwnerActor::new(policy, executor.clone())?;
            // This is the only task spawned by this facade. The owner handle is
            // cloneable; all public camera views share its one serialized actor.
            executor.spawn_bg(actor.run(adapter));
            Ok(Self {
                owner,
                config: Arc::new(config),
            })
        }
    }

    /// Returns a camera view for the registered target.
    ///
    /// Typed profiles require exact equality with their validated profile facts;
    /// [`crate::profiles::AnyProfile`] accepts the registered runtime profile.
    pub fn camera<P: CameraProfile>(&self, target: CameraId) -> Result<Camera<P>> {
        let target = TargetSlot::try_from(target)?;
        let registration = self.config.registered_target(target)?;
        P::validate_spec(registration.profile())?;
        Ok(Camera::from_core(self.core(registration)))
    }

    /// The erased view of `target`, registered with `profile`.
    fn core(&self, registration: Arc<crate::TargetRegistration>) -> AsyncCameraCore {
        AsyncCameraCore {
            owner: self.owner.clone(),
            registration,
            class: ClassSelection::Request,
        }
    }

    /// Returns the operational tuning this session is currently preparing
    /// requests under.
    ///
    /// This is the live value, not the one the session was opened with: it
    /// reflects the most recent successful [`set_tuning`](Self::set_tuning),
    /// including one made through another clone of this session.
    #[must_use]
    pub fn tuning(&self) -> OperationalTuning {
        self.owner.tuning()
    }

    /// Replaces this session's operational tuning at runtime.
    ///
    /// # Scope
    ///
    /// **Every request prepared after this future resolves uses the new
    /// values** — its acknowledgement, completion, settlement, and inquiry
    /// deadlines, its retry budget, and its pacing floor. The owner's
    /// session-wide pacing and per-target command-socket capacity are
    /// re-derived immediately, so work still queued behind pacing is released
    /// under the new values too.
    ///
    /// **Requests already in flight keep the deadlines they were admitted
    /// with.** 2.0 stamps a request's deadlines once, at preparation, and the
    /// engine derives its absolute phase deadlines from that stamp; nothing is
    /// re-timed underneath an [`Operation`] a caller is already holding. This
    /// is deliberate: a deadline that moved after submission would change the
    /// contract of work a caller has already observed, so tuning applies only
    /// to requests prepared after the call. To widen a deadline for a command
    /// that is already running, cancel it and resubmit.
    ///
    /// The update is not a merge: a field left unset returns to its profile
    /// default rather than keeping the value a previous call installed.
    /// Construction-only recovery policy lives on [`crate::config::SessionConfig`], outside
    /// `OperationalTuning`, and is therefore unaffected by this call.
    ///
    /// The update travels through the owner's control boundary and the owner is
    /// its only writer, so two session clones reconfiguring concurrently
    /// resolve last-writer-wins in the order the owner accepted them; no reader
    /// ever observes a mixture of the two.
    ///
    /// # Errors
    ///
    /// Rejects tuning that construction would reject for a registered profile:
    /// values that weaken pacing minima, raise a socket limit, undercut a
    /// deadline, or specify incoherent retry timing, leaving the session's
    /// current tuning untouched. Returns the session's terminal error
    /// if the owner has shut down; a retained terminal error takes precedence
    /// over validation of the proposed update.
    pub async fn set_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        let validated_tuning = self.config.validate_tuning(tuning);
        self.owner.reconfigure(validated_tuning).await
    }

    /// Requests the single owner actor to shut down.
    ///
    /// This is an idempotent, non-blocking shutdown request: it returns once
    /// the request is accepted by the owner's bounded shutdown boundary, so
    /// it never waits and is not `async`. It does not join
    /// the detached actor task or claim that transport teardown has finished.
    /// After this call, new request admission is rejected with
    /// [`Error::RuntimeShutdown`](crate::Error::RuntimeShutdown). All
    /// [`Session`] clones share this one shutdown boundary, so a consuming
    /// [`Self::close`] on any clone waits for the same sole actor.
    pub fn shutdown(&self) -> Result<()> {
        self.owner.shutdown()
    }

    /// Requests owner shutdown and consumes this session, waiting for teardown.
    ///
    /// Unlike [`Self::shutdown`], this is a deterministic driver/transport
    /// teardown barrier. It does not resolve until the detached owner has
    /// dropped its driver (including the owned transport), so reopening the
    /// same endpoint after `close` is safe. The barrier does not promise an
    /// executor-specific task join after that release point. If shutdown was
    /// accepted, the terminal owner result is returned after teardown: an
    /// explicit shutdown returns `Ok(())`, while a transport close or stream
    /// poison that won the source ordering race is returned unchanged. If
    /// sending this call's shutdown signal fails immediately, that send error
    /// is preserved.
    pub async fn close(self) -> Result<()> {
        let shutdown_result = self.shutdown();
        let teardown_result = self.owner.wait_closed().await;
        match shutdown_result {
            Err(error) => Err(error),
            Ok(()) => teardown_result,
        }
    }

    /// Returns a scalar snapshot of the owner without cloning diagnostics.
    ///
    /// After normal conclusion, returns the retained terminal snapshot. An
    /// owner that unwinds before conclusion reports `OwnerTerminated`.
    pub async fn metrics(&self) -> Result<MetricsSnapshot> {
        self.owner.metrics().await
    }

    /// Subscribes to the owner's bounded, best-effort diagnostic stream.
    ///
    /// `capacity` must be in `1..=128`; at most four live subscriptions are
    /// retained by one owner. A slow subscriber never stalls command or
    /// transport processing; its dropped events are visible in [`Self::metrics`].
    ///
    /// Delivery is not ordered against receipt resolution: a wait on an
    /// operation can return before that operation's terminal event is queued
    /// here.
    pub async fn subscribe_diagnostics(&self, capacity: usize) -> Result<Diagnostics> {
        let subscription = self.owner.subscribe_diagnostics(capacity).await?;
        Ok(Diagnostics::from_owner(
            subscription,
            self.owner.diagnostic_clock(),
        ))
    }
}

/// One async owner session with one typed or runtime-profile camera.
///
/// This is the single-target counterpart of [`Session`]. The profile is named
/// once, at construction, and the [`Camera`] this type hands out was built from
/// that same profile parameter: the session path's second, runtime-checked
/// profile naming (`session.camera::<P>(target)?`) has no equivalent here, so a
/// profile mismatch is not expressible.
///
/// The value owns its session, so it is self-sufficient: the connection lives
/// as long as this camera session (or a [`Camera`] taken out of it) is alive.
/// Dropping the final async owner handle may release the actor and transport,
/// but does not emit a protocol STOP or provide a teardown barrier. Use
/// [`close`](Self::close) for the explicit teardown that mirrors
/// [`Session::close`].
pub struct CameraSession<P: CameraProfile = AnyProfile> {
    session: Session,
    camera: Camera<P>,
}

impl<P: CameraProfile> fmt::Debug for CameraSession<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CameraSession")
            .field("target", &self.camera.target())
            .field("profile", &self.camera.profile())
            .finish_non_exhaustive()
    }
}

impl<P: CameraProfile> CameraSession<P> {
    /// Binds the sole camera selected by a validated `CameraConfig<P>`.
    ///
    /// Construction already checked the selection's `CameraProfile` contract,
    /// so this internal bind reuses those facts without another profile resolution.
    pub(crate) fn from_registration(
        session: Session,
        registration: Arc<crate::TargetRegistration>,
    ) -> Self {
        let camera = Camera::from_core(session.core(registration));
        Self { session, camera }
    }

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

    /// Erases only the compile-time profile parameter, retaining the same owner.
    #[must_use]
    pub fn into_any(self) -> CameraSession<AnyProfile> {
        CameraSession {
            session: self.session,
            camera: self.camera.into_any(),
        }
    }

    /// Checks the complete registered profile before selecting a typed view.
    pub fn into_typed<T: CompileTimeProfile>(self) -> Result<CameraSession<T>> {
        Ok(CameraSession {
            session: self.session,
            camera: self.camera.into_typed::<T>()?,
        })
    }

    /// Returns this session's profile-bound camera view.
    ///
    /// The profile is not named again and the call cannot fail.
    pub const fn camera(&self) -> &Camera<P> {
        &self.camera
    }

    /// Returns the submission-class default this session's camera carries.
    ///
    /// See [`Camera::submission_class`].
    #[must_use]
    pub const fn submission_class(&self) -> Option<SubmissionClass> {
        self.camera.submission_class()
    }

    /// Sets the submission-class default for this session's camera.
    ///
    /// This is [`Camera::set_submission_class`] applied to the camera this
    /// session owns, so every view handed out by [`camera`](Self::camera) and
    /// every noun accessor reached through it submits in `class` afterwards.
    /// A [`Camera`] already taken out with [`into_camera`](Self::into_camera)
    /// carries its own copy and is unaffected.
    pub fn set_submission_class(&mut self, class: Option<SubmissionClass>) {
        self.camera.set_submission_class(class);
    }

    /// Consumes this value and returns the owned camera.
    ///
    /// The camera keeps the owner alive, so the connection survives; the
    /// explicit [`close`](Self::close) is given up in exchange.
    pub fn into_camera(self) -> Camera<P> {
        self.camera
    }

    /// Returns the owned session behind this camera.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// Returns this session's sole camera target.
    #[must_use]
    pub fn target(&self) -> CameraId {
        self.camera.target()
    }

    /// Returns the operational tuning this session is currently preparing
    /// requests under.
    #[must_use]
    pub fn tuning(&self) -> OperationalTuning {
        self.session.tuning()
    }

    /// Replaces this session's operational tuning at runtime.
    ///
    /// This is [`Session::set_tuning`] on the session this camera owns; see
    /// there for the exact scope, in particular that requests already in flight
    /// keep the deadlines they were admitted with.
    ///
    /// # Errors
    ///
    /// See [`Session::set_tuning`].
    pub async fn set_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        self.session.set_tuning(tuning).await
    }

    /// Requests owner shutdown without consuming this value.
    ///
    /// This is the idempotent, non-joining signal; use [`Self::close`] when
    /// the actor and owned transport must be fully released before continuing.
    pub fn shutdown(&self) -> Result<()> {
        self.session.shutdown()
    }

    /// Requests owner shutdown and consumes this camera session, waiting for
    /// the actor and its owned transport to be fully dropped.
    ///
    /// This is the deterministic teardown barrier described by
    /// [`Session::close`]; use [`Self::shutdown`] when only an idempotent,
    /// non-joining signal is required.
    pub async fn close(self) -> Result<()> {
        self.session.close().await
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

/// Profile-generic camera view over one session target.
///
/// The profile parameter supplies compile-time capability gates; all owner and
/// transport state remains in the shared erased core.
#[must_use]
pub struct Camera<P: CameraProfile = AnyProfile> {
    core: AsyncCameraCore,
    _profile: PhantomData<fn() -> P>,
}

impl<P: CameraProfile> Clone for Camera<P> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            _profile: PhantomData,
        }
    }
}

impl<P: CameraProfile> fmt::Debug for Camera<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Camera")
            .field("target", &self.core.target())
            .field("profile", &self.core.profile())
            .finish_non_exhaustive()
    }
}

impl<P: CameraProfile> Camera<P> {
    pub(crate) fn from_core(core: AsyncCameraCore) -> Self {
        Self {
            core,
            _profile: PhantomData,
        }
    }

    pub(crate) const fn core(&self) -> &AsyncCameraCore {
        &self.core
    }

    crate::camera_view::camera_view_getters!();

    /// Erases only the compile-time profile parameter, retaining this owner and target.
    pub fn into_any(self) -> Camera<AnyProfile> {
        Camera::from_core(self.core)
    }

    /// Checks the complete registered profile before selecting a typed view.
    pub fn into_typed<T: CompileTimeProfile>(self) -> Result<Camera<T>> {
        T::validate_spec(self.core.profile())?;
        Ok(Camera::from_core(self.core))
    }

    /// Executes a plain command through this camera's shared owner.
    ///
    /// This intentionally has no request-specific `Permits` bound. Noun and
    /// accessor methods use sealed `Permits<gate::X>` bounds: evidenced typed
    /// profiles and `AnyProfile` permit calls, while preparation checks the
    /// registered profile before I/O. `Has*` markers remain evidence claims;
    /// this direct method is the uniform typed/downstream extension boundary.
    ///
    /// Preparation calls [`crate::request::Request::validate_for_target`] before
    /// encoding, owner admission, or transport I/O. Crate-provided requests
    /// submitted directly return [`crate::Error::FeatureNotSupported`] there
    /// when the selected profile lacks their required feature. Raw and other
    /// downstream requests are explicit low-level extensions and may provide
    /// their own profile validation.
    ///
    /// Ordinary commands wait for their terminal protocol application. A raw
    /// [`crate::raw::RawReplyShape::NoReply`] command instead succeeds once its
    /// local transport write succeeds; it does not claim camera application.
    pub async fn execute<C>(&self, command: &C) -> Result<()>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute(command).await
    }

    /// Sends an inquiry and decodes its response through the shared owner.
    ///
    /// This intentionally has no request-specific `Permits` bound. Noun and
    /// accessor methods use sealed `Permits<gate::X>` bounds: evidenced typed
    /// profiles and `AnyProfile` permit calls, while preparation checks the
    /// registered profile before I/O. `Has*` markers remain evidence claims;
    /// this direct method is the uniform typed/downstream extension boundary.
    ///
    /// Preparation calls [`crate::request::Request::validate_for_target`] before
    /// encoding, owner admission, or transport I/O. Crate-provided inquiries
    /// submitted directly return [`crate::Error::FeatureNotSupported`] there
    /// when the selected profile lacks their required feature. Raw and other
    /// downstream requests are explicit low-level extensions and may provide
    /// their own profile validation.
    pub async fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire(inquiry).await
    }

    /// Submits a typed operation and returns its owner-backed handle.
    pub async fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit(operation).await
    }
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
        transport::{AsyncTransport, HasTransportConfig, TransportConfig},
        CommandTimeouts, ProfileSpec,
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
                    inner: ScriptedTransport::new(steps).with_executor(runtime),
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

        fn transport_config(&self) -> &TransportConfig {
            self.inner.transport_config()
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
            .maximum_command_sockets(1)
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
        let transport =
            ScriptedTransport::new([helpers::auto_respond_step()]).with_executor(runtime.clone());
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
        let transport = ScriptedTransport::new(Vec::<Step>::new()).with_executor(runtime.clone());
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true)
            .with_tuning(OperationalTuning::new().with_maximum_command_sockets(u8::MAX));
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
        let transport = ScriptedTransport::new(Vec::<Step>::new()).with_executor(runtime.clone());
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

    #[test]
    fn stop_all_motion_on_a_zoom_only_runtime_profile_writes_only_zoom_stop() {
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport =
            ScriptedTransport::new([helpers::auto_respond_step()]).with_executor(runtime.clone());
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
        let transport = ScriptedTransport::new(Vec::<Step>::new()).with_executor(runtime.clone());
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
            crate::HaltOutcome::Failed(Error::TransportError { .. })
        ));
        assert!(matches!(report.focus, crate::HaltOutcome::Applied));
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
        let transport =
            ScriptedTransport::new([helpers::auto_respond_step()]).with_executor(runtime.clone());
        let probe = transport.clone();
        let config =
            SessionConfig::from_compile_time::<PtzOpticsG2>().expect("typed configuration");
        let session = runtime
            .run_until(Session::open(transport, &config, runtime.clone()))
            .expect("owner session");
        let selected = session
            .camera::<PtzOpticsG2>(CameraId::CAMERA_1)
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
        assert_eq!(typed.target(), CameraId::CAMERA_1);
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
        type Join<T>
            = E::Join<T>
        where
            T: Send + 'static;
        type Detach = E::Detach;
        type Timer = E::Timer;
        fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
            self.timers
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.inner.timer(deadline)
        }
        fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.inner.spawn_with_detach(future)
        }
        fn block_on<F: Future>(&self, future: F) -> F::Output {
            self.inner.block_on(future)
        }
        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            self.inner.sleep(duration)
        }
        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            future: F,
        ) -> impl Future<Output = Result<T>> + Send + 'a
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.inner.timeout(duration, future)
        }
        fn now(&self) -> std::time::Instant {
            self.inner.now()
        }
    }

    fn assert_diagnostics_reuse_one_timer(clock: &ManualClock, executor: impl Executor) {
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
        let transport =
            ScriptedTransport::new(Vec::<Step>::new()).with_executor(runtime.inner.clone());
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("configuration");
        let session = runtime
            .block_on(Session::open(transport, &config, runtime.clone()))
            .expect("session");
        let before = runtime.timers.load(Ordering::Relaxed);
        let mut diagnostics = runtime
            .block_on(session.subscribe_diagnostics(128))
            .expect("subscription");
        assert_eq!(
            runtime.timers.load(Ordering::Relaxed),
            before + 1,
            "one bounded timer allocation at subscription creation"
        );
        let created = runtime.timers.load(Ordering::Relaxed);
        // Drop a pending receive, then reuse the same timer and subscription.
        runtime.block_on(async {
            let mut receive = pin!(diagnostics.recv_with_timeout(Duration::from_secs(60)));
            assert!(futures_lite::future::poll_once(receive.as_mut())
                .await
                .is_none());
        });
        // Measure within one tracked task: between block_on roots the clock
        // may also service the owner's independent idle deadlines.
        runtime.block_on(async {
            for _ in 0..3 {
                let start = clock.now();
                assert!(diagnostics
                    .recv_with_timeout(Duration::from_secs(2))
                    .await
                    .expect("empty timed observation")
                    .is_none());
                assert_eq!(clock.now() - start, Duration::from_secs(2));
            }
        });
        assert_eq!(
            runtime.timers.load(Ordering::Relaxed),
            created,
            "receives reset the original timer"
        );
        runtime
            .block_on(session.close())
            .expect("owner closes while subscription exists");
        assert!(runtime
            .block_on(diagnostics.recv_with_timeout(Duration::ZERO))
            .expect("queued ending event")
            .is_some());
        while diagnostics.try_recv().is_some() {}
        assert!(matches!(
            runtime.block_on(diagnostics.recv()),
            Err(Error::RuntimeShutdown)
        ));
        assert!(matches!(
            runtime.block_on(diagnostics.recv_with_timeout(Duration::ZERO)),
            Err(Error::RuntimeShutdown)
        ));
    }

    #[test]
    fn diagnostics_reuse_one_timer_on_the_selected_manual_clock() {
        let clock = ManualClock::new();
        assert_diagnostics_reuse_one_timer(&clock, ManualClockExecutor::local(&clock));
    }

    #[cfg(feature = "runtime-tokio")]
    #[test]
    fn diagnostics_reuse_the_selected_tokio_manual_clock() {
        let selected = tokio::runtime::Runtime::new().expect("Tokio runtime");
        let clock = ManualClock::new();
        assert_diagnostics_reuse_one_timer(
            &clock,
            ManualClockExecutor::new(
                crate::runtime::TokioExecutor::from_handle(selected.handle().clone()),
                &clock,
            ),
        );
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn diagnostics_reuse_the_selected_smol_manual_clock() {
        let clock = ManualClock::new();
        assert_diagnostics_reuse_one_timer(
            &clock,
            ManualClockExecutor::new(crate::runtime::SmolExecutor::new(), &clock),
        );
    }

    #[test]
    fn diagnostics_keep_the_transport_terminal_cause_after_async_close() {
        use crate::{profiles::GenericVisca, types::FocusMode};
        let clock = ManualClock::new();
        let runtime = Arc::new(ManualClockExecutor::local(&clock));
        let transport = ScriptedTransport::new([
            Step::on_send(None, vec![]),
            helpers::errors::connection_lost(),
        ])
        .with_executor(runtime.clone());
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("configuration");
        let session = runtime
            .run_until(Session::open(transport, &config, runtime.clone()))
            .expect("session");
        let diagnostics = runtime
            .run_until(session.subscribe_diagnostics(128))
            .expect("subscription");
        assert!(runtime
            .run_until(
                session
                    .camera::<GenericVisca>(CameraId::CAMERA_1)
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
