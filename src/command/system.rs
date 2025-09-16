//! System control commands for VISCA cameras.
//!
//! This module provides commands for system-level operations including
//! address setting, interface clearing, and command cancellation.
//!
//! # VISCA Compliance
//! Core system commands like address setting and interface clearing are baseline VISCA.
//!
//! ## Vendor-Specific Features
//! - `MotionSyncMode` and `MotionSyncSpeed` - PtzOptics specific (firmware 1.1.6+)
//!   These features coordinate pan, tilt, and zoom movements for smoother preset recalls.

use grafton_visca_macros::ViscaEnum;

use crate::command::bytes::{constants, VISCA_TERMINATOR};
use crate::macros::internal::*;
use crate::ViscaSocket;

visca_const_command! {
    /// Command to set camera address (broadcast, serial only).
    ///
    /// This is used during initial setup of VISCA cameras on a serial bus.
    /// Note: This is a broadcast command that affects all cameras on the bus.
    ///
    pub(crate) struct AddressSetCommand;
    bytes_terminated = [0x88, 0x30, 0x01, VISCA_TERMINATOR];
    timeout = Quick;
}

visca_const_command! {
    /// Command to clear the interface (broadcast, serial only).
    ///
    /// This resets the command buffer and clears any pending commands.
    /// Note: This is a broadcast command that affects all cameras on the bus.
    pub(crate) struct InterfaceClearCommand;
    bytes_terminated = [0x88, 0x01, 0x00, 0x01, VISCA_TERMINATOR];
    timeout = Quick;
}

/// Motion sync modes for coordinated camera movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
pub enum MotionSyncMode {
    /// Motion sync enabled.
    On = 0x02,
    /// Motion sync disabled.
    Off = 0x03,
}

/// Motion sync preset speed settings for camera movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
pub enum MotionSyncPreset {
    /// Slow motion sync speed.
    Slow = 0x00,
    /// Normal motion sync speed.
    Normal = 0x01,
    /// Fast motion sync speed.
    Fast = 0x02,
}

visca_param_command! {
    /// Command to cancel pending commands on a specific socket.
    ///
    /// This cancels any in-progress commands on the specified socket.
    pub(crate) struct CommandCancelCommand {
        socket: ViscaSocket,
    }
    prefix = constants::system_cmd::CANCEL_PREFIX;
    param_byte = socket.as_cancel_byte();
    timeout = Quick;
}

impl CommandCancelCommand {
    /// Create a new command cancel command.
    pub fn new(socket: ViscaSocket) -> Self {
        Self { socket }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::macros::test_utils::visca_test;
    use crate::timeout::CommandTimeout;
    use crate::{command::encode::ViscaCommand, timeout::CommandCategory};

    visca_test!(
        AddressSetCommand,
        test_address_set_command,
        AddressSetCommand::new(),
        &[0x88, 0x30, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        InterfaceClearCommand,
        test_interface_clear_command,
        InterfaceClearCommand::new(),
        &[0x88, 0x01, 0x00, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        CommandCancelCommand,
        test_command_cancel_socket1,
        CommandCancelCommand::new(ViscaSocket::S1),
        &[0x81, 0x21, VISCA_TERMINATOR]
    );

    visca_test!(
        CommandCancelCommand,
        test_command_cancel_socket2,
        CommandCancelCommand::new(ViscaSocket::S2),
        &[0x81, 0x22, VISCA_TERMINATOR]
    );

    #[test]
    fn test_response_type_and_timeout() {
        let cmd = AddressSetCommand::new();
        assert!(cmd.response_kind().is_none());
        assert_eq!(cmd.timeout_class(), CommandCategory::Quick);

        let cmd = InterfaceClearCommand::new();
        assert!(cmd.response_kind().is_none());
        assert_eq!(cmd.timeout_class(), CommandCategory::Quick);

        let cmd = CommandCancelCommand::new(ViscaSocket::S1);
        assert!(cmd.response_kind().is_none());
        assert_eq!(cmd.timeout_class(), CommandCategory::Quick);
    }

    #[test]
    fn test_socket_enum() {
        assert_eq!(ViscaSocket::S1, ViscaSocket::S1);
        assert_eq!(ViscaSocket::S2, ViscaSocket::S2);
        assert_ne!(ViscaSocket::S1, ViscaSocket::S2);
    }
}
