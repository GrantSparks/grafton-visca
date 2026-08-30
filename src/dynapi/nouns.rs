//! Object-safe noun projection for the owner-backed dynamic camera.
//!
//! This module is deliberately a mechanical projection of the closed static
//! surface in [`crate::command::surface`].  Every method routes through the
//! generic preparation/admission methods on [`super::DynSessionCamera`]; the
//! dynamic layer does not duplicate capability or profile validation.
//!
//! Neither the trait declarations nor their implementations are written out by
//! hand: both are generated from the shared row table in
//! [`crate::noun_table`], which the async and blocking facades consume from the
//! same rows.  This module owns only what is specific to the erased surface —
//! the object-safe trait shapes, the boxed `DynFuture` return types, and the
//! `DynSessionCamera` receiver plumbing.

#![cfg(feature = "dyn-api")]

use crate::{
    camera::{IdleWait, MotionQuery},
    command,
    noun_table::noun_table,
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    Error, Result, ZoomDomain,
};

use super::{DynAppliedOperation, DynFuture, DynSessionCamera, DynTargetedOperation};

/// Number of target-facing built-in command methods in this projection.
pub const DYN_NOUN_TARGET_METHOD_COUNT: usize = 146;

/// Number of typed inquiry methods in this projection.
pub const DYN_NOUN_INQUIRY_METHOD_COUNT: usize = 63;

/// Number of domain nouns (motion is a separate safety/observation view).
pub const DYN_NOUN_COUNT: usize = 14;

/// Number of declared non-ledger convenience methods on the noun traits.
///
/// A dynamic noun method is one of exactly three things: a projection of a
/// built-in command row, a typed inquiry accessor, or one of the hand-written
/// convenience wrappers named in [`DYN_NOUN_CONVENIENCE_METHODS`]. Keeping the
/// third category declared is what lets the inventory gate keep checking that
/// the projection carries nothing else.
pub const DYN_NOUN_CONVENIENCE_METHOD_COUNT: usize = 9;

/// The non-ledger convenience wrappers carried by the dynamic noun traits.
///
/// Each entry delegates to a ledger method with a fixed argument, so it adds
/// ergonomics without adding a command row. `src/noun_parity.rs` separately
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
    ("DynMotionSync", "set_speed"),
    ("DynMenu", "toggle_display"),
];

