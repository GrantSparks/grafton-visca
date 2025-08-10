//! White balance methods for cameras using the new GAT architecture.

use crate::{
    command::{
        white_balance::{
            AWBSensitivityCommand, AutoWhiteBalanceSensitivity, WhiteBalanceCommand,
            WhiteBalanceMode,
        },
        Response,
    },
    Error,
};

/// White balance operations (async).
#[cfg(feature = "async")]
pub trait WhiteBalanceOps: Sized {
    /// Set white balance mode to any supported mode.
    async fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error>;

    /// Set auto white balance mode.
    async fn white_balance_auto(&self) -> Result<(), Error>;

    /// Set indoor white balance preset (optimized for incandescent/tungsten lighting).
    async fn white_balance_indoor(&self) -> Result<(), Error>;

    /// Set outdoor white balance preset (optimized for daylight).
    async fn white_balance_outdoor(&self) -> Result<(), Error>;

    /// Set one-push white balance mode (calibrate once based on current scene).
    async fn white_balance_one_push(&self) -> Result<(), Error>;

    /// Set auto tracking white balance (Sony FR7 specific).
    async fn white_balance_atw(&self) -> Result<(), Error>;

    /// Set manual white balance mode.
    async fn white_balance_manual(&self) -> Result<(), Error>;

    /// Set color temperature white balance mode.
    async fn white_balance_color_temperature(&self) -> Result<(), Error>;

    /// Set AWB sensitivity level (PTZOptics specific).
    async fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> Result<(), Error>;
}

/// White balance operations (blocking).
pub trait WhiteBalanceOpsBlocking: Sized {
    /// Set white balance mode to any supported mode.
    fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error>;

    /// Set auto white balance mode.
    fn white_balance_auto(&self) -> Result<(), Error>;

    /// Set indoor white balance preset (optimized for incandescent/tungsten lighting).
    fn white_balance_indoor(&self) -> Result<(), Error>;

    /// Set outdoor white balance preset (optimized for daylight).
    fn white_balance_outdoor(&self) -> Result<(), Error>;

    /// Set one-push white balance mode (calibrate once based on current scene).
    fn white_balance_one_push(&self) -> Result<(), Error>;

    /// Set auto tracking white balance (Sony FR7 specific).
    fn white_balance_atw(&self) -> Result<(), Error>;

    /// Set manual white balance mode.
    fn white_balance_manual(&self) -> Result<(), Error>;

    /// Set color temperature white balance mode.
    fn white_balance_color_temperature(&self) -> Result<(), Error>;

    /// Set AWB sensitivity level (PTZOptics specific).
    fn set_awb_sensitivity(&self, sensitivity: AutoWhiteBalanceSensitivity) -> Result<(), Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    WhiteBalanceOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error> {
        let command = WhiteBalanceCommand { mode };
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn white_balance_auto(&self) -> Result<(), Error> {
        WhiteBalanceOps::set_white_balance_mode(self, WhiteBalanceMode::Auto).await
    }

    async fn white_balance_indoor(&self) -> Result<(), Error> {
        WhiteBalanceOps::set_white_balance_mode(self, WhiteBalanceMode::Indoor).await
    }

    async fn white_balance_outdoor(&self) -> Result<(), Error> {
        WhiteBalanceOps::set_white_balance_mode(self, WhiteBalanceMode::Outdoor).await
    }

    async fn white_balance_one_push(&self) -> Result<(), Error> {
        WhiteBalanceOps::set_white_balance_mode(self, WhiteBalanceMode::OnePush).await
    }

    async fn white_balance_atw(&self) -> Result<(), Error> {
        WhiteBalanceOps::set_white_balance_mode(self, WhiteBalanceMode::ATW).await
    }

    async fn white_balance_manual(&self) -> Result<(), Error> {
        WhiteBalanceOps::set_white_balance_mode(self, WhiteBalanceMode::Manual).await
    }

    async fn white_balance_color_temperature(&self) -> Result<(), Error> {
        WhiteBalanceOps::set_white_balance_mode(self, WhiteBalanceMode::ColorTemperature).await
    }

    async fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> Result<(), Error> {
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
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    WhiteBalanceOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error> {
        let command = WhiteBalanceCommand { mode };
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn white_balance_auto(&self) -> Result<(), Error> {
        WhiteBalanceOpsBlocking::set_white_balance_mode(self, WhiteBalanceMode::Auto)
    }

    fn white_balance_indoor(&self) -> Result<(), Error> {
        WhiteBalanceOpsBlocking::set_white_balance_mode(self, WhiteBalanceMode::Indoor)
    }

    fn white_balance_outdoor(&self) -> Result<(), Error> {
        WhiteBalanceOpsBlocking::set_white_balance_mode(self, WhiteBalanceMode::Outdoor)
    }

    fn white_balance_one_push(&self) -> Result<(), Error> {
        WhiteBalanceOpsBlocking::set_white_balance_mode(self, WhiteBalanceMode::OnePush)
    }

    fn white_balance_atw(&self) -> Result<(), Error> {
        WhiteBalanceOpsBlocking::set_white_balance_mode(self, WhiteBalanceMode::ATW)
    }

    fn white_balance_manual(&self) -> Result<(), Error> {
        WhiteBalanceOpsBlocking::set_white_balance_mode(self, WhiteBalanceMode::Manual)
    }

    fn white_balance_color_temperature(&self) -> Result<(), Error> {
        WhiteBalanceOpsBlocking::set_white_balance_mode(self, WhiteBalanceMode::ColorTemperature)
    }

    fn set_awb_sensitivity(&self, sensitivity: AutoWhiteBalanceSensitivity) -> Result<(), Error> {
        let command = AWBSensitivityCommand { sensitivity };
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
