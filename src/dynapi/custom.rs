//! Object-safe custom operation requests for the owner-backed dynamic API.
//!
//! The public marker traits in this module intentionally contain no second
//! lifecycle model.  A downstream request implements the canonical static
//! [`crate::OperationCommand`] contract once and receives dynamic support
//! through a blanket implementation.  The private adapters below only make
//! the object-safe view usable by the existing generic preparation path.

#![cfg(feature = "dyn-api")]

use std::marker::PhantomData;

use crate::{
    completion::{self, AppliedOnly, Targeted},
    dynapi::{
        owner_projection::DynKind, DynAppliedOperation, DynFuture, DynSessionCamera,
        DynTargetedOperation,
    },
    raw::{RawReplyShape, MAX_BYTES},
    request,
    requests::{AppliedStateAuthority, EncodeError, RequestContractAuthority},
    AffectedAxes, CameraId, ControlClass, Error, OperationCommand, Request, RetryClass,
    TimeoutClass,
};

mod private {
    use super::*;

    /// Object-safe projection of the canonical operation-request contract of
    /// one completion kind `K`.
    pub(crate) trait ErasedOperation<K: completion::Kind>: Send + Sync {
        fn timeout_class(&self) -> TimeoutClass;

        fn retry_class(&self) -> RetryClass;

        fn control_class(&self) -> ControlClass;

        fn admission_control_class(&self) -> Result<ControlClass, Error>;

        fn declared_max_size(&self) -> usize;

        fn reply_shape(&self) -> RawReplyShape;

        fn encoded_size(&self) -> usize;

        fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError>;

        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error>;

        fn applied_state_projection(
            &self,
            authority: AppliedStateAuthority,
        ) -> Option<crate::runtime::engine::AppliedStateProjection>;

        fn affected_axes(&self) -> AffectedAxes;
    }

    /// Seals a public request marker to canonical operation types of kind
    /// `K`.
    pub(crate) trait Sealed<K: completion::Kind> {}

    impl<K: completion::Kind, T: ?Sized> Sealed<K> for T where T: OperationCommand<K> {}

    impl<K: completion::Kind, T: ?Sized> ErasedOperation<K> for T
    where
        T: OperationCommand<K>,
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

        fn admission_control_class(&self) -> Result<ControlClass, Error> {
            crate::requests::admission_control_class(self)
        }

        fn declared_max_size(&self) -> usize {
            T::MAX_SIZE
        }

        fn reply_shape(&self) -> RawReplyShape {
            Request::reply_shape(self)
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
pub trait DynTargetedRequest:
    private::Sealed<Targeted> + private::ErasedOperation<Targeted>
{
}

impl<T: ?Sized> DynTargetedRequest for T where T: OperationCommand<Targeted> {}

/// A sealed object-safe view of any canonical applied-only operation request.
///
/// Implementing [`crate::OperationCommand<AppliedOnly>`] is the only
/// requirement; the blanket implementation supplies this dynamic marker
/// automatically.  Applied-only requests never gain a settled lifecycle by
/// passing through this view.
#[allow(private_bounds)]
pub trait DynAppliedRequest:
    private::Sealed<AppliedOnly> + private::ErasedOperation<AppliedOnly>
{
}

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

/// The canonical request a type-erased operation request of kind `K` is
/// submitted as.
struct OperationAdapter<'a, K, D: ?Sized> {
    request: &'a D,
    kind: PhantomData<fn() -> K>,
}

impl<'a, K, D: ?Sized> OperationAdapter<'a, K, D> {
    const fn new(request: &'a D) -> Self {
        Self {
            request,
            kind: PhantomData,
        }
    }
}

impl<K, D> Request for OperationAdapter<'_, K, D>
where
    K: completion::Kind,
    D: private::ErasedOperation<K> + ?Sized,
{
    type Class = request::Operation<K>;

    // Dynamic policy is delegated through instance methods below. These
    // constants provide the canonical global allocation fallback; the hidden
    // size/control hooks project the original concrete request's declaration
    // and urgent authority without granting either to an erased adapter.
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        self.request.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.request.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.request.control_class()
    }

    #[allow(private_interfaces)]
    fn admission_control_class(
        &self,
        _authority: RequestContractAuthority,
    ) -> Result<ControlClass, Error> {
        self.request.admission_control_class()
    }

    #[allow(private_interfaces)]
    fn declared_max_size(&self, _authority: RequestContractAuthority) -> usize {
        self.request.declared_max_size()
    }

    fn reply_shape(&self) -> RawReplyShape {
        self.request.reply_shape()
    }

    fn encoded_size(&self) -> usize {
        self.request.encoded_size()
    }

    fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, EncodeError> {
        self.request.write_into(camera, buffer)
    }

    fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        self.request.validate_for_profile(profile)
    }

    fn applied_state_projection(
        &self,
        authority: AppliedStateAuthority,
    ) -> Option<crate::runtime::engine::AppliedStateProjection> {
        self.request.applied_state_projection(authority)
    }
}

impl<K, D> OperationCommand<K> for OperationAdapter<'_, K, D>
where
    K: completion::Kind,
    D: private::ErasedOperation<K> + ?Sized,
{
    fn affected_axes(&self) -> AffectedAxes {
        self.request.affected_axes()
    }
}

/// Submits one object-safe request of kind `K` through the canonical owner
/// path.
///
/// Preparation, profile validation, encoding, admission, cancellation, and
/// the kind's completion lifecycle remain in the implementation the static
/// camera views use; this only erases the request type and maps the root
/// handle to the kind's dynamic handle.
fn submit_erased<'a, K, D>(
    camera: &'a DynSessionCamera,
    request: &'a D,
) -> DynFuture<'a, Result<K::Handle, Error>>
where
    K: DynKind,
    D: private::ErasedOperation<K> + ?Sized,
{
    Box::pin(async move { camera.submit_erased(&OperationAdapter::new(request)).await })
}

