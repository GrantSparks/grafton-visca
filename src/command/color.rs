//! Color control commands for VISCA cameras.
//!
//! This module provides commands for controlling color-related settings
//! including white balance tuning, saturation, and hue adjustments.

use crate::{
    command::{bytes::ConstCommandBuilder, encode::WireEncode},
    error::Error,
    types::{BlueTuning, HueLevel, RedTuning, SaturationLevel},
    visca_command,
};

visca_command! {
    /// One-Push White Balance Trigger command.
    ///
    /// Performs a one-time automatic white balance adjustment based on
    /// the current scene. The camera will analyze the image and set the
    /// white balance to achieve neutral colors.
    pub struct OnePushTriggerCommand;
    bytes = [0x01, 0x04, 0x10, 0x05];
}

impl Default for OnePushTriggerCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl OnePushTriggerCommand {
    /// Create a new one-push trigger command.
    pub fn new() -> Self {
        OnePushTriggerCommand
    }
}

visca_command! {
    /// Red Channel Tuning command.
    ///
    /// Fine-tunes the red channel gain for white balance adjustment.
    /// This is typically used after setting a base white balance mode
    /// to make small corrections.
    pub struct RedTuningCommand { level: RedTuning };
    prefix = [0x01, 0x04, 0x43, 0x00, 0x00];
    param = {
        // Convert -10..+10 to 0x00..0x14 (0x00 = -10, 0x0A = 0, 0x14 = +10)
        // RedTuning is already validated at construction via visca_range_type!,
        // so level_offset is guaranteed to be in 0..=20.
        #[allow(clippy::cast_sign_loss)]
        let encoded = (level.value() + 10) as u8;
        [0x00, encoded]
    };
    max_param_size = 2;
}

impl RedTuningCommand {
    /// Create a new red tuning command.
    pub fn new(level: RedTuning) -> Self {
        Self { level }
    }
}

visca_command! {
    /// Blue Channel Tuning command.
    ///
    /// Fine-tunes the blue channel gain for white balance adjustment.
    /// This is typically used after setting a base white balance mode
    /// to make small corrections.
    pub struct BlueTuningCommand { level: BlueTuning };
    prefix = [0x01, 0x04, 0x44, 0x00, 0x00];
    param = {
        // Convert -10..+10 to 0x00..0x14 (0x00 = -10, 0x0A = 0, 0x14 = +10)
        // BlueTuning is already validated at construction via visca_range_type!,
        // so level_offset is guaranteed to be in 0..=20.
        #[allow(clippy::cast_sign_loss)]
        let encoded = (level.value() + 10) as u8;
        [0x00, encoded]
    };
    max_param_size = 2;
}

impl BlueTuningCommand {
    /// Create a new blue tuning command.
    pub fn new(level: BlueTuning) -> Self {
        Self { level }
    }
}

visca_command! {
    /// Saturation control command.
    ///
    /// Adjusts the color saturation level of the image.
    /// Lower values produce more muted colors, while higher values
    /// produce more vivid colors.
    pub struct SaturationCommand {
        level: SaturationLevel
    };
    prefix = [0x01, 0x04, 0x49, 0x00, 0x00, 0x00];
    param = level.value();
    max_param_size = 1;
}

impl SaturationCommand {
    /// Create a new saturation command.
    pub fn new(level: SaturationLevel) -> Self {
        Self { level }
    }
}

visca_command! {
    /// Hue adjustment command.
    ///
    /// Adjusts the hue (color phase) of the image, shifting all colors
    /// around the color wheel. This can be used to correct color casts
    /// or create artistic effects.
    pub struct HueCommand {
        level: HueLevel
    };
    prefix = [0x01, 0x04, 0x4F, 0x00, 0x00, 0x00];
    param = level.value();
    max_param_size = 1;
}

impl HueCommand {
    /// Create a new hue command.
    pub fn new(level: HueLevel) -> Self {
        Self { level }
    }
}

