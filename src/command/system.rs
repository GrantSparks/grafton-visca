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
    command::{Command, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Command to set camera address (broadcast, serial only).
///
/// This is used during initial setup of VISCA cameras on a serial bus.
/// Note: This is a broadcast command that affects all cameras on the bus.
#[derive(Debug, Copy, Clone)]
pub struct AddressSetCommand;

impl Command for AddressSetCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        // Broadcast command: 88 30 01 FF
        Ok(vec![0x88, 0x30, 0x01, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Command to clear the interface (broadcast, serial only).
///
/// This resets the command buffer and clears any pending commands.
/// Note: This is a broadcast command that affects all cameras on the bus.
#[derive(Debug, Copy, Clone)]
pub struct InterfaceClearCommand;

impl Command for InterfaceClearCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        // Broadcast command: 88 01 00 01 FF
        Ok(vec![0x88, 0x01, 0x00, 0x01, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
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
pub struct CommandCancelCommand {
    /// The socket to cancel commands on.
    pub socket: Socket,
}

impl Command for CommandCancelCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self.socket {
            Socket::Socket1 => vec![0x81, 0x21, 0xFF],
            Socket::Socket2 => vec![0x81, 0x22, 0xFF],
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_set_command() {
        let cmd = AddressSetCommand;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x88, 0x30, 0x01, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.command_category(), CommandCategory::Quick);
    }

    #[test]
    fn test_interface_clear_command() {
        let cmd = InterfaceClearCommand;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x88, 0x01, 0x00, 0x01, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.command_category(), CommandCategory::Quick);
    }

    #[test]
    fn test_command_cancel_socket1() {
        let cmd = CommandCancelCommand {
            socket: Socket::Socket1,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x21, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.command_category(), CommandCategory::Quick);
    }

    #[test]
    fn test_command_cancel_socket2() {
        let cmd = CommandCancelCommand {
            socket: Socket::Socket2,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x22, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.command_category(), CommandCategory::Quick);
    }

    #[test]
    fn test_socket_enum() {
        assert_eq!(Socket::Socket1, Socket::Socket1);
        assert_eq!(Socket::Socket2, Socket::Socket2);
        assert_ne!(Socket::Socket1, Socket::Socket2);
    }
}