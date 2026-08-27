//! Owner-backed dynamic projections for the final async session facade.
//!
//! These values are intentionally thin wrappers around the root async
//! lifecycle handles. They erase only the closed completion marker; the owner
//! remains the sole authority for admission, deadlines, cancellation, and
//! physical settlement.

#![cfg(feature = "dyn-api")]

use std::{fmt, time::Duration};

use crate::{
    async_session::{AsyncCameraCore, Camera, Session},
    camera::{IdleWait, MotionQuery},
    capabilities::{Capabilities, TypedSupportSurface},
    completion::{AppliedOnly, Targeted},
    operation::{Cancellation, Operation},
    CameraId, CancellationOutcome, CompileTimeProfile, Error, Inquiry, OperationCommand,
    OperationId, PlainCommand, ProfileSpec, Result, StateCache,
};

use super::DynFuture;

/// A dynamically projected targeted operation.
///
/// This wrapper erases the runtime profile, transport, executor, and target
/// details while retaining the root [`Operation<Targeted>`] as its only
/// lifecycle implementation. Targeted operations additionally expose
/// physical-settlement waits.
///
/// Dropping this handle without resolving it is exactly `detach`: it
/// relinquishes observation and never stops hardware, as documented on
/// [`Operation`].
#[must_use = "await, cancel, or explicitly detach this dynamic operation"]
#[derive(Debug)]
pub struct DynTargetedOperation {
    inner: Operation<Targeted>,
}

impl DynTargetedOperation {
    pub(crate) fn from_operation(inner: Operation<Targeted>) -> Self {
        Self { inner }
    }

    /// Returns the operation's opaque owner-assigned identity.
    #[must_use]
    pub fn id(&self) -> OperationId {
        self.inner.id()
    }

    /// Waits for exact protocol application using the operation's configured
    /// observer deadline.
    pub async fn applied(self) -> Result<(), Error> {
        self.inner.applied().await
    }

    /// Waits for exact protocol application using an explicit observer
    /// deadline.
    pub async fn applied_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        self.inner.applied_with_timeout(timeout).await
    }

    /// Requests cancellation and returns its exact terminal observer.
    pub async fn cancel(self) -> Result<DynCancellation, Error> {
        self.inner
            .cancel()
            .await
            .map(DynCancellation::from_cancellation)
    }

    /// Waits for exact application and physical settling using the owner's
    /// prepared settlement plan.
    pub async fn settled(self) -> Result<(), Error> {
        self.inner.settled().await
    }

    /// Waits for exact application and physical settling using an explicit
    /// observer deadline. The owner retains all polling and deadline logic.
    pub async fn settled_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        self.inner.settled_with_timeout(timeout).await
    }

    /// Relinquishes observation without changing the operation's protocol
    /// state.
    pub fn detach(self) {
        self.inner.detach();
    }
}

/// A dynamically projected applied-only operation.
///
/// This type deliberately has no `settled` or `settled_with_timeout` method:
/// its structural API represents the closed applied-only completion kind.
///
/// Dropping this handle without resolving it is exactly `detach`: it
/// relinquishes observation and never stops hardware, as documented on
/// [`Operation`].
#[must_use = "await, cancel, or explicitly detach this dynamic operation"]
#[derive(Debug)]
pub struct DynAppliedOperation {
    inner: Operation<AppliedOnly>,
}

impl DynAppliedOperation {
    pub(crate) fn from_operation(inner: Operation<AppliedOnly>) -> Self {
        Self { inner }
    }

    /// Returns the operation's opaque owner-assigned identity.
    #[must_use]
    pub fn id(&self) -> OperationId {
        self.inner.id()
    }

    /// Waits for exact protocol application using the operation's configured
    /// observer deadline.
    pub async fn applied(self) -> Result<(), Error> {
        self.inner.applied().await
    }

    /// Waits for exact protocol application using an explicit observer
    /// deadline.
    pub async fn applied_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        self.inner.applied_with_timeout(timeout).await
    }

    /// Requests cancellation and returns its exact terminal observer.
    pub async fn cancel(self) -> Result<DynCancellation, Error> {
        self.inner
            .cancel()
            .await
            .map(DynCancellation::from_cancellation)
    }

    /// Relinquishes observation without changing the operation's protocol
    /// state.
    pub fn detach(self) {
        self.inner.detach();
    }
}

/// A dynamically projected cancellation observer.
///
/// The wrapped [`Cancellation`] retains the original operation's terminal
/// receiver, so cancellation outcomes remain race-accurate and are never
/// reconstructed by this dynamic layer.
#[must_use = "observe or explicitly detach this dynamic cancellation"]
#[derive(Debug)]
pub struct DynCancellation {
    inner: Cancellation,
}

impl DynCancellation {
    pub(crate) fn from_cancellation(inner: Cancellation) -> Self {
        Self { inner }
    }

    /// Observes the exact terminal cancellation outcome.
    pub async fn outcome(self, timeout: Duration) -> Result<CancellationOutcome, Error> {
        self.inner.outcome(timeout).await
    }

    /// Relinquishes cancellation observation without undoing cancellation
    /// intent.
    pub fn detach(self) {
        self.inner.detach();
    }
}

/// A small owner-backed dynamic camera view.
///
/// The view owns a clone of the session's owner handle and a validated runtime
/// [`ProfileSpec`]. It does not carry a legacy profile/transport/executor
/// generic and never constructs a legacy response-future handle.
#[must_use]
#[derive(Clone)]
pub struct DynSessionCamera {
    core: AsyncCameraCore,
}

