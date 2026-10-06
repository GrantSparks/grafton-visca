//! Canonical static noun views for the blocking owner-backed camera.
//!
//! The accessors in this module are deliberately small borrowed views.  They
//! do not own a transport, create a runtime, or introduce another command
//! path: every operation goes through [`super::BlockingCameraCore`].  The
//! method spelling follows the closed ledger in `command::surface` and the
//! return class is visible in the operation handle (`AppliedOnly` or
//! `Targeted`).
//!
//! The methods themselves are not written here: they are generated from the
//! shared row table in [`crate::noun_table`], which the async and erased
//! facades consume from the same rows.  This module owns only what is specific
//! to the blocking surface — the accessor types and the synchronous `fn`
//! shape.

use std::fmt;

use crate::{
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
    completion::{self, AppliedOnly, Targeted},
    noun_facade::noun_request,
    noun_table::{motion_table, noun_table},
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    CompileTimeProfile, Inquiry, OperationCommand, PlainCommand, Result, ZoomDomain,
};

use super::{BlockingCameraCore, Camera, Operation};

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
        pub struct $name<'view, P: CompileTimeProfile> {
            camera: &'view Camera<P>,
        }

        impl<P: CompileTimeProfile> fmt::Debug for $name<'_, P> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }

        impl<'view, P: CompileTimeProfile> $name<'view, P> {
            fn new(camera: &'view Camera<P>) -> Self {
                Self { camera }
            }
        }

        impl<P: CompileTimeProfile> Camera<P> {
            accessor_method!($(#[$meta])* $name, $method $(, $bound)?);
        }
    };
}

/// Generates one blocking noun method per [`noun_table`] row.
///
/// The row grammar is documented on [`crate::noun_table`]; the request form is
/// parsed only by [`noun_request!`]. This consumer carries everything the
/// blocking surface adds to a row: a synchronous `pub fn`, `Operation<Kind>`
/// handles, and the free `execute` / `inquire` / `submit` hops onto the owner
/// core.
macro_rules! blocking_noun_methods {
    (@noun $noun:ident { $($header:tt)* };
        $(
            $(#[$doc:meta])*
            $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
        )*
    ) => {
        $(
            blocking_noun_methods!(@method [$(#[$doc])*] $kind $method($($arg: $ty),*) -> $ret;
                [$($gate $(+ $extra)*)?]; [$($request)*]);
        )*
    };

    (@method [$(#[$doc:meta])*] inquiry $method:ident() -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub fn $method(&self) -> Result<$ret>
        $(where P: $gate $(+ $extra)*)?
        {
            inquire(self.camera, &$($request)*)
        }
    };

    (@method [$(#[$doc:meta])*] plain $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<()>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = noun_request!(self.camera.profile(); $ret; $($request)*)?;
            execute(self.camera, &request)
        }
    };

    (@method [$(#[$doc:meta])*] applied $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<AppliedOnly>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = noun_request!(self.camera.profile(); $ret; $($request)*)?;
            submit(self.camera, &request)
        }
    };

    (@method [$(#[$doc:meta])*] targeted $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;
        [$($gate:ident $(+ $extra:ident)*)?]; [$($request:tt)*]) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<Targeted>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = noun_request!(self.camera.profile(); $ret; $($request)*)?;
            submit(self.camera, &request)
        }
    };
}

accessor!(
    /// Power controls and the power-state inquiry.
    PowerAccessor,
    power,
    HasPower
);
accessor!(
    /// Optical and digital zoom controls.
    ZoomAccessor,
    zoom,
    HasZoom
);
accessor!(
    /// Firmware/version and persistence controls.
    SystemAccessor,
    system
);
accessor!(
    /// Pan/tilt movement, limits, and position inquiry.
    PanTiltAccessor,
    pan_tilt,
    HasPanTilt
);
accessor!(
    /// Focus movement, modes, and typed focus inquiries.
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
    /// Image-processing, noise-reduction, and orientation controls.
    ImageAccessor,
    image,
    HasImageProcessing
);
accessor!(
    /// Preset commands and recall-speed control.
    PresetsAccessor,
    presets,
    HasPresets
);
accessor!(
    /// Menu display and navigation controls.
    MenuAccessor,
    menu,
    HasMenuControl
);
accessor!(
    /// Streaming, vendor, and variable-speed controls.
    AdvancedAccessor,
    advanced
);

/// Tally controls for a profile which declares tally support.
#[must_use]
pub struct TallyAccessor<'view, P: CompileTimeProfile + HasTally> {
    camera: &'view Camera<P>,
}

impl<P: CompileTimeProfile + HasTally> fmt::Debug for TallyAccessor<'_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TallyAccessor")
            .finish_non_exhaustive()
    }
}

/// ND-filter controls for a profile which declares ND support.
#[must_use]
pub struct NdFilterAccessor<'view, P: CompileTimeProfile + HasNdFilter> {
    camera: &'view Camera<P>,
}

impl<P: CompileTimeProfile + HasNdFilter> fmt::Debug for NdFilterAccessor<'_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NdFilterAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion-sync controls for a profile which declares motion-sync support.
#[must_use]
pub struct MotionSyncAccessor<'view, P: CompileTimeProfile + HasMotionSync> {
    camera: &'view Camera<P>,
}

impl<P: CompileTimeProfile + HasMotionSync> fmt::Debug for MotionSyncAccessor<'_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MotionSyncAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion safety and observation methods kept separate from the pan/tilt noun.
///
/// The typed [`Camera`] and, with `dyn-api`, the runtime-profile
/// `BlockingDynSessionCamera` both return this one view, so every blocking
/// camera exposes the same motion surface.
#[must_use]
pub struct MotionAccessor<'view> {
    core: &'view BlockingCameraCore,
}

impl fmt::Debug for MotionAccessor<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MotionAccessor")
            .finish_non_exhaustive()
    }
}

