//! Sony FR7 preset speed and duration settings, R7 v4.00 pp. 19 and 22.
//!
//! These settings configure a later preset recall; they do not start motion.
//! Separate speeds are raw bytes, while common speeds and durations use
//! nibble fields. Preset numbers are the zero-based packet values, one less
//! than the number shown in the camera menu.
//! Source: [Sony VISCA Command List v4.00](https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf).

use crate::wire::WireValue;
use std::time::Duration;

use crate::{
    command::preset_wire::PresetNumber,
    command::{
        bytes::{constants::sony_fr7_presets, FrameWriter},
        encode::WireEncode,
        response::{Nibbles, Payload},
    },
    CameraId, Error,
};

/// An FR7 preset recall speed, from 1 through 50 (R7 p. 19).
#[crate::value(min = 1, max = 50, display = "Preset Speed {}")]
pub struct PresetSpeed(u8);

/// FR7 preset recall duration in tenths of a second, from 10 through 990.
///
/// R7 pp. 19 and 22 encode one second as `00A` and 99 seconds as `3DE`.
#[crate::value(min = 10, max = 990, display = "Preset Duration (tenths) {}")]
pub(crate) struct PresetDuration(u16);

impl PresetDuration {
    /// Checks the exact 100 ms duration domain carried by the protocol.
    fn from_duration(duration: Duration) -> Result<Self, Error> {
        if !(Duration::from_secs(1)..=Duration::from_secs(99)).contains(&duration)
            || !duration.subsec_nanos().is_multiple_of(100_000_000)
        {
            return Err(Error::invalid_parameter(
                "preset_duration",
                format!("{duration:?}"),
                "duration must be between 1 and 99 seconds in exact 100 ms steps",
            ));
        }
        let tenths = u16::try_from(duration.as_millis() / 100).map_err(|_| {
            Error::invalid_parameter(
                "preset_duration",
                format!("{duration:?}"),
                "duration exceeds the protocol domain",
            )
        })?;
        Self::new(tenths)
    }

    /// Returns the exact physical duration represented by the wire ticks.
    const fn duration(self) -> Duration {
        Duration::from_millis(self.value() as u64 * 100)
    }
}

/// Whether FR7 recalls use separate preset settings or one common speed.
#[crate::value_enum]
pub enum PresetSpeedSelection {
    /// Use each preset's selected speed or duration.
    Separate = 0x01,
    /// Use one common speed for all presets.
    Common = 0x02,
}

/// Which separate recall setting an FR7 preset uses.
#[crate::value_enum]
pub enum PresetSeparateMode {
    /// Use the preset's separate speed.
    Speed = 0x00,
    /// Use the preset's specified duration.
    Duration = 0x01,
}

/// Select separate preset settings or a common recall speed (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Fr7SetPresetSpeedSelection {
    mode: PresetSpeedSelection,
}

impl Fr7SetPresetSpeedSelection {
    /// Creates the selection setting.
    pub const fn new(mode: PresetSpeedSelection) -> Self {
        Self { mode }
    }
}

impl WireEncode for Fr7SetPresetSpeedSelection {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&sony_fr7_presets::PRESET_SPEED_SELECTION)
            .byte(self.mode.to_wire())
            .finish()
    }
}

/// Set the FR7's common preset recall speed (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Fr7SetCommonPresetSpeed {
    speed: PresetSpeed,
}

impl Fr7SetCommonPresetSpeed {
    /// Creates the common-speed setting.
    pub const fn new(speed: PresetSpeed) -> Self {
        Self { speed }
    }
}

impl WireEncode for Fr7SetCommonPresetSpeed {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&sony_fr7_presets::COMMON_PRESET_SPEED)
            .nibbles::<2>(self.speed.value())
            .finish()
    }
}

/// Select the separate recall mode for one FR7 preset (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Fr7SetSeparatePresetMode {
    preset: PresetNumber,
    mode: PresetSeparateMode,
}

impl Fr7SetSeparatePresetMode {
    /// Creates a preset-local mode setting.
    pub const fn new(preset: PresetNumber, mode: PresetSeparateMode) -> Self {
        Self { preset, mode }
    }

    /// Returns the zero-based preset packet number.
    #[must_use]
    pub const fn preset(self) -> PresetNumber {
        self.preset
    }
}

impl WireEncode for Fr7SetSeparatePresetMode {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&sony_fr7_presets::SEPARATE_PRESET_MODE)
            .byte(self.preset.value())
            .byte(self.mode.to_wire())
            .finish()
    }
}

