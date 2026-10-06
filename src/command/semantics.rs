//! Authoritative semantic classification for built-in command requests.
//!
//! This module is deliberately a small, data-oriented ledger.  It is the
//! source that the typed-request conversion consumes; command bytes do not
//! carry semantic metadata.  The `builtin_command_ledger!` list below is
//! the one place a built-in command is named: [`BuiltinCommand`], its
//! [`BuiltinCommand::ALL`] inventory and its
//! [`BuiltinCommand::classification`] are all expanded from that list, so an
//! unreviewed variant cannot exist and no second row list can drift.
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
//! Operation axes are [`crate::AffectedAxes`]. Iris and ND filter are
//! included because their commands physically reposition a lens/filter and
//! both have exact position inquiries in the built-in inquiry inventory. They
//! are intentionally not silently mapped to an all-axis movement query.
//!
//! The production typed-request lowering consumes this ledger through
//! [`BuiltinRequestContract`]: every built-in request type names one ledger
//! row, its declared class is checked against that row at compile time, and
//! its fixed operation axes and write-only state effect are read from it.
//! The ledger is therefore load-bearing in ordinary builds, not only in the
//! independent semantic tests.

use crate::{AffectedAxes, StateKey};

/// Exact axis selection for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinAxisSelection {
    /// The command always affects this exact non-empty set.
    Exact(AffectedAxes),
    /// Preset recall selects the exact set from validated profile facts.
    ///
    /// This is not an all-axis fallback: a profile must provide a concrete,
    /// non-empty set before a preset request can be prepared.
    ProfilePresetRecall,
}

impl BuiltinAxisSelection {
    /// Returns whether two selections are the same exact axes or both
    /// profile-selected.
    const fn same_selection(self, other: Self) -> bool {
        match (self, other) {
            (Self::Exact(left), Self::Exact(right)) => left.bits() == right.bits(),
            (Self::ProfilePresetRecall, Self::ProfilePresetRecall) => true,
            (Self::Exact(_), Self::ProfilePresetRecall)
            | (Self::ProfilePresetRecall, Self::Exact(_)) => false,
        }
    }
}

/// Required cache projection for one write-only state command.
///
/// The value itself is carried by the typed request. This closed requirement
/// tells preparation whether that value can be written,
/// removed, or must be forgotten after exact application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppliedStateEffectRequirement {
    /// The request supplies a deterministic replacement value.
    Set(StateKey),
    /// The request removes one deterministic state entry (for example a
    /// cleared pan/tilt limit corner). A boolean `Off` command is `Set` with
    /// a false value, because false is still known after exact application.
    Clear(StateKey),
    /// The request changes state without exposing a deterministic value.
    Invalidate(StateKey),
}

impl AppliedStateEffectRequirement {
    /// Returns whether two effects have the same verb and state key.
    const fn same_effect(self, other: Self) -> bool {
        match (self, other) {
            (Self::Set(left), Self::Set(right))
            | (Self::Clear(left), Self::Clear(right))
            | (Self::Invalidate(left), Self::Invalidate(right)) => left as u8 == right as u8,
            _ => false,
        }
    }

    /// Returns the affected write-only state key.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn state(self) -> StateKey {
        match self {
            Self::Set(state) | Self::Clear(state) | Self::Invalidate(state) => state,
        }
    }
}

