//! Sony FR7 preset speed and duration settings, R7 v4.00 pp. 19 and 22.
//!
//! These settings configure a later preset recall; they do not start motion.
//! Separate speeds are raw bytes, while common speeds and durations use
//! nibble fields. Preset numbers are the zero-based packet values, one less
//! than the number shown in the camera menu.
//! Source: [Sony VISCA Command List v4.00](https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf).

use grafton_visca_macros::ViscaEnum;

use crate::{
    command::{
        bytes::FrameWriter,
        encode::WireEncode,
        response::{Nibbles, Payload},
    },
    CameraId, Error,
};

crate::__grafton_visca_newtype! {
    /// A zero-based FR7 preset packet number, `00..=63` hexadecimal.
    ///
    /// R7 p. 19 encodes the camera menu's preset number minus one.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "0", max = "99", display_prefix = "FR7 Preset Number")]
    pub struct Fr7PresetNumber(u8);
}

crate::__grafton_visca_newtype! {
    /// An FR7 preset recall speed, from 1 through 50 (R7 p. 19).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "1", max = "50", display_prefix = "Preset Speed")]
    pub struct PresetSpeed(u8);
}

crate::__grafton_visca_newtype! {
    /// FR7 preset recall duration in tenths of a second, from 10 through 990.
    ///
    /// R7 pp. 19 and 22 encode one second as `00A` and 99 seconds as `3DE`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[visca_value(min = "10", max = "990", display_prefix = "Preset Duration (tenths)")]
    pub struct PresetDuration(u16);
}

impl PresetDuration {
    /// Creates a duration from 1.0 through 99.0 seconds, rounded to a tenth.
    ///
    /// Exact halfway values round away from zero. Nonfinite and out-of-domain
    /// values are refused before rounding.
    pub fn from_seconds(seconds: f64) -> Result<Self, Error> {
        if !(1.0..=99.0).contains(&seconds) {
            return Err(Error::invalid_parameter(
                "preset_duration_seconds",
                seconds.to_string(),
                "duration must be finite and between 1.0 and 99.0 seconds",
            ));
        }
        Self::new((seconds * 10.0).round() as u16)
    }

    /// Returns the duration in seconds.
    #[must_use]
    pub fn to_seconds(self) -> f64 {
        f64::from(self.value()) / 10.0
    }

    /// Decodes the `00A..=3DE` protocol value, measured in tenths of a second.
    pub const fn from_protocol_value(value: u16) -> Result<Self, Error> {
        Self::new(value)
    }

    /// Returns the `00A..=3DE` protocol value, measured in tenths of a second.
    #[must_use]
    pub const fn to_protocol_value(self) -> u16 {
        self.value()
    }
}

/// Whether FR7 recalls use separate preset settings or one common speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum Fr7PresetSpeedSelection {
    /// Use each preset's selected speed or duration.
    Separate = 0x01,
    /// Use one common speed for all presets.
    Common = 0x02,
}

/// Which separate recall setting an FR7 preset uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum Fr7PresetSeparateMode {
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
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Fr7SetPresetSpeedSelection {
    mode: Fr7PresetSpeedSelection,
}

impl Fr7SetPresetSpeedSelection {
    /// Creates the selection setting.
    pub const fn new(mode: Fr7PresetSpeedSelection) -> Self {
        Self { mode }
    }

    /// Returns the requested selection.
    #[must_use]
    pub const fn mode(self) -> Fr7PresetSpeedSelection {
        self.mode
    }
}

impl WireEncode for Fr7SetPresetSpeedSelection {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x7e, 0x04, 0x1b])
            .byte(u8::from(self.mode))
            .finish()
    }
}

/// Set the FR7's common preset recall speed (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Fr7SetCommonPresetSpeed {
    speed: PresetSpeed,
}

impl Fr7SetCommonPresetSpeed {
    /// Creates the common-speed setting.
    pub const fn new(speed: PresetSpeed) -> Self {
        Self { speed }
    }

    /// Returns the requested speed.
    #[must_use]
    pub const fn speed(self) -> PresetSpeed {
        self.speed
    }
}

impl WireEncode for Fr7SetCommonPresetSpeed {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x7e, 0x04, 0x1c])
            .nibbles::<2>(self.speed.value())
            .finish()
    }
}

/// Select the separate recall mode for one FR7 preset (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Fr7SetSeparatePresetMode {
    preset: Fr7PresetNumber,
    mode: Fr7PresetSeparateMode,
}

