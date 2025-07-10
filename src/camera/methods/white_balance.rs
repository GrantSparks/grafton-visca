//! White balance methods for cameras that support white balance control.

use crate::camera::Camera;
use crate::capabilities::{ProfileMetadata, SupportsWhiteBalance};
use crate::Error;

/// Extension trait that adds white balance methods to cameras.
#[allow(async_fn_in_trait)]
pub trait WhiteBalanceMethods {
    /// Set auto white balance mode.
    #[cfg(not(feature = "async"))]
    fn white_balance_auto(&mut self) -> Result<(), Error>;

    /// Set auto white balance mode.
    #[cfg(feature = "async")]
    async fn white_balance_auto(&self) -> Result<(), Error>;
}

// Blanket implementation for cameras with white balance support
#[cfg(not(feature = "async"))]
impl<P, T> WhiteBalanceMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsWhiteBalance,
    T: crate::transport::blocking::BlockingTransport,
{
    fn white_balance_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(&commands::WHITE_BALANCE_AUTO);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> WhiteBalanceMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsWhiteBalance,
    T: crate::transport::AsyncTransport,
{
    async fn white_balance_auto(&self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(&commands::WHITE_BALANCE_AUTO);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }
}
