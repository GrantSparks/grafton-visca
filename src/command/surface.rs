//! Closed static camera-surface metadata for built-in requests.
//!
//! [`crate::command::semantics::BuiltinCommand::ALL`] is the only command-row
//! inventory.  This module adds no second list: its exhaustive match derives
//! noun spelling and marker facts for each existing semantic row.  Adding a
//! new command therefore requires the source inventory, semantic
//! classification, and this surface decision to be updated together.

use crate::{capabilities::TypedSupportSurface, noun_table::noun_table};

use super::semantics::{BuiltinCommand, BuiltinRequestClass};

// MSRV note: Rust 1.88's dead-code analysis does not follow uses produced by
// the continuation-style `noun_table!` expansion or by the const ledger below.
// These items are intentionally retained as production metadata, so scope the
// allowances to this ledger (and only its genuinely unused marker variants)
// instead of disabling dead-code diagnostics for the module.

/// Static noun containing one target-facing built-in request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum StaticNoun {
    /// Power controls.
    Power,
    /// Zoom controls.
    Zoom,
    /// System controls.
    System,
    /// Pan/tilt controls.
    PanTilt,
    /// Focus controls.
    Focus,
    /// Exposure and iris controls.
    Exposure,
    /// White-balance and channel controls.
    WhiteBalance,
    /// Image-processing controls.
    Image,
    /// Preset controls.
    Presets,
    /// Tally controls.
    Tally,
    /// Neutral-density filter controls.
    NdFilter,
    /// Motion-sync controls.
    MotionSync,
    /// Menu controls.
    Menu,
    /// Advanced and vendor controls.
    Advanced,
}

/// Profile or typed-capability marker required by a static surface row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StaticMarkerRequirement {
    /// The base profile/domain marker is sufficient.
    #[allow(dead_code)]
    None,
    /// A compile-time profile marker trait is required.
    #[allow(dead_code)]
    Profile(&'static str),
    /// A runtime/static typed capability gate is required.
    Typed(TypedSupportSurface),
}

/// What kind of public surface disposition a built-in request has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum StaticSurfaceDisposition {
    /// A normal target-facing noun method.
    Noun {
        /// Noun accessor containing the method.
        noun: StaticNoun,
        /// Canonical method spelling shared by all facades.
        method: &'static str,
        /// Compile-time/runtime support marker.
        marker: StaticMarkerRequirement,
    },
    /// A broadcast handshake, never a target camera method.
    BroadcastHandshake {
        /// Internal protocol spelling retained for the broadcast namespace.
        method: &'static str,
    },
    /// An internal cancellation primitive, never a noun method.
    InternalCancellation {
        /// Internal protocol spelling retained for owner cancellation.
        method: &'static str,
    },
}

/// One derived row in the closed static noun ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) struct StaticSurfaceEntry {
    /// Exactly one source semantic row.
    pub(crate) command: BuiltinCommand,
    /// Noun mapping or explicit non-noun exception.
    pub(crate) disposition: StaticSurfaceDisposition,
    /// Semantic return class copied from the authoritative row.
    pub(crate) class: BuiltinRequestClass,
}

impl StaticSurfaceEntry {
    /// Returns whether the row may become a target camera noun method.
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn is_target_facing(self) -> bool {
        matches!(self.disposition, StaticSurfaceDisposition::Noun { .. })
    }
}

macro_rules! noun_marker {
    (Power) => {
        StaticMarkerRequirement::Profile("HasPower")
    };
    (Zoom) => {
        StaticMarkerRequirement::Profile("HasZoom")
    };
    (System) => {
        StaticMarkerRequirement::None
    };
    (PanTilt) => {
        StaticMarkerRequirement::Profile("HasPanTilt")
    };
    (Focus) => {
        StaticMarkerRequirement::Profile("HasFocus")
    };
    (Exposure) => {
        StaticMarkerRequirement::Profile("HasExposure")
    };
    (WhiteBalance) => {
        StaticMarkerRequirement::Profile("HasWhiteBalance")
    };
    (Image) => {
        StaticMarkerRequirement::Profile("HasImageProcessing")
    };
    (Presets) => {
        StaticMarkerRequirement::Profile("HasPresets")
    };
    (Tally) => {
        StaticMarkerRequirement::Typed(TypedSupportSurface::Tally)
    };
    (NdFilter) => {
        StaticMarkerRequirement::Typed(TypedSupportSurface::NdFilter)
    };
    (MotionSync) => {
        StaticMarkerRequirement::Typed(TypedSupportSurface::MotionSync)
    };
    (Menu) => {
        StaticMarkerRequirement::Profile("HasMenuControl")
    };
    (Advanced) => {
        StaticMarkerRequirement::None
    };
}

