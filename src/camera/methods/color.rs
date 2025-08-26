//! Color adjustment methods for cameras using mode markers.

use crate::{
    command::color::{
        BlueGain, BlueTuningCommand, ColorTemperature, OnePushTriggerCommand, RedGain,
        RedTuningCommand,
    },
    types::{BlueChannel, BlueTuning, ColorTemp, RedChannel, RedTuning},
    Error,
};

/// Color operations (async).
#[cfg(feature = "async")]
pub trait ColorControl: Sized {
    /// Trigger one-push white balance.
    async fn one_push_trigger(&self) -> Result<(), Error>;

    /// Set color temperature.
    async fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error>;

    /// Reset color temperature to default value.
    async fn reset_color_temperature(&self) -> Result<(), Error>;

    /// Increase color temperature (makes image cooler/bluer).
    async fn increase_color_temperature(&self) -> Result<(), Error>;

    /// Decrease color temperature (makes image warmer/redder).
    async fn decrease_color_temperature(&self) -> Result<(), Error>;

    /// Set or query color temperature.
    async fn color_temperature(&self, temp: Option<ColorTemp>) -> Result<(), Error>;

    /// Set red gain.
    async fn set_red_gain(&self, gain: RedChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease red gain.
    async fn red_gain(&self, command: RedGain) -> Result<(), Error>;

    /// Set blue gain.
    async fn set_blue_gain(&self, gain: BlueChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease blue gain.
    async fn blue_gain(&self, command: BlueGain) -> Result<(), Error>;

    /// Set red tuning.
    async fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error>;

    /// Set blue tuning.
    async fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error>;
}

/// Color operations (blocking).
pub trait ColorControlBlocking: Sized {
    /// Trigger one-push white balance.
    fn one_push_trigger(&mut self) -> Result<(), Error>;

    /// Set color temperature.
    fn set_color_temperature(&mut self, temp: ColorTemp) -> Result<(), Error>;

    /// Reset color temperature to default value.
    fn reset_color_temperature(&mut self) -> Result<(), Error>;

    /// Increase color temperature (makes image cooler/bluer).
    fn increase_color_temperature(&mut self) -> Result<(), Error>;

    /// Decrease color temperature (makes image warmer/redder).
    fn decrease_color_temperature(&mut self) -> Result<(), Error>;

    /// Set or query color temperature.
    fn color_temperature(&mut self, temp: Option<ColorTemp>) -> Result<(), Error>;

    /// Set red gain.
    fn set_red_gain(&mut self, gain: RedChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease red gain.
    fn red_gain(&mut self, command: RedGain) -> Result<(), Error>;

    /// Set blue gain.
    fn set_blue_gain(&mut self, gain: BlueChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease blue gain.
    fn blue_gain(&mut self, command: BlueGain) -> Result<(), Error>;

    /// Set red tuning.
    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error>;

    /// Set blue tuning.
    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> ColorControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn one_push_trigger(&self) -> Result<(), Error> {
        self.send_command(&OnePushTriggerCommand).await?;
        Ok(())
    }

    async fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error> {
        self.send_command(&ColorTemperature::SetTemperature(temp))
            .await?;
        Ok(())
    }

    async fn reset_color_temperature(&self) -> Result<(), Error> {
        self.send_command(&ColorTemperature::Reset).await?;
        Ok(())
    }

    async fn increase_color_temperature(&self) -> Result<(), Error> {
        self.send_command(&ColorTemperature::Up).await?;
        Ok(())
    }

    async fn decrease_color_temperature(&self) -> Result<(), Error> {
        self.send_command(&ColorTemperature::Down).await?;
        Ok(())
    }

    async fn color_temperature(&self, temp: Option<ColorTemp>) -> Result<(), Error> {
        match temp {
            Some(t) => {
                self.send_command(&ColorTemperature::SetTemperature(t))
                    .await?
            }
            None => {
                return Err(Error::Unsupported);
            }
        };
        Ok(())
    }

    async fn set_red_gain(&self, gain: RedChannel) -> Result<(), Error> {
        self.send_command(&RedGain::SetValue(gain)).await?;
        Ok(())
    }

    async fn red_gain(&self, command: RedGain) -> Result<(), Error> {
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_blue_gain(&self, gain: BlueChannel) -> Result<(), Error> {
        self.send_command(&BlueGain::SetValue(gain)).await?;
        Ok(())
    }

    async fn blue_gain(&self, command: BlueGain) -> Result<(), Error> {
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error> {
        self.send_command(&RedTuningCommand::new(tuning)).await?;
        Ok(())
    }

    async fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error> {
        self.send_command(&BlueTuningCommand::new(tuning)).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> ColorControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + Default,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn one_push_trigger(&mut self) -> Result<(), Error> {
        self.send_command(&OnePushTriggerCommand)?;
        Ok(())
    }

    fn set_color_temperature(&mut self, temp: ColorTemp) -> Result<(), Error> {
        self.send_command(&ColorTemperature::SetTemperature(temp))?;
        Ok(())
    }

    fn reset_color_temperature(&mut self) -> Result<(), Error> {
        self.send_command(&ColorTemperature::Reset)?;
        Ok(())
    }

    fn increase_color_temperature(&mut self) -> Result<(), Error> {
        self.send_command(&ColorTemperature::Up)?;
        Ok(())
    }

    fn decrease_color_temperature(&mut self) -> Result<(), Error> {
        self.send_command(&ColorTemperature::Down)?;
        Ok(())
    }

    fn color_temperature(&mut self, temp: Option<ColorTemp>) -> Result<(), Error> {
        match temp {
            Some(t) => self.send_command(&ColorTemperature::SetTemperature(t))?,
            None => {
                return Err(Error::Unsupported);
            }
        };
        Ok(())
    }

    fn set_red_gain(&mut self, gain: RedChannel) -> Result<(), Error> {
        self.send_command(&RedGain::SetValue(gain))?;
        Ok(())
    }

    fn red_gain(&mut self, command: RedGain) -> Result<(), Error> {
        self.send_command(&command)?;
        Ok(())
    }

    fn set_blue_gain(&mut self, gain: BlueChannel) -> Result<(), Error> {
        self.send_command(&BlueGain::SetValue(gain))?;
        Ok(())
    }

    fn blue_gain(&mut self, command: BlueGain) -> Result<(), Error> {
        self.send_command(&command)?;
        Ok(())
    }

    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error> {
        self.send_command(&RedTuningCommand::new(tuning))?;
        Ok(())
    }

    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error> {
        self.send_command(&BlueTuningCommand::new(tuning))?;
        Ok(())
    }
}
