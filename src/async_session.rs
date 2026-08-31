//! Final, profile-generic async owner facade.
//!
//! This module is intentionally small. A [`Session`] owns exactly one
//! serialized [`AsyncOwnerActor`](crate::runtime::owner::AsyncOwnerActor) and
//! the transport moved into that actor. [`Camera`] values are inexpensive
//! target views: cloning one only clones the owner handle and its immutable
//! target/profile facts.

#![cfg(feature = "async")]

use std::{fmt, marker::PhantomData, sync::Arc, time::Duration};

use crate::{
    camera::{IdleWait, MotionQuery},
    completion,
    executor::Executor,
    operation::Operation,
    prepared::{
        prepare_command, prepare_inquiry, prepare_operation, prepare_position_queries,
        ClassSelection,
    },
    profile::{CompileTimeProfile, OperationalTuning, ProfileSpec},
    request::builtin::{FocusStop, PanTiltStop, ZoomStop},
    runtime::{
        owner::AsyncTransportAdapter,
        owner::{
            ensure_async_before_deadline, sample_positions_async, AsyncOwnerActor, AsyncOwnerHandle,
        },
    },
    stop_request::pan_tilt_stop_request,
    transport::{AsyncTransport, HasTransportConfig},
    CameraId, DiagnosticSubscription, Error, Inquiry, MetricsSnapshot, OperationCommand,
    PlainCommand, Result, SessionConfig, StateCache, SubmissionClass,
};

const MOTION_QUERY_OBSERVER_BUDGET: Duration = Duration::from_secs(30);

/// One async serialized owner session.
#[derive(Clone)]
pub struct Session {
    owner: AsyncOwnerHandle,
    config: Arc<SessionConfig>,
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
    /// Starts one owner actor over a caller-owned async transport.
    ///
    /// Startup performs profile, tuning, transport-buffer, and immutable owner
    /// policy validation before spawning exactly one actor task. The future
    /// returns after the actor is detached; request methods return after their
    /// own admission or terminal observation as documented below.
    pub async fn open<E, T>(
        transport: T,
        config: SessionConfig,
        executor: impl Into<Arc<E>>,
    ) -> Result<Self>
    where
        E: Executor,
        T: AsyncTransport + HasTransportConfig + 'static,
    {
        config.validate_for_transport(transport.standard_transport_kind())?;
        let tuning = config.tuning();
        let profiles = config.profile_registry();
        let adapter = AsyncTransportAdapter::new_with_profile_registry(
            transport,
            &profiles,
            tuning,
            config.admission_capacity(),
        )?;
        let policy = adapter.policy().clone();

        let executor: Arc<E> = executor.into();
        let (owner, actor) = AsyncOwnerActor::new(policy, (*executor).clone())?;
        // This is the only task spawned by this facade. The owner handle is
        // cloneable; all public camera views share its one serialized actor.
        executor.spawn_bg(actor.run(adapter));
        Ok(Self {
            owner,
            config: Arc::new(config),
        })
    }

    /// Returns the statically checked view for the sole registered target.
    pub fn camera<P>(&self) -> Result<Camera<P>>
    where
        P: CompileTimeProfile,
    {
        let target = self.config.sole_target().ok_or_else(|| {
            if self.config.target_count() == 0 {
                Error::InvalidState("session has no registered target".into())
            } else {
                Error::InvalidState(
                    "session has multiple registered targets; select one with camera_for".into(),
                )
            }
        })?;
        self.camera_for::<P>(target)
    }

    /// Returns a target-specific statically checked camera view.
    pub fn camera_for<P>(&self, target: CameraId) -> Result<Camera<P>>
    where
        P: CompileTimeProfile,
    {
        let profile = self.config.profile_arc_for_compile_time::<P>(target)?;
        Ok(Camera::from_core(AsyncCameraCore {
            owner: self.owner.clone(),
            target,
            profile,
            class: ClassSelection::Request,
        }))
    }

