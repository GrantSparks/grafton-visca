//! Image processing methods for cameras that support image adjustments.

use crate::camera::Camera;
use crate::capabilities::{ImageProcessing, ProfileMetadata};
use crate::command::Command;
use crate::Error;
use grafton_visca_macros::dual_native_method;

/// Extension trait that adds image processing methods to cameras.
#[allow(async_fn_in_trait)]
pub trait ImageProcessingMethodsExt {
    /// Enable image flip.
    #[cfg(not(feature = "async"))]
    fn enable_flip(&mut self) -> Result<(), Error>;

    /// Enable image flip.
    #[cfg(feature = "async")]
    async fn enable_flip(&self) -> Result<(), Error>;

    /// Set contrast level.
    #[cfg(not(feature = "async"))]
    fn set_contrast(&mut self, level: crate::types::ContrastLevel) -> Result<(), Error>;

    /// Set contrast level.
    #[cfg(feature = "async")]
    async fn set_contrast(&self, level: crate::types::ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    #[cfg(not(feature = "async"))]
    fn set_sharpness(&mut self, level: crate::types::SharpnessLevel) -> Result<(), Error>;

    /// Set sharpness level.
    #[cfg(feature = "async")]
    async fn set_sharpness(&self, level: crate::types::SharpnessLevel) -> Result<(), Error>;

    /// Set saturation level.
    #[cfg(not(feature = "async"))]
    fn set_saturation(&mut self, level: crate::types::SaturationLevel) -> Result<(), Error>;

    /// Set saturation level.
    #[cfg(feature = "async")]
    async fn set_saturation(&self, level: crate::types::SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    #[cfg(not(feature = "async"))]
    fn set_hue(&mut self, level: crate::types::HueLevel) -> Result<(), Error>;

    /// Set hue level.
    #[cfg(feature = "async")]
    async fn set_hue(&self, level: crate::types::HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    #[cfg(not(feature = "async"))]
    fn set_noise_reduction_2d(
        &mut self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    #[cfg(feature = "async")]
    async fn set_noise_reduction_2d(
        &self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    #[cfg(not(feature = "async"))]
    fn set_noise_reduction_3d(
        &mut self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    #[cfg(feature = "async")]
    async fn set_noise_reduction_3d(
        &self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> Result<(), Error>;
}

// Blanket implementation for cameras with image processing support - blocking
#[cfg(not(feature = "async"))]
impl<P, T> ImageProcessingMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + ImageProcessing,
    T: crate::transport::blocking::BlockingTransport,
{
    #[dual_native_method]
    fn enable_flip(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::IMAGE_FLIP_ON);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_contrast(&mut self, level: crate::types::ContrastLevel) -> Result<(), Error> {
        use crate::command::image_adjustment::ContrastCommand;

        let cmd = ContrastCommand { value: level };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_sharpness(&mut self, level: crate::types::SharpnessLevel) -> Result<(), Error> {
        use crate::command::image_adjustment::SharpnessCommand;

        let cmd = SharpnessCommand::SetLevel {
            value: level.value(),
        };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_saturation(&mut self, level: crate::types::SaturationLevel) -> Result<(), Error> {
        use crate::command::color::SaturationCommand;

        let cmd = SaturationCommand { level };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_hue(&mut self, level: crate::types::HueLevel) -> Result<(), Error> {
        use crate::command::color::HueCommand;

        let cmd = HueCommand { level };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_noise_reduction_2d(
        &mut self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> Result<(), Error> {
        use crate::command::image::NoiseReduction2DCommand;

        let cmd = NoiseReduction2DCommand::Level(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_noise_reduction_3d(
        &mut self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> Result<(), Error> {
        use crate::command::image::NoiseReduction3DCommand;

        let cmd = NoiseReduction3DCommand::Level(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Blanket implementation for cameras with image processing support - async
#[cfg(feature = "async")]
impl<P, T> ImageProcessingMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + ImageProcessing,
    T: crate::transport::AsyncTransport,
{
    #[dual_native_method]
    fn enable_flip(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::IMAGE_FLIP_ON);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_contrast(&mut self, level: crate::types::ContrastLevel) -> Result<(), Error> {
        use crate::command::image_adjustment::ContrastCommand;

        let cmd = ContrastCommand { value: level };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_sharpness(&mut self, level: crate::types::SharpnessLevel) -> Result<(), Error> {
        use crate::command::image_adjustment::SharpnessCommand;

        let cmd = SharpnessCommand::SetLevel {
            value: level.value(),
        };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_saturation(&mut self, level: crate::types::SaturationLevel) -> Result<(), Error> {
        use crate::command::color::SaturationCommand;

        let cmd = SaturationCommand { level };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_hue(&mut self, level: crate::types::HueLevel) -> Result<(), Error> {
        use crate::command::color::HueCommand;

        let cmd = HueCommand { level };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_noise_reduction_2d(
        &mut self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> Result<(), Error> {
        use crate::command::image::NoiseReduction2DCommand;

        let cmd = NoiseReduction2DCommand::Level(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_noise_reduction_3d(
        &mut self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> Result<(), Error> {
        use crate::command::image::NoiseReduction3DCommand;

        let cmd = NoiseReduction3DCommand::Level(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}