/// Color Temperature command.
///
/// Controls the color temperature setting when white balance is in color temperature mode.
/// Color temperature is measured in Kelvin (K) and affects the warmth/coolness of the image.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum ColorTemperature {
    /// Reset color temperature to default value.
    Reset,
    /// Increase color temperature (makes image cooler/bluer).
    Up,
    /// Decrease color temperature (makes image warmer/redder).
    Down,
    /// Set color temperature directly.
    ///
    /// Lower values produce warmer (more orange/red) colors,
    /// higher values produce cooler (more blue) colors.
    SetTemperature(crate::types::ColorTemp),
}

impl WireEncode for ColorTemperature {
    const MAX_SIZE: usize = 8;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        match self {
            ColorTemperature::Reset => {
                let builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::TEMPERATURE_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(0x00);
                builder.terminate().build_into(buffer)
            }
            ColorTemperature::Up => {
                let builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::TEMPERATURE_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(0x02);
                builder.terminate().build_into(buffer)
            }
            ColorTemperature::Down => {
                let builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::TEMPERATURE_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(0x03);
                builder.terminate().build_into(buffer)
            }
            ColorTemperature::SetTemperature(temp) => {
                let builder = ConstCommandBuilder::<7>::from_prefix(
                    crate::command::bytes::constants::color::TEMPERATURE_PREFIX,
                )
                .with_camera_id(camera_id)
                .push_nibble_pair(temp.value());
                builder.terminate().build_into(buffer)
            }
        }
    }
}

/// Red Channel Direct command (different from tuning).
///
/// Controls the red channel gain in manual white balance mode.
/// This provides direct control over the red color channel intensity.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum RedGain {
    /// Reset red gain to default value.
    Reset,
    /// Increment red gain value by one step.
    Up,
    /// Decrement red gain value by one step.
    Down,
    /// Set red gain to a specific value.
    ///
    /// Higher values increase the intensity of red in the image.
    SetValue(crate::types::RedChannel),
}

impl WireEncode for RedGain {
    const MAX_SIZE: usize = 9;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        match self {
            RedGain::Reset => {
                let mut builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::RED_GAIN_CONTROL_PREFIX,
                );
                builder = builder.with_camera_id(camera_id).push(0x00);
                builder.terminate().build_into(buffer)
            }
            RedGain::Up => {
                let mut builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::RED_GAIN_CONTROL_PREFIX,
                );
                builder = builder.with_camera_id(camera_id).push(0x02);
                builder.terminate().build_into(buffer)
            }
            RedGain::Down => {
                let mut builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::RED_GAIN_CONTROL_PREFIX,
                );
                builder = builder.with_camera_id(camera_id).push(0x03);
                builder.terminate().build_into(buffer)
            }
            RedGain::SetValue(value) => {
                // Note: different command byte 0x43 for direct setting
                let builder = ConstCommandBuilder::<9>::from_prefix(
                    crate::command::bytes::constants::color::RED_GAIN_DIRECT_PREFIX,
                )
                .with_camera_id(camera_id)
                .push_nibble_pair(u16::from(value.value()));
                builder.terminate().build_into(buffer)
            }
        }
    }
}

/// Blue Channel Direct command (different from tuning).
///
/// Controls the blue channel gain in manual white balance mode.
/// This provides direct control over the blue color channel intensity.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum BlueGain {
    /// Reset blue gain to default value.
    Reset,
    /// Increment blue gain value by one step.
    Up,
    /// Decrement blue gain value by one step.
    Down,
    /// Set blue gain to a specific value.
    ///
    /// Higher values increase the intensity of blue in the image.
    SetValue(crate::types::BlueChannel),
}

