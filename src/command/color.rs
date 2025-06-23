//! Color control commands for VISCA cameras.
//!
//! This module provides commands for controlling color-related settings
//! including white balance tuning, saturation, and hue adjustments.

// Crate imports
use crate::{
    command::{response::ResponseType, Command},
    constants::CameraModel,
    error::Error,
    timeout::CommandCategory,
    types::{BlueTuning, HueLevel, RedTuning, SaturationLevel},
};

/// One-Push White Balance Trigger command.
///
/// Performs a one-time automatic white balance adjustment based on
/// the current scene. The camera will analyze the image and set the
/// white balance to achieve neutral colors.
#[derive(Debug, Copy, Clone)]
pub struct OnePushTriggerCommand;

impl Command for OnePushTriggerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x10, 0x05, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Red Gain Tuning command.
///
/// Fine-tunes the red channel gain for white balance adjustment.
/// This is typically used after setting a base white balance mode
/// to make small corrections.
#[derive(Debug, Copy, Clone)]
pub struct RedTuningCommand {
    /// Red tuning level.
    pub level: RedTuning,
}

impl Command for RedTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        // Convert -10..+10 to 0x00..0x14
        let level_value = self.level.value();
        let level_offset = level_value + 10;
        debug_assert!((0..=20).contains(&level_offset));
        // Safe cast: level_offset is guaranteed to be 0..=20 after validation
        #[allow(clippy::cast_sign_loss)]
        let encoded = level_offset as u8;
        Ok(vec![0x81, 0x0A, 0x01, 0x12, encoded, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::PTZOpticsG2 => {
                // G2 supports values -10 to +10 (protocol values 0x00 to 0x14)
                // RedTuning type already enforces this constraint
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Blue Gain Tuning command.
///
/// Fine-tunes the blue channel gain for white balance adjustment.
/// This is typically used after setting a base white balance mode
/// to make small corrections.
#[derive(Debug, Copy, Clone)]
pub struct BlueTuningCommand {
    /// Blue tuning level.
    pub level: BlueTuning,
}

impl Command for BlueTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        // Convert -10..+10 to 0x00..0x14
        let level_value = self.level.value();
        let level_offset = level_value + 10;
        debug_assert!((0..=20).contains(&level_offset));
        // Safe cast: level_offset is guaranteed to be 0..=20 after validation
        #[allow(clippy::cast_sign_loss)]
        let encoded = level_offset as u8;
        Ok(vec![0x81, 0x0A, 0x01, 0x13, encoded, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::PTZOpticsG2 => {
                // G2 supports values -10 to +10 (protocol values 0x00 to 0x14)
                // BlueTuning type already enforces this constraint
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Saturation control command.
///
/// Adjusts the color saturation level of the image.
/// Lower values produce more muted colors, while higher values
/// produce more vivid colors.
#[derive(Debug, Copy, Clone)]
pub struct SaturationCommand {
    /// Saturation level.
    pub level: SaturationLevel,
}

impl Command for SaturationCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![
            0x81,
            0x01,
            0x04,
            0x49,
            0x00,
            0x00,
            0x00,
            self.level.value(),
            0xFF,
        ])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::PTZOpticsG2 => {
                // G2 supports values 0x0 to 0xE (15 values)
                // SaturationLevel type already enforces this constraint
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Hue adjustment command.
///
/// Adjusts the hue (color phase) of the image, shifting all colors
/// around the color wheel. This can be used to correct color casts
/// or create artistic effects.
#[derive(Debug, Copy, Clone)]
pub struct HueCommand {
    /// Hue level.
    pub level: HueLevel,
}

impl Command for HueCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![
            0x81,
            0x01,
            0x04,
            0x4F,
            0x00,
            0x00,
            0x00,
            self.level.value(),
            0xFF,
        ])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::PTZOpticsG2 => {
                // G2 supports values 0x0 to 0xE (15 values)
                // HueLevel type already enforces this constraint
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Color Temperature command.
///
/// Controls the color temperature setting when white balance is in color temperature mode.
/// Color temperature is measured in Kelvin (K) and affects the warmth/coolness of the image.
#[derive(Debug, Copy, Clone)]
pub enum ColorTemperatureCommand {
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
    SetTemperature(crate::types::ColorTemperature),
}

impl Command for ColorTemperatureCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x20, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x20, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x20, 0x03, 0xFF],
            Self::SetTemperature(temp) => {
                let value = temp.value();
                let high = ((value >> 4) & 0x0F) as u8;
                let low = (value & 0x0F) as u8;
                vec![0x81, 0x01, 0x04, 0x20, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Red Gain Direct command (different from tuning).
///
/// Controls the red channel gain in manual white balance mode.
/// This provides direct control over the red color channel intensity.
#[derive(Debug, Copy, Clone)]
pub enum RedGainCommand {
    /// Reset red gain to default value.
    Reset,
    /// Increment red gain value by one step.
    Up,
    /// Decrement red gain value by one step.
    Down,
    /// Set red gain to a specific value.
    ///
    /// Higher values increase the intensity of red in the image.
    SetValue(crate::types::RedGain),
}

impl Command for RedGainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x03, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x03, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x03, 0x03, 0xFF],
            Self::SetValue(gain) => {
                let value = gain.value();
                let high = (value >> 4) & 0x0F;
                let low = value & 0x0F;
                vec![0x81, 0x01, 0x04, 0x43, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Blue Gain Direct command (different from tuning).
///
/// Controls the blue channel gain in manual white balance mode.
/// This provides direct control over the blue color channel intensity.
#[derive(Debug, Copy, Clone)]
pub enum BlueGainCommand {
    /// Reset blue gain to default value.
    Reset,
    /// Increment blue gain value by one step.
    Up,
    /// Decrement blue gain value by one step.
    Down,
    /// Set blue gain to a specific value.
    ///
    /// Higher values increase the intensity of blue in the image.
    SetValue(crate::types::BlueGain),
}

impl Command for BlueGainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x04, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x04, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x04, 0x03, 0xFF],
            Self::SetValue(gain) => {
                let value = gain.value();
                let high = (value >> 4) & 0x0F;
                let low = value & 0x0F;
                vec![0x81, 0x01, 0x04, 0x44, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
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

    #[test]
    fn test_one_push_trigger_command() {
        let cmd = OnePushTriggerCommand;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x10, 0x05, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_red_tuning_command() {
        // Test valid range
        for level in -10..=10 {
            let tuning = RedTuning::new(level).unwrap();
            let cmd = RedTuningCommand { level: tuning };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(bytes.len(), 6);
            assert_eq!(bytes[0..4], [0x81, 0x0A, 0x01, 0x12]);
            assert_eq!(bytes[4], (level + 10) as u8);
            assert_eq!(bytes[5], 0xFF);
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid values - can't create invalid RedTuning
        assert!(RedTuning::new(-11).is_err());
        assert!(RedTuning::new(11).is_err());
    }

    #[test]
    fn test_red_tuning_g2_validation() {
        // Test valid G2 values
        for level in -10..=10 {
            let tuning = RedTuning::new(level).unwrap();
            let cmd = RedTuningCommand { level: tuning };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with RedTuning type
    }

    #[test]
    fn test_blue_tuning_command() {
        // Test valid range
        for level in -10..=10 {
            let tuning = BlueTuning::new(level).unwrap();
            let cmd = BlueTuningCommand { level: tuning };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(bytes.len(), 6);
            assert_eq!(bytes[0..4], [0x81, 0x0A, 0x01, 0x13]);
            assert_eq!(bytes[4], (level + 10) as u8);
            assert_eq!(bytes[5], 0xFF);
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid values - can't create invalid BlueTuning
        assert!(BlueTuning::new(-11).is_err());
        assert!(BlueTuning::new(11).is_err());
    }

    #[test]
    fn test_blue_tuning_g2_validation() {
        // Test valid G2 values
        for level in -10..=10 {
            let tuning = BlueTuning::new(level).unwrap();
            let cmd = BlueTuningCommand { level: tuning };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with BlueTuning type
    }

    #[test]
    fn test_saturation_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let sat_level = SaturationLevel::new(level).unwrap();
            let cmd = SaturationCommand { level: sat_level };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid value - can't create invalid SaturationLevel
        assert!(SaturationLevel::new(0x0F).is_err());
    }

    #[test]
    fn test_saturation_g2_validation() {
        // Test valid G2 values
        for level in 0x00..=0x0E {
            let sat_level = SaturationLevel::new(level).unwrap();
            let cmd = SaturationCommand { level: sat_level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with SaturationLevel type
    }

    #[test]
    fn test_hue_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let hue_level = HueLevel::new(level).unwrap();
            let cmd = HueCommand { level: hue_level };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid value - can't create invalid HueLevel
        assert!(HueLevel::new(0x0F).is_err());
    }

    #[test]
    fn test_hue_g2_validation() {
        // Test valid G2 values
        for level in 0x00..=0x0E {
            let hue_level = HueLevel::new(level).unwrap();
            let cmd = HueCommand { level: hue_level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with HueLevel type
    }

    #[test]
    fn test_color_temperature_command() {
        // Test Reset
        let cmd = ColorTemperatureCommand::Reset;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x00, 0xFF]
        );

        // Test Up
        let cmd = ColorTemperatureCommand::Up;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x02, 0xFF]
        );

        // Test Down
        let cmd = ColorTemperatureCommand::Down;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x03, 0xFF]
        );

        // Test Direct with valid values
        let test_values = vec![0x00, 0x10, 0x20, 0x37];
        for temp in test_values {
            let color_temp = crate::types::ColorTemperature::new(temp).unwrap();
            let cmd = ColorTemperatureCommand::SetTemperature(color_temp);
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..5], [0x81, 0x01, 0x04, 0x20, 0x00]);
            assert_eq!(bytes[5], 0x00);
            assert_eq!(bytes[6], ((temp >> 4) & 0x0F) as u8);
            assert_eq!(bytes[7], (temp & 0x0F) as u8);
            assert_eq!(bytes[8], 0xFF);
        }

        // Test Direct with invalid value - can't create invalid ColorTemperature
        assert!(crate::types::ColorTemperature::new(0x38).is_err());
    }

    #[test]
    fn test_red_gain_command() {
        // Test Reset
        let cmd = RedGainCommand::Reset;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x00, 0xFF]
        );

        // Test Up
        let cmd = RedGainCommand::Up;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x02, 0xFF]
        );

        // Test Down
        let cmd = RedGainCommand::Down;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x03, 0xFF]
        );

        // Test Direct with various values
        let test_values = vec![0x00, 0x55, 0xAA, 0xFF];
        for gain in test_values {
            let red_gain = crate::types::RedGain::new(gain).unwrap();
            let cmd = RedGainCommand::SetValue(red_gain);
            let bytes = cmd.to_bytes().unwrap();
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
        let cmd = BlueGainCommand::Reset;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x00, 0xFF]
        );

        // Test Up
        let cmd = BlueGainCommand::Up;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x02, 0xFF]
        );

        // Test Down
        let cmd = BlueGainCommand::Down;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x03, 0xFF]
        );

        // Test Direct with various values
        let test_values = vec![0x00, 0x55, 0xAA, 0xFF];
        for gain in test_values {
            let blue_gain = crate::types::BlueGain::new(gain).unwrap();
            let cmd = BlueGainCommand::SetValue(blue_gain);
            let bytes = cmd.to_bytes().unwrap();
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
            Box::new(OnePushTriggerCommand),
            Box::new(RedTuningCommand {
                level: RedTuning::new(0).unwrap(),
            }),
            Box::new(BlueTuningCommand {
                level: BlueTuning::new(0).unwrap(),
            }),
            Box::new(SaturationCommand {
                level: SaturationLevel::new(0).unwrap(),
            }),
            Box::new(HueCommand {
                level: HueLevel::new(0).unwrap(),
            }),
            Box::new(ColorTemperatureCommand::Reset),
            Box::new(RedGainCommand::Reset),
            Box::new(BlueGainCommand::Reset),
        ];

        for cmd in cmds {
            let _ = format!("{:?}", cmd);
        }

        // Test Clone
        let red_cmd1 = RedTuningCommand {
            level: RedTuning::new(5).unwrap(),
        };
        let red_cmd2 = red_cmd1;
        assert_eq!(red_cmd1.level.value(), red_cmd2.level.value());
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary values for tuning commands
        let red_min = RedTuningCommand {
            level: RedTuning::new(-10).unwrap(),
        };
        assert!(red_min.to_bytes().is_ok());
        assert_eq!(red_min.to_bytes().unwrap()[4], 0x00);

        let red_max = RedTuningCommand {
            level: RedTuning::new(10).unwrap(),
        };
        assert!(red_max.to_bytes().is_ok());
        assert_eq!(red_max.to_bytes().unwrap()[4], 0x14);

        let blue_min = BlueTuningCommand {
            level: BlueTuning::new(-10).unwrap(),
        };
        assert!(blue_min.to_bytes().is_ok());
        assert_eq!(blue_min.to_bytes().unwrap()[4], 0x00);

        let blue_max = BlueTuningCommand {
            level: BlueTuning::new(10).unwrap(),
        };
        assert!(blue_max.to_bytes().is_ok());
        assert_eq!(blue_max.to_bytes().unwrap()[4], 0x14);

        // Test boundary values for saturation and hue
        let sat_min = SaturationCommand {
            level: SaturationLevel::new(0x00).unwrap(),
        };
        assert!(sat_min.to_bytes().is_ok());

        let sat_max = SaturationCommand {
            level: SaturationLevel::new(0x0E).unwrap(),
        };
        assert!(sat_max.to_bytes().is_ok());

        let hue_min = HueCommand {
            level: HueLevel::new(0x00).unwrap(),
        };
        assert!(hue_min.to_bytes().is_ok());

        let hue_max = HueCommand {
            level: HueLevel::new(0x0E).unwrap(),
        };
        assert!(hue_max.to_bytes().is_ok());

        // Test boundary value for color temperature
        let temp_min = ColorTemperatureCommand::SetTemperature(
            crate::types::ColorTemperature::new(0x00).unwrap(),
        );
        assert!(temp_min.to_bytes().is_ok());

        let temp_max = ColorTemperatureCommand::SetTemperature(
            crate::types::ColorTemperature::new(0x37).unwrap(),
        );
        assert!(temp_max.to_bytes().is_ok());
    }

    #[test]
    fn test_nibble_encoding() {
        // Test that Direct commands properly encode values as nibbles
        let cmd = ColorTemperatureCommand::SetTemperature(
            crate::types::ColorTemperature::new(0x25).unwrap(),
        );
        let bytes = cmd.to_bytes().unwrap();
        assert_eq!(bytes[6], 0x02); // High nibble
        assert_eq!(bytes[7], 0x05); // Low nibble

        let cmd = RedGainCommand::SetValue(crate::types::RedGain::new(0xAB).unwrap());
        let bytes = cmd.to_bytes().unwrap();
        assert_eq!(bytes[6], 0x0A); // High nibble
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = BlueGainCommand::SetValue(crate::types::BlueGain::new(0xF0).unwrap());
        let bytes = cmd.to_bytes().unwrap();
        assert_eq!(bytes[6], 0x0F); // High nibble
        assert_eq!(bytes[7], 0x00); // Low nibble
    }

    #[test]
    #[allow(clippy::manual_range_contains)]
    fn test_tuning_encoding_formula() {
        // Verify the -10 to +10 => 0x00 to 0x14 conversion
        for level in -10..=10 {
            let expected = (level + 10) as u8;

            let red_cmd = RedTuningCommand {
                level: RedTuning::new(level).unwrap(),
            };
            let red_bytes = red_cmd.to_bytes().unwrap();
            assert_eq!(red_bytes[4], expected);

            let blue_cmd = BlueTuningCommand {
                level: BlueTuning::new(level).unwrap(),
            };
            let blue_bytes = blue_cmd.to_bytes().unwrap();
            assert_eq!(blue_bytes[4], expected);
        }
    }
}
