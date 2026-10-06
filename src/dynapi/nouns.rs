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

/// Number of target-facing built-in command methods in this projection.
pub const DYN_NOUN_TARGET_METHOD_COUNT: usize = 146;

/// Number of typed inquiry methods in this projection.
pub const DYN_NOUN_INQUIRY_METHOD_COUNT: usize = 62;

/// Number of domain nouns (motion is a separate safety/observation view).
pub const DYN_NOUN_COUNT: usize = 14;

/// Number of declared non-ledger convenience methods on the noun traits.
///
/// A dynamic noun method is one of exactly three things: a projection of a
/// built-in command row, a typed inquiry accessor, or one of the hand-written
/// convenience wrappers named in [`DYN_NOUN_CONVENIENCE_METHODS`]. Keeping the
/// third category declared is what lets the inventory gate keep checking that
/// the projection carries nothing else.
pub const DYN_NOUN_CONVENIENCE_METHOD_COUNT: usize = 8;

/// The non-ledger convenience wrappers carried by the dynamic noun traits.
///
/// Each entry builds the request of a ledger row from its own arguments, so
/// it adds ergonomics without adding a command row. `src/noun_parity.rs` separately
/// requires the async and blocking facades to expose the same method set, so
/// none of these can become dynamic-only.
pub const DYN_NOUN_CONVENIENCE_METHODS: &[(&str, &str)] = &[
    ("DynZoom", "set_normalized"),
    ("DynZoom", "set_normalized_in_domain"),
    ("DynPanTilt", "up"),
    ("DynPanTilt", "down"),
    ("DynPanTilt", "left"),
    ("DynPanTilt", "right"),
    ("DynNdFilter", "set_stops"),
    ("DynMenu", "toggle_display"),
];

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

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use std::collections::HashSet;

    use crate::command::{
        inquiry_structs::{BuiltinInquiryQuery, BUILTIN_INQUIRIES, BUILTIN_INQUIRY_ACCESSORS},
        semantics::{BuiltinCommand, BuiltinRequestClass},
        surface::{surface_entry, StaticSurfaceDisposition},
    };

    use super::{DYN_NOUN_COUNT, DYN_NOUN_INQUIRY_METHOD_COUNT, DYN_NOUN_TARGET_METHOD_COUNT};

    #[test]
    fn dynamic_command_counts_follow_static_surface_ledger() {
        // The public compile-pass fixture type-checks every dynamic method. Keep
        // its expected class totals anchored here to the authoritative
        // command ledger rather than relying only on dynamic constants.
        let mut target_facing = 0;
        let mut target_plain = 0;
        let mut target_applied_only = 0;
        let mut target_targeted = 0;
        let mut all_plain = 0;
        let mut all_applied_only = 0;
        let mut all_targeted = 0;
        let mut noun_names = HashSet::new();
        let mut commands = HashSet::new();
        let mut exceptions = Vec::new();

        for command in BuiltinCommand::ALL {
            assert!(commands.insert(*command), "duplicate command ledger row");
            let entry = surface_entry(*command);

            match entry.class {
                BuiltinRequestClass::Plain { .. } => all_plain += 1,
                BuiltinRequestClass::AppliedOnly { .. } => all_applied_only += 1,
                BuiltinRequestClass::Targeted { .. } => all_targeted += 1,
            }

            match entry.disposition {
                StaticSurfaceDisposition::Noun { noun, .. } => {
                    target_facing += 1;
                    noun_names.insert(noun);
                    match entry.class {
                        BuiltinRequestClass::Plain { .. } => target_plain += 1,
                        BuiltinRequestClass::AppliedOnly { .. } => target_applied_only += 1,
                        BuiltinRequestClass::Targeted { .. } => target_targeted += 1,
                    }
                }
                StaticSurfaceDisposition::BroadcastHandshake { .. }
                | StaticSurfaceDisposition::InternalCancellation { .. } => {
                    exceptions.push(*command);
                }
            }
        }

        assert_eq!(
            exceptions,
            vec![
                BuiltinCommand::AddressSet,
                BuiltinCommand::InterfaceClear,
                BuiltinCommand::CommandCancel,
            ]
        );

        // Every total below is derived from `BuiltinCommand::ALL`; none of
        // them is written down, so adding or removing a ledger row moves the
        // declared projection sizes rather than breaking a literal.
        assert_eq!(noun_names.len(), DYN_NOUN_COUNT);
        assert_eq!(DYN_NOUN_TARGET_METHOD_COUNT, target_facing);
        assert_eq!(target_facing + exceptions.len(), BuiltinCommand::ALL.len());
        assert_eq!(
            all_plain + all_applied_only + all_targeted,
            BuiltinCommand::ALL.len()
        );
        // Both broadcast handshakes and the cancellation primitive are plain
        // rows, so the noun split moves only the plain class.
        assert_eq!(
            (
                target_plain + exceptions.len(),
                target_applied_only,
                target_targeted
            ),
            (all_plain, all_applied_only, all_targeted)
        );
    }

    #[test]
    fn dynamic_inquiry_count_follows_generated_typed_accessor_ledger() {
        // The generated inquiry table is shared by the static accessors; the
        // dynamic fixture type-checks the corresponding erased response types.
        let typed_queryable = BUILTIN_INQUIRIES
            .iter()
            .filter(|metadata| {
                !matches!(metadata.query, BuiltinInquiryQuery::DecodeOnly) && metadata.typed
            })
            .count();
        assert_eq!(typed_queryable, BUILTIN_INQUIRY_ACCESSORS.len());
        assert_eq!(
            DYN_NOUN_INQUIRY_METHOD_COUNT,
            BUILTIN_INQUIRY_ACCESSORS.len()
        );

        let mut mappings = HashSet::new();
        for accessor in BUILTIN_INQUIRY_ACCESSORS {
            assert!(
                mappings.insert((accessor.trait_name, accessor.method)),
                "duplicate inquiry mapping {}::{}",
                accessor.trait_name,
                accessor.method,
            );
            let metadata = BUILTIN_INQUIRIES
                .iter()
                .find(|metadata| metadata.name == accessor.command.name())
                .unwrap_or_else(|| {
                    panic!("missing inquiry metadata for {}", accessor.command.name())
                });
            assert!(metadata.typed, "{} must be typed", metadata.name);
            assert!(
                !matches!(metadata.query, BuiltinInquiryQuery::DecodeOnly),
                "{} must be queryable",
                metadata.name,
            );
        }

        let mut untyped_queryable = BUILTIN_INQUIRIES
            .iter()
            .filter(|metadata| {
                !matches!(metadata.query, BuiltinInquiryQuery::DecodeOnly) && !metadata.typed
            })
            .map(|metadata| metadata.name)
            .collect::<Vec<_>>();
        untyped_queryable.sort_unstable();
        assert_eq!(untyped_queryable, ["DefogModeInquiry"]);
    }
}
