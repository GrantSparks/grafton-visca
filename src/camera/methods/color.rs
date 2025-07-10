//! Color adjustment methods for cameras.

use crate::camera::Camera;
use crate::capabilities::ProfileMetadata;
use crate::command::color::{
    BlueGainCommand, BlueTuningCommand, ColorTemperatureCommand, OnePushTriggerCommand,
    RedGainCommand, RedTuningCommand,
};
use crate::command::Command;
use crate::types::{BlueGain, BlueTuning, ColorTemperature, RedGain, RedTuning};
use crate::Error;

/// Extension trait that adds color adjustment methods to cameras.
#[allow(async_fn_in_trait)]
pub trait ColorMethodsExt {
    /// Trigger one-push white balance.
    #[cfg(not(feature = "async"))]
    fn one_push_trigger(&mut self) -> Result<(), Error>;

    /// Trigger one-push white balance.
    #[cfg(feature = "async")]
    async fn one_push_trigger(&self) -> Result<(), Error>;

    /// Set color temperature.
    #[cfg(not(feature = "async"))]
    fn set_color_temperature(&mut self, temp: ColorTemperature) -> Result<(), Error>;

    /// Set color temperature.
    #[cfg(feature = "async")]
    async fn set_color_temperature(&self, temp: ColorTemperature) -> Result<(), Error>;

    /// Set or query color temperature.
    #[cfg(not(feature = "async"))]
    fn color_temperature(&mut self, temp: Option<ColorTemperature>) -> Result<(), Error>;

    /// Set or query color temperature.
    #[cfg(feature = "async")]
    async fn color_temperature(&self, temp: Option<ColorTemperature>) -> Result<(), Error>;

    /// Set red gain.
    #[cfg(not(feature = "async"))]
    fn set_red_gain(&mut self, gain: RedGain) -> Result<(), Error>;

    /// Set red gain.
    #[cfg(feature = "async")]
    async fn set_red_gain(&self, gain: RedGain) -> Result<(), Error>;

    /// Set, reset, increase or decrease red gain.
    #[cfg(not(feature = "async"))]
    fn red_gain(&mut self, command: RedGainCommand) -> Result<(), Error>;

    /// Set, reset, increase or decrease red gain.
    #[cfg(feature = "async")]
    async fn red_gain(&self, command: RedGainCommand) -> Result<(), Error>;

    /// Set blue gain.
    #[cfg(not(feature = "async"))]
    fn set_blue_gain(&mut self, gain: BlueGain) -> Result<(), Error>;

    /// Set blue gain.
    #[cfg(feature = "async")]
    async fn set_blue_gain(&self, gain: BlueGain) -> Result<(), Error>;

    /// Set, reset, increase or decrease blue gain.
    #[cfg(not(feature = "async"))]
    fn blue_gain(&mut self, command: BlueGainCommand) -> Result<(), Error>;

    /// Set, reset, increase or decrease blue gain.
    #[cfg(feature = "async")]
    async fn blue_gain(&self, command: BlueGainCommand) -> Result<(), Error>;

    /// Set red tuning.
    #[cfg(not(feature = "async"))]
    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error>;

    /// Set red tuning.
    #[cfg(feature = "async")]
    async fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error>;

    /// Set blue tuning.
    #[cfg(not(feature = "async"))]
    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error>;

    /// Set blue tuning.
    #[cfg(feature = "async")]
    async fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error>;
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P, T> ColorMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::blocking::BlockingTransport,
{
    fn one_push_trigger(&mut self) -> Result<(), Error> {
        let response_bytes = self.transport.send_blocking(&OnePushTriggerCommand.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn set_color_temperature(&mut self, temp: ColorTemperature) -> Result<(), Error> {
        let cmd = ColorTemperatureCommand::SetTemperature(temp);
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn color_temperature(&mut self, temp: Option<ColorTemperature>) -> Result<(), Error> {
        match temp {
            Some(t) => {
                let cmd = ColorTemperatureCommand::SetTemperature(t);
                let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
                crate::command::Response::parse(&response_bytes)?.into_result()
            },
            None => {
                use crate::command::inquiry::InquiryCommand;
                let cmd = InquiryCommand::ColorTemperature;
                let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
                crate::command::Response::parse(&response_bytes)?.into_result()
            }
        }
    }

    fn set_red_gain(&mut self, gain: RedGain) -> Result<(), Error> {
        let cmd = RedGainCommand::SetValue(gain);
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn red_gain(&mut self, command: RedGainCommand) -> Result<(), Error> {
        let response_bytes = self.transport.send_blocking(&command.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn set_blue_gain(&mut self, gain: BlueGain) -> Result<(), Error> {
        let cmd = BlueGainCommand::SetValue(gain);
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn blue_gain(&mut self, command: BlueGainCommand) -> Result<(), Error> {
        let response_bytes = self.transport.send_blocking(&command.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error> {
        let cmd = RedTuningCommand { level: tuning };
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error> {
        let cmd = BlueTuningCommand { level: tuning };
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> ColorMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::AsyncTransport,
{
    async fn one_push_trigger(&self) -> Result<(), Error> {
        let response_bytes = self.transport.send_async(&OnePushTriggerCommand.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn set_color_temperature(&self, temp: ColorTemperature) -> Result<(), Error> {
        let cmd = ColorTemperatureCommand::SetTemperature(temp);
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn color_temperature(&self, temp: Option<ColorTemperature>) -> Result<(), Error> {
        match temp {
            Some(t) => {
                let cmd = ColorTemperatureCommand::SetTemperature(t);
                let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
                crate::command::Response::parse(&response_bytes)?.into_result()
            },
            None => {
                use crate::command::inquiry::InquiryCommand;
                let cmd = InquiryCommand::ColorTemperature;
                let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
                crate::command::Response::parse(&response_bytes)?.into_result()
            }
        }
    }

    async fn set_red_gain(&self, gain: RedGain) -> Result<(), Error> {
        let cmd = RedGainCommand::SetValue(gain);
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn red_gain(&self, command: RedGainCommand) -> Result<(), Error> {
        let response_bytes = self.transport.send_async(&command.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn set_blue_gain(&self, gain: BlueGain) -> Result<(), Error> {
        let cmd = BlueGainCommand::SetValue(gain);
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn blue_gain(&self, command: BlueGainCommand) -> Result<(), Error> {
        let response_bytes = self.transport.send_async(&command.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error> {
        let cmd = RedTuningCommand { level: tuning };
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error> {
        let cmd = BlueTuningCommand { level: tuning };
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::PTZOpticsG2;

    #[test]
    fn test_color_methods_compile() {
        #[derive(Debug)]
        struct MockTransport;

        #[cfg(not(feature = "async"))]
        impl crate::transport::blocking::BlockingTransport for MockTransport {
            fn send(&mut self, _data: &[u8]) -> Result<(), Error> {
                Ok(())
            }
            fn receive(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, Error> {
                Ok(vec![0x90, 0x50, 0xFF])
            }
            fn is_connected(&self) -> bool {
                true
            }
            fn description(&self) -> &str {
                "MockTransport"
            }
        }

        let mut _camera: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);

        #[cfg(not(feature = "async"))]
        {
            let _ = _camera.one_push_trigger();
            let _ = _camera.set_color_temperature(ColorTemperature::new(5600).unwrap());
            let _ = _camera.set_red_gain(RedGain::new(100).unwrap());
            let _ = _camera.set_blue_gain(BlueGain::new(100).unwrap());
        }
    }
}