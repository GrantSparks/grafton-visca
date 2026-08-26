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

#![allow(dead_code)]

/// A physical axis named by a built-in operation.
///
/// This is intentionally separate from the current movement-only
/// [`crate::AffectedAxes`] type.  It lets this phase record the complete
/// semantic contract before the later settlement implementation grows exact
/// iris and ND-filter inquiry plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
pub struct BuiltinAxes(u8);

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
pub enum BuiltinAxisSelection {
    /// The command always affects this exact non-empty set.
    Exact(BuiltinAxes),
    /// Preset recall selects the exact set from validated profile facts.
    ///
    /// This is not an all-axis fallback: a profile must provide a concrete,
    /// non-empty set before a preset request can be prepared.
    ProfilePresetRecall,
}

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
            AutoNdFilter, AutoSlowShutter, DigitalZoomMode, FocusLockMode, ImageFreeze,
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
            | Self::ImageFlipOff
            | Self::ImageFlipHorizontal
            | Self::ImageFlipVertical
            | Self::ImageFlipBoth
            | Self::ImageFlipCombined
            | Self::PictureEffect
            | Self::MenuDisplay
            | Self::MenuNavigate
            | Self::MenuSelect
            | Self::MenuCancel
            | Self::DirectMenu
            | Self::MotionSyncMode
            | Self::MotionSyncPreset => Plain { state_effect: None },
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

/// Returns the semantic row for one pan/tilt direction.
#[must_use]
pub(crate) const fn map_pan_tilt_direction(
    direction: crate::command::PanTiltDirection,
) -> BuiltinCommand {
    use crate::command::PanTiltDirection;
    match direction {
        PanTiltDirection::Stop => BuiltinCommand::PanTiltStop,
        PanTiltDirection::Up
        | PanTiltDirection::Down
        | PanTiltDirection::Left
        | PanTiltDirection::Right
        | PanTiltDirection::UpLeft
        | PanTiltDirection::UpRight
        | PanTiltDirection::DownLeft
        | PanTiltDirection::DownRight => BuiltinCommand::PanTiltDrive,
    }
}

/// Returns the semantic row for one pan/tilt command value.
#[must_use]
pub(crate) fn map_pan_tilt(command: &crate::command::PanTilt) -> BuiltinCommand {
    use crate::command::PanTilt;
    match command {
        PanTilt::Home => BuiltinCommand::PanTiltHome,
        PanTilt::Reset => BuiltinCommand::PanTiltReset,
        PanTilt::Move { direction, .. } => map_pan_tilt_direction(*direction),
        PanTilt::AbsolutePosition { .. } | PanTilt::AbsolutePositionRaw { .. } => {
            BuiltinCommand::PanTiltAbsolute
        }
        PanTilt::RelativePosition { .. } | PanTilt::RelativePositionRaw { .. } => {
            BuiltinCommand::PanTiltRelative
        }
        PanTilt::LimitSet { .. } | PanTilt::LimitSetRaw { .. } => BuiltinCommand::PanTiltLimitSet,
        PanTilt::LimitClear { .. } => BuiltinCommand::PanTiltLimitClear,
    }
}

/// Returns the semantic row for a pan/tilt limit corner operation.
#[must_use]
pub(crate) const fn map_pan_tilt_limit_corner(
    _corner: crate::command::PanTiltLimitCorner,
    clear: bool,
) -> BuiltinCommand {
    if clear {
        BuiltinCommand::PanTiltLimitClear
    } else {
        BuiltinCommand::PanTiltLimitSet
    }
}

/// Returns the semantic row for one zoom command value.
#[must_use]
pub(crate) fn map_zoom(command: &crate::command::Zoom) -> BuiltinCommand {
    match command {
        crate::command::Zoom::Stop => BuiltinCommand::ZoomStop,
        crate::command::Zoom::TeleStd => BuiltinCommand::ZoomTele,
        crate::command::Zoom::WideStd => BuiltinCommand::ZoomWide,
        crate::command::Zoom::TeleVariable(_) => BuiltinCommand::ZoomTeleVariable,
        crate::command::Zoom::WideVariable(_) => BuiltinCommand::ZoomWideVariable,
        crate::command::Zoom::Position(_) => BuiltinCommand::ZoomPosition,
    }
}

