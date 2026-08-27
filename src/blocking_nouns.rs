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
//! to the blocking surface — the accessor types, the synchronous `fn` shape,
//! and the session-lifetime plumbing that `Operation<'session, _>` needs.

use std::fmt;

use crate::{
    camera::{IdleWait, MotionQuery},
    capabilities::{
        HasAutoFocusSensitivity, HasAutoTrackingWhiteBalance, HasAutoWhiteBalanceSensitivity,
        HasBacklightCompensation, HasBrightnessControl, HasColorTemperature, HasCombinedImageFlip,
        HasContrastControl, HasDigitalZoomRange, HasDigitalZoomToggle, HasDirectMenuControl,
        HasDirectZoom, HasExposure, HasExposureCompensation, HasFocus, HasFocusLock,
        HasFocusNearLimitInquiry, HasFocusZone, HasGammaControl, HasHueControl, HasImageFlip,
        HasImageMirror, HasImageProcessing, HasIrisControl, HasLuminanceControl, HasMenuControl,
        HasMotionSync, HasNdFilter, HasNoiseReduction, HasNoiseReduction2D, HasNoiseReduction3D,
        HasOnePushFocus, HasOnePushWhiteBalance, HasPanTilt, HasPictureEffect, HasPower,
        HasPresets, HasPtzOpticsSnapFocus, HasPushAutoFocus, HasRgbGain, HasRgbTuning,
        HasSaturationControl, HasSharpnessControl, HasTally, HasVariableSpeed, HasWhiteBalance,
        HasWideDynamicRange, HasZoom,
    },
    command,
    completion::{self, AppliedOnly, Targeted},
    noun_table::noun_table,
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    CompileTimeProfile, Inquiry, OperationCommand, PlainCommand, Result, ZoomDomain,
};

use super::{Camera, Operation};