impl WireEncode for BlueGain {
    const MAX_SIZE: usize = 9;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        match self {
            BlueGain::Reset => {
                let builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::BLUE_GAIN_CONTROL_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(0x00);
                builder.terminate().build_into(buffer)
            }
            BlueGain::Up => {
                let builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::BLUE_GAIN_CONTROL_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(0x02);
                builder.terminate().build_into(buffer)
            }
            BlueGain::Down => {
                let builder = ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::color::BLUE_GAIN_CONTROL_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(0x03);
                builder.terminate().build_into(buffer)
            }
            BlueGain::SetValue(value) => {
                // Note: different command byte 0x44 for direct setting
                let builder = ConstCommandBuilder::<9>::from_prefix(
                    crate::command::bytes::constants::color::BLUE_GAIN_DIRECT_PREFIX,
                )
                .with_camera_id(camera_id)
                .push_nibble_pair(u16::from(value.value()));
                builder.terminate().build_into(buffer)
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::cast_sign_loss,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use crate::{command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test};

    visca_test!(
        OnePushTriggerCommand,
        test_one_push_trigger_command,
        OnePushTriggerCommand::new(),
        &[0x81, 0x01, 0x04, 0x10, 0x05, VISCA_TERMINATOR]
    );

    #[test]
    fn test_red_tuning_command() {
        // Test valid range
        for level in -10..=10 {
            let tuning = RedTuning::new(level).unwrap();
            let cmd = RedTuningCommand::new(tuning);
            let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                .unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..6], [0x81, 0x01, 0x04, 0x43, 0x00, 0x00]);
            assert_eq!(bytes[6], 0x00);
            assert_eq!(bytes[7], (level + 10) as u8);
            assert_eq!(bytes[8], 0xFF);
        }

        // Test invalid values - can't create invalid RedTuning
        assert!(RedTuning::new(-11).is_err());
        assert!(RedTuning::new(11).is_err());
    }

