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
    /// Red tuning level (-10 to +10, where 0 is neutral).
    pub level: i8,
}

impl Command for RedTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        if self.level < -10 || self.level > 10 {
            return Err(Error::InvalidParameter(
                "Red tuning level must be between -10 and +10".into(),
            ));
        }
        // Convert -10..+10 to 0x00..0x14
        // We've validated level is between -10 and +10, so level + 10 is 0..20
        let level_offset = self.level + 10;
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
                if self.level < -10 || self.level > 10 {
                    return Err(Error::ModelValidation {
                        model,
                        command: "RedTuning".to_string(),
                        reason: format!(
                            "Value {} not supported on G2 cameras (range is -10 to +10)",
                            self.level
                        ),
                    });
                }
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
    /// Blue tuning level (-10 to +10, where 0 is neutral).
    pub level: i8,
}

impl Command for BlueTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        if self.level < -10 || self.level > 10 {
            return Err(Error::InvalidParameter(
                "Blue tuning level must be between -10 and +10".into(),
            ));
        }
        // Convert -10..+10 to 0x00..0x14
        // We've validated level is between -10 and +10, so level + 10 is 0..20
        let level_offset = self.level + 10;
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
                if self.level < -10 || self.level > 10 {
                    return Err(Error::ModelValidation {
                        model,
                        command: "BlueTuning".to_string(),
                        reason: format!(
                            "Value {} not supported on G2 cameras (range is -10 to +10)",
                            self.level
                        ),
                    });
                }
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
    /// Saturation level (0x0 = 60%, 0xE = 200%).
    pub level: u8,
}

