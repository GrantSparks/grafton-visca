//! System control methods for cameras using the new GAT architecture.

use crate::{
    command::{
        system::{AddressSetCommand, CommandCancelCommand, InterfaceClearCommand, Socket},
        Response,
    },
    Error,
};

/// System operations (async).
#[cfg(feature = "async")]
pub trait SystemOps: Sized {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    async fn trigger_address_assignment(&self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    async fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    async fn cancel_command(&self, socket: Socket) -> Result<(), Error>;
}

/// System operations (blocking).
pub trait SystemOpsBlocking: Sized {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    fn trigger_address_assignment(&self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    fn cancel_command(&self, socket: Socket) -> Result<(), Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> SystemOps
    for crate::camera::generic::Camera<P, T>
{
    async fn trigger_address_assignment(&self) -> Result<(), Error> {
        let cmd = AddressSetCommand::new();
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn interface_clear(&self) -> Result<(), Error> {
        let cmd = InterfaceClearCommand::new();
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn cancel_command(&self, socket: Socket) -> Result<(), Error> {
        let cmd = CommandCancelCommand::new(socket);
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> SystemOpsBlocking
    for crate::camera::generic::Camera<P, T>
{
    fn trigger_address_assignment(&self) -> Result<(), Error> {
        let cmd = AddressSetCommand::new();
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn interface_clear(&self) -> Result<(), Error> {
        let cmd = InterfaceClearCommand::new();
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn cancel_command(&self, socket: Socket) -> Result<(), Error> {
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
    #[test]
    fn test_system_methods_compile() {
        // This test demonstrates that system methods are available for all cameras

        fn _test_system_methods<P, T>(_camera: &crate::Camera<P, T>)
        where
            P: crate::capabilities::Profile,
            T: crate::transport::UnifiedTransport,
        {
            // All cameras can use system methods
        }
    }
}
