//! Image processing methods for cameras using the new GAT architecture.

use crate::{
    command::{resolution::PictureEffectMode, ImageFlipMode},
    types::{
        ContrastLevel, HueLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel,
        SaturationLevel, SharpnessLevel,
    },
    Error,
};

/// Image processing operations.
pub trait ImageProcessingControl {
    /// Enable image flip.
    #[cfg(feature = "async")]
    fn enable_flip(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable image flip.
    #[cfg(not(feature = "async"))]
    fn enable_flip(&mut self) -> Result<(), Error>;

    /// Disable image flip.
    #[cfg(feature = "async")]
    fn disable_flip(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable image flip.
    #[cfg(not(feature = "async"))]
    fn disable_flip(&mut self) -> Result<(), Error>;

    /// Enable horizontal flip (mirror).
    #[cfg(feature = "async")]
    fn enable_horizontal_flip(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable horizontal flip (mirror).
    #[cfg(not(feature = "async"))]
    fn enable_horizontal_flip(&mut self) -> Result<(), Error>;

    /// Disable horizontal flip (mirror).
    #[cfg(feature = "async")]
    fn disable_horizontal_flip(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable horizontal flip (mirror).
    #[cfg(not(feature = "async"))]
    fn disable_horizontal_flip(&mut self) -> Result<(), Error>;

    /// Set contrast level.
    #[cfg(feature = "async")]
    fn set_contrast(
        &self,
        level: ContrastLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set contrast level.
    #[cfg(not(feature = "async"))]
    fn set_contrast(&mut self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    #[cfg(feature = "async")]
    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set sharpness level.
    #[cfg(not(feature = "async"))]
    fn set_sharpness(&mut self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set sharpness mode (auto or manual).
    #[cfg(feature = "async")]
    fn set_sharpness_mode(
        &self,
        mode: crate::command::SharpnessMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set sharpness mode (auto or manual).
    #[cfg(not(feature = "async"))]
    fn set_sharpness_mode(&mut self, mode: crate::command::SharpnessMode) -> Result<(), Error>;

    /// Reset sharpness to default.
    #[cfg(feature = "async")]
    fn reset_sharpness(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset sharpness to default.
    #[cfg(not(feature = "async"))]
    fn reset_sharpness(&mut self) -> Result<(), Error>;

    /// Increase sharpness by one step.
    #[cfg(feature = "async")]
    fn increase_sharpness(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Increase sharpness by one step.
    #[cfg(not(feature = "async"))]
    fn increase_sharpness(&mut self) -> Result<(), Error>;

    /// Decrease sharpness by one step.
    #[cfg(feature = "async")]
    fn decrease_sharpness(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Decrease sharpness by one step.
    #[cfg(not(feature = "async"))]
    fn decrease_sharpness(&mut self) -> Result<(), Error>;

    /// Set saturation level.
    #[cfg(feature = "async")]
    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set saturation level.
    #[cfg(not(feature = "async"))]
    fn set_saturation(&mut self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    #[cfg(feature = "async")]
    fn set_hue(
        &self,
        level: HueLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set hue level.
    #[cfg(not(feature = "async"))]
    fn set_hue(&mut self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    #[cfg(feature = "async")]
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set noise reduction 2D level.
    #[cfg(not(feature = "async"))]
    fn set_noise_reduction_2d(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Disable noise reduction 2D.
    #[cfg(feature = "async")]
    fn disable_noise_reduction_2d(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable noise reduction 2D.
    #[cfg(not(feature = "async"))]
    fn disable_noise_reduction_2d(&mut self) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    #[cfg(feature = "async")]
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set noise reduction 3D level.
    #[cfg(not(feature = "async"))]
    fn set_noise_reduction_3d(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error>;

    /// Disable noise reduction 3D.
    #[cfg(feature = "async")]
    fn disable_noise_reduction_3d(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable noise reduction 3D.
    #[cfg(not(feature = "async"))]
    fn disable_noise_reduction_3d(&mut self) -> Result<(), Error>;

    /// Set image flip mode (combined horizontal and vertical).
    #[cfg(feature = "async")]
    fn set_image_flip(
        &self,
        mode: ImageFlipMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set image flip mode (combined horizontal and vertical).
    #[cfg(not(feature = "async"))]
    fn set_image_flip(&mut self, mode: ImageFlipMode) -> Result<(), Error>;

    /// Set luminance (brightness) level.
    #[cfg(feature = "async")]
    fn set_luminance(
        &self,
        level: LuminanceLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set luminance (brightness) level.
    #[cfg(not(feature = "async"))]
    fn set_luminance(&mut self, level: LuminanceLevel) -> Result<(), Error>;

    /// Enable image freeze.
    /// Freezes the camera's video output on the last frame.
    #[cfg(feature = "async")]
    fn enable_freeze(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable image freeze.
    /// Freezes the camera's video output on the last frame.
    #[cfg(not(feature = "async"))]
    fn enable_freeze(&mut self) -> Result<(), Error>;

    /// Disable image freeze.
    /// Resumes normal video output.
    #[cfg(feature = "async")]
    fn disable_freeze(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable image freeze.
    /// Resumes normal video output.
    #[cfg(not(feature = "async"))]
    fn disable_freeze(&mut self) -> Result<(), Error>;

    /// Enable black and white mode.
    /// Switches the camera output to monochrome (black and white).
    #[cfg(feature = "async")]
    fn enable_black_white(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable black and white mode.
    /// Switches the camera output to monochrome (black and white).
    #[cfg(not(feature = "async"))]
    fn enable_black_white(&mut self) -> Result<(), Error>;

    /// Disable black and white mode.
    /// Switches the camera output to color mode.
    #[cfg(feature = "async")]
    fn disable_black_white(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable black and white mode.
    /// Switches the camera output to color mode.
    #[cfg(not(feature = "async"))]
    fn disable_black_white(&mut self) -> Result<(), Error>;

    /// Set picture effect mode.
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// Note that not all effects are supported on all camera models.
    #[cfg(feature = "async")]
    fn set_picture_effect(
        &self,
        mode: PictureEffectMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set picture effect mode.
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// Note that not all effects are supported on all camera models.
    #[cfg(not(feature = "async"))]
    fn set_picture_effect(&mut self, mode: PictureEffectMode) -> Result<(), Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P, Tr, Exec> ImageProcessingControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn enable_flip(&self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFlip::new(crate::command::flip::Flip::On);
        self.send_command(&cmd).await?;
        Ok(())
    }
    async fn disable_flip(&self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFlip::new(crate::command::flip::Flip::Off);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn enable_horizontal_flip(&self) -> Result<(), Error> {
        let cmd = crate::command::flip::HorizontalFlipCommand::new(
            crate::command::flip::HorizontalFlip::On,
        );
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn disable_horizontal_flip(&self) -> Result<(), Error> {
        let cmd = crate::command::flip::HorizontalFlipCommand::new(
            crate::command::flip::HorizontalFlip::Off,
        );
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error> {
        let cmd = crate::command::image::Contrast::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::SetLevel {
            value: level.value(),
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_sharpness_mode(&self, _mode: crate::command::SharpnessMode) -> Result<(), Error> {
        // SharpnessMode command not documented in VISCA protocol spec
        // This may be a proprietary extension - returning unsupported for now
        Err(Error::Unsupported)
    }

    async fn reset_sharpness(&self) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::Reset;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn increase_sharpness(&self) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::Up;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn decrease_sharpness(&self) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::Down;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error> {
        let cmd = crate::command::color::SaturationCommand::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_hue(&self, level: HueLevel) -> Result<(), Error> {
        let cmd = crate::command::color::HueCommand::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction2D::Level(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn disable_noise_reduction_2d(&self) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction2D::Off;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction3D::Level(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn disable_noise_reduction_3d(&self) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction3D::Off;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error> {
        // Use the combined flip command (PtzOptics A4 opcode)
        // This is more efficient than sending separate vertical and horizontal commands
        let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error> {
        let cmd = crate::command::image::Luminance::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn enable_freeze(&self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFreezeCommand::new(crate::command::flip::Freeze::On);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn disable_freeze(&self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFreezeCommand::new(crate::command::flip::Freeze::Off);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn enable_black_white(&self) -> Result<(), Error> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn disable_black_white(&self) -> Result<(), Error> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::Off,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_picture_effect(&self, mode: PictureEffectMode) -> Result<(), Error> {
        let cmd = crate::command::image::PictureEffectCommand { mode };
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P, Tr> ImageProcessingControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn enable_flip(&mut self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFlip::new(crate::command::flip::Flip::On);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }
    fn disable_flip(&mut self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFlip::new(crate::command::flip::Flip::Off);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn enable_horizontal_flip(&mut self) -> Result<(), Error> {
        let cmd = crate::command::flip::HorizontalFlipCommand::new(
            crate::command::flip::HorizontalFlip::On,
        );
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn disable_horizontal_flip(&mut self) -> Result<(), Error> {
        let cmd = crate::command::flip::HorizontalFlipCommand::new(
            crate::command::flip::HorizontalFlip::Off,
        );
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_contrast(&mut self, level: ContrastLevel) -> Result<(), Error> {
        let cmd = crate::command::image::Contrast::new(level);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_sharpness(&mut self, level: SharpnessLevel) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::SetLevel {
            value: level.value(),
        };
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_sharpness_mode(&mut self, _mode: crate::command::SharpnessMode) -> Result<(), Error> {
        // SharpnessMode command not documented in VISCA protocol spec
        // This may be a proprietary extension - returning unsupported for now
        Err(Error::Unsupported)
    }

    fn reset_sharpness(&mut self) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::Reset;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn increase_sharpness(&mut self) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::Up;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn decrease_sharpness(&mut self) -> Result<(), Error> {
        let cmd = crate::command::image::Sharpness::Down;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_saturation(&mut self, level: SaturationLevel) -> Result<(), Error> {
        let cmd = crate::command::color::SaturationCommand::new(level);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_hue(&mut self, level: HueLevel) -> Result<(), Error> {
        let cmd = crate::command::color::HueCommand::new(level);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_noise_reduction_2d(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction2D::Level(level);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn disable_noise_reduction_2d(&mut self) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction2D::Off;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_noise_reduction_3d(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction3D::Level(level);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn disable_noise_reduction_3d(&mut self) -> Result<(), Error> {
        let cmd = crate::command::image::NoiseReduction3D::Off;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_image_flip(&mut self, mode: ImageFlipMode) -> Result<(), Error> {
        // Use the combined flip command (PtzOptics A4 opcode)
        // This is more efficient than sending separate vertical and horizontal commands
        let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_luminance(&mut self, level: LuminanceLevel) -> Result<(), Error> {
        let cmd = crate::command::image::Luminance::new(level);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn enable_freeze(&mut self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFreezeCommand::new(crate::command::flip::Freeze::On);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn disable_freeze(&mut self) -> Result<(), Error> {
        let cmd = crate::command::flip::ImageFreezeCommand::new(crate::command::flip::Freeze::Off);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn enable_black_white(&mut self) -> Result<(), Error> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite,
        };
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn disable_black_white(&mut self) -> Result<(), Error> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::Off,
        };
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn set_picture_effect(&mut self, mode: PictureEffectMode) -> Result<(), Error> {
        let cmd = crate::command::image::PictureEffectCommand { mode };
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }
}
