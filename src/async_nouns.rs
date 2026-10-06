//! Static, owner-backed noun accessors for the async camera facade.
//!
//! Each accessor is a thin view over [`crate::async_session::AsyncCameraCore`]
//! through its typed [`crate::Camera`] owner.  Request preparation, profile
//! validation, admission, and operation completion therefore remain in the
//! same path as [`crate::Camera::execute`], [`crate::Camera::inquire`], and
//! [`crate::Camera::submit`].
//!
//! An async noun method borrows its accessor for the returned future. Bind the
//! accessor before passing multiple noun futures to `join!`, `select!`, or a
//! task collection; see the concurrency note in the 2.0 usage guide. A direct
//! `camera.power().on().await` remains valid.
//!
//! The methods themselves are not written here: they are generated from the
//! shared row table in [`crate::noun_table`], which the blocking and erased
//! facades consume from the same rows.  This module owns only what is specific
//! to the async surface — the accessor types, the `async fn` shape, and the
//! `Camera<P>` receiver plumbing.

#![cfg(feature = "async")]

use crate::{
    async_session::{AsyncCameraCore, Camera},
    capabilities::{
        HasAutoFocusSensitivity, HasAutoTrackingWhiteBalance, HasAutoWhiteBalanceSensitivity,
        HasBacklightCompensation, HasBrightnessControl, HasColorTemperature,
        HasColorTemperatureInquiry, HasCombinedImageFlip, HasContrastControl, HasDefogLevel,
        HasDigitalZoomToggle, HasDirectMenuControl, HasDirectZoom, HasExposure,
        HasExposureCompensation, HasExposureMode, HasFocus, HasFocusLock, HasFocusNearLimitInquiry,
        HasFocusZone, HasFocusZoneInquiry, HasGammaControl, HasHueControl, HasImageFlip,
        HasImageFreeze, HasImageMirror, HasImageProcessing, HasIrisControl, HasIrisControlInquiry,
        HasLuminanceControl, HasMenuControl, HasMotionSync, HasNdFilter, HasNoiseReduction2D,
        HasNoiseReduction2DControl, HasNoiseReduction2DMode, HasNoiseReduction3D,
        HasNoiseReduction3DControl, HasOnePushFocus, HasOnePushWhiteBalance, HasPanTilt,
        HasPictureEffect, HasPower, HasPresets, HasPtzOpticsAntiFlicker,
        HasPtzOpticsMulticastStreaming, HasPtzOpticsNdiQuality, HasPtzOpticsPresetRecallSpeed,
        HasPtzOpticsSettingsSave, HasPtzOpticsSnapFocus, HasPtzOpticsTally, HasPushAutoFocus,
        HasRgbGain, HasRgbTuning, HasSaturationControl, HasSharpnessControl,
        HasSonyAutoSlowShutter, HasSonySpotlight, HasTally, HasTallyBrightness, HasUsbAudio,
        HasVariableSpeed, HasVersionInquiry, HasWhiteBalance, HasWideDynamicRange, HasZoom,
    },
    command,
    completion::{AppliedOnly, Targeted},
    noun_facade::noun_request,
    noun_table::{motion_table, noun_table},
    operation::Operation,
    profile::CompileTimeProfile,
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    Result, ZoomDomain,
};