impl Fr7SetSeparatePresetMode {
    /// Creates a preset-local mode setting.
    pub const fn new(preset: Fr7PresetNumber, mode: Fr7PresetSeparateMode) -> Self {
        Self { preset, mode }
    }

    /// Returns the zero-based preset packet number.
    #[must_use]
    pub const fn preset(self) -> Fr7PresetNumber {
        self.preset
    }

    /// Returns the requested mode.
    #[must_use]
    pub const fn mode(self) -> Fr7PresetSeparateMode {
        self.mode
    }
}

impl WireEncode for Fr7SetSeparatePresetMode {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x7e, 0x04, 0x27])
            .byte(self.preset.value())
            .byte(u8::from(self.mode))
            .finish()
    }
}

/// Set the separate recall speed for one FR7 preset (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Fr7SetSeparatePresetSpeed {
    preset: Fr7PresetNumber,
    speed: PresetSpeed,
}

impl Fr7SetSeparatePresetSpeed {
    /// Creates a preset-local speed setting.
    pub const fn new(preset: Fr7PresetNumber, speed: PresetSpeed) -> Self {
        Self { preset, speed }
    }

    /// Returns the zero-based preset packet number.
    #[must_use]
    pub const fn preset(self) -> Fr7PresetNumber {
        self.preset
    }

    /// Returns the requested speed.
    #[must_use]
    pub const fn speed(self) -> PresetSpeed {
        self.speed
    }
}

impl WireEncode for Fr7SetSeparatePresetSpeed {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x7e, 0x01, 0x0b])
            .byte(self.preset.value())
            .byte(self.speed.value())
            .finish()
    }
}

/// Set the separate recall duration for one FR7 preset (R7 p. 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct Fr7SetSeparatePresetDuration {
    preset: Fr7PresetNumber,
    duration: PresetDuration,
}

impl Fr7SetSeparatePresetDuration {
    /// Creates a preset-local duration setting.
    pub const fn new(preset: Fr7PresetNumber, duration: PresetDuration) -> Self {
        Self { preset, duration }
    }

    /// Returns the zero-based preset packet number.
    #[must_use]
    pub const fn preset(self) -> Fr7PresetNumber {
        self.preset
    }

    /// Returns the requested duration.
    #[must_use]
    pub const fn duration(self) -> PresetDuration {
        self.duration
    }
}

impl WireEncode for Fr7SetSeparatePresetDuration {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&[0x01, 0x7e, 0x04, 0x67])
            .byte(self.preset.value())
            .nibbles::<3>(self.duration.to_protocol_value())
            .finish()
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
) -> Result<Fr7PresetSpeedSelection, Error> {
    Fr7PresetSpeedSelection::try_from(single_byte(payload)?)
}

pub(crate) fn decode_common_preset_speed(payload: Payload<'_>) -> Result<PresetSpeed, Error> {
    PresetSpeed::new(Nibbles::<2>::try_from(payload)?.zero_extended_u8()?)
}

pub(crate) fn decode_separate_preset_mode(
    payload: Payload<'_>,
) -> Result<Fr7PresetSeparateMode, Error> {
    Fr7PresetSeparateMode::try_from(single_byte(payload)?)
}

pub(crate) fn decode_separate_preset_speed(payload: Payload<'_>) -> Result<PresetSpeed, Error> {
    PresetSpeed::new(single_byte(payload)?)
}

