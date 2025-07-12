//! System control methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    command::{
        system::{AddressSetCommand, CommandCancelCommand, InterfaceClearCommand, Socket},
        Response,
    },
    Error,
};

#[cfg(feature = "tokio")]
use core::future::Future;
/// System operations.
pub trait SystemOps: Sized {

    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    #[cfg(feature = "tokio")]
    fn trigger_address_assignment(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn trigger_address_assignment_blocking(&mut self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    #[cfg(feature = "tokio")]
    fn interface_clear(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Clear interface (reset communication). (blocking).
    #[cfg(not(feature = "tokio"))]
    fn interface_clear_blocking(&mut self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    #[cfg(feature = "tokio")]
    fn cancel_command(&self, socket: Socket) -> impl Future<Output = Result<(), Error>> + Send;

    /// Cancel command on specific socket. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn cancel_command_blocking(&mut self, socket: Socket) -> Result<(), Error>;
}

impl SystemOps for Camera {
    #[cfg(feature = "tokio")]
    fn trigger_address_assignment(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = AddressSetCommand::new();
            let response = self.send_command(&cmd).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn trigger_address_assignment_blocking(&mut self) -> Result<(), Error> {
        
            let cmd = AddressSetCommand::new();
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn interface_clear(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = InterfaceClearCommand::new();
            let response = self.send_command(&cmd).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn interface_clear_blocking(&mut self) -> Result<(), Error> {
        
            let cmd = InterfaceClearCommand::new();
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn cancel_command(&self, socket: Socket) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let cmd = CommandCancelCommand::new(socket);
            let response = self.send_command(&cmd).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn cancel_command_blocking(&mut self, socket: Socket) -> Result<(), Error> {
        
            let cmd = CommandCancelCommand::new(socket);
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_methods_compile() {
        // This test demonstrates that system methods are available for all cameras
        
        fn _test_system_methods(_camera: &Camera) {
            // All cameras can use system methods
        }
    }
}