macro_rules! accessor_method {
    ($(#[$meta:meta])* $name:ident, $method:ident) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, 'session, P> {
            $name::new(self)
        }
    };
    ($(#[$meta:meta])* $name:ident, $method:ident, $bound:path) => {
        $(#[$meta])*
        pub fn $method(&self) -> $name<'_, 'session, P>
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
        pub struct $name<'view, 'session, P: CompileTimeProfile> {
            camera: &'view Camera<'session, P>,
        }

        impl<P: CompileTimeProfile> fmt::Debug for $name<'_, '_, P> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }

        impl<'view, 'session, P: CompileTimeProfile> $name<'view, 'session, P> {
            fn new(camera: &'view Camera<'session, P>) -> Self {
                Self { camera }
            }
        }

        impl<'session, P: CompileTimeProfile> Camera<'session, P> {
            accessor_method!($(#[$meta])* $name, $method $(, $bound)?);
        }
    };
}

/// Generates one blocking noun method per [`noun_table`] row.
///
/// The row grammar is documented on [`crate::noun_table`].  This consumer
/// carries everything the blocking surface adds to a row: a synchronous
/// `pub fn`, `Operation<'session, Kind>` handles, and the free `execute` /
/// `inquire` / `submit` hops onto the owner core.
macro_rules! blocking_noun_methods {
    () => {};

    (
        $(#[$doc:meta])*
        inquiry $method:ident() -> $response:ty $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self) -> Result<$response>
        $(where P: $gate $(+ $extra)*)?
        {
            inquire(self.camera, &$request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        plain $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = checked $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<()>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = $request?;
            execute(self.camera, &request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        plain $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = with_profile |$profile:ident| $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<()>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = {
                let $profile = self.camera.profile();
                $request?
            };
            execute(self.camera, &request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        plain $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<()>
        $(where P: $gate $(+ $extra)*)?
        {
            execute(self.camera, &$request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = delegate $target:ident($($delegated:expr),*);
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<'session, AppliedOnly>>
        $(where P: $gate $(+ $extra)*)?
        {
            self.$target($($delegated),*)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = checked $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<'session, AppliedOnly>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = $request?;
            submit(self.camera, &request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = with_core |$core:ident| $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<'session, AppliedOnly>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = {
                let $core = self.camera.core();
                $request?
            };
            submit(self.camera, &request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        applied $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<'session, AppliedOnly>>
        $(where P: $gate $(+ $extra)*)?
        {
            submit(self.camera, &$request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        targeted $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = checked $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<'session, Targeted>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = $request?;
            submit(self.camera, &request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        targeted $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = with_profile |$profile:ident| $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<'session, Targeted>>
        $(where P: $gate $(+ $extra)*)?
        {
            let request = {
                let $profile = self.camera.profile();
                $request?
            };
            submit(self.camera, &request)
        }
        blocking_noun_methods!($($rest)*);
    };

    (
        $(#[$doc:meta])*
        targeted $method:ident($($arg:ident: $ty:ty),*) $(where $gate:tt $(+ $extra:tt)*)?
            = $request:expr;
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        pub fn $method(&self, $($arg: $ty),*) -> Result<Operation<'session, Targeted>>
        $(where P: $gate $(+ $extra)*)?
        {
            submit(self.camera, &$request)
        }
        blocking_noun_methods!($($rest)*);
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
pub struct TallyAccessor<'view, 'session, P: CompileTimeProfile + HasTally> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile + HasTally> fmt::Debug for TallyAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TallyAccessor")
            .finish_non_exhaustive()
    }
}

/// ND-filter controls for a profile which declares ND support.
#[must_use]
pub struct NdFilterAccessor<'view, 'session, P: CompileTimeProfile + HasNdFilter> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile + HasNdFilter> fmt::Debug for NdFilterAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NdFilterAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion-sync controls for a profile which declares motion-sync support.
#[must_use]
pub struct MotionSyncAccessor<'view, 'session, P: CompileTimeProfile + HasMotionSync> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile + HasMotionSync> fmt::Debug for MotionSyncAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MotionSyncAccessor")
            .finish_non_exhaustive()
    }
}

/// Motion safety and observation methods kept separate from the pan/tilt noun.
#[must_use]
pub struct MotionAccessor<'view, 'session, P: CompileTimeProfile> {
    camera: &'view Camera<'session, P>,
}

impl<P: CompileTimeProfile> fmt::Debug for MotionAccessor<'_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MotionAccessor")
            .finish_non_exhaustive()
    }
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns the separate motion safety and observation view.
    pub fn motion(&self) -> MotionAccessor<'_, 'session, P> {
        MotionAccessor { camera: self }
    }
}

impl<'view, 'session, P: CompileTimeProfile> MotionAccessor<'view, 'session, P> {
    /// Stops pan/tilt, zoom, and focus through the camera's single owner.
    pub fn stop_all_motion(&self) -> Result<()> {
        self.camera.core().stop_all_motion()
    }

    /// Reports whether any mechanical movement axis is moving.
    ///
    /// This samples [`AffectedAxes::MOVEMENT`] with the default tolerance; use
    /// [`Self::is_moving_axes`] to pick the axes or the tolerance.
    ///
    /// [`AffectedAxes::MOVEMENT`]: crate::AffectedAxes::MOVEMENT
    pub fn is_moving(&self) -> Result<bool> {
        self.camera.core().is_moving(MotionQuery::default())
    }

    /// Reports whether the selected physical axes are moving.
    pub fn is_moving_axes(&self, query: MotionQuery) -> Result<bool> {
        self.camera.core().is_moving(query)
    }

    /// Waits for the selected physical axes to become idle.
    pub fn wait_until_idle(&self, wait: IdleWait) -> Result<()> {
        self.camera.core().wait_until_idle(wait)
    }
}

fn execute<'session, P, C>(camera: &Camera<'session, P>, command: &C) -> Result<()>
where
    P: CompileTimeProfile,
    C: PlainCommand + ?Sized,
{
    camera.core().execute(command)
}

fn inquire<'session, P, Q>(camera: &Camera<'session, P>, inquiry: &Q) -> Result<Q::Response>
where
    P: CompileTimeProfile,
    Q: Inquiry + ?Sized,
{
    camera.core().inquire(inquiry)
}

fn submit<'session, P, K, O>(
    camera: &Camera<'session, P>,
    operation: &O,
) -> Result<Operation<'session, K>>
where
    P: CompileTimeProfile,
    K: completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    camera.core().submit(operation)
}

impl<'view, 'session, P: CompileTimeProfile> PowerAccessor<'view, 'session, P> {
    noun_table!(Power => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> ZoomAccessor<'view, 'session, P> {
    noun_table!(Zoom => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> SystemAccessor<'view, 'session, P> {
    noun_table!(System => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> PanTiltAccessor<'view, 'session, P> {
    noun_table!(PanTilt => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> FocusAccessor<'view, 'session, P> {
    noun_table!(Focus => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> PresetsAccessor<'view, 'session, P> {
    noun_table!(Presets => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> ExposureAccessor<'view, 'session, P> {
    noun_table!(Exposure => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> WhiteBalanceAccessor<'view, 'session, P> {
    noun_table!(WhiteBalance => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> ImageAccessor<'view, 'session, P> {
    noun_table!(Image => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> MenuAccessor<'view, 'session, P> {
    noun_table!(Menu => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile> AdvancedAccessor<'view, 'session, P> {
    noun_table!(Advanced => blocking_noun_methods);
}

impl<'view, 'session, P: CompileTimeProfile + HasTally> TallyAccessor<'view, 'session, P> {
    fn new(camera: &'view Camera<'session, P>) -> Self {
        Self { camera }
    }

    noun_table!(Tally => blocking_noun_methods);
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns tally controls for a profile that declares tally support.
    pub fn tally(&self) -> TallyAccessor<'_, 'session, P>
    where
        P: HasTally,
    {
        TallyAccessor::new(self)
    }
}

impl<'view, 'session, P: CompileTimeProfile + HasNdFilter> NdFilterAccessor<'view, 'session, P> {
    fn new(camera: &'view Camera<'session, P>) -> Self {
        Self { camera }
    }

    noun_table!(NdFilter => blocking_noun_methods);
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns ND-filter controls for a profile that declares ND support.
    pub fn nd_filter(&self) -> NdFilterAccessor<'_, 'session, P>
    where
        P: HasNdFilter,
    {
        NdFilterAccessor::new(self)
    }
}

impl<'view, 'session, P: CompileTimeProfile + HasMotionSync>
    MotionSyncAccessor<'view, 'session, P>
{
    fn new(camera: &'view Camera<'session, P>) -> Self {
        Self { camera }
    }

    noun_table!(MotionSync => blocking_noun_methods);
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    /// Returns motion-sync controls for a profile that declares support.
    pub fn motion_sync(&self) -> MotionSyncAccessor<'_, 'session, P>
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
