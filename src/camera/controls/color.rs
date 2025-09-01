//! Color adjustment methods for cameras.

use crate::{
    command::color::{
        BlueGain, BlueTuningCommand, ColorTemperature, OnePushTriggerCommand, RedGain,
        RedTuningCommand,
    },
    types::{BlueChannel, BlueTuning, ColorTemp, RedChannel, RedTuning},
    Error,
};

/// Color operations for cameras.
///
/// This trait provides color control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait ColorControl {
    /// Trigger one-push white balance.
    #[cfg(feature = "async")]
    fn one_push_trigger(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Trigger one-push white balance.
    #[cfg(not(feature = "async"))]
    fn one_push_trigger(&mut self) -> Result<(), Error>;

    /// Set color temperature.
    #[cfg(feature = "async")]
    fn set_color_temperature(
        &self,
        temp: ColorTemp,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set color temperature.
    #[cfg(not(feature = "async"))]
    fn set_color_temperature(&mut self, temp: ColorTemp) -> Result<(), Error>;

    /// Reset color temperature to default value.
    #[cfg(feature = "async")]
    fn reset_color_temperature(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset color temperature to default value.
    #[cfg(not(feature = "async"))]
    fn reset_color_temperature(&mut self) -> Result<(), Error>;

    /// Increase color temperature (makes image cooler/bluer).
    #[cfg(feature = "async")]
    fn increase_color_temperature(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Increase color temperature (makes image cooler/bluer).
    #[cfg(not(feature = "async"))]
    fn increase_color_temperature(&mut self) -> Result<(), Error>;

    /// Decrease color temperature (makes image warmer/redder).
    #[cfg(feature = "async")]
    fn decrease_color_temperature(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Decrease color temperature (makes image warmer/redder).
    #[cfg(not(feature = "async"))]
    fn decrease_color_temperature(&mut self) -> Result<(), Error>;

    /// Set or query color temperature.
    #[cfg(feature = "async")]
    fn color_temperature(
        &self,
        temp: Option<ColorTemp>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set or query color temperature.
    #[cfg(not(feature = "async"))]
    fn color_temperature(&mut self, temp: Option<ColorTemp>) -> Result<(), Error>;

    /// Set red gain.
    #[cfg(feature = "async")]
    fn set_red_gain(
        &self,
        gain: RedChannel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set red gain.
    #[cfg(not(feature = "async"))]
    fn set_red_gain(&mut self, gain: RedChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease red gain.
    #[cfg(feature = "async")]
    fn red_gain(
        &self,
        command: RedGain,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set, reset, increase or decrease red gain.
    #[cfg(not(feature = "async"))]
    fn red_gain(&mut self, command: RedGain) -> Result<(), Error>;

    /// Set blue gain.
    #[cfg(feature = "async")]
    fn set_blue_gain(
        &self,
        gain: BlueChannel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set blue gain.
    #[cfg(not(feature = "async"))]
    fn set_blue_gain(&mut self, gain: BlueChannel) -> Result<(), Error>;

    /// Set, reset, increase or decrease blue gain.
    #[cfg(feature = "async")]
    fn blue_gain(
        &self,
        command: BlueGain,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set, reset, increase or decrease blue gain.
    #[cfg(not(feature = "async"))]
    fn blue_gain(&mut self, command: BlueGain) -> Result<(), Error>;

    /// Set red tuning.
    #[cfg(feature = "async")]
    fn set_red_tuning(
        &self,
        tuning: RedTuning,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set red tuning.
    #[cfg(not(feature = "async"))]
    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error>;

    /// Set blue tuning.
    #[cfg(feature = "async")]
    fn set_blue_tuning(
        &self,
        tuning: BlueTuning,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set blue tuning.
    #[cfg(not(feature = "async"))]
    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async color control trait (deprecated, use ColorControl instead).
/// Blocking color control trait (deprecated, use ColorControl instead).
// Async implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> ColorControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> ColorControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn one_push_trigger(&mut self) -> Result<(), Error> {
        pollster::block_on(self.send_command(&OnePushTriggerCommand))?;
        Ok(())
    }
    fn set_color_temperature(&mut self, temp: ColorTemp) -> Result<(), Error> {
        pollster::block_on(self.send_command(&ColorTemperature::SetTemperature(temp)))?;
        Ok(())
    }

    fn reset_color_temperature(&mut self) -> Result<(), Error> {
        pollster::block_on(self.send_command(&ColorTemperature::Reset))?;
        Ok(())
    }

    fn increase_color_temperature(&mut self) -> Result<(), Error> {
        pollster::block_on(self.send_command(&ColorTemperature::Up))?;
        Ok(())
    }

    fn decrease_color_temperature(&mut self) -> Result<(), Error> {
        pollster::block_on(self.send_command(&ColorTemperature::Down))?;
        Ok(())
    }

    fn color_temperature(&mut self, temp: Option<ColorTemp>) -> Result<(), Error> {
        match temp {
            Some(t) => pollster::block_on(self.send_command(&ColorTemperature::SetTemperature(t)))?,
            None => {
                return Err(Error::Unsupported);
            }
        };
        Ok(())
    }

    fn set_red_gain(&mut self, gain: RedChannel) -> Result<(), Error> {
        pollster::block_on(self.send_command(&RedGain::SetValue(gain)))?;
        Ok(())
    }

    fn red_gain(&mut self, command: RedGain) -> Result<(), Error> {
        pollster::block_on(self.send_command(&command))?;
        Ok(())
    }

    fn set_blue_gain(&mut self, gain: BlueChannel) -> Result<(), Error> {
        pollster::block_on(self.send_command(&BlueGain::SetValue(gain)))?;
        Ok(())
    }

    fn blue_gain(&mut self, command: BlueGain) -> Result<(), Error> {
        pollster::block_on(self.send_command(&command))?;
        Ok(())
    }

    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error> {
        pollster::block_on(self.send_command(&RedTuningCommand::new(tuning)))?;
        Ok(())
    }

    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error> {
        pollster::block_on(self.send_command(&BlueTuningCommand::new(tuning)))?;
        Ok(())
    }
}
