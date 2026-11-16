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
    CameraId, CancellationOutcome, Error, Inquiry, MetricsSnapshot, OperationCommand,
    OperationalTuning, PlainCommand, Result, SubmissionClass,
};

use crate::OperationId;

/// A blocking handle on one admitted operation.
///
/// The completion marker `K` is the only type parameter. No profile,
/// transport, executor, or runtime type appears in this public handle, and it
/// borrows nothing: it keeps the session's owner alive, so it may outlive the
/// [`Session`] and [`Camera`] it came from and move to another thread. The
/// owner remains authoritative for protocol lifecycle, timeouts, settlement,
/// and cancellation policy; the handle observes it.
///
/// # Waits borrow the handle
///
/// Every wait takes `&mut self`, and the handle caches the authoritative
/// result once received. Waiting again returns the cached result, an
/// [`applied`](Self::applied) wait can be followed by
/// [`settled`](Operation::settled), and a wait that times out releases only
/// that wait: the operation keeps running and the handle keeps observing it.
///
/// # Dropping never stops the camera
///
/// Dropping this handle is exactly [`detach`](Self::detach): it relinquishes
/// observation and nothing else. The owner keeps the protocol state, never
/// interprets handle drop as cancellation, and no STOP is written, so an early
/// `?` return or a panic unwinding past a live handle leaves physical movement
/// running until something ends it.
///
/// To bound movement by a scope, write a small guard whose own `Drop` submits
/// the typed STOP — see the guard pattern in `docs/migration_2_0.md` and
/// `examples/operation_handles.rs`. For an explicit stop on a path you
/// control, use `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
/// `camera.focus().stop()`, or `camera.motion().stop_all()`;
/// [`cancel`](Self::cancel) is protocol cancellation and does not by itself
/// prove motion ended.
#[must_use = "observe, cancel, or explicitly detach this operation"]
pub struct Operation<K>
where
    K: completion::Kind,
{
    receipt: BlockingOperationReceipt<K>,
}

impl<K> Operation<K>
where
    K: completion::Kind,
{
    /// Creates a public handle over an admitted owner receipt.
    ///
    /// This constructor is crate-private: only the blocking admission path may
    /// create a lifecycle handle, after the owner admitted the request.
    pub(crate) fn from_receipt(receipt: BlockingOperationReceipt<K>) -> Self {
        Self { receipt }
    }

    /// Returns the opaque identity assigned when this operation was admitted.
    #[must_use]
    pub fn id(&self) -> OperationId {
        OperationId::from_nonzero(self.receipt.id())
    }

    /// Waits for exact protocol application, bounded by the configured
    /// observer deadline.
    pub fn applied(&mut self) -> Result<(), Error> {
        self.receipt.applied(None)
    }

    /// Waits for exact protocol application, bounded by `timeout`.
    ///
    /// The deadline bounds only this wait. If it expires the wait returns
    /// [`Error::ObservationTimeout`]: the operation keeps running, its
    /// scheduler deadline is unchanged, and the handle can wait again.
    pub fn applied_with_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.receipt.applied(Some(timeout))
    }

    /// Cancels the operation and waits for the cancellation's conclusion,
    /// bounded by the configured cancellation deadline.
    ///
    /// The result is [`crate::operation::CancellationOutcome::Cancelled`] when cancellation
    /// won, [`crate::operation::CancellationOutcome::Completed`] when the operation was applied
    /// first, and the operation's own error when it failed first.
    ///
    /// Cancelling is idempotent. The handle has one cancellation intent;
    /// calling `cancel` again, including after a timed-out call, observes that
    /// intent instead of sending a second cancellation. Once the operation's
    /// outcome is known, `cancel` answers from it without sending anything.
    ///
    /// # A refused cancellation leaves the operation running
    ///
    /// Cancelling a request that is still queued always succeeds. Cancelling
    /// one that has already been written needs profile support for the
    /// standard VISCA socket-cancel command; without it — the built-in
    /// PTZOptics profiles ([`crate::profiles::PtzOpticsG2`], [`crate::profiles::PtzOpticsG3`] and
    /// [`crate::profiles::PtzOptics30x`]) are in that position — the owner refuses with
    /// [`Error::CancellationNotSupported`] and leaves the original request scheduled,
    /// retryable, and able to complete. The handle is unaffected and can
    /// still wait for it. A refusal does not stop the camera; a moving axis
    /// ends with an applied typed STOP.
    ///
    /// A cancellation that was accepted but then failed without ending the
    /// operation (its cancel write failed, or its observation deadline
    /// expired) returns that error; the handle still observes the operation's
    /// own outcome.
    ///
    /// [`crate::profiles::PtzOpticsG2`]: crate::profiles::PtzOpticsG2
    /// [`crate::profiles::PtzOpticsG3`]: crate::profiles::PtzOpticsG3
    /// [`crate::profiles::PtzOptics30x`]: crate::profiles::PtzOptics30x
    pub fn cancel(&mut self) -> Result<CancellationOutcome, Error> {
        self.receipt.cancel(None)
    }

    /// [`cancel`](Self::cancel), bounded by `timeout` instead of the
    /// configured cancellation deadline. If it expires, the cancellation
    /// intent stays recorded and a later `cancel` observes it.
    pub fn cancel_with_timeout(&mut self, timeout: Duration) -> Result<CancellationOutcome, Error> {
        self.receipt.cancel(Some(timeout))
    }

    /// Explicitly relinquishes this operation's observation.
    ///
    /// Detaching never records cancellation and never emits a physical STOP.
    /// It is the explicit spelling of what dropping the handle already does;
    /// movement continues until something else ends it.
    pub fn detach(self) {}
}

