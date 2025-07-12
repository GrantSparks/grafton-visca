//! White balance methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    command::{
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        Response,
    },
    Error,
};

/// WhiteBalance operations.
pub trait WhiteBalanceOps: Sized {
    /// Set auto white balance mode.
    #[cfg(feature = "tokio")]
    async fn white_balance_auto(&self) -> Result<(), Error>;

    /// Set auto white balance mode. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn white_balance_auto_blocking(&mut self) -> Result<(), Error>;
}

impl WhiteBalanceOps for Camera {
    #[cfg(feature = "tokio")]
    async fn white_balance_auto(&self) -> Result<(), Error> {
        let command = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn white_balance_auto_blocking(&mut self) -> Result<(), Error> {
        let command = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
