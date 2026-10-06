//! Shared expansion helpers for the noun facades generated from
//! [`crate::noun_table`].

/// The one parser of a noun row's request form.
///
/// A row writes its request as `[<expr>]`, `[checked <expr>]` or
/// `[with_profile |<name>| <expr>]`; this macro evaluates any of them to
/// `crate::Result<$ty>`. `$profile` is substituted only into the
/// `with_profile` form, so a row that does not need the session profile never
/// reads it.
macro_rules! noun_request {
    ($profile:expr; $ty:ty; checked $request:expr) => {{
        let request: crate::Result<$ty> = $request;
        request
    }};
    ($profile:expr; $ty:ty; with_profile |$binding:ident| $request:expr) => {{
        let $binding = $profile;
        let request: crate::Result<$ty> = $request;
        request
    }};
    ($profile:expr; $ty:ty; $request:expr) => {{
        let request: $ty = $request;
        crate::Result::<$ty>::Ok(request)
    }};
}

pub(crate) use noun_request;

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
/// bounds nothing. A row's `where` clause narrows its own method.
macro_rules! static_noun_facade {
    (@accessor [always] $($rest:tt)*) => {
        $crate::noun_facade::static_noun_facade!(@emit [] [] $($rest)*);
    };
    (@accessor [domain $marker:ident] $($rest:tt)*) => {
        $crate::noun_facade::static_noun_facade!(@emit [] [$marker] $($rest)*);
    };
    (@accessor [typed $marker:ident] $($rest:tt)*) => {
        $crate::noun_facade::static_noun_facade!(@emit [$marker] [$marker] $($rest)*);
    };

    (@emit [$($struct_bound:ident)?] [$($getter_bound:ident)?]
        $accessor:ident $getter:ident $doc:tt { $($methods:tt)* }) => {
        #[doc = $doc]
        #[must_use]
        pub struct $accessor<'view, P: $crate::CompileTimeProfile $(+ $crate::capabilities::$struct_bound)?> {
            camera: &'view Camera<P>,
        }

        impl<P: $crate::CompileTimeProfile $(+ $crate::capabilities::$struct_bound)?> ::core::fmt::Debug
            for $accessor<'_, P>
        {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.debug_struct(stringify!($accessor)).finish_non_exhaustive()
            }
        }

        impl<'view, P: $crate::CompileTimeProfile $(+ $crate::capabilities::$struct_bound)?>
            $accessor<'view, P>
        {
            fn new(camera: &'view Camera<P>) -> Self {
                Self { camera }
            }

            $($methods)*
        }

        impl<P: $crate::CompileTimeProfile> Camera<P> {
            #[doc = $doc]
            pub fn $getter(&self) -> $accessor<'_, P>
            $(where P: $crate::capabilities::$getter_bound)?
            {
                $accessor::new(self)
            }
        }
    };

    (@method [$($async:tt)?] [$($await:tt)*] [$(#[$doc:meta])*] inquiry $method:ident() -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub $($async)? fn $method(&self) -> $crate::Result<$ret>
        $(where P: $crate::capabilities::$gate $(+ $crate::capabilities::$extra)*)?
        {
            self.camera.inquire(&$($request)*) $($await)*
        }
    };
    (@method [$($async:tt)?] [$($await:tt)*] [$(#[$doc:meta])*] plain
        $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub $($async)? fn $method(&self, $($arg: $ty),*) -> $crate::Result<()>
        $(where P: $crate::capabilities::$gate $(+ $crate::capabilities::$extra)*)?
        {
            let request = $crate::noun_facade::noun_request!(self.camera.profile(); $ret; $($request)*)?;
            self.camera.execute(&request) $($await)*
        }
    };
    (@method [$($async:tt)?] [$($await:tt)*] [$(#[$doc:meta])*] applied
        $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub $($async)? fn $method(&self, $($arg: $ty),*)
            -> $crate::Result<Operation<$crate::completion::AppliedOnly>>
        $(where P: $crate::capabilities::$gate $(+ $crate::capabilities::$extra)*)?
        {
            let request = $crate::noun_facade::noun_request!(self.camera.profile(); $ret; $($request)*)?;
            self.camera.submit::<$crate::completion::AppliedOnly, _>(&request) $($await)*
        }
    };
    (@method [$($async:tt)?] [$($await:tt)*] [$(#[$doc:meta])*] targeted
        $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub $($async)? fn $method(&self, $($arg: $ty),*)
            -> $crate::Result<Operation<$crate::completion::Targeted>>
        $(where P: $crate::capabilities::$gate $(+ $crate::capabilities::$extra)*)?
        {
            let request = $crate::noun_facade::noun_request!(self.camera.profile(); $ret; $($request)*)?;
            self.camera.submit::<$crate::completion::Targeted, _>(&request) $($await)*
        }
    };

    ($mode:tt $await:tt
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                dyn_trait: $dyn_trait:ident,
                gate: $gate:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[$row_doc:meta])*
                $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $rgate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        $(
            $crate::noun_facade::static_noun_facade!(@accessor $gate $accessor $getter $doc {
                $(
                    $crate::noun_facade::static_noun_facade!(@method $mode $await
                        [$(#[$row_doc])*] $kind $method($($arg: $ty),*) -> $ret;
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
        pub $($async)? fn $name(&self $(, $arg: $ty)*) -> $crate::Result<$value> {
            self.core.$core_method($($call),*) $($await)*
        }
    };

    ($mode:tt $await:tt $core:ident
        @motion {
            accessor: $accessor:ident,
            getter: $getter:ident,
            dyn_trait: $dyn_trait:ident,
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

        impl<P: $crate::CompileTimeProfile> Camera<P> {
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