    /// Builds an erased owner view for the dynamic API. This deliberately does
    /// not perform compile-time profile matching: dynamic callers use the
    /// stored validated [`ProfileSpec`] at each operation boundary.
    #[cfg(feature = "dyn-api")]
    pub(crate) fn camera_core(&self) -> Result<AsyncCameraCore> {
        let target = self.config.sole_target().ok_or_else(|| {
            if self.config.target_count() == 0 {
                Error::InvalidState("session has no registered target".into())
            } else {
                Error::InvalidState(
                    "session has multiple registered targets; select one with camera_for".into(),
                )
            }
        })?;
        self.camera_core_for(target)
    }

    /// Builds an erased target view for the dynamic API.
    #[cfg(feature = "dyn-api")]
    pub(crate) fn camera_core_for(&self, target: CameraId) -> Result<AsyncCameraCore> {
        let profile = self
            .config
            .profile_arc(target)
            .ok_or_else(|| Error::InvalidRequest("session target is not registered".into()))?;
        Ok(AsyncCameraCore {
            owner: self.owner.clone(),
            target,
            profile,
            class: ClassSelection::Request,
        })
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
    /// is the one deliberate difference from 1.2.0's
    /// `Camera::set_timeout_config`, which recomputed deadlines on every
    /// housekeeping pass and therefore also covered work in flight. To widen a
    /// deadline for a command that is already running, cancel it and resubmit.
    ///
    /// The update is not a merge: `tuning` replaces the previous value whole,
    /// so a field left unset returns to its profile default rather than keeping
    /// the value a previous call installed.
    ///
    /// The update travels through the owner's control boundary and the owner is
    /// its only writer, so two session clones reconfiguring concurrently
    /// resolve last-writer-wins in the order the owner accepted them; no reader
    /// ever observes a mixture of the two.
    ///
    /// # Errors
    ///
    /// Rejects exactly what construction rejects — tuning that weakens a
    /// registered profile's pacing minima, raises its socket limit, undercuts
    /// its deadlines, or specifies incoherent retry timing — leaving the
    /// session's current tuning untouched. Returns the session's terminal error
    /// if the owner has shut down.
    pub async fn set_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        self.config.validate_tuning(tuning)?;
        self.owner.reconfigure(tuning).await
    }

    /// Requests the single owner actor to shut down.
    ///
    /// This is an idempotent shutdown request. It completes once the request
    /// is accepted by the owner's bounded shutdown boundary; it does not join
    /// the detached actor task or claim that transport teardown has finished.
    /// After this call, new request admission is rejected with
    /// [`Error::RuntimeShutdown`]. All [`Session`] clones share this one
    /// shutdown boundary, so a consuming [`Self::close`] on any clone waits
    /// for the same sole actor.
    pub async fn shutdown(&self) -> Result<()> {
        self.owner.shutdown().await
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
        let shutdown_result = self.shutdown().await;
        let teardown_result = self.owner.wait_closed().await;
        match shutdown_result {
            Err(error) => Err(error),
            Ok(()) => teardown_result,
        }
    }

    /// Returns a scalar snapshot of the owner without cloning diagnostics.
    pub async fn metrics(&self) -> Result<MetricsSnapshot> {
        self.owner.metrics().await
    }

    /// Subscribes to the owner's bounded, best-effort diagnostic stream.
    ///
    /// `capacity` must be in `1..=128`; at most four live subscriptions are
    /// retained by one owner. A slow subscriber never stalls command or
    /// transport processing; its dropped events are visible in [`Self::metrics`].
    pub async fn subscribe_diagnostics(&self, capacity: usize) -> Result<DiagnosticSubscription> {
        self.owner
            .subscribe_diagnostics(capacity)
            .await
            .map(DiagnosticSubscription::from_owner)
    }
}

