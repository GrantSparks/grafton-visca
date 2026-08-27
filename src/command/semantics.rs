//! Authoritative semantic classification for built-in command requests.
//!
//! This module is deliberately a small, data-oriented ledger.  It is the
//! source that the typed-request conversion consumes; command bytes do not
//! carry semantic metadata. In particular,
//! adding a built-in command variant requires adding a row to
//! [`BuiltinCommand::classification`], so an unreviewed variant cannot acquire
//! an accidental plain/operation default.
//!
//! The classification rule from issue #542 is:
//!
//! * a request is an operation only when it starts, stops, or retargets
//!   physical actuation and exact VISCA cancellation has useful lifecycle
//!   meaning;
//! * [`BuiltinRequestClass::Targeted`] is reserved for a meaningful physical
//!   end state;
//! * [`BuiltinRequestClass::AppliedOnly`] is used when actuation has no
//!   meaningful settled target; and
//! * configuration, mode selection, and stored-state edits are plain.
//!
//! The first three movement axes mirror [`crate::AffectedAxes`].  Iris and ND
//! filter are included here because their commands physically reposition a
//! lens/filter and both have exact position inquiries in the built-in inquiry
//! inventory.  They are intentionally not silently mapped to an all-axis
//! movement query.  The later preparation phase must add matching profile
//! inquiry facts before exposing targeted iris/ND requests.
//!
//! # Why the ledger carries `#[allow(dead_code)]`
//!
//! Issue #636: this module is an authoritative *ledger*, and its only in-crate
//! consumer today is the closed noun/method surface in
//! [`crate::command::surface`], which is `#[cfg(test)]` — it, `crate::noun_parity`
//! and the `dynapi`/`blocking_nouns`/`async_nouns` audits are what force every
//! row to stay classified. Nothing in a non-test build reads a classification
//! yet; the production consumer is the typed-request lowering in
//! [`crate::prepared`], which grows it as the #542 preparation phases land.
//! Each item that is waiting for that consumer carries its own targeted
//! `#[allow(dead_code)]` rather than the module-wide blanket allow this file
//! used to open with. [`WriteOnlyState`] is deliberately not among them: the
//! engine's applied-state projection already consumes it.

/// A physical axis named by a built-in operation.
///
/// This is intentionally separate from the current movement-only
/// [`crate::AffectedAxes`] type.  It lets this phase record the complete
/// semantic contract before the later settlement implementation grows exact
/// iris and ND-filter inquiry plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum BuiltinAxis {
    /// Pan and tilt mechanism.
    PanTilt,
    /// Optical/digital zoom mechanism.
    Zoom,
    /// Focus mechanism.
    Focus,
    /// Iris/aperture mechanism.
    Iris,
    /// Neutral-density filter mechanism.
    NdFilter,
}

/// A validated, non-empty set of physical axes used by a built-in operation.
///
/// The only constructors are the non-empty constants and [`Self::union`], so
/// an operation row cannot carry an empty set.  `from_bits` is provided for
/// table and test tooling and rejects zero/unknown bits rather than using an
/// all-axis fallback.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub struct BuiltinAxes(u8);

// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
impl BuiltinAxes {
    const PAN_TILT_BIT: u8 = 1 << 0;
    const ZOOM_BIT: u8 = 1 << 1;
    const FOCUS_BIT: u8 = 1 << 2;
    const IRIS_BIT: u8 = 1 << 3;
    const ND_FILTER_BIT: u8 = 1 << 4;
    const VALID_BITS: u8 = Self::PAN_TILT_BIT
        | Self::ZOOM_BIT
        | Self::FOCUS_BIT
        | Self::IRIS_BIT
        | Self::ND_FILTER_BIT;

    /// Pan/tilt axis set.
    pub const PAN_TILT: Self = Self(Self::PAN_TILT_BIT);
    /// Zoom axis set.
    pub const ZOOM: Self = Self(Self::ZOOM_BIT);
    /// Focus axis set.
    pub const FOCUS: Self = Self(Self::FOCUS_BIT);
    /// Iris axis set.
    pub const IRIS: Self = Self(Self::IRIS_BIT);
    /// ND-filter axis set.
    pub const ND_FILTER: Self = Self(Self::ND_FILTER_BIT);

    /// Reconstruct a set from its stable bit representation.
    ///
    /// Returns `None` for an empty set or unknown bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits == 0 || bits & !Self::VALID_BITS != 0 {
            None
        } else {
            Some(Self(bits))
        }
    }

    /// Returns the stable bit representation.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Returns whether `other` is included in this set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Combines two already-valid, non-empty sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the number of individual axes in this set.
    #[must_use]
    pub const fn len(self) -> u8 {
        self.0.count_ones() as u8
    }

    /// Returns whether exactly one physical axis is selected.
    #[must_use]
    pub const fn is_single(self) -> bool {
        self.len() == 1
    }

    /// Returns an iterator over the individual axes.
    #[must_use]
    pub const fn iter(self) -> BuiltinAxisIter {
        BuiltinAxisIter { bits: self.0 }
    }
}

impl core::fmt::Debug for BuiltinAxes {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_set().entries(self.iter()).finish()
    }
}

/// Iterator returned by [`BuiltinAxes::iter`].
#[derive(Debug, Clone, Copy)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub struct BuiltinAxisIter {
    bits: u8,
}

impl Iterator for BuiltinAxisIter {
    type Item = BuiltinAxis;

