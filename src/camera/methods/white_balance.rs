//! White balance methods for cameras using mode markers.

use crate::{
    command::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
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

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> WhiteBalanceOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error> {
        use crate::command::white_balance::WhiteBalanceCommand;
        let cmd = WhiteBalanceCommand { mode };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn white_balance_auto(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Auto).await
    }

    async fn white_balance_indoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Indoor).await
    }

    async fn white_balance_outdoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Outdoor).await
    }

    async fn white_balance_one_push(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::OnePush).await
    }

    async fn white_balance_atw(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::ATW).await
    }

    async fn white_balance_manual(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Manual).await
    }

    async fn white_balance_color_temperature(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)
            .await
    }

    async fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> Result<(), Error> {
        use crate::command::white_balance::AWBSensitivityCommand;
        let cmd = AWBSensitivityCommand { sensitivity };
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> WhiteBalanceOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error> {
        use crate::command::white_balance::WhiteBalanceCommand;
        let cmd = WhiteBalanceCommand { mode };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn white_balance_auto(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Auto)
    }

    fn white_balance_indoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Indoor)
    }

    fn white_balance_outdoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Outdoor)
    }

    fn white_balance_one_push(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::OnePush)
    }

    fn white_balance_atw(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::ATW)
    }

    fn white_balance_manual(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Manual)
    }

    fn white_balance_color_temperature(&self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)
    }

    fn set_awb_sensitivity(&self, sensitivity: AutoWhiteBalanceSensitivity) -> Result<(), Error> {
        use crate::command::white_balance::AWBSensitivityCommand;
        let cmd = AWBSensitivityCommand { sensitivity };
        self.send_command(&cmd)?;
        Ok(())
    }
}
