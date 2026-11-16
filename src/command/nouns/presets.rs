//! Preset memory, recall and configuration requests.

use crate::command::nouns::{forward_inquiry, forward_request};
use crate::{
    command::{inquiry_structs, preset_wire, sony_fr7_presets},
    request::builtin,
    types, Request, TargetRegistration,
};

/// Request recall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recall {
    inner: builtin::PresetRecall,
    preset: types::PresetNumber,
}
impl Recall {
    /// Checks the recall against the target and captures its exact motion axes.
    pub fn for_target(
        target: &TargetRegistration,
        preset: types::PresetNumber,
    ) -> crate::Result<Self> {
        let inner = builtin::PresetRecall::for_profile(preset, target.profile())?;
        let request = Self { inner, preset };
        request.validate_for_target(target)?;
        Ok(request)
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
    encoding: crate::capabilities::PresetRecallSpeedEncoding,
}

impl SetRecallSpeed {
    /// Checks the target's speed domain and selects its documented encoding.
    pub fn for_target(
        target: &TargetRegistration,
        speed: types::PresetSpeed,
    ) -> crate::Result<Self> {
        let request = Self::selected_for_target(target, speed)?;
        request.validate_for_target(target)?;
        Ok(request)
    }

    pub(crate) fn selected_for_target(
        target: &TargetRegistration,
        speed: types::PresetSpeed,
    ) -> crate::Result<Self> {
        let encoding = target.profile().encodings().preset_recall_speed().ok_or(
            crate::Error::FeatureNotSupported {
                feature: "preset recall speed encoding",
            },
        )?;
        Ok(Self { speed, encoding })
    }

    /// Returns the requested recall speed.
    #[must_use]
    pub const fn speed(&self) -> types::PresetSpeed {
        self.speed
    }

    fn validate_encoding(&self, target: &TargetRegistration) -> crate::Result<()> {
        if target.profile().encodings().preset_recall_speed() != Some(self.encoding) {
            return Err(crate::Error::InvalidRequest {
                reason: "preset recall speed was constructed for a different protocol encoding"
                    .into(),
            });
        }
        if matches!(
            self.encoding,
            crate::capabilities::PresetRecallSpeedEncoding::PtzOptics
        ) && self.speed.value() > 24
        {
            return Err(crate::Error::parameter_out_of_range(
                "preset recall speed",
                i32::from(self.speed.value()),
                1,
                24,
            ));
        }
        // The standard inner request validates its profile range. FR7's
        // protocol request carries only the checked union value, so this
        // target-selected wrapper validates that arm's narrower profile range.
        if self.encoding == crate::capabilities::PresetRecallSpeedEncoding::PtzOptics {
            return Ok(());
        }
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
    select |request, target| (request.encoding) default (request.encoding) {
        crate::capabilities::PresetRecallSpeedEncoding::PtzOptics => (
            preset_wire::PresetRecallSpeedCommand, PresetRecallSpeed,
            preset_wire::PresetRecallSpeedCommand::new(request.speed)
        ),
        crate::capabilities::PresetRecallSpeedEncoding::SonyFr7 => (
            sony_fr7_presets::Fr7SetCommonPresetSpeed, Fr7SetCommonPresetSpeed,
            sony_fr7_presets::Fr7SetCommonPresetSpeed::new(request.speed)
        ),
    }
    validate = |request, target| request.validate_encoding(target)
);

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        command::semantics::{BuiltinCommand, BuiltinRequestContract},
        CameraId, ProfileSpec,
    };

    fn target<P: crate::CompileTimeProfile>() -> TargetRegistration {
        TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<P>().expect("profile"),
        )
        .expect("target")
    }

    #[test]
    fn recall_speed_uses_selected_register_and_profile_bounds() {
        let ptz = target::<crate::profiles::PtzOpticsG2>();
        let fr7 = target::<crate::profiles::SonyFr7>();
        let ptz_request =
            SetRecallSpeed::for_target(&ptz, types::PresetSpeed::new(24).expect("24"))
                .expect("PTZ speed");
        let fr7_request =
            SetRecallSpeed::for_target(&fr7, types::PresetSpeed::new(50).expect("50"))
                .expect("FR7 speed");
        assert_eq!(ptz_request.speed().value(), 24);
        assert_eq!(
            ptz_request.selected_ledger_row(),
            BuiltinCommand::PresetRecallSpeed
        );
        assert_eq!(
            fr7_request.selected_ledger_row(),
            BuiltinCommand::Fr7SetCommonPresetSpeed
        );
        let mut bytes = [0; SetRecallSpeed::MAX_SIZE];
        let len = ptz_request
            .write_into(CameraId::CAMERA_1, &mut bytes)
            .expect("PTZ frame");
        assert_eq!(&bytes[..len], &[0x81, 0x01, 0x06, 0x01, 24, 0xFF]);
        let len = fr7_request
            .write_into(CameraId::CAMERA_1, &mut bytes)
            .expect("FR7 frame");
        assert_eq!(
            &bytes[..len],
            &[0x81, 0x01, 0x7E, 0x04, 0x1C, 0x03, 0x02, 0xFF]
        );
        assert!(SetRecallSpeed::for_target(
            &ptz,
            types::PresetSpeed::new(25).expect("union speed")
        )
        .is_err());
        assert!(ptz_request.validate_for_target(&fr7).is_err());
        assert!(fr7_request.validate_for_target(&ptz).is_err());
    }

    #[test]
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn recall_speed_retains_each_registers_cache_contract() {
        let ptz = SetRecallSpeed::for_target(
            &target::<crate::profiles::PtzOpticsG2>(),
            types::PresetSpeed::MIN,
        )
        .expect("PTZ");
        let fr7 = SetRecallSpeed::for_target(
            &target::<crate::profiles::SonyFr7>(),
            types::PresetSpeed::MIN,
        )
        .expect("FR7");
        assert_eq!(
            crate::requests::applied_state_projection(&ptz),
            Some(
                crate::request::builtin_contract::AppliedStateProjection::Set(
                    crate::KnownState::PresetsRecallSpeed(types::PresetSpeed::MIN)
                )
            )
        );
        assert_eq!(crate::requests::applied_state_projection(&fr7), None);
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