    fn next(&mut self) -> Option<Self::Item> {
        let axis = if self.bits & BuiltinAxes::PAN_TILT_BIT != 0 {
            self.bits &= !BuiltinAxes::PAN_TILT_BIT;
            BuiltinAxis::PanTilt
        } else if self.bits & BuiltinAxes::ZOOM_BIT != 0 {
            self.bits &= !BuiltinAxes::ZOOM_BIT;
            BuiltinAxis::Zoom
        } else if self.bits & BuiltinAxes::FOCUS_BIT != 0 {
            self.bits &= !BuiltinAxes::FOCUS_BIT;
            BuiltinAxis::Focus
        } else if self.bits & BuiltinAxes::IRIS_BIT != 0 {
            self.bits &= !BuiltinAxes::IRIS_BIT;
            BuiltinAxis::Iris
        } else if self.bits & BuiltinAxes::ND_FILTER_BIT != 0 {
            self.bits &= !BuiltinAxes::ND_FILTER_BIT;
            BuiltinAxis::NdFilter
        } else {
            return None;
        };
        Some(axis)
    }
}

/// Exact axis selection for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum BuiltinAxisSelection {
    /// The command always affects this exact non-empty set.
    Exact(BuiltinAxes),
    /// Preset recall selects the exact set from validated profile facts.
    ///
    /// This is not an all-axis fallback: a profile must provide a concrete,
    /// non-empty set before a preset request can be prepared.
    ProfilePresetRecall,
}

// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
impl BuiltinAxisSelection {
    /// Returns whether this selection is known to be non-empty.
    #[must_use]
    pub const fn is_non_empty(self) -> bool {
        match self {
            Self::Exact(_) | Self::ProfilePresetRecall => true,
        }
    }

    /// Returns the exact fixed axes, if this is not profile-selected.
    #[must_use]
    pub const fn exact(self) -> Option<BuiltinAxes> {
        match self {
            Self::Exact(axes) => Some(axes),
            Self::ProfilePresetRecall => None,
        }
    }
}

/// Built-in command families/domains audited by this ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum BuiltinCommandDomain {
    /// Pan/tilt movement and limits.
    PanTilt,
    /// Zoom movement and digital-zoom mode.
    Zoom,
    /// Focus movement and focus configuration.
    Focus,
    /// Preset memory operations.
    Presets,
    /// Power state commands.
    Power,
    /// Exposure-mode and exposure-compensation settings.
    Exposure,
    /// Iris/aperture mechanism.
    Iris,
    /// Electronic shutter settings.
    Shutter,
    /// Brightness settings.
    Brightness,
    /// Gain settings.
    Gain,
    /// White-balance mode and sensitivity.
    WhiteBalance,
    /// Color-temperature and color-channel settings.
    Color,
    /// Image processing and orientation settings.
    Image,
    /// Neutral-density filter settings and mechanism.
    NdFilter,
    /// Tally-light settings.
    Tally,
    /// On-screen menu controls.
    Menu,
    /// Streaming and USB-audio controls.
    Streaming,
    /// Address, interface, cancellation, and settings persistence.
    System,
    /// Motion-sync configuration.
    MotionSync,
    /// Pan/tilt variable-speed mode configuration.
    VariableSpeed,
}

// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
impl BuiltinCommandDomain {
    /// Every audited built-in command domain.
    pub const ALL: &[Self] = &[
        Self::PanTilt,
        Self::Zoom,
        Self::Focus,
        Self::Presets,
        Self::Power,
        Self::Exposure,
        Self::Iris,
        Self::Shutter,
        Self::Brightness,
        Self::Gain,
        Self::WhiteBalance,
        Self::Color,
        Self::Image,
        Self::NdFilter,
        Self::Tally,
        Self::Menu,
        Self::Streaming,
        Self::System,
        Self::MotionSync,
        Self::VariableSpeed,
    ];

    /// Returns every audited built-in command domain.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        Self::ALL
    }
}

/// Private semantic spelling for the public [`crate::state_cache::StateKey`].
///
/// Keeping this alias in the ledger preserves the existing internal naming
/// while ensuring that the public cache key list and the applied-state ledger
/// have one source of truth.
pub(crate) use crate::state_cache::StateKey as WriteOnlyState;

/// Required cache projection for one write-only state command.
///
/// The value itself is carried by the typed request in a later phase.  This
/// closed requirement tells preparation whether that value can be written,
/// removed, or must be forgotten after exact application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum AppliedStateEffectRequirement {
    /// The request supplies a deterministic replacement value.
    Set(WriteOnlyState),
    /// The request removes one deterministic state entry (for example a
    /// cleared pan/tilt limit corner). A boolean `Off` command is `Set` with
    /// a false value, because false is still known after exact application.
    Clear(WriteOnlyState),
    /// The request changes state without exposing a deterministic value.
    Invalidate(WriteOnlyState),
}

// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
impl AppliedStateEffectRequirement {
    /// Returns the affected write-only state key.
    #[must_use]
    pub const fn state(self) -> WriteOnlyState {
        match self {
            Self::Set(state) | Self::Clear(state) | Self::Invalidate(state) => state,
        }
    }

    /// Returns the effect verb.
    #[must_use]
    pub const fn kind(self) -> AppliedStateEffectKind {
        match self {
            Self::Set(_) => AppliedStateEffectKind::Set,
            Self::Clear(_) => AppliedStateEffectKind::Clear,
            Self::Invalidate(_) => AppliedStateEffectKind::Invalidate,
        }
    }
}