/// Submits one object-safe targeted request through the canonical owner path.
///
/// Preparation, profile validation, encoding, admission, cancellation, and
/// profile-selected protocol settlement remain in the same implementation used
/// by static camera views. This helper only erases the request type and maps
/// the resulting canonical handle to [`DynTargetedOperation`].
pub fn submit_targeted<'a>(
    camera: &'a DynSessionCamera,
    request: &'a dyn DynTargetedRequest,
) -> DynFuture<'a, Result<DynTargetedOperation, Error>> {
    submit_erased::<Targeted, _>(camera, request)
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
    submit_erased::<AppliedOnly, _>(camera, request)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        raw::RawReplyShape,
        request::builtin::ZoomStop,
        runtime::engine::{ControlClass as EngineControlClass, ReplyShape},
        CameraId, OperationalTuning, ProfileSpec, SubmissionClass,
    };

    fn profile() -> Option<ProfileSpec> {
        let result = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>();
        assert!(result.is_ok(), "generic profile must construct");
        result.ok()
    }

    fn write_frame(target: CameraId, buffer: &mut [u8]) -> crate::Result<usize> {
        buffer[..4].copy_from_slice(&[target.to_address_byte(), 0x01, 0x02, 0xff]);
        Ok(4)
    }

    struct CompletionOnlyTargeted;

    impl Request for CompletionOnlyTargeted {
        type Class = request::Operation<Targeted>;

        const MAX_SIZE: usize = 4;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn reply_shape(&self) -> RawReplyShape {
            RawReplyShape::CompletionOnly
        }

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> crate::Result<usize> {
            write_frame(target, buffer)
        }
    }

    impl OperationCommand<Targeted> for CompletionOnlyTargeted {
        fn affected_axes(&self) -> AffectedAxes {
            AffectedAxes::ZOOM
        }
    }

    struct OversizedTargeted;

    impl Request for OversizedTargeted {
        type Class = request::Operation<Targeted>;

        const MAX_SIZE: usize = 3;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn encoded_size(&self) -> usize {
            4
        }

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> crate::Result<usize> {
            write_frame(target, buffer)
        }
    }

    impl OperationCommand<Targeted> for OversizedTargeted {
        fn affected_axes(&self) -> AffectedAxes {
            AffectedAxes::ZOOM
        }
    }

    macro_rules! applied_request {
        ($name:ident, $max_size:expr, $encoded_size:expr, $reply_shape:expr, $control:expr) => {
            struct $name;

            impl Request for $name {
                type Class = request::Operation<AppliedOnly>;

                const MAX_SIZE: usize = $max_size;
                const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
                const RETRY_CLASS: RetryClass = RetryClass::Never;
                const CONTROL_CLASS: ControlClass = $control;

                fn reply_shape(&self) -> RawReplyShape {
                    $reply_shape
                }

                fn encoded_size(&self) -> usize {
                    $encoded_size
                }

                fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> crate::Result<usize> {
                    write_frame(target, buffer)
                }
            }

            impl OperationCommand<AppliedOnly> for $name {
                fn affected_axes(&self) -> AffectedAxes {
                    AffectedAxes::ZOOM
                }
            }
        };
    }

    applied_request!(
        NoReplyApplied,
        4,
        4,
        RawReplyShape::NoReply,
        ControlClass::Normal
    );
    applied_request!(
        OversizedApplied,
        3,
        4,
        RawReplyShape::AckThenCompletion,
        ControlClass::Normal
    );
    applied_request!(
        DownstreamUrgentApplied,
        4,
        4,
        RawReplyShape::AckThenCompletion,
        ControlClass::Urgent
    );

    #[test]
    fn erased_custom_reply_shapes_preserve_operation_lifecycle_contract() {
        let Some(profile) = profile() else {
            return;
        };
        let targeted = CompletionOnlyTargeted;
        let targeted: &dyn DynTargetedRequest = &targeted;
        let targeted_adapter = OperationAdapter::<Targeted, _>::new(targeted);
        let targeted_prepared = crate::prepared::prepare_operation::<Targeted, _>(
            &targeted_adapter,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        );
        assert!(
            targeted_prepared.is_ok(),
            "completion-only custom request prepares through erasure"
        );
        let Ok(targeted_prepared) = targeted_prepared else {
            return;
        };
        let targeted_context = targeted_prepared.admit_with(|request, _, _, _| *request.context());
        assert_eq!(targeted_context.reply_shape, ReplyShape::CompletionOnly);

        let applied = NoReplyApplied;
        let applied: &dyn DynAppliedRequest = &applied;
        let applied_adapter = OperationAdapter::<AppliedOnly, _>::new(applied);
        assert!(
            matches!(
                crate::prepared::prepare_operation::<AppliedOnly, _>(
                    &applied_adapter,
                    CameraId::CAMERA_1,
                    &profile,
                    OperationalTuning::new(),
                    crate::prepared::ClassSelection::Request,
                ),
                Err(Error::InvalidRequest(_))
            ),
            "no-reply custom operation must be rejected through erasure"
        );
    }

    #[test]
    fn erased_requests_enforce_their_original_max_size() {
        let Some(profile) = profile() else {
            return;
        };
        let targeted = OversizedTargeted;
        let static_targeted = crate::prepared::prepare_operation::<Targeted, _>(
            &targeted,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        );
        assert!(
            matches!(static_targeted, Err(Error::InvalidRequest(_))),
            "static targeted request must reject encoded_size above MAX_SIZE"
        );

        let erased_targeted: &dyn DynTargetedRequest = &targeted;
        let targeted_adapter = OperationAdapter::<Targeted, _>::new(erased_targeted);
        let dynamic_targeted = crate::prepared::prepare_operation::<Targeted, _>(
            &targeted_adapter,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        );
        assert!(
            matches!(dynamic_targeted, Err(Error::InvalidRequest(_))),
            "erased targeted request must retain its concrete MAX_SIZE bound"
        );

        let applied = OversizedApplied;
        let static_applied = crate::prepared::prepare_operation::<AppliedOnly, _>(
            &applied,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        );
        assert!(
            matches!(static_applied, Err(Error::InvalidRequest(_))),
            "static applied request must reject encoded_size above MAX_SIZE"
        );

        let erased: &dyn DynAppliedRequest = &applied;
        let adapter = OperationAdapter::<AppliedOnly, _>::new(erased);
        let dynamic_applied = crate::prepared::prepare_operation::<AppliedOnly, _>(
            &adapter,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        );
        assert!(
            matches!(dynamic_applied, Err(Error::InvalidRequest(_))),
            "erased applied request must retain its concrete MAX_SIZE bound"
        );
    }

    #[test]
    fn erased_urgent_authority_is_forwarded_only_for_crate_owned_stops() {
        let Some(profile) = profile() else {
            return;
        };
        let downstream = DownstreamUrgentApplied;
        let erased: &dyn DynAppliedRequest = &downstream;
        let adapter = OperationAdapter::<AppliedOnly, _>::new(erased);
        let downstream_result = crate::prepared::prepare_operation::<AppliedOnly, _>(
            &adapter,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        );
        assert!(
            matches!(downstream_result, Err(Error::InvalidRequest(_))),
            "erasure must not authorize a downstream urgent request"
        );

        let stop = ZoomStop;
        let erased_stop: &dyn DynAppliedRequest = &stop;
        let stop_adapter = OperationAdapter::<AppliedOnly, _>::new(erased_stop);
        let prepared = crate::prepared::prepare_operation::<AppliedOnly, _>(
            &stop_adapter,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Handle(SubmissionClass::Background),
        );
        assert!(
            prepared.is_ok(),
            "erased crate-owned stop keeps urgent authority"
        );
        let Ok(prepared) = prepared else {
            return;
        };
        let context = prepared.admit_with(|request, _, _, _| *request.context());
        assert_eq!(context.control.class, EngineControlClass::Urgent);
    }
}
