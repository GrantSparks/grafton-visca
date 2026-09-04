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
    AffectedAxes, CameraId, DiagnosticSubscription, Error, Inquiry, MetricsSnapshot,
    OperationCommand, PlainCommand, Result, SessionConfig, StateCache, SubmissionClass,
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
        let mut adapter = AsyncTransportAdapter::new_with_profile_registry(
            transport,
            &profiles,
            tuning,
            config.admission_capacity(),
            config.strict_unconfirmed_poison(),
        )?;
        let policy = adapter.policy().clone();

        let executor: Arc<E> = executor.into();
        if config.sony_sequence_reset_on_connect() {
            executor
                .timeout(policy.write_timeout, adapter.send_sony_sequence_reset())
                .await??;
        }
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

    /// Returns the runtime-profile camera view for the sole registered target.
    #[cfg(feature = "dyn-api")]
    pub fn camera_dyn(&self) -> Result<crate::dynapi::DynSessionCamera> {
        crate::dynapi::DynSessionCamera::from_session(self)
    }

    /// Returns a target-specific runtime-profile camera view.
    #[cfg(feature = "dyn-api")]
    pub fn camera_dyn_for(&self, target: CameraId) -> Result<crate::dynapi::DynSessionCamera> {
        crate::dynapi::DynSessionCamera::from_session_target(self, target)
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
                    "session has multiple registered targets; select one with camera_dyn_for"
                        .into(),
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
    /// The update is not a merge: a field left unset returns to its profile
    /// default rather than keeping the value a previous call installed.
    /// Construction-only recovery policy lives on [`SessionConfig`], outside
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
        let validated_tuning = self.config.validate_tuning(tuning).map(|()| tuning);
        self.owner.reconfigure(validated_tuning).await
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
/// Dropping the final async owner handle may release the actor and transport,
/// but does not emit a protocol STOP or provide a teardown barrier. Use
/// [`close`](Self::close) for the explicit teardown that mirrors
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

        if self.profile.supports_axes(AffectedAxes::PAN_TILT) {
            let pan_tilt_result = match self.pan_tilt_stop_request() {
                Ok(stop) => match self.submit::<completion::AppliedOnly, _>(&stop).await {
                    Ok(operation) => operation.applied().await,
                    Err(error) => Err(error),
                },
                Err(error) => Err(error),
            };
            retain_first_error(&mut first_error, pan_tilt_result);
        }

        if self.profile.supports_axes(AffectedAxes::ZOOM) {
            let zoom_result = match self.submit::<completion::AppliedOnly, _>(&ZoomStop).await {
                Ok(operation) => operation.applied().await,
                Err(error) => Err(error),
            };
            retain_first_error(&mut first_error, zoom_result);
        }

        if self.profile.supports_axes(AffectedAxes::FOCUS) {
            let focus_result = match self.submit::<completion::AppliedOnly, _>(&FocusStop).await {
                Ok(operation) => operation.applied().await,
                Err(error) => Err(error),
            };
            retain_first_error(&mut first_error, focus_result);
        }

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

    /// Derives a camera view whose ordinary work uses `class`.
    ///
    /// The original view is unchanged. Commands, inquiries, operations, and
    /// noun methods submitted through the returned view all inherit this
    /// class; intrinsically urgent stops remain urgent.
    pub fn with_submission_class(&self, class: SubmissionClass) -> Self {
        let mut selected = self.clone();
        selected.set_submission_class(Some(class));
        selected
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
    /// This intentionally has no request-specific `P: Has*` bound. Noun and
    /// accessor methods carry the `Has*` marker bounds that define the
    /// compile-time profile-permission surface; this direct method is the
    /// uniform typed/downstream extension boundary.
    ///
    /// Preparation calls [`crate::Request::validate_for_profile`] before
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
    /// This intentionally has no request-specific `P: Has*` bound. Noun and
    /// accessor methods carry the `Has*` marker bounds that define the
    /// compile-time profile-permission surface; this direct method is the
    /// uniform typed/downstream extension boundary.
    ///
    /// Preparation calls [`crate::Request::validate_for_profile`] before
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

fn retain_first_error(first_error: &mut Option<Error>, result: Result<()>) {
    if let Err(error) = result {
        if first_error.is_none() {
            *first_error = Some(error);
        }
    }
}

#[cfg(all(test, feature = "test-utils"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        sync::{Arc, Mutex},
    };

    use crate::{
        capabilities::Capabilities,
        profile::{PositionInquirySupport, ProfileEnvelope, ProfileTiming, TransportCompatibility},
        testing::testkit::{helpers, DeterministicExecutor, ScriptedTransport, Step},
        transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
        CommandTimeouts,
    };

    struct FailFirstZoomStopSend {
        inner: ScriptedTransport<DeterministicExecutor>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        failed: bool,
    }

    impl FailFirstZoomStopSend {
        fn new(
            steps: impl Into<Vec<Step>>,
            runtime: Arc<DeterministicExecutor>,
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
                    Err(Error::TransportError(
                        "injected zoom stop send failure".into(),
                    ))
                } else {
                    self.inner.send(bytes).await
                }
            }
        }

        fn recv_into<'a>(
            &'a mut self,
            dst: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            self.inner.recv_into(dst)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    fn partial_motion_profile(has_zoom: bool, has_focus: bool) -> ProfileSpec {
        let mut capabilities =
            Capabilities::runtime_baseline("Partial motion test camera", 1).expect("baseline");
        capabilities.has_zoom = has_zoom;
        capabilities.has_focus = has_focus;

        ProfileSpec::builder(capabilities)
            .transports(TransportCompatibility::new(Some(5678), None, false))
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
                    .minimum_inquiry_spacing(Duration::ZERO)
                    .minimum_command_spacing(Duration::ZERO)
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(1)
            .supports_operation_complete(true)
            .supports_command_cancel(false)
            .preset_recall_axes(None)
            .position_inquiries(PositionInquirySupport::new(false, false, false))
            .build()
            .expect("valid partial motion profile")
    }

    fn partial_motion_core(session: &Session) -> AsyncCameraCore {
        AsyncCameraCore {
            owner: session.owner.clone(),
            target: CameraId::CAMERA_1,
            profile: session
                .config
                .profile_arc(CameraId::CAMERA_1)
                .expect("registered partial motion profile"),
            class: ClassSelection::Request,
        }
    }

    #[test]
    fn session_open_sends_opt_in_sony_sequence_reset_before_owner_work() {
        let (runtime, _) = DeterministicExecutor::new();
        let transport = ScriptedTransport::new(Vec::<Step>::new()).with_executor(runtime.clone());
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFR7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);

        let session = runtime
            .run_until(Session::open::<DeterministicExecutor, _>(
                transport,
                config,
                runtime.clone(),
            ))
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
        let (runtime, _) = DeterministicExecutor::new();
        let transport =
            ScriptedTransport::new([helpers::auto_respond_step()]).with_executor(runtime.clone());
        let probe = transport.clone();
        let session = runtime
            .run_until(Session::open::<DeterministicExecutor, _>(
                transport,
                SessionConfig::new(partial_motion_profile(true, false)),
                runtime.clone(),
            ))
            .expect("zoom-only session");

        runtime
            .run_until(partial_motion_core(&session).stop_all_motion())
            .expect("zoom-only stop succeeds");

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
        let (runtime, _) = DeterministicExecutor::new();
        let transport = ScriptedTransport::new(Vec::<Step>::new()).with_executor(runtime.clone());
        let probe = transport.clone();
        let session = runtime
            .run_until(Session::open::<DeterministicExecutor, _>(
                transport,
                SessionConfig::new(partial_motion_profile(false, false)),
                runtime.clone(),
            ))
            .expect("no-axis session");

        runtime
            .run_until(partial_motion_core(&session).stop_all_motion())
            .expect("no-axis stop is a successful no-op");

        assert!(
            probe.sent().is_empty(),
            "a profile without motion axes must not emit a stop frame"
        );
        runtime
            .run_until(session.close())
            .expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_returns_the_first_supported_failure_after_later_stops() {
        let (runtime, _) = DeterministicExecutor::new();
        let (transport, writes) =
            FailFirstZoomStopSend::new([helpers::auto_respond_step()], runtime.clone());
        let session = runtime
            .run_until(Session::open::<DeterministicExecutor, _>(
                transport,
                SessionConfig::new(partial_motion_profile(true, true)),
                runtime.clone(),
            ))
            .expect("zoom-focus session");

        assert!(matches!(
            runtime.run_until(partial_motion_core(&session).stop_all_motion()),
            Err(Error::TransportError(_))
        ));
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
}