/// Semantic class of one built-in request.
///
/// Operation variants carry their exact non-empty axis selection directly in
/// the enum.  It is therefore impossible for a ledger row to represent an
/// operation without affected axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinRequestClass {
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
    /// Returns the completion class, if this is an operation.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn completion(self) -> Option<BuiltinCompletionClass> {
        match self {
            Self::Plain { .. } => None,
            Self::Targeted { .. } => Some(BuiltinCompletionClass::Targeted),
            Self::AppliedOnly { .. } => Some(BuiltinCompletionClass::AppliedOnly),
        }
    }

    /// Returns the operation's exact axis selection, if any.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn axes(self) -> Option<BuiltinAxisSelection> {
        match self {
            Self::Plain { .. } => None,
            Self::Targeted { axes } | Self::AppliedOnly { axes } => Some(axes),
        }
    }

    /// Returns the request kind this class requires.
    #[must_use]
    pub(crate) const fn kind(self) -> BuiltinRequestKind {
        match self {
            Self::Plain { .. } => BuiltinRequestKind::Plain,
            Self::Targeted { .. } => BuiltinRequestKind::Targeted,
            Self::AppliedOnly { .. } => BuiltinRequestKind::AppliedOnly,
        }
    }

    /// Returns whether two classes impose the same request contract: the
    /// same class, the same axis selection and the same state effect.
    #[must_use]
    pub(crate) const fn same_contract(self, other: Self) -> bool {
        match (self, other) {
            (
                Self::Plain { state_effect: left },
                Self::Plain {
                    state_effect: right,
                },
            ) => match (left, right) {
                (None, None) => true,
                (Some(left), Some(right)) => left.same_effect(right),
                (None, Some(_)) | (Some(_), None) => false,
            },
            (Self::Targeted { axes: left }, Self::Targeted { axes: right })
            | (Self::AppliedOnly { axes: left }, Self::AppliedOnly { axes: right }) => {
                left.same_selection(right)
            }
            _ => false,
        }
    }

    /// Returns the closed write-only state requirement, if any.
    #[must_use]
    pub(crate) const fn state_effect(self) -> Option<AppliedStateEffectRequirement> {
        match self {
            Self::Plain { state_effect } => state_effect,
            Self::Targeted { .. } | Self::AppliedOnly { .. } => None,
        }
    }
}

/// Completion class selected by a built-in operation.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinCompletionClass {
    /// Physical motion has a meaningful terminal state.
    Targeted,
    /// Physical actuation is observed only through command application.
    AppliedOnly,
}

/// The request kind of one ledger row: the closed `Request::Class` a typed
/// request serving it must declare, and the kind a noun-table row spells as
/// `plain`, `applied` or `targeted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinRequestKind {
    /// `request::Plain`.
    Plain,
    /// `request::Operation<completion::AppliedOnly>`.
    AppliedOnly,
    /// `request::Operation<completion::Targeted>`.
    Targeted,
}

/// The ledger kind of a closed request class marker.
pub(crate) trait BuiltinClassMarker {
    /// Kind a built-in request with this class must serve.
    const KIND: BuiltinRequestKind;
}

impl BuiltinClassMarker for crate::request::Plain {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::Plain;
}

impl BuiltinClassMarker for crate::request::Operation<crate::completion::Targeted> {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::Targeted;
}

impl BuiltinClassMarker for crate::request::Operation<crate::completion::AppliedOnly> {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::AppliedOnly;
}

/// A built-in request type's tie to the ledger.
///
/// Everything else in the type's contract (the class check, fixed operation
/// axes, the write-only state effect and the typed capability gate) is read
/// from [`Self::LEDGER_ROW`].
pub(crate) trait BuiltinRequestContract: crate::Request {
    /// A ledger row this type serves. Every ledger row served by the type
    /// must classify identically and, unless [`Self::GATE_BY_VALUE`], share
    /// its typed capability gate; the typed-request inventory checks both at
    /// compile time.
    const LEDGER_ROW: BuiltinCommand;
    /// The type's rows carry different typed capability gates, so its
    /// validator names the row for each value.
    const GATE_BY_VALUE: bool = false;
}

/// The exact fixed axes of `T`'s ledger row.
///
/// # Panics
///
/// Fails const evaluation when the row is plain or selects its axes from the
/// profile.
#[allow(clippy::panic)]
pub(crate) const fn fixed_axes<T: BuiltinRequestContract>() -> AffectedAxes {
    match T::LEDGER_ROW.classification() {
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::Exact(axes),
        }
        | BuiltinRequestClass::AppliedOnly {
            axes: BuiltinAxisSelection::Exact(axes),
        } => axes,
        _ => panic!("ledger row has no fixed operation axes"),
    }
}

/// The write-only state effect of `T`'s ledger row.
///
/// # Panics
///
/// Fails const evaluation when the row has no state effect.
#[allow(clippy::panic)]
pub(crate) const fn state_effect<T: BuiltinRequestContract>() -> AppliedStateEffectRequirement {
    match T::LEDGER_ROW.classification().state_effect() {
        Some(effect) => effect,
        None => panic!("ledger row has no write-only state effect"),
    }
}

