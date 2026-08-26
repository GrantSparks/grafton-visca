//! Object-safe custom operation requests for the owner-backed dynamic API.
//!
//! The public marker traits in this module intentionally contain no second
//! lifecycle model.  A downstream request implements the canonical static
//! [`crate::OperationCommand`] contract once and receives dynamic support
//! through a blanket implementation.  The private adapters below only make
//! the object-safe view usable by the existing generic preparation path.

#![cfg(feature = "dyn-api")]

use crate::{
    async_session::AsyncCameraCore,
    completion::{AppliedOnly, Targeted},
    dynapi::{DynAppliedOperation, DynFuture, DynSessionCamera, DynTargetedOperation},
    raw::MAX_BYTES,
    request,
    requests::{AppliedStateAuthority, EncodeError},
    AffectedAxes, CameraId, ControlClass, Error, OperationCommand, Request, RetryClass,
    TimeoutClass,
};

mod private {
    use super::*;

    /// Object-safe projection of the canonical targeted-request contract.
    pub(crate) trait TargetedRequest: Send + Sync {
        fn timeout_class(&self) -> TimeoutClass;

        fn retry_class(&self) -> RetryClass;

        fn control_class(&self) -> ControlClass;

        fn encoded_size(&self) -> usize;

        fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError>;

        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error>;

        fn applied_state_projection(
            &self,
            authority: AppliedStateAuthority,
        ) -> Option<crate::runtime::engine::AppliedStateProjection>;

        fn affected_axes(&self) -> AffectedAxes;
    }

    /// Object-safe projection of the canonical applied-only request contract.
    pub(crate) trait AppliedRequest: Send + Sync {
        fn timeout_class(&self) -> TimeoutClass;

        fn retry_class(&self) -> RetryClass;

        fn control_class(&self) -> ControlClass;

        fn encoded_size(&self) -> usize;

        fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError>;

        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error>;

        fn applied_state_projection(
            &self,
            authority: AppliedStateAuthority,
        ) -> Option<crate::runtime::engine::AppliedStateProjection>;

        fn affected_axes(&self) -> AffectedAxes;
    }

    /// Seals the public targeted marker to canonical targeted operation types.
    pub(crate) trait TargetedSealed {}

    impl<T: ?Sized> TargetedSealed for T where T: OperationCommand<Targeted> {}

    /// Seals the public applied-only marker to canonical applied-only operation
    /// types.
    pub(crate) trait AppliedSealed {}

    impl<T: ?Sized> AppliedSealed for T where T: OperationCommand<AppliedOnly> {}

    impl<T: ?Sized> TargetedRequest for T
    where
        T: OperationCommand<Targeted>,
    {
        fn timeout_class(&self) -> TimeoutClass {
            Request::timeout_class(self)
        }

        fn retry_class(&self) -> RetryClass {
            Request::retry_class(self)
        }

        fn control_class(&self) -> ControlClass {
            Request::control_class(self)
        }

        fn encoded_size(&self) -> usize {
            Request::encoded_size(self)
        }

        fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError> {
            Request::write_into(self, camera, buffer)
        }

        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
            Request::validate_for_profile(self, profile)
        }

        fn applied_state_projection(
            &self,
            authority: AppliedStateAuthority,
        ) -> Option<crate::runtime::engine::AppliedStateProjection> {
            Request::applied_state_projection(self, authority)
        }

        fn affected_axes(&self) -> AffectedAxes {
            OperationCommand::affected_axes(self)
        }
    }

    impl<T: ?Sized> AppliedRequest for T
    where
        T: OperationCommand<AppliedOnly>,
    {
        fn timeout_class(&self) -> TimeoutClass {
            Request::timeout_class(self)
        }

        fn retry_class(&self) -> RetryClass {
            Request::retry_class(self)
        }

        fn control_class(&self) -> ControlClass {
            Request::control_class(self)
        }

        fn encoded_size(&self) -> usize {
            Request::encoded_size(self)
        }

        fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError> {
            Request::write_into(self, camera, buffer)
        }

        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
            Request::validate_for_profile(self, profile)
        }

        fn applied_state_projection(
            &self,
            authority: AppliedStateAuthority,
        ) -> Option<crate::runtime::engine::AppliedStateProjection> {
            Request::applied_state_projection(self, authority)
        }

        fn affected_axes(&self) -> AffectedAxes {
            OperationCommand::affected_axes(self)
        }
    }
}

/// A sealed object-safe view of any canonical targeted operation request.
///
/// Implementing [`crate::OperationCommand<Targeted>`] is the only requirement;
/// the blanket implementation supplies this dynamic marker automatically.
/// The request retains its canonical policy, profile validation, encoding, and
/// affected-axis behavior when submitted through [`submit_targeted`].
#[allow(private_bounds)]
pub trait DynTargetedRequest: private::TargetedSealed + private::TargetedRequest {}

impl<T: ?Sized> DynTargetedRequest for T where T: OperationCommand<Targeted> {}

/// A sealed object-safe view of any canonical applied-only operation request.
///
/// Implementing [`crate::OperationCommand<AppliedOnly>`] is the only
/// requirement; the blanket implementation supplies this dynamic marker
/// automatically.  Applied-only requests never gain a settled lifecycle by
/// passing through this view.
#[allow(private_bounds)]
pub trait DynAppliedRequest: private::AppliedSealed + private::AppliedRequest {}

impl<T: ?Sized> DynAppliedRequest for T where T: OperationCommand<AppliedOnly> {}

