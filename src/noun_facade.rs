//! Shared expansion helpers for the noun facades generated from
//! [`crate::noun_table`].

/// The one parser of a noun row's request form.
///
/// A row writes its request as `[<expr>]`, `[by_value <const expr>]`,
/// `[by_value [<const witness>] <expr>]`, `[checked <expr>]` or
/// `[with_profile |<name>| <expr>]`; this macro
/// evaluates any of them to `crate::Result<$ty>`. A `by_value` request
/// evaluates like `<expr>`; the marker only tells the typed-request inventory
/// to check the constant's ledger row. `$profile` is substituted only into the
/// `with_profile` form, so a row that does not need the session profile never
/// reads it.
macro_rules! noun_request {
    ($profile:expr, $target:expr; $ty:ty; by_value [$witness:expr] $request:expr) => {
        $crate::noun_facade::noun_request!($profile, $target; $ty; $request)
    };
    ($profile:expr, $target:expr; $ty:ty; by_value $request:expr) => {
        $crate::noun_facade::noun_request!($profile, $target; $ty; $request)
    };
    ($profile:expr, $target:expr; $ty:ty; checked $request:expr) => {{
        let request: crate::Result<$ty> = $request;
        request
    }};
    ($profile:expr, $target:expr; $ty:ty; with_profile |$binding:ident| $request:expr) => {{
        let $binding = $profile;
        let request: crate::Result<$ty> = $request;
        request
    }};
    ($profile:expr, $target:expr; $ty:ty; with_target |$binding:ident| $request:expr) => {{
        let $binding = $target;
        let request: crate::Result<$ty> = $request;
        request
    }};
    ($profile:expr, $target:expr; $ty:ty; $request:expr) => {{
        let request: $ty = $request;
        crate::Result::<$ty>::Ok(request)
    }};
}

pub(crate) use noun_request;

/// Builds the raw inquiry before evaluating any response mapping.
///
/// Fallible and target/profile-bound forms mirror `noun_request!`; the
/// inferred request type comes from the inquiry expression itself.
macro_rules! noun_inquiry_request {
    ($profile:expr, $target:expr; mapped [$($request:tt)*] |$reply:ident, $binding:ident| $mapping:expr) => {
        $crate::noun_facade::noun_inquiry_request!($profile, $target; $($request)*)
    };
    ($profile:expr, $target:expr; checked $request:expr) => {{
        let request: $crate::Result<_> = $request;
        request
    }};
    ($profile:expr, $target:expr; with_target |$binding:ident| $request:expr) => {{
        let $binding = $target;
        let request: $crate::Result<_> = $request;
        request
    }};
    ($profile:expr, $target:expr; with_profile |$binding:ident| $request:expr) => {{
        let $binding = $profile;
        let request: $crate::Result<_> = $request;
        request
    }};
    ($profile:expr, $target:expr; $request:expr) => {
        $crate::Result::<_>::Ok($request)
    };
}

pub(crate) use noun_inquiry_request;

/// Converts a decoded inquiry using the selected profile after protocol decoding.
macro_rules! noun_inquiry_response {
    ($value:expr, $selected_profile:expr; mapped [$($request:tt)*] |$reply:ident, $profile:ident| $mapping:expr) => {{
        let $reply = $value;
        let $profile = $selected_profile;
        let response: $crate::Result<_> = $mapping;
        response
    }};
    ($value:expr, $selected_profile:expr; $($request:tt)*) => {
        $crate::Result::Ok($value)
    };
}

pub(crate) use noun_inquiry_response;

/// Proves a static marker's runtime bit at the method's monomorphization.
macro_rules! assert_profile_gate {
    (@marker $profile:ident, $marker:ident) => {
        const {
            if let Some(supported) = <$profile as $crate::profiles::CameraProfile>::TYPED_SUPPORT {
                assert!(
                    supported.contains($crate::capabilities::typed_surface!($marker)),
                    concat!("profile marker `", stringify!($marker), "` requires its typed-support bit"),
                );
            }
        }
    };
    ($profile:ident; [typed $base:ident]; [$($marker:ident $(+ $extra:ident)*)?]) => {
        $crate::noun_facade::assert_profile_gate!(@marker $profile, $base);
        $($crate::noun_facade::assert_profile_gate!(@marker $profile, $marker);
          $($crate::noun_facade::assert_profile_gate!(@marker $profile, $extra);)*)?
    };
    ($profile:ident; $base:tt; [$($marker:ident $(+ $extra:ident)*)?]) => {
        $($crate::noun_facade::assert_profile_gate!(@marker $profile, $marker);
          $($crate::noun_facade::assert_profile_gate!(@marker $profile, $extra);)*)?
    };
}

