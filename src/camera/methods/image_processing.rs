//! Image processing methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    command::{
        color::{HueCommand, SaturationCommand},
        flip::{Flip, ImageFlipCommand},
        image::{NoiseReduction2D, NoiseReduction3D},
        image_adjustment::{ContrastCommand, Sharpness},
    },
    types::{
        ContrastLevel, HueLevel, NoiseReduction2DLevel, NoiseReduction3DLevel, SaturationLevel,
        SharpnessLevel,
    },
    Error,
};



/// Unified trait for image processing operations.
pub trait ImageProcessingOps: Sized {
    /// Enable image flip.
    #[cfg(feature = "tokio")]
    async fn enable_flip(&self) -> Result<(), Error>;

    /// Enable image flip (blocking).
    #[cfg(not(feature = "tokio"))]
    fn enable_flip_blocking(&mut self) -> Result<(), Error>;

    /// Set contrast level.
    #[cfg(feature = "tokio")]
    async fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error>;

    /// Set contrast level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_contrast_blocking(&mut self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    #[cfg(feature = "tokio")]
    async fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> Result<(), Error>;

    /// Set sharpness level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_sharpness_blocking(&mut self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set saturation level.
    #[cfg(feature = "tokio")]
    async fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> Result<(), Error>;

    /// Set saturation level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_saturation_blocking(&mut self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    #[cfg(feature = "tokio")]
    async fn set_hue(&self, level: HueLevel) -> Result<(), Error>;

    /// Set hue level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_hue_blocking(&mut self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    #[cfg(feature = "tokio")]
    async fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> Result<(), Error>;

    /// Set noise reduction 2D level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_2d_blocking(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    #[cfg(feature = "tokio")]
    async fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> Result<(), Error>;

    /// Set noise reduction 3D level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_3d_blocking(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error>;
}

impl ImageProcessingOps for Camera {
    #[cfg(feature = "tokio")]
    async fn enable_flip(&self) -> Result<(), Error> {
            let cmd = ImageFlipCommand::new(Flip::On);
            self.send_command(&cmd).await?;
            Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn enable_flip_blocking(&mut self) -> Result<(), Error> {
        let cmd = ImageFlipCommand::new(Flip::On);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error> {
            let cmd = ContrastCommand::new(level);
            self.send_command(&cmd).await?;
            Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_contrast_blocking(&mut self, level: ContrastLevel) -> Result<(), Error> {
        let cmd = ContrastCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> Result<(), Error> {
            let cmd = Sharpness::SetLevel {
                value: level.value(),
            };
            self.send_command(&cmd).await?;
            Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_sharpness_blocking(&mut self, level: SharpnessLevel) -> Result<(), Error> {
        let cmd = Sharpness::SetLevel {
            value: level.value(),
        };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> Result<(), Error> {
            let cmd = SaturationCommand::new(level);
            self.send_command(&cmd).await?;
            Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_saturation_blocking(&mut self, level: SaturationLevel) -> Result<(), Error> {
        let cmd = SaturationCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_hue(&self, level: HueLevel) -> Result<(), Error> {
            let cmd = HueCommand::new(level);
            self.send_command(&cmd).await?;
            Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_hue_blocking(&mut self, level: HueLevel) -> Result<(), Error> {
        let cmd = HueCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> Result<(), Error> {
            let cmd = NoiseReduction2D::Level(level);
            self.send_command(&cmd).await?;
            Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_2d_blocking(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction2D::Level(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> Result<(), Error> {
            let cmd = NoiseReduction3D::Level(level);
            self.send_command(&cmd).await?;
            Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_3d_blocking(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction3D::Level(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }
}