/// Fails const evaluation when `T`'s declared request class disagrees with
/// its ledger row.
pub(crate) const fn assert_request_contract<T>()
where
    T: BuiltinRequestContract,
    T::Class: BuiltinClassMarker,
{
    assert!(
        T::LEDGER_ROW.classification().kind() as u8 == <T::Class as BuiltinClassMarker>::KIND as u8,
        "request class disagrees with its ledger row",
    );
}

/// The closed semantic ledger: one row per built-in command, in protocol
/// order.
///
/// `plain`, `plain(Set(<key>))`, `plain(Clear(<key>))`,
/// `plain(Invalidate(<key>))`, `targeted(<axes>)`,
/// `targeted(profile_preset_recall)` and `applied(<axes>)` are the only row
/// forms; `<key>` names a [`StateKey`] and `<axes>` an
/// [`AffectedAxes`] constant. [`BuiltinCommand`], [`BuiltinCommand::ALL`] and
/// [`BuiltinCommand::classification`] are all expanded from this one list, so
/// a row cannot be declared without a classification, listed twice, or left
/// out of the inventory.
macro_rules! builtin_command_ledger {
    ($( $(#[$meta:meta])* $command:ident => $class:ident $(($($effect:tt)*))? ),+ $(,)?) => {
        /// Every built-in command request that must be audited before typed
        /// conversion; generated from `builtin_command_ledger!`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum BuiltinCommand {
            $( $(#[$meta])* $command, )+
        }

        impl BuiltinCommand {
            /// Every ledger row, in protocol order.
            pub(crate) const ALL: &'static [Self] = &[ $( Self::$command, )+ ];

            /// Returns the ledger's classification of this row.
            #[must_use]
            pub(crate) const fn classification(self) -> BuiltinRequestClass {
                match self {
                    $( Self::$command => ledger_class!($class $(($($effect)*))?), )+
                }
            }
        }
    };
}

/// Expands one ledger row form into its [`BuiltinRequestClass`].
macro_rules! ledger_class {
    (plain) => {
        BuiltinRequestClass::Plain { state_effect: None }
    };
    (plain(Set($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Set(StateKey::$key)),
        }
    };
    (plain(Clear($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Clear(StateKey::$key)),
        }
    };
    (plain(Invalidate($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Invalidate(
                StateKey::$key,
            )),
        }
    };
    // Must precede the generic `targeted($axes)` arm.
    (targeted(profile_preset_recall)) => {
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::ProfilePresetRecall,
        }
    };
    (targeted($axes:ident)) => {
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::Exact(AffectedAxes::$axes),
        }
    };
    (applied($axes:ident)) => {
        BuiltinRequestClass::AppliedOnly {
            axes: BuiltinAxisSelection::Exact(AffectedAxes::$axes),
        }
    };
    ($($row:tt)*) => {
        compile_error!(concat!(
            "unknown ledger class `",
            stringify!($($row)*),
            "`; expected plain, plain(Set(<StateKey>)), plain(Clear(<StateKey>)), ",
            "plain(Invalidate(<StateKey>)), targeted(<AXES>), ",
            "targeted(profile_preset_recall) or applied(<AXES>)"
        ))
    };
}

builtin_command_ledger! {
    // Pan/tilt.
    PanTiltHome => targeted(PAN_TILT),
    PanTiltReset => targeted(PAN_TILT),
    PanTiltDrive => applied(PAN_TILT),
    PanTiltStop => applied(PAN_TILT),
    PanTiltAbsolute => targeted(PAN_TILT),
    PanTiltRelative => targeted(PAN_TILT),
    PanTiltLimitSet => plain(Set(PanTiltLimits)),
    PanTiltLimitClear => plain(Clear(PanTiltLimits)),
    // Zoom.
    ZoomStop => applied(ZOOM),
    ZoomTele => applied(ZOOM),
    ZoomWide => applied(ZOOM),
    ZoomTeleVariable => applied(ZOOM),
    ZoomWideVariable => applied(ZOOM),
    ZoomPosition => targeted(ZOOM),
    DigitalZoom => plain(Set(DigitalZoomMode)),
    // Focus.
    FocusStop => applied(FOCUS),
    FocusFar => applied(FOCUS),
    FocusNear => applied(FOCUS),
    FocusFarVariable => applied(FOCUS),
    FocusNearVariable => applied(FOCUS),
    FocusPosition => targeted(FOCUS),
    FocusAuto => plain,
    FocusManual => plain,
    FocusOnePush => applied(FOCUS),
    FocusInfinity => targeted(FOCUS),
    FocusToggle => plain,
    FocusSnap => applied(FOCUS),
    FocusZone => plain,
    FocusAutoSensitivity => plain,
    FocusNearLimit => plain,
    FocusLock => plain(Set(FocusLockMode)),
    PushAfPress => applied(FOCUS),
    PushAfRelease => applied(FOCUS),
    // Presets.
    PresetRecall => targeted(profile_preset_recall),
    PresetRecallSpeed => plain(Set(PresetRecallSpeed)),
    PresetSet => plain,
    PresetReset => plain,
    // Power.
    PowerOn => plain,
    PowerStandby => plain,
    // Exposure and iris.
    ExposureMode => plain,
    ExposureCompensationOn => plain,
    ExposureCompensationOff => plain,
    ExposureCompensationReset => plain,
    ExposureCompensationUp => plain,
    ExposureCompensationDown => plain,
    ExposureCompensationDirect => plain,
    DynamicRange => plain,
    // Iris commands are finite physical aperture movements.  They have a
    // meaningful end state and an exact Iris inquiry, so every
    // reset/step/direct form is targeted rather than applied-only.
    IrisReset => targeted(IRIS),
    IrisUp => targeted(IRIS),
    IrisDown => targeted(IRIS),
    IrisDirect => targeted(IRIS),
    ShutterReset => plain,
    ShutterUp => plain,
    ShutterDown => plain,
    ShutterDirect => plain,
    BrightnessReset => plain,
    BrightnessUp => plain,
    BrightnessDown => plain,
    BrightnessSet => plain,
    AntiFlicker => plain,
    SpotlightOn => plain(Set(Spotlight)),
    SpotlightOff => plain(Set(Spotlight)),
    AutoSlowShutterOn => plain(Set(AutoSlowShutter)),
    AutoSlowShutterOff => plain(Set(AutoSlowShutter)),
    // Gain.
    GainReset => plain,
    GainUp => plain,
    GainDown => plain,
    GainDirect => plain,
    GainLimit => plain,
    // White balance.
    WhiteBalanceAuto => plain,
    WhiteBalanceIndoor => plain,
    WhiteBalanceOutdoor => plain,
    WhiteBalanceOnePush => plain,
    WhiteBalanceAutoTracking => plain,
    WhiteBalanceManual => plain,
    WhiteBalanceColorTemperature => plain,
    AutoWhiteBalanceSensitivity => plain,
    OnePushWhiteBalanceTrigger => plain,
    // Color.
    RedTuning => plain,
    BlueTuning => plain,
    Saturation => plain,
    Hue => plain,
    ColorTemperatureReset => plain,
    ColorTemperatureUp => plain,
    ColorTemperatureDown => plain,
    ColorTemperatureDirect => plain,
    RedGainReset => plain,
    RedGainUp => plain,
    RedGainDown => plain,
    RedGainDirect => plain,
    BlueGainReset => plain,
    BlueGainUp => plain,
    BlueGainDown => plain,
    BlueGainDirect => plain,
    // Image processing and orientation.
    SharpnessMode => plain,
    SharpnessReset => plain,
    SharpnessUp => plain,
    SharpnessDown => plain,
    SharpnessDirect => plain,
    Luminance => plain,
    Contrast => plain,
    Gamma => plain,
    Backlight => plain,
    NoiseReduction2dMode => plain,
    NoiseReduction2d => plain,
    NoiseReduction2dOff => plain,
    NoiseReduction3d => plain,
    NoiseReduction3dOff => plain,
    // The single-axis flip and mirror opcodes move one axis and say nothing
    // about the other, so a complete pair stops being known.
    ImageFlipOff => plain(Invalidate(Flip)),
    ImageFlipHorizontal => plain(Invalidate(Flip)),
    ImageFlipHorizontalOff => plain(Invalidate(Flip)),
    ImageFlipVertical => plain(Invalidate(Flip)),
    // The combined-flip opcode carries both axes in one parameter, so it
    // establishes a complete horizontal/vertical pair.
    ImageFlipBoth => plain(Set(Flip)),
    ImageFlipCombined => plain(Set(Flip)),
    // The `Off` request supplies `false`: a known value, not removal of the
    // cache entry.
    ImageFreezeOn => plain(Set(ImageFreeze)),
    ImageFreezeOff => plain(Set(ImageFreeze)),
    PictureEffect => plain,
    // Neutral-density filter.
    NdFilterMode => plain(Set(NdFilterMode)),
    // ND direct and step commands physically reposition the filter. The
    // direct target and each finite step have meaningful end states and are
    // therefore targeted on the ND axis.
    NdFilterDirect => targeted(ND_FILTER),
    NdFilterStepUp => targeted(ND_FILTER),
    NdFilterStepDown => targeted(ND_FILTER),
    NdFilterAutoOn => plain(Set(AutoNdFilter)),
    NdFilterAutoOff => plain(Set(AutoNdFilter)),
    // Tally. Red/green tally status has profile-gated inquiries. Brightness
    // and vendor output mode do not, so their applied effects are either
    // deterministic or explicitly invalidated.
    TallyRedOn => plain,
    TallyRedOff => plain,
    TallyBrightLow => plain(Set(TallyBrightness)),
    TallyBrightHigh => plain(Set(TallyBrightness)),
    TallyGreenOn => plain,
    TallyGreenOff => plain,
    TallyFlash => plain(Invalidate(TallyMode)),
    TallyOn => plain(Set(TallyMode)),
    TallyOff => plain(Set(TallyMode)),
    // Menus.
    MenuDisplay => plain,
    MenuNavigate => plain,
    MenuSelect => plain,
    MenuCancel => plain,
    DirectMenu => plain,
    // Streaming. The multicast `Off` request supplies `false`: a known value,
    // not removal of the cache entry.
    MulticastStreamingOn => plain(Set(MulticastStreaming)),
    MulticastStreamingOff => plain(Set(MulticastStreaming)),
    NdiQuality => plain(Set(NdiQuality)),
    UsbAudioOn => plain,
    UsbAudioOff => plain,
    // System.
    AddressSet => plain,
    InterfaceClear => plain,
    CommandCancel => plain,
    SettingsSave => plain,
    // Motion-related configuration.
    MotionSyncMode => plain,
    MotionSyncPreset => plain,
    VariableSpeedMode => plain(Set(VariableSpeedMode)),
}

#[cfg(test)]
impl BuiltinCommand {
    /// Returns whether this command is a plain request.
    #[must_use]
    pub(crate) const fn is_plain(self) -> bool {
        matches!(self.classification(), BuiltinRequestClass::Plain { .. })
    }

    /// Returns the operation completion class, if this command is an
    /// operation.
    #[must_use]
    pub(crate) const fn completion(self) -> Option<BuiltinCompletionClass> {
        self.classification().completion()
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
        for command in BuiltinCommand::ALL {
            assert!(seen.insert(*command), "duplicate ledger row: {command:?}");
            let class = command.classification();
            if command.is_plain() {
                assert!(class.axes().is_none());
            } else {
                assert!(
                    command.completion().is_some(),
                    "operation row has no completion class: {command:?}"
                );
                let axes = class.axes().expect("operation must declare axes");
                if let BuiltinAxisSelection::Exact(axes) = axes {
                    assert_ne!(axes.bits(), 0);
                }
            }
        }
        assert_eq!(seen.len(), BuiltinCommand::ALL.len());
    }

    #[test]
    fn operation_rows_use_only_exact_axes_or_profile_selected_preset_axes() {
        for command in BuiltinCommand::ALL {
            match command.classification() {
                BuiltinRequestClass::Targeted { axes }
                | BuiltinRequestClass::AppliedOnly { axes } => {
                    if let BuiltinAxisSelection::Exact(axes) = axes {
                        assert!(axes.is_single() || axes.axis_count() > 1);
                    }
                }
                BuiltinRequestClass::Plain { .. } => {}
            }
        }
        assert_eq!(
            BuiltinCommand::PresetRecall.classification().axes(),
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
                command.classification().axes(),
                Some(BuiltinAxisSelection::Exact(AffectedAxes::IRIS))
            );
        }
        for command in [
            BuiltinCommand::NdFilterDirect,
            BuiltinCommand::NdFilterStepUp,
            BuiltinCommand::NdFilterStepDown,
        ] {
            assert_eq!(command.completion(), Some(BuiltinCompletionClass::Targeted));
            assert_eq!(
                command.classification().axes(),
                Some(BuiltinAxisSelection::Exact(AffectedAxes::ND_FILTER))
            );
        }
        for command in [BuiltinCommand::PushAfPress, BuiltinCommand::PushAfRelease] {
            assert_eq!(
                command.completion(),
                Some(BuiltinCompletionClass::AppliedOnly)
            );
            assert_eq!(
                command.classification().axes(),
                Some(BuiltinAxisSelection::Exact(AffectedAxes::FOCUS))
            );
        }
    }

    #[test]
    fn write_only_state_effects_are_closed() {
        assert_eq!(
            BuiltinCommand::PanTiltLimitSet
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(StateKey::PanTiltLimits))
        );
        assert_eq!(
            BuiltinCommand::PanTiltLimitClear
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Clear(
                StateKey::PanTiltLimits
            ))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOn
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(StateKey::ImageFreeze))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOff
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(StateKey::ImageFreeze))
        );
        assert_eq!(
            BuiltinCommand::TallyFlash.classification().state_effect(),
            Some(AppliedStateEffectRequirement::Invalidate(
                StateKey::TallyMode
            ))
        );
        assert_eq!(
            BuiltinCommand::DigitalZoom.classification().state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                StateKey::DigitalZoomMode
            ))
        );

        let represented: HashSet<_> = BuiltinCommand::ALL
            .iter()
            .filter_map(|command| {
                command
                    .classification()
                    .state_effect()
                    .map(|effect| effect.state())
            })
            .collect();
        let listed: HashSet<_> = StateKey::ALL.iter().copied().collect();
        assert_eq!(listed.len(), StateKey::ALL.len());
        for state in StateKey::ALL {
            assert!(
                represented.contains(state),
                "write-only state has no ledger effect: {state:?}"
            );
        }
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .classification()
                .state_effect()
                .is_some_and(|effect| matches!(effect, AppliedStateEffectRequirement::Set(_)))
        }));
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .classification()
                .state_effect()
                .is_some_and(|effect| matches!(effect, AppliedStateEffectRequirement::Clear(_)))
        }));
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .classification()
                .state_effect()
                .is_some_and(|effect| {
                    matches!(effect, AppliedStateEffectRequirement::Invalidate(_))
                })
        }));
    }

    /// Pins the request policy of every built-in row whose timeout class or
    /// retry class is a deliberate decision rather than the generic default for
    /// its domain, so that changing one is a visible, reviewed edit here.
    ///
    /// - Urgent stops and focus triggers complete on the quick deadline but keep
    ///   the movement retry class, because an unacknowledged stop must still be
    ///   retried on movement error evidence.
    /// - Pan/tilt limit edits and focus-mode settings change configuration, not
    ///   position, so they are plain quick/standard writes.
    /// - Targeted iris and ND-filter operations settle through profile-selected
    ///   protocol inquiries, so they use the movement deadline.
    /// - Image-tuning writes (sharpness, gamma, noise reduction, flip) are quick
    ///   configuration writes; the 2D noise-reduction mode control shares that
    ///   policy.
    /// - `CommandCancel` is quick and never retried; preset set/reset/recall use
    ///   the preset deadline and retry class.
    #[test]
    fn deliberate_request_timeout_and_retry_policies_are_pinned() {
        use crate::request::builtin as req;
        use crate::{RetryClass as Retry, TimeoutClass as Timeout};

        fn pin<R: crate::Request>(
            command: BuiltinCommand,
            timeout: Timeout,
            retry: Retry,
            pinned: &mut HashSet<BuiltinCommand>,
        ) {
            assert_eq!(R::TIMEOUT_CLASS, timeout, "{command:?} timeout class");
            assert_eq!(R::RETRY_CLASS, retry, "{command:?} retry class");
            assert!(
                BuiltinCommand::ALL.contains(&command),
                "{command:?} is not a BuiltinCommand row"
            );
            assert!(pinned.insert(command), "{command:?} pinned twice");
        }

        let mut pinned = HashSet::new();
        let p = &mut pinned;

        // Urgent stops and applied-only focus triggers: quick deadline,
        // movement retry class.
        pin::<req::PanTiltStop>(
            BuiltinCommand::PanTiltStop,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::ZoomStop>(BuiltinCommand::ZoomStop, Timeout::Quick, Retry::Movement, p);
        pin::<req::FocusStop>(
            BuiltinCommand::FocusStop,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::FocusTrigger>(
            BuiltinCommand::FocusOnePush,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::FocusTrigger>(
            BuiltinCommand::FocusSnap,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::PushAfPress>(
            BuiltinCommand::PushAfPress,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::PushAfRelease>(
            BuiltinCommand::PushAfRelease,
            Timeout::Quick,
            Retry::Movement,
            p,
        );

        // Plain configuration edits: quick deadline, standard retry class.
        pin::<req::PanTiltLimitSet>(
            BuiltinCommand::PanTiltLimitSet,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        pin::<req::PanTiltLimitClear>(
            BuiltinCommand::PanTiltLimitClear,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        for command in [
            BuiltinCommand::FocusAuto,
            BuiltinCommand::FocusManual,
            BuiltinCommand::FocusToggle,
        ] {
            pin::<req::FocusModeCommand>(command, Timeout::Quick, Retry::Standard, p);
        }
        for command in [
            BuiltinCommand::SharpnessMode,
            BuiltinCommand::SharpnessReset,
            BuiltinCommand::SharpnessUp,
            BuiltinCommand::SharpnessDown,
            BuiltinCommand::SharpnessDirect,
        ] {
            pin::<crate::command::Sharpness>(command, Timeout::Quick, Retry::Standard, p);
        }
        pin::<crate::command::GammaCommand>(
            BuiltinCommand::Gamma,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        for command in [
            BuiltinCommand::NoiseReduction2d,
            BuiltinCommand::NoiseReduction2dOff,
        ] {
            pin::<crate::command::NoiseReduction2D>(command, Timeout::Quick, Retry::Standard, p);
        }
        for command in [
            BuiltinCommand::NoiseReduction3d,
            BuiltinCommand::NoiseReduction3dOff,
        ] {
            pin::<crate::command::NoiseReduction3D>(command, Timeout::Quick, Retry::Standard, p);
        }
        pin::<crate::command::NoiseReduction2DModeCommand>(
            BuiltinCommand::NoiseReduction2dMode,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        for command in [
            BuiltinCommand::ImageFlipBoth,
            BuiltinCommand::ImageFlipCombined,
        ] {
            pin::<crate::command::ImageFlipCombinedCommand>(
                command,
                Timeout::Quick,
                Retry::Standard,
                p,
            );
        }

        // Targeted iris and ND-filter operations: movement deadline and retry.
        pin::<req::IrisReset>(
            BuiltinCommand::IrisReset,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::IrisUp>(
            BuiltinCommand::IrisUp,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::IrisDown>(
            BuiltinCommand::IrisDown,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::IrisDirect>(
            BuiltinCommand::IrisDirect,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::NdFilterDirect>(
            BuiltinCommand::NdFilterDirect,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::NdFilterStepUp>(
            BuiltinCommand::NdFilterStepUp,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::NdFilterStepDown>(
            BuiltinCommand::NdFilterStepDown,
            Timeout::Movement,
            Retry::Movement,
            p,
        );

        // Cancellation is never replayed; presets have their own class.
        pin::<crate::command::system::CommandCancelCommand>(
            BuiltinCommand::CommandCancel,
            Timeout::Quick,
            Retry::Never,
            p,
        );
        pin::<req::PresetSet>(BuiltinCommand::PresetSet, Timeout::Preset, Retry::Preset, p);
        pin::<req::PresetReset>(
            BuiltinCommand::PresetReset,
            Timeout::Preset,
            Retry::Preset,
            p,
        );
        pin::<req::PresetRecall>(
            BuiltinCommand::PresetRecall,
            Timeout::Preset,
            Retry::Preset,
            p,
        );
    }
}
