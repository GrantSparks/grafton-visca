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
//! The production typed-request lowering consumes this ledger through the
//! private contracts near [`BuiltinRequestClass`].  Every concrete built-in
//! request row supplies a monomorphized contract marker; the marker compares
//! the request's closed class, fixed/profile-dependent axes, and (where
//! applicable) applied-state requirement with the ledger row's
//! classification.  The inventory is therefore useful in ordinary builds, not
//! only in the independent semantic tests.

use crate::AffectedAxes;

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

/// Private semantic spelling for the public [`crate::state_cache::StateKey`].
///
/// Keeping this alias in the ledger preserves the existing internal naming
/// while ensuring that the public cache key list and the applied-state ledger
/// have one source of truth.
pub(crate) use crate::state_cache::StateKey as WriteOnlyState;

/// Required cache projection for one write-only state command.
///
/// The value itself is carried by the typed request. This closed requirement
/// tells preparation whether that value can be written,
/// removed, or must be forgotten after exact application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppliedStateEffectRequirement {
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
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn state(self) -> WriteOnlyState {
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

    /// Returns the closed write-only state requirement, if any.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn state_effect(self) -> Option<AppliedStateEffectRequirement> {
        match self {
            Self::Plain { state_effect } => state_effect,
            Self::Targeted { .. } | Self::AppliedOnly { .. } => None,
        }
    }
}

/// Completion class selected by a built-in operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinCompletionClass {
    /// Physical motion has a meaningful terminal state.
    Targeted,
    /// Physical actuation is observed only through command application.
    AppliedOnly,
}

/// Private class contract implemented by the two closed operation markers.
///
/// This keeps the expected completion class in the type-level request
/// contract. A coverage marker that names `Operation<Targeted>` therefore
/// cannot accidentally validate an `AppliedOnly` ledger row (or vice versa).
pub(crate) trait BuiltinCompletionContract {
    /// Ledger completion class represented by this request marker.
    const CLASS: BuiltinCompletionClass;
}

impl BuiltinCompletionContract for crate::completion::Targeted {
    const CLASS: BuiltinCompletionClass = BuiltinCompletionClass::Targeted;
}

impl BuiltinCompletionContract for crate::completion::AppliedOnly {
    const CLASS: BuiltinCompletionClass = BuiltinCompletionClass::AppliedOnly;
}

/// Private operation contract shared by concrete built-in operation types.
///
/// `AXIS_SELECTION` is the exact ledger spelling. Profile-dependent
/// operations use [`BuiltinAxisSelection::ProfilePresetRecall`]; they never
/// manufacture a broad fallback set.
pub(crate) trait BuiltinOperationContract {
    /// Exact fixed axes or the profile-selected preset-recall marker.
    const AXIS_SELECTION: BuiltinAxisSelection;
}

/// Private fixed-axis contract consumed by production operation methods.
///
/// The associated constant is deliberately separate from
/// [`BuiltinOperationContract::AXIS_SELECTION`]: a profile-dependent
/// operation has no fixed `AffectedAxes` value and must retain its validated
/// runtime profile selection.
pub(crate) trait BuiltinFixedOperationContract: BuiltinOperationContract {
    /// Concrete non-empty axes returned by `OperationCommand::affected_axes`.
    const AFFECTED_AXES: AffectedAxes;
}

/// Private state-effect contract consumed by production applied-state
/// projection.
///
/// Only built-in request types with a write-only state effect implement this
/// trait. Plain rows without an effect use the `None` branch of the plain
/// coverage marker, so an omitted implementation cannot hide a state row.
pub(crate) trait BuiltinStateEffectContract {
    /// Closed state effect required after exact protocol application.
    const STATE_EFFECT: AppliedStateEffectRequirement;
}

/// Fails const evaluation when a plain ledger row and its typed request
/// disagree about class or state effect.
pub(crate) const fn assert_plain_request_contract(
    row: BuiltinCommand,
    expected_effect: Option<AppliedStateEffectRequirement>,
) {
    let contract_matches = match row.classification() {
        BuiltinRequestClass::Plain { state_effect } => {
            option_state_effect_equal(state_effect, expected_effect)
        }
        BuiltinRequestClass::Targeted { .. } | BuiltinRequestClass::AppliedOnly { .. } => false,
    };
    assert!(contract_matches);
}

const fn option_state_effect_equal(
    left: Option<AppliedStateEffectRequirement>,
    right: Option<AppliedStateEffectRequirement>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => state_effect_equal(left, right),
        (None, Some(_)) | (Some(_), None) => false,
    }
}

const fn state_effect_equal(
    left: AppliedStateEffectRequirement,
    right: AppliedStateEffectRequirement,
) -> bool {
    match (left, right) {
        (AppliedStateEffectRequirement::Set(left), AppliedStateEffectRequirement::Set(right))
        | (
            AppliedStateEffectRequirement::Clear(left),
            AppliedStateEffectRequirement::Clear(right),
        )
        | (
            AppliedStateEffectRequirement::Invalidate(left),
            AppliedStateEffectRequirement::Invalidate(right),
        ) => state_key_equal(left, right),
        _ => false,
    }
}

const fn state_key_equal(left: WriteOnlyState, right: WriteOnlyState) -> bool {
    left as u8 == right as u8
}

/// Monomorphizes the closed plain-request class marker and validates the
/// ledger row in a production const initializer.
pub(crate) const fn plain_request_contract<T>(
    row: BuiltinCommand,
    expected_effect: Option<AppliedStateEffectRequirement>,
) -> fn()
where
    T: crate::requests::Request<Class = crate::request::Plain>,
{
    assert_plain_request_contract(row, expected_effect);
    builtin_plain_marker::<T>
}