    #[test]
    fn test_blue_tuning_command() {
        // Test valid range
        for level in -10..=10 {
            let tuning = BlueTuning::new(level).unwrap();
            let cmd = BlueTuningCommand::new(tuning);
            let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                .unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..6], [0x81, 0x01, 0x04, 0x44, 0x00, 0x00]);
            assert_eq!(bytes[6], 0x00);
            assert_eq!(bytes[7], (level + 10) as u8);
            assert_eq!(bytes[8], 0xFF);
        }

        // Test invalid values - can't create invalid BlueTuning
        assert!(BlueTuning::new(-11).is_err());
        assert!(BlueTuning::new(11).is_err());
    }

    #[test]
    fn test_saturation_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let sat_level = SaturationLevel::new(level).unwrap();
            let cmd = SaturationCommand::new(sat_level);
            let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                .unwrap();
            assert_eq!(
                bytes,
                vec![
                    0x81,
                    0x01,
                    0x04,
                    0x49,
                    0x00,
                    0x00,
                    0x00,
                    level,
                    VISCA_TERMINATOR
                ]
            );
        }

        // Test invalid value - can't create invalid SaturationLevel
        assert!(SaturationLevel::new(0x0F).is_err());
    }

    #[test]
    fn test_hue_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let hue_level = HueLevel::new(level).unwrap();
            let cmd = HueCommand::new(hue_level);
            let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                .unwrap();
            assert_eq!(
                bytes,
                vec![
                    0x81,
                    0x01,
                    0x04,
                    0x4F,
                    0x00,
                    0x00,
                    0x00,
                    level,
                    VISCA_TERMINATOR
                ]
            );
        }

        // Test invalid value - can't create invalid HueLevel
        assert!(HueLevel::new(0x0F).is_err());
    }

    #[test]
    fn test_color_temperature_command() {
        // Test Reset
        let cmd = ColorTemperature::Reset;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x00, VISCA_TERMINATOR]
        );

        // Test Up
        let cmd = ColorTemperature::Up;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x02, VISCA_TERMINATOR]
        );

        // Test Down
        let cmd = ColorTemperature::Down;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x03, VISCA_TERMINATOR]
        );

        // Test Direct with valid values
        let test_values = vec![0x00, 0x10, 0x20, 0x37];
        for temp in test_values {
            let color_temp = crate::types::ColorTemp::new(temp).unwrap();
            let cmd = ColorTemperature::SetTemperature(color_temp);
            let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                .unwrap();
            assert_eq!(bytes.len(), 7);
            assert_eq!(bytes[0..4], [0x81, 0x01, 0x04, 0x20]);
            assert_eq!(bytes[4], ((temp >> 4) & 0x0F) as u8);
            assert_eq!(bytes[5], (temp & 0x0F) as u8);
            assert_eq!(bytes[6], 0xFF);
        }

        // Test Direct with invalid value - can't create invalid ColorTemperature
        assert!(crate::types::ColorTemp::new(0x38).is_err());
    }

    #[test]
    fn test_red_gain_command() {
        // Test Reset
        let cmd = RedGain::Reset;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x00, VISCA_TERMINATOR]
        );

        // Test Up
        let cmd = RedGain::Up;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x02, VISCA_TERMINATOR]
        );

        // Test Down
        let cmd = RedGain::Down;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x03, VISCA_TERMINATOR]
        );

        // Test Direct with various values
        let test_values = vec![0x00, 0x55, 0xAA, VISCA_TERMINATOR];
        for gain in test_values {
            let red_gain = crate::types::RedChannel::new(gain).unwrap();
            let cmd = RedGain::SetValue(red_gain);
            let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                .unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..5], [0x81, 0x01, 0x04, 0x43, 0x00]);
            assert_eq!(bytes[5], 0x00);
            assert_eq!(bytes[6], (gain >> 4) & 0x0F);
            assert_eq!(bytes[7], gain & 0x0F);
            assert_eq!(bytes[8], 0xFF);
        }
    }

    #[test]
    fn test_blue_gain_command() {
        // Test Reset
        let cmd = BlueGain::Reset;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x00, VISCA_TERMINATOR]
        );

        // Test Up
        let cmd = BlueGain::Up;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x02, VISCA_TERMINATOR]
        );

        // Test Down
        let cmd = BlueGain::Down;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x03, VISCA_TERMINATOR]
        );

        // Test Direct with various values
        let test_values = vec![0x00, 0x55, 0xAA, VISCA_TERMINATOR];
        for gain in test_values {
            let blue_gain = crate::types::BlueChannel::new(gain).unwrap();
            let cmd = BlueGain::SetValue(blue_gain);
            let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
                .unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..5], [0x81, 0x01, 0x04, 0x44, 0x00]);
            assert_eq!(bytes[5], 0x00);
            assert_eq!(bytes[6], (gain >> 4) & 0x0F);
            assert_eq!(bytes[7], gain & 0x0F);
            assert_eq!(bytes[8], 0xFF);
        }
    }

    #[test]
    fn test_command_traits() {
        // Test that all commands implement Debug and Clone
        let cmds: Vec<Box<dyn std::fmt::Debug>> = vec![
            Box::new(OnePushTriggerCommand::new()),
            Box::new(RedTuningCommand::new(RedTuning::NEUTRAL)),
            Box::new(BlueTuningCommand::new(BlueTuning::NEUTRAL)),
            Box::new(SaturationCommand::new(SaturationLevel::MIN)),
            Box::new(HueCommand::new(HueLevel::MIN)),
            Box::new(ColorTemperature::Reset),
            Box::new(RedGain::Reset),
            Box::new(BlueGain::Reset),
        ];

        for cmd in cmds {
            let _ = format!("{:?}", cmd);
        }

        // Test Clone
        let red_cmd1 = RedTuningCommand::new(RedTuning::new(5).unwrap());
        let red_cmd2 = red_cmd1;
        assert_eq!(
            crate::command::test_wire_bytes(&red_cmd1, crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            crate::command::test_wire_bytes(&red_cmd2, crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary values for tuning commands
        let red_min = RedTuningCommand::new(RedTuning::new(-10).unwrap());
        assert!(
            crate::command::test_wire_bytes(&red_min, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );
        assert_eq!(
            crate::command::test_wire_bytes(&red_min, crate::camera_id::CameraId::CAMERA_1)
                .unwrap()[7],
            0x00
        );

        let red_max = RedTuningCommand::new(RedTuning::MAX);
        assert!(
            crate::command::test_wire_bytes(&red_max, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );
        assert_eq!(
            crate::command::test_wire_bytes(&red_max, crate::camera_id::CameraId::CAMERA_1)
                .unwrap()[7],
            0x14
        );

        let blue_min = BlueTuningCommand::new(BlueTuning::new(-10).unwrap());
        assert!(
            crate::command::test_wire_bytes(&blue_min, crate::camera_id::CameraId::CAMERA_1)
                .is_ok()
        );
        assert_eq!(
            crate::command::test_wire_bytes(&blue_min, crate::camera_id::CameraId::CAMERA_1)
                .unwrap()[7],
            0x00
        );

        let blue_max = BlueTuningCommand::new(BlueTuning::MAX);
        assert!(
            crate::command::test_wire_bytes(&blue_max, crate::camera_id::CameraId::CAMERA_1)
                .is_ok()
        );
        assert_eq!(
            crate::command::test_wire_bytes(&blue_max, crate::camera_id::CameraId::CAMERA_1)
                .unwrap()[7],
            0x14
        );

        // Test boundary values for saturation and hue
        let sat_min = SaturationCommand::new(SaturationLevel::new(0x00).unwrap());
        assert!(
            crate::command::test_wire_bytes(&sat_min, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        let sat_max = SaturationCommand::new(SaturationLevel::new(0x0E).unwrap());
        assert!(
            crate::command::test_wire_bytes(&sat_max, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        let hue_min = HueCommand::new(HueLevel::new(0x00).unwrap());
        assert!(
            crate::command::test_wire_bytes(&hue_min, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        let hue_max = HueCommand::new(HueLevel::new(0x0E).unwrap());
        assert!(
            crate::command::test_wire_bytes(&hue_max, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        // Test boundary value for color temperature
        let temp_min =
            ColorTemperature::SetTemperature(crate::types::ColorTemp::new(0x00).unwrap());
        assert!(
            crate::command::test_wire_bytes(&temp_min, crate::camera_id::CameraId::CAMERA_1)
                .is_ok()
        );

        let temp_max =
            ColorTemperature::SetTemperature(crate::types::ColorTemp::new(0x37).unwrap());
        assert!(
            crate::command::test_wire_bytes(&temp_max, crate::camera_id::CameraId::CAMERA_1)
                .is_ok()
        );
    }

    #[test]
    fn test_nibble_encoding() {
        // Test that Direct commands properly encode values as nibbles
        let cmd = ColorTemperature::SetTemperature(crate::types::ColorTemp::new(0x25).unwrap());
        let bytes =
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap();
        assert_eq!(bytes[4], 0x02);
        assert_eq!(bytes[5], 0x05);

        let cmd = RedGain::SetValue(crate::types::RedChannel::new(0xAB).unwrap());
        let bytes =
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap();
        assert_eq!(bytes[6], 0x0A);
        assert_eq!(bytes[7], 0x0B);

        let cmd = BlueGain::SetValue(crate::types::BlueChannel::new(0xF0).unwrap());
        let bytes =
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).unwrap();
        assert_eq!(bytes[6], 0x0F);
        assert_eq!(bytes[7], 0x00);
    }

    #[test]
    #[allow(clippy::manual_range_contains)]
    fn test_tuning_encoding_formula() {
        // Verify the -10 to +10 => 0x00 to 0x14 conversion
        for level in -10..=10 {
            let expected = (level + 10) as u8;

            let red_cmd = RedTuningCommand::new(RedTuning::new(level).unwrap());
            let red_bytes =
                crate::command::test_wire_bytes(&red_cmd, crate::camera_id::CameraId::CAMERA_1)
                    .unwrap();
            assert_eq!(red_bytes[7], expected);

            let blue_cmd = BlueTuningCommand::new(BlueTuning::new(level).unwrap());
            let blue_bytes =
                crate::command::test_wire_bytes(&blue_cmd, crate::camera_id::CameraId::CAMERA_1)
                    .unwrap();
            assert_eq!(blue_bytes[7], expected);
        }
    }
}