/// Object-safe custom operation submission through the owner-backed dynamic
/// camera.
///
/// The request traits retain the canonical static operation contract; this
/// trait only erases the request type at the boundary and returns the same
/// dynamic operation handles as the built-in noun surface.  It does not add a
/// second lifecycle or a per-call timeout.
pub trait DynCustomOperations: Send + Sync {
    /// Submits a custom targeted request through the canonical owner.
    fn submit_targeted<'a>(
        &'a self,
        request: &'a dyn DynTargetedRequest,
    ) -> DynFuture<'a, Result<DynTargetedOperation, Error>>;

    /// Submits a custom applied-only request through the canonical owner.
    fn submit_applied<'a>(
        &'a self,
        request: &'a dyn DynAppliedRequest,
    ) -> DynFuture<'a, Result<DynAppliedOperation, Error>>;
}

struct TargetedRequestAdapter<'a>(&'a dyn DynTargetedRequest);

impl Request for TargetedRequestAdapter<'_> {
    type Class = request::Operation<Targeted>;

    // Dynamic policy is delegated through instance methods below.  These
    // constants satisfy the canonical trait's fixed-type requirements while
    // never overriding the request's selected values.
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        private::TargetedRequest::timeout_class(self.0)
    }

    fn retry_class(&self) -> RetryClass {
        private::TargetedRequest::retry_class(self.0)
    }

    fn control_class(&self) -> ControlClass {
        private::TargetedRequest::control_class(self.0)
    }

    fn encoded_size(&self) -> usize {
        private::TargetedRequest::encoded_size(self.0)
    }

    fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError> {
        private::TargetedRequest::write_into(self.0, camera, buffer)
    }

    fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        private::TargetedRequest::validate_for_profile(self.0, profile)
    }

    fn applied_state_projection(
        &self,
        authority: AppliedStateAuthority,
    ) -> Option<crate::runtime::engine::AppliedStateProjection> {
        private::TargetedRequest::applied_state_projection(self.0, authority)
    }
}

impl OperationCommand<Targeted> for TargetedRequestAdapter<'_> {
    fn affected_axes(&self) -> AffectedAxes {
        private::TargetedRequest::affected_axes(self.0)
    }
}

struct AppliedRequestAdapter<'a>(&'a dyn DynAppliedRequest);

impl Request for AppliedRequestAdapter<'_> {
    type Class = request::Operation<AppliedOnly>;

    // See the targeted adapter for why these constants are inert fallbacks.
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        private::AppliedRequest::timeout_class(self.0)
    }

    fn retry_class(&self) -> RetryClass {
        private::AppliedRequest::retry_class(self.0)
    }

    fn control_class(&self) -> ControlClass {
        private::AppliedRequest::control_class(self.0)
    }

    fn encoded_size(&self) -> usize {
        private::AppliedRequest::encoded_size(self.0)
    }

    fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError> {
        private::AppliedRequest::write_into(self.0, camera, buffer)
    }

    fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        private::AppliedRequest::validate_for_profile(self.0, profile)
    }

    fn applied_state_projection(
        &self,
        authority: AppliedStateAuthority,
    ) -> Option<crate::runtime::engine::AppliedStateProjection> {
        private::AppliedRequest::applied_state_projection(self.0, authority)
    }
}

impl OperationCommand<AppliedOnly> for AppliedRequestAdapter<'_> {
    fn affected_axes(&self) -> AffectedAxes {
        private::AppliedRequest::affected_axes(self.0)
    }
}

/// Submits one object-safe targeted request through the canonical owner path.
///
/// Preparation, profile validation, encoding, admission, cancellation, and
/// physical settling remain in the same implementation used by static camera
/// views.  This helper only erases the request type and maps the resulting
/// canonical handle to [`DynTargetedOperation`].
pub fn submit_targeted<'a>(
    camera: &'a DynSessionCamera,
    request: &'a dyn DynTargetedRequest,
) -> DynFuture<'a, Result<DynTargetedOperation, Error>> {
    submit_targeted_with_core(camera.core(), request)
}

/// Internal targeted helper kept separate so future noun modules can compose
/// with the core without introducing another dynamic owner.
fn submit_targeted_with_core<'a>(
    core: &'a AsyncCameraCore,
    request: &'a dyn DynTargetedRequest,
) -> DynFuture<'a, Result<DynTargetedOperation, Error>> {
    Box::pin(async move {
        let adapter = TargetedRequestAdapter(request);
        core.submit::<Targeted, _>(&adapter)
            .await
            .map(DynTargetedOperation::from_operation)
    })
}

/// Submits one object-safe applied-only request through the canonical owner
/// path.
///
/// Applied-only requests retain the exact applied/detach/cancel lifecycle and
/// cannot acquire a settled operation through this helper.
pub fn submit_applied<'a>(
    camera: &'a DynSessionCamera,
    request: &'a dyn DynAppliedRequest,
) -> DynFuture<'a, Result<DynAppliedOperation, Error>> {
    submit_applied_with_core(camera.core(), request)
}

impl DynCustomOperations for DynSessionCamera {
    fn submit_targeted<'a>(
        &'a self,
        request: &'a dyn DynTargetedRequest,
    ) -> DynFuture<'a, Result<DynTargetedOperation, Error>> {
        submit_targeted(self, request)
    }

    fn submit_applied<'a>(
        &'a self,
        request: &'a dyn DynAppliedRequest,
    ) -> DynFuture<'a, Result<DynAppliedOperation, Error>> {
        submit_applied(self, request)
    }
}

/// Internal applied-only helper kept separate so future noun modules can
/// compose with the core without introducing another dynamic owner.
fn submit_applied_with_core<'a>(
    core: &'a AsyncCameraCore,
    request: &'a dyn DynAppliedRequest,
) -> DynFuture<'a, Result<DynAppliedOperation, Error>> {
    Box::pin(async move {
        let adapter = AppliedRequestAdapter(request);
        core.submit::<AppliedOnly, _>(&adapter)
            .await
            .map(DynAppliedOperation::from_operation)
    })
}
