//! Power methods for cameras using the new GAT architecture.

use crate::{
    command::{PowerCommand, Response},
    Error,
};

/// Power operations (async).
#[cfg(feature = "async")]
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
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    PowerOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn power_on(&self) -> Result<(), Error> {
        let command = PowerCommand::On;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => {
                // Wait for camera to be ready using runtime if available
                if let Some(runtime) = self.runtime() {
                    runtime.sleep(self.power_on_time()).await;
                } else {
                    // Fallback to tokio if available
                    #[cfg(feature = "tokio")]
                    if tokio::runtime::Handle::try_current().is_ok() {
                        tokio::time::sleep(self.power_on_time()).await;
                    }
                    // If no runtime available, just return immediately
                    // Users will need to handle delays at the application level
                }
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
                // Wait for standby/off using runtime if available
                if let Some(runtime) = self.runtime() {
                    runtime.sleep(self.standby_time()).await;
                } else {
                    // Fallback to tokio if available
                    #[cfg(feature = "tokio")]
                    if tokio::runtime::Handle::try_current().is_ok() {
                        tokio::time::sleep(self.standby_time()).await;
                    }
                    // If no runtime available, just return immediately
                    // Users will need to handle delays at the application level
                }
                Ok(())
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    PowerOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
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
