//! Color control implementation for PTZ cameras.
//!
//! This module provides comprehensive color adjustment functionality including:
//! - Color temperature control for white balance fine-tuning
//! - Red and blue gain adjustment for color balance correction
//! - Red and blue tuning for precise color calibration
//! - One-push white balance calibration
//! - Individual color channel control
//!
//! Color controls allow precise adjustment of the camera's color reproduction
//! to match specific lighting conditions or achieve desired artistic effects.
//! These controls work in conjunction with white balance settings to provide
//! comprehensive color management.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::color::{
        BlueGain, BlueTuningCommand, ColorTemperature, OnePushTriggerCommand, RedGain,
        RedTuningCommand,
    },
    command::white_balance::WhiteBalanceCommand,
    mode::Mode,
    types::{BlueChannel, BlueTuning, ColorTemp, RedChannel, RedTuning},
    Error,
};

/// Color operations for PTZ cameras.
///
/// This trait provides comprehensive color control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Color Adjustment Types
///
/// - **Color Temperature**: Overall color warmth/coolness (measured in Kelvin)
/// - **Red Gain**: Intensity of the red color channel
/// - **Blue Gain**: Intensity of the blue color channel
/// - **Red Tuning**: Fine adjustment of red color reproduction
/// - **Blue Tuning**: Fine adjustment of blue color reproduction
/// - **One-Push**: Automatic color calibration based on current scene
///
/// # Color Temperature Scale
///
/// - Lower values (2000-3000K): Warm/reddish light (incandescent, candlelight)
/// - Medium values (4000-5000K): Neutral light (fluorescent, LED)
/// - Higher values (6000-8000K): Cool/bluish light (daylight, overcast)
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.set_color_temperature(ColorTemp::new(5600)?)?;  // Daylight
/// camera.set_red_gain(RedChannel::new(128)?)?;  // Adjust red channel
/// camera.one_push_trigger()?;  // Auto calibrate
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.set_color_temperature(ColorTemp::new(5600)?).await?;  // Daylight
/// camera.set_red_gain(RedChannel::new(128)?).await?;  // Adjust red channel
/// camera.one_push_trigger().await?;  // Auto calibrate
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait ColorControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;
}

/// One-push white balance operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait OnePushWhiteBalanceControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set one-push white balance mode.
    fn white_balance_one_push(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Trigger one-push white balance calibration.
    fn one_push_trigger(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Color-temperature operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ColorTemperatureControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set color-temperature white balance mode.
    fn white_balance_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set color temperature.
    fn set_color_temperature(
        &self,
        temp: ColorTemp,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset color temperature to default value.
    fn reset_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase color temperature.
    fn increase_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease color temperature.
    fn decrease_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Manual red/blue gain operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait RgbGainControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set red gain.
    fn set_red_gain(&self, gain: RedChannel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Control red gain (set, reset, increase or decrease).
    fn control_red_gain(
        &self,
        command: RedGain,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set blue gain.
    fn set_blue_gain(&self, gain: BlueChannel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Control blue gain (set, reset, increase or decrease).
    fn control_blue_gain(
        &self,
        command: BlueGain,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Red/blue tuning operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait RgbTuningControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set red tuning.
    fn set_red_tuning(&self, tuning: RedTuning)
        -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set blue tuning.
    fn set_blue_tuning(
        &self,
        tuning: BlueTuning,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ColorControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;
}

impl<M, P, Tr, Exec> OnePushWhiteBalanceControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasOnePushWhiteBalance,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn white_balance_one_push(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(WhiteBalanceCommand {
            mode: crate::command::white_balance::WhiteBalanceMode::OnePush,
        })
    }

    fn one_push_trigger(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(OnePushTriggerCommand)
    }
}

impl<M, P, Tr, Exec> ColorTemperatureControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasColorTemperature,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn white_balance_color_temperature(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(WhiteBalanceCommand {
            mode: crate::command::white_balance::WhiteBalanceMode::ColorTemperature,
        })
    }

    fn set_color_temperature(&self, temp: ColorTemp) -> M::Fut<'_, Result<(), Error>> {
        self.execute(ColorTemperature::SetTemperature(temp))
    }

    fn reset_color_temperature(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(ColorTemperature::Reset)
    }

    fn increase_color_temperature(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(ColorTemperature::Up)
    }

    fn decrease_color_temperature(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(ColorTemperature::Down)
    }
}

impl<M, P, Tr, Exec> RgbGainControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbGain,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_red_gain(&self, gain: RedChannel) -> M::Fut<'_, Result<(), Error>> {
        self.execute(RedGain::SetValue(gain))
    }

    fn control_red_gain(&self, command: RedGain) -> M::Fut<'_, Result<(), Error>> {
        self.execute(command)
    }

    fn set_blue_gain(&self, gain: BlueChannel) -> M::Fut<'_, Result<(), Error>> {
        self.execute(BlueGain::SetValue(gain))
    }

    fn control_blue_gain(&self, command: BlueGain) -> M::Fut<'_, Result<(), Error>> {
        self.execute(command)
    }
}

impl<M, P, Tr, Exec> RgbTuningControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbTuning,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_red_tuning(&self, tuning: RedTuning) -> M::Fut<'_, Result<(), Error>> {
        self.execute(RedTuningCommand::new(tuning))
    }

    fn set_blue_tuning(&self, tuning: BlueTuning) -> M::Fut<'_, Result<(), Error>> {
        self.execute(BlueTuningCommand::new(tuning))
    }
}
