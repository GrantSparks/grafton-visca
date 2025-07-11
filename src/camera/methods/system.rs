//! System control methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::ProfileMetadata,
    command::{
        system::{AddressSetCommand, CommandCancelCommand, InterfaceClearCommand, Socket},
        Response,
    },
    transport::gat_transport::Transport,
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore - provides future-returning methods.
pub trait SystemCoreExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Trigger automatic address assignment (broadcast command for serial bus) - returns a future.
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    fn trigger_address_assignment(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Clear interface (reset communication) - returns a future.
    fn interface_clear(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Cancel command on specific socket - returns a future.
    fn cancel_command(&self, socket: Socket) -> impl Future<Output = Result<(), Error>> + '_;
}

impl<P, T> SystemCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn trigger_address_assignment(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let cmd = AddressSetCommand;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn interface_clear(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let cmd = InterfaceClearCommand;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn cancel_command(&self, socket: Socket) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let cmd = CommandCancelCommand { socket };
            let response = self.send_command(&cmd).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for async Camera facade.
#[allow(async_fn_in_trait)]
pub trait SystemAsyncExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    async fn trigger_address_assignment(&self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    async fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    async fn cancel_command(&self, socket: Socket) -> Result<(), Error>;
}

impl<P, T> SystemAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    async fn trigger_address_assignment(&self) -> Result<(), Error> {
        self.core().trigger_address_assignment().await
    }

    async fn interface_clear(&self) -> Result<(), Error> {
        self.core().interface_clear().await
    }

    async fn cancel_command(&self, socket: Socket) -> Result<(), Error> {
        self.core().cancel_command(socket).await
    }
}

/// Extension trait for blocking Camera facade.
pub trait SystemBlockingExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    fn trigger_address_assignment(&self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    fn cancel_command(&self, socket: Socket) -> Result<(), Error>;
}

impl<P, T> SystemBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn trigger_address_assignment(&self) -> Result<(), Error> {
        block_on(self.core().trigger_address_assignment())
    }

    fn interface_clear(&self) -> Result<(), Error> {
        block_on(self.core().interface_clear())
    }

    fn cancel_command(&self, socket: Socket) -> Result<(), Error> {
        block_on(self.core().cancel_command(socket))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::PTZOpticsG2;

    #[test]
    fn test_system_methods_compile() {
        // This test demonstrates that system methods are available for all cameras

        fn _test_system_methods<T: Transport>(_camera: &CameraAsync<PTZOpticsG2, T>) {
            // All cameras can use system methods
        }
    }
}
