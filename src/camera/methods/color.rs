//! Color adjustment methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::ProfileMetadata,
    command::{
        color::{
            BlueGainCommand, BlueTuningCommand, ColorTemperatureCommand, OnePushTriggerCommand,
            RedGainCommand, RedTuningCommand,
        },
        inquiry::InquiryCommand,
        Response,
    },
    transport::core::{BlockingTransport, Transport},
    types::{BlueGain, BlueTuning, ColorTemperature, RedGain, RedTuning},
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore that adds color adjustment methods.
pub trait ColorCoreExt<P: ProfileMetadata> {
    /// Trigger one-push white balance.
    fn one_push_trigger(&self) -> impl Future<Output = Result<(), Error>>;

    /// Set color temperature.
    fn set_color_temperature(
        &self,
        temp: ColorTemperature,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Set or query color temperature.
    fn color_temperature(
        &self,
        temp: Option<ColorTemperature>,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Set red gain.
    fn set_red_gain(&self, gain: RedGain) -> impl Future<Output = Result<(), Error>>;

    /// Set, reset, increase or decrease red gain.
    fn red_gain(&self, command: RedGainCommand) -> impl Future<Output = Result<(), Error>>;

    /// Set blue gain.
    fn set_blue_gain(&self, gain: BlueGain) -> impl Future<Output = Result<(), Error>>;

    /// Set, reset, increase or decrease blue gain.
    fn blue_gain(&self, command: BlueGainCommand) -> impl Future<Output = Result<(), Error>>;

    /// Set red tuning.
    fn set_red_tuning(&self, tuning: RedTuning) -> impl Future<Output = Result<(), Error>>;

    /// Set blue tuning.
    fn set_blue_tuning(&self, tuning: BlueTuning) -> impl Future<Output = Result<(), Error>>;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> ColorCoreExt<P> for CameraCore<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn one_push_trigger(&self) -> impl Future<Output = Result<(), Error>> {
        async move {
            match self.send_command(&OnePushTriggerCommand::new()).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_color_temperature(
        &self,
        temp: ColorTemperature,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = ColorTemperatureCommand::SetTemperature(temp);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn color_temperature(
        &self,
        temp: Option<ColorTemperature>,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            match temp {
                Some(t) => {
                    let cmd = ColorTemperatureCommand::SetTemperature(t);
                    match self.send_command(&cmd).await? {
                        Response::Ack => Ok(()),
                        Response::Error(e) => Err(e.into()),
                        _ => Err(Error::UnexpectedResponseType),
                    }
                }
                None => {
                    let cmd = InquiryCommand::ColorTemperature;
                    match self.send_command(&cmd).await? {
                        Response::Ack => Ok(()),
                        Response::Error(e) => Err(e.into()),
                        _ => Err(Error::UnexpectedResponseType),
                    }
                }
            }
        }
    }

    fn set_red_gain(&self, gain: RedGain) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = RedGainCommand::SetValue(gain);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn red_gain(&self, command: RedGainCommand) -> impl Future<Output = Result<(), Error>> {
        async move {
            match self.send_command(&command).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_blue_gain(&self, gain: BlueGain) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = BlueGainCommand::SetValue(gain);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn blue_gain(&self, command: BlueGainCommand) -> impl Future<Output = Result<(), Error>> {
        async move {
            match self.send_command(&command).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_red_tuning(&self, tuning: RedTuning) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = RedTuningCommand::new(tuning);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_blue_tuning(&self, tuning: BlueTuning) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = BlueTuningCommand::new(tuning);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for CameraAsync that adds color adjustment methods.
pub trait ColorAsyncExt<P: ProfileMetadata>: Sized {
    /// Trigger one-push white balance.
    fn one_push_trigger(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set color temperature.
    fn set_color_temperature(
        &self,
        temp: ColorTemperature,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set or query color temperature.
    fn color_temperature(
        &self,
        temp: Option<ColorTemperature>,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set red gain.
    fn set_red_gain(&self, gain: RedGain) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set, reset, increase or decrease red gain.
    fn red_gain(&self, command: RedGainCommand) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set blue gain.
    fn set_blue_gain(&self, gain: BlueGain) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set, reset, increase or decrease blue gain.
    fn blue_gain(&self, command: BlueGainCommand)
        -> impl Future<Output = Result<(), Error>> + Send;

    /// Set red tuning.
    fn set_red_tuning(&self, tuning: RedTuning) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set blue tuning.
    fn set_blue_tuning(&self, tuning: BlueTuning)
        -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> ColorAsyncExt<P> for CameraAsync<P, T>
where
    P: ProfileMetadata + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn one_push_trigger(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().one_push_trigger().await }
    }

    fn set_color_temperature(
        &self,
        temp: ColorTemperature,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_color_temperature(temp).await }
    }

    fn color_temperature(
        &self,
        temp: Option<ColorTemperature>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().color_temperature(temp).await }
    }

    fn set_red_gain(&self, gain: RedGain) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_red_gain(gain).await }
    }

    fn red_gain(&self, command: RedGainCommand) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().red_gain(command).await }
    }

    fn set_blue_gain(&self, gain: BlueGain) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_blue_gain(gain).await }
    }

    fn blue_gain(
        &self,
        command: BlueGainCommand,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().blue_gain(command).await }
    }

    fn set_red_tuning(&self, tuning: RedTuning) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_red_tuning(tuning).await }
    }