macro_rules! broadcast_entry {
    ($command:ident, $method:expr) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::BroadcastHandshake { method: $method },
            class: registry_class(BuiltinCommand::$command, RegistryKind::Plain),
        }
    };
}

macro_rules! internal_entry {
    ($command:ident, $method:expr) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::InternalCancellation { method: $method },
            class: registry_class(BuiltinCommand::$command, RegistryKind::Plain),
        }
    };
}

/// The request kind recorded by one noun-table row.
///
/// `BuiltinRequestClass` carries more information than a facade needs (axes,
/// state effects, and completion policy).  Keeping this small projection in
/// the surface consumer lets the registry check that its `plain`/`applied`/
/// `targeted` spelling agrees with the authoritative semantic ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
enum RegistryKind {
    Plain,
    AppliedOnly,
    Targeted,
}

/// Resolve a registry kind against the authoritative semantic classification.
///
/// This is deliberately a const function with no fallback: a row whose kind
/// does not match its command ID fails during const evaluation below, before a
/// facade can accidentally expose the wrong operation handle.
#[allow(clippy::panic)]
#[allow(dead_code)]
const fn registry_class(command: BuiltinCommand, kind: RegistryKind) -> BuiltinRequestClass {
    let class = command.classification();
    match (kind, class) {
        (RegistryKind::Plain, BuiltinRequestClass::Plain { .. })
        | (RegistryKind::AppliedOnly, BuiltinRequestClass::AppliedOnly { .. })
        | (RegistryKind::Targeted, BuiltinRequestClass::Targeted { .. }) => class,
        _ => panic!("noun-table request kind disagrees with BuiltinCommand::classification"),
    }
}

/// Human-readable closed-inventory totals for the built-in surface.
///
/// There are 149 command IDs: 146 target-facing noun rows and three protocol
/// exceptions (the two broadcast handshakes and internal cancellation).
#[allow(dead_code)]
pub(crate) const BUILTIN_COMMAND_COUNT: usize = 149;
#[allow(dead_code)]
pub(crate) const TARGET_FACING_COMMAND_COUNT: usize = 146;
#[allow(dead_code)]
pub(crate) const NON_NOUN_COMMAND_COUNT: usize = 3;

macro_rules! registry_class {
    (plain) => {
        RegistryKind::Plain
    };
    (applied) => {
        RegistryKind::AppliedOnly
    };
    (targeted) => {
        RegistryKind::Targeted
    };
}

