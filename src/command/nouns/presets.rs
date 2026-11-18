//! Preset memory, recall and configuration requests.

use crate::command::nouns::{forward_inquiry, forward_request};
use crate::{
    command::{inquiry_structs, preset_wire, sony_fr7_presets},
    request::builtin,
    types, TargetRegistration,
};

/// Request recall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recall {
    inner: builtin::PresetRecall,
    preset: types::PresetNumber,
}
impl Recall {
    /// Creates a recall intent. Preparation selects the receiving camera's
    /// motion axes and checks its preset range.
    #[must_use]
    pub const fn new(preset: types::PresetNumber) -> Self {
        Self {
            inner: builtin::PresetRecall::new(preset),
            preset,
        }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }
}
forward_request!(Recall, builtin::PresetRecall, Targeted);

/// Request store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Store {
    inner: builtin::PresetSet,
    preset: types::PresetNumber,
}
impl Store {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber) -> Self {
        let inner = builtin::PresetSet::new(preset);
        Self { inner, preset }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }
}
forward_request!(Store, builtin::PresetSet, Plain);

/// Request clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clear {
    inner: builtin::PresetReset,
    preset: types::PresetNumber,
}
impl Clear {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber) -> Self {
        let inner = builtin::PresetReset::new(preset);
        Self { inner, preset }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }
}
forward_request!(Clear, builtin::PresetReset, Plain);

/// Selects how later preset recalls choose their transition setting: `Separate` uses each preset's configured speed or duration, while `Common` uses the shared recall speed. This configures recalls without starting one (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSpeedSelection {
    inner: sony_fr7_presets::Fr7SetPresetSpeedSelection,
    selection: types::PresetSpeedSelection,
}
impl SetSpeedSelection {
    /// Creates the request from checked domain values.
    pub fn new(selection: types::PresetSpeedSelection) -> Self {
        let inner = sony_fr7_presets::Fr7SetPresetSpeedSelection::new(selection);
        Self { inner, selection }
    }

    /// Returns the requested selection.
    #[must_use]
    pub const fn selection(&self) -> types::PresetSpeedSelection {
        self.selection
    }
}
forward_request!(
    SetSpeedSelection,
    sony_fr7_presets::Fr7SetPresetSpeedSelection,
    Plain
);

/// Reads whether later recalls use per-preset speed/duration settings (`Separate`) or one shared recall speed (`Common`) (R7 pp. 19 and 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedSelectionInquiry;
forward_inquiry!(SpeedSelectionInquiry, inquiry_structs::Fr7PresetSpeedSelectionInquiry; inquiry_structs::Fr7PresetSpeedSelectionInquiry);

/// Reads the shared preset-recall speed level, `1..=50` for the FR7 encoding. This is the value used when the speed selection is `Common` (R7 pp. 19 and 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallSpeedInquiry;
forward_inquiry!(RecallSpeedInquiry, inquiry_structs::Fr7CommonPresetSpeedInquiry; inquiry_structs::Fr7CommonPresetSpeedInquiry);

/// Selects whether the selected preset uses its configured speed or its configured duration on a later recall. `PresetNumber` is the zero-based packet slot `0..=99` for FR7, corresponding to menu presets `1..=100` (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSeparateMode {
    inner: sony_fr7_presets::Fr7SetSeparatePresetMode,
    preset: types::PresetNumber,
    mode: types::PresetSeparateMode,
}
impl SetSeparateMode {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber, mode: types::PresetSeparateMode) -> Self {
        let inner = sony_fr7_presets::Fr7SetSeparatePresetMode::new(preset, mode);
        Self {
            inner,
            preset,
            mode,
        }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(&self) -> types::PresetSeparateMode {
        self.mode
    }
}
forward_request!(
    SetSeparateMode,
    sony_fr7_presets::Fr7SetSeparatePresetMode,
    Plain
);

/// Reads whether the selected preset uses its configured speed or its configured duration on a later recall. `PresetNumber` is the zero-based packet slot `0..=99` for FR7, corresponding to menu presets `1..=100` (R7 pp. 19 and 22).
#[derive(Debug, Clone, Copy)]
pub struct SeparateModeInquiry {
    inner: inquiry_structs::Fr7SeparatePresetModeInquiry,
    preset: types::PresetNumber,
}
impl SeparateModeInquiry {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber) -> Self {
        let inner = inquiry_structs::Fr7SeparatePresetModeInquiry::new(preset);
        Self { inner, preset }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }
}
forward_inquiry!(
    SeparateModeInquiry,
    inquiry_structs::Fr7SeparatePresetModeInquiry
);