impl Operation<completion::Targeted> {
    /// Waits for exact application and the profile-selected protocol
    /// settlement condition. The default budget is the larger of the configured
    /// settlement override and this request's retry budget. Use
    /// [`settled_with_timeout`](Self::settled_with_timeout) for an exact bound.
    ///
    /// Application is cached, so calling this after
    /// [`applied`](Operation::applied) continues from it. Polled settlement proves observed stability, not arrival at the requested endpoint.
    /// Later conflicting admission supersedes unfinished polled evidence. Settlement proven
    /// by position polling is cached once proven; a polling wait that times
    /// out restarts with a fresh proof.
    pub fn settled(&mut self) -> Result<crate::Settlement, Error> {
        self.receipt.settled(None)
    }

    /// [`settled`](Self::settled), bounded by `timeout`.
    ///
    /// The deadline bounds only this wait; the prepared scheduler and
    /// settlement policy are unchanged. Each abandoned polling attempt may
    /// leave its admitted position inquiries running until their own
    /// deadlines, within the session's admission capacity.
    pub fn settled_with_timeout(&mut self, timeout: Duration) -> Result<crate::Settlement, Error> {
        self.receipt.settled(Some(timeout))
    }
}

impl<K> fmt::Debug for Operation<K>
where
    K: completion::Kind,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.receipt.fmt_public_handle(formatter)
    }
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
    pub fn recv(&self) -> Result<crate::observability::DiagnosticEvent> {
        self.inner.recv_blocking()
    }

    /// Waits at most `timeout` on the owner's clock for an event.
    ///
    /// Returns `Ok(None)` when the queue remains empty, or the owner's
    /// original terminal cause once every queued event has been received.
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

