//! Image processing methods for cameras that support image adjustments.

use crate::camera::Camera;
use crate::capabilities::{ImageProcessing, ProfileMetadata};
use crate::Error;

/// Extension trait that adds image processing methods to cameras.
#[allow(async_fn_in_trait)]
pub trait ImageProcessingMethodsExt {
    /// Enable image flip.
    #[cfg(not(feature = "async"))]
    fn enable_flip(&mut self) -> Result<(), Error>;

    /// Enable image flip.
    #[cfg(feature = "async")]
    async fn enable_flip(&self) -> Result<(), Error>;
}

// Blanket implementation for cameras with image processing support
#[cfg(not(feature = "async"))]
impl<P, T> ImageProcessingMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + ImageProcessing,
    T: crate::transport::blocking::BlockingTransport,
{
    fn enable_flip(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::IMAGE_FLIP_ON);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> ImageProcessingMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + ImageProcessing,
    T: crate::transport::AsyncTransport,
{
    async fn enable_flip(&self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::IMAGE_FLIP_ON);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }
}
