//! Power methods for cameras using the new GAT architecture.

use crate::{camera::unified::Camera, command::PowerCommand, Error, Response};

/// Power operations (async).
pub trait PowerOps: Sized {
    /// Power on the camera.
    async fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    async fn power_off(&self) -> Result<(), Error>;
}

/// Power operations (blocking).
pub trait PowerOpsBlocking: Sized {
    /// Power on the camera.
    fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    fn power_off(&self) -> Result<(), Error>;
}

// Async implementation
impl PowerOps for Camera {
    async fn power_on(&self) -> Result<(), Error> {
        let command = PowerCommand::On;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => {
                // Wait for camera to be ready
                // Note: Sleep is runtime-specific, so we just return immediately
                // Users should handle delays at the application level
                Ok(())
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn power_off(&self) -> Result<(), Error> {
        let command = PowerCommand::Standby;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => {
                // Wait for standby/off
                // Note: Sleep is runtime-specific, so we just return immediately
                // Users should handle delays at the application level
                Ok(())
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
impl PowerOpsBlocking for Camera {
    fn power_on(&self) -> Result<(), Error> {
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

    fn power_off(&self) -> Result<(), Error> {
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