/// The verb selected by [`AppliedStateEffectRequirement`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum AppliedStateEffectKind {
    /// Replace a known cached value.
    Set,
    /// Remove a known cached value.
    Clear,
    /// Mark a cached value unknown.
    Invalidate,
}

/// Semantic class of one built-in request.
///
/// Operation variants carry their exact non-empty axis selection directly in
/// the enum.  It is therefore impossible for a ledger row to represent an
/// operation without affected axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum BuiltinRequestClass {
    /// Configuration, mode, persistence, and stored-state edit.
    Plain {
        /// Closed cache requirement, when this is a write-only state edit.
        state_effect: Option<AppliedStateEffectRequirement>,
    },
    /// Physical actuation with a meaningful end state.
    Targeted {
        /// Exact fixed or profile-selected affected axes.
        axes: BuiltinAxisSelection,
    },
    /// Physical actuation whose terminal protocol application is the only
    /// meaningful lifecycle point.
    AppliedOnly {
        /// Exact fixed affected axes.
        axes: BuiltinAxisSelection,
    },
}

// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
impl BuiltinRequestClass {
    /// Returns whether this is either operation class.
    #[must_use]
    pub const fn is_operation(self) -> bool {
        !matches!(self, Self::Plain { .. })
    }

    /// Returns the completion class, if this is an operation.
    #[must_use]
    pub const fn completion(self) -> Option<BuiltinCompletionClass> {
        match self {
            Self::Plain { .. } => None,
            Self::Targeted { .. } => Some(BuiltinCompletionClass::Targeted),
            Self::AppliedOnly { .. } => Some(BuiltinCompletionClass::AppliedOnly),
        }
    }

    /// Returns the operation's exact axis selection, if any.
    #[must_use]
    pub const fn axes(self) -> Option<BuiltinAxisSelection> {
        match self {
            Self::Plain { .. } => None,
            Self::Targeted { axes } | Self::AppliedOnly { axes } => Some(axes),
        }
    }

    /// Returns the closed write-only state requirement, if any.
    #[must_use]
    pub const fn state_effect(self) -> Option<AppliedStateEffectRequirement> {
        match self {
            Self::Plain { state_effect } => state_effect,
            Self::Targeted { .. } | Self::AppliedOnly { .. } => None,
        }
    }
}

/// Completion class selected by a built-in operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum BuiltinCompletionClass {
    /// Physical motion has a meaningful terminal state.
    Targeted,
    /// Physical actuation is observed only through command application.
    AppliedOnly,
}

/// Every built-in command request that must be audited before typed
/// conversion.
///
/// The enum is intentionally flat: each protocol variant gets one reviewable
/// row.  `classification` is exhaustive, so adding a future variant without
/// a semantic decision is a compile error.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
pub enum BuiltinCommand {
    // Pan/tilt.
    PanTiltHome,
    PanTiltReset,
    PanTiltDrive,
    PanTiltStop,
    PanTiltAbsolute,
    PanTiltRelative,
    PanTiltLimitSet,
    PanTiltLimitClear,
    // Zoom.
    ZoomStop,
    ZoomTele,
    ZoomWide,
    ZoomTeleVariable,
    ZoomWideVariable,
    ZoomPosition,
    DigitalZoom,
    // Focus.
    FocusStop,
    FocusFar,
    FocusNear,
    FocusFarVariable,
    FocusNearVariable,
    FocusPosition,
    FocusAuto,
    FocusManual,
    FocusOnePush,
    FocusInfinity,
    FocusToggle,
    FocusSnap,
    FocusZone,
    FocusAutoSensitivity,
    FocusNearLimit,
    FocusLock,
    PushAfPress,
    PushAfRelease,
    // Presets.
    PresetRecall,
    PresetRecallSpeed,
    PresetSet,
    PresetReset,
    // Power.
    PowerOn,
    PowerStandby,
    // Exposure and iris.
    ExposureMode,
    ExposureCompensationOn,
    ExposureCompensationOff,
    ExposureCompensationReset,
    ExposureCompensationUp,
    ExposureCompensationDown,
    ExposureCompensationDirect,
    DynamicRange,
    IrisReset,
    IrisUp,
    IrisDown,
    IrisDirect,
    ShutterReset,
    ShutterUp,
    ShutterDown,
    ShutterDirect,
    BrightnessReset,
    BrightnessUp,
    BrightnessDown,
    BrightnessSet,
    BrightnessDirect,
    AntiFlicker,
    SpotlightOn,
    SpotlightOff,
    AutoSlowShutterOn,
    AutoSlowShutterOff,
    // Gain.
    GainReset,
    GainUp,
    GainDown,
    GainDirect,
    GainLimit,
    // White balance.
    WhiteBalanceAuto,
    WhiteBalanceIndoor,
    WhiteBalanceOutdoor,
    WhiteBalanceOnePush,
    WhiteBalanceAutoTracking,
    WhiteBalanceManual,
    WhiteBalanceColorTemperature,
    AutoWhiteBalanceSensitivity,
    OnePushWhiteBalanceTrigger,
    // Color.
    RedTuning,
    BlueTuning,
    Saturation,
    Hue,
    ColorTemperatureReset,
    ColorTemperatureUp,
    ColorTemperatureDown,
    ColorTemperatureDirect,
    RedGainReset,
    RedGainUp,
    RedGainDown,
    RedGainDirect,
    BlueGainReset,
    BlueGainUp,
    BlueGainDown,
    BlueGainDirect,
    // Image processing and orientation.
    SharpnessMode,
    SharpnessReset,
    SharpnessUp,
    SharpnessDown,
    SharpnessDirect,
    Luminance,
    Contrast,
    Gamma,
    Backlight,
    NoiseReduction2d,
    NoiseReduction3d,
    ImageFlipOff,
    ImageFlipHorizontal,
    ImageFlipHorizontalOff,
    ImageFlipVertical,
    ImageFlipBoth,
    ImageFlipCombined,
    ImageFreezeOn,
    ImageFreezeOff,
    PictureEffect,
    // Neutral-density filter.
    NdFilterMode,
    NdFilterDirect,
    NdFilterStepUp,
    NdFilterStepDown,
    NdFilterAutoOn,
    NdFilterAutoOff,
    // Tally.
    TallyRedOn,
    TallyRedOff,
    TallyBrightLow,
    TallyBrightHigh,
    TallyGreenOn,
    TallyGreenOff,
    TallyFlash,
    TallyOn,
    TallyOff,
    // Menus.
    MenuDisplay,
    MenuNavigate,
    MenuSelect,
    MenuCancel,
    DirectMenu,
    // Streaming.
    MulticastStreamingOn,
    MulticastStreamingOff,
    NdiQuality,
    UsbAudioOn,
    UsbAudioOff,
    // System.
    AddressSet,
    InterfaceClear,
    CommandCancel,
    SettingsSave,
    // Motion-related configuration.
    MotionSyncMode,
    MotionSyncPreset,
    VariableSpeedMode,
}