/// One async owner session that owns exactly one compile-time bound camera.
///
/// This is the single-target counterpart of [`Session`]. The profile is named
/// once, at construction, and the [`Camera`] this type hands out was built from
/// that same profile parameter: the session path's second, runtime-checked
/// profile naming (`session.camera::<P>()?`) has no equivalent here, so a
/// profile mismatch is not expressible.
///
/// The value owns its session, so it is self-sufficient: the connection lives
/// as long as this camera session (or a [`Camera`] taken out of it) is alive.
/// Use [`close`](Self::close) for the explicit teardown that mirrors
/// [`Session::close`].
pub struct CameraSession<P: CompileTimeProfile> {
    session: Session,
    camera: Camera<P>,
}

impl<P: CompileTimeProfile> fmt::Debug for CameraSession<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CameraSession")
            .field("target", &self.camera.target())
            .field("profile", &self.camera.profile())
            .finish_non_exhaustive()
    }
}

impl<P: CompileTimeProfile> CameraSession<P> {
    /// Builds the single-camera view for a session whose sole target was
    /// registered from the same `P`.
    ///
    /// This is crate-private and is only reachable from the `P`-typed
    /// construction paths, where the registered [`ProfileSpec`] was lowered
    /// from this exact `P`. That is what makes the bind structural instead of
    /// a runtime profile comparison.
    pub(crate) fn from_session(session: Session, target: CameraId) -> Result<Self> {
        let profile = session.config.profile_arc(target).ok_or_else(|| {
            Error::InvalidState("single-camera session lost its registered target".into())
        })?;
        let camera = Camera::from_core(AsyncCameraCore {
            owner: session.owner.clone(),
            target,
            profile,
            class: ClassSelection::Request,
        });
        Ok(Self { session, camera })
    }

    /// Starts one single-camera owner session over a caller-owned transport.
    ///
    /// This is the [`Session::open`] counterpart for callers that already own
    /// a transport. The profile comes from `config`, so the returned camera is
    /// bound to the same `P` the configuration was written for.
    pub async fn open<E, T>(
        transport: T,
        config: &crate::camera::CameraConfig<P>,
        executor: impl Into<Arc<E>>,
    ) -> Result<Self>
    where
        E: Executor,
        T: AsyncTransport + HasTransportConfig + 'static,
    {
        let target = config.camera_id;
        let session = Session::open(transport, config.session_config()?, executor).await?;
        Self::from_session(session, target)
    }

