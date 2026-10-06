//! Object-safe noun projection for the owner-backed dynamic camera.
//!
//! This module is deliberately a mechanical projection of the closed static
//! surface in [`crate::command::surface`].  Every method routes through the
//! generic preparation/admission methods on [`super::DynSessionCamera`]; the
//! dynamic layer does not duplicate capability or profile validation.
//!
//! Nothing here is written by hand: the noun traits, their implementations,
//! [`DynSessionCameraNouns`] and the `DynSessionCamera` getters are generated
//! from [`crate::noun_table`] by [`dyn_noun_facade!`], and the motion view by
//! [`dyn_motion_facade!`], from the same rows and noun headers the static
//! facades expand.  The consumers own only what is specific to the erased
//! surface — the object-safe trait shapes, the boxed `DynFuture` return types,
//! and the `DynSessionCamera` receiver plumbing.

#![cfg(feature = "async")]

use crate::{
    command,
    noun_facade::noun_request,
    noun_table::{motion_table, noun_table},
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    Error, Result, ZoomDomain,
};

use super::{DynAppliedOperation, DynFuture, DynSessionCamera, DynTargetedOperation};

/// Generates the object-safe noun surface from [`noun_table`].
///
/// Per noun header it emits the `Dyn*` trait (documented by the header `doc`)
/// with one declaration per row and its implementation for
/// [`DynSessionCamera`]; once, it emits [`DynSessionCameraNouns`] and the
/// inherent `DynSessionCamera` getters, each documented by its noun's `doc`.
/// The erased surface carries no compile-time capability bounds — its
/// enforcement is the runtime `validate_for_profile` check inside
/// [`crate::prepared::prepare_command`] — so the row `where` clauses and the
/// header gates are ignored here.
macro_rules! dyn_noun_facade {
    (@declare [$(#[$doc:meta])*] inquiry $method:ident() -> $ret:ty) => {
        $(#[$doc])*
        fn $method(&self) -> DynFuture<'_, Result<$ret, Error>>;
    };
    (@declare [$(#[$doc:meta])*] plain $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty) => {
        $(#[$doc])*
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<(), Error>>;
    };
    (@declare [$(#[$doc:meta])*] applied $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty) => {
        $(#[$doc])*
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    };
    (@declare [$(#[$doc:meta])*] targeted $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty) => {
        $(#[$doc])*
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    };

    (@implement inquiry $method:ident() -> $ret:ty; [$($request:tt)*]) => {
        fn $method(&self) -> DynFuture<'_, Result<$ret, Error>> {
            Box::pin(async move { self.inquire(&$($request)*).await })
        }
    };
    (@implement plain $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty; [$($request:tt)*]) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<(), Error>> {
            Box::pin(async move {
                let request = noun_request!(self.profile(); $ret; $($request)*)?;
                self.execute(&request).await
            })
        }
    };
    (@implement applied $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty; [$($request:tt)*]) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
            Box::pin(async move {
                let request = noun_request!(self.profile(); $ret; $($request)*)?;
                self.submit_applied(&request).await
            })
        }
    };
    (@implement targeted $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty; [$($request:tt)*]) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
            Box::pin(async move {
                let request = noun_request!(self.profile(); $ret; $($request)*)?;
                self.submit_targeted(&request).await
            })
        }
    };

    (
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
            #[doc = $doc]
            pub trait $dyn_trait: Send + Sync {
                $(
                    dyn_noun_facade!(@declare [$(#[$row_doc])*] $kind
                        $method($($arg: $ty),*) -> $ret);
                )*
            }

            impl $dyn_trait for DynSessionCamera {
                $(
                    dyn_noun_facade!(@implement $kind $method($($arg: $ty),*) -> $ret;
                        [$($request)*]);
                )*
            }
        )*

        /// Object-safe accessors for all final dynamic nouns.
        pub trait DynSessionCameraNouns: Send + Sync {
            $(
                #[doc = $doc]
                fn $getter(&self) -> &dyn $dyn_trait;
            )*

            motion_table!(dyn_motion_facade, [getter_declaration]);
        }

        impl DynSessionCamera {
            $(
                #[doc = $doc]
                pub fn $getter(&self) -> &dyn $dyn_trait {
                    self
                }
            )*
        }

        impl DynSessionCameraNouns for DynSessionCamera {
            $(
                fn $getter(&self) -> &dyn $dyn_trait {
                    self
                }
            )*

            motion_table!(dyn_motion_facade, [getter_implementation]);
        }
    };
}

/// Generates the object-safe motion view from [`motion_table`].
///
/// `[view]` emits the `DynMotion` trait and its implementation over the owner
/// core; `[inherent_getter]` emits the `DynSessionCamera::motion` getter
/// inside the camera's own `impl` block in `owner_projection`; and
/// `[getter_declaration]` and `[getter_implementation]` emit the
/// [`DynSessionCameraNouns`] getter. Every item takes the header `doc`.
macro_rules! dyn_motion_facade {
    (@declare [$(#[$doc:meta])*] $name:ident($($arg:ident: $ty:ty),*) -> $value:ty) => {
        $(#[$doc])*
        fn $name(&self $(, $arg: $ty)*) -> DynFuture<'_, Result<$value, Error>>;
    };
    (@implement $name:ident($($arg:ident: $ty:ty),*) -> $value:ty => $core:ident($($call:expr),*)) => {
        fn $name(&self $(, $arg: $ty)*) -> DynFuture<'_, Result<$value, Error>> {
            Box::pin(self.core().$core($($call),*))
        }
    };

    ($mode:tt
        @motion {
            accessor: $accessor:ident,
            getter: $getter:ident,
            dyn_trait: $dyn_trait:ident,
            doc: $doc:literal $(,)?
        };
        $(
            $(#[$row_doc:meta])*
            fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $value:ty
                => $core:ident($($call:expr),*);
        )*
    ) => {
        dyn_motion_facade!(@emit $mode $getter $dyn_trait $doc {
            $( dyn_motion_facade!(@declare [$(#[$row_doc])*] $name($($arg: $ty),*) -> $value); )*
        } {
            $( dyn_motion_facade!(@implement $name($($arg: $ty),*) -> $value => $core($($call),*)); )*
        });
    };

    (@emit [view] $getter:ident $dyn_trait:ident $doc:tt
        { $($declarations:tt)* } { $($implementations:tt)* }) => {
        #[doc = $doc]
        pub trait $dyn_trait: Send + Sync {
            $($declarations)*
        }

        impl $dyn_trait for DynSessionCamera {
            $($implementations)*
        }
    };
    (@emit [inherent_getter] $getter:ident $dyn_trait:ident $doc:tt $($bodies:tt)*) => {
        #[doc = $doc]
        #[must_use]
        pub fn $getter(&self) -> &dyn $dyn_trait {
            self
        }
    };
    (@emit [getter_declaration] $getter:ident $dyn_trait:ident $doc:tt $($bodies:tt)*) => {
        #[doc = $doc]
        fn $getter(&self) -> &dyn $dyn_trait;
    };
    (@emit [getter_implementation] $getter:ident $dyn_trait:ident $doc:tt $($bodies:tt)*) => {
        fn $getter(&self) -> &dyn $dyn_trait {
            self
        }
    };
}

pub(super) use dyn_motion_facade;

noun_table!(dyn_noun_facade);

motion_table!(dyn_motion_facade, [view]);
