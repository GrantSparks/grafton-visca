//! System control methods for cameras.

use crate::camera::Camera;
use crate::capabilities::ProfileMetadata;
use crate::command::system::{AddressSetCommand, CommandCancelCommand, InterfaceClearCommand};
use crate::command::Command;
use crate::Error;

/// Extension trait that adds system control methods to cameras.
#[allow(async_fn_in_trait)]
pub trait SystemMethodsExt {
    /// Set camera address (1-7).
    #[cfg(not(feature = "async"))]
    fn set_address(&mut self, address: u8) -> Result<(), Error>;

    /// Set camera address (1-7).
    #[cfg(feature = "async")]
    async fn set_address(&self, address: u8) -> Result<(), Error>;

    /// Clear interface (reset communication).
    #[cfg(not(feature = "async"))]
    fn interface_clear(&mut self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    #[cfg(feature = "async")]
    async fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    #[cfg(not(feature = "async"))]
    fn cancel_command(&mut self, socket: crate::command::system::Socket) -> Result<(), Error>;

    /// Cancel command on specific socket.
    #[cfg(feature = "async")]
    async fn cancel_command(&self, socket: crate::command::system::Socket) -> Result<(), Error>;
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P, T> SystemMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::blocking::BlockingTransport,
{
    fn set_address(&mut self, address: u8) -> Result<(), Error> {
        // Note: AddressSetCommand is a broadcast command that doesn't take an address parameter
        // The address parameter here is ignored, but kept for API compatibility
        let _ = address;
        let cmd = AddressSetCommand;
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn interface_clear(&mut self) -> Result<(), Error> {
        let cmd = InterfaceClearCommand;
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn cancel_command(&mut self, socket: crate::command::system::Socket) -> Result<(), Error> {
        let cmd = CommandCancelCommand { socket };
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> SystemMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::AsyncTransport,
{
    async fn set_address(&self, address: u8) -> Result<(), Error> {
        // Note: AddressSetCommand is a broadcast command that doesn't take an address parameter
        // The address parameter here is ignored, but kept for API compatibility
        let _ = address;
        let cmd = AddressSetCommand;
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn interface_clear(&self) -> Result<(), Error> {
        let cmd = InterfaceClearCommand;
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn cancel_command(&self, socket: crate::command::system::Socket) -> Result<(), Error> {
        let cmd = CommandCancelCommand { socket };
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::PTZOpticsG2;

    #[test]
    fn test_system_methods_compile() {
        #[derive(Debug)]
        struct MockTransport;

        #[cfg(not(feature = "async"))]
        impl crate::transport::blocking::BlockingTransport for MockTransport {
            fn send(&mut self, _data: &[u8]) -> Result<(), Error> {
                Ok(())
            }
            fn receive(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, Error> {
                Ok(vec![0x90, 0x50, 0xFF])
            }
            fn is_connected(&self) -> bool {
                true
            }
            fn description(&self) -> &str {
                "MockTransport"
            }
        }

        let mut _camera: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);

        #[cfg(not(feature = "async"))]
        {
            let _ = _camera.set_address(1);
            let _ = _camera.interface_clear();
            let _ = _camera.cancel_command(crate::command::system::Socket::Socket1);
        }
    }
}