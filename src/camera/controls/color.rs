//! Unified color adjustment implementation using Mode trait.

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

/// Color operations for cameras.
///
/// This trait provides color control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait ColorControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Trigger one-push white balance.
    fn one_push_trigger(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set color temperature.
    fn set_color_temperature(
        &self,
        temp: ColorTemp,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset color temperature to default value.
    fn reset_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase color temperature (makes image cooler/bluer).
    fn increase_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease color temperature (makes image warmer/redder).
    fn decrease_color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

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

    fn one_push_trigger(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(OnePushTriggerCommand)
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

    fn set_red_tuning(&self, tuning: RedTuning) -> M::Fut<'_, Result<(), Error>> {
        self.execute(RedTuningCommand::new(tuning))
    }

    fn set_blue_tuning(&self, tuning: BlueTuning) -> M::Fut<'_, Result<(), Error>> {
        self.execute(BlueTuningCommand::new(tuning))
    }
}
