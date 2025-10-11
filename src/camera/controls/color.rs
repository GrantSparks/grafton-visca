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

    /// Trigger one-push white balance.
    ///
    /// Performs an automatic color calibration based on the current scene.
    /// This analyzes the image and adjusts color parameters to achieve
    /// neutral whites under the current lighting conditions.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn one_push_trigger(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set color temperature.
    ///
    /// Sets the color temperature to a specific value measured in Kelvin.
    /// This controls the overall warmth or coolness of the image.
    ///
    /// # Parameters
    /// - `temp`: The color temperature in Kelvin (typically 2000-8000K)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_color_temperature(
        &self,
        temp: ColorTemp,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset color temperature to default value.
    ///
    /// Resets the color temperature to the camera's default setting,
    /// typically around 5600K (daylight).
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_color_temperature(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase color temperature (makes image cooler/bluer).
    ///
    /// Raises the color temperature by one step, making the image appear
    /// cooler with a bluish tint. This compensates for warm lighting.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_color_temperature(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease color temperature (makes image warmer/redder).
    ///
    /// Lowers the color temperature by one step, making the image appear
    /// warmer with a reddish tint. This compensates for cool lighting.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_color_temperature(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set red gain.
    ///
    /// Sets the intensity of the red color channel to a specific value.
    /// This allows fine-tuning of red color reproduction in the image.
    ///
    /// # Parameters
    /// - `gain`: The red channel gain value to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_red_gain(
        &self,
        gain: RedChannel,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Control red gain (set, reset, increase or decrease).
    ///
    /// Provides comprehensive control over the red color channel using
    /// different command types (absolute value, relative adjustments, reset).
    ///
    /// # Parameters
    /// - `command`: The red gain command to execute
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn control_red_gain(
        &self,
        command: RedGain,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set blue gain.
    ///
    /// Sets the intensity of the blue color channel to a specific value.
    /// This allows fine-tuning of blue color reproduction in the image.
    ///
    /// # Parameters
    /// - `gain`: The blue channel gain value to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_blue_gain(
        &self,
        gain: BlueChannel,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Control blue gain (set, reset, increase or decrease).
    ///
    /// Provides comprehensive control over the blue color channel using
    /// different command types (absolute value, relative adjustments, reset).
    ///
    /// # Parameters
    /// - `command`: The blue gain command to execute
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn control_blue_gain(
        &self,
        command: BlueGain,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set red tuning.
    ///
    /// Applies fine-tuning adjustments to red color reproduction.
    /// This provides more precise control than gain adjustment alone.
    ///
    /// # Parameters
    /// - `tuning`: The red tuning value to apply
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_red_tuning(
        &self,
        tuning: RedTuning,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set blue tuning.
    ///
    /// Applies fine-tuning adjustments to blue color reproduction.
    /// This provides more precise control than gain adjustment alone.
    ///
    /// # Parameters
    /// - `tuning`: The blue tuning value to apply
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_blue_tuning(
        &self,
        tuning: BlueTuning,
        opts: crate::CommandOptions<'_>,
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

    fn one_push_trigger(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(OnePushTriggerCommand, opts)
    }

    fn set_color_temperature(
        &self,
        temp: ColorTemp,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(ColorTemperature::SetTemperature(temp), opts)
    }

    fn reset_color_temperature(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(ColorTemperature::Reset, opts)
    }

    fn increase_color_temperature(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(ColorTemperature::Up, opts)
    }

    fn decrease_color_temperature(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(ColorTemperature::Down, opts)
    }

    fn set_red_gain(
        &self,
        gain: RedChannel,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(RedGain::SetValue(gain), opts)
    }

    fn control_red_gain(
        &self,
        command: RedGain,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(command, opts)
    }

    fn set_blue_gain(
        &self,
        gain: BlueChannel,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(BlueGain::SetValue(gain), opts)
    }

    fn control_blue_gain(
        &self,
        command: BlueGain,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(command, opts)
    }

    fn set_red_tuning(
        &self,
        tuning: RedTuning,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(RedTuningCommand::new(tuning), opts)
    }

    fn set_blue_tuning(
        &self,
        tuning: BlueTuning,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(BlueTuningCommand::new(tuning), opts)
    }
}