/// Sets the speed level for the selected preset's later recall, `1..=50` on FR7. It applies when selection is `Separate` and that preset's mode is `Speed`; the packet slot `0..=99` corresponds to menu presets `1..=100` (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSeparateSpeed {
    inner: sony_fr7_presets::Fr7SetSeparatePresetSpeed,
    preset: types::PresetNumber,
    speed: types::PresetSpeed,
}
impl SetSeparateSpeed {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber, speed: types::PresetSpeed) -> Self {
        let inner = sony_fr7_presets::Fr7SetSeparatePresetSpeed::new(preset, speed);
        Self {
            inner,
            preset,
            speed,
        }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }

    /// Returns the requested speed.
    #[must_use]
    pub const fn speed(&self) -> types::PresetSpeed {
        self.speed
    }
}
forward_request!(
    SetSeparateSpeed,
    sony_fr7_presets::Fr7SetSeparatePresetSpeed,
    Plain
);

/// Reads the speed level for the selected preset's later recall, `1..=50` on FR7. It applies when selection is `Separate` and that preset's mode is `Speed`; the packet slot `0..=99` corresponds to menu presets `1..=100` (R7 pp. 19 and 22).
#[derive(Debug, Clone, Copy)]
pub struct SeparateSpeedInquiry {
    inner: inquiry_structs::Fr7SeparatePresetSpeedInquiry,
    preset: types::PresetNumber,
}
impl SeparateSpeedInquiry {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber) -> Self {
        let inner = inquiry_structs::Fr7SeparatePresetSpeedInquiry::new(preset);
        Self { inner, preset }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }
}
forward_inquiry!(
    SeparateSpeedInquiry,
    inquiry_structs::Fr7SeparatePresetSpeedInquiry
);

/// Sets the transition duration for the selected preset's later recall, from 1 to 99 seconds in exact 100 ms increments. It applies when selection is `Separate` and that preset's mode is `Duration`; packet slots `0..=99` correspond to menu presets `1..=100` (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSeparateDuration {
    inner: sony_fr7_presets::Fr7SetSeparatePresetDuration,
    preset: types::PresetNumber,
    duration: std::time::Duration,
}
impl SetSeparateDuration {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber, duration: std::time::Duration) -> crate::Result<Self> {
        let inner = sony_fr7_presets::Fr7SetSeparatePresetDuration::new(preset, duration)?;
        Ok(Self {
            inner,
            preset,
            duration,
        })
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }

    /// Returns the requested duration.
    #[must_use]
    pub const fn duration(&self) -> std::time::Duration {
        self.duration
    }
}
forward_request!(
    SetSeparateDuration,
    sony_fr7_presets::Fr7SetSeparatePresetDuration,
    Plain
);

/// Reads the transition duration for the selected preset's later recall, from 1 to 99 seconds in exact 100 ms increments. It applies when selection is `Separate` and that preset's mode is `Duration`; packet slots `0..=99` correspond to menu presets `1..=100` (R7 pp. 19 and 22).
#[derive(Debug, Clone, Copy)]
pub struct SeparateDurationInquiry {
    inner: inquiry_structs::Fr7SeparatePresetDurationInquiry,
    preset: types::PresetNumber,
}
impl SeparateDurationInquiry {
    /// Creates the request from checked domain values.
    pub fn new(preset: types::PresetNumber) -> Self {
        let inner = inquiry_structs::Fr7SeparatePresetDurationInquiry::new(preset);
        Self { inner, preset }
    }

    /// Returns the requested preset.
    #[must_use]
    pub const fn preset(&self) -> types::PresetNumber {
        self.preset
    }
}
forward_inquiry!(
    SeparateDurationInquiry,
    inquiry_structs::Fr7SeparatePresetDurationInquiry
);

/// Set the speed used for subsequent preset recalls.
///
/// The target selects the protocol register: PTZOptics accepts 1 through 24;
/// Sony FR7 accepts 1 through 50. This setting does not start a recall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetRecallSpeed {
    speed: types::PresetSpeed,
}

impl SetRecallSpeed {
    /// Captures a speed to validate and encode for the receiving target.
    #[must_use]
    pub const fn new(speed: types::PresetSpeed) -> Self {
        Self { speed }
    }

    fn encoding_for_target(
        target: &TargetRegistration,
    ) -> crate::Result<crate::capabilities::PresetRecallSpeedEncoding> {
        target.profile().encodings().preset_recall_speed().ok_or(
            crate::Error::FeatureNotSupported {
                feature: "preset recall speed encoding",
            },
        )
    }

    /// Returns the requested recall speed.
    #[must_use]
    pub const fn speed(&self) -> types::PresetSpeed {
        self.speed
    }

    fn validate_speed(&self, target: &TargetRegistration) -> crate::Result<()> {
        #[cfg(test)]
        builtin::record_request_value_validation();
        let range = target
            .profile()
            .capabilities()
            .presets
            .as_ref()
            .and_then(|caps| caps.recall_speed_range.as_ref())
            .ok_or(crate::Error::FeatureNotSupported {
                feature: "preset recall speed range",
            })?;
        if self.speed.value() < range.min() || self.speed.value() > range.max() {
            return Err(crate::Error::parameter_out_of_range(
                "preset recall speed",
                i32::from(self.speed.value()),
                i32::from(range.min()),
                i32::from(range.max()),
            ));
        }
        Ok(())
    }
}