/// Convert a row's static capability spelling to the surface marker
/// requirement.  The noun table owns the spelling; this map keeps the
/// TypedSupportSurface classification in the surface consumer only.
macro_rules! surface_marker {
    (HasDirectZoom) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::DirectZoom,
        )
    };
    (HasDigitalZoomToggle) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::DigitalZoomToggle,
        )
    };
    (HasDigitalZoomRange) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::DigitalZoomRange,
        )
    };
    (HasIrisControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::IrisControl,
        )
    };
    (HasOnePushFocus) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::OnePushFocus,
        )
    };
    (HasPtzOpticsSnapFocus) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::PtzOpticsSnapFocus,
        )
    };
    (HasFocusLock) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::FocusLock,
        )
    };
    (HasPushAutoFocus) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::PushAutoFocus,
        )
    };
    (HasFocusZone) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::FocusZone,
        )
    };
    (HasAutoFocusSensitivity) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::AutoFocusSensitivity,
        )
    };
    (HasFocusNearLimitInquiry) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::FocusNearLimitInquiry,
        )
    };
    (HasBacklightCompensation) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::BacklightCompensation,
        )
    };
    (HasWideDynamicRange) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::WideDynamicRange,
        )
    };
    (HasExposureCompensation) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::ExposureCompensation,
        )
    };
    (HasBrightnessControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::BrightnessControl,
        )
    };
    (HasOnePushWhiteBalance) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::OnePushWhiteBalance,
        )
    };
    (HasAutoTrackingWhiteBalance) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::AutoTrackingWhiteBalance,
        )
    };
    (HasAutoWhiteBalanceSensitivity) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::AutoWhiteBalanceSensitivity,
        )
    };
    (HasColorTemperature) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::ColorTemperature,
        )
    };
    (HasRgbGain) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::RgbGain,
        )
    };
    (HasRgbTuning) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::RgbTuning,
        )
    };
    (HasImageFlip) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::ImageFlip,
        )
    };
    (HasImageMirror) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::ImageMirror,
        )
    };
    (HasCombinedImageFlip) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::CombinedImageFlip,
        )
    };
    (HasContrastControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::ContrastControl,
        )
    };
    (HasSharpnessControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::SharpnessControl,
        )
    };
    (HasSaturationControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::SaturationControl,
        )
    };
    (HasHueControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::HueControl,
        )
    };
    (HasLuminanceControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::LuminanceControl,
        )
    };
    (HasGammaControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::GammaControl,
        )
    };
    (HasNoiseReduction) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::NoiseReduction,
        )
    };
    (HasNoiseReduction2D) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
        )
    };
    (HasNoiseReduction3D) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
        )
    };
    (HasPictureEffect) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::PictureEffect,
        )
    };
    (HasTally) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::Tally,
        )
    };
    (HasDirectMenuControl) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::DirectMenu,
        )
    };
    (HasNdFilter) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::NdFilter,
        )
    };
    (HasVariableSpeed) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::VariableSpeed,
        )
    };
    (HasMotionSync) => {
        $crate::command::surface::StaticMarkerRequirement::Typed(
            $crate::capabilities::TypedSupportSurface::MotionSync,
        )
    };
}

/// Resolve a profile marker name to the runtime typed-support surface that
/// backs it.  The noun-table marker projection above remains the authority;
/// this adapter lets generated built-in inquiries reuse the same mapping
/// without maintaining a second per-inquiry table.
macro_rules! typed_surface_for_marker {
    (crate :: capabilities :: $marker:ident) => {
        $crate::command::surface::typed_surface_for_marker!($marker)
    };
    ($marker:ident) => {{
        match $crate::command::surface::surface_marker!($marker) {
            $crate::command::surface::StaticMarkerRequirement::Typed(surface) => surface,
            $crate::command::surface::StaticMarkerRequirement::None
            | $crate::command::surface::StaticMarkerRequirement::Profile(_) => {
                unreachable!("typed inquiry gate must map to a typed support surface")
            }
        }
    }};
}

pub(crate) use surface_marker;
pub(crate) use typed_surface_for_marker;

macro_rules! noun_entry {
    ($kind:ident, $command:ident, $noun:ident, $method:expr) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::Noun {
                noun: StaticNoun::$noun,
                method: $method,
                marker: noun_marker!($noun),
            },
            class: registry_class(BuiltinCommand::$command, registry_class!($kind)),
        }
    };
    ($kind:ident, $command:ident, $noun:ident, $method:expr, $marker:expr) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::Noun {
                noun: StaticNoun::$noun,
                method: $method,
                marker: $marker,
            },
            class: registry_class(BuiltinCommand::$command, registry_class!($kind)),
        }
    };
}

