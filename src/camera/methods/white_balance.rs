//! White balance methods for cameras using the new GAT architecture.

use crate::{
    command::{
        white_balance::{
            AWBSensitivity, AWBSensitivityCommand, WhiteBalanceCommand, WhiteBalanceMode,
        },
        Response,
    },
    Error,
};

/// White balance operations (async).
#[cfg(feature = "async")]
pub trait WhiteBalanceOps: Sized {
    /// Set auto white balance mode.
    async fn white_balance_auto(&self) -> Result<(), Error>;

    /// Set AWB sensitivity level (PTZOptics specific).
    async fn set_awb_sensitivity(&self, sensitivity: AWBSensitivity) -> Result<(), Error>;
}

/// White balance operations (blocking).
pub trait WhiteBalanceOpsBlocking: Sized {
    /// Set auto white balance mode.
    fn white_balance_auto(&self) -> Result<(), Error>;

    /// Set AWB sensitivity level (PTZOptics specific).
    fn set_awb_sensitivity(&self, sensitivity: AWBSensitivity) -> Result<(), Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> WhiteBalanceOps
    for crate::camera::generic::Camera<P, T>
{
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

    async fn set_awb_sensitivity(&self, sensitivity: AWBSensitivity) -> Result<(), Error> {
        let command = AWBSensitivityCommand { sensitivity };
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> WhiteBalanceOpsBlocking
    for crate::camera::generic::Camera<P, T>
{
    fn white_balance_auto(&self) -> Result<(), Error> {
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

    fn set_awb_sensitivity(&self, sensitivity: AWBSensitivity) -> Result<(), Error> {
        let command = AWBSensitivityCommand { sensitivity };
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