macro_rules! accessor_method {
    ($(#[$meta:meta])* $name:ident, $method:ident) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, P> {
            $name::new(self)
        }
    };
    ($(#[$meta:meta])* $name:ident, $method:ident, $bound:path) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, P>
        where
            P: $bound,
        {
            $name::new(self)
        }
    };
}

macro_rules! accessor {
    ($(#[$meta:meta])* $name:ident, $method:ident $(, $bound:path)? ) => {
        $(#[$meta])*
        #[must_use]
        pub struct $name<'a, P: CompileTimeProfile> {
            camera: &'a Camera<P>,
        }

        impl<'a, P: CompileTimeProfile> std::fmt::Debug for $name<'a, P> {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }

        impl<'a, P: CompileTimeProfile> $name<'a, P> {
            fn new(camera: &'a Camera<P>) -> Self {
                Self { camera }
            }
        }

        impl<P: CompileTimeProfile> Camera<P> {
            accessor_method!($(#[$meta])* $name, $method $(, $bound)?);
        }
    };
}

/// Generates one async noun method per [`noun_table`] row.
///
/// The row grammar is documented on [`crate::noun_table`]; the request form is
/// parsed only by [`noun_request!`]. This consumer carries everything the
/// async surface adds to a row: `pub async fn`, the `Result<..>` alias,
/// `Operation<Kind>` handles without a session lifetime, and the `self.camera`
/// owner hop.
macro_rules! async_noun_methods {
    (@noun $noun:ident { $($header:tt)* };
        $(
            $(#[$doc:meta])*
            $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
        )*
    ) => {
        $(
            async_noun_methods!(@method [$(#[$doc])*] $kind $method($($arg: $ty),*) -> $ret;
                [$($gate $(+ $extra)*)?]; [$($request)*]);
        )*
    };

    (@method [$(#[$doc:meta])*] inquiry $method:ident() -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub async fn $method(&self) -> Result<$ret>
        $(where P: $gate $(+ $extra)*)?
        {
            self.camera.inquire(&$($request)*).await
        }
    };

    (@method [$(#[$doc:meta])*] plain $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub async fn $method(&self, $($arg: $ty),*) -> Result<()>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = noun_request!(self.camera.profile(); $ret; $($request)*)?;
            self.camera.execute(&request).await
        }
    };

    (@method [$(#[$doc:meta])*] applied $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub async fn $method(&self, $($arg: $ty),*) -> Result<Operation<AppliedOnly>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = noun_request!(self.camera.profile(); $ret; $($request)*)?;
            self.camera.submit::<AppliedOnly, _>(&request).await
        }
    };

    (@method [$(#[$doc:meta])*] targeted $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub async fn $method(&self, $($arg: $ty),*) -> Result<Operation<Targeted>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = noun_request!(self.camera.profile(); $ret; $($request)*)?;
            self.camera.submit::<Targeted, _>(&request).await
        }
    };
}

accessor!(
    /// Power commands and inquiries for one camera target.
    PowerAccessor,
    power,
    HasPower
);
accessor!(
    /// Optical and digital zoom commands and inquiries.
    ZoomAccessor,
    zoom,
    HasZoom
);
accessor!(
    /// Camera system persistence and version inquiries.
    SystemAccessor,
    system
);
accessor!(
    /// Pan/tilt movement, limits, and position inquiries.
    PanTiltAccessor,
    pan_tilt,
    HasPanTilt
);
accessor!(
    /// Focus movement, modes, and focus inquiries.
    FocusAccessor,
    focus,
    HasFocus
);
accessor!(
    /// Exposure, iris, shutter, brightness, and gain controls.
    ExposureAccessor,
    exposure,
    HasExposure
);
accessor!(
    /// White-balance and channel controls.
    WhiteBalanceAccessor,
    white_balance,
    HasWhiteBalance
);
accessor!(
    /// Image-processing and orientation controls.
    ImageAccessor,
    image,
    HasImageProcessing
);
accessor!(
    /// Camera preset commands and preset inquiries.
    PresetsAccessor,
    presets,
    HasPresets
);

/// Tally-light commands and inquiries for a profile with tally support.
#[must_use]
pub struct TallyAccessor<'a, P: CompileTimeProfile + HasTally> {
    camera: &'a Camera<P>,
}

impl<'a, P: CompileTimeProfile + HasTally> std::fmt::Debug for TallyAccessor<'a, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TallyAccessor")
            .finish_non_exhaustive()
    }
}

/// ND-filter commands and inquiries for a profile with ND support.
#[must_use]
pub struct NdFilterAccessor<'a, P: CompileTimeProfile + HasNdFilter> {
    camera: &'a Camera<P>,
}

impl<'a, P: CompileTimeProfile + HasNdFilter> std::fmt::Debug for NdFilterAccessor<'a, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NdFilterAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion-sync commands and inquiries for a profile with motion-sync support.
#[must_use]
pub struct MotionSyncAccessor<'a, P: CompileTimeProfile + HasMotionSync> {
    camera: &'a Camera<P>,
}

impl<'a, P: CompileTimeProfile + HasMotionSync> std::fmt::Debug for MotionSyncAccessor<'a, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MotionSyncAccessor")
            .finish_non_exhaustive()
    }
}

accessor!(
    /// On-screen menu display and navigation commands and inquiries.
    MenuAccessor,
    menu,
    HasMenuControl
);
accessor!(
    /// Streaming, vendor, and variable-speed controls.
    AdvancedAccessor,
    advanced
);

/// Direct motion safety and observation methods.
#[must_use]
pub struct MotionAccessor<'view> {
    core: &'view AsyncCameraCore,
}

impl std::fmt::Debug for MotionAccessor<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MotionAccessor")
            .finish_non_exhaustive()
    }
}

/// Expands [`motion_table!`] rows into the async motion methods.
macro_rules! async_motion_methods {
    (@motion { $($header:tt)* };
    $(
        $(#[$doc:meta])*
        fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $value:ty => $core:ident($($call:expr),*);
    )*) => {
        $(
            $(#[$doc])*
            pub async fn $name(&self $(, $arg: $ty)*) -> Result<$value> {
                self.core.$core($($call),*).await
            }
        )*
    };
}

impl MotionAccessor<'_> {
    motion_table!(async_motion_methods);
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the separate motion safety and observation view.
    pub fn motion(&self) -> MotionAccessor<'_> {
        MotionAccessor { core: self.core() }
    }
}

impl<'a, P: CompileTimeProfile> PowerAccessor<'a, P> {
    noun_table!(Power => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> ZoomAccessor<'a, P> {
    noun_table!(Zoom => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> SystemAccessor<'a, P> {
    noun_table!(System => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> PanTiltAccessor<'a, P> {
    noun_table!(PanTilt => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> FocusAccessor<'a, P> {
    noun_table!(Focus => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> PresetsAccessor<'a, P> {
    noun_table!(Presets => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> ExposureAccessor<'a, P> {
    noun_table!(Exposure => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> WhiteBalanceAccessor<'a, P> {
    noun_table!(WhiteBalance => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> ImageAccessor<'a, P> {
    noun_table!(Image => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> MenuAccessor<'a, P> {
    noun_table!(Menu => async_noun_methods);
}

impl<'a, P: CompileTimeProfile> AdvancedAccessor<'a, P> {
    noun_table!(Advanced => async_noun_methods);
}

impl<'a, P: CompileTimeProfile + HasTally> TallyAccessor<'a, P> {
    fn new(camera: &'a Camera<P>) -> Self {
        Self { camera }
    }

    noun_table!(Tally => async_noun_methods);
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the tally accessor for a profile that declares tally support.
    pub fn tally(&self) -> TallyAccessor<'_, P>
    where
        P: HasTally,
    {
        TallyAccessor::new(self)
    }
}

impl<'a, P: CompileTimeProfile + HasNdFilter> NdFilterAccessor<'a, P> {
    fn new(camera: &'a Camera<P>) -> Self {
        Self { camera }
    }

    noun_table!(NdFilter => async_noun_methods);
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the ND-filter accessor for a profile that declares ND support.
    pub fn nd_filter(&self) -> NdFilterAccessor<'_, P>
    where
        P: HasNdFilter,
    {
        NdFilterAccessor::new(self)
    }
}

impl<'a, P: CompileTimeProfile + HasMotionSync> MotionSyncAccessor<'a, P> {
    fn new(camera: &'a Camera<P>) -> Self {
        Self { camera }
    }

    noun_table!(MotionSync => async_noun_methods);
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the motion-sync accessor for a profile that declares support.
    pub fn motion_sync(&self) -> MotionSyncAccessor<'_, P>
    where
        P: HasMotionSync,
    {
        MotionSyncAccessor::new(self)
    }
}

#[cfg(test)]
mod inventory_tests {
    use crate::command::semantics::BuiltinCommand;
    use crate::command::surface::{surface_entry, StaticSurfaceDisposition};
    use crate::noun_parity::{consumed_nouns, noun_table_key, table_surface, without_test_modules};

    /// Issue #570's async ledger gate, adapted to the table-driven facade.
    ///
    /// The ledger method spellings no longer live in this file — they live in
    /// the one row table every facade consumes — so the property is now
    /// checked in two halves: this file must consume the shared table for the
    /// noun each ledger row belongs to, and the table must carry that row.
    /// Together those are the same statement the old needle count made: every
    /// ledger row has exactly one async definition.
    #[test]
    fn every_ledger_method_has_one_async_definition_per_row() {
        // Read the declaration region only: this very module names accessors
        // as string literals, so scanning the whole file would let the test
        // data satisfy the test.
        let source = without_test_modules(include_str!("async_nouns.rs"), "src/async_nouns.rs");
        let source = source.as_str();
        for accessor in [
            "PowerAccessor",
            "ZoomAccessor",
            "SystemAccessor",
            "PanTiltAccessor",
            "FocusAccessor",
            "ExposureAccessor",
            "WhiteBalanceAccessor",
            "ImageAccessor",
            "PresetsAccessor",
            "TallyAccessor",
            "NdFilterAccessor",
            "MotionSyncAccessor",
            "MenuAccessor",
            "AdvancedAccessor",
        ] {
            assert!(
                source.contains(&format!("{accessor}<'")),
                "missing canonical async noun accessor {accessor}",
            );
        }
        assert!(source.contains("pub struct MotionAccessor"));

        let consumed = consumed_nouns(source, "src/async_nouns.rs", "async_noun_methods");
        let table = table_surface();

        for command in BuiltinCommand::ALL {
            let StaticSurfaceDisposition::Noun { noun, method, .. } =
                surface_entry(*command).disposition
            else {
                continue;
            };
            let key = noun_table_key(noun);
            assert!(
                consumed.contains(key),
                "the async facade no longer generates the {key} noun from the shared table",
            );
            assert!(
                table[key].contains_key(method),
                "ledger row {key}::{method} has no row in the shared noun table",
            );
        }

        for forbidden in [
            "address_set",
            "interface_clear",
            "cancel_command",
            "pan_tilt_home",
            "zoom_stop",
            "defog_mode",
            "nr_speed",
        ] {
            let declaration = format!("pub async fn {forbidden}(");
            assert!(!source.contains(&declaration));
            assert!(!table
                .values()
                .any(|methods| methods.contains_key(forbidden)));
        }
    }
}