/// Set the separate recall speed for one FR7 preset (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Fr7SetSeparatePresetSpeed {
    preset: PresetNumber,
    speed: PresetSpeed,
}

impl Fr7SetSeparatePresetSpeed {
    /// Creates a preset-local speed setting.
    pub const fn new(preset: PresetNumber, speed: PresetSpeed) -> Self {
        Self { preset, speed }
    }

    /// Returns the zero-based preset packet number.
    #[must_use]
    pub const fn preset(self) -> PresetNumber {
        self.preset
    }
}

impl WireEncode for Fr7SetSeparatePresetSpeed {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&sony_fr7_presets::SEPARATE_PRESET_SPEED)
            .byte(self.preset.value())
            .byte(self.speed.value())
            .finish()
    }
}

/// Set the separate recall duration for one FR7 preset (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Fr7SetSeparatePresetDuration {
    preset: PresetNumber,
    duration: Duration,
}

impl Fr7SetSeparatePresetDuration {
    /// Creates a preset-local duration setting.
    pub fn new(preset: PresetNumber, duration: Duration) -> Result<Self, Error> {
        PresetDuration::from_duration(duration)?;
        Ok(Self { preset, duration })
    }

    /// Returns the zero-based preset packet number.
    #[must_use]
    pub const fn preset(self) -> PresetNumber {
        self.preset
    }

    /// Returns the requested duration.
    #[must_use]
    #[cfg(test)]
    pub const fn duration(self) -> Duration {
        self.duration
    }
}

impl WireEncode for Fr7SetSeparatePresetDuration {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&sony_fr7_presets::SEPARATE_PRESET_DURATION)
            .byte(self.preset.value())
            .nibbles::<3>(PresetDuration::from_duration(self.duration)?.to_wire())
            .finish()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Fr7SetSeparatePresetDuration {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Setting {
            preset: PresetNumber,
            duration: Duration,
        }
        let setting = Setting::deserialize(deserializer)?;
        Self::new(setting.preset, setting.duration).map_err(serde::de::Error::custom)
    }
}

fn single_byte(payload: Payload<'_>) -> Result<u8, Error> {
    match payload.as_slice() {
        [value] => Ok(*value),
        bytes => Err(Error::invalid_response_length(1, bytes)),
    }
}

pub(crate) fn decode_preset_speed_selection(
    payload: Payload<'_>,
) -> Result<PresetSpeedSelection, Error> {
    PresetSpeedSelection::from_wire(single_byte(payload)?)
}

pub(crate) fn decode_common_preset_speed(payload: Payload<'_>) -> Result<PresetSpeed, Error> {
    PresetSpeed::new(Nibbles::<2>::try_from(payload)?.zero_extended_u8()?)
}

pub(crate) fn decode_separate_preset_mode(
    payload: Payload<'_>,
) -> Result<PresetSeparateMode, Error> {
    PresetSeparateMode::from_wire(single_byte(payload)?)
}

pub(crate) fn decode_separate_preset_speed(payload: Payload<'_>) -> Result<PresetSpeed, Error> {
    PresetSpeed::new(single_byte(payload)?)
}