impl fmt::Debug for DynSessionCamera {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DynSessionCamera")
            .field("target", &self.core.target())
            .field("profile", self.core.profile())
            .finish_non_exhaustive()
    }
}

impl DynSessionCamera {
    /// Creates a dynamic view from an owner-backed session camera.
    pub(crate) fn new(core: AsyncCameraCore) -> Self {
        Self { core }
    }

    /// Creates a dynamic view for the session's default target.
    pub fn from_session(session: &Session) -> Result<Self> {
        Ok(Self::new(session.camera_core()?))
    }

    /// Creates a dynamic view for one registered target.
    pub fn from_session_target(session: &Session, target: CameraId) -> Result<Self> {
        Ok(Self::new(session.camera_core_for(target)?))
    }

    /// Returns the fixed target selected by this view.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.core.target()
    }

    /// Returns this view's validated runtime profile facts.
    #[must_use]
    pub fn profile(&self) -> &ProfileSpec {
        self.core.profile()
    }

    /// Returns the validated runtime capability inventory for this target.
    #[must_use]
    pub fn capabilities(&self) -> &Capabilities {
        self.profile().capabilities()
    }

    /// Returns whether this target supports one typed static surface.
    #[must_use]
    pub fn supports_typed(&self, surface: TypedSupportSurface) -> bool {
        self.capabilities().supports_typed(surface)
    }

    /// Returns a cheap target-local read-only view of the owner's state cache.
    #[must_use]
    pub fn state_cache(&self) -> StateCache {
        self.core.state_cache()
    }

    /// Projects this dynamic view into a statically checked profile view.
    ///
    /// The stored runtime profile is compared in full before the typed view is
    /// returned; no owner or transport is created by this projection.
    pub fn camera<P>(&self) -> Result<Camera<P>>
    where
        P: CompileTimeProfile,
    {
        self.core.clone().into_typed::<P>()
    }

    /// Executes a plain command through the shared owner.
    pub async fn execute<C>(&self, command: &C) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute(command).await
    }

    /// Sends a typed inquiry through the shared owner.
    pub async fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire(inquiry).await
    }

    /// Submits any typed targeted request through the same preparation and
    /// owner admission path as [`Camera::submit`].
    pub async fn submit_targeted<O>(&self, operation: &O) -> Result<DynTargetedOperation, Error>
    where
        O: OperationCommand<Targeted> + Sync + ?Sized,
    {
        self.core
            .submit::<Targeted, O>(operation)
            .await
            .map(DynTargetedOperation::from_operation)
    }

    /// Submits any typed applied-only request through the same preparation and
    /// owner admission path as [`Camera::submit`].
    pub async fn submit_applied<O>(&self, operation: &O) -> Result<DynAppliedOperation, Error>
    where
        O: OperationCommand<AppliedOnly> + Sync + ?Sized,
    {
        self.core
            .submit::<AppliedOnly, O>(operation)
            .await
            .map(DynAppliedOperation::from_operation)
    }

    /// Returns the canonical owner-backed motion safety and observation view.
    #[must_use]
    pub fn motion(&self) -> &dyn super::nouns::DynMotion {
        self
    }

    /// Exposes the one erased core to crate-local dynamic noun modules.
    ///
    /// The core is borrowed rather than cloned here so noun accessors cannot
    /// introduce another owner or registry.  They must continue to route
    /// preparation and admission through its crate-private methods.
    pub(crate) fn core(&self) -> &AsyncCameraCore {
        &self.core
    }

    /// Stops all motion by delegating to the owner-backed camera view.
    pub(crate) fn stop_all_motion(&self) -> DynFuture<'_, Result<(), Error>> {
        Box::pin(self.core.stop_all_motion())
    }

    /// Observes exactly the selected motion axes through the owner-backed
    /// camera view.
    pub(crate) fn is_moving(&self, query: MotionQuery) -> DynFuture<'_, Result<bool, Error>> {
        Box::pin(self.core.is_moving(query))
    }

    /// Waits for exactly the selected motion axes through the owner-backed
    /// camera view.
    pub(crate) fn wait_until_idle(&self, wait: IdleWait) -> DynFuture<'_, Result<(), Error>> {
        Box::pin(self.core.wait_until_idle(wait))
    }
}

/// Object-safe root view over one owner-backed dynamic camera.
///
/// The noun and custom-operation supertraits expose the complete dynamic
/// surface while this root trait contributes only target/profile facts.  All
/// of those views are backed by the same owner and registry.
pub trait DynSessionCameraControl:
    super::nouns::DynSessionCameraNouns + super::custom::DynCustomOperations
{
    /// Returns the fixed target selected by this view.
    fn target(&self) -> CameraId;

    /// Returns the validated runtime profile facts for this target.
    fn profile(&self) -> &ProfileSpec;

    /// Returns the validated runtime capability inventory for this target.
    fn capabilities(&self) -> &Capabilities;

    /// Returns whether this target supports one typed static surface.
    fn supports_typed(&self, surface: TypedSupportSurface) -> bool;

    /// Returns a target-local read-only state-cache view.
    fn state_cache(&self) -> StateCache;
}

impl DynSessionCameraControl for DynSessionCamera {
    fn target(&self) -> CameraId {
        Self::target(self)
    }

    fn profile(&self) -> &ProfileSpec {
        Self::profile(self)
    }

    fn capabilities(&self) -> &Capabilities {
        Self::capabilities(self)
    }

    fn supports_typed(&self, surface: TypedSupportSurface) -> bool {
        Self::supports_typed(self, surface)
    }

    fn state_cache(&self) -> StateCache {
        Self::state_cache(self)
    }
}
