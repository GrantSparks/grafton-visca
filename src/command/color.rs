//! Color control commands for VISCA cameras.
//!
//! This module provides commands for controlling color-related settings
//! including white balance tuning, saturation, and hue adjustments.

// Crate imports
use crate::{
    command::{const_encoding::CommandBuilder, encode_visca::EncodeVisca, response::ResponseType},
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
pub(crate) struct OnePushTriggerCommand {
    /// Internal command bytes.
    command: [u8; 6],
}

impl OnePushTriggerCommand {
    /// Create a new one-push white balance trigger command.
    pub fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(crate::command::const_encoding::constants::color::WB_ONE_PUSH_TRIGGER);
        Self {
            command: cmd.build(),
        }
    }
}

impl Default for OnePushTriggerCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl EncodeVisca for OnePushTriggerCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&self.command);
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Red Channel Tuning command.
///
/// Fine-tunes the red channel gain for white balance adjustment.
/// This is typically used after setting a base white balance mode
/// to make small corrections.
#[derive(Debug, Copy, Clone)]
pub(crate) struct RedTuningCommand {
    /// Internal command bytes.
    command: [u8; 9],
}

impl RedTuningCommand {
    /// Create a new red tuning command.
    pub fn new(level: RedTuning) -> Self {
        let mut cmd = CommandBuilder::<9>::new();
        cmd.append(crate::command::const_encoding::constants::color::RED_GAIN_DIRECT_PREFIX);
        // Convert -10..+10 to 0x00..0x14 (0x00 = -10, 0x0A = 0, 0x14 = +10)
        let level_value = level.value();
        let level_offset = level_value + 10;
        debug_assert!((0..=20).contains(&level_offset));
        // Safe cast: level_offset is guaranteed to be 0..=20 after validation
        #[allow(clippy::cast_sign_loss)]
        let encoded = level_offset as u8;
        // Split into two nibbles for 0p 0q format
        cmd.push(0x00); // High nibble always 0 for range 0x00-0x14
        cmd.push(encoded);
        Self {
            command: cmd.build(),
        }
    }
}

impl EncodeVisca for RedTuningCommand {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&self.command);
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Blue Channel Tuning command.
///
/// Fine-tunes the blue channel gain for white balance adjustment.
/// This is typically used after setting a base white balance mode
/// to make small corrections.
#[derive(Debug, Copy, Clone)]
pub(crate) struct BlueTuningCommand {
    /// Internal command bytes.
    command: [u8; 9],
}

impl BlueTuningCommand {
    /// Create a new blue tuning command.
    pub fn new(level: BlueTuning) -> Self {
        let mut cmd = CommandBuilder::<9>::new();
        cmd.append(crate::command::const_encoding::constants::color::BLUE_GAIN_DIRECT_PREFIX);
        // Convert -10..+10 to 0x00..0x14 (0x00 = -10, 0x0A = 0, 0x14 = +10)
        let level_value = level.value();
        let level_offset = level_value + 10;
        debug_assert!((0..=20).contains(&level_offset));
        // Safe cast: level_offset is guaranteed to be 0..=20 after validation
        #[allow(clippy::cast_sign_loss)]
        let encoded = level_offset as u8;
        // Split into two nibbles for 0p 0q format
        cmd.push(0x00); // High nibble always 0 for range 0x00-0x14
        cmd.push(encoded);
        Self {
            command: cmd.build(),
        }
    }
}

impl EncodeVisca for BlueTuningCommand {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&self.command);
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

crate::visca_builder! {
    /// Saturation control command.
    ///
    /// Adjusts the color saturation level of the image.
    /// Lower values produce more muted colors, while higher values
    /// produce more vivid colors.
    pub(crate) struct SaturationCommand {
        /// The saturation level to set.
        level: SaturationLevel,
    }
    builder<9> => |builder, level| {
        let _ = builder.append(crate::command::const_encoding::constants::color::SATURATION_PREFIX);
        let _ = builder.push(level.value());
    }
    timeout = Quick;
}

impl SaturationCommand {
    /// Create a new saturation command.
    pub fn new(level: SaturationLevel) -> Self {
        Self { level }
    }
}