pub(crate) fn decode_separate_preset_duration(payload: Payload<'_>) -> Result<Duration, Error> {
    let &[high, middle, low] = Nibbles::<3>::try_from(payload)?.as_array();
    // Pad the strict three-nibble view to reuse the canonical nibble combiner.
    let padded = [0, high, middle, low];
    let value = Nibbles::<4>::from_array(&padded)?
        .u16_quad(0)
        .ok_or(Error::InvalidResponseFormat)?;
    PresetDuration::from_wire(value).map(PresetDuration::duration)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn literal(command: &impl WireEncode, expected: &[u8]) {
        let mut frame = [0; 16];
        let written = command.write_into(CameraId::CAMERA_1, &mut frame).unwrap();
        assert_eq!(&frame[..written], expected);
        for length in 0..expected.len() {
            let error = command
                .write_into(CameraId::CAMERA_1, &mut frame[..length])
                .expect_err("every shorter buffer fails");
            assert!(matches!(error, Error::BufferTooSmall { required, actual }
                if required == expected.len() && actual == length));
        }
        let written = command.write_into(CameraId::CAMERA_7, &mut frame).unwrap();
        assert_eq!(frame[0], 0x87);
        assert_eq!(&frame[1..written], &expected[1..]);
    }

    #[test]
    fn r7_page_19_literals_preserve_raw_and_nibble_field_widths() {
        literal(
            &Fr7SetPresetSpeedSelection::new(PresetSpeedSelection::Separate),
            &[0x81, 1, 0x7e, 4, 0x1b, 1, 0xff],
        );
        literal(
            &Fr7SetPresetSpeedSelection::new(PresetSpeedSelection::Common),
            &[0x81, 1, 0x7e, 4, 0x1b, 2, 0xff],
        );
        literal(
            &Fr7SetCommonPresetSpeed::new(PresetSpeed::MIN),
            &[0x81, 1, 0x7e, 4, 0x1c, 0, 1, 0xff],
        );
        literal(
            &Fr7SetCommonPresetSpeed::new(PresetSpeed::MAX),
            &[0x81, 1, 0x7e, 4, 0x1c, 3, 2, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetMode::new(PresetNumber::MIN, PresetSeparateMode::Speed),
            &[0x81, 1, 0x7e, 4, 0x27, 0, 0, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetMode::new(
                PresetNumber::new(99).unwrap(),
                PresetSeparateMode::Duration,
            ),
            &[0x81, 1, 0x7e, 4, 0x27, 0x63, 1, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetSpeed::new(PresetNumber::MIN, PresetSpeed::MIN),
            &[0x81, 1, 0x7e, 1, 0x0b, 0, 1, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetSpeed::new(PresetNumber::new(99).unwrap(), PresetSpeed::MAX),
            &[0x81, 1, 0x7e, 1, 0x0b, 0x63, 0x32, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetDuration::new(PresetNumber::MIN, Duration::from_secs(1)).unwrap(),
            &[0x81, 1, 0x7e, 4, 0x67, 0, 0, 0, 0xa, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetDuration::new(
                PresetNumber::new(99).unwrap(),
                Duration::from_secs(99),
            )
            .unwrap(),
            &[0x81, 1, 0x7e, 4, 0x67, 0x63, 3, 0xd, 0xe, 0xff],
        );
    }

    #[test]
    fn every_wire_value_round_trips_and_outside_values_fail() {
        for raw in 0..=99 {
            assert_eq!(PresetNumber::new(raw).unwrap().value(), raw);
        }
        assert!(PresetNumber::new(100).is_ok());
        assert!(PresetNumber::new(u8::MAX).is_ok());
        for raw in 1..=50 {
            assert_eq!(PresetSpeed::new(raw).unwrap().value(), raw);
        }
        assert!(PresetSpeed::new(0).is_err());
        assert!(PresetSpeed::new(51).is_err());
        for raw in 10..=990 {
            let duration = PresetDuration::from_wire(raw).unwrap();
            assert_eq!(duration.to_wire(), raw);
            assert_eq!(
                PresetDuration::from_duration(duration.duration()).unwrap(),
                duration
            );
        }
        assert!(PresetDuration::new(9).is_err());
        assert!(PresetDuration::new(991).is_err());
        assert!(PresetDuration::from_wire(u16::MAX).is_err());
    }

    #[test]
    fn public_duration_requires_exact_protocol_steps() {
        for tenths in 10..=990 {
            let duration = Duration::from_millis(tenths * 100);
            let request = Fr7SetSeparatePresetDuration::new(PresetNumber::MIN, duration).unwrap();
            assert_eq!(request.duration(), duration);
        }
        for invalid in [
            Duration::ZERO,
            Duration::from_millis(999),
            Duration::from_millis(1001),
            Duration::from_millis(99_001),
            Duration::MAX,
        ] {
            assert!(Fr7SetSeparatePresetDuration::new(PresetNumber::MIN, invalid).is_err());
        }
    }

    #[test]
    fn closed_mode_tables_have_no_aliases_or_unknown_wire_values() {
        for byte in 0..=u8::MAX {
            let selection = PresetSpeedSelection::from_wire(byte);
            assert_eq!(selection.is_ok(), matches!(byte, 1 | 2));
            if let Ok(mode) = selection {
                assert_eq!(mode.to_wire(), byte);
            }
            let separate = PresetSeparateMode::from_wire(byte);
            assert_eq!(separate.is_ok(), matches!(byte, 0 | 1));
            if let Ok(mode) = separate {
                assert_eq!(mode.to_wire(), byte);
            }
        }
    }

    #[test]
    fn r7_page_22_decoders_distinguish_raw_speed_from_nibble_speed() {
        assert_eq!(
            decode_preset_speed_selection(Payload::new(&[1])).unwrap(),
            PresetSpeedSelection::Separate
        );
        assert_eq!(
            decode_preset_speed_selection(Payload::new(&[2])).unwrap(),
            PresetSpeedSelection::Common
        );
        assert_eq!(
            decode_separate_preset_mode(Payload::new(&[0])).unwrap(),
            PresetSeparateMode::Speed
        );
        assert_eq!(
            decode_separate_preset_mode(Payload::new(&[1])).unwrap(),
            PresetSeparateMode::Duration
        );
        assert_eq!(
            decode_common_preset_speed(Payload::new(&[0, 1])).unwrap(),
            PresetSpeed::MIN
        );
        assert_eq!(
            decode_common_preset_speed(Payload::new(&[3, 2])).unwrap(),
            PresetSpeed::MAX
        );
        assert_eq!(
            decode_separate_preset_speed(Payload::new(&[1])).unwrap(),
            PresetSpeed::MIN
        );
        assert_eq!(
            decode_separate_preset_speed(Payload::new(&[0x32])).unwrap(),
            PresetSpeed::MAX
        );
        assert_eq!(
            decode_separate_preset_duration(Payload::new(&[0, 0, 0xa])).unwrap(),
            Duration::from_secs(1)
        );
        assert_eq!(
            decode_separate_preset_duration(Payload::new(&[3, 0xd, 0xe])).unwrap(),
            Duration::from_secs(99)
        );
        for invalid in [&[][..], &[1, 0][..]] {
            assert!(decode_preset_speed_selection(Payload::new(invalid)).is_err());
            assert!(decode_separate_preset_mode(Payload::new(invalid)).is_err());
            assert!(decode_separate_preset_speed(Payload::new(invalid)).is_err());
        }
        for invalid in [
            &[0x32][..],
            &[3, 2, 0][..],
            &[0x10, 0][..],
            &[0, 0][..],
            &[3, 3][..],
        ] {
            assert!(decode_common_preset_speed(Payload::new(invalid)).is_err());
        }
        assert!(decode_preset_speed_selection(Payload::new(&[0])).is_err());
        assert!(decode_separate_preset_mode(Payload::new(&[2])).is_err());
        assert!(decode_separate_preset_speed(Payload::new(&[0])).is_err());
        assert!(decode_separate_preset_speed(Payload::new(&[0x33])).is_err());
        for invalid in [
            &[0, 0xa][..],
            &[0, 0, 0, 0xa][..],
            &[0, 0, 0x10][..],
            &[0, 0, 9][..],
            &[3, 0xd, 0xf][..],
        ] {
            assert!(decode_separate_preset_duration(Payload::new(invalid)).is_err());
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_preserves_checked_domains_inside_setting_records() {
        let setting = Fr7SetSeparatePresetDuration::new(
            PresetNumber::new(99).unwrap(),
            Duration::from_secs(99),
        )
        .unwrap();
        let json = serde_json::to_string(&setting).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap()["duration"],
            serde_json::json!({"secs": 99, "nanos": 0})
        );
        assert_eq!(
            serde_json::from_str::<Fr7SetSeparatePresetDuration>(&json).unwrap(),
            setting
        );
        for invalid in [
            r#"{"preset":0,"duration":{"secs":0,"nanos":900000000}}"#,
            r#"{"preset":0,"duration":{"secs":99,"nanos":100000000}}"#,
            r#"{"preset":0,"duration":{"secs":1,"nanos":1}}"#,
            r#"{"preset":0,"duration":{"secs":1,"nanos":0},"extra":true}"#,
        ] {
            assert!(serde_json::from_str::<Fr7SetSeparatePresetDuration>(invalid).is_err());
        }
        assert!(
            serde_json::from_str::<Fr7SetSeparatePresetSpeed>(r#"{"preset":0,"speed":0}"#).is_err()
        );
        assert!(serde_json::from_str::<Fr7SetCommonPresetSpeed>(r#"{"speed":51}"#).is_err());
        assert!(
            serde_json::from_str::<Fr7SetPresetSpeedSelection>(r#"{"mode":"unknown"}"#).is_err()
        );
        assert!(serde_json::from_str::<Fr7SetSeparatePresetMode>(
            r#"{"preset":0,"mode":"unknown"}"#
        )
        .is_err());
    }
}