/// Declares one object-safe noun method per [`noun_table`] row.
///
/// The row grammar is documented on [`crate::noun_table`].  The erased surface
/// carries no compile-time capability bounds — its enforcement is the runtime
/// `validate_for_profile` check inside [`crate::prepared::prepare_command`] —
/// so this consumer drops the row's `where` clause and keeps only the shape.
macro_rules! dyn_noun_declarations {
    () => {};

    // The flat registry projection keeps noun context while collecting rows;
    // dynamic traits only need the row shapes and therefore strip it here.
    (@noun $noun:ident; $($rows:tt)*) => {
        dyn_noun_declarations!($($rows)*);
    };

    // The declared shape of a command row is its kind, name and arguments; how
    // its request value is built is invisible from the signature.  The `@shape`
    // arms below emit that shape, and the row arms further down strip whichever
    // request form the row uses before reaching them.
    (@shape $(#[$doc:meta])* plain $method:ident($($arg:ident: $ty:ty),*)) => {
        $(#[$doc])*
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<(), Error>>;
    };
    (@shape $(#[$doc:meta])* applied $method:ident($($arg:ident: $ty:ty),*)) => {
        $(#[$doc])*
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynAppliedOperation, Error>>;
    };
    (@shape $(#[$doc:meta])* targeted $method:ident($($arg:ident: $ty:ty),*)) => {
        $(#[$doc])*
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynTargetedOperation, Error>>;
    };

    (
        $(#[$doc:meta])*
        inquiry $($inquiry:ident)::+ $method:ident() -> $response:ty
            $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        fn $method(&self) -> DynFuture<'_, Result<$response, Error>>;
        dyn_noun_declarations!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = checked $request:expr;
        $($rest:tt)*
    ) => {
        dyn_noun_declarations!(@shape $(#[$doc])* $kind $method($($arg: $ty),*));
        dyn_noun_declarations!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)?
            = with_profile |$profile:ident| $request:expr;
        $($rest:tt)*
    ) => {
        dyn_noun_declarations!(@shape $(#[$doc])* $kind $method($($arg: $ty),*));
        dyn_noun_declarations!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)?
            = with_core |$core:ident| $request:expr;
        $($rest:tt)*
    ) => {
        dyn_noun_declarations!(@shape $(#[$doc])* $kind $method($($arg: $ty),*));
        dyn_noun_declarations!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            $(where $gate:tt $(+ $extra:tt)*)?
            = delegate $target:ident($($delegated:expr),*);
        $($rest:tt)*
    ) => {
        dyn_noun_declarations!(@shape $(#[$doc])* $kind $method($($arg: $ty),*));
        dyn_noun_declarations!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        dyn_noun_declarations!(@shape $(#[$doc])* $kind $method($($arg: $ty),*));
        dyn_noun_declarations!($($rest)*);
    };
}

/// Implements one object-safe noun method per [`noun_table`] row.
///
/// Every arm boxes the same owner hop the typed facades take synchronously:
/// build the request from the row, then reach `DynSessionCamera`'s generic
/// preparation and admission methods.
macro_rules! dyn_noun_impls {
    () => {};

    // Strip the all-rows projection's noun context before expanding the
    // object-safe request implementation grammar.
    (@noun $noun:ident; $($rows:tt)*) => {
        dyn_noun_impls!($($rows)*);
    };

    (
        $(#[$doc:meta])*
        inquiry $($inquiry:ident)::+ $method:ident() -> $response:ty
            $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self) -> DynFuture<'_, Result<$response, Error>> {
            Box::pin(async move {
                let request: $($inquiry)::+ = $request;
                self.inquire(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        plain [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = checked $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<(), Error>> {
            Box::pin(async move {
                let request: $request_ty = $request?;
                self.execute(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        plain [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)?
            = with_profile |$profile:ident| $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<(), Error>> {
            Box::pin(async move {
                let request = {
                    let $profile = self.profile();
                    let request: $request_ty = $request?;
                    request
                };
                self.execute(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        plain [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<(), Error>> {
            Box::pin(async move {
                let request: $request_ty = $request;
                self.execute(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            $(where $gate:tt $(+ $extra:tt)*)?
            = delegate $target:ident($($delegated:expr),*);
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
            self.$target($($delegated),*)
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = checked $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
            Box::pin(async move {
                let request: $request_ty = $request?;
                self.submit_applied(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)?
            = with_core |$core:ident| $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
            Box::pin(async move {
                let request = {
                    let $core = self.core();
                    let request: $request_ty = $request?;
                    request
                };
                self.submit_applied(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynAppliedOperation, Error>> {
            Box::pin(async move {
                let request: $request_ty = $request;
                self.submit_applied(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        targeted [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = checked $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
            Box::pin(async move {
                let request: $request_ty = $request?;
                self.submit_targeted(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        targeted [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)?
            = with_profile |$profile:ident| $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
            Box::pin(async move {
                let request = {
                    let $profile = self.profile();
                    let request: $request_ty = $request?;
                    request
                };
                self.submit_targeted(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        targeted [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
            -> $request_ty:ty $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        fn $method(&self, $($arg: $ty),*) -> DynFuture<'_, Result<DynTargetedOperation, Error>> {
            Box::pin(async move {
                let request: $request_ty = $request;
                self.submit_targeted(&request).await
            })
        }
        dyn_noun_impls!($($rest)*);
    };
}

/// Object-safe power noun.
pub trait DynPower: Send + Sync {
    noun_table!(Power => dyn_noun_declarations);
}

/// Object-safe zoom noun.
pub trait DynZoom: Send + Sync {
    noun_table!(Zoom => dyn_noun_declarations);
}

/// Object-safe system noun.
pub trait DynSystem: Send + Sync {
    noun_table!(System => dyn_noun_declarations);
}

/// Object-safe pan/tilt noun.
pub trait DynPanTilt: Send + Sync {
    noun_table!(PanTilt => dyn_noun_declarations);
}

/// Object-safe focus noun.
pub trait DynFocus: Send + Sync {
    noun_table!(Focus => dyn_noun_declarations);
}

/// Object-safe preset noun.
pub trait DynPresets: Send + Sync {
    noun_table!(Presets => dyn_noun_declarations);
}

/// Object-safe exposure noun.
pub trait DynExposure: Send + Sync {
    noun_table!(Exposure => dyn_noun_declarations);
}

/// Object-safe white-balance noun.
pub trait DynWhiteBalance: Send + Sync {
    noun_table!(WhiteBalance => dyn_noun_declarations);
}

/// Object-safe image-processing noun.
pub trait DynImage: Send + Sync {
    noun_table!(Image => dyn_noun_declarations);
}

/// Object-safe tally noun.
pub trait DynTally: Send + Sync {
    noun_table!(Tally => dyn_noun_declarations);
}

/// Object-safe neutral-density filter noun.
pub trait DynNdFilter: Send + Sync {
    noun_table!(NdFilter => dyn_noun_declarations);
}

/// Object-safe motion-sync noun.
pub trait DynMotionSync: Send + Sync {
    noun_table!(MotionSync => dyn_noun_declarations);
}

/// Object-safe on-screen menu noun.
pub trait DynMenu: Send + Sync {
    noun_table!(Menu => dyn_noun_declarations);
}

/// Object-safe advanced/vendor noun.
pub trait DynAdvanced: Send + Sync {
    noun_table!(Advanced => dyn_noun_declarations);
}

/// Object-safe motion safety and observation noun.
pub trait DynMotion: Send + Sync {
    /// Stops all supported pan/tilt, zoom, and focus movement.
    fn stop_all_motion(&self) -> DynFuture<'_, Result<(), Error>>;
    /// Reports whether any mechanical movement axis is moving.
    ///
    /// This samples [`AffectedAxes::MOVEMENT`] with the default tolerance; use
    /// [`Self::is_moving_axes`] to pick the axes or the tolerance.
    ///
    /// [`AffectedAxes::MOVEMENT`]: crate::AffectedAxes::MOVEMENT
    fn is_moving(&self) -> DynFuture<'_, Result<bool, Error>>;
    /// Reports whether the selected physical axes are moving.
    fn is_moving_axes(&self, query: MotionQuery) -> DynFuture<'_, Result<bool, Error>>;
    /// Waits until the selected physical axes become idle.
    fn wait_until_idle(&self, wait: IdleWait) -> DynFuture<'_, Result<(), Error>>;
}

/// Object-safe accessors for all final dynamic nouns.
pub trait DynSessionCameraNouns: Send + Sync {
    /// Returns the dynamic power noun.
    fn power(&self) -> &dyn DynPower;
    /// Returns the dynamic zoom noun.
    fn zoom(&self) -> &dyn DynZoom;
    /// Returns the dynamic system noun.
    fn system(&self) -> &dyn DynSystem;
    /// Returns the dynamic pan/tilt noun.
    fn pan_tilt(&self) -> &dyn DynPanTilt;
    /// Returns the dynamic focus noun.
    fn focus(&self) -> &dyn DynFocus;
    /// Returns the dynamic exposure noun.
    fn exposure(&self) -> &dyn DynExposure;
    /// Returns the dynamic white-balance noun.
    fn white_balance(&self) -> &dyn DynWhiteBalance;
    /// Returns the dynamic image noun.
    fn image(&self) -> &dyn DynImage;
    /// Returns the dynamic preset noun.
    fn presets(&self) -> &dyn DynPresets;
    /// Returns the dynamic tally noun.
    fn tally(&self) -> &dyn DynTally;
    /// Returns the dynamic ND-filter noun.
    fn nd_filter(&self) -> &dyn DynNdFilter;
    /// Returns the dynamic motion-sync noun.
    fn motion_sync(&self) -> &dyn DynMotionSync;
    /// Returns the dynamic menu noun.
    fn menu(&self) -> &dyn DynMenu;
    /// Returns the dynamic advanced/vendor noun.
    fn advanced(&self) -> &dyn DynAdvanced;
    /// Returns the dynamic motion safety and observation noun.
    fn motion(&self) -> &dyn DynMotion;
}

impl DynSessionCamera {
    /// Returns the dynamic power noun.
    pub fn power(&self) -> &dyn DynPower {
        self
    }

    /// Returns the dynamic zoom noun.
    pub fn zoom(&self) -> &dyn DynZoom {
        self
    }

    /// Returns the dynamic system noun.
    pub fn system(&self) -> &dyn DynSystem {
        self
    }

    /// Returns the dynamic pan/tilt noun.
    pub fn pan_tilt(&self) -> &dyn DynPanTilt {
        self
    }

    /// Returns the dynamic focus noun.
    pub fn focus(&self) -> &dyn DynFocus {
        self
    }

    /// Returns the dynamic exposure noun.
    pub fn exposure(&self) -> &dyn DynExposure {
        self
    }

    /// Returns the dynamic white-balance noun.
    pub fn white_balance(&self) -> &dyn DynWhiteBalance {
        self
    }

    /// Returns the dynamic image noun.
    pub fn image(&self) -> &dyn DynImage {
        self
    }

    /// Returns the dynamic preset noun.
    pub fn presets(&self) -> &dyn DynPresets {
        self
    }

    /// Returns the dynamic tally noun.
    pub fn tally(&self) -> &dyn DynTally {
        self
    }

    /// Returns the dynamic ND-filter noun.
    pub fn nd_filter(&self) -> &dyn DynNdFilter {
        self
    }

    /// Returns the dynamic motion-sync noun.
    pub fn motion_sync(&self) -> &dyn DynMotionSync {
        self
    }

    /// Returns the dynamic menu noun.
    pub fn menu(&self) -> &dyn DynMenu {
        self
    }

    /// Returns the dynamic advanced/vendor noun.
    pub fn advanced(&self) -> &dyn DynAdvanced {
        self
    }
}

impl DynSessionCameraNouns for DynSessionCamera {
    fn power(&self) -> &dyn DynPower {
        Self::power(self)
    }
    fn zoom(&self) -> &dyn DynZoom {
        Self::zoom(self)
    }
    fn system(&self) -> &dyn DynSystem {
        Self::system(self)
    }
    fn pan_tilt(&self) -> &dyn DynPanTilt {
        Self::pan_tilt(self)
    }
    fn focus(&self) -> &dyn DynFocus {
        Self::focus(self)
    }
    fn exposure(&self) -> &dyn DynExposure {
        Self::exposure(self)
    }
    fn white_balance(&self) -> &dyn DynWhiteBalance {
        Self::white_balance(self)
    }
    fn image(&self) -> &dyn DynImage {
        Self::image(self)
    }
    fn presets(&self) -> &dyn DynPresets {
        Self::presets(self)
    }
    fn tally(&self) -> &dyn DynTally {
        Self::tally(self)
    }
    fn nd_filter(&self) -> &dyn DynNdFilter {
        Self::nd_filter(self)
    }
    fn motion_sync(&self) -> &dyn DynMotionSync {
        Self::motion_sync(self)
    }
    fn menu(&self) -> &dyn DynMenu {
        Self::menu(self)
    }
    fn advanced(&self) -> &dyn DynAdvanced {
        Self::advanced(self)
    }
    fn motion(&self) -> &dyn DynMotion {
        self
    }
}

impl DynPower for DynSessionCamera {
    noun_table!(Power => dyn_noun_impls);
}

impl DynZoom for DynSessionCamera {
    noun_table!(Zoom => dyn_noun_impls);
}

impl DynSystem for DynSessionCamera {
    noun_table!(System => dyn_noun_impls);
}

impl DynPanTilt for DynSessionCamera {
    noun_table!(PanTilt => dyn_noun_impls);
}

impl DynFocus for DynSessionCamera {
    noun_table!(Focus => dyn_noun_impls);
}

impl DynPresets for DynSessionCamera {
    noun_table!(Presets => dyn_noun_impls);
}

impl DynExposure for DynSessionCamera {
    noun_table!(Exposure => dyn_noun_impls);
}

impl DynWhiteBalance for DynSessionCamera {
    noun_table!(WhiteBalance => dyn_noun_impls);
}

impl DynImage for DynSessionCamera {
    noun_table!(Image => dyn_noun_impls);
}

impl DynTally for DynSessionCamera {
    noun_table!(Tally => dyn_noun_impls);
}

impl DynNdFilter for DynSessionCamera {
    noun_table!(NdFilter => dyn_noun_impls);
}

impl DynMotionSync for DynSessionCamera {
    noun_table!(MotionSync => dyn_noun_impls);
}

impl DynMenu for DynSessionCamera {
    noun_table!(Menu => dyn_noun_impls);
}

impl DynAdvanced for DynSessionCamera {
    noun_table!(Advanced => dyn_noun_impls);
}

impl DynMotion for DynSessionCamera {
    fn stop_all_motion(&self) -> DynFuture<'_, Result<(), Error>> {
        self.stop_all_motion()
    }

    fn is_moving(&self) -> DynFuture<'_, Result<bool, Error>> {
        DynSessionCamera::is_moving(self, MotionQuery::default())
    }

    fn is_moving_axes(&self, query: MotionQuery) -> DynFuture<'_, Result<bool, Error>> {
        DynSessionCamera::is_moving(self, query)
    }

    fn wait_until_idle(&self, wait: IdleWait) -> DynFuture<'_, Result<(), Error>> {
        self.wait_until_idle(wait)
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use std::collections::HashSet;

    use crate::command::{
        inquiry_structs::{BuiltinInquiryQuery, BUILTIN_INQUIRIES, BUILTIN_INQUIRY_ACCESSORS},
        semantics::{BuiltinCommand, BuiltinRequestClass},
        surface::{surface_entry, StaticSurfaceDisposition},
    };
    use crate::noun_parity::{consumed_nouns, dyn_trait_noun_key, table_surface};

    use super::{
        DYN_NOUN_CONVENIENCE_METHODS, DYN_NOUN_CONVENIENCE_METHOD_COUNT, DYN_NOUN_COUNT,
        DYN_NOUN_INQUIRY_METHOD_COUNT, DYN_NOUN_TARGET_METHOD_COUNT,
    };

    #[test]
    fn declared_convenience_wrappers_exist_and_are_disjoint_from_the_ledger() {
        let source = include_str!("nouns.rs");
        let table = table_surface();

        assert_eq!(
            DYN_NOUN_CONVENIENCE_METHODS.len(),
            DYN_NOUN_CONVENIENCE_METHOD_COUNT,
        );

        let mut ledger_methods = HashSet::new();
        for command in BuiltinCommand::ALL {
            if let StaticSurfaceDisposition::Noun { method, .. } =
                surface_entry(*command).disposition
            {
                ledger_methods.insert(method);
            }
        }

        let mut declared = HashSet::new();
        for (noun, method) in DYN_NOUN_CONVENIENCE_METHODS {
            assert!(
                declared.insert((*noun, *method)),
                "duplicate convenience entry {noun}::{method}",
            );
            assert!(
                source.contains(&format!("pub trait {noun}:")),
                "convenience entry names an unknown dynamic noun {noun}",
            );
            // The declaration itself now lives in the one shared row table
            // every facade is generated from, so that is where it is looked up.
            let key = dyn_trait_noun_key(noun);
            assert!(
                table[key].contains_key(*method),
                "convenience method {noun}::{method} is not declared",
            );
            // A convenience wrapper must never shadow a ledger spelling: that
            // would let a command row silently lose its own method.
            assert!(
                !ledger_methods.contains(method),
                "convenience method {noun}::{method} collides with a ledger row spelling",
            );
        }
    }

    /// Every dynamic noun trait is generated from the shared table, and the
    /// table row set of each is exactly what the trait declares.
    #[test]
    fn every_dynamic_noun_is_generated_from_the_shared_table() {
        let source = include_str!("nouns.rs");
        let declarations = consumed_nouns(source, "src/dynapi/nouns.rs", "dyn_noun_declarations");
        let implementations = consumed_nouns(source, "src/dynapi/nouns.rs", "dyn_noun_impls");
        assert_eq!(
            declarations, implementations,
            "a dynamic noun trait and its impl disagree on which table rows they carry",
        );
        assert_eq!(declarations.len(), DYN_NOUN_COUNT);
        assert_eq!(declarations, table_surface().keys().cloned().collect());
    }

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
        assert_eq!(untyped_queryable, ["DefogModeInquiry", "NrSpeedInquiry"]);
    }
}
