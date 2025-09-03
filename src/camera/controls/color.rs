//! Unified color adjustment implementation using Mode trait.

use crate::{
    camera::CameraSend,
    command::color::{
        BlueGain, BlueTuningCommand, ColorTemperature, OnePushTriggerCommand, RedGain,
        RedTuningCommand,
    },
    mode::Mode,
    types::{BlueChannel, BlueTuning, ColorTemp, RedChannel, RedTuning},
    Error,
};

/// Unified color operations for cameras.
///
/// This trait provides color control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
pub trait ColorControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Trigger one-push white balance.
    fn one_push_trigger(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set color temperature.
    fn set_color_temperature(
        &self,
        temp: ColorTemp,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Reset color temperature to default value.
    fn reset_color_temperature(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Increase color temperature (makes image cooler/bluer).
    fn increase_color_temperature(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Decrease color temperature (makes image warmer/redder).
    fn decrease_color_temperature(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set or query color temperature.
    fn color_temperature(
        &self,
        temp: Option<ColorTemp>,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set red gain.
    fn set_red_gain(&self, gain: RedChannel) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set, reset, increase or decrease red gain.
    fn red_gain(&self, command: RedGain) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set blue gain.
    fn set_blue_gain(&self, gain: BlueChannel) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set, reset, increase or decrease blue gain.
    fn blue_gain(&self, command: BlueGain) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set red tuning.
    fn set_red_tuning(&self, tuning: RedTuning)
        -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set blue tuning.
    fn set_blue_tuning(
        &self,
        tuning: BlueTuning,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ColorControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CameraSend<M>,
{
    type Mode = M;

    fn one_push_trigger(&self) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(OnePushTriggerCommand)
    }

    fn set_color_temperature(&self, temp: ColorTemp) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(ColorTemperature::SetTemperature(temp))
    }

    fn reset_color_temperature(&self) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(ColorTemperature::Reset)
    }

    fn increase_color_temperature(&self) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(ColorTemperature::Up)
    }

    fn decrease_color_temperature(&self) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(ColorTemperature::Down)
    }

    fn color_temperature(&self, temp: Option<ColorTemp>) -> M::Ret<'_, Result<(), Error>> {
        match temp {
            Some(t) => self.send_and_complete(ColorTemperature::SetTemperature(t)),
            None => self.error(Error::Unsupported),
        }
    }

    fn set_red_gain(&self, gain: RedChannel) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(RedGain::SetValue(gain))
    }

    fn red_gain(&self, command: RedGain) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(command)
    }

    fn set_blue_gain(&self, gain: BlueChannel) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(BlueGain::SetValue(gain))
    }

    fn blue_gain(&self, command: BlueGain) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(command)
    }

    fn set_red_tuning(&self, tuning: RedTuning) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(RedTuningCommand::new(tuning))
    }

    fn set_blue_tuning(&self, tuning: BlueTuning) -> M::Ret<'_, Result<(), Error>> {
        self.send_and_complete(BlueTuningCommand::new(tuning))
    }
}
