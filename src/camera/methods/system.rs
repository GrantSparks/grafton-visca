//! System control methods for cameras using the new GAT architecture.

use crate::{command::system::Socket, Error};

/// System operations (async).
#[cfg(feature = "async")]
pub trait SystemControl: Send + Sync + 'static + Sized {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    fn trigger_address_assignment(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Clear interface (reset communication).
    fn interface_clear(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Cancel command on specific socket.
    fn cancel_command(
        &self,
        socket: Socket,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;
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
impl<P, Tr, Exec> SystemControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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
#[cfg(not(feature = "async"))]
impl<P, T> SystemControlBlocking for crate::camera::BlockingCamera<P, T>
where
    P: crate::capabilities::Profile + Default,
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