// Ledger row awaiting the `prepared` lowering; see the module note (#636).
#[allow(dead_code)]
impl BuiltinCommand {
    /// The complete closed command inventory.
    ///
    /// Keep this list in protocol/domain order.  The exhaustive match in
    /// [`Self::classification`] is the compiler-enforced part of the ledger;
    /// tests additionally assert that this list has no duplicate/missing rows.
    pub const ALL: &[Self] = &[
        Self::PanTiltHome,
        Self::PanTiltReset,
        Self::PanTiltDrive,
        Self::PanTiltStop,
        Self::PanTiltAbsolute,
        Self::PanTiltRelative,
        Self::PanTiltLimitSet,
        Self::PanTiltLimitClear,
        Self::ZoomStop,
        Self::ZoomTele,
        Self::ZoomWide,
        Self::ZoomTeleVariable,
        Self::ZoomWideVariable,
        Self::ZoomPosition,
        Self::DigitalZoom,
        Self::FocusStop,
        Self::FocusFar,
        Self::FocusNear,
        Self::FocusFarVariable,
        Self::FocusNearVariable,
        Self::FocusPosition,
        Self::FocusAuto,
        Self::FocusManual,
        Self::FocusOnePush,
        Self::FocusInfinity,
        Self::FocusToggle,
        Self::FocusSnap,
        Self::FocusZone,
        Self::FocusAutoSensitivity,
        Self::FocusNearLimit,
        Self::FocusLock,
        Self::PushAfPress,
        Self::PushAfRelease,
        Self::PresetRecall,
        Self::PresetRecallSpeed,
        Self::PresetSet,
        Self::PresetReset,
        Self::PowerOn,
        Self::PowerStandby,
        Self::ExposureMode,
        Self::ExposureCompensationOn,
        Self::ExposureCompensationOff,
        Self::ExposureCompensationReset,
        Self::ExposureCompensationUp,
        Self::ExposureCompensationDown,
        Self::ExposureCompensationDirect,
        Self::DynamicRange,
        Self::IrisReset,
        Self::IrisUp,
        Self::IrisDown,
        Self::IrisDirect,
        Self::ShutterReset,
        Self::ShutterUp,
        Self::ShutterDown,
        Self::ShutterDirect,
        Self::BrightnessReset,
        Self::BrightnessUp,
        Self::BrightnessDown,
        Self::BrightnessSet,
        Self::BrightnessDirect,
        Self::AntiFlicker,
        Self::SpotlightOn,
        Self::SpotlightOff,
        Self::AutoSlowShutterOn,
        Self::AutoSlowShutterOff,
        Self::GainReset,
        Self::GainUp,
        Self::GainDown,
        Self::GainDirect,
        Self::GainLimit,
        Self::WhiteBalanceAuto,
        Self::WhiteBalanceIndoor,
        Self::WhiteBalanceOutdoor,
        Self::WhiteBalanceOnePush,
        Self::WhiteBalanceAutoTracking,
        Self::WhiteBalanceManual,
        Self::WhiteBalanceColorTemperature,
        Self::AutoWhiteBalanceSensitivity,
        Self::OnePushWhiteBalanceTrigger,
        Self::RedTuning,
        Self::BlueTuning,
        Self::Saturation,
        Self::Hue,
        Self::ColorTemperatureReset,
        Self::ColorTemperatureUp,
        Self::ColorTemperatureDown,
        Self::ColorTemperatureDirect,
        Self::RedGainReset,
        Self::RedGainUp,
        Self::RedGainDown,
        Self::RedGainDirect,
        Self::BlueGainReset,
        Self::BlueGainUp,
        Self::BlueGainDown,
        Self::BlueGainDirect,
        Self::SharpnessMode,
        Self::SharpnessReset,
        Self::SharpnessUp,
        Self::SharpnessDown,
        Self::SharpnessDirect,
        Self::Luminance,
        Self::Contrast,
        Self::Gamma,
        Self::Backlight,
        Self::NoiseReduction2d,
        Self::NoiseReduction3d,
        Self::ImageFlipOff,
        Self::ImageFlipHorizontal,
        Self::ImageFlipHorizontalOff,
        Self::ImageFlipVertical,
        Self::ImageFlipBoth,
        Self::ImageFlipCombined,
        Self::ImageFreezeOn,
        Self::ImageFreezeOff,
        Self::PictureEffect,
        Self::NdFilterMode,
        Self::NdFilterDirect,
        Self::NdFilterStepUp,
        Self::NdFilterStepDown,
        Self::NdFilterAutoOn,
        Self::NdFilterAutoOff,
        Self::TallyRedOn,
        Self::TallyRedOff,
        Self::TallyBrightLow,
        Self::TallyBrightHigh,
        Self::TallyGreenOn,
        Self::TallyGreenOff,
        Self::TallyFlash,
        Self::TallyOn,
        Self::TallyOff,
        Self::MenuDisplay,
        Self::MenuNavigate,
        Self::MenuSelect,
        Self::MenuCancel,
        Self::DirectMenu,
        Self::MulticastStreamingOn,
        Self::MulticastStreamingOff,
        Self::NdiQuality,
        Self::UsbAudioOn,
        Self::UsbAudioOff,
        Self::AddressSet,
        Self::InterfaceClear,
        Self::CommandCancel,
        Self::SettingsSave,
        Self::MotionSyncMode,
        Self::MotionSyncPreset,
        Self::VariableSpeedMode,
    ];