/// Returns the semantic row for one focus command value.
#[must_use]
pub(crate) fn map_focus(command: &crate::command::Focus) -> BuiltinCommand {
    match command {
        crate::command::Focus::Stop => BuiltinCommand::FocusStop,
        crate::command::Focus::Far => BuiltinCommand::FocusFar,
        crate::command::Focus::Near => BuiltinCommand::FocusNear,
        crate::command::Focus::FarWithSpeed(_) => BuiltinCommand::FocusFarVariable,
        crate::command::Focus::NearWithSpeed(_) => BuiltinCommand::FocusNearVariable,
        crate::command::Focus::Position(_) => BuiltinCommand::FocusPosition,
        crate::command::Focus::Auto => BuiltinCommand::FocusAuto,
        crate::command::Focus::Manual => BuiltinCommand::FocusManual,
        crate::command::Focus::OnePushTrigger => BuiltinCommand::FocusOnePush,
        crate::command::Focus::Infinity => BuiltinCommand::FocusInfinity,
        crate::command::Focus::Toggle => BuiltinCommand::FocusToggle,
        crate::command::Focus::Snap => BuiltinCommand::FocusSnap,
    }
}

/// Returns the semantic row for one exposure-compensation value.
#[must_use]
pub(crate) fn map_exposure_compensation(
    command: &crate::command::ExposureCompensation,
) -> BuiltinCommand {
    match command {
        crate::command::ExposureCompensation::On => BuiltinCommand::ExposureCompensationOn,
        crate::command::ExposureCompensation::Off => BuiltinCommand::ExposureCompensationOff,
        crate::command::ExposureCompensation::Reset => BuiltinCommand::ExposureCompensationReset,
        crate::command::ExposureCompensation::Up => BuiltinCommand::ExposureCompensationUp,
        crate::command::ExposureCompensation::Down => BuiltinCommand::ExposureCompensationDown,
        crate::command::ExposureCompensation::SetLevel(_) => {
            BuiltinCommand::ExposureCompensationDirect
        }
    }
}

/// Returns the semantic row for one exposure mode.
#[must_use]
pub(crate) const fn map_exposure_mode(mode: crate::command::ExposureMode) -> BuiltinCommand {
    match mode {
        crate::command::ExposureMode::Auto
        | crate::command::ExposureMode::Manual
        | crate::command::ExposureMode::Shutter
        | crate::command::ExposureMode::Iris
        | crate::command::ExposureMode::Bright => BuiltinCommand::ExposureMode,
    }
}

/// Returns the semantic row for one anti-flicker mode.
#[must_use]
pub(crate) const fn map_anti_flicker_mode(mode: crate::command::AntiFlickerMode) -> BuiltinCommand {
    match mode {
        crate::command::AntiFlickerMode::Off
        | crate::command::AntiFlickerMode::Hz50
        | crate::command::AntiFlickerMode::Hz60 => BuiltinCommand::AntiFlicker,
    }
}

/// Returns the semantic row for one iris command value.
#[must_use]
pub(crate) fn map_iris(command: &crate::command::Iris) -> BuiltinCommand {
    match command {
        crate::command::Iris::Reset => BuiltinCommand::IrisReset,
        crate::command::Iris::Up => BuiltinCommand::IrisUp,
        crate::command::Iris::Down => BuiltinCommand::IrisDown,
        crate::command::Iris::SetAperture(_) => BuiltinCommand::IrisDirect,
    }
}

/// Returns the semantic row for one shutter command value.
#[must_use]
pub(crate) fn map_shutter(command: &crate::command::Shutter) -> BuiltinCommand {
    match command {
        crate::command::Shutter::Reset => BuiltinCommand::ShutterReset,
        crate::command::Shutter::Up => BuiltinCommand::ShutterUp,
        crate::command::Shutter::Down => BuiltinCommand::ShutterDown,
        crate::command::Shutter::SetSpeed(_) => BuiltinCommand::ShutterDirect,
    }
}

