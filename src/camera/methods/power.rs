//! Power methods for cameras using the new GAT architecture.

use crate::{command::PowerCommand, Error};

/// Power operations (async).
#[cfg(feature = "async")]
pub trait PowerOps: Sized {
    /// Power on the camera.
    async fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    async fn power_off(&self) -> Result<(), Error>;
}

/// Power operations (blocking).
#[cfg(not(feature = "async"))]
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
        self.send_action_command(&command).await?;
        // Wait for camera to be ready using runtime
        let runtime = self.require_runtime()?;
        runtime.sleep(self.power_on_time()).await;
        Ok(())
    }

    async fn power_off(&self) -> Result<(), Error> {
        let command = PowerCommand::Standby;
        self.send_action_command(&command).await?;
        // Wait for standby/off using runtime
        let runtime = self.require_runtime()?;
        runtime.sleep(self.standby_time()).await;
        Ok(())
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<
        P: crate::capabilities::Profile,
        T: crate::transport::Transport
            + Send
            + Sync
            + 'static
            + crate::transport::core::BlockingTransport,
    > PowerOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn power_on(&self) -> Result<(), Error> {
        let command = PowerCommand::On;
        self.send_action_command_blocking(&command)?;
        // Wait for camera to be ready
        std::thread::sleep(self.power_on_time());
        Ok(())
    }

    fn power_off(&self) -> Result<(), Error> {
        let command = PowerCommand::Standby;
        self.send_action_command_blocking(&command)?;
        // Wait for standby/off
        std::thread::sleep(self.standby_time());
        Ok(())
    }
}
