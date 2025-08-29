//! System control methods for cameras.

use crate::{command::system::Socket, Error};

/// System operations for cameras.
///
/// This trait provides system control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait SystemControl {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    #[cfg(feature = "async")]
    fn trigger_address_assignment(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    #[cfg(not(feature = "async"))]
    fn trigger_address_assignment(&mut self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    #[cfg(feature = "async")]
    fn interface_clear(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Clear interface (reset communication).
    #[cfg(not(feature = "async"))]
    fn interface_clear(&mut self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    #[cfg(feature = "async")]
    fn cancel_command(
        &self,
        socket: Socket,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Cancel command on specific socket.
    #[cfg(not(feature = "async"))]
    fn cancel_command(&mut self, socket: Socket) -> Result<(), Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async system control trait (deprecated, use SystemControl instead).
/// Blocking system control trait (deprecated, use SystemControl instead).
// Async implementation for AsyncCamera
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> SystemControl for crate::camera::BlockingCamera<P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
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