    fn set_blue_tuning(
        &self,
        tuning: BlueTuning,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_blue_tuning(tuning).await }
    }
}

/// Extension trait for CameraBlocking that adds color adjustment methods.
pub trait ColorBlockingExt<P: ProfileMetadata>: Sized {
    /// Trigger one-push white balance.
    fn one_push_trigger(&mut self) -> Result<(), Error>;

    /// Set color temperature.
    fn set_color_temperature(&mut self, temp: ColorTemperature) -> Result<(), Error>;

    /// Set or query color temperature.
    fn color_temperature(&mut self, temp: Option<ColorTemperature>) -> Result<(), Error>;

    /// Set red gain.
    fn set_red_gain(&mut self, gain: RedGain) -> Result<(), Error>;

    /// Set, reset, increase or decrease red gain.
    fn red_gain(&mut self, command: RedGainCommand) -> Result<(), Error>;

    /// Set blue gain.
    fn set_blue_gain(&mut self, gain: BlueGain) -> Result<(), Error>;

    /// Set, reset, increase or decrease blue gain.
    fn blue_gain(&mut self, command: BlueGainCommand) -> Result<(), Error>;

    /// Set red tuning.
    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error>;

    /// Set blue tuning.
    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error>;
}

impl<P, T> ColorBlockingExt<P> for CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: BlockingTransport,
{
    fn one_push_trigger(&mut self) -> Result<(), Error> {
        block_on(self.core().one_push_trigger())
    }

    fn set_color_temperature(&mut self, temp: ColorTemperature) -> Result<(), Error> {
        block_on(self.core().set_color_temperature(temp))
    }

    fn color_temperature(&mut self, temp: Option<ColorTemperature>) -> Result<(), Error> {
        block_on(self.core().color_temperature(temp))
    }

    fn set_red_gain(&mut self, gain: RedGain) -> Result<(), Error> {
        block_on(self.core().set_red_gain(gain))
    }

    fn red_gain(&mut self, command: RedGainCommand) -> Result<(), Error> {
        block_on(self.core().red_gain(command))
    }

    fn set_blue_gain(&mut self, gain: BlueGain) -> Result<(), Error> {
        block_on(self.core().set_blue_gain(gain))
    }

    fn blue_gain(&mut self, command: BlueGainCommand) -> Result<(), Error> {
        block_on(self.core().blue_gain(command))
    }

    fn set_red_tuning(&mut self, tuning: RedTuning) -> Result<(), Error> {
        block_on(self.core().set_red_tuning(tuning))
    }

    fn set_blue_tuning(&mut self, tuning: BlueTuning) -> Result<(), Error> {
        block_on(self.core().set_blue_tuning(tuning))
    }
}