pub(crate) fn decode_separate_preset_duration(
    payload: Payload<'_>,
) -> Result<PresetDuration, Error> {
    let &[high, middle, low] = Nibbles::<3>::try_from(payload)?.as_array();
    // Pad the strict three-nibble view to reuse the canonical nibble combiner.
    let padded = [0, high, middle, low];
    let value = Nibbles::<4>::from_array(&padded)?
        .u16_quad(0)
        .ok_or(Error::InvalidResponseFormat)?;
    PresetDuration::from_protocol_value(value)
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
            &Fr7SetPresetSpeedSelection::new(Fr7PresetSpeedSelection::Separate),
            &[0x81, 1, 0x7e, 4, 0x1b, 1, 0xff],
        );
        literal(
            &Fr7SetPresetSpeedSelection::new(Fr7PresetSpeedSelection::Common),
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
            &Fr7SetSeparatePresetMode::new(Fr7PresetNumber::MIN, Fr7PresetSeparateMode::Speed),
            &[0x81, 1, 0x7e, 4, 0x27, 0, 0, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetMode::new(Fr7PresetNumber::MAX, Fr7PresetSeparateMode::Duration),
            &[0x81, 1, 0x7e, 4, 0x27, 0x63, 1, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetSpeed::new(Fr7PresetNumber::MIN, PresetSpeed::MIN),
            &[0x81, 1, 0x7e, 1, 0x0b, 0, 1, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetSpeed::new(Fr7PresetNumber::MAX, PresetSpeed::MAX),
            &[0x81, 1, 0x7e, 1, 0x0b, 0x63, 0x32, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetDuration::new(Fr7PresetNumber::MIN, PresetDuration::MIN),
            &[0x81, 1, 0x7e, 4, 0x67, 0, 0, 0, 0xa, 0xff],
        );
        literal(
            &Fr7SetSeparatePresetDuration::new(Fr7PresetNumber::MAX, PresetDuration::MAX),
            &[0x81, 1, 0x7e, 4, 0x67, 0x63, 3, 0xd, 0xe, 0xff],
        );
    }

    #[test]
    fn every_wire_value_round_trips_and_outside_values_fail() {
        for raw in 0..=99 {
            assert_eq!(Fr7PresetNumber::new(raw).unwrap().value(), raw);
        }
        assert!(Fr7PresetNumber::new(100).is_err());
        assert!(Fr7PresetNumber::new(u8::MAX).is_err());
        for raw in 1..=50 {
            assert_eq!(PresetSpeed::new(raw).unwrap().value(), raw);
        }
        assert!(PresetSpeed::new(0).is_err());
        assert!(PresetSpeed::new(51).is_err());
        for raw in 10..=990 {
            let duration = PresetDuration::from_protocol_value(raw).unwrap();
            assert_eq!(duration.to_protocol_value(), raw);
            assert_eq!(
                PresetDuration::from_seconds(duration.to_seconds()).unwrap(),
                duration
            );
        }
        assert!(PresetDuration::new(9).is_err());
        assert!(PresetDuration::new(991).is_err());
        assert!(PresetDuration::from_protocol_value(u16::MAX).is_err());
    }

    #[test]
    fn duration_rounding_is_nearest_and_does_not_admit_outside_physical_bounds() {
        for (seconds, tenths) in [
            (1.0, 10),
            (1.04, 10),
            (1.05, 11),
            (1.06, 11),
            (98.95, 990),
            (99.0, 990),
        ] {
            assert_eq!(
                PresetDuration::from_seconds(seconds).unwrap().value(),
                tenths
            );
        }
        for invalid in [0.99, 99.001, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(PresetDuration::from_seconds(invalid).is_err());
        }
    }

    #[test]
    fn closed_mode_tables_have_no_aliases_or_unknown_wire_values() {
        for byte in 0..=u8::MAX {
            let selection = Fr7PresetSpeedSelection::try_from(byte);
            assert_eq!(selection.is_ok(), matches!(byte, 1 | 2));
            if let Ok(mode) = selection {
                assert_eq!(u8::from(mode), byte);
            }
            let separate = Fr7PresetSeparateMode::try_from(byte);
            assert_eq!(separate.is_ok(), matches!(byte, 0 | 1));
            if let Ok(mode) = separate {
                assert_eq!(u8::from(mode), byte);
            }
        }
    }

    #[test]
    fn r7_page_22_decoders_distinguish_raw_speed_from_nibble_speed() {
        assert_eq!(
            decode_preset_speed_selection(Payload::new(&[1])).unwrap(),
            Fr7PresetSpeedSelection::Separate
        );
        assert_eq!(
            decode_preset_speed_selection(Payload::new(&[2])).unwrap(),
            Fr7PresetSpeedSelection::Common
        );
        assert_eq!(
            decode_separate_preset_mode(Payload::new(&[0])).unwrap(),
            Fr7PresetSeparateMode::Speed
        );
        assert_eq!(
            decode_separate_preset_mode(Payload::new(&[1])).unwrap(),
            Fr7PresetSeparateMode::Duration
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
            PresetDuration::MIN
        );
        assert_eq!(
            decode_separate_preset_duration(Payload::new(&[3, 0xd, 0xe])).unwrap(),
            PresetDuration::MAX
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
        let setting = Fr7SetSeparatePresetDuration::new(Fr7PresetNumber::MAX, PresetDuration::MAX);
        let json = serde_json::to_string(&setting).unwrap();
        assert_eq!(
            serde_json::from_str::<Fr7SetSeparatePresetDuration>(&json).unwrap(),
            setting
        );
        for invalid in [
            r#"{"preset":100,"duration":10}"#,
            r#"{"preset":0,"duration":9}"#,
            r#"{"preset":0,"duration":991}"#,
            r#"{"preset":0,"duration":10,"extra":true}"#,
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
