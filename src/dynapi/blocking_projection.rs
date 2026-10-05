//! Native blocking runtime-profile projection.

use std::fmt;

use crate::{
    blocking::{BlockingCameraCore, Camera, Operation},
    completion, CompileTimeProfile, Error, Inquiry, OperationCommand, PlainCommand, Result,
};

/// An owner-backed blocking camera view for a runtime [`ProfileSpec`](crate::ProfileSpec).
///
/// This view shares the same owner worker as a typed blocking [`Camera`] and,
/// like it, is `Clone + Send + Sync` and borrows nothing. It erases only the
/// compile-time profile marker: commands,
/// inquiries, and operations still use their ordinary typed request values and
/// are validated against the stored runtime profile before any wire I/O.
#[must_use]
#[derive(Clone)]
pub struct BlockingDynSessionCamera {
    core: BlockingCameraCore,
}

impl fmt::Debug for BlockingDynSessionCamera {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BlockingDynSessionCamera")
            .field("target", &self.core.target())
            .field("profile", self.core.profile())
            .finish_non_exhaustive()
    }
}

impl BlockingDynSessionCamera {
    pub(crate) const fn new(core: BlockingCameraCore) -> Self {
        Self { core }
    }

    crate::camera_view::camera_view_getters!(runtime_profile);

    /// Projects this runtime view into a statically checked profile view.
    ///
    /// Every stored profile fact must match the compile-time projection for
    /// `P`; no owner or transport is created by this conversion.
    pub fn camera<P>(&self) -> Result<Camera<P>>
    where
        P: CompileTimeProfile,
    {
        self.core.clone().into_typed::<P>()
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
    pub fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit(operation)
    }

    /// Returns the separate motion safety and observation view, the same
    /// view a typed blocking [`Camera`] returns.
    pub fn motion(&self) -> crate::blocking::MotionAccessor<'_> {
        crate::blocking::MotionAccessor::new(&self.core)
    }
}