    /// Returns this session's compile-time bound camera view.
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
    pub const fn target(&self) -> CameraId {
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
    pub async fn shutdown(&self) -> Result<()> {
        self.session.shutdown().await
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

/// Erased owner-backed target view shared by typed and dynamic projections.
///
/// This is crate-private so the public camera type cannot accidentally expose
/// transport or executor details. It is the only place that retains the owner
/// handle and performs request preparation.
#[derive(Clone)]
pub(crate) struct AsyncCameraCore {
    owner: AsyncOwnerHandle,
    target: CameraId,
    profile: Arc<ProfileSpec>,
    class: ClassSelection,
}

impl AsyncCameraCore {
    #[cfg(feature = "dyn-api")]
    pub(crate) fn into_typed<P>(self) -> Result<Camera<P>>
    where
        P: CompileTimeProfile,
    {
        self.profile.ensure_compile_time::<P>()?;
        Ok(Camera {
            core: self,
            _profile: PhantomData,
        })
    }

    pub(crate) const fn target(&self) -> CameraId {
        self.target
    }

    pub(crate) fn profile(&self) -> &ProfileSpec {
        self.profile.as_ref()
    }

    pub(crate) fn state_cache(&self) -> StateCache {
        self.owner.state_cache(self.target)
    }

    /// Reads the session's live operational tuning.
    ///
    /// This is read once per preparation rather than copied into the view, so a
    /// [`Camera`] cloned before [`Session::set_tuning`] still prepares its next
    /// request under the new values (#631).
    fn tuning(&self) -> OperationalTuning {
        self.owner.tuning()
    }

    pub(crate) const fn submission_class(&self) -> Option<SubmissionClass> {
        self.class.handle_default()
    }

    pub(crate) fn set_submission_class(&mut self, class: Option<SubmissionClass>) {
        self.class = ClassSelection::from_handle_default(class);
    }

    pub(crate) async fn execute<C>(&self, command: &C) -> Result<()>
    where
        C: PlainCommand + ?Sized,
    {
        self.execute_with_selection(command, self.class).await
    }

    pub(crate) async fn execute_with_submission_class<C>(
        &self,
        command: &C,
        class: SubmissionClass,
    ) -> Result<()>
    where
        C: PlainCommand + ?Sized,
    {
        self.execute_with_selection(command, ClassSelection::Explicit(class))
            .await
    }

    async fn execute_with_selection<C>(&self, command: &C, class: ClassSelection) -> Result<()>
    where
        C: PlainCommand + ?Sized,
    {
        let prepared = prepare_command(
            command,
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            class,
        )?;
        let receipt = self.owner.submit_command(prepared).await?;
        receipt.wait(self.owner.receipt_control()).await
    }

    pub(crate) async fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response>
    where
        Q: Inquiry + ?Sized,
    {
        self.inquire_with_selection(inquiry, self.class).await
    }

    pub(crate) async fn inquire_with_submission_class<Q>(
        &self,
        inquiry: &Q,
        class: SubmissionClass,
    ) -> Result<Q::Response>
    where
        Q: Inquiry + ?Sized,
    {
        self.inquire_with_selection(inquiry, ClassSelection::Explicit(class))
            .await
    }

    async fn inquire_with_selection<Q>(
        &self,
        inquiry: &Q,
        class: ClassSelection,
    ) -> Result<Q::Response>
    where
        Q: Inquiry + ?Sized,
    {
        let prepared = prepare_inquiry(
            inquiry,
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            class,
        )?;
        let receipt = self.owner.submit_inquiry(prepared).await?;
        receipt.wait(self.owner.receipt_control()).await
    }

    pub(crate) async fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.submit_with_selection::<K, O>(operation, self.class)
            .await
    }

    pub(crate) async fn submit_with_submission_class<K, O>(
        &self,
        operation: &O,
        class: SubmissionClass,
    ) -> Result<Operation<K>>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.submit_with_selection::<K, O>(operation, ClassSelection::Explicit(class))
            .await
    }

    async fn submit_with_selection<K, O>(
        &self,
        operation: &O,
        class: ClassSelection,
    ) -> Result<Operation<K>>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        let prepared = prepare_operation::<K, _>(
            operation,
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            class,
        )?;
        let receipt = self.owner.submit_operation(prepared).await?;
        Ok(Operation::from_receipt(
            receipt,
            self.owner.receipt_control(),
        ))
    }

