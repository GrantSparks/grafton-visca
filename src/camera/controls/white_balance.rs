//! White balance control implementation for PTZ cameras.
//!
//! This module provides comprehensive white balance control functionality including:
//! - Automatic white balance with sensitivity adjustment
//! - Preset modes for common lighting conditions (indoor/outdoor)
//! - One-push calibration for custom lighting scenarios
//! - Manual white balance for precise color control
//! - Auto tracking white balance (ATW) for changing conditions
//! - Color temperature mode for specific color balance needs
//!
//! White balance ensures that colors appear natural under different lighting conditions
//! by adjusting the camera's color sensitivity to match the light source.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
    mode::Mode,
    Error,
};

/// White balance operations for PTZ cameras.
///
/// This trait provides comprehensive white balance control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # White Balance Modes
///
/// - **Auto**: Camera automatically adjusts white balance based on scene analysis
/// - **Indoor**: Optimized for incandescent/tungsten lighting (warm light)
/// - **Outdoor**: Optimized for daylight conditions (cool light)
/// - **One-Push**: Single calibration based on a white/neutral reference in the scene
/// - **ATW**: Auto tracking white balance that continuously adapts to changing conditions
/// - **Manual**: Direct user control over white balance parameters
/// - **Color Temperature**: Set specific color temperature value
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.white_balance_auto()?;  // Enable auto white balance
/// camera.white_balance_indoor()?;  // Use indoor preset
/// camera.white_balance_one_push()?;  // Calibrate from current scene
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.white_balance_auto().await?;  // Enable auto white balance
/// camera.white_balance_indoor().await?;  // Use indoor preset
/// camera.white_balance_one_push().await?;  // Calibrate from current scene
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait WhiteBalanceControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set white balance mode to any supported mode.
    ///
    /// Allows setting any of the supported white balance modes directly.
    ///
    /// # Parameters
    /// - `mode`: The white balance mode to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_white_balance_mode(
        &self,
        mode: WhiteBalanceMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto white balance mode.
    ///
    /// In auto white balance mode, the camera continuously analyzes the scene
    /// and adjusts color balance to make whites appear neutral.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_auto(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set indoor white balance preset.
    ///
    /// Optimized for incandescent/tungsten lighting typically found indoors.
    /// This preset adds a cooling effect to compensate for the warm color
    /// temperature of indoor lighting (approximately 3200K).
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_indoor(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set outdoor white balance preset.
    ///
    /// Optimized for daylight conditions typically found outdoors.
    /// This preset is calibrated for the cool color temperature of
    /// daylight (approximately 5600K).
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_outdoor(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set one-push white balance mode.
    ///
    /// Performs a single white balance calibration based on the current scene.
    /// Point the camera at a white or neutral gray reference in the scene
    /// when using this mode for best results.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_one_push(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto tracking white balance (ATW).
    ///
    /// Auto tracking white balance continuously adapts to changing lighting
    /// conditions in real-time. This is useful for environments with
    /// varying or mixed lighting sources. (Sony FR7 specific feature)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_atw(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set manual white balance mode.
    ///
    /// In manual white balance mode, color balance parameters must be
    /// adjusted manually. The camera will not automatically correct
    /// for different lighting conditions.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_manual(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set color temperature white balance mode.
    ///
    /// Allows setting white balance based on a specific color temperature value.
    /// This mode provides precise control for matching specific lighting conditions.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto white balance sensitivity level.
    ///
    /// Controls how responsive the auto white balance system is to changes
    /// in lighting conditions. Higher sensitivity means faster adaptation
    /// but may cause color instability. (PtzOptics specific feature)
    ///
    /// # Parameters
    /// - `sensitivity`: The AWB sensitivity level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
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
