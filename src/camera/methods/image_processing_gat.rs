//! Image processing methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{
        async_facade::CameraAsync,
        blocking_facade::CameraBlocking,
        core::CameraCore,
    },
    capabilities::{ImageProcessing, ProfileMetadata},
    command::{
        const_encoding::{commands, CommandBuilder},
        image_adjustment::{ContrastCommand, SharpnessCommand},
        color::{SaturationCommand, HueCommand},
        image::{NoiseReduction2DCommand, NoiseReduction3DCommand},
        Command, Response,
    },
    transport::gat_transport::Transport,
    types::{
        ContrastLevel, SharpnessLevel, SaturationLevel, HueLevel,
        NoiseReduction2DLevel, NoiseReduction3DLevel,
    },
    Error,
};
use core::future::Future;

/// Enable flip command.
struct EnableFlipCommand([u8; 6]);

impl EnableFlipCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::IMAGE_FLIP_ON);
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
    fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> impl Future<Output = Result<(), Error>>;

    /// Set noise reduction 3D level.
    fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> impl Future<Output = Result<(), Error>>;
}

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
            let cmd = ContrastCommand { value: level };
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
            let cmd = SaturationCommand { level };
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_hue(&self, level: HueLevel) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = HueCommand { level };
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = NoiseReduction2DCommand::Level(level);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> impl Future<Output = Result<(), Error>> {
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
}

impl<P, T> ImageProcessingAsyncExt<P> for CameraAsync<P, T>
where
    P: ProfileMetadata + ImageProcessing,
    T: Transport,
{
    async fn enable_flip(&self) -> Result<(), Error> {
        self.core().enable_flip().await
    }

    async fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error> {
        self.core().set_contrast(level).await
    }

    async fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error> {
        self.core().set_sharpness(level).await
    }

    async fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error> {
        self.core().set_saturation(level).await
    }

    async fn set_hue(&self, level: HueLevel) -> Result<(), Error> {
        self.core().set_hue(level).await
    }

    async fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        self.core().set_noise_reduction_2d(level).await
    }

    async fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        self.core().set_noise_reduction_3d(level).await
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
    T: Transport,
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