pub(crate) use assert_profile_gate;

/// A row's `#[into]` parameter accepts any value convertible to its declared type.
/// Unmarked parameters retain their exact declared type in both facades.
macro_rules! noun_parameter_type {
    ([into]; $ty:ty) => { impl ::core::convert::Into<$ty> };
    ([]; $ty:ty) => { $ty };
}

pub(crate) use noun_parameter_type;

/// Converts marked inputs before evaluating the row's request expression.
macro_rules! noun_parameter_value {
    ([into]; $argument:ident; $ty:ty) => {
        let $argument: $ty = ::core::convert::Into::into($argument);
    };
    ([]; $argument:ident; $ty:ty) => {};
}

pub(crate) use noun_parameter_value;

/// Generates one static noun facade — blocking or async — from
/// [`crate::noun_table::noun_table`].
///
/// Invoke it as `noun_table!(static_noun_facade, [<async>], [<.await>])`
/// from a module that has `Camera` and `Operation` in scope: the blocking
/// facade passes `[] []`, the async facade `[async] [.await]`. These two mode
/// tokens are the only difference between the facades, so the blocking facade
/// emits no `async` code at all.
///
/// Per noun header it emits the accessor struct, its `Debug`, a private
/// constructor, the `Camera` getter and one inherent `impl` with a method per
/// row. The header `doc` documents both the struct and the getter. The header
/// `gate` places the bounds: `[typed M]` bounds the struct, its `Debug`, its
/// `impl` and the getter; `[domain M]` bounds only the getter; `[always]`
/// bounds nothing. A row's `where` clause narrows its own method. The row's
/// `#[into]` parameter marker selects conversion through `Into`; unmarked
/// parameters keep their declared type.
macro_rules! static_noun_facade {
    (@accessor [always] $($rest:tt)*) => {
        $crate::noun_facade::static_noun_facade!(@emit [] [] $($rest)*);
    };
    (@accessor [domain $marker:ident] $($rest:tt)*) => {
        $crate::noun_facade::static_noun_facade!(@emit [$marker] [$marker] $($rest)*);
    };
    (@accessor [typed $marker:ident] $($rest:tt)*) => {
        $crate::noun_facade::static_noun_facade!(@emit [$marker] [$marker] $($rest)*);
    };

    (@emit [$($struct_bound:ident)?] [$($getter_bound:ident)?]
        $accessor:ident $getter:ident $doc:tt { $($methods:tt)* }) => {
        #[doc = $doc]
        #[must_use]
        pub struct $accessor<'view, P: $crate::profiles::CameraProfile $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($struct_bound)>)? = $crate::profiles::AnyProfile> {
            camera: &'view Camera<P>,
        }

        impl<P: $crate::profiles::CameraProfile $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($struct_bound)>)?> ::core::marker::Copy
            for $accessor<'_, P>
        {}

        impl<P: $crate::profiles::CameraProfile $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($struct_bound)>)?> ::core::clone::Clone
            for $accessor<'_, P>
        {
            fn clone(&self) -> Self { *self }
        }

        impl<P: $crate::profiles::CameraProfile $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($struct_bound)>)?> ::core::fmt::Debug
            for $accessor<'_, P>
        {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.debug_struct(stringify!($accessor)).finish_non_exhaustive()
            }
        }

        impl<'view, P: $crate::profiles::CameraProfile $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($struct_bound)>)?>
            $accessor<'view, P>
        {
            fn new(camera: &'view Camera<P>) -> Self {
                Self { camera }
            }

            $($methods)*
        }

        impl<P: $crate::profiles::CameraProfile> Camera<P> {
            #[doc = $doc]
            pub fn $getter(&self) -> $accessor<'_, P>
            $(where P: $crate::capabilities::Permits<$crate::capabilities::permit_gate!($getter_bound)>)?
            {
                $accessor::new(self)
            }
        }
    };

    (@method [$($async:tt)?] [$($await:tt)*] $base:tt [$(#[$doc:meta])*] inquiry $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        #[doc = ""]
        #[doc = $crate::noun_table::completion_doc!(inquiry)]
        pub $($async)? fn $method(self $(, $arg: $crate::noun_facade::noun_parameter_type!([$($coercion)?]; $ty))*) -> $crate::Result<$ret>
        $(where P: $crate::capabilities::Permits<$crate::capabilities::permit_gate!($gate)> $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($extra)>)*)?
        {
            $($crate::noun_facade::noun_parameter_value!([$($coercion)?]; $arg; $ty);)*
            $crate::noun_facade::assert_profile_gate!(P; $base; [$($gate $(+ $extra)*)?]);
            let request = $crate::noun_facade::noun_inquiry_request!(
                self.camera.profile(), self.camera.registration(); $($request)*
            )?;
            let reply = self.camera.inquire(&request) $($await)*?;
            $crate::noun_facade::noun_inquiry_response!(reply, self.camera.profile(); $($request)*)
        }
    };
    (@method [$($async:tt)?] [$($await:tt)*] $base:tt [$(#[$doc:meta])*] plain
        $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        #[doc = ""]
        #[doc = $crate::noun_table::completion_doc!(plain)]
        pub $($async)? fn $method(self $(, $arg: $crate::noun_facade::noun_parameter_type!([$($coercion)?]; $ty))*) -> $crate::Result<()>
        $(where P: $crate::capabilities::Permits<$crate::capabilities::permit_gate!($gate)> $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($extra)>)*)?
        {
            $($crate::noun_facade::noun_parameter_value!([$($coercion)?]; $arg; $ty);)*
            $crate::noun_facade::assert_profile_gate!(P; $base; [$($gate $(+ $extra)*)?]);
            let request = $crate::noun_facade::noun_request!(self.camera.profile(), self.camera.registration(); $ret; $($request)*)?;
            self.camera.execute(&request) $($await)*
        }
    };
    (@method [$($async:tt)?] [$($await:tt)*] $base:tt [$(#[$doc:meta])*] applied
        $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        #[doc = ""]
        #[doc = $crate::noun_table::completion_doc!(applied)]
        pub $($async)? fn $method(self $(, $arg: $crate::noun_facade::noun_parameter_type!([$($coercion)?]; $ty))*)
            -> $crate::Result<Operation<$crate::completion::AppliedOnly>>
        $(where P: $crate::capabilities::Permits<$crate::capabilities::permit_gate!($gate)> $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($extra)>)*)?
        {
            $($crate::noun_facade::noun_parameter_value!([$($coercion)?]; $arg; $ty);)*
            $crate::noun_facade::assert_profile_gate!(P; $base; [$($gate $(+ $extra)*)?]);
            let request = $crate::noun_facade::noun_request!(self.camera.profile(), self.camera.registration(); $ret; $($request)*)?;
            self.camera.submit::<$crate::completion::AppliedOnly, _>(&request) $($await)*
        }
    };
    (@method [$($async:tt)?] [$($await:tt)*] $base:tt [$(#[$doc:meta])*] targeted
        $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        #[doc = ""]
        #[doc = $crate::noun_table::completion_doc!(targeted)]
        pub $($async)? fn $method(self $(, $arg: $crate::noun_facade::noun_parameter_type!([$($coercion)?]; $ty))*)
            -> $crate::Result<Operation<$crate::completion::Targeted>>
        $(where P: $crate::capabilities::Permits<$crate::capabilities::permit_gate!($gate)> $(+ $crate::capabilities::Permits<$crate::capabilities::permit_gate!($extra)>)*)?
        {
            $($crate::noun_facade::noun_parameter_value!([$($coercion)?]; $arg; $ty);)*
            $crate::noun_facade::assert_profile_gate!(P; $base; [$($gate $(+ $extra)*)?]);
            let request = $crate::noun_facade::noun_request!(self.camera.profile(), self.camera.registration(); $ret; $($request)*)?;
            self.camera.submit::<$crate::completion::Targeted, _>(&request) $($await)*
        }
    };

    ($mode:tt $await:tt
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                gate: $gate:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[$row_doc:meta])*
                $kind:ident [$($command:ident)?] $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $rgate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        $(
            $crate::noun_facade::static_noun_facade!(@accessor $gate $accessor $getter $doc {
                $(
                    $crate::noun_facade::static_noun_facade!(@method $mode $await $gate
                        [$(#[$row_doc])*] $kind $method($($(#[$coercion])? $arg: $ty),*) -> $ret;
                        [$($rgate $(+ $extra)*)?]; [$($request)*]);
                )*
            });
        )*
    };
}

pub(crate) use static_noun_facade;

/// Generates the static motion view — blocking or async — from
/// [`crate::noun_table::motion_table`].
///
/// Invoke it as `motion_table!(static_motion_facade, [<async>], [<.await>],
/// <CoreType>)` from a module that has `Camera` in scope. It emits the
/// `MotionAccessor` view over the owner core, its `Debug`, its crate-visible
/// constructor, the `Camera::motion` getter (both documented by the header
/// `doc`) and the motion methods.
macro_rules! static_motion_facade {
    (@method [$($async:tt)?] [$($await:tt)*] [$(#[$doc:meta])*]
        $name:ident($($arg:ident: $ty:ty),*) -> $value:ty => $core_method:ident($($call:expr),*)) => {
        $(#[$doc])*
        pub $($async)? fn $name(self $(, $arg: $ty)*) -> $crate::Result<$value> {
            self.core.$core_method($($call),*) $($await)*
        }
    };

    ($mode:tt $await:tt $core:ident
        @motion {
            accessor: $accessor:ident,
            getter: $getter:ident,
            doc: $doc:literal $(,)?
        };
        $(
            $(#[$row_doc:meta])*
            fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $value:ty
                => $core_method:ident($($call:expr),*);
        )*
    ) => {
        #[doc = $doc]
        #[must_use]
        pub struct $accessor<'view> {
            core: &'view $core,
        }

        impl ::core::marker::Copy for $accessor<'_> {}

        impl ::core::clone::Clone for $accessor<'_> {
            fn clone(&self) -> Self { *self }
        }

        impl ::core::fmt::Debug for $accessor<'_> {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.debug_struct(stringify!($accessor)).finish_non_exhaustive()
            }
        }

        impl<'view> $accessor<'view> {
            pub(crate) const fn new(core: &'view $core) -> Self {
                Self { core }
            }
        }

        impl<P: $crate::profiles::CameraProfile> Camera<P> {
            #[doc = $doc]
            pub fn $getter(&self) -> $accessor<'_> {
                $accessor::new(self.core())
            }
        }

        impl $accessor<'_> {
            $(
                $crate::noun_facade::static_motion_facade!(@method $mode $await
                    [$(#[$row_doc])*] $name($($arg: $ty),*) -> $value => $core_method($($call),*));
            )*
        }
    };
}

pub(crate) use static_motion_facade;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::types::{PanSpeed, PanTiltSpeed, Speed, SpeedLevel, TiltSpeed};

    struct NonCopyProfile {
        _state: String,
    }

    impl crate::CompileTimeProfile for NonCopyProfile {
        const TYPED_SUPPORT: crate::capabilities::TypedSupportSet =
            <crate::profiles::GenericVisca as crate::CompileTimeProfile>::TYPED_SUPPORT;
        const ID: Option<crate::profiles::ProfileId> =
            <crate::profiles::GenericVisca as crate::CompileTimeProfile>::ID;

        fn spec() -> crate::Result<crate::ProfileSpec> {
            <crate::profiles::GenericVisca as crate::CompileTimeProfile>::spec()
        }
    }

    impl crate::capabilities::HasPower for NonCopyProfile {}

    fn assert_copy<T: Copy>() {}

    #[test]
    fn borrowed_accessors_do_not_require_the_profile_to_be_copy() {
        #[cfg(feature = "blocking")]
        {
            assert_copy::<crate::blocking::PowerAccessor<'_, NonCopyProfile>>();
            assert_copy::<crate::blocking::MotionAccessor<'_>>();
        }
        #[cfg(feature = "async")]
        {
            assert_copy::<crate::async_nouns::PowerAccessor<'_, NonCopyProfile>>();
            assert_copy::<crate::async_nouns::MotionAccessor<'_>>();
        }
    }

    #[test]
    fn static_speed_inputs_accept_coarse_and_exact_values_before_lowering() {
        fn accept(speed: noun_parameter_type!([into]; Speed<PanTiltSpeed>)) -> Speed<PanTiltSpeed> {
            noun_parameter_value!([into]; speed; Speed<PanTiltSpeed>);
            speed
        }
        let exact = PanTiltSpeed::new(
            PanSpeed::new(12).expect("pan"),
            TiltSpeed::new(10).expect("tilt"),
        );
        assert_eq!(accept(exact), Speed::Exact(exact));
        assert_eq!(accept(SpeedLevel::Medium), Speed::Level(SpeedLevel::Medium));
    }

    #[test]
    fn inquiry_mapping_uses_the_reply_and_profile_without_rebuilding_the_request() {
        let constructed = std::cell::Cell::new(0);
        let request = noun_inquiry_request!(
            (), ();
            mapped[{
                constructed.set(constructed.get() + 1);
                7_u8
            }] | reply,
            profile | Ok(u16::from(reply) + profile)
        )
        .expect("inquiry request");
        let mapped: crate::Result<u16> = noun_inquiry_response!(
            request, 100_u16;
            mapped [{ panic!("the inquiry expression must not run twice") }]
                |reply, profile| Ok(u16::from(reply) + profile)
        );
        assert_eq!(mapped.expect("mapped reply"), 107);
        assert_eq!(constructed.get(), 1);
    }

    #[test]
    fn target_bound_inquiry_construction_does_not_read_an_unused_profile() {
        let request: crate::Result<u8> = noun_inquiry_request!(
            panic!("this request uses only its target"), &7_u8;
            with_target |target| Ok(*target)
        );
        assert_eq!(request.expect("target-bound inquiry"), 7);
    }

    #[test]
    fn inquiry_mapping_preserves_conversion_errors() {
        let mapped: crate::Result<u8> = noun_inquiry_response!(
            7_u8, ();
            mapped [7_u8] |reply, profile| {
                let _ = (reply, profile);
                Err(crate::Error::FeatureNotSupported { feature: "coordinate conversion" })
            }
        );
        assert!(matches!(
            mapped,
            Err(crate::Error::FeatureNotSupported {
                feature: "coordinate conversion"
            })
        ));
    }

    #[cfg(feature = "async")]
    #[test]
    fn speed_futures_are_send_for_coarse_and_exact_inputs() {
        fn assert_send<T: ::core::future::Future + Send>(future: T) {
            drop(future);
        }
        let _compile_send: fn(&crate::async_session::Camera<crate::profiles::GenericVisca>) =
            |camera| {
                use crate::types::{FocusSpeed, PanTiltDirection, PanTiltPosition, ZoomSpeed};
                let position = PanTiltPosition::from_degrees(0.0, 0.0).expect("finite position");
                let pan_tilt = PanTiltSpeed::new(PanSpeed::MIN, TiltSpeed::MIN);
                assert_send(
                    camera
                        .pan_tilt()
                        .drive(PanTiltDirection::Up, SpeedLevel::Medium),
                );
                assert_send(camera.pan_tilt().drive(PanTiltDirection::Up, pan_tilt));
                assert_send(camera.pan_tilt().absolute(position, SpeedLevel::Medium));
                assert_send(camera.pan_tilt().absolute(position, pan_tilt));
                assert_send(camera.pan_tilt().relative(position, SpeedLevel::Medium));
                assert_send(camera.pan_tilt().relative(position, pan_tilt));
                assert_send(camera.zoom().tele_variable(SpeedLevel::Medium));
                assert_send(camera.zoom().tele_variable(ZoomSpeed::MIN));
                assert_send(camera.zoom().wide_variable(SpeedLevel::Medium));
                assert_send(camera.zoom().wide_variable(ZoomSpeed::MIN));
                assert_send(camera.focus().far_variable(SpeedLevel::Medium));
                assert_send(camera.focus().far_variable(FocusSpeed::MIN));
                assert_send(camera.focus().near_variable(SpeedLevel::Medium));
                assert_send(camera.focus().near_variable(FocusSpeed::MIN));
            };
    }

    #[cfg(feature = "async")]
    #[test]
    fn temporary_noun_views_leave_futures_borrowing_only_the_camera() {
        let _compile_lifetimes: fn(&crate::Camera<crate::profiles::GenericVisca>) = |camera| {
            let zoom = camera.zoom().stop();
            let focus = camera.focus().stop();
            let joined = futures_lite::future::zip(zoom, focus);
            drop(joined);
        };
    }
}