    pub(crate) async fn stop_all_motion(&self) -> Result<()> {
        let mut first_error = None;

        let pan_tilt_result = match self.pan_tilt_stop_request() {
            Ok(stop) => match self.submit::<completion::AppliedOnly, _>(&stop).await {
                Ok(operation) => operation.applied().await,
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        retain_first_error(&mut first_error, pan_tilt_result);

        let zoom_result = match self.submit::<completion::AppliedOnly, _>(&ZoomStop).await {
            Ok(operation) => operation.applied().await,
            Err(error) => Err(error),
        };
        retain_first_error(&mut first_error, zoom_result);

        let focus_result = match self.submit::<completion::AppliedOnly, _>(&FocusStop).await {
            Ok(operation) => operation.applied().await,
            Err(error) => Err(error),
        };
        retain_first_error(&mut first_error, focus_result);

        first_error.map_or(Ok(()), Err)
    }

    pub(crate) async fn is_moving(&self, query: MotionQuery) -> Result<bool> {
        let queries = prepare_position_queries(
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            query.axes,
        )?;
        let deadline = self.owner.deadline_after(MOTION_QUERY_OBSERVER_BUDGET)?;
        let control = self.owner.receipt_control();
        let mut detector = crate::prepared::MotionDetector::new(query.axes, query.tolerance);

        let baseline = sample_positions_async(&self.owner, &control, &queries, deadline).await?;
        if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
            return Err(Error::InvalidState(
                "new movement detector rejected its baseline snapshot".into(),
            ));
        }
        ensure_async_before_deadline(&self.owner, deadline)?;
        let next = sample_positions_async(&self.owner, &control, &queries, deadline).await?;
        Ok(matches!(
            detector.observe(next)?,
            crate::prepared::MotionState::Moving
        ))
    }

    pub(crate) async fn wait_until_idle(&self, wait: IdleWait) -> Result<()> {
        let queries =
            prepare_position_queries(self.target, self.profile.as_ref(), self.tuning(), wait.axes)?;
        let deadline = self.owner.deadline_after(wait.timeout)?;
        let control = self.owner.receipt_control();
        let mut detector = crate::prepared::MotionDetector::new(wait.axes, wait.tolerance);

        let baseline = sample_positions_async(&self.owner, &control, &queries, deadline).await?;
        if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
            return Err(Error::InvalidState(
                "new movement detector rejected its baseline snapshot".into(),
            ));
        }

        loop {
            ensure_async_before_deadline(&self.owner, deadline)?;
            let remaining = deadline.saturating_duration_since(self.owner.now());
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            self.owner.sleep(wait.interval.min(remaining)).await;
            ensure_async_before_deadline(&self.owner, deadline)?;
            let snapshot =
                sample_positions_async(&self.owner, &control, &queries, deadline).await?;
            match detector.observe(snapshot)? {
                crate::prepared::MotionState::Settled => return Ok(()),
                crate::prepared::MotionState::Moving => {}
                crate::prepared::MotionState::NeedSample => {
                    return Err(Error::InvalidState(
                        "movement detector lost its baseline snapshot".into(),
                    ));
                }
            }
        }
    }

    pub(crate) fn pan_tilt_stop_request(&self) -> Result<PanTiltStop> {
        pan_tilt_stop_request(self.profile.as_ref())
    }
}

/// Profile-generic camera view over one session target.
///
/// The profile parameter supplies compile-time capability gates; all owner and
/// transport state remains in the shared erased core.
#[must_use]
pub struct Camera<P: CompileTimeProfile> {
    core: AsyncCameraCore,
    _profile: PhantomData<fn() -> P>,
}

impl<P: CompileTimeProfile> Clone for Camera<P> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            _profile: PhantomData,
        }
    }
}

impl<P: CompileTimeProfile> fmt::Debug for Camera<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Camera")
            .field("target", &self.core.target)
            .field("profile", &self.core.profile)
            .finish_non_exhaustive()
    }
}

impl<P: CompileTimeProfile> Camera<P> {
    pub(crate) fn from_core(core: AsyncCameraCore) -> Self {
        Self {
            core,
            _profile: PhantomData,
        }
    }

