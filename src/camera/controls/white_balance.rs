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
    command::white_balance::{
        AWBSensitivityCommand, AutoWhiteBalanceSensitivity, WhiteBalanceCommand, WhiteBalanceMode,
    },
    mode::Mode,
    Error,
};

fn white_balance_mode_feature(mode: WhiteBalanceMode) -> &'static str {
    match mode {
        WhiteBalanceMode::Auto => "Auto white balance mode",
        WhiteBalanceMode::Indoor => "Indoor white balance mode",
        WhiteBalanceMode::Outdoor => "Outdoor white balance mode",
        WhiteBalanceMode::ATW => "Auto-tracking white balance mode",
        WhiteBalanceMode::Manual => "Manual white balance mode",
        WhiteBalanceMode::OnePush => "One-push white balance mode",
        WhiteBalanceMode::ColorTemperature => "Color-temperature white balance mode",
    }
}

fn ensure_white_balance_mode_supported<P>(mode: WhiteBalanceMode) -> Result<(), Error>
where
    P: crate::capabilities::white_balance::WhiteBalance,
{
    if mode == WhiteBalanceMode::OnePush && !P::SUPPORTS_ONE_PUSH_WB {
        return Err(Error::FeatureNotSupported {
            feature: white_balance_mode_feature(mode),
        });
    }

    if mode == WhiteBalanceMode::ColorTemperature && !P::SUPPORTS_COLOR_TEMP {
        return Err(Error::FeatureNotSupported {
            feature: white_balance_mode_feature(mode),
        });
    }

    if P::WB_MODES.contains(&mode) {
        Ok(())
    } else {
        Err(Error::FeatureNotSupported {
            feature: white_balance_mode_feature(mode),
        })
    }
}

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

    /// Set manual white balance mode.
    ///
    /// In manual white balance mode, color balance parameters must be
    /// adjusted manually. The camera will not automatically correct
    /// for different lighting conditions.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn white_balance_manual(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Auto-tracking white balance operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait AutoTrackingWhiteBalanceControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set auto-tracking white balance mode.
    fn white_balance_atw(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Auto white-balance sensitivity operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait AutoWhiteBalanceSensitivityControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set auto white-balance sensitivity level.
    fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

impl<M, P, Tr, Exec> WhiteBalanceControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::white_balance::WhiteBalance,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> M::Fut<'_, Result<(), Error>> {
        if let Err(err) = ensure_white_balance_mode_supported::<P>(mode) {
            return self.error(err);
        }

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

    fn white_balance_manual(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::Manual)
    }
}

impl<M, P, Tr, Exec> AutoTrackingWhiteBalanceControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasAutoTrackingWhiteBalance,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn white_balance_atw(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_white_balance_mode(WhiteBalanceMode::ATW)
    }
}

impl<M, P, Tr, Exec> AutoWhiteBalanceSensitivityControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasAutoWhiteBalanceSensitivity,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = AWBSensitivityCommand { sensitivity };
        self.execute(cmd)
    }
}
