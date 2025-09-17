//! White balance control implementation using Mode trait.

use crate::{
    camera::ViscaClient,
    command::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
    mode::Mode,
    Error,
};

/// White balance operations for cameras.
///
/// This trait provides white balance control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait WhiteBalanceControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set white balance mode to any supported mode.
    fn set_white_balance_mode(
        &self,
        mode: WhiteBalanceMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto white balance mode.
    fn white_balance_auto(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set indoor white balance preset (optimized for incandescent/tungsten lighting).
    fn white_balance_indoor(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set outdoor white balance preset (optimized for daylight).
    fn white_balance_outdoor(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set one-push white balance mode (calibrate once based on current scene).
    fn white_balance_one_push(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto tracking white balance (Sony FR7 specific).
    fn white_balance_atw(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set manual white balance mode.
    fn white_balance_manual(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set color temperature white balance mode.
    fn white_balance_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set AWB sensitivity level (PtzOptics specific).
    fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

impl<M, P, Tr, Exec> WhiteBalanceControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::white_balance::WhiteBalanceCommand;
        let cmd = WhiteBalanceCommand { mode };
        self.execute(cmd)
    }

    fn white_balance_auto(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::Auto)
    }

    fn white_balance_indoor(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::Indoor)
    }

    fn white_balance_outdoor(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::Outdoor)
    }

    fn white_balance_one_push(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::OnePush)
    }

    fn white_balance_atw(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::ATW)
    }

    fn white_balance_manual(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::Manual)
    }

    fn white_balance_color_temperature(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)
    }

    fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::white_balance::AWBSensitivityCommand;
        let cmd = AWBSensitivityCommand { sensitivity };
        self.execute(cmd)
    }
}