/// Returns the semantic row for one brightness command value.
#[must_use]
pub(crate) fn map_brightness(command: &crate::command::Brightness) -> BuiltinCommand {
    match command {
        crate::command::Brightness::Reset => BuiltinCommand::BrightnessReset,
        crate::command::Brightness::Up => BuiltinCommand::BrightnessUp,
        crate::command::Brightness::Down => BuiltinCommand::BrightnessDown,
        crate::command::Brightness::SetLevel(_) => BuiltinCommand::BrightnessSet,
        crate::command::Brightness::Direct(_) => BuiltinCommand::BrightnessDirect,
    }
}

/// Returns the semantic row for one gain command value.
#[must_use]
pub(crate) fn map_gain(command: &crate::command::Gain) -> BuiltinCommand {
    match command {
        crate::command::Gain::Reset => BuiltinCommand::GainReset,
        crate::command::Gain::Up => BuiltinCommand::GainUp,
        crate::command::Gain::Down => BuiltinCommand::GainDown,
        crate::command::Gain::SetValue(_) => BuiltinCommand::GainDirect,
    }
}

/// Returns the semantic row for one sharpness command value.
#[must_use]
pub(crate) fn map_sharpness(command: &crate::command::Sharpness) -> BuiltinCommand {
    match command {
        crate::command::Sharpness::Mode(_) => BuiltinCommand::SharpnessMode,
        crate::command::Sharpness::Reset => BuiltinCommand::SharpnessReset,
        crate::command::Sharpness::Up => BuiltinCommand::SharpnessUp,
        crate::command::Sharpness::Down => BuiltinCommand::SharpnessDown,
        crate::command::Sharpness::SetLevel { .. } => BuiltinCommand::SharpnessDirect,
    }
}

/// Returns the semantic row for one sharpness mode.
#[must_use]
pub(crate) const fn map_sharpness_mode(mode: crate::command::SharpnessMode) -> BuiltinCommand {
    match mode {
        crate::command::SharpnessMode::Auto | crate::command::SharpnessMode::Manual => {
            BuiltinCommand::SharpnessMode
        }
    }
}

/// Returns the semantic row for one color-temperature command value.
#[must_use]
pub(crate) fn map_color_temperature(command: &crate::command::ColorTemperature) -> BuiltinCommand {
    match command {
        crate::command::ColorTemperature::Reset => BuiltinCommand::ColorTemperatureReset,
        crate::command::ColorTemperature::Up => BuiltinCommand::ColorTemperatureUp,
        crate::command::ColorTemperature::Down => BuiltinCommand::ColorTemperatureDown,
        crate::command::ColorTemperature::SetTemperature(_) => {
            BuiltinCommand::ColorTemperatureDirect
        }
    }
}

/// Returns the semantic row for one red-gain command value.
#[must_use]
pub(crate) fn map_red_gain(command: &crate::command::RedGain) -> BuiltinCommand {
    match command {
        crate::command::RedGain::Reset => BuiltinCommand::RedGainReset,
        crate::command::RedGain::Up => BuiltinCommand::RedGainUp,
        crate::command::RedGain::Down => BuiltinCommand::RedGainDown,
        crate::command::RedGain::SetValue(_) => BuiltinCommand::RedGainDirect,
    }
}

/// Returns the semantic row for one blue-gain command value.
#[must_use]
pub(crate) fn map_blue_gain(command: &crate::command::BlueGain) -> BuiltinCommand {
    match command {
        crate::command::BlueGain::Reset => BuiltinCommand::BlueGainReset,
        crate::command::BlueGain::Up => BuiltinCommand::BlueGainUp,
        crate::command::BlueGain::Down => BuiltinCommand::BlueGainDown,
        crate::command::BlueGain::SetValue(_) => BuiltinCommand::BlueGainDirect,
    }
}

/// Returns the semantic row for one simple flip mode.
#[must_use]
pub(crate) const fn map_flip(mode: crate::command::Flip) -> BuiltinCommand {
    match mode {
        crate::command::Flip::On => BuiltinCommand::ImageFlipVertical,
        crate::command::Flip::Off => BuiltinCommand::ImageFlipOff,
    }
}

/// Returns the semantic row for one combined image-flip mode.
#[must_use]
pub(crate) const fn map_image_flip_mode(mode: crate::command::ImageFlipMode) -> BuiltinCommand {
    match mode {
        crate::command::ImageFlipMode::Off => BuiltinCommand::ImageFlipOff,
        crate::command::ImageFlipMode::Horizontal => BuiltinCommand::ImageFlipHorizontal,
        crate::command::ImageFlipMode::Vertical => BuiltinCommand::ImageFlipVertical,
        crate::command::ImageFlipMode::Both => BuiltinCommand::ImageFlipBoth,
    }
}

