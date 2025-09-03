//! Unified system control implementation using Mode trait.

use crate::{camera::CameraSend, mode::Mode, Error, ViscaSocket};

/// Unified system operations for cameras.
///
/// This trait provides system control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
pub trait SystemControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    fn trigger_address_assignment(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Clear interface (reset communication).
    fn interface_clear(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Cancel command on specific socket.
    fn cancel_command(
        &self,
        socket: ViscaSocket,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> SystemControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CameraSend<M>,
{
    type Mode = M;

    fn trigger_address_assignment(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::system::AddressSetCommand;
        let cmd = AddressSetCommand::new();
        self.send_and_complete(cmd)
    }

    fn interface_clear(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::system::InterfaceClearCommand;
        let cmd = InterfaceClearCommand::new();
        self.send_and_complete(cmd)
    }

    fn cancel_command(&self, socket: ViscaSocket) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::system::CommandCancelCommand;
        let cmd = CommandCancelCommand::new(socket);
        // For now, just use send_and_complete - the error handling can be added later
        // TODO: Add custom error handling for NoSocket and CommandCanceled cases
        self.send_and_complete(cmd)
    }
}
