//! Image processing control implementation for PTZ cameras.
//!
//! This module provides comprehensive image processing and enhancement functionality including:
//! - Image orientation control (flip, mirror, rotation)
//! - Visual quality adjustments (contrast, sharpness, saturation, hue)
//! - Noise reduction for improved image quality
//! - Special effects and picture modes
//! - Image freeze for static display
//! - Color mode switching (color/black & white)
//! - Luminance (brightness) control
//!
//! These controls allow fine-tuning of the camera's image output to achieve
//! the desired visual quality for different environments and use cases.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    capabilities::ImageProcessing as ImageProcessingCap,
    command::{resolution::PictureEffectMode, ImageFlipMode},
    mode::Mode,
    types::{
        ContrastLevel, HueLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel,
        SaturationLevel, SharpnessLevel,
    },
    Error,
};

/// Image processing operations for PTZ cameras.
///
/// This trait provides comprehensive image processing control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Image Quality Controls
///
/// - **Contrast**: Adjusts the difference between light and dark areas
/// - **Sharpness**: Controls edge enhancement for image clarity
/// - **Saturation**: Adjusts color intensity and vividness
/// - **Hue**: Shifts the overall color tone of the image
/// - **Luminance**: Controls overall brightness level
///
/// # Noise Reduction
///
/// - **2D Noise Reduction**: Reduces noise within individual frames
/// - **3D Noise Reduction**: Reduces noise across multiple frames (temporal)
///
/// # Image Orientation
///
/// - **Flip**: Vertical image inversion
/// - **Mirror**: Horizontal image reflection
/// - **Combined**: Both horizontal and vertical flipping
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.set_contrast(ContrastLevel::new(5)?)?;  // Adjust contrast
/// camera.enable_horizontal_flip()?;  // Mirror image
/// camera.set_noise_reduction_2d(NoiseReduction2DLevel::new(3)?)?;  // Reduce noise
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.set_contrast(ContrastLevel::new(5)?).await?;  // Adjust contrast
/// camera.enable_horizontal_flip().await?;  // Mirror image
/// camera.set_noise_reduction_2d(NoiseReduction2DLevel::new(3)?).await?;  // Reduce noise
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait ImageProcessingControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable image flip.
    ///
    /// Flips the image vertically (upside down). This is useful when the camera
    /// is mounted in an inverted position.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable image flip.
    ///
    /// Returns the image to normal (right-side up) orientation.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable horizontal flip (mirror).
    ///
    /// Mirrors the image horizontally (left-right reversal). This creates
    /// a mirror effect where left and right are swapped.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_horizontal_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable horizontal flip (mirror).
    ///
    /// Returns the image to normal (non-mirrored) orientation.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_horizontal_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set contrast level.
    ///
    /// Adjusts the difference between light and dark areas in the image.
    /// Higher values increase contrast (more dramatic differences),
    /// lower values decrease contrast (flatter appearance).
    ///
    /// **Note:** Contrast is write-only on most cameras. There is no corresponding
    /// inquiry command to read back the current contrast level. The camera will accept
    /// and apply the setting, but you cannot query the current value.
    ///
    /// # Parameters
    /// - `level`: The contrast level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_contrast(
        &self,
        level: ContrastLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set sharpness level.
    ///
    /// Controls edge enhancement to make the image appear sharper or softer.
    /// Higher values increase sharpness (more edge enhancement),
    /// lower values decrease sharpness (softer appearance).
    ///
    /// Use [`InquiryControl::sharpness_level`] to query the current value, and
    /// [`InquiryControl::sharpness_mode`] to query whether sharpness is in auto or manual mode.
    ///
    /// # Parameters
    /// - `level`: The sharpness level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    ///
    /// [`InquiryControl::sharpness_level`]: crate::camera::controls::inquiry::InquiryControl::sharpness_level
    /// [`InquiryControl::sharpness_mode`]: crate::camera::controls::inquiry::InquiryControl::sharpness_mode
    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set sharpness mode (auto or manual).
    ///
    /// **Note:** This operation is currently not supported.
    ///
    /// # Parameters
    /// - `mode`: The sharpness mode to set
    ///
    /// # Errors
    /// Always returns `Error::NotSupported` as this feature is not implemented.
    fn set_sharpness_mode(
        &self,
        mode: crate::command::SharpnessMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset sharpness to default.
    ///
    /// Resets the sharpness level to the camera's default setting.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_sharpness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase sharpness by one step.
    ///
    /// Increases edge enhancement by one increment, making the image sharper.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_sharpness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease sharpness by one step.
    ///
    /// Decreases edge enhancement by one increment, making the image softer.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_sharpness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set saturation level.
    ///
    /// Adjusts the intensity and vividness of colors in the image.
    /// Higher values make colors more vibrant, lower values make them more muted.
    ///
    /// # Parameters
    /// - `level`: The saturation level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set hue level.
    ///
    /// Shifts the overall color tone of the image. This can be used to
    /// correct color casts or create artistic color effects.
    ///
    /// # Parameters
    /// - `level`: The hue level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_hue(&self, level: HueLevel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set noise reduction 2D level.
    ///
    /// Enables spatial noise reduction that processes individual frames
    /// to reduce grain and artifacts. Higher levels provide more noise
    /// reduction but may reduce fine detail.
    ///
    /// # Parameters
    /// - `level`: The 2D noise reduction level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable noise reduction 2D.
    ///
    /// Turns off spatial noise reduction, which may result in more grain
    /// but preserves maximum image detail.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_noise_reduction_2d(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set noise reduction 3D level.
    ///
    /// Enables temporal noise reduction that compares multiple frames
    /// to reduce noise. This is more effective than 2D reduction but
    /// may cause motion artifacts with fast movement.
    ///
    /// # Parameters
    /// - `level`: The 3D noise reduction level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable noise reduction 3D.
    ///
    /// Turns off temporal noise reduction, eliminating potential motion
    /// artifacts but allowing more noise in the image.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_noise_reduction_3d(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set image flip mode (combined horizontal and vertical).
    ///
    /// Sets both horizontal and vertical flip states simultaneously using
    /// a single command. This is more efficient than setting each direction separately.
    ///
    /// # Parameters
    /// - `mode`: The combined flip mode to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_image_flip(
        &self,
        mode: ImageFlipMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set luminance (brightness) level.
    ///
    /// Adjusts the overall brightness of the image output without
    /// affecting exposure settings. This is different from exposure
    /// brightness as it's applied in post-processing.
    ///
    /// **Note:** Luminance is write-only on most cameras. There is no corresponding
    /// inquiry command to read back the current luminance level. The camera will accept
    /// and apply the setting, but you cannot query the current value.
    ///
    /// # Parameters
    /// - `level`: The luminance level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_luminance(
        &self,
        level: LuminanceLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable image freeze.
    ///
    /// Freezes the camera's video output on the last frame. This is useful
    /// for maintaining a static image during camera movement or configuration.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_freeze(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable image freeze.
    ///
    /// Resumes normal live video output after being frozen.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_freeze(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable black and white mode.
    ///
    /// Switches the camera output to monochrome (black and white).
    /// This can be useful for artistic effects or in low-light situations
    /// where color information is not important.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_black_white(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable black and white mode.
    ///
    /// Switches the camera output back to full color mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_black_white(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set picture effect mode.
    ///
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// The available effects vary by camera model and may include options
    /// like pastel, mosaic, or other creative filters.
    ///
    /// # Parameters
    /// - `mode`: The picture effect mode to apply
    ///
    /// # Note
    /// Not all effects are supported on all camera models. Check your
    /// camera documentation for supported effect modes.
    ///
    /// # Errors
    /// Returns an error if the effect is not supported or the command fails.
    fn set_picture_effect(
        &self,
        mode: PictureEffectMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ImageProcessingControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + ImageProcessingCap + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            // Get current horizontal state from cache
            let cached = self.cache().flip_state();
            let h = cached.map(|s| s.horizontal).unwrap_or(false);
            let mode = if h {
                ImageFlipMode::Both
            } else {
                ImageFlipMode::Vertical
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(h, true);
            })
        } else {
            // Legacy: Use separate 0x66 command
            let cmd = crate::command::flip::ImageFlip {
                flip: crate::command::flip::Flip::On,
            };
            self.execute(cmd)
        }
    }

    fn disable_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            let cached = self.cache().flip_state();
            let h = cached.map(|s| s.horizontal).unwrap_or(false);
            let mode = if h {
                ImageFlipMode::Horizontal
            } else {
                ImageFlipMode::Off
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(h, false);
            })
        } else {
            // Legacy: Use separate 0x66 command
            let cmd = crate::command::flip::ImageFlip {
                flip: crate::command::flip::Flip::Off,
            };
            self.execute(cmd)
        }
    }

    fn enable_horizontal_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            let cached = self.cache().flip_state();
            let v = cached.map(|s| s.vertical).unwrap_or(false);
            let mode = if v {
                ImageFlipMode::Both
            } else {
                ImageFlipMode::Horizontal
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(true, v);
            })
        } else {
            // Legacy: Use separate 0x61 command
            let cmd = crate::command::flip::HorizontalFlip { on: true };
            self.execute(cmd)
        }
    }

    fn disable_horizontal_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            let cached = self.cache().flip_state();
            let v = cached.map(|s| s.vertical).unwrap_or(false);
            let mode = if v {
                ImageFlipMode::Vertical
            } else {
                ImageFlipMode::Off
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(false, v);
            })
        } else {
            // Legacy: Use separate 0x61 command
            let cmd = crate::command::flip::HorizontalFlip { on: false };
            self.execute(cmd)
        }
    }

    fn set_contrast(&self, level: ContrastLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Contrast::new(level);
        self.execute(cmd)
    }

    fn set_sharpness(&self, level: SharpnessLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::SetLevel {
            value: level.value(),
        };
        self.execute(cmd)
    }

    /// Set the sharpness mode.
    ///
    /// **Note:** This operation is currently not supported and will always return
    /// `Error::NotSupported`. The SharpnessMode command is not documented in the
    /// standard VISCA protocol specification and may be a proprietary extension.
    fn set_sharpness_mode(
        &self,
        _mode: crate::command::SharpnessMode,
    ) -> M::Fut<'_, Result<(), Error>> {
        // SharpnessMode command not documented in VISCA protocol spec
        // This may be a proprietary extension - returning unsupported for now
        self.error(Error::NotSupported)
    }

    fn reset_sharpness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Reset;
        self.execute(cmd)
    }

    fn increase_sharpness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Up;
        self.execute(cmd)
    }

    fn decrease_sharpness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Down;
        self.execute(cmd)
    }

    fn set_saturation(&self, level: SaturationLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::color::SaturationCommand::new(level);
        self.execute(cmd)
    }

    fn set_hue(&self, level: HueLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::color::HueCommand::new(level);
        self.execute(cmd)
    }

    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction2D::with_level(level);
        self.execute(cmd)
    }

    fn disable_noise_reduction_2d(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction2D::off();
        self.execute(cmd)
    }

    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction3D::with_level(level);
        self.execute(cmd)
    }

    fn disable_noise_reduction_3d(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction3D::off();
        self.execute(cmd)
    }

    fn set_image_flip(&self, mode: ImageFlipMode) -> M::Fut<'_, Result<(), Error>> {
        // Use the combined flip command (PtzOptics A4 opcode)
        // This is more efficient than sending separate vertical and horizontal commands
        let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
        let (h, v) = match mode {
            ImageFlipMode::Off => (false, false),
            ImageFlipMode::Horizontal => (true, false),
            ImageFlipMode::Vertical => (false, true),
            ImageFlipMode::Both => (true, true),
        };
        self.execute_updating_cache(cmd, move |cache| {
            cache.set_flip_state(h, v);
        })
    }

    fn set_luminance(&self, level: LuminanceLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Luminance::new(level);
        self.execute(cmd)
    }

    fn enable_freeze(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::flip::ImageFreeze { on: true };
        self.execute(cmd)
    }

    fn disable_freeze(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::flip::ImageFreeze { on: false };
        self.execute(cmd)
    }

    fn enable_black_white(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite,
        };
        self.execute(cmd)
    }

    fn disable_black_white(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::Off,
        };
        self.execute(cmd)
    }

    fn set_picture_effect(&self, mode: PictureEffectMode) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand { mode };
        self.execute(cmd)
    }
}