/// Returns the semantic row for one picture-effect mode.
#[must_use]
pub(crate) const fn map_picture_effect_mode(
    mode: crate::command::PictureEffectMode,
) -> BuiltinCommand {
    match mode {
        crate::command::PictureEffectMode::Off
        | crate::command::PictureEffectMode::Negative
        | crate::command::PictureEffectMode::BlackAndWhite
        | crate::command::PictureEffectMode::Sepia
        | crate::command::PictureEffectMode::Sketch
        | crate::command::PictureEffectMode::Emboss
        | crate::command::PictureEffectMode::Mosaic
        | crate::command::PictureEffectMode::Unknown(_) => BuiltinCommand::PictureEffect,
    }
}

/// Returns the semantic row for one menu direction.
#[must_use]
pub(crate) const fn map_menu_direction(direction: crate::command::MenuDirection) -> BuiltinCommand {
    match direction {
        crate::command::MenuDirection::Up
        | crate::command::MenuDirection::Down
        | crate::command::MenuDirection::Left
        | crate::command::MenuDirection::Right => BuiltinCommand::MenuNavigate,
    }
}
/// Returns the semantic row for one menu action.
#[must_use]
pub(crate) const fn map_menu_action(action: crate::command::MenuAction) -> BuiltinCommand {
    match action {
        crate::command::MenuAction::Select => BuiltinCommand::MenuSelect,
        crate::command::MenuAction::Cancel => BuiltinCommand::MenuCancel,
    }
}

/// Returns the semantic row for one ND-filter mode.
#[must_use]
pub(crate) const fn map_nd_filter_mode(mode: crate::command::NdFilterMode) -> BuiltinCommand {
    match mode {
        crate::command::NdFilterMode::Preset | crate::command::NdFilterMode::Variable => {
            BuiltinCommand::NdFilterMode
        }
    }
}

/// Returns the semantic row for one ND-filter step.
#[must_use]
pub(crate) const fn map_nd_filter_step(step: crate::command::NdFilterStep) -> BuiltinCommand {
    match step {
        crate::command::NdFilterStep::Up => BuiltinCommand::NdFilterStepUp,
        crate::command::NdFilterStep::Down => BuiltinCommand::NdFilterStepDown,
    }
}

/// Returns the semantic row for one preset action.
#[must_use]
pub(crate) const fn map_preset_action(action: crate::command::PresetAction) -> BuiltinCommand {
    match action {
        crate::command::PresetAction::Reset => BuiltinCommand::PresetReset,
        crate::command::PresetAction::Set => BuiltinCommand::PresetSet,
        crate::command::PresetAction::Recall => BuiltinCommand::PresetRecall,
    }
}

/// Returns the semantic row for one white-balance mode.
#[must_use]
pub(crate) const fn map_white_balance_mode(
    mode: crate::command::WhiteBalanceMode,
) -> BuiltinCommand {
    match mode {
        crate::command::WhiteBalanceMode::Auto => BuiltinCommand::WhiteBalanceAuto,
        crate::command::WhiteBalanceMode::Indoor => BuiltinCommand::WhiteBalanceIndoor,
        crate::command::WhiteBalanceMode::Outdoor => BuiltinCommand::WhiteBalanceOutdoor,
        crate::command::WhiteBalanceMode::OnePush => BuiltinCommand::WhiteBalanceOnePush,
        crate::command::WhiteBalanceMode::ATW => BuiltinCommand::WhiteBalanceAutoTracking,
        crate::command::WhiteBalanceMode::Manual => BuiltinCommand::WhiteBalanceManual,
        crate::command::WhiteBalanceMode::ColorTemperature => {
            BuiltinCommand::WhiteBalanceColorTemperature
        }
    }
}

