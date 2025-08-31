//! White balance methods for cameras.

use crate::{
    command::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
    Error,
};

/// White balance operations for cameras.
///
/// This trait provides white balance control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait WhiteBalanceControl {
    /// Set white balance mode to any supported mode.
    #[cfg(feature = "async")]
    fn set_white_balance_mode(
        &self,
        mode: WhiteBalanceMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set white balance mode to any supported mode.
    #[cfg(not(feature = "async"))]
    fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), Error>;

    /// Set auto white balance mode.
    #[cfg(feature = "async")]
    fn white_balance_auto(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set auto white balance mode.
    #[cfg(not(feature = "async"))]
    fn white_balance_auto(&mut self) -> Result<(), Error>;

    /// Set indoor white balance preset (optimized for incandescent/tungsten lighting).
    #[cfg(feature = "async")]
    fn white_balance_indoor(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set indoor white balance preset (optimized for incandescent/tungsten lighting).
    #[cfg(not(feature = "async"))]
    fn white_balance_indoor(&mut self) -> Result<(), Error>;

    /// Set outdoor white balance preset (optimized for daylight).
    #[cfg(feature = "async")]
    fn white_balance_outdoor(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set outdoor white balance preset (optimized for daylight).
    #[cfg(not(feature = "async"))]
    fn white_balance_outdoor(&mut self) -> Result<(), Error>;

    /// Set one-push white balance mode (calibrate once based on current scene).
    #[cfg(feature = "async")]
    fn white_balance_one_push(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set one-push white balance mode (calibrate once based on current scene).
    #[cfg(not(feature = "async"))]
    fn white_balance_one_push(&mut self) -> Result<(), Error>;

    /// Set auto tracking white balance (Sony FR7 specific).
    #[cfg(feature = "async")]
    fn white_balance_atw(&self)
        -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set auto tracking white balance (Sony FR7 specific).
    #[cfg(not(feature = "async"))]
    fn white_balance_atw(&mut self) -> Result<(), Error>;

    /// Set manual white balance mode.
    #[cfg(feature = "async")]
    fn white_balance_manual(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set manual white balance mode.
    #[cfg(not(feature = "async"))]
    fn white_balance_manual(&mut self) -> Result<(), Error>;

    /// Set color temperature white balance mode.
    #[cfg(feature = "async")]
    fn white_balance_color_temperature(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set color temperature white balance mode.
    #[cfg(not(feature = "async"))]
    fn white_balance_color_temperature(&mut self) -> Result<(), Error>;

    /// Set AWB sensitivity level (PtzOptics specific).
    #[cfg(feature = "async")]
    fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set AWB sensitivity level (PtzOptics specific).
    #[cfg(not(feature = "async"))]
    fn set_awb_sensitivity(
        &mut self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> Result<(), Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async white balance control trait (deprecated, use WhiteBalanceControl instead).
/// Blocking white balance control trait (deprecated, use WhiteBalanceControl instead).
// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, Tr, Exec> WhiteBalanceControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> WhiteBalanceControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), Error> {
        use crate::command::white_balance::WhiteBalanceCommand;
        let cmd = WhiteBalanceCommand { mode };
        self.send_command(&cmd).into_inner()?;
        Ok(())
    }

    fn white_balance_auto(&mut self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Auto)
    }

    fn white_balance_indoor(&mut self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Indoor)
    }

    fn white_balance_outdoor(&mut self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Outdoor)
    }

    fn white_balance_one_push(&mut self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::OnePush)
    }

    fn white_balance_atw(&mut self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::ATW)
    }

    fn white_balance_manual(&mut self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::Manual)
    }

    fn white_balance_color_temperature(&mut self) -> Result<(), Error> {
        self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)
    }

    fn set_awb_sensitivity(
        &mut self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> Result<(), Error> {
        use crate::command::white_balance::AWBSensitivityCommand;

        let cmd = AWBSensitivityCommand { sensitivity };
        self.send_command(&cmd).into_inner()?;
        Ok(())
    }
}
