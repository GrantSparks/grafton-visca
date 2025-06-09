//! High-level extension trait for image settings control operations.

// Crate imports
use crate::{
    command::{
        BlackWhiteCommand, ContrastCommand, HueCommand, ImageFlipCombinedCommand, ImageFlipMode,
        LuminanceCommand, NoiseReduction2DCommand, NoiseReduction3DCommand, SaturationCommand,
        SharpnessCommand,
    },
    error::Error as ViscaError,
    Response, Transport,
};

/// Extension trait providing high-level image settings control methods.
pub trait ViscaImageExt: Transport {
    /// Enable or disable black and white mode.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable black and white mode
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Enable black and white mode
    /// client.set_black_white_mode(true)?;
    ///
    /// // Return to color mode
    /// client.set_black_white_mode(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_black_white_mode(&mut self, enabled: bool) -> Result<(), ViscaError> {
        let command = BlackWhiteCommand { on: enabled };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set 2D noise reduction level.
    ///
    /// # Arguments
    /// * `level` - Noise reduction level (None for off, Some(1-5) for levels)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if level is not in the range 1-5,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Turn off 2D noise reduction
    /// client.set_noise_reduction_2d(None)?;
    ///
    /// // Set to low noise reduction
    /// client.set_noise_reduction_2d(Some(1))?;
    ///
    /// // Set to maximum noise reduction
    /// client.set_noise_reduction_2d(Some(5))?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_noise_reduction_2d(&mut self, level: Option<u8>) -> Result<(), ViscaError> {
        use crate::types::NoiseReduction2DLevel;
        let command = match level {
            None => NoiseReduction2DCommand::Off,
            Some(lvl) => NoiseReduction2DCommand::Level(NoiseReduction2DLevel::new(lvl)?),
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set 3D noise reduction level.
    ///
    /// # Arguments
    /// * `level` - Noise reduction level (None for off, Some(1-8) for levels)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if level is not in the range 1-8,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Turn off 3D noise reduction
    /// client.set_noise_reduction_3d(None)?;
    ///
    /// // Set to moderate noise reduction
    /// client.set_noise_reduction_3d(Some(4))?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_noise_reduction_3d(&mut self, level: Option<u8>) -> Result<(), ViscaError> {
        use crate::types::NoiseReduction3DLevel;
        let command = match level {
            None => NoiseReduction3DCommand::Off,
            Some(lvl) => NoiseReduction3DCommand::Level(NoiseReduction3DLevel::new(lvl)?),
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set image flip settings.
    ///
    /// # Arguments
    /// * `horizontal` - Whether to flip horizontally
    /// * `vertical` - Whether to flip vertically
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // No flip
    /// client.set_image_flip(false, false)?;
    ///
    /// // Horizontal flip only
    /// client.set_image_flip(true, false)?;
    ///
    /// // Vertical flip only
    /// client.set_image_flip(false, true)?;
    ///
    /// // Both horizontal and vertical flip (180° rotation)
    /// client.set_image_flip(true, true)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_image_flip(&mut self, horizontal: bool, vertical: bool) -> Result<(), ViscaError> {
        let mode = match (horizontal, vertical) {
            (false, false) => ImageFlipMode::Off,
            (true, false) => ImageFlipMode::Horizontal,
            (false, true) => ImageFlipMode::Vertical,
            (true, true) => ImageFlipMode::Both,
        };
        let command = ImageFlipCombinedCommand { mode };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set sharpness level.
    ///
    /// # Arguments
    /// * `level` - Sharpness level (0 to 14, 7 is default)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if level is greater than 14,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set minimum sharpness (softest)
    /// client.set_sharpness(0)?;
    ///
    /// // Set default sharpness
    /// client.set_sharpness(7)?;
    ///
    /// // Set maximum sharpness
    /// client.set_sharpness(14)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_sharpness(&mut self, level: u8) -> Result<(), ViscaError> {
        if level > 14 {
            return Err(ViscaError::InvalidParameter(
                "Sharpness level must be 0-14".into(),
            ));
        }
        let command = SharpnessCommand::Direct { value: level };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase sharpness.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.sharpness_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn sharpness_up(&mut self) -> Result<(), ViscaError> {
        let command = SharpnessCommand::Up;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease sharpness.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.sharpness_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn sharpness_down(&mut self) -> Result<(), ViscaError> {
        let command = SharpnessCommand::Down;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset sharpness.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.sharpness_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn sharpness_reset(&mut self) -> Result<(), ViscaError> {
        let command = SharpnessCommand::Reset;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set color saturation level.
    ///
    /// # Arguments
    /// * `level` - Saturation level (0 to 14, 7 is default)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if level is greater than 14,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set minimum saturation (monochrome)
    /// client.set_saturation(0)?;
    ///
    /// // Set default saturation
    /// client.set_saturation(7)?;
    ///
    /// // Set maximum saturation (vivid colors)
    /// client.set_saturation(14)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_saturation(&mut self, level: u8) -> Result<(), ViscaError> {
        if level > 0x0E {
            return Err(ViscaError::InvalidParameter(
                "Saturation level must be 0x0-0xE (0-14)".into(),
            ));
        }
        let command = SaturationCommand { level };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set hue adjustment.
    ///
    /// # Arguments
    /// * `level` - Hue level (0 to 14, 7 is default/neutral)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if level is greater than 14,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Shift hue towards red
    /// client.set_hue(4)?;
    ///
    /// // Reset hue to default
    /// client.set_hue(7)?;
    ///
    /// // Shift hue towards green
    /// client.set_hue(10)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_hue(&mut self, level: u8) -> Result<(), ViscaError> {
        if level > 0x0E {
            return Err(ViscaError::InvalidParameter(
                "Hue level must be 0x0-0xE (0-14)".into(),
            ));
        }
        let command = HueCommand { level };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set contrast level.
    ///
    /// # Arguments
    /// * `level` - Contrast level (0 to 14, 7 is default)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if level is greater than 14,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set minimum contrast
    /// client.set_contrast(0)?;
    ///
    /// // Set default contrast
    /// client.set_contrast(7)?;
    ///
    /// // Set maximum contrast
    /// client.set_contrast(14)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_contrast(&mut self, level: u8) -> Result<(), ViscaError> {
        use crate::types::ContrastLevel;

        if level > 14 {
            return Err(ViscaError::InvalidParameter(
                "Contrast level must be 0-14".into(),
            ));
        }
        let command = ContrastCommand {
            value: ContrastLevel::new(level)?,
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    // Note: The contrast command only supports direct value setting, not up/down/reset

    /// Set luminance (brightness) level.
    ///
    /// # Arguments
    /// * `level` - Luminance level (0 to 14, 7 is default)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if level is greater than 14,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set minimum luminance
    /// client.set_luminance(0)?;
    ///
    /// // Set default luminance
    /// client.set_luminance(7)?;
    ///
    /// // Set maximum luminance
    /// client.set_luminance(14)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_luminance(&mut self, level: u8) -> Result<(), ViscaError> {
        use crate::types::LuminanceLevel;

        if level > 14 {
            return Err(ViscaError::InvalidParameter(
                "Luminance level must be 0-14".into(),
            ));
        }
        let command = LuminanceCommand {
            value: LuminanceLevel::new(level)?,
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Apply an image preset configuration.
    ///
    /// # Arguments
    /// * `preset` - The image preset to apply
    ///
    /// # Errors
    /// Returns `ViscaError` if any of the underlying setting commands fail to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaImageExt, ImagePreset};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Apply vivid preset
    /// client.apply_image_preset(ImagePreset::Vivid)?;
    ///
    /// // Apply cinema preset
    /// client.apply_image_preset(ImagePreset::Cinema)?;
    ///
    /// // Reset to default
    /// client.apply_image_preset(ImagePreset::Default)?;
    /// # Ok(())
    /// # }
    /// ```
    fn apply_image_preset(&mut self, preset: ImagePreset) -> Result<(), ViscaError> {
        match preset {
            ImagePreset::Default => {
                self.set_sharpness(7)?;
                self.set_saturation(7)?;
                self.set_contrast(7)?;
                self.set_hue(7)?;
            }
            ImagePreset::Vivid => {
                self.set_sharpness(10)?;
                self.set_saturation(11)?;
                self.set_contrast(9)?;
                self.set_hue(7)?;
            }
            ImagePreset::Cinema => {
                self.set_sharpness(4)?;
                self.set_saturation(9)?;
                self.set_contrast(8)?;
                self.set_hue(7)?;
            }
            ImagePreset::Monochrome => {
                self.set_saturation(0)?;
                self.set_contrast(9)?;
                self.set_sharpness(8)?;
            }
            ImagePreset::Soft => {
                self.set_sharpness(2)?;
                self.set_saturation(6)?;
                self.set_contrast(5)?;
                self.set_hue(7)?;
            }
        }
        Ok(())
    }
}

/// Common image preset configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePreset {
    /// Default settings (all at neutral)
    Default,
    /// Vivid colors and sharp image
    Vivid,
    /// Cinema-like settings
    Cinema,
    /// Black and white with enhanced contrast
    Monochrome,
    /// Soft, dreamy look
    Soft,
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: Transport> ViscaImageExt for T {}
