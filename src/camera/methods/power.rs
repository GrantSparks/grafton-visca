//! Power methods for cameras using the new GAT architecture.

use crate::{camera::unified::Camera, command::PowerCommand, Error, Response};

/// Power operations.
pub trait PowerOps: Sized {
    /// Power on the camera.
    #[cfg(feature = "tokio")]
    async fn power_on(&self) -> Result<(), Error>;

    /// Power on the camera. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn power_on_blocking(&mut self) -> Result<(), Error>;

    /// Power off the camera.
    #[cfg(feature = "tokio")]
    async fn power_off(&self) -> Result<(), Error>;

    /// Power off the camera. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn power_off_blocking(&mut self) -> Result<(), Error>;
}

impl PowerOps for Camera {
    #[cfg(feature = "tokio")]
    async fn power_on(&self) -> Result<(), Error> {
        let command = PowerCommand::On;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => {
                // Wait for camera to be ready
                tokio::time::sleep(self.power_on_time()).await;
                Ok(())
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn power_on_blocking(&mut self) -> Result<(), Error> {
        let command = PowerCommand::On;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => {
                // Wait for camera to be ready
                std::thread::sleep(self.power_on_time());
                Ok(())
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
    #[cfg(feature = "tokio")]
    async fn power_off(&self) -> Result<(), Error> {
        let command = PowerCommand::Standby;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => {
                // Wait for standby/off
                tokio::time::sleep(self.standby_time()).await;
                Ok(())
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn power_off_blocking(&mut self) -> Result<(), Error> {
        let command = PowerCommand::Standby;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => {
                // Wait for standby/off
                std::thread::sleep(self.standby_time());
                Ok(())
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
