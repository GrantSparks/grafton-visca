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

#[cfg(feature = "tokio")]
use core::future::Future;

/// Unified trait for image processing operations.
pub trait ImageProcessingOps: Sized {
    /// Enable image flip.
    #[cfg(feature = "tokio")]
    fn enable_flip(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Enable image flip (blocking).
    #[cfg(not(feature = "tokio"))]
    fn enable_flip_blocking(&mut self) -> Result<(), Error>;

    /// Set contrast level.
    #[cfg(feature = "tokio")]
    fn set_contrast(&self, level: ContrastLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set contrast level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_contrast_blocking(&mut self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    #[cfg(feature = "tokio")]
    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set sharpness level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_sharpness_blocking(&mut self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set saturation level.
    #[cfg(feature = "tokio")]
    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set saturation level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_saturation_blocking(&mut self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    #[cfg(feature = "tokio")]
    fn set_hue(&self, level: HueLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set hue level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_hue_blocking(&mut self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    #[cfg(feature = "tokio")]
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set noise reduction 2D level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_2d_blocking(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    #[cfg(feature = "tokio")]
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set noise reduction 3D level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_3d_blocking(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error>;
}

impl ImageProcessingOps for Camera {
    #[cfg(feature = "tokio")]
    fn enable_flip(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = ImageFlipCommand::new(Flip::On);
            self.send_command(&cmd).await?;
            Ok(())
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn enable_flip_blocking(&mut self) -> Result<(), Error> {
        let cmd = ImageFlipCommand::new(Flip::On);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    fn set_contrast(&self, level: ContrastLevel) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = ContrastCommand::new(level);
            self.send_command(&cmd).await?;
            Ok(())
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn set_contrast_blocking(&mut self, level: ContrastLevel) -> Result<(), Error> {
        let cmd = ContrastCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = Sharpness::SetLevel {
                value: level.value(),
            };
            self.send_command(&cmd).await?;
            Ok(())
        }
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
    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = SaturationCommand::new(level);
            self.send_command(&cmd).await?;
            Ok(())
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn set_saturation_blocking(&mut self, level: SaturationLevel) -> Result<(), Error> {
        let cmd = SaturationCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    fn set_hue(&self, level: HueLevel) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = HueCommand::new(level);
            self.send_command(&cmd).await?;
            Ok(())
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn set_hue_blocking(&mut self, level: HueLevel) -> Result<(), Error> {
        let cmd = HueCommand::new(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = NoiseReduction2D::Level(level);
            self.send_command(&cmd).await?;
            Ok(())
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_2d_blocking(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction2D::Level(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = NoiseReduction3D::Level(level);
            self.send_command(&cmd).await?;
            Ok(())
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn set_noise_reduction_3d_blocking(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        let cmd = NoiseReduction3D::Level(level);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }
}