/// Consume one flat projection of every noun row.
///
/// Inquiry and empty-ID convenience rows are intentionally consumed without
/// emitting a command arm.  Every canonical row emits one arm per explicitly
/// listed BuiltinCommand variant.  The exceptions are rows in the same
/// projection, so the resulting match is complete without a wildcard or a
/// second hand-written inventory.
macro_rules! surface_rows {
    (@start $input:ident; [$($arms:tt)*]; @noun $noun:ident; $($rest:tt)*) => {
        surface_rows!(@rows $input; $noun; [$($arms)*]; $($rest)*)
    };

    (@rows $input:ident; $noun:ident; [$($arms:tt)*]; @noun $next:ident; $($rest:tt)*) => {
        surface_rows!(@rows $input; $next; [$($arms)*]; $($rest)*)
    };

    // Append one command row.  The gate is split into separate arms so the
    // command-list repetition below can reuse it without a repetition-depth
    // mismatch in macro_rules.
    (
        @append $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $kind:ident;
        [$($command:ident),*];
        $method:expr;
        ;
        $($rest:tt)*
    ) => {
        surface_rows!(@rows $input; $noun; [
            $($arms)*
            $(
                BuiltinCommand::$command => noun_entry!(
                    $kind,
                    $command,
                    $noun,
                    $method,
                    noun_marker!($noun)
                ),
            )*
        ]; $($rest)*)
    };

    (
        @append $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $kind:ident;
        [$($command:ident),*];
        $method:expr;
        $gate:ident;
        $($rest:tt)*
    ) => {
        surface_rows!(@rows $input; $noun; [
            $($arms)*
            $(
                BuiltinCommand::$command => noun_entry!(
                    $kind,
                    $command,
                    $noun,
                    $method,
                    surface_marker!($gate)
                ),
            )*
        ]; $($rest)*)
    };

    (
        @rows $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        @exceptions;
        $($rest:tt)*
    ) => {
        surface_rows!(@exceptions $input; [$($arms)*]; $($rest)*)
    };

    // Consume one row at a time.  Keeping the row grammar non-repetitive avoids
    // a local ambiguity between the row's documentation attributes and the
    // trailing token stream.
    (
        @rows $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $(#[$doc:meta])*
        inquiry $($inquiry:ident)::+ $method:ident() -> $response:ty
            $(where $gate:ident $(+ $extra:ident)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        surface_rows!(@rows $input; $noun; [$($arms)*]; $($rest)*)
    };

    (
        @rows $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident(
            $($arg:ident: $ty:ty),*
        ) $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)?
            = checked $request:expr;
        $($rest:tt)*
    ) => {
        surface_rows!(@append $input; $noun; [$($arms)*]; $kind; [$($command),*]; stringify!($method); $($gate)?; $($rest)*)
    };

    (
        @rows $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident(
            $($arg:ident: $ty:ty),*
        ) $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)?
            = with_profile |$profile:ident| $request:expr;
        $($rest:tt)*
    ) => {
        surface_rows!(@append $input; $noun; [$($arms)*]; $kind; [$($command),*]; stringify!($method); $($gate)?; $($rest)*)
    };

    (
        @rows $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident(
            $($arg:ident: $ty:ty),*
        ) $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)?
            = with_core |$core:ident| $request:expr;
        $($rest:tt)*
    ) => {
        surface_rows!(@append $input; $noun; [$($arms)*]; $kind; [$($command),*]; stringify!($method); $($gate)?; $($rest)*)
    };

    (
        @rows $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident(
            $($arg:ident: $ty:ty),*
        ) $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)?
            = delegate $target:ident($($delegated:expr),*);
        $($rest:tt)*
    ) => {
        surface_rows!(@append $input; $noun; [$($arms)*]; $kind; [$($command),*]; stringify!($method); $($gate)?; $($rest)*)
    };

    (
        @rows $input:ident;
        $noun:ident;
        [$($arms:tt)*];
        $(#[$doc:meta])*
        $kind:ident [$($command:ident),*] $method:ident(
            $($arg:ident: $ty:ty),*
        ) $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)?
            = $request:expr;
        $($rest:tt)*
    ) => {
        surface_rows!(@append $input; $noun; [$($arms)*]; $kind; [$($command),*]; stringify!($method); $($gate)?; $($rest)*)
    };

    (@exceptions $input:ident; [$($arms:tt)*]; broadcast [$($command:ident),*] $method:ident;
        $($rest:tt)*) => {
        surface_rows!(@exceptions $input; [
            $($arms)*
            $(
                BuiltinCommand::$command => broadcast_entry!($command, stringify!($method)),
            )*
        ]; $($rest)*)
    };

    (@exceptions $input:ident; [$($arms:tt)*]; internal [$($command:ident),*] $method:ident;
        $($rest:tt)*) => {
        surface_rows!(@exceptions $input; [
            $($arms)*
            $(
                BuiltinCommand::$command => internal_entry!($command, stringify!($method)),
            )*
        ]; $($rest)*)
    };

    (@exceptions $input:ident; [$($arms:tt)*];) => {
        match $input {
            $($arms)*
        }
    };

    // The public projection always starts with a noun marker.  Restrict this
    // entry arm to that shape so recursive dispatcher calls cannot match it.
    ($input:ident; @noun $noun:ident; $($rest:tt)*) => {
        surface_rows!(@start $input; []; @noun $noun; $($rest)*)
    };
}

