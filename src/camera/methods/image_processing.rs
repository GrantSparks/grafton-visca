//! Image processing methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{ImageProcessing, ProfileMetadata},
    command::{
        color::{HueCommand, SaturationCommand},
        const_encoding::{commands, CommandBuilder},
        image::{NoiseReduction2DCommand, NoiseReduction3DCommand},
        image_adjustment::{ContrastCommand, SharpnessCommand},
        Command, Response,
    },
    transport::core::{BlockingTransport, Transport},
    types::{
        ContrastLevel, HueLevel, NoiseReduction2DLevel, NoiseReduction3DLevel, SaturationLevel,
        SharpnessLevel,
    },
    Error,
};
use core::future::Future;

/// Enable flip command.
struct EnableFlipCommand([u8; 6]);

impl EnableFlipCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FLIP_PREFIX);
        cmd.push(0x02); // On value
        Self(cmd.build())
    }
}

impl Command for EnableFlipCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        None // Action command
    }
}

/// Extension trait for CameraCore that adds image processing methods.
pub trait ImageProcessingCoreExt<P: ProfileMetadata + ImageProcessing> {
    /// Enable image flip.
    fn enable_flip(&self) -> impl Future<Output = Result<(), Error>>;

    /// Set contrast level.
    fn set_contrast(&self, level: ContrastLevel) -> impl Future<Output = Result<(), Error>>;

    /// Set sharpness level.
    fn set_sharpness(&self, level: SharpnessLevel) -> impl Future<Output = Result<(), Error>>;

    /// Set saturation level.
    fn set_saturation(&self, level: SaturationLevel) -> impl Future<Output = Result<(), Error>>;

    /// Set hue level.
    fn set_hue(&self, level: HueLevel) -> impl Future<Output = Result<(), Error>>;

    /// Set noise reduction 2D level.
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Set noise reduction 3D level.
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> impl Future<Output = Result<(), Error>>;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> ImageProcessingCoreExt<P> for CameraCore<P, T>
where
    P: ProfileMetadata + ImageProcessing,
    T: Transport,
{
    fn enable_flip(&self) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = EnableFlipCommand::new();
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_contrast(&self, level: ContrastLevel) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = ContrastCommand::new(level);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_sharpness(&self, level: SharpnessLevel) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = SharpnessCommand::SetLevel {
                value: level.value(),
            };
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_saturation(&self, level: SaturationLevel) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = SaturationCommand::new(level);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_hue(&self, level: HueLevel) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = HueCommand::new(level);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = NoiseReduction2DCommand::Level(level);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = NoiseReduction3DCommand::Level(level);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for CameraAsync that adds image processing methods.
pub trait ImageProcessingAsyncExt<P: ProfileMetadata + ImageProcessing>: Sized {
    /// Enable image flip.
    fn enable_flip(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set contrast level.
    fn set_contrast(&self, level: ContrastLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set sharpness level.
    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set saturation level.
    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set hue level.
    fn set_hue(&self, level: HueLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set noise reduction 2D level.
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set noise reduction 3D level.
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> ImageProcessingAsyncExt<P> for CameraAsync<P, T>
where
    P: ProfileMetadata + ImageProcessing + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn enable_flip(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().enable_flip().await }
    }

    fn set_contrast(&self, level: ContrastLevel) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_contrast(level).await }
    }

    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_sharpness(level).await }
    }

    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_saturation(level).await }
    }

    fn set_hue(&self, level: HueLevel) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_hue(level).await }
    }

    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_noise_reduction_2d(level).await }
    }

    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_noise_reduction_3d(level).await }
    }
}

/// Extension trait for CameraBlocking that adds image processing methods.
pub trait ImageProcessingBlockingExt<P: ProfileMetadata + ImageProcessing>: Sized {
    /// Enable image flip.
    fn enable_flip(&mut self) -> Result<(), Error>;

    /// Set contrast level.
    fn set_contrast(&mut self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    fn set_sharpness(&mut self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set saturation level.
    fn set_saturation(&mut self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    fn set_hue(&mut self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    fn set_noise_reduction_2d(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    fn set_noise_reduction_3d(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error>;
}

impl<P, T> ImageProcessingBlockingExt<P> for CameraBlocking<P, T>
where
    P: ProfileMetadata + ImageProcessing,
    T: BlockingTransport,
{
    fn enable_flip(&mut self) -> Result<(), Error> {
        block_on(self.core().enable_flip())
    }

    fn set_contrast(&mut self, level: ContrastLevel) -> Result<(), Error> {
        block_on(self.core().set_contrast(level))
    }

    fn set_sharpness(&mut self, level: SharpnessLevel) -> Result<(), Error> {
        block_on(self.core().set_sharpness(level))
    }

    fn set_saturation(&mut self, level: SaturationLevel) -> Result<(), Error> {
        block_on(self.core().set_saturation(level))
    }

    fn set_hue(&mut self, level: HueLevel) -> Result<(), Error> {
        block_on(self.core().set_hue(level))
    }

    fn set_noise_reduction_2d(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        block_on(self.core().set_noise_reduction_2d(level))
    }

    fn set_noise_reduction_3d(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        block_on(self.core().set_noise_reduction_3d(level))
    }
}