impl<'view> MotionAccessor<'view> {
    pub(crate) const fn new(core: &'view BlockingCameraCore) -> Self {
        Self { core }
    }
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns the separate motion safety and observation view.
    pub fn motion(&self) -> MotionAccessor<'_> {
        MotionAccessor::new(self.core())
    }
}

/// Expands [`motion_table!`] rows into the synchronous motion methods.
macro_rules! blocking_motion_methods {
    (@motion { $($header:tt)* };
    $(
        $(#[$doc:meta])*
        fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $value:ty => $core:ident($($call:expr),*);
    )*) => {
        $(
            $(#[$doc])*
            pub fn $name(&self $(, $arg: $ty)*) -> Result<$value> {
                self.core.$core($($call),*)
            }
        )*
    };
}

impl MotionAccessor<'_> {
    motion_table!(blocking_motion_methods);
}

fn execute<P, C>(camera: &Camera<P>, command: &C) -> Result<()>
where
    P: CompileTimeProfile,
    C: PlainCommand + ?Sized,
{
    camera.core().execute(command)
}

fn inquire<P, Q>(camera: &Camera<P>, inquiry: &Q) -> Result<Q::Response>
where
    P: CompileTimeProfile,
    Q: Inquiry + ?Sized,
{
    camera.core().inquire(inquiry)
}

fn submit<P, K, O>(camera: &Camera<P>, operation: &O) -> Result<Operation<K>>
where
    P: CompileTimeProfile,
    K: completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    camera.core().submit(operation)
}

impl<'view, P: CompileTimeProfile> PowerAccessor<'view, P> {
    noun_table!(Power => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> ZoomAccessor<'view, P> {
    noun_table!(Zoom => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> SystemAccessor<'view, P> {
    noun_table!(System => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> PanTiltAccessor<'view, P> {
    noun_table!(PanTilt => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> FocusAccessor<'view, P> {
    noun_table!(Focus => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> PresetsAccessor<'view, P> {
    noun_table!(Presets => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> ExposureAccessor<'view, P> {
    noun_table!(Exposure => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> WhiteBalanceAccessor<'view, P> {
    noun_table!(WhiteBalance => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> ImageAccessor<'view, P> {
    noun_table!(Image => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> MenuAccessor<'view, P> {
    noun_table!(Menu => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile> AdvancedAccessor<'view, P> {
    noun_table!(Advanced => blocking_noun_methods);
}

impl<'view, P: CompileTimeProfile + HasTally> TallyAccessor<'view, P> {
    fn new(camera: &'view Camera<P>) -> Self {
        Self { camera }
    }

    noun_table!(Tally => blocking_noun_methods);
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns tally controls for a profile that declares tally support.
    pub fn tally(&self) -> TallyAccessor<'_, P>
    where
        P: HasTally,
    {
        TallyAccessor::new(self)
    }
}

impl<'view, P: CompileTimeProfile + HasNdFilter> NdFilterAccessor<'view, P> {
    fn new(camera: &'view Camera<P>) -> Self {
        Self { camera }
    }

    noun_table!(NdFilter => blocking_noun_methods);
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns ND-filter controls for a profile that declares ND support.
    pub fn nd_filter(&self) -> NdFilterAccessor<'_, P>
    where
        P: HasNdFilter,
    {
        NdFilterAccessor::new(self)
    }
}

impl<'view, P: CompileTimeProfile + HasMotionSync> MotionSyncAccessor<'view, P> {
    fn new(camera: &'view Camera<P>) -> Self {
        Self { camera }
    }

    noun_table!(MotionSync => blocking_noun_methods);
}

impl<P: CompileTimeProfile> Camera<P> {
    /// Returns motion-sync controls for a profile that declares support.
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

    /// Issue #570's blocking ledger gate, adapted to the table-driven facade.
    ///
    /// See the async twin in `src/async_nouns.rs` for why the property is now
    /// checked as "this file consumes the shared table for the noun" plus "the
    /// shared table carries the row".
    #[test]
    fn every_ledger_method_has_one_blocking_definition_per_row() {
        // Read the declaration region only: this very module names accessors
        // as string literals, so scanning the whole file would let the test
        // data satisfy the test.
        let source =
            without_test_modules(include_str!("blocking_nouns.rs"), "src/blocking_nouns.rs");
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
                "missing canonical blocking noun accessor {accessor}",
            );
        }
        assert!(source.contains("pub struct MotionAccessor"));

        let consumed = consumed_nouns(source, "src/blocking_nouns.rs", "blocking_noun_methods");
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
                "the blocking facade no longer generates the {key} noun from the shared table",
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
            let declaration = format!("pub fn {forbidden}(");
            assert!(!source.contains(&declaration));
            assert!(!table
                .values()
                .any(|methods| methods.contains_key(forbidden)));
        }
    }
}