    /// Returns every audited built-in command row.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        Self::ALL
    }

    /// Returns the command's audited domain.
    #[must_use]
    pub const fn domain(self) -> BuiltinCommandDomain {
        match self {
            Self::PanTiltHome
            | Self::PanTiltReset
            | Self::PanTiltDrive
            | Self::PanTiltStop
            | Self::PanTiltAbsolute
            | Self::PanTiltRelative
            | Self::PanTiltLimitSet
            | Self::PanTiltLimitClear => BuiltinCommandDomain::PanTilt,
            Self::ZoomStop
            | Self::ZoomTele
            | Self::ZoomWide
            | Self::ZoomTeleVariable
            | Self::ZoomWideVariable
            | Self::ZoomPosition
            | Self::DigitalZoom => BuiltinCommandDomain::Zoom,
            Self::FocusStop
            | Self::FocusFar
            | Self::FocusNear
            | Self::FocusFarVariable
            | Self::FocusNearVariable
            | Self::FocusPosition
            | Self::FocusAuto
            | Self::FocusManual
            | Self::FocusOnePush
            | Self::FocusInfinity
            | Self::FocusToggle
            | Self::FocusSnap
            | Self::FocusZone
            | Self::FocusAutoSensitivity
            | Self::FocusNearLimit
            | Self::FocusLock
            | Self::PushAfPress
            | Self::PushAfRelease => BuiltinCommandDomain::Focus,
            Self::PresetRecall | Self::PresetRecallSpeed | Self::PresetSet | Self::PresetReset => {
                BuiltinCommandDomain::Presets
            }
            Self::PowerOn | Self::PowerStandby => BuiltinCommandDomain::Power,
            Self::ExposureMode
            | Self::ExposureCompensationOn
            | Self::ExposureCompensationOff
            | Self::ExposureCompensationReset
            | Self::ExposureCompensationUp
            | Self::ExposureCompensationDown
            | Self::ExposureCompensationDirect
            | Self::DynamicRange
            | Self::AntiFlicker
            | Self::SpotlightOn
            | Self::SpotlightOff
            | Self::AutoSlowShutterOn
            | Self::AutoSlowShutterOff => BuiltinCommandDomain::Exposure,
            Self::IrisReset | Self::IrisUp | Self::IrisDown | Self::IrisDirect => {
                BuiltinCommandDomain::Iris
            }
            Self::ShutterReset | Self::ShutterUp | Self::ShutterDown | Self::ShutterDirect => {
                BuiltinCommandDomain::Shutter
            }
            Self::BrightnessReset
            | Self::BrightnessUp
            | Self::BrightnessDown
            | Self::BrightnessSet
            | Self::BrightnessDirect => BuiltinCommandDomain::Brightness,
            Self::GainReset
            | Self::GainUp
            | Self::GainDown
            | Self::GainDirect
            | Self::GainLimit => BuiltinCommandDomain::Gain,
            Self::WhiteBalanceAuto
            | Self::WhiteBalanceIndoor
            | Self::WhiteBalanceOutdoor
            | Self::WhiteBalanceOnePush
            | Self::WhiteBalanceAutoTracking
            | Self::WhiteBalanceManual
            | Self::WhiteBalanceColorTemperature
            | Self::AutoWhiteBalanceSensitivity
            | Self::OnePushWhiteBalanceTrigger => BuiltinCommandDomain::WhiteBalance,
            Self::RedTuning
            | Self::BlueTuning
            | Self::Saturation
            | Self::Hue
            | Self::ColorTemperatureReset
            | Self::ColorTemperatureUp
            | Self::ColorTemperatureDown
            | Self::ColorTemperatureDirect
            | Self::RedGainReset
            | Self::RedGainUp
            | Self::RedGainDown
            | Self::RedGainDirect
            | Self::BlueGainReset
            | Self::BlueGainUp
            | Self::BlueGainDown
            | Self::BlueGainDirect => BuiltinCommandDomain::Color,
            Self::SharpnessMode
            | Self::SharpnessReset
            | Self::SharpnessUp
            | Self::SharpnessDown
            | Self::SharpnessDirect
            | Self::Luminance
            | Self::Contrast
            | Self::Gamma
            | Self::Backlight
            | Self::NoiseReduction2d
            | Self::NoiseReduction3d
            | Self::ImageFlipOff
            | Self::ImageFlipHorizontal
            | Self::ImageFlipHorizontalOff
            | Self::ImageFlipVertical
            | Self::ImageFlipBoth
            | Self::ImageFlipCombined
            | Self::ImageFreezeOn
            | Self::ImageFreezeOff
            | Self::PictureEffect => BuiltinCommandDomain::Image,
            Self::NdFilterMode
            | Self::NdFilterDirect
            | Self::NdFilterStepUp
            | Self::NdFilterStepDown
            | Self::NdFilterAutoOn
            | Self::NdFilterAutoOff => BuiltinCommandDomain::NdFilter,
            Self::TallyRedOn
            | Self::TallyRedOff
            | Self::TallyBrightLow
            | Self::TallyBrightHigh
            | Self::TallyGreenOn
            | Self::TallyGreenOff
            | Self::TallyFlash
            | Self::TallyOn
            | Self::TallyOff => BuiltinCommandDomain::Tally,
            Self::MenuDisplay
            | Self::MenuNavigate
            | Self::MenuSelect
            | Self::MenuCancel
            | Self::DirectMenu => BuiltinCommandDomain::Menu,
            Self::MulticastStreamingOn
            | Self::MulticastStreamingOff
            | Self::NdiQuality
            | Self::UsbAudioOn
            | Self::UsbAudioOff => BuiltinCommandDomain::Streaming,
            Self::AddressSet | Self::InterfaceClear | Self::CommandCancel | Self::SettingsSave => {
                BuiltinCommandDomain::System
            }
            Self::MotionSyncMode | Self::MotionSyncPreset => BuiltinCommandDomain::MotionSync,
            Self::VariableSpeedMode => BuiltinCommandDomain::VariableSpeed,
        }
    }

    /// Returns the authoritative semantic classification for this command.
    ///
    /// This match is intentionally exhaustive.  Do not add a default arm:
    /// the compiler must force a semantic decision for every new variant.
    #[must_use]
    pub const fn classification(self) -> BuiltinRequestClass {
        use AppliedStateEffectRequirement::{Clear, Invalidate, Set};
        use BuiltinAxisSelection::{Exact, ProfilePresetRecall};
        use BuiltinRequestClass::{AppliedOnly, Plain, Targeted};
        use WriteOnlyState::{
            AutoNdFilter, AutoSlowShutter, DigitalZoomMode, Flip, FocusLockMode, ImageFreeze,
            MulticastStreaming, NdFilterMode, NdiQuality, PanTiltLimits, PresetRecallSpeed,
            Spotlight, TallyBrightness, TallyMode, VariableSpeedMode,
        };

        match self {
            Self::PanTiltHome
            | Self::PanTiltReset
            | Self::PanTiltAbsolute
            | Self::PanTiltRelative => Targeted {
                axes: Exact(BuiltinAxes::PAN_TILT),
            },
            Self::PanTiltDrive | Self::PanTiltStop => AppliedOnly {
                axes: Exact(BuiltinAxes::PAN_TILT),
            },
            Self::PanTiltLimitSet => Plain {
                state_effect: Some(Set(PanTiltLimits)),
            },
            Self::PanTiltLimitClear => Plain {
                state_effect: Some(Clear(PanTiltLimits)),
            },
            Self::ZoomPosition => Targeted {
                axes: Exact(BuiltinAxes::ZOOM),
            },
            Self::ZoomStop
            | Self::ZoomTele
            | Self::ZoomWide
            | Self::ZoomTeleVariable
            | Self::ZoomWideVariable => AppliedOnly {
                axes: Exact(BuiltinAxes::ZOOM),
            },
            Self::DigitalZoom => Plain {
                state_effect: Some(Set(DigitalZoomMode)),
            },
            Self::FocusPosition | Self::FocusInfinity => Targeted {
                axes: Exact(BuiltinAxes::FOCUS),
            },
            Self::FocusStop
            | Self::FocusFar
            | Self::FocusNear
            | Self::FocusFarVariable
            | Self::FocusNearVariable
            | Self::FocusOnePush
            | Self::FocusSnap
            | Self::PushAfPress
            | Self::PushAfRelease => AppliedOnly {
                axes: Exact(BuiltinAxes::FOCUS),
            },
            Self::FocusAuto
            | Self::FocusManual
            | Self::FocusToggle
            | Self::FocusZone
            | Self::FocusAutoSensitivity
            | Self::FocusNearLimit => Plain { state_effect: None },
            Self::FocusLock => Plain {
                state_effect: Some(Set(FocusLockMode)),
            },
            Self::PresetRecall => Targeted {
                axes: ProfilePresetRecall,
            },
            Self::PresetRecallSpeed => Plain {
                state_effect: Some(Set(PresetRecallSpeed)),
            },
            Self::PresetSet | Self::PresetReset => Plain { state_effect: None },
            Self::PowerOn | Self::PowerStandby => Plain { state_effect: None },
            Self::ExposureMode
            | Self::ExposureCompensationOn
            | Self::ExposureCompensationOff
            | Self::ExposureCompensationReset
            | Self::ExposureCompensationUp
            | Self::ExposureCompensationDown
            | Self::ExposureCompensationDirect
            | Self::DynamicRange
            | Self::ShutterReset
            | Self::ShutterUp
            | Self::ShutterDown
            | Self::ShutterDirect
            | Self::BrightnessReset
            | Self::BrightnessUp
            | Self::BrightnessDown
            | Self::BrightnessSet
            | Self::BrightnessDirect
            | Self::AntiFlicker
            | Self::GainReset
            | Self::GainUp
            | Self::GainDown
            | Self::GainDirect
            | Self::GainLimit => Plain { state_effect: None },
            // Iris commands are finite physical aperture movements.  They
            // have a meaningful end state and an exact Iris inquiry, so every
            // reset/step/direct form is targeted rather than applied-only.
            Self::IrisReset | Self::IrisUp | Self::IrisDown | Self::IrisDirect => Targeted {
                axes: Exact(BuiltinAxes::IRIS),
            },
            Self::SpotlightOn | Self::SpotlightOff => Plain {
                state_effect: Some(Set(Spotlight)),
            },
            Self::AutoSlowShutterOn | Self::AutoSlowShutterOff => Plain {
                state_effect: Some(Set(AutoSlowShutter)),
            },
            Self::WhiteBalanceAuto
            | Self::WhiteBalanceIndoor
            | Self::WhiteBalanceOutdoor
            | Self::WhiteBalanceOnePush
            | Self::WhiteBalanceAutoTracking
            | Self::WhiteBalanceManual
            | Self::WhiteBalanceColorTemperature
            | Self::AutoWhiteBalanceSensitivity
            | Self::OnePushWhiteBalanceTrigger
            | Self::RedTuning
            | Self::BlueTuning
            | Self::Saturation
            | Self::Hue
            | Self::ColorTemperatureReset
            | Self::ColorTemperatureUp
            | Self::ColorTemperatureDown
            | Self::ColorTemperatureDirect
            | Self::RedGainReset
            | Self::RedGainUp
            | Self::RedGainDown
            | Self::RedGainDirect
            | Self::BlueGainReset
            | Self::BlueGainUp
            | Self::BlueGainDown
            | Self::BlueGainDirect
            | Self::SharpnessMode
            | Self::SharpnessReset
            | Self::SharpnessUp
            | Self::SharpnessDown
            | Self::SharpnessDirect
            | Self::Luminance
            | Self::Contrast
            | Self::Gamma
            | Self::Backlight
            | Self::NoiseReduction2d
            | Self::NoiseReduction3d
            | Self::PictureEffect
            | Self::MenuDisplay
            | Self::MenuNavigate
            | Self::MenuSelect
            | Self::MenuCancel
            | Self::DirectMenu
            | Self::MotionSyncMode
            | Self::MotionSyncPreset => Plain { state_effect: None },
            // The combined-flip opcode carries both axes in one parameter, so
            // it establishes a complete horizontal/vertical pair.
            Self::ImageFlipBoth | Self::ImageFlipCombined => Plain {
                state_effect: Some(Set(Flip)),
            },
            // The single-axis flip and mirror opcodes move one axis and say
            // nothing about the other, so a complete pair stops being known.
            Self::ImageFlipOff
            | Self::ImageFlipHorizontal
            | Self::ImageFlipHorizontalOff
            | Self::ImageFlipVertical => Plain {
                state_effect: Some(Invalidate(Flip)),
            },
            // ND direct and step commands physically reposition the filter.
            // The direct target and each finite step have meaningful end
            // states and are therefore targeted on the ND axis.
            Self::NdFilterDirect | Self::NdFilterStepUp | Self::NdFilterStepDown => Targeted {
                axes: Exact(BuiltinAxes::ND_FILTER),
            },
            Self::NdFilterMode => Plain {
                state_effect: Some(Set(NdFilterMode)),
            },
            Self::NdFilterAutoOn | Self::NdFilterAutoOff => Plain {
                state_effect: Some(Set(AutoNdFilter)),
            },
            Self::ImageFreezeOn => Plain {
                state_effect: Some(Set(ImageFreeze)),
            },
            Self::ImageFreezeOff => Plain {
                // The typed request supplies `false`; this is a known value,
                // not removal of the cache entry.
                state_effect: Some(Set(ImageFreeze)),
            },
            Self::MulticastStreamingOn => Plain {
                state_effect: Some(Set(MulticastStreaming)),
            },
            Self::MulticastStreamingOff => Plain {
                // The typed request supplies `false`; this is a known value,
                // not removal of the cache entry.
                state_effect: Some(Set(MulticastStreaming)),
            },
            Self::NdiQuality => Plain {
                state_effect: Some(Set(NdiQuality)),
            },
            Self::UsbAudioOn | Self::UsbAudioOff => Plain { state_effect: None },
            Self::VariableSpeedMode => Plain {
                state_effect: Some(Set(VariableSpeedMode)),
            },
            // Red/green tally status has profile-gated inquiries. Brightness
            // and vendor output mode do not, so their applied effects are
            // either deterministic or explicitly invalidated.
            Self::TallyRedOn | Self::TallyRedOff | Self::TallyGreenOn | Self::TallyGreenOff => {
                Plain { state_effect: None }
            }
            Self::TallyBrightLow | Self::TallyBrightHigh => Plain {
                state_effect: Some(Set(TallyBrightness)),
            },
            Self::TallyFlash => Plain {
                state_effect: Some(Invalidate(TallyMode)),
            },
            Self::TallyOn | Self::TallyOff => Plain {
                state_effect: Some(Set(TallyMode)),
            },
            Self::AddressSet | Self::InterfaceClear | Self::CommandCancel | Self::SettingsSave => {
                Plain { state_effect: None }
            }
        }
    }

    /// Returns whether this command is a plain request.
    #[must_use]
    pub const fn is_plain(self) -> bool {
        matches!(self.classification(), BuiltinRequestClass::Plain { .. })
    }

    /// Returns the exact operation axes, if this command is an operation.
    #[must_use]
    pub const fn affected_axes(self) -> Option<BuiltinAxisSelection> {
        self.classification().axes()
    }

    /// Returns the operation completion class, if this command is an
    /// operation.
    #[must_use]
    pub const fn completion(self) -> Option<BuiltinCompletionClass> {
        self.classification().completion()
    }

    /// Returns the write-only cache effect requirement, if any.
    #[must_use]
    pub const fn state_effect(self) -> Option<AppliedStateEffectRequirement> {
        self.classification().state_effect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    #[test]
    fn ledger_is_closed_and_every_row_has_a_classification() {
        let mut seen = HashSet::new();
        let mut domains = HashSet::new();
        for command in BuiltinCommand::all() {
            assert!(seen.insert(*command), "duplicate ledger row: {command:?}");
            domains.insert(command.domain());
            let class = command.classification();
            if class.is_operation() {
                assert!(
                    command.completion().is_some(),
                    "operation row has no completion class: {command:?}"
                );
                let axes = class.axes().expect("operation must declare axes");
                assert!(axes.is_non_empty());
                if let BuiltinAxisSelection::Exact(axes) = axes {
                    assert_ne!(axes.bits(), 0);
                }
            } else {
                assert!(class.axes().is_none());
            }
        }
        assert_eq!(seen.len(), BuiltinCommand::all().len());
        assert_eq!(domains.len(), BuiltinCommandDomain::all().len());
        for domain in BuiltinCommandDomain::all() {
            assert!(
                domains.contains(domain),
                "domain missing from ledger: {domain:?}"
            );
        }
    }

    #[test]
    fn operation_rows_use_only_exact_axes_or_profile_selected_preset_axes() {
        for command in BuiltinCommand::ALL {
            match command.classification() {
                BuiltinRequestClass::Targeted { axes }
                | BuiltinRequestClass::AppliedOnly { axes } => {
                    assert!(axes.is_non_empty());
                    if let BuiltinAxisSelection::Exact(axes) = axes {
                        assert!(axes.is_single() || axes.len() > 1);
                    }
                }
                BuiltinRequestClass::Plain { .. } => {}
            }
        }
        assert_eq!(
            BuiltinCommand::PresetRecall.affected_axes(),
            Some(BuiltinAxisSelection::ProfilePresetRecall)
        );
    }

    #[test]
    fn physical_actuation_audit_decisions_are_explicit() {
        for command in [
            BuiltinCommand::IrisReset,
            BuiltinCommand::IrisUp,
            BuiltinCommand::IrisDown,
            BuiltinCommand::IrisDirect,
        ] {
            assert_eq!(command.completion(), Some(BuiltinCompletionClass::Targeted));
            assert_eq!(
                command.affected_axes(),
                Some(BuiltinAxisSelection::Exact(BuiltinAxes::IRIS))
            );
        }
        for command in [
            BuiltinCommand::NdFilterDirect,
            BuiltinCommand::NdFilterStepUp,
            BuiltinCommand::NdFilterStepDown,
        ] {
            assert_eq!(command.completion(), Some(BuiltinCompletionClass::Targeted));
            assert_eq!(
                command.affected_axes(),
                Some(BuiltinAxisSelection::Exact(BuiltinAxes::ND_FILTER))
            );
        }
        for command in [BuiltinCommand::PushAfPress, BuiltinCommand::PushAfRelease] {
            assert_eq!(
                command.completion(),
                Some(BuiltinCompletionClass::AppliedOnly)
            );
            assert_eq!(
                command.affected_axes(),
                Some(BuiltinAxisSelection::Exact(BuiltinAxes::FOCUS))
            );
        }
    }

    #[test]
    fn write_only_state_effects_are_closed() {
        assert_eq!(
            BuiltinCommand::PanTiltLimitSet.state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::PanTiltLimits
            ))
        );
        assert_eq!(
            BuiltinCommand::PanTiltLimitClear.state_effect(),
            Some(AppliedStateEffectRequirement::Clear(
                WriteOnlyState::PanTiltLimits
            ))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOn.state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::ImageFreeze
            ))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOff.state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::ImageFreeze
            ))
        );
        assert_eq!(
            BuiltinCommand::TallyFlash.state_effect(),
            Some(AppliedStateEffectRequirement::Invalidate(
                WriteOnlyState::TallyMode
            ))
        );
        assert_eq!(
            BuiltinCommand::DigitalZoom.state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::DigitalZoomMode
            ))
        );

        let represented: HashSet<_> = BuiltinCommand::ALL
            .iter()
            .filter_map(|command| command.state_effect().map(|effect| effect.state()))
            .collect();
        let listed: HashSet<_> = WriteOnlyState::ALL.iter().copied().collect();
        assert_eq!(listed.len(), WriteOnlyState::ALL.len());
        for state in WriteOnlyState::ALL {
            assert!(
                represented.contains(state),
                "write-only state has no ledger effect: {state:?}"
            );
        }
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .state_effect()
                .is_some_and(|effect| effect.kind() == AppliedStateEffectKind::Set)
        }));
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .state_effect()
                .is_some_and(|effect| effect.kind() == AppliedStateEffectKind::Clear)
        }));
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .state_effect()
                .is_some_and(|effect| effect.kind() == AppliedStateEffectKind::Invalidate)
        }));
    }
}