/// One blocking owner session.
///
/// The transport is moved into one native owner worker thread at
/// construction (D24). The session and its [`Camera`] views are
/// `Clone + Send + Sync`. An [`Operation`] handle is `Send + Sync`, borrows
/// nothing, and can move to another thread; its waits borrow that handle.
/// The worker serializes every call, and one thread waiting on an operation never blocks
/// another thread's submit, cancel, or STOP. Timers, reads, and correlation
/// cleanup progress on the worker even while no caller is inside the session.
///
/// Every clone shares the one worker. Dropping the last handle — session,
/// camera, or operation — stops it and releases the transport without sending
/// a protocol STOP; [`close`](Self::close) is the explicit, joining teardown.
#[derive(Clone)]
pub struct Session {
    owner: BlockingOwnerHandle,
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
        let mut adapter = BlockingTransportAdapter::new_with_targets(
            transport,
            &profiles,
            config.validated_tuning(),
            config.admission_capacity(),
            config.strict_unconfirmed_poison(),
        )?;
        if config.sony_sequence_reset_on_connect() {
            adapter.send_sony_sequence_reset()?;
        }
        let policy = adapter.policy().clone();
        Ok(Self {
            owner: BlockingOwnerHandle::spawn(policy, adapter, clock)?,
            config: Arc::new(config),
        })
    }

    /// Returns a camera view for the registered target.
    ///
    /// Typed profiles require exact equality with their validated profile facts;
    /// [`crate::profiles::AnyProfile`] accepts the registered runtime profile.
    pub fn camera<P: CameraProfile>(&self, target: CameraId) -> Result<Camera<P>, Error> {
        let target = TargetSlot::try_from(target)?;
        let registration = self.config.registered_target(target)?;
        P::validate_spec(registration.profile())?;
        Ok(Camera::from_core(self.core(registration)))
    }

    /// The erased view of `target`, registered with `profile`.
    fn core(&self, registration: Arc<crate::TargetRegistration>) -> BlockingCameraCore {
        BlockingCameraCore {
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
    /// **Every request prepared after this call returns uses the new values**
    /// — its acknowledgement, completion, settlement, and inquiry deadlines,
    /// its retry budget, and its pacing floor. The owner's session-wide pacing
    /// and per-target command-socket capacity are re-derived immediately, so
    /// work still queued behind pacing is released under the new values too.
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
    /// current tuning untouched. Returns the session's terminal error if the
    /// owner has shut down; a retained terminal error takes precedence over
    /// validation of the proposed update.
    pub fn set_tuning(&self, tuning: OperationalTuning) -> Result<(), Error> {
        let validated_tuning = self.config.validate_tuning(tuning);
        self.owner.reconfigure(validated_tuning)
    }

    /// Requests owner shutdown.
    ///
    /// This is an idempotent shutdown request. It returns once the request is
    /// accepted by the owner's bounded shutdown boundary; it does not join the
    /// worker or claim that transport teardown has finished. After this call,
    /// new request admission is rejected with [`Error::RuntimeShutdown`]. All
    /// [`Session`] clones share this one shutdown boundary.
    pub fn shutdown(&self) -> Result<(), Error> {
        self.owner.shutdown()
    }

    /// Requests owner shutdown and consumes this session, waiting for teardown.
    ///
    /// Unlike [`Self::shutdown`], this is a deterministic transport teardown
    /// barrier: it returns only after the worker has dropped its transport,
    /// so reopening the same endpoint after `close` is safe. If shutdown was
    /// accepted, the terminal owner result is returned after teardown: an
    /// explicit shutdown returns `Ok(())`, while a transport close or stream
    /// poison that won the race is returned unchanged. If sending this call's
    /// shutdown signal fails immediately, that error is preserved. A worker
    /// that panicked is reported as [`Error::OwnerTerminated`].
    /// A custom transport calling this from its own worker requests shutdown
    /// but receives [`Error::InvalidState`] immediately: it cannot wait for
    /// its own teardown. A caller on another thread can still close and join.
    /// Other calls that would wait for this worker also fail immediately with
    /// [`Error::InvalidState`], without enqueuing work.
    pub fn close(self) -> Result<(), Error> {
        self.owner.close()
    }

    /// Returns a scalar snapshot of the owner without cloning diagnostics.
    ///
    /// After normal conclusion, returns the retained terminal snapshot. An
    /// owner that unwinds before conclusion reports `OwnerTerminated`.
    pub fn metrics(&self) -> Result<MetricsSnapshot, Error> {
        self.owner.metrics()
    }

    /// Subscribes to the owner's bounded, best-effort diagnostic stream.
    ///
    /// `capacity` must be in `1..=128`; at most four live subscriptions are
    /// retained by one owner. A slow subscriber never stalls command or
    /// transport processing; its dropped events are visible in [`Self::metrics`].
    ///
    /// Delivery is not ordered against receipt resolution: a wait on an
    /// operation can return before that operation's terminal event is queued
    /// here. Once [`Self::close`] returns, every event the owner published is
    /// queued.
    pub fn subscribe_diagnostics(&self, capacity: usize) -> Result<Diagnostics, Error> {
        self.owner
            .subscribe_diagnostics(capacity)
            .map(Diagnostics::from_owner)
    }
}

/// One blocking owner session with one typed or runtime-profile camera.
///
/// This is the single-target counterpart of [`Session`]. The profile is named
/// once, at construction, and the [`Camera`] this type hands out was built from
/// that same profile parameter: the session path's second, runtime-checked
/// profile naming (`session.camera::<P>(target)?`) has no equivalent here, so a
/// profile mismatch is not expressible.
///
/// The value owns its session, so it is self-sufficient: the connection lives
/// as long as this camera session (or a [`Camera`] taken out of it) is alive.
/// Dropping the final owner handle stops the worker and releases the
/// transport, but does not emit a protocol STOP. Use [`close`](Self::close)
/// for the explicit teardown that mirrors [`Session::close`].
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
    pub fn set_tuning(&self, tuning: OperationalTuning) -> Result<(), Error> {
        self.session.set_tuning(tuning)
    }

    /// Requests owner shutdown without consuming this value.
    ///
    /// This is the idempotent, non-joining signal; use [`Self::close`] when
    /// the worker and owned transport must be fully released before continuing.
    pub fn shutdown(&self) -> Result<(), Error> {
        self.session.shutdown()
    }

    /// Requests owner shutdown and consumes this camera session, waiting for
    /// the worker and its owned transport to be fully released.
    ///
    /// This is the deterministic teardown barrier described by
    /// [`Session::close`]; use [`Self::shutdown`] when only an idempotent,
    /// non-joining signal is required.
    pub fn close(self) -> Result<(), Error> {
        self.session.close()
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

/// A profile-aware camera view onto one [`Session`] target.
///
/// The profile parameter supplies compile-time capability gates; owner and
/// transport state remain in the shared erased core. The view is
/// `Clone + Send + Sync` and borrows nothing: it keeps the session's owner
/// alive, and concurrent calls through it are serialized by the owner worker.
#[must_use]
pub struct Camera<P: CameraProfile = AnyProfile> {
    core: BlockingCameraCore,
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
    pub(crate) fn from_core(core: BlockingCameraCore) -> Self {
        Self {
            core,
            _profile: PhantomData,
        }
    }

    pub(crate) const fn core(&self) -> &BlockingCameraCore {
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
    pub fn execute<C>(&self, command: &C) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute(command)
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
    pub fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire(inquiry)
    }

    /// Submits a typed operation and returns its owner-backed handle once the
    /// owner has admitted it.
    pub fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit(operation)
    }
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
        transport::{BlockingTransport, HasTransportConfig, TransportConfig},
        CommandTimeouts, ProfileSpec,
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
                    inner: ScriptedBlockingTransport::new(steps),
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
            .maximum_command_sockets(1)
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

    /// Two views taken from the same session drive its one owned transport,
    /// and an operation submitted through either of them is applied rather
    /// than merely being constructed.
    #[test]
    fn camera_views_share_the_session_owner() {
        use crate::testing::testkit::{helpers, ScriptedBlockingTransport};

        let transport = ScriptedBlockingTransport::new([
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
        ]);
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap();
        let session = Session::open(transport, &config).unwrap();

        let first = session
            .camera::<crate::profiles::PtzOpticsG2>(CameraId::CAMERA_1)
            .unwrap();
        let second = session
            .camera::<crate::profiles::PtzOpticsG2>(CameraId::CAMERA_1)
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
        let transport = ScriptedBlockingTransport::new([helpers::auto_respond_step()]);
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

    /// Issue #805: tuning is validated once, when the session opens, before
    /// any write: an open that would send the Sony RESET writes nothing.
    #[test]
    fn session_open_rejects_invalid_tuning_before_any_write() {
        let transport = ScriptedBlockingTransport::new([]);
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true)
            .with_tuning(OperationalTuning::new().with_maximum_command_sockets(u8::MAX));
        assert!(matches!(
            Session::open(transport, &config),
            Err(Error::InvalidRequest { .. } | Error::InvalidParameter { .. })
        ));
        assert!(probe.sent().is_empty());
    }

    #[test]
    fn session_open_sends_opt_in_sony_sequence_reset_before_owner_work() {
        let transport = ScriptedBlockingTransport::new([]);
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
        let transport = ScriptedBlockingTransport::new([helpers::auto_respond_step()]);
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
        let transport = ScriptedBlockingTransport::new([]);
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
        session.shutdown().expect("session shutdown");
    }
    #[test]
    fn profile_erasure_and_typed_conversion_preserve_one_owner_and_view_settings() {
        use crate::{
            profiles::{PtzOpticsG2, SonyFr7},
            testing::testkit::{helpers, ScriptedBlockingTransport},
        };
        let transport = ScriptedBlockingTransport::new([helpers::auto_respond_step()]);
        let probe = transport.clone();
        let config =
            SessionConfig::from_compile_time::<PtzOpticsG2>().expect("typed configuration");
        let session = Session::open(transport, &config).expect("owner session");
        let selected = session
            .camera::<PtzOpticsG2>(CameraId::CAMERA_1)
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
                ScriptedBlockingTransport::new([helpers::auto_respond_step()]),
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
                .camera::<GenericVisca>(CameraId::CAMERA_1)
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
        let transport = ScriptedBlockingTransport::new([
            Step::on_send(None, vec![]),
            helpers::errors::connection_lost(),
        ]);
        let session = Session::open(transport, &config).expect("session");
        let diagnostics = session.subscribe_diagnostics(128).expect("subscription");
        assert!(session
            .camera::<GenericVisca>(CameraId::CAMERA_1)
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