forward_request!(SetRecallSpeed, preset_wire::PresetRecallSpeedCommand, Plain;
    select target |request, target| (Self::encoding_for_target(target)?)
        default (crate::capabilities::PresetRecallSpeedEncoding::PtzOptics) {
        crate::capabilities::PresetRecallSpeedEncoding::PtzOptics => (
            preset_wire::PresetRecallSpeedCommand, PresetRecallSpeed,
            preset_wire::PresetRecallSpeedCommand::new(request.speed)
        ),
        crate::capabilities::PresetRecallSpeedEncoding::SonyFr7 => (
            sony_fr7_presets::Fr7SetCommonPresetSpeed, Fr7SetCommonPresetSpeed,
            sony_fr7_presets::Fr7SetCommonPresetSpeed::new(request.speed),
            request.validate_speed(target)
        ),
    }
);

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        command::semantics::{BuiltinCommand, BuiltinRequestContract},
        CameraId, ProfileSpec, Request,
    };

    fn target<P: crate::CompileTimeProfile>() -> TargetRegistration {
        TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<P>().expect("profile"),
        )
    }

    #[test]
    fn recall_speed_uses_each_receiving_register_and_range() {
        let ptz = target::<crate::profiles::PtzOpticsG2>();
        let fr7 = target::<crate::profiles::SonyFr7>();
        let request = SetRecallSpeed::new(types::PresetSpeed::new(5).expect("shared speed"));
        assert_eq!(request.speed().value(), 5);
        assert_eq!(
            request
                .selected_ledger_row_for_target(&ptz)
                .expect("PTZ row"),
            BuiltinCommand::PresetRecallSpeed
        );
        assert_eq!(
            request
                .selected_ledger_row_for_target(&fr7)
                .expect("FR7 row"),
            BuiltinCommand::Fr7SetCommonPresetSpeed
        );
        let mut bytes = [0; SetRecallSpeed::MAX_SIZE];
        assert!(request.write_into(CameraId::CAMERA_1, &mut bytes).is_err());
        let len = ptz.encode(&request, &mut bytes).expect("PTZ frame");
        assert_eq!(&bytes[..len], &[0x81, 0x01, 0x06, 0x01, 5, 0xFF]);
        let len = fr7.encode(&request, &mut bytes).expect("FR7 frame");
        assert_eq!(&bytes[..len], &[0x81, 0x01, 0x7E, 0x04, 0x1C, 0, 5, 0xFF]);
        let fr7_speed = SetRecallSpeed::new(types::PresetSpeed::new(50).expect("FR7 speed"));
        assert!(ptz.encode(&fr7_speed, &mut bytes).is_err());
        let len = fr7.encode(&fr7_speed, &mut bytes).expect("FR7 frame");
        assert_eq!(&bytes[..len], &[0x81, 0x01, 0x7E, 0x04, 0x1C, 3, 2, 0xFF]);
    }

    #[test]
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn recall_speed_retains_each_registers_cache_contract() {
        let request = SetRecallSpeed::new(types::PresetSpeed::MIN);
        let mut bytes = [0; SetRecallSpeed::MAX_SIZE];
        let (ptz, ()) = crate::requests::encode_for_target(
            &request,
            &target::<crate::profiles::PtzOpticsG2>(),
            &mut bytes,
            || Ok(()),
        )
        .expect("PTZ");
        let (fr7, ()) = crate::requests::encode_for_target(
            &request,
            &target::<crate::profiles::SonyFr7>(),
            &mut bytes,
            || Ok(()),
        )
        .expect("FR7");
        assert_eq!(
            ptz.applied_state,
            Some(
                crate::request::builtin_contract::AppliedStateProjection::Set(
                    crate::KnownState::PresetsRecallSpeed(types::PresetSpeed::MIN)
                )
            )
        );
        assert_eq!(fr7.applied_state, None);
    }

    #[test]
    fn duration_constructor_preserves_physical_exact_steps() {
        let preset = types::PresetNumber::MIN;
        let duration = std::time::Duration::from_millis(12_300);
        let request = SetSeparateDuration::new(preset, duration).expect("exact step");
        assert_eq!(request.preset(), preset);
        assert_eq!(request.duration(), duration);
        let mut bytes = [0; SetSeparateDuration::MAX_SIZE];
        let len = request
            .write_into(CameraId::CAMERA_1, &mut bytes)
            .expect("duration frame");
        assert_eq!(
            &bytes[..len],
            &[0x81, 0x01, 0x7E, 0x04, 0x67, 0x00, 0x00, 0x07, 0x0B, 0xFF]
        );
        assert!(
            SetSeparateDuration::new(preset, std::time::Duration::from_millis(12_301)).is_err()
        );
        assert!(SetSeparateDuration::new(preset, std::time::Duration::from_millis(999)).is_err());
        assert!(SetSeparateDuration::new(preset, std::time::Duration::from_secs(100)).is_err());
    }
}
