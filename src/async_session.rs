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
    prepared::{prepare_command, prepare_inquiry, prepare_operation, prepare_position_queries},
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
    PlainCommand, Result, SessionConfig, StateCache,
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
        let adapter =
            AsyncTransportAdapter::new_with_profile_registry(transport, &profiles, tuning)?;
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
            tuning: self.config.tuning(),
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
            tuning: self.config.tuning(),
        })
    }

    /// Requests the single owner actor to shut down.
    ///
    /// This is an idempotent shutdown request. It completes once the request
    /// is accepted by the owner's bounded shutdown boundary; it does not join
    /// the detached actor task or claim that transport teardown has finished.
    /// After this call, new request admission is rejected with
    /// [`Error::RuntimeShutdown`].
    pub async fn shutdown(&self) -> Result<()> {
        self.owner.shutdown().await
    }

    /// Requests owner shutdown and consumes this session.
    ///
    /// Like [`Self::shutdown`], this is not a task-join operation.
    pub async fn close(self) -> Result<()> {
        self.shutdown().await
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
    tuning: OperationalTuning,
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

    pub(crate) async fn execute<C>(&self, command: &C) -> Result<()>
    where
        C: PlainCommand + ?Sized,
    {
        let prepared = prepare_command(command, self.target, self.profile.as_ref(), self.tuning)?;
        let receipt = self.owner.submit_command(prepared).await?;
        receipt.wait(self.owner.receipt_control()).await
    }

    pub(crate) async fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response>
    where
        Q: Inquiry + ?Sized,
    {
        let prepared = prepare_inquiry(inquiry, self.target, self.profile.as_ref(), self.tuning)?;
        let receipt = self.owner.submit_inquiry(prepared).await?;
        receipt.wait(self.owner.receipt_control()).await
    }

    pub(crate) async fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        let prepared =
            prepare_operation::<K, _>(operation, self.target, self.profile.as_ref(), self.tuning)?;
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
        let queries =
            prepare_position_queries(self.target, self.profile.as_ref(), self.tuning, query.axes)?;
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
            prepare_position_queries(self.target, self.profile.as_ref(), self.tuning, wait.axes)?;
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

    /// Executes a plain command through this camera's shared owner.
    pub async fn execute<C>(&self, command: &C) -> Result<()>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute(command).await
    }

    /// Sends an inquiry and decodes its response through the shared owner.
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