/// Monomorphizes a state-bearing plain request and derives its expected ledger
/// effect from the private production state contract.
pub(crate) const fn state_request_contract<T>(row: BuiltinCommand) -> fn()
where
    T: crate::requests::Request<Class = crate::request::Plain> + BuiltinStateEffectContract,
{
    assert_plain_request_contract(row, Some(<T as BuiltinStateEffectContract>::STATE_EFFECT));
    builtin_plain_marker::<T>
}

const fn builtin_plain_marker<T>()
where
    T: crate::requests::Request<Class = crate::request::Plain>,
{
}

/// Fails const evaluation when a fixed-axis operation row and its typed
/// request disagree about completion class or axes.
pub(crate) const fn assert_fixed_operation_contract<T, K>(row: BuiltinCommand) -> fn()
where
    T: crate::requests::Request<Class = crate::request::Operation<K>>
        + crate::requests::OperationCommand<K>
        + BuiltinFixedOperationContract,
    K: crate::completion::Kind + BuiltinCompletionContract,
{
    let expected_axes = <T as BuiltinOperationContract>::AXIS_SELECTION;
    let concrete_axes = <T as BuiltinFixedOperationContract>::AFFECTED_AXES;
    assert!(matches!(expected_axes, BuiltinAxisSelection::Exact(_)));
    if let BuiltinAxisSelection::Exact(axes) = expected_axes {
        assert!(axes.bits() == concrete_axes.bits());
    }
    assert_operation_classification(row, K::CLASS, expected_axes);
    builtin_operation_marker::<T, K>
}

/// Fails const evaluation when a profile-dependent operation row and its
/// typed request disagree about completion class or axis-selection mode.
pub(crate) const fn assert_profile_operation_contract<T, K>(row: BuiltinCommand) -> fn()
where
    T: crate::requests::Request<Class = crate::request::Operation<K>>
        + crate::requests::OperationCommand<K>
        + BuiltinOperationContract,
    K: crate::completion::Kind + BuiltinCompletionContract,
{
    let expected_axes = <T as BuiltinOperationContract>::AXIS_SELECTION;
    assert!(matches!(
        expected_axes,
        BuiltinAxisSelection::ProfilePresetRecall
    ));
    assert_operation_classification(row, K::CLASS, expected_axes);
    builtin_operation_marker::<T, K>
}

const fn assert_operation_classification(
    row: BuiltinCommand,
    expected_class: BuiltinCompletionClass,
    expected_axes: BuiltinAxisSelection,
) {
    let contract_matches = match (row.classification(), expected_class) {
        (BuiltinRequestClass::Targeted { axes }, BuiltinCompletionClass::Targeted)
        | (BuiltinRequestClass::AppliedOnly { axes }, BuiltinCompletionClass::AppliedOnly) => {
            axis_selection_equal(axes, expected_axes)
        }
        (
            BuiltinRequestClass::Plain { .. },
            BuiltinCompletionClass::Targeted | BuiltinCompletionClass::AppliedOnly,
        )
        | (BuiltinRequestClass::Targeted { .. }, BuiltinCompletionClass::AppliedOnly)
        | (BuiltinRequestClass::AppliedOnly { .. }, BuiltinCompletionClass::Targeted) => false,
    };
    assert!(contract_matches);
}

const fn axis_selection_equal(left: BuiltinAxisSelection, right: BuiltinAxisSelection) -> bool {
    match (left, right) {
        (BuiltinAxisSelection::Exact(left), BuiltinAxisSelection::Exact(right)) => {
            left.bits() == right.bits()
        }
        (BuiltinAxisSelection::ProfilePresetRecall, BuiltinAxisSelection::ProfilePresetRecall) => {
            true
        }
        (BuiltinAxisSelection::Exact(_), BuiltinAxisSelection::ProfilePresetRecall)
        | (BuiltinAxisSelection::ProfilePresetRecall, BuiltinAxisSelection::Exact(_)) => false,
    }
}

const fn builtin_operation_marker<T, K>()
where
    T: crate::requests::Request<Class = crate::request::Operation<K>>
        + crate::requests::OperationCommand<K>,
    K: crate::completion::Kind,
{
}

/// The closed semantic ledger: one row per built-in command, in protocol
/// order.
///
/// `plain`, `plain(Set(<key>))`, `plain(Clear(<key>))`,
/// `plain(Invalidate(<key>))`, `targeted(<axes>)`,
/// `targeted(profile_preset_recall)` and `applied(<axes>)` are the only row
/// forms; `<key>` names a [`WriteOnlyState`] and `<axes>` an
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
            state_effect: Some(AppliedStateEffectRequirement::Set(WriteOnlyState::$key)),
        }
    };
    (plain(Clear($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Clear(WriteOnlyState::$key)),
        }
    };
    (plain(Invalidate($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Invalidate(
                WriteOnlyState::$key,
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
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::PanTiltLimits
            ))
        );
        assert_eq!(
            BuiltinCommand::PanTiltLimitClear
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Clear(
                WriteOnlyState::PanTiltLimits
            ))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOn
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::ImageFreeze
            ))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOff
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::ImageFreeze
            ))
        );
        assert_eq!(
            BuiltinCommand::TallyFlash.classification().state_effect(),
            Some(AppliedStateEffectRequirement::Invalidate(
                WriteOnlyState::TallyMode
            ))
        );
        assert_eq!(
            BuiltinCommand::DigitalZoom.classification().state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                WriteOnlyState::DigitalZoomMode
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