/// Derive the one surface row for a semantic command.
///
/// This match is intentionally exhaustive and contains no default arm.  The
/// row list is supplied by noun_table!(All => surface_rows), so adding a
/// command requires adding its explicit owner row there.  BuiltinCommand
/// remains the semantic authority for classification.
#[must_use]
#[deny(unreachable_patterns)]
#[allow(dead_code)]
pub(crate) const fn surface_entry(command: BuiltinCommand) -> StaticSurfaceEntry {
    macro_rules! surface_rows_for_entry {
        ($($rows:tt)*) => {
            surface_rows!(command; $($rows)*)
        };
    }

    noun_table!(All => surface_rows_for_entry)
}

/// Force every registry arm through const evaluation.
///
/// `surface_entry` is also called by runtime tests, but those calls alone
/// would not evaluate the `registry_class` checks until the tests run.  This
/// item makes a mismatched row kind a compile-time failure for every command
/// ID in the closed semantic inventory.
const _: () = {
    let mut index = 0;
    while index < BuiltinCommand::ALL.len() {
        let _ = surface_entry(BuiltinCommand::ALL[index]);
        index += 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_ledger_is_exhaustive_unique_and_class_balanced() {
        let mut seen = std::collections::HashSet::new();
        let mut plain = 0;
        let mut applied_only = 0;
        let mut targeted = 0;
        for command in BuiltinCommand::ALL {
            let entry = surface_entry(*command);
            assert!(seen.insert(*command), "duplicate surface ledger row");
            assert_eq!(entry.command, *command);
            assert_eq!(entry.class, command.classification());
            match entry.class {
                BuiltinRequestClass::Plain { .. } => plain += 1,
                BuiltinRequestClass::AppliedOnly { .. } => applied_only += 1,
                BuiltinRequestClass::Targeted { .. } => targeted += 1,
            }
            match entry.disposition {
                StaticSurfaceDisposition::Noun { method, .. }
                | StaticSurfaceDisposition::BroadcastHandshake { method }
                | StaticSurfaceDisposition::InternalCancellation { method } => {
                    assert!(!method.is_empty());
                }
            }
        }

        // The class distribution is pinned, not re-derived.  Summing the three
        // counters back to `ALL.len()` proves nothing — the match above is
        // exhaustive, so the sum is an identity — while the split between the
        // classes is exactly what a silent reclassification changes.
        //
        // Maintenance: adding a command row moves one of these three numbers by
        // one, and reclassifying a row moves two.  Update the triple in the
        // same commit as the ledger change and say in the commit message which
        // class the row landed in; a change here that nobody intended is the
        // regression this assertion exists to surface.
        assert_eq!(
            (plain, applied_only, targeted),
            (118, 16, 15),
            "the semantic class distribution of the closed ledger changed",
        );
        assert_eq!(seen.len(), BuiltinCommand::ALL.len());
        assert_eq!(BuiltinCommand::ALL.len(), BUILTIN_COMMAND_COUNT);
        assert_eq!(
            seen.iter()
                .filter(|command| surface_entry(**command).is_target_facing())
                .count(),
            TARGET_FACING_COMMAND_COUNT,
        );
        assert_eq!(
            BuiltinCommand::ALL.len() - TARGET_FACING_COMMAND_COUNT,
            NON_NOUN_COMMAND_COUNT,
        );
    }

    /// Issue #651: paired on/off rows agree on their capability gate.
    ///
    /// Both halves of a toggle must be reachable from the same set of
    /// profiles, or a profile can reach one direction and get stuck there.
    /// The noise-reduction pairs are the case that motivated this: the level
    /// newtypes are bounded `1..=5` and `1..=8`, so `set_noise_reduction_*`
    /// cannot express off and the disable rows are the only way to send the
    /// `0x00` wire value.
    ///
    /// `set_flip_both` and `set_flip_mode` are here for the same reason.  They
    /// send the *same* combined opcode, so a split gate would let a profile
    /// send that opcode's `Both` without being able to send its `Off`.
    #[test]
    fn paired_rows_share_one_capability_gate() {
        /// `None` marks a non-noun row, which the assertions below reject
        /// rather than skip: a pair that stopped being a noun pair would
        /// otherwise satisfy this test vacuously.
        fn facts(command: BuiltinCommand) -> (&'static str, Option<StaticMarkerRequirement>) {
            match surface_entry(command).disposition {
                StaticSurfaceDisposition::Noun { method, marker, .. } => (method, Some(marker)),
                StaticSurfaceDisposition::BroadcastHandshake { method }
                | StaticSurfaceDisposition::InternalCancellation { method } => (method, None),
            }
        }

        for (on, off) in [
            (
                BuiltinCommand::NoiseReduction2d,
                BuiltinCommand::NoiseReduction2dOff,
            ),
            (
                BuiltinCommand::NoiseReduction3d,
                BuiltinCommand::NoiseReduction3dOff,
            ),
            (
                BuiltinCommand::ImageFlipHorizontal,
                BuiltinCommand::ImageFlipHorizontalOff,
            ),
            (
                BuiltinCommand::ImageFlipOff,
                BuiltinCommand::ImageFlipVertical,
            ),
            (
                BuiltinCommand::ImageFlipBoth,
                BuiltinCommand::ImageFlipCombined,
            ),
        ] {
            let (on_method, on_marker) = facts(on);
            let (off_method, off_marker) = facts(off);
            assert!(
                on_marker.is_some(),
                "`{on_method}` stopped being a noun row, so this pair no \
                 longer proves anything",
            );
            assert_eq!(
                on_marker, off_marker,
                "`{on_method}` and `{off_method}` are two directions of one \
                 toggle but are gated differently, so a profile can reach one \
                 direction and not the other",
            );
        }
    }

    #[test]
    fn broadcast_and_internal_rows_never_have_noun_dispositions() {
        assert!(matches!(
            surface_entry(BuiltinCommand::AddressSet).disposition,
            StaticSurfaceDisposition::BroadcastHandshake { .. }
        ));
        assert!(matches!(
            surface_entry(BuiltinCommand::InterfaceClear).disposition,
            StaticSurfaceDisposition::BroadcastHandshake { .. }
        ));
        assert!(matches!(
            surface_entry(BuiltinCommand::CommandCancel).disposition,
            StaticSurfaceDisposition::InternalCancellation { .. }
        ));

        // The target-facing total is derived: the ledger is closed, so the
        // noun rows are exactly the rows that are not one of the three named
        // non-noun exceptions above.
        let exceptions: Vec<BuiltinCommand> = BuiltinCommand::ALL
            .iter()
            .copied()
            .filter(|command| !surface_entry(*command).is_target_facing())
            .collect();
        assert_eq!(
            exceptions,
            vec![
                BuiltinCommand::AddressSet,
                BuiltinCommand::InterfaceClear,
                BuiltinCommand::CommandCancel,
            ]
        );

        // Counting the noun rows and adding the exceptions back is an identity,
        // because `is_target_facing` is what splits the two groups.  The
        // property worth asserting is that the two groups do not overlap in
        // method spelling: the broadcast and cancellation rows own protocol
        // names that must never resurface as a camera noun method on any of the
        // three facades.
        let reserved: Vec<&'static str> = BuiltinCommand::ALL
            .iter()
            .filter_map(|command| match surface_entry(*command).disposition {
                StaticSurfaceDisposition::BroadcastHandshake { method }
                | StaticSurfaceDisposition::InternalCancellation { method } => Some(method),
                StaticSurfaceDisposition::Noun { .. } => None,
            })
            .collect();
        assert_eq!(reserved.len(), exceptions.len());
        for command in BuiltinCommand::ALL {
            if let StaticSurfaceDisposition::Noun { noun, method, .. } =
                surface_entry(*command).disposition
            {
                assert!(
                    !reserved.contains(&method),
                    "noun {noun:?} reuses the reserved non-noun spelling {method}",
                );
            }
        }
    }
}
