//! Color adjustment methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    command::{
        color::{
            BlueGain, BlueTuningCommand, ColorTemperature, OnePushTriggerCommand, RedGain,
            RedTuningCommand,
        },
        inquiry::ColorTemperatureInquiry,
    },
    types::{BlueChannel, BlueTuning, ColorTemp, RedChannel, RedTuning},
    Error,
};

/// Unified trait for Camera that adds color adjustment methods.
pub trait ColorOps: Sized {
    // Async methods
    #[cfg(feature = "tokio")]
    /// Trigger one-push white balance.
    async fn one_push_trigger(&self) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set color temperature.
    async fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set or query color temperature.
    async fn color_temperature(&self, temp: Option<ColorTemp>) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set red gain.
    async fn set_red_gain(&self, gain: RedChannel) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set, reset, increase or decrease red gain.
    async fn red_gain(&self, command: RedGain) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set blue gain.
    async fn set_blue_gain(&self, gain: BlueChannel) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set, reset, increase or decrease blue gain.
    async fn blue_gain(&self, command: BlueGain) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set red tuning.
    async fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error>;

    #[cfg(feature = "tokio")]
    /// Set blue tuning.
    async fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error>;

    // Blocking methods
    #[cfg(not(feature = "tokio"))]
    /// Trigger one-push white balance.
    fn one_push_trigger_blocking(&mut self) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set color temperature.
    fn set_color_temperature_blocking(&mut self, temp: ColorTemp) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set or query color temperature.
    fn color_temperature_blocking(&mut self, temp: Option<ColorTemp>) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set red gain.
    fn set_red_gain_blocking(&mut self, gain: RedChannel) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set, reset, increase or decrease red gain.
    fn red_gain_blocking(&mut self, command: RedGain) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set blue gain.
    fn set_blue_gain_blocking(&mut self, gain: BlueChannel) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set, reset, increase or decrease blue gain.
    fn blue_gain_blocking(&mut self, command: BlueGain) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set red tuning.
    fn set_red_tuning_blocking(&mut self, tuning: RedTuning) -> Result<(), Error>;

    #[cfg(not(feature = "tokio"))]
    /// Set blue tuning.
    fn set_blue_tuning_blocking(&mut self, tuning: BlueTuning) -> Result<(), Error>;
}

impl ColorOps for Camera {
    #[cfg(feature = "tokio")]
    async fn one_push_trigger(&self) -> Result<(), Error> {
        self.send_command(&OnePushTriggerCommand::new()).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn one_push_trigger_blocking(&mut self) -> Result<(), Error> {
        self.send_command_blocking(&OnePushTriggerCommand::new())?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error> {
        self.send_command(&ColorTemperature::SetTemperature(temp))
            .await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_color_temperature_blocking(&mut self, temp: ColorTemp) -> Result<(), Error> {
        self.send_command_blocking(&ColorTemperature::SetTemperature(temp))?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn color_temperature(&self, temp: Option<ColorTemp>) -> Result<(), Error> {
        match temp {
            Some(t) => {
                self.send_command(&ColorTemperature::SetTemperature(t))
                    .await?
            }
            None => self.send_command(&ColorTemperatureInquiry).await?,
        };
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn color_temperature_blocking(&mut self, temp: Option<ColorTemp>) -> Result<(), Error> {
        match temp {
            Some(t) => self.send_command_blocking(&ColorTemperature::SetTemperature(t))?,
            None => self.send_command_blocking(&ColorTemperatureInquiry)?,
        };
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_red_gain(&self, gain: RedChannel) -> Result<(), Error> {
        self.send_command(&RedGain::SetValue(gain)).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_red_gain_blocking(&mut self, gain: RedChannel) -> Result<(), Error> {
        self.send_command_blocking(&RedGain::SetValue(gain))?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn red_gain(&self, command: RedGain) -> Result<(), Error> {
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn red_gain_blocking(&mut self, command: RedGain) -> Result<(), Error> {
        self.send_command_blocking(&command)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_blue_gain(&self, gain: BlueChannel) -> Result<(), Error> {
        self.send_command(&BlueGain::SetValue(gain)).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_blue_gain_blocking(&mut self, gain: BlueChannel) -> Result<(), Error> {
        self.send_command_blocking(&BlueGain::SetValue(gain))?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn blue_gain(&self, command: BlueGain) -> Result<(), Error> {
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn blue_gain_blocking(&mut self, command: BlueGain) -> Result<(), Error> {
        self.send_command_blocking(&command)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error> {
        self.send_command(&RedTuningCommand::new(tuning)).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_red_tuning_blocking(&mut self, tuning: RedTuning) -> Result<(), Error> {
        self.send_command_blocking(&RedTuningCommand::new(tuning))?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error> {
        self.send_command(&BlueTuningCommand::new(tuning)).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_blue_tuning_blocking(&mut self, tuning: BlueTuning) -> Result<(), Error> {
        self.send_command_blocking(&BlueTuningCommand::new(tuning))?;
        Ok(())
    }
}
