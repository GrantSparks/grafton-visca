//! Exposure methods for cameras that support exposure control.

use crate::camera::Camera;
use crate::capabilities::{ProfileMetadata, SupportsExposure};
use crate::Error;

/// Extension trait that adds exposure methods to cameras.
#[allow(async_fn_in_trait)]
pub trait ExposureMethods {
    /// Set auto exposure mode.
    #[cfg(not(feature = "async"))]
    fn exposure_auto(&mut self) -> Result<(), Error>;

    /// Set auto exposure mode.
    #[cfg(feature = "async")]
    async fn exposure_auto(&self) -> Result<(), Error>;

    /// Set manual exposure mode.
    #[cfg(not(feature = "async"))]
    fn exposure_manual(&mut self) -> Result<(), Error>;

    /// Set manual exposure mode.
    #[cfg(feature = "async")]
    async fn exposure_manual(&self) -> Result<(), Error>;
}

// Blanket implementation for cameras with exposure support
#[cfg(not(feature = "async"))]
impl<P, T> ExposureMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsExposure,
    T: crate::transport::blocking::BlockingTransport,
{
    fn exposure_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_AUTO);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    fn exposure_manual(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_MANUAL);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> ExposureMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsExposure,
    T: crate::transport::AsyncTransport,
{
    async fn exposure_auto(&self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_AUTO);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }

    async fn exposure_manual(&self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_MANUAL);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }
}
