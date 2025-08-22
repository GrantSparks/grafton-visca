//! System control methods for cameras using the new GAT architecture.

use crate::{command::system::Socket, Error};

/// System operations (async).
#[cfg(feature = "async")]
pub trait SystemControl: Sized {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    async fn trigger_address_assignment(&self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    async fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    async fn cancel_command(&self, socket: Socket) -> Result<(), Error>;
}

/// System operations (blocking).
pub trait SystemControlBlocking: Sized {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    fn trigger_address_assignment(&mut self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    fn interface_clear(&mut self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    fn cancel_command(&mut self, socket: Socket) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> SystemControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn trigger_address_assignment(&self) -> Result<(), Error> {
        use crate::command::system::AddressSetCommand;

        let cmd = AddressSetCommand::new();
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn interface_clear(&self) -> Result<(), Error> {
        use crate::command::system::InterfaceClearCommand;

        let cmd = InterfaceClearCommand::new();
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn cancel_command(&self, socket: Socket) -> Result<(), Error> {
        use crate::command::system::CommandCancelCommand;

        let cmd = CommandCancelCommand::new(socket);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> SystemControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn trigger_address_assignment(&mut self) -> Result<(), Error> {
        use crate::command::system::AddressSetCommand;

        let cmd = AddressSetCommand::new();
        self.send_command(&cmd)?;
        Ok(())
    }

    fn interface_clear(&mut self) -> Result<(), Error> {
        use crate::command::system::InterfaceClearCommand;

        let cmd = InterfaceClearCommand::new();
        self.send_command(&cmd)?;
        Ok(())
    }

    fn cancel_command(&mut self, socket: Socket) -> Result<(), Error> {
        use crate::command::system::CommandCancelCommand;

        let cmd = CommandCancelCommand::new(socket);
        self.send_command(&cmd)?;
        Ok(())
    }
}