/// Returns the semantic row for one AWB-sensitivity mode.
#[must_use]
pub(crate) const fn map_awb_sensitivity(
    sensitivity: crate::command::AutoWhiteBalanceSensitivity,
) -> BuiltinCommand {
    match sensitivity {
        crate::command::AutoWhiteBalanceSensitivity::High
        | crate::command::AutoWhiteBalanceSensitivity::Normal
        | crate::command::AutoWhiteBalanceSensitivity::Low => {
            BuiltinCommand::AutoWhiteBalanceSensitivity
        }
    }
}

/// Returns the semantic row for one multicast mode.
#[must_use]
pub(crate) const fn map_multicast_streaming(
    mode: crate::command::MulticastStreaming,
) -> BuiltinCommand {
    match mode {
        crate::command::MulticastStreaming::On => BuiltinCommand::MulticastStreamingOn,
        crate::command::MulticastStreaming::Off => BuiltinCommand::MulticastStreamingOff,
    }
}

/// Returns the semantic row for one USB-audio mode.
#[must_use]
pub(crate) const fn map_usb_audio(mode: crate::command::UsbAudio) -> BuiltinCommand {
    match mode {
        crate::command::UsbAudio::On => BuiltinCommand::UsbAudioOn,
        crate::command::UsbAudio::Off => BuiltinCommand::UsbAudioOff,
    }
}

/// Returns the semantic row for one NDI quality value.
#[must_use]
pub(crate) const fn map_ndi_quality(quality: crate::types::NdiQuality) -> BuiltinCommand {
    match quality {
        crate::types::NdiQuality::High
        | crate::types::NdiQuality::Medium
        | crate::types::NdiQuality::Low
        | crate::types::NdiQuality::Off => BuiltinCommand::NdiQuality,
    }
}

/// Returns the semantic row for one motion-sync mode.
#[must_use]
pub(crate) const fn map_motion_sync_mode(mode: crate::command::MotionSyncMode) -> BuiltinCommand {
    match mode {
        crate::command::MotionSyncMode::On | crate::command::MotionSyncMode::Off => {
            BuiltinCommand::MotionSyncMode
        }
    }
}

/// Returns the semantic row for one motion-sync preset value.
#[must_use]
pub(crate) const fn map_motion_sync_preset(
    preset: crate::command::MotionSyncPreset,
) -> BuiltinCommand {
    match preset {
        crate::command::MotionSyncPreset::Slow
        | crate::command::MotionSyncPreset::Normal
        | crate::command::MotionSyncPreset::Fast => BuiltinCommand::MotionSyncPreset,
    }
}

/// Returns the semantic row for one variable-speed mode.
#[must_use]
pub(crate) const fn map_variable_speed_mode(
    mode: crate::command::VariableSpeedMode,
) -> BuiltinCommand {
    match mode {
        crate::command::VariableSpeedMode::Standard24
        | crate::command::VariableSpeedMode::Fine50 => BuiltinCommand::VariableSpeedMode,
    }
}

/// Returns the semantic row for one focus-zone value.
#[must_use]
pub(crate) const fn map_focus_zone(zone: crate::command::FocusZone) -> BuiltinCommand {
    match zone {
        crate::command::FocusZone::Top
        | crate::command::FocusZone::Center
        | crate::command::FocusZone::Bottom => BuiltinCommand::FocusZone,
    }
}

/// Returns the semantic row for one auto-focus sensitivity value.
#[must_use]
pub(crate) const fn map_focus_sensitivity(
    sensitivity: crate::command::AutoFocusSensitivity,
) -> BuiltinCommand {
    match sensitivity {
        crate::command::AutoFocusSensitivity::Low
        | crate::command::AutoFocusSensitivity::Normal
        | crate::command::AutoFocusSensitivity::High => BuiltinCommand::FocusAutoSensitivity,
    }
}

/// Returns the semantic row for one focus-lock value.
#[must_use]
pub(crate) const fn map_focus_lock(lock: crate::command::FocusLock) -> BuiltinCommand {
    match lock {
        crate::command::FocusLock::On | crate::command::FocusLock::Off => BuiltinCommand::FocusLock,
    }
}

/// Returns the semantic row for one Push-AF value.
#[must_use]
pub(crate) const fn map_push_af(push: crate::command::PushAF) -> BuiltinCommand {
    match push {
        crate::command::PushAF::Press => BuiltinCommand::PushAfPress,
        crate::command::PushAF::Release => BuiltinCommand::PushAfRelease,
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
