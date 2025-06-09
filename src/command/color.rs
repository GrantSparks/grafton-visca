//! Color control commands for VISCA cameras.
//!
//! This module provides commands for controlling color-related settings
//! including white balance tuning, saturation, and hue adjustments.

// Crate imports
use crate::{
    command::{response::ResponseType, Command},
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
