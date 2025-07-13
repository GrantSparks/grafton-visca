//! Image processing methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    command::{
        color::{HueCommand, SaturationCommand},
        flip::{Flip, ImageFlipCommand},
        image::{ImageFlipCombinedCommand, NoiseReduction2D, NoiseReduction3D},
        image_adjustment::{ContrastCommand, LuminanceCommand, Sharpness},
        ImageFlipMode,
    },
    types::{
        ContrastLevel, HueLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel,
        SaturationLevel, SharpnessLevel,
    },
    Error,
};

/// Image processing operations (async).
pub trait ImageProcessingOps: Sized {
    /// Enable image flip.
    async fn enable_flip(&self) -> Result<(), Error>;

    /// Set contrast level.
    async fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    async fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set saturation level.
    async fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    async fn set_hue(&self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    async fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    async fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error>;

    /// Set image flip mode (combined horizontal and vertical).
    async fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error>;

    /// Set luminance (brightness) level.
    async fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error>;
}

/// Image processing operations (blocking).
pub trait ImageProcessingOpsBlocking: Sized {
    /// Enable image flip.
    fn enable_flip(&self) -> Result<(), Error>;

    /// Set contrast level.
    fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set saturation level.
    fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    fn set_hue(&self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error>;

    /// Set image flip mode (combined horizontal and vertical).
    fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error>;

    /// Set luminance (brightness) level.
    fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error>;
}

// Async implementation
impl ImageProcessingOps for Camera {
    async fn enable_flip(&self) -> Result<(), Error> {
        let cmd = ImageFlipCommand::new(Flip::On);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error> {
        let cmd = ContrastCommand::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error> {
        let cmd = Sharpness::SetLevel {
            value: level.value(),
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error> {
        let cmd = SaturationCommand::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_hue(&self, level: HueLevel) -> Result<(), Error> {
        let cmd = HueCommand::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction2D::Level(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction3D::Level(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error> {
        let cmd = ImageFlipCombinedCommand::new(mode);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error> {
        let cmd = LuminanceCommand::new(level);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation
impl ImageProcessingOpsBlocking for Camera {
    fn enable_flip(&self) -> Result<(), Error> {
        let cmd = ImageFlipCommand::new(Flip::On);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error> {
        let cmd = ContrastCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error> {
        let cmd = Sharpness::SetLevel {
            value: level.value(),
        };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error> {
        let cmd = SaturationCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_hue(&self, level: HueLevel) -> Result<(), Error> {
        let cmd = HueCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction2D::Level(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction3D::Level(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error> {
        let cmd = ImageFlipCombinedCommand::new(mode);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error> {
        let cmd = LuminanceCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }
}
