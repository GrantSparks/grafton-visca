//! Color adjustment methods for cameras using the new GAT architecture.

use crate::{
    camera::Camera,
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

/// Color operations (async).
pub trait ColorOps: Sized {
    /// Trigger one-push white balance.
    async fn one_push_trigger(&self) -> Result<(), Error>;

    /// Set color temperature.
    async fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error>;

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
pub trait ColorOpsBlocking: Sized {
    /// Trigger one-push white balance.
    fn one_push_trigger(&self) -> Result<(), Error>;

    /// Set color temperature.
    fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error>;

    /// Set or query color temperature.
    fn color_temperature(&self, temp: Option<ColorTemp>) -> Result<(), Error>;

    /// Set red gain.
    fn set_red_gain(&self, gain: RedChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease red gain.
    fn red_gain(&self, command: RedGain) -> Result<(), Error>;

    /// Set blue gain.
    fn set_blue_gain(&self, gain: BlueChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease blue gain.
    fn blue_gain(&self, command: BlueGain) -> Result<(), Error>;

    /// Set red tuning.
    fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error>;

    /// Set blue tuning.
    fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error>;
}

// Async implementation
impl ColorOps for Camera {
    async fn one_push_trigger(&self) -> Result<(), Error> {
        self.send_command(&OnePushTriggerCommand::new()).await?;
        Ok(())
    }

    async fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error> {
        self.send_command(&ColorTemperature::SetTemperature(temp))
            .await?;
        Ok(())
    }

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

// Blocking implementation
impl ColorOpsBlocking for Camera {
    fn one_push_trigger(&self) -> Result<(), Error> {
        self.send_command_blocking(&OnePushTriggerCommand::new())?;
        Ok(())
    }

    fn set_color_temperature(&self, temp: ColorTemp) -> Result<(), Error> {
        self.send_command_blocking(&ColorTemperature::SetTemperature(temp))?;
        Ok(())
    }

    fn color_temperature(&self, temp: Option<ColorTemp>) -> Result<(), Error> {
        match temp {
            Some(t) => self.send_command_blocking(&ColorTemperature::SetTemperature(t))?,
            None => self.send_command_blocking(&ColorTemperatureInquiry)?,
        };
        Ok(())
    }

    fn set_red_gain(&self, gain: RedChannel) -> Result<(), Error> {
        self.send_command_blocking(&RedGain::SetValue(gain))?;
        Ok(())
    }

    fn red_gain(&self, command: RedGain) -> Result<(), Error> {
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_blue_gain(&self, gain: BlueChannel) -> Result<(), Error> {
        self.send_command_blocking(&BlueGain::SetValue(gain))?;
        Ok(())
    }

    fn blue_gain(&self, command: BlueGain) -> Result<(), Error> {
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error> {
        self.send_command_blocking(&RedTuningCommand::new(tuning))?;
        Ok(())
    }

    fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error> {
        self.send_command_blocking(&BlueTuningCommand::new(tuning))?;
        Ok(())
    }
}
