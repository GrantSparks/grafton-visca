//! System control implementation for PTZ cameras.
//!
//! This module provides low-level system control functionality including:
//! - Address assignment for multi-camera serial bus configurations
//! - Interface clearing for communication reset
//! - Command cancellation for specific communication sockets
//! - System-level VISCA protocol operations
//!
//! System controls are primarily used for communication management and
//! multi-camera setups where multiple cameras share a single serial bus.
//! These operations are typically used during system initialization or
//! when recovering from communication errors.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{camera::ViscaClient, mode::Mode, Error, ViscaSocket};

/// System operations for PTZ cameras.
///
/// This trait provides system-level control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # System Operations
///
/// - **Address Assignment**: Configure camera addresses in multi-camera serial setups
/// - **Interface Clear**: Reset communication state and clear any pending operations
/// - **Command Cancel**: Cancel specific commands on designated communication sockets
///
/// # Multi-Camera Considerations
///
/// When multiple cameras are connected via serial bus (RS-232/RS-422), each camera
/// needs a unique address. The address assignment operation helps establish these
/// addresses automatically.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.trigger_address_assignment()?;  // Auto-assign addresses
/// camera.interface_clear()?;  // Clear communication
/// camera.cancel_command(socket)?;  // Cancel specific command
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.trigger_address_assignment().await?;  // Auto-assign addresses
/// camera.interface_clear().await?;  // Clear communication
/// camera.cancel_command(socket).await?;  // Cancel specific command
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait SystemControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Trigger automatic address assignment (broadcast command for serial bus).
    ///
    /// Initiates the automatic address assignment process for cameras connected
    /// via serial bus. This is a broadcast command that triggers all cameras
    /// on the bus to automatically configure their addresses.
    ///
    /// # Note
    /// This doesn't set a specific address but triggers the auto-addressing process.
    /// Each camera will assign itself a unique address based on its position
    /// in the daisy chain.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or if the addressing process fails.
    fn trigger_address_assignment(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Clear interface (reset communication).
    ///
    /// Resets the communication interface and clears any pending commands
    /// or error states. This is useful for recovering from communication
    /// errors or when reinitializing the connection.
    ///
    /// # Errors
    /// Returns an error if the interface clear command fails to send.
    fn interface_clear(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Cancel command on specific socket.
    ///
    /// Cancels any pending command on the specified VISCA socket.
    /// This is useful for stopping long-running operations or clearing
    /// the command queue for a specific communication channel.
    ///
    /// # Parameters
    /// - `socket`: The VISCA socket number to cancel commands on
    ///
    /// # Errors
    /// Returns an error if the cancel command fails to send or if the
    /// specified socket is invalid.
    fn cancel_command(
        &self,
        socket: ViscaSocket,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Save current camera settings to non-volatile memory.
    ///
    /// Persists the current camera configuration to non-volatile memory,
    /// ensuring settings survive power cycles. This is primarily used for
    /// PTZOptics cameras after changing certain settings like image flip.
    ///
    /// # PTZOptics Flip Commands
    ///
    /// PTZOptics cameras may require calling this method after changing flip
    /// settings to ensure the changes persist across power cycles. The library
    /// tracks this requirement via the `REQUIRES_SETTINGS_SAVE_FOR_FLIP` profile
    /// capability flag.
    ///
    /// ```ignore
    /// // After changing flip settings on PTZOptics cameras:
    /// camera.enable_flip()?;
    /// camera.save_settings()?;  // Persist the flip change
    /// ```
    ///
    /// # VISCA Command
    /// `81 01 04 A5 10 FF`
    ///
    /// # Vendor Support
    /// - PTZOptics G2/G3/30X: Supported
    /// - Sony: Not applicable
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn save_settings(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> SystemControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn trigger_address_assignment(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::system::AddressSetCommand;
        let cmd = AddressSetCommand::new();
        self.execute(cmd)
    }

    fn interface_clear(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::system::InterfaceClearCommand;
        let cmd = InterfaceClearCommand::new();
        self.execute(cmd)
    }

    fn cancel_command(&self, socket: ViscaSocket) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::system::CommandCancelCommand;
        let cmd = CommandCancelCommand::new(socket);

        // Execute the cancel command
        // Note: NoSocket and CommandCanceled are success cases when canceling:
        // - NoSocket (0x05): No command was executing, nothing to cancel (OK)
        // - CommandCanceled (0x04): Command was successfully canceled (OK)
        // These are mapped to success in the runtime layer via to_public_error()
        self.execute(cmd)
    }

    fn save_settings(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::system::SettingsSaveCommand;
        let cmd = SettingsSaveCommand::new();
        self.execute(cmd)
    }
}