crate::visca_builder! {
    /// Hue adjustment command.
    ///
    /// Adjusts the hue (color phase) of the image, shifting all colors
    /// around the color wheel. This can be used to correct color casts
    /// or create artistic effects.
    pub(crate) struct HueCommand {
        /// The hue level to set.
        level: HueLevel,
    }
    builder<9> => |builder, level| {
        let _ = builder.append(crate::command::const_encoding::constants::color::HUE_PREFIX);
        let _ = builder.push(level.value());
    }
    timeout = Quick;
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

impl EncodeVisca for ColorTemperature {
    type Response = ();
    const MAX_SIZE: usize = 8;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        match self {
            ColorTemperature::Reset => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x20;
                buffer[4] = 0x00;
                buffer[5] = 0xFF;
                Ok(6)
            }
            ColorTemperature::Up => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x20;
                buffer[4] = 0x02;
                buffer[5] = 0xFF;
                Ok(6)
            }
            ColorTemperature::Down => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x20;
                buffer[4] = 0x03;
                buffer[5] = 0xFF;
                Ok(6)
            }
            ColorTemperature::SetTemperature(temp) => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x20;
                let value = temp.value();
                buffer[4] = ((value >> 4) & 0x0F) as u8; // High nibble (0p)
                buffer[5] = (value & 0x0F) as u8; // Low nibble (0q)
                buffer[6] = 0xFF;
                Ok(7)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Red Channel Direct command (different from tuning).
///
/// Controls the red channel gain in manual white balance mode.
/// This provides direct control over the red color channel intensity.
#[derive(Debug, Copy, Clone)]
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

impl EncodeVisca for RedGain {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        match self {
            RedGain::Reset => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x03;
                buffer[4] = 0x00;
                buffer[5] = 0xFF;
                Ok(6)
            }
            RedGain::Up => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x03;
                buffer[4] = 0x02;
                buffer[5] = 0xFF;
                Ok(6)
            }
            RedGain::Down => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x03;
                buffer[4] = 0x03;
                buffer[5] = 0xFF;
                Ok(6)
            }
            RedGain::SetValue(value) => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x43; // Note: different command byte for direct setting
                buffer[4] = 0x00;
                buffer[5] = 0x00;
                let val = value.value();
                buffer[6] = (val >> 4) & 0x0F; // High nibble
                buffer[7] = val & 0x0F; // Low nibble
                buffer[8] = 0xFF;
                Ok(9)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Blue Channel Direct command (different from tuning).
///
/// Controls the blue channel gain in manual white balance mode.
/// This provides direct control over the blue color channel intensity.
#[derive(Debug, Copy, Clone)]
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