    /// Returns this view's fixed camera target.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.core.target()
    }

    /// Returns this view's immutable validated profile facts.
    #[must_use]
    pub fn profile(&self) -> &ProfileSpec {
        self.core.profile()
    }

    /// Returns this view's validated runtime capability inventory.
    ///
    /// This is the runtime discovery view of the same facts the compile-time
    /// marker traits gate: it reports what the profile documents, not
    /// permission to call a typed API.
    #[must_use]
    pub fn capabilities(&self) -> &crate::capabilities::Capabilities {
        self.core.profile().capabilities()
    }

    /// Returns a cheap live read-only view of this camera's target-local state.
    #[must_use]
    pub fn state_cache(&self) -> StateCache {
        self.core.state_cache()
    }

    pub(crate) const fn core(&self) -> &AsyncCameraCore {
        &self.core
    }

    /// Returns this handle's submission-class default, if it carries one.
    ///
    /// `None` — the initial value — means every request uses its intrinsic
    /// [`crate::ControlClass`]. See
    /// [`set_submission_class`](Self::set_submission_class).
    #[must_use]
    pub const fn submission_class(&self) -> Option<SubmissionClass> {
        self.core.submission_class()
    }

    /// Sets the ordinary-work [`SubmissionClass`] every later submission from
    /// *this handle* uses, or clears it with `None`.
    ///
    /// The owner dispatches ready work from the highest occupied class first
    /// and FIFO within a class, so lowering this default makes the handle's
    /// traffic yield the transport to other handles' ordinary work, and
    /// raising it makes the handle's traffic overtake work that is still
    /// **queued**. It never interrupts, cancels, or reorders a request that
    /// has already been written to the transport, and it does not change the
    /// class of requests submitted before the call.
    ///
    /// The default is a property of *this handle*, not of the camera or the
    /// session: another handle onto the same target keeps its own value, and a
    /// [`clone`](Clone::clone) copies the current value and then diverges. It
    /// applies to every request the handle submits — commands, inquiries, and
    /// operations, including the ones the noun accessors submit — with exactly
    /// one exception.
    ///
    /// # Urgent requests are never demoted
    ///
    /// A request the crate classifies [`crate::ControlClass::Urgent`] — the typed
    /// stops [`PanTiltStop`], [`ZoomStop`], [`FocusStop`], and owner-issued
    /// protocol cancellation — ignores this default and stays urgent. A handle demoted to
    /// [`SubmissionClass::Background`] for telemetry polling therefore still
    /// preempts with an emergency stop. Per-submission overrides obey the same
    /// safety floor, and [`SubmissionClass`] deliberately has no urgent
    /// variant for callers to manufacture.
    ///
    /// Owner-internal traffic that no caller submitted — the settlement
    /// polling behind [`Operation::settled`](crate::Operation::settled) and
    /// the observation inquiries behind `motion()` — keeps its own built-in
    /// class.
    pub fn set_submission_class(&mut self, class: Option<SubmissionClass>) {
        self.core.set_submission_class(class);
    }

    /// Executes a plain command through this camera's shared owner.
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

    /// Executes a plain command in an explicitly named scheduling lane.
    ///
    /// `class` replaces this handle's
    /// [`set_submission_class`](Self::set_submission_class) default for this
    /// submission only. It applies to ordinary work; an intrinsically
    /// [`crate::ControlClass::Urgent`] command remains urgent.
    pub async fn execute_with_submission_class<C>(
        &self,
        command: &C,
        class: SubmissionClass,
    ) -> Result<()>
    where
        C: PlainCommand + ?Sized,
    {
        self.core
            .execute_with_submission_class(command, class)
            .await
    }

    /// Sends an inquiry and decodes its response through the shared owner.
    pub async fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire(inquiry).await
    }

    /// Sends an inquiry in an explicitly named scheduling lane.
    ///
    /// `class` replaces this handle's
    /// [`set_submission_class`](Self::set_submission_class) default for this
    /// submission only. It cannot weaken an intrinsic urgent safety class.
    pub async fn inquire_with_submission_class<Q>(
        &self,
        inquiry: &Q,
        class: SubmissionClass,
    ) -> Result<Q::Response>
    where
        Q: Inquiry + ?Sized,
    {
        self.core
            .inquire_with_submission_class(inquiry, class)
            .await
    }

    /// Submits a typed operation and returns its owner-backed handle.
    pub async fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit(operation).await
    }

    /// Submits a typed operation in an explicitly named scheduling lane.
    ///
    /// `class` replaces this handle's
    /// [`set_submission_class`](Self::set_submission_class) default for this
    /// submission only. As with
    /// [`execute_with_submission_class`](Self::execute_with_submission_class),
    /// an intrinsically urgent stop remains urgent.
    pub async fn submit_with_submission_class<K, O>(
        &self,
        operation: &O,
        class: SubmissionClass,
    ) -> Result<Operation<K>>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core
            .submit_with_submission_class::<K, O>(operation, class)
            .await
    }
}

fn retain_first_error(first_error: &mut Option<Error>, result: Result<()>) {
    if let Err(error) = result {
        if first_error.is_none() {
            *first_error = Some(error);
        }
    }
}
