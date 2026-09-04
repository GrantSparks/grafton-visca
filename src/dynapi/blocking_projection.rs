//! Native blocking runtime-profile projection.

use std::fmt;

use crate::{
    blocking::{BlockingCameraCore, Camera, Operation, Session},
    camera::{IdleWait, MotionQuery},
    capabilities::{Capabilities, TypedSupportSurface},
    completion, CameraId, CompileTimeProfile, Error, Inquiry, OperationCommand, PlainCommand,
    ProfileSpec, Result, StateCache, SubmissionClass,
};

/// An owner-backed blocking camera view for a runtime [`ProfileSpec`].
///
/// This view borrows the same caller-thread owner as a typed blocking
/// [`Camera`]. It erases only the compile-time profile marker: commands,
/// inquiries, and operations still use their ordinary typed request values and
/// are validated against the stored runtime profile before any wire I/O.
#[must_use]
#[derive(Clone, Copy)]
pub struct BlockingDynSessionCamera<'session> {
    core: BlockingCameraCore<'session>,
}

impl fmt::Debug for BlockingDynSessionCamera<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BlockingDynSessionCamera")
            .field("target", &self.core.target())
            .field("profile", self.core.profile())
            .finish_non_exhaustive()
    }
}

impl<'session> BlockingDynSessionCamera<'session> {
    pub(crate) const fn new(core: BlockingCameraCore<'session>) -> Self {
        Self { core }
    }

    /// Creates a runtime-profile view for the session's sole target.
    pub fn from_session(session: &'session Session) -> Result<Self> {
        session.camera_dyn()
    }

    /// Creates a runtime-profile view for one registered target.
    pub fn from_session_target(session: &'session Session, target: CameraId) -> Result<Self> {
        session.camera_dyn_for(target)
    }

    /// Returns this view's fixed camera target.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.core.target()
    }

    /// Returns this view's validated runtime profile facts.
    #[must_use]
    pub const fn profile(&self) -> &ProfileSpec {
        self.core.profile()
    }

    /// Returns this view's validated runtime capability inventory.
    #[must_use]
    pub const fn capabilities(&self) -> &Capabilities {
        self.profile().capabilities()
    }

    /// Returns whether this profile permits one optional typed surface.
    #[must_use]
    pub fn supports_typed(&self, surface: TypedSupportSurface) -> bool {
        self.capabilities().supports_typed(surface)
    }

    /// Returns a cheap target-local read-only state-cache view.
    #[must_use]
    pub fn state_cache(&self) -> StateCache {
        self.core.state_cache()
    }

    /// Projects this runtime view into a statically checked profile view.
    ///
    /// Every stored profile fact must match the compile-time projection for
    /// `P`; no owner or transport is created by this conversion.
    pub fn camera<P>(&self) -> Result<Camera<'session, P>>
    where
        P: CompileTimeProfile,
    {
        self.core.into_typed::<P>()
    }

    /// Returns this view's ordinary-work submission-class default.
    #[must_use]
    pub const fn submission_class(&self) -> Option<SubmissionClass> {
        self.core.submission_class()
    }

    /// Derives a runtime-profile view whose ordinary work uses `class`.
    ///
    /// The original view is unchanged, and intrinsically urgent stops remain
    /// urgent through the returned view.
    pub fn with_submission_class(&self, class: SubmissionClass) -> Self {
        let mut selected = *self;
        selected.set_submission_class(Some(class));
        selected
    }

    /// Sets this view's ordinary-work submission-class default.
    ///
    /// Intrinsically urgent requests are never demoted.
    pub fn set_submission_class(&mut self, class: Option<SubmissionClass>) {
        self.core.set_submission_class(class);
    }

    /// Executes a plain command through the shared blocking owner.
    pub fn execute<C>(&self, command: &C) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute(command)
    }

    /// Sends a typed inquiry through the shared blocking owner.
    pub fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire(inquiry)
    }

    /// Admits a typed operation and returns its native blocking handle.
    pub fn submit<K, O>(&self, operation: &O) -> Result<Operation<'session, K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit(operation)
    }

    /// Stops every profile-supported motion axis through the shared owner.
    pub fn stop_all_motion(&self) -> Result<(), Error> {
        self.core.stop_all_motion()
    }

    /// Observes movement on the selected profile-supported axes.
    pub fn is_moving(&self, query: MotionQuery) -> Result<bool, Error> {
        self.core.is_moving(query)
    }

    /// Waits for the selected profile-supported axes to become idle.
    pub fn wait_until_idle(&self, wait: IdleWait) -> Result<(), Error> {
        self.core.wait_until_idle(wait)
    }
}
