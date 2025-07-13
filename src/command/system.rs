//! System control commands for VISCA cameras.
//!
//! This module provides commands for system-level operations including
//! address setting, interface clearing, and command cancellation.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{const_encoding::CommandBuilder, encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Command to set camera address (broadcast, serial only).
///
/// This is used during initial setup of VISCA cameras on a serial bus.
/// Note: This is a broadcast command that affects all cameras on the bus.
#[derive(Debug, Copy, Clone)]
pub(crate) struct AddressSetCommand {
    /// Internal command bytes.
    command: [u8; 4],
}

impl AddressSetCommand {
    /// Create a new address set command.
    pub fn new() -> Self {
        let mut cmd = CommandBuilder::<4>::new();
        cmd.append(crate::command::const_encoding::constants::system::ADDRESS_SET);
        Self {
            command: cmd.build(),
        }
    }
}

impl Default for AddressSetCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl EncodeVisca for AddressSetCommand {
    type Response = ();
    const MAX_SIZE: usize = 4;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&self.command);
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Command to clear the interface (broadcast, serial only).
///
/// This resets the command buffer and clears any pending commands.
/// Note: This is a broadcast command that affects all cameras on the bus.
#[derive(Debug, Copy, Clone)]
pub(crate) struct InterfaceClearCommand {
    /// Internal command bytes.
    command: [u8; 5],
}

impl InterfaceClearCommand {
    /// Create a new interface clear command.
    pub fn new() -> Self {
        let mut cmd = CommandBuilder::<5>::new();
        cmd.append(crate::command::const_encoding::constants::system::INTERFACE_CLEAR);
        Self {
            command: cmd.build(),
        }
    }
}

impl Default for InterfaceClearCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl EncodeVisca for InterfaceClearCommand {
    type Response = ();
    const MAX_SIZE: usize = 5;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&self.command);
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Socket to cancel commands on.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Socket {
    /// Socket 1
    Socket1,
    /// Socket 2
    Socket2,
}

/// Command to cancel pending commands on a specific socket.
///
/// This cancels any in-progress commands on the specified socket.
#[derive(Debug, Copy, Clone)]
pub(crate) struct CommandCancelCommand {
    /// Internal command bytes.
    command: [u8; 3],
}

impl CommandCancelCommand {
    /// Create a new command cancel command.
    pub fn new(socket: Socket) -> Self {
        let mut cmd = CommandBuilder::<3>::new();
        cmd.append(crate::command::const_encoding::constants::system::COMMAND_CANCEL_PREFIX);
        cmd.push(match socket {
            Socket::Socket1 => 0x21,
            Socket::Socket2 => 0x22,
        });
        Self {
            command: cmd.build(),
        }
    }
}

impl EncodeVisca for CommandCancelCommand {
    type Response = ();
    const MAX_SIZE: usize = 3;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&self.command);
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_address_set_command() {
        let cmd = AddressSetCommand::new();
        assert_eq!(cmd.try_into_vec().unwrap(), vec![0x88, 0x30, 0x01, 0xFF]);
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_interface_clear_command() {
        let cmd = InterfaceClearCommand::new();
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x88, 0x01, 0x00, 0x01, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_command_cancel_socket1() {
        let cmd = CommandCancelCommand::new(Socket::Socket1);
        assert_eq!(cmd.try_into_vec().unwrap(), vec![0x81, 0x21, 0xFF]);
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_command_cancel_socket2() {
        let cmd = CommandCancelCommand::new(Socket::Socket2);
        assert_eq!(cmd.try_into_vec().unwrap(), vec![0x81, 0x22, 0xFF]);
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_socket_enum() {
        assert_eq!(Socket::Socket1, Socket::Socket1);
        assert_eq!(Socket::Socket2, Socket::Socket2);
        assert_ne!(Socket::Socket1, Socket::Socket2);
    }
}