impl EncodeVisca for BlueGain {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        match self {
            BlueGain::Reset => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x04;
                buffer[4] = 0x00;
                buffer[5] = 0xFF;
                Ok(6)
            }
            BlueGain::Up => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x04;
                buffer[4] = 0x02;
                buffer[5] = 0xFF;
                Ok(6)
            }
            BlueGain::Down => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x04;
                buffer[4] = 0x03;
                buffer[5] = 0xFF;
                Ok(6)
            }
            BlueGain::SetValue(value) => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x44; // Note: different command byte for direct setting
                buffer[4] = 0x00;
                buffer[5] = 0x00;
                let val = value.value();
                buffer[6] = (val >> 4) & 0x0F; // High nibble
                buffer[7] = val & 0x0F; // Low nibble
                buffer[8] = 0xFF;
                Ok(9)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
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
    use crate::{constants::CameraVariant, visca_test, EncodeVisca};

    visca_test!(
        OnePushTriggerCommand,
        test_one_push_trigger_command,
        OnePushTriggerCommand::new(),
        &[0x81, 0x01, 0x04, 0x10, 0x05, 0xFF]
    );

    #[test]
    fn test_red_tuning_command() {
        // Test valid range
        for level in -10..=10 {
            let tuning = RedTuning::new(level).unwrap();
            let cmd = RedTuningCommand::new(tuning);
            let bytes = cmd.try_into_vec().unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..6], [0x81, 0x01, 0x04, 0x43, 0x00, 0x00]);
            assert_eq!(bytes[6], 0x00); // High nibble (0p)
            assert_eq!(bytes[7], (level + 10) as u8); // Low nibble (0q)
            assert_eq!(bytes[8], 0xFF); // Terminator
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
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
            let cmd = RedTuningCommand::new(tuning);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with RedTuning type
    }

    #[test]
    fn test_blue_tuning_command() {
        // Test valid range
        for level in -10..=10 {
            let tuning = BlueTuning::new(level).unwrap();
            let cmd = BlueTuningCommand::new(tuning);
            let bytes = cmd.try_into_vec().unwrap();
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..6], [0x81, 0x01, 0x04, 0x44, 0x00, 0x00]);
            assert_eq!(bytes[6], 0x00); // High nibble (0p)
            assert_eq!(bytes[7], (level + 10) as u8); // Low nibble (0q)
            assert_eq!(bytes[8], 0xFF); // Terminator
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
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
            let cmd = BlueTuningCommand::new(tuning);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with BlueTuning type
    }

    #[test]
    fn test_saturation_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let sat_level = SaturationLevel::new(level).unwrap();
            let cmd = SaturationCommand::new(sat_level);
            let bytes = cmd.try_into_vec().unwrap();
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
        }

        // Test invalid value - can't create invalid SaturationLevel
        assert!(SaturationLevel::new(0x0F).is_err());
    }

    #[test]
    fn test_saturation_g2_validation() {
        // Test valid G2 values
        for level in 0x00..=0x0E {
            let sat_level = SaturationLevel::new(level).unwrap();
            let cmd = SaturationCommand::new(sat_level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with SaturationLevel type
    }

    #[test]
    fn test_hue_command() {
        // Test valid range
        for level in 0x00..=0x0E {
            let hue_level = HueLevel::new(level).unwrap();
            let cmd = HueCommand::new(hue_level);
            let bytes = cmd.try_into_vec().unwrap();
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
        }

        // Test invalid value - can't create invalid HueLevel
        assert!(HueLevel::new(0x0F).is_err());
    }

    #[test]
    fn test_hue_g2_validation() {
        // Test valid G2 values
        for level in 0x00..=0x0E {
            let hue_level = HueLevel::new(level).unwrap();
            let cmd = HueCommand::new(hue_level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Invalid values can't be created with HueLevel type
    }

    #[test]
    fn test_color_temperature_command() {
        // Test Reset
        let cmd = ColorTemperature::Reset;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x00, 0xFF]
        );

        // Test Up
        let cmd = ColorTemperature::Up;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x02, 0xFF]
        );

        // Test Down
        let cmd = ColorTemperature::Down;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x03, 0xFF]
        );

        // Test Direct with valid values
        let test_values = vec![0x00, 0x10, 0x20, 0x37];
        for temp in test_values {
            let color_temp = crate::types::ColorTemp::new(temp).unwrap();
            let cmd = ColorTemperature::SetTemperature(color_temp);
            let bytes = cmd.try_into_vec().unwrap();
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
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x00, 0xFF]
        );

        // Test Up
        let cmd = RedGain::Up;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x02, 0xFF]
        );

        // Test Down
        let cmd = RedGain::Down;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x03, 0xFF]
        );

        // Test Direct with various values
        let test_values = vec![0x00, 0x55, 0xAA, 0xFF];
        for gain in test_values {
            let red_gain = crate::types::RedChannel::new(gain).unwrap();
            let cmd = RedGain::SetValue(red_gain);
            let bytes = cmd.try_into_vec().unwrap();
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
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x00, 0xFF]
        );

        // Test Up
        let cmd = BlueGain::Up;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x02, 0xFF]
        );

        // Test Down
        let cmd = BlueGain::Down;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x03, 0xFF]
        );

        // Test Direct with various values
        let test_values = vec![0x00, 0x55, 0xAA, 0xFF];
        for gain in test_values {
            let blue_gain = crate::types::BlueChannel::new(gain).unwrap();
            let cmd = BlueGain::SetValue(blue_gain);
            let bytes = cmd.try_into_vec().unwrap();
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
            red_cmd1.try_into_vec().unwrap(),
            red_cmd2.try_into_vec().unwrap()
        );
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary values for tuning commands
        let red_min = RedTuningCommand::new(RedTuning::new(-10).unwrap());
        assert!(red_min.try_into_vec().is_ok());
        assert_eq!(red_min.try_into_vec().unwrap()[7], 0x00); // Value at index 7 after padding

        let red_max = RedTuningCommand::new(RedTuning::MAX);
        assert!(red_max.try_into_vec().is_ok());
        assert_eq!(red_max.try_into_vec().unwrap()[7], 0x14); // Value at index 7 after padding

        let blue_min = BlueTuningCommand::new(BlueTuning::new(-10).unwrap());
        assert!(blue_min.try_into_vec().is_ok());
        assert_eq!(blue_min.try_into_vec().unwrap()[7], 0x00); // Value at index 7 after padding

        let blue_max = BlueTuningCommand::new(BlueTuning::MAX);
        assert!(blue_max.try_into_vec().is_ok());
        assert_eq!(blue_max.try_into_vec().unwrap()[7], 0x14); // Value at index 7 after padding

        // Test boundary values for saturation and hue
        let sat_min = SaturationCommand::new(SaturationLevel::new(0x00).unwrap());
        assert!(sat_min.try_into_vec().is_ok());

        let sat_max = SaturationCommand::new(SaturationLevel::new(0x0E).unwrap());
        assert!(sat_max.try_into_vec().is_ok());

        let hue_min = HueCommand::new(HueLevel::new(0x00).unwrap());
        assert!(hue_min.try_into_vec().is_ok());

        let hue_max = HueCommand::new(HueLevel::new(0x0E).unwrap());
        assert!(hue_max.try_into_vec().is_ok());

        // Test boundary value for color temperature
        let temp_min =
            ColorTemperature::SetTemperature(crate::types::ColorTemp::new(0x00).unwrap());
        assert!(temp_min.try_into_vec().is_ok());

        let temp_max =
            ColorTemperature::SetTemperature(crate::types::ColorTemp::new(0x37).unwrap());
        assert!(temp_max.try_into_vec().is_ok());
    }

    #[test]
    fn test_nibble_encoding() {
        // Test that Direct commands properly encode values as nibbles
        let cmd = ColorTemperature::SetTemperature(crate::types::ColorTemp::new(0x25).unwrap());
        let bytes = cmd.try_into_vec().unwrap();
        assert_eq!(bytes[4], 0x02); // High nibble (ColorTemp has no padding)
        assert_eq!(bytes[5], 0x05); // Low nibble

        let cmd = RedGain::SetValue(crate::types::RedChannel::new(0xAB).unwrap());
        let bytes = cmd.try_into_vec().unwrap();
        assert_eq!(bytes[6], 0x0A); // High nibble (RedGain has 00 00 padding)
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = BlueGain::SetValue(crate::types::BlueChannel::new(0xF0).unwrap());
        let bytes = cmd.try_into_vec().unwrap();
        assert_eq!(bytes[6], 0x0F); // High nibble (BlueGain has 00 00 padding)
        assert_eq!(bytes[7], 0x00); // Low nibble
    }

    #[test]
    #[allow(clippy::manual_range_contains)]
    fn test_tuning_encoding_formula() {
        // Verify the -10 to +10 => 0x00 to 0x14 conversion
        for level in -10..=10 {
            let expected = (level + 10) as u8;

            let red_cmd = RedTuningCommand::new(RedTuning::new(level).unwrap());
            let red_bytes = red_cmd.try_into_vec().unwrap();
            assert_eq!(red_bytes[7], expected); // Value is at index 7 after 00 00 padding

            let blue_cmd = BlueTuningCommand::new(BlueTuning::new(level).unwrap());
            let blue_bytes = blue_cmd.try_into_vec().unwrap();
            assert_eq!(blue_bytes[7], expected); // Value is at index 7 after 00 00 padding
        }
    }
}
