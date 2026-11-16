//! Canonical noun requests and their private wire-contract forwarding.

/// The source facts used to connect a noun query to the inquiry registry.
///
/// The noun table owns the gate; the underlying inquiry inventory owns its
/// kind and source. Keeping these projections separate avoids another query
/// inventory while permitting queries with a different decoded value.
pub(crate) trait NounInquiryContract: crate::Inquiry {
    const KIND: crate::command::inquiry_structs::InquiryKind;
    const SOURCE: crate::source_evidence::Source;
}

/// Forwards a canonical request to its private wire request.
///
/// The ordinary form borrows a private `inner` field. A unit request supplies
/// its constant wire value after a semicolon. Parameterized requests may
/// instead supply `|request| expression` to construct their wire value from
/// checked fields. Constructors, fields, getters and derives belong to the
/// concrete request declaration.
///
/// `rows = [FirstRow, OtherRow]` declares the exact semantic rows of a
/// narrower wrapper. Its first row is the representative; every listed row
/// must preserve the inner request's class, axes and state effect.
macro_rules! forward_request {
    (@class Plain) => { $crate::request::PlainClass };
    (@class Targeted) => { $crate::request::OperationClass<$crate::completion::Targeted> };
    (@class AppliedOnly) => { $crate::request::OperationClass<$crate::completion::AppliedOnly> };

    (@inner $this:ident [field]) => { &$this.inner };
    (@inner $this:ident [unit $inner:expr]) => { &const { $inner } };
    (@inner $this:ident [project |$request:ident| $inner:expr]) => {
        &{ let $request = $this; $inner }
    };

    (@validate $this:ident, $target:ident []) => { ::core::result::Result::<(), $crate::Error>::Ok(()) };
    (@validate $this:ident, $target:ident [|$request:ident, $registration:ident| $validate:expr]) => {{
        let $request = $this;
        let $registration = $target;
        let _ = &$request;
        let _ = &$registration;
        $validate
    }};

    // The full Request contract is shared with inquiry forwarding. In
    // particular the authority-bearing hooks must reach the inner request;
    // falling back to their trait defaults would lose reviewed admission,
    // profile-selected bytes, or exact applied-state projection.
    (@request $type:ty, $inner:ty, $class:ty, [$($value:tt)*], [$($validate:tt)*]) => {
        impl $crate::Request for $type {
            type Class = $class;
            const MAX_SIZE: usize = <$inner as $crate::Request>::MAX_SIZE;
            const TIMEOUT_CLASS: $crate::TimeoutClass = <$inner as $crate::Request>::TIMEOUT_CLASS;
            const RETRY_CLASS: $crate::RetryClass = <$inner as $crate::Request>::RETRY_CLASS;
            const CONTROL_CLASS: $crate::ControlClass = <$inner as $crate::Request>::CONTROL_CLASS;

            fn timeout_class(&self) -> $crate::TimeoutClass {
                <$inner as $crate::Request>::timeout_class(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn retry_class(&self) -> $crate::RetryClass {
                <$inner as $crate::Request>::retry_class(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn control_class(&self) -> $crate::ControlClass {
                <$inner as $crate::Request>::control_class(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn motion_axes(&self) -> ::core::option::Option<$crate::AffectedAxes> {
                <$inner as $crate::Request>::motion_axes(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            #[allow(private_interfaces)]
            fn admission_control_class(
                &self,
                authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::Result<$crate::ControlClass> {
                <$inner as $crate::Request>::admission_control_class(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
            fn reply_shape(&self) -> $crate::raw::RawReplyShape {
                <$inner as $crate::Request>::reply_shape(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            #[allow(private_interfaces)]
            fn declared_max_size(&self, authority: $crate::requests::RequestContractAuthority) -> usize {
                <$inner as $crate::Request>::declared_max_size(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
            fn write_into(&self, camera_id: $crate::CameraId, buffer: &mut [u8]) -> $crate::Result<usize> {
                <$inner as $crate::Request>::write_into(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    camera_id,
                    buffer,
                )
            }
            #[allow(private_interfaces)]
            fn write_for_target(
                &self,
                target: &$crate::TargetRegistration,
                buffer: &mut [u8],
                authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::Result<usize> {
                <$inner as $crate::Request>::write_for_target(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    target,
                    buffer,
                    authority,
                )
            }
            fn validate_for_target(&self, target: &$crate::TargetRegistration) -> $crate::Result<()> {
                $crate::command::nouns::forward_request!(@validate self, target [$($validate)*])?;
                <$inner as $crate::Request>::validate_for_target(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    target,
                )
            }
            #[allow(private_interfaces)]
            fn pan_tilt_max_speed(
                &self,
                authority: $crate::requests::RequestContractAuthority,
            ) -> ::core::option::Option<u8> {
                <$inner as $crate::Request>::pan_tilt_max_speed(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
            #[cfg(any(feature = "blocking", feature = "async"))]
            #[allow(private_interfaces)]
            fn applied_state_projection(
                &self,
                authority: $crate::requests::AppliedStateAuthority,
            ) -> ::core::option::Option<$crate::runtime::engine::AppliedStateProjection> {
                <$inner as $crate::Request>::applied_state_projection(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
        }
    };

    (@operation $type:ty, $inner:ty, Plain, [$($value:tt)*]) => {};
    (@operation $type:ty, $inner:ty, $class:ident, [$($value:tt)*]) => {
        impl $crate::OperationCommand<$crate::completion::$class> for $type {
            fn affected_axes(&self) -> $crate::AffectedAxes {
                <$inner as $crate::OperationCommand<$crate::completion::$class>>::affected_axes(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
        }
    };
    (@row $inner:ty, []) => {
        <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROW
    };
    (@row $inner:ty, [$first:ident $(, $other:ident)*]) => {
        $crate::command::semantics::BuiltinCommand::$first
    };
    (@rows $inner:ty, []) => {
        <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROWS
    };
    (@rows $inner:ty, [$($row:ident),+]) => {
        &[$($crate::command::semantics::BuiltinCommand::$row),+]
    };
    (@check_rows $inner:ty, [$($row:ident),*]) => {
        $(const _: () = ::core::assert!(
            $crate::command::semantics::BuiltinCommand::$row.classification().same_contract(
                <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROW.classification(),
            ),
            "a forwarded request row changes its inner class, axes or state-effect contract",
        );)*
        const _: () = {
            let declared: &[$crate::command::semantics::BuiltinCommand] =
                &[$($crate::command::semantics::BuiltinCommand::$row),*];
            let inherited = <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROWS;
            let mut index = 0;
            while index < declared.len() {
                let mut known = false;
                let mut candidate = 0;
                while candidate < inherited.len() {
                    if declared[index] as usize == inherited[candidate] as usize {
                        known = true;
                    }
                    candidate += 1;
                }
                ::core::assert!(known, "a forwarded request declares a row outside its inner request");
                let mut previous = 0;
                while previous < index {
                    ::core::assert!(
                        declared[index] as usize != declared[previous] as usize,
                        "a forwarded request repeats a ledger row",
                    );
                    previous += 1;
                }
                index += 1;
            }
        };
    };
    (@command $type:ty, $inner:ty, $class:ident, [$($value:tt)*], [$($row:ident),*]) => {
        $crate::command::nouns::forward_request!(
            @request $type, $inner,
            $crate::command::nouns::forward_request!(@class $class),
            [$($value)*], []
        );
        impl $crate::command::semantics::BuiltinRequestContract for $type {
            const LEDGER_ROW: $crate::command::semantics::BuiltinCommand =
                $crate::command::nouns::forward_request!(@row $inner, [$($row),*]);
            type State = <$inner as $crate::command::semantics::BuiltinRequestContract>::State;
            const LEDGER_ROWS: &'static [$crate::command::semantics::BuiltinCommand] =
                $crate::command::nouns::forward_request!(@rows $inner, [$($row),*]);
            #[cfg(test)]
            fn selected_ledger_row(&self) -> $crate::command::semantics::BuiltinCommand {
                <$inner as $crate::command::semantics::BuiltinRequestContract>::selected_ledger_row(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
        }
        const _: () = <$type as $crate::command::semantics::BuiltinRequestContract>::CLASS_MATCHES_ROW;
        $crate::command::nouns::forward_request!(@check_rows $inner, [$($row),*]);
        $crate::command::nouns::forward_request!(@operation $type, $inner, $class, [$($value)*]);
    };

    ($type:ty, $inner:ty, $class:ident $(, rows = [$($row:ident),+ $(,)?])?) => {
        $crate::command::nouns::forward_request!(@command $type, $inner, $class, [field], [$($($row),+)?]);
    };
    ($type:ty, $inner:ty, $class:ident $(, rows = [$($row:ident),+ $(,)?])?; |$request:ident| $value:expr) => {
        $crate::command::nouns::forward_request!(@command $type, $inner, $class, [project |$request| $value], [$($($row),+)?]);
    };
    ($type:ty, $inner:ty, $class:ident $(, rows = [$($row:ident),+ $(,)?])?; $value:expr) => {
        $crate::command::nouns::forward_request!(@command $type, $inner, $class, [unit $value], [$($($row),+)?]);
    };
}

pub(crate) use forward_request;

/// Forwards query encoding and decoding, including bound profile context.
///
/// `@request` supplies only Request and source metadata forwarding, for a
/// query with a different decoded response. It may add a target validator;
/// the concrete type then implements every Inquiry hook explicitly.
macro_rules! forward_inquiry {
    (@request_impl $type:ty, $inner:ty, [$($value:tt)*], [$($validate:tt)*]) => {
        $crate::command::nouns::forward_request!(
            @request $type, $inner, $crate::request::InquiryClass, [$($value)*],
            [|request, registration| {
                <$type as $crate::command::surface::BuiltinInquiryGate>::GATE.validate(
                    registration.profile(),
                    concat!("inquiry ", stringify!($type)),
                )?;
                $crate::command::nouns::forward_request!(@validate request, registration [$($validate)*])
            }]
        );
        impl $crate::command::nouns::NounInquiryContract for $type {
            const KIND: $crate::command::inquiry_structs::InquiryKind =
                <$inner as $crate::command::inquiry_structs::GeneratedInquiry>::KIND;
            const SOURCE: $crate::source_evidence::Source =
                <$inner as $crate::command::inquiry_structs::GeneratedInquiry>::SOURCE;
        }
    };
    (@inquiry $type:ty, $inner:ty, [$($value:tt)*]) => {
        impl $crate::Inquiry for $type {
            type Response = <$inner as $crate::Inquiry>::Response;
            fn payload_shape(&self) -> $crate::PayloadShape {
                <$inner as $crate::Inquiry>::payload_shape(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn payload_shape_for_profile(&self, profile: &$crate::ProfileSpec) -> $crate::PayloadShape {
                <$inner as $crate::Inquiry>::payload_shape_for_profile(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    profile,
                )
            }
            fn decoder(&self) -> $crate::ResponseDecoder<Self::Response> {
                <$inner as $crate::Inquiry>::decoder(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn decoder_for_profile(&self, profile: &$crate::ProfileSpec) -> $crate::ResponseDecoder<Self::Response> {
                <$inner as $crate::Inquiry>::decoder_for_profile(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    profile,
                )
            }
            #[allow(private_interfaces)]
            fn builtin_inquiry_syntax_retry(
                &self,
                authority: $crate::requests::BuiltinInquiryAuthority,
            ) -> bool {
                <$inner as $crate::Inquiry>::builtin_inquiry_syntax_retry(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
        }
    };

    (@request $type:ty, $inner:ty $(, validate = |$request:ident, $target:ident| $validate:expr)?) => {
        $crate::command::nouns::forward_inquiry!(
            @request_impl $type, $inner, [field], [$(|$request, $target| $validate)?]
        );
    };
    (@request $type:ty, $inner:ty $(, validate = |$request:ident, $target:ident| $validate:expr)?; $value:expr) => {
        $crate::command::nouns::forward_inquiry!(
            @request_impl $type, $inner, [unit $value], [$(|$request, $target| $validate)?]
        );
    };
    ($type:ty, $inner:ty) => {
        $crate::command::nouns::forward_inquiry!(@request_impl $type, $inner, [field], []);
        $crate::command::nouns::forward_inquiry!(@inquiry $type, $inner, [field]);
    };
    ($type:ty, $inner:ty; $value:expr) => {
        $crate::command::nouns::forward_inquiry!(@request_impl $type, $inner, [unit $value], []);
        $crate::command::nouns::forward_inquiry!(@inquiry $type, $inner, [unit $value]);
    };
}

pub(crate) use forward_inquiry;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{
        command::{
            inquiry_structs::ZoomPositionInquiry,
            semantics::{BuiltinCommand, BuiltinRequestContract},
            surface::{BuiltinInquiryGate, RowGate},
        },
        request::builtin::{ImageFlipCommand, ZoomDrive, ZoomStop},
        CameraId, Inquiry, OperationCommand, Request,
    };

    struct Flip {
        enabled: bool,
    }
    super::forward_request!(Flip, ImageFlipCommand, Plain; |request| ImageFlipCommand::new(request.enabled));

    struct Stop;
    super::forward_request!(Stop, ZoomStop, AppliedOnly; ZoomStop);

    struct Tele;
    super::forward_request!(Tele, ZoomDrive, AppliedOnly, rows = [ZoomTele]; ZoomDrive::Tele);

    struct Query;
    super::forward_inquiry!(Query, ZoomPositionInquiry; ZoomPositionInquiry);
    impl BuiltinInquiryGate for Query {
        const GATE: RowGate = RowGate::Always;
    }

    struct RestrictedQuery;
    super::forward_inquiry!(RestrictedQuery, ZoomPositionInquiry; ZoomPositionInquiry);
    impl BuiltinInquiryGate for RestrictedQuery {
        const GATE: RowGate = RowGate::Typed(crate::capabilities::TypedSupportSet::from_surface(
            crate::capabilities::TypedSupportSurface::UsbAudio,
        ));
    }

    struct CheckedQuery {
        inner: ZoomPositionInquiry,
        allowed: bool,
    }
    super::forward_inquiry!(@request CheckedQuery, ZoomPositionInquiry,
        validate = |request, _target| {
            if request.allowed {
                Ok(())
            } else {
                Err(crate::Error::FeatureNotSupported { feature: "query domain" })
            }
        }
    );
    super::forward_inquiry!(@inquiry CheckedQuery, ZoomPositionInquiry, [field]);
    impl BuiltinInquiryGate for CheckedQuery {
        const GATE: RowGate = RowGate::Always;
    }

    fn target() -> crate::TargetRegistration {
        crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
                .expect("generic profile"),
        )
        .expect("target registration")
    }

    #[test]
    fn projections_preserve_value_selected_rows_and_applied_state() {
        for enabled in [false, true] {
            let request = Flip { enabled };
            let inner = ImageFlipCommand::new(enabled);
            assert_eq!(
                request.selected_ledger_row(),
                if enabled {
                    BuiltinCommand::ImageFlipVertical
                } else {
                    BuiltinCommand::ImageFlipOff
                }
            );
            let mut actual = [0; 6];
            let mut expected = [0; 6];
            assert_eq!(
                request
                    .write_into(CameraId::CAMERA_1, &mut actual)
                    .expect("forwarded frame"),
                inner
                    .write_into(CameraId::CAMERA_1, &mut expected)
                    .expect("inner frame")
            );
            assert_eq!(actual, expected);
            #[cfg(any(feature = "blocking", feature = "async"))]
            assert_eq!(
                crate::requests::applied_state_projection(&request),
                crate::requests::applied_state_projection(&inner)
            );
        }
        assert_eq!(
            <Flip as BuiltinRequestContract>::LEDGER_ROWS,
            <ImageFlipCommand as BuiltinRequestContract>::LEDGER_ROWS
        );
    }

    #[test]
    fn unit_operations_retain_reserved_admission_authority_and_exact_axes() {
        let request = Stop;
        assert_eq!(
            request
                .admission_control_class(crate::requests::RequestContractAuthority::new())
                .expect("reviewed stop admission"),
            crate::ControlClass::Urgent
        );
        assert_eq!(request.affected_axes(), crate::AffectedAxes::ZOOM);
        assert_eq!(request.selected_ledger_row(), BuiltinCommand::ZoomStop);
        let mut actual = [0; 6];
        let mut expected = [0; 6];
        assert_eq!(
            request
                .write_into(CameraId::CAMERA_1, &mut actual)
                .expect("forwarded frame"),
            ZoomStop
                .write_into(CameraId::CAMERA_1, &mut expected)
                .expect("inner frame")
        );
        assert_eq!(actual, expected);
        assert_eq!(
            <Tele as BuiltinRequestContract>::LEDGER_ROWS,
            &[BuiltinCommand::ZoomTele]
        );
        assert_eq!(Tele.selected_ledger_row(), BuiltinCommand::ZoomTele);
        assert_eq!(Tele.affected_axes(), crate::AffectedAxes::ZOOM);
    }

    #[test]
    fn unit_inquiries_retain_profile_decoding_and_source_kind() {
        let target = target();
        let query = Query;
        assert_eq!(
            query.payload_shape_for_profile(target.profile()),
            ZoomPositionInquiry.payload_shape_for_profile(target.profile())
        );
        assert_eq!(
            query
                .decoder_for_profile(target.profile())
                .decode(&[0, 1, 2, 3])
                .expect("forwarded response"),
            ZoomPositionInquiry
                .decoder_for_profile(target.profile())
                .decode(&[0, 1, 2, 3])
                .expect("inner response")
        );
        assert_eq!(
            <Query as super::NounInquiryContract>::KIND,
            <ZoomPositionInquiry as crate::command::inquiry_structs::GeneratedInquiry>::KIND
        );
        let _ = <Query as super::NounInquiryContract>::SOURCE;
        #[cfg(any(feature = "blocking", feature = "async"))]
        assert!(crate::requests::builtin_inquiry_syntax_retry(&query));
    }

    #[test]
    fn request_only_inquiry_forwarding_checks_each_target() {
        let target = target();
        let mut query = CheckedQuery {
            inner: ZoomPositionInquiry,
            allowed: true,
        };
        assert!(query.validate_for_target(&target).is_ok());
        query.allowed = false;
        assert!(matches!(
            query.validate_for_target(&target),
            Err(crate::Error::FeatureNotSupported {
                feature: "query domain"
            })
        ));
    }

    #[test]
    fn canonical_inquiry_gate_is_checked_before_the_private_wire_query() {
        let target = target();
        assert!(ZoomPositionInquiry.validate_for_target(&target).is_ok());
        assert!(matches!(
            RestrictedQuery.validate_for_target(&target),
            Err(crate::Error::FeatureNotSupported {
                feature: "inquiry RestrictedQuery"
            })
        ));
    }
}
