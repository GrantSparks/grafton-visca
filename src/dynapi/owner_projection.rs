//! Owner-backed dynamic projections for the final async session facade.
//!
//! These values are intentionally thin wrappers around the root async
//! lifecycle handles. They erase only the closed completion marker; the owner
//! remains the sole authority for admission, deadlines, cancellation, and
//! profile-selected protocol settlement.

#![cfg(feature = "dyn-api")]

use std::{fmt, time::Duration};

use crate::{
    async_session::{AsyncCameraCore, Camera},
    capabilities::{Capabilities, TypedSupportSurface},
    completion::{AppliedOnly, Targeted},
    operation::Operation,
    CameraId, CancellationOutcome, CompileTimeProfile, Error, Inquiry, OperationCommand,
    OperationId, PlainCommand, ProfileSpec, Result, StateCache,
};

/// The lifecycle methods every dynamic operation handle shares with the root
/// [`Operation`] it wraps. Each projection adds only what its completion kind
/// allows, so the dynamic layer never re-implements lifecycle behaviour.
macro_rules! dyn_operation_lifecycle {
    ($handle:ident, $kind:ty) => {
        impl DynKind for $kind {
            type Handle = $handle;

            fn erase(inner: Operation<Self>) -> $handle {
                $handle { inner }
            }
        }

        impl $handle {
            /// Returns the operation's opaque owner-assigned identity.
            #[must_use]
            pub fn id(&self) -> OperationId {
                self.inner.id()
            }

            /// Waits for exact protocol application, bounded by the
            /// operation's configured observer deadline. See
            /// [`Operation::applied`].
            pub async fn applied(&mut self) -> Result<(), Error> {
                self.inner.applied().await
            }

            /// Waits for exact protocol application, bounded by `timeout`.
            /// See [`Operation::applied_with_timeout`].
            pub async fn applied_with_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
                self.inner.applied_with_timeout(timeout).await
            }

            /// Cancels the operation and waits for the cancellation's
            /// conclusion. Idempotent; a refusal leaves the operation running
            /// and this handle unaffected. See [`Operation::cancel`].
            pub async fn cancel(&mut self) -> Result<CancellationOutcome, Error> {
                self.inner.cancel().await
            }

            /// [`cancel`](Self::cancel), bounded by `timeout`. See
            /// [`Operation::cancel_with_timeout`].
            pub async fn cancel_with_timeout(
                &mut self,
                timeout: Duration,
            ) -> Result<CancellationOutcome, Error> {
                self.inner.cancel_with_timeout(timeout).await
            }

            /// Relinquishes observation without changing the operation's
            /// protocol state.
            pub fn detach(self) {
                self.inner.detach();
            }
        }
    };
}

/// A completion kind and the dynamic handle that erases its root
/// [`Operation`], so the erased submission paths are written once over the
/// kind (#817).
pub(crate) trait DynKind: crate::completion::Kind + Sized {
    /// The dynamic handle for an operation of this kind.
    type Handle;

    /// Wraps an admitted root operation in its dynamic handle.
    fn erase(inner: Operation<Self>) -> Self::Handle;
}

/// A dynamically projected targeted operation.
///
/// This wrapper erases the runtime profile, transport, executor, and target
/// details while retaining the root [`Operation<Targeted>`] as its only
/// lifecycle implementation. Targeted operations additionally expose
/// profile-selected protocol-settlement waits. Waits borrow the handle and
/// cache their result, as documented on [`Operation`].
///
/// Dropping this handle is exactly `detach`: it relinquishes observation and
/// never stops hardware, as documented on [`Operation`].
#[must_use = "await, cancel, or explicitly detach this dynamic operation"]
#[derive(Debug)]
pub struct DynTargetedOperation {
    inner: Operation<Targeted>,
}

dyn_operation_lifecycle!(DynTargetedOperation, Targeted);

impl DynTargetedOperation {
    /// Waits for exact application and the owner's profile-selected protocol
    /// settlement condition. See [`Operation::settled`].
    pub async fn settled(&mut self) -> Result<crate::Settlement, Error> {
        self.inner.settled().await
    }

    /// Waits for exact application and the profile-selected protocol
    /// settlement condition, bounded by `timeout`. The owner retains all
    /// polling and deadline logic. See [`Operation::settled_with_timeout`].
    pub async fn settled_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<crate::Settlement, Error> {
        self.inner.settled_with_timeout(timeout).await
    }
}

/// A dynamically projected applied-only operation.
///
/// This type deliberately has no `settled` or `settled_with_timeout` method:
/// its structural API represents the closed applied-only completion kind.
/// Waits borrow the handle and cache their result, as documented on
/// [`Operation`].
///
/// Dropping this handle is exactly `detach`: it relinquishes observation and
/// never stops hardware, as documented on [`Operation`].
#[must_use = "await, cancel, or explicitly detach this dynamic operation"]
#[derive(Debug)]
pub struct DynAppliedOperation {
    inner: Operation<AppliedOnly>,
}

dyn_operation_lifecycle!(DynAppliedOperation, AppliedOnly);

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

    crate::camera_view::camera_view_getters!(runtime_profile);

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
    ///
    /// Ordinary commands wait for their terminal protocol application. A raw
    /// [`crate::raw::RawReplyShape::NoReply`] command instead succeeds once its
    /// local transport write succeeds; it does not claim camera application.
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
        self.submit_erased(operation).await
    }

    /// Submits any typed applied-only request through the same preparation and
    /// owner admission path as [`Camera::submit`].
    pub async fn submit_applied<O>(&self, operation: &O) -> Result<DynAppliedOperation, Error>
    where
        O: OperationCommand<AppliedOnly> + Sync + ?Sized,
    {
        self.submit_erased(operation).await
    }

    /// Submits `operation` through the typed admission path and erases the
    /// returned root handle into its kind's dynamic handle.
    pub(crate) async fn submit_erased<K, O>(&self, operation: &O) -> Result<K::Handle, Error>
    where
        K: DynKind,
        O: OperationCommand<K> + Sync + ?Sized,
    {
        self.core.submit::<K, O>(operation).await.map(K::erase)
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