impl Command for SaturationCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        if self.level > 0x0E {
            return Err(Error::InvalidParameter(
                "Saturation level must be between 0x0 (60%) and 0xE (200%)".into(),
            ));
        }
        Ok(vec![
            0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, self.level, 0xFF,
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
                if self.level > 0x0E {
                    return Err(Error::ModelValidation {
                        model,
                        command: "Saturation".to_string(),
                        reason: format!(
                            "Value {:#02X} not supported on G2 cameras (max is 0x0E)",
                            self.level
                        ),
                    });
                }
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
    /// Hue level (0x0 to 0xE, representing 0 to 14 degrees of rotation).
    pub level: u8,
}

impl Command for HueCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        if self.level > 0x0E {
            return Err(Error::InvalidParameter(
                "Hue level must be between 0x0 (0) and 0xE (14)".into(),
            ));
        }
        Ok(vec![
            0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, self.level, 0xFF,
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
                if self.level > 0x0E {
                    return Err(Error::ModelValidation {
                        model,
                        command: "Hue".to_string(),
                        reason: format!(
                            "Value {:#02X} not supported on G2 cameras (max is 0x0E)",
                            self.level
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

// Use the visca_command! macro for ColorTemperatureCommand
crate::visca_command! {
    /// Color Temperature command.
    ///
    /// Controls the color temperature setting when white balance is in color temperature mode.
    /// Color temperature is measured in Kelvin (K) and affects the warmth/coolness of the image.
    category = "Quick",
    enum ColorTemperatureCommand {
        /// Reset color temperature to default value.
        Reset => [0x81, 0x01, 0x04, 0x20, 0x00, 0xFF],
        /// Increase color temperature (makes image cooler/bluer).
        Up => [0x81, 0x01, 0x04, 0x20, 0x02, 0xFF],
        /// Decrease color temperature (makes image warmer/redder).
        Down => [0x81, 0x01, 0x04, 0x20, 0x03, 0xFF],
        /// Set color temperature directly.
        ///
        /// Valid range: 0x00 (2500K - very warm) to 0x37 (8000K - very cool).
        /// Lower values produce warmer (more orange/red) colors,
        /// higher values produce cooler (more blue) colors.
        Direct(temp: u16) => {
            if *temp > 0x37 {
                return Err(Error::InvalidParameter(
                    "Color temperature must be between 0x00 (2500K) and 0x37 (8000K)".into(),
                ));
            }
            let high = ((temp >> 4) & 0x0F) as u8;
            let low = (temp & 0x0F) as u8;
            Ok(vec![0x81, 0x01, 0x04, 0x20, 0x00, 0x00, high, low, 0xFF])
        }
    }
}

// Use the visca_command! macro for RedGainCommand
crate::visca_command! {
    /// Red Gain Direct command (different from tuning).
    ///
    /// Controls the red channel gain in manual white balance mode.
    /// This provides direct control over the red color channel intensity.
    category = "Quick",
    enum RedGainCommand {
        /// Reset red gain to default value.
        Reset => [0x81, 0x01, 0x04, 0x03, 0x00, 0xFF],
        /// Increment red gain value by one step.
        Up => [0x81, 0x01, 0x04, 0x03, 0x02, 0xFF],
        /// Decrement red gain value by one step.
        Down => [0x81, 0x01, 0x04, 0x03, 0x03, 0xFF],
        /// Set red gain to a specific value.
        ///
        /// Valid range: 0x00 (minimum) to 0xFF (maximum).
        /// Higher values increase the intensity of red in the image.
        Direct(gain: u8) => {
            let high = (gain >> 4) & 0x0F;
            let low = gain & 0x0F;
            Ok(vec![0x81, 0x01, 0x04, 0x43, 0x00, 0x00, high, low, 0xFF])
        }
    }
}

// Use the visca_command! macro for BlueGainCommand
crate::visca_command! {
    /// Blue Gain Direct command (different from tuning).
    ///
    /// Controls the blue channel gain in manual white balance mode.
    /// This provides direct control over the blue color channel intensity.
    category = "Quick",
    enum BlueGainCommand {
        /// Reset blue gain to default value.
        Reset => [0x81, 0x01, 0x04, 0x04, 0x00, 0xFF],
        /// Increment blue gain value by one step.
        Up => [0x81, 0x01, 0x04, 0x04, 0x02, 0xFF],
        /// Decrement blue gain value by one step.
        Down => [0x81, 0x01, 0x04, 0x04, 0x03, 0xFF],
        /// Set blue gain to a specific value.
        ///
        /// Valid range: 0x00 (minimum) to 0xFF (maximum).
        /// Higher values increase the intensity of blue in the image.
        Direct(gain: u8) => {
            let high = (gain >> 4) & 0x0F;
            let low = gain & 0x0F;
            Ok(vec![0x81, 0x01, 0x04, 0x44, 0x00, 0x00, high, low, 0xFF])
        }
    }
}

#[cfg(test)]
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
            let cmd = RedTuningCommand { level };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(bytes.len(), 6);
            assert_eq!(bytes[0..4], [0x81, 0x0A, 0x01, 0x12]);
            assert_eq!(bytes[4], (level + 10) as u8);
            assert_eq!(bytes[5], 0xFF);
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid values
        let cmd = RedTuningCommand { level: -11 };
        assert!(cmd.to_bytes().is_err());

        let cmd = RedTuningCommand { level: 11 };
        assert!(cmd.to_bytes().is_err());
    }

    #[test]
    fn test_red_tuning_g2_validation() {
        // Test valid G2 values
        for level in -10..=10 {
            let cmd = RedTuningCommand { level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test invalid G2 values
        let cmd = RedTuningCommand { level: -11 };
        let result = cmd.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(matches!(result, Err(Error::ModelValidation { .. })));

        let cmd = RedTuningCommand { level: 11 };
        let result = cmd.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(matches!(result, Err(Error::ModelValidation { .. })));
    }

    #[test]
    fn test_blue_tuning_command() {
        // Test valid range
        for level in -10..=10 {
            let cmd = BlueTuningCommand { level };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(bytes.len(), 6);
            assert_eq!(bytes[0..4], [0x81, 0x0A, 0x01, 0x13]);
            assert_eq!(bytes[4], (level + 10) as u8);
            assert_eq!(bytes[5], 0xFF);
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid values
        let cmd = BlueTuningCommand { level: -11 };
        assert!(cmd.to_bytes().is_err());

        let cmd = BlueTuningCommand { level: 11 };
        assert!(cmd.to_bytes().is_err());
    }

    #[test]
    fn test_blue_tuning_g2_validation() {
        // Test valid G2 values
        for level in -10..=10 {
            let cmd = BlueTuningCommand { level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test invalid G2 values
        let cmd = BlueTuningCommand { level: -11 };
        let result = cmd.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(matches!(result, Err(Error::ModelValidation { .. })));

        let cmd = BlueTuningCommand { level: 11 };
        let result = cmd.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(matches!(result, Err(Error::ModelValidation { .. })));
    }

    #[test]
    fn test_saturation_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let cmd = SaturationCommand { level };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid value
        let cmd = SaturationCommand { level: 0x0F };
        assert!(cmd.to_bytes().is_err());
    }

    #[test]
    fn test_saturation_g2_validation() {
        // Test valid G2 values
        for level in 0x00..=0x0E {
            let cmd = SaturationCommand { level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test invalid G2 value
        let cmd = SaturationCommand { level: 0x0F };
        let result = cmd.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(matches!(result, Err(Error::ModelValidation { .. })));
    }

    #[test]
    fn test_hue_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let cmd = HueCommand { level };
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }

        // Test invalid value
        let cmd = HueCommand { level: 0x0F };
        assert!(cmd.to_bytes().is_err());
    }

    #[test]
    fn test_hue_g2_validation() {
        // Test valid G2 values
        for level in 0x00..=0x0E {
            let cmd = HueCommand { level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test invalid G2 value
        let cmd = HueCommand { level: 0x0F };
        let result = cmd.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(matches!(result, Err(Error::ModelValidation { .. })));
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
            let cmd = ColorTemperatureCommand::Direct(temp);
            let bytes = cmd.to_bytes().unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..5], [0x81, 0x01, 0x04, 0x20, 0x00]);
            assert_eq!(bytes[5], 0x00);
            assert_eq!(bytes[6], ((temp >> 4) & 0x0F) as u8);
            assert_eq!(bytes[7], (temp & 0x0F) as u8);
            assert_eq!(bytes[8], 0xFF);
        }

        // Test Direct with invalid value
        let cmd = ColorTemperatureCommand::Direct(0x38);
        assert!(cmd.to_bytes().is_err());
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
            let cmd = RedGainCommand::Direct(gain);
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
            let cmd = BlueGainCommand::Direct(gain);
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
            Box::new(RedTuningCommand { level: 0 }),
            Box::new(BlueTuningCommand { level: 0 }),
            Box::new(SaturationCommand { level: 0 }),
            Box::new(HueCommand { level: 0 }),
            Box::new(ColorTemperatureCommand::Reset),
            Box::new(RedGainCommand::Reset),
            Box::new(BlueGainCommand::Reset),
        ];

        for cmd in cmds {
            let _ = format!("{:?}", cmd);
        }

        // Test Clone
        let red_cmd1 = RedTuningCommand { level: 5 };
        let red_cmd2 = red_cmd1;
        assert_eq!(red_cmd1.level, red_cmd2.level);
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary values for tuning commands
        let red_min = RedTuningCommand { level: -10 };
        assert!(red_min.to_bytes().is_ok());
        assert_eq!(red_min.to_bytes().unwrap()[4], 0x00);

        let red_max = RedTuningCommand { level: 10 };
        assert!(red_max.to_bytes().is_ok());
        assert_eq!(red_max.to_bytes().unwrap()[4], 0x14);

        let blue_min = BlueTuningCommand { level: -10 };
        assert!(blue_min.to_bytes().is_ok());
        assert_eq!(blue_min.to_bytes().unwrap()[4], 0x00);

        let blue_max = BlueTuningCommand { level: 10 };
        assert!(blue_max.to_bytes().is_ok());
        assert_eq!(blue_max.to_bytes().unwrap()[4], 0x14);

        // Test boundary values for saturation and hue
        let sat_min = SaturationCommand { level: 0x00 };
        assert!(sat_min.to_bytes().is_ok());

        let sat_max = SaturationCommand { level: 0x0E };
        assert!(sat_max.to_bytes().is_ok());

        let hue_min = HueCommand { level: 0x00 };
        assert!(hue_min.to_bytes().is_ok());

        let hue_max = HueCommand { level: 0x0E };
        assert!(hue_max.to_bytes().is_ok());

        // Test boundary value for color temperature
        let temp_min = ColorTemperatureCommand::Direct(0x00);
        assert!(temp_min.to_bytes().is_ok());

        let temp_max = ColorTemperatureCommand::Direct(0x37);
        assert!(temp_max.to_bytes().is_ok());
    }

    #[test]
    fn test_nibble_encoding() {
        // Test that Direct commands properly encode values as nibbles
        let cmd = ColorTemperatureCommand::Direct(0x25);
        let bytes = cmd.to_bytes().unwrap();
        assert_eq!(bytes[6], 0x02); // High nibble
        assert_eq!(bytes[7], 0x05); // Low nibble

        let cmd = RedGainCommand::Direct(0xAB);
        let bytes = cmd.to_bytes().unwrap();
        assert_eq!(bytes[6], 0x0A); // High nibble
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = BlueGainCommand::Direct(0xF0);
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

            let red_cmd = RedTuningCommand { level };
            let red_bytes = red_cmd.to_bytes().unwrap();
            assert_eq!(red_bytes[4], expected);

            let blue_cmd = BlueTuningCommand { level };
            let blue_bytes = blue_cmd.to_bytes().unwrap();
            assert_eq!(blue_bytes[4], expected);
        }
    }
}
