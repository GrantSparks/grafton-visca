//! System control commands for VISCA cameras.
//!
//! This module provides commands for system-level operations including
//! address setting, interface clearing, and command cancellation.
//!
//! # VISCA Compliance
//! Core system commands like address setting and interface clearing are baseline VISCA.
//!
//! ## Vendor-Specific Features
//! - `MotionSyncMode` and `MotionSyncSpeed` - PTZOptics specific (firmware 1.1.6+)
//!   These features coordinate pan, tilt, and zoom movements for smoother preset recalls.

// Standard library imports
use std::borrow::Cow;

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{error::Error, visca_param_command};

crate::visca_const_command! {
    /// Command to set camera address (broadcast, serial only).
    ///
    /// This is used during initial setup of VISCA cameras on a serial bus.
    /// Note: This is a broadcast command that affects all cameras on the bus.
    pub(crate) struct AddressSetCommand;
    bytes = [0x88, 0x30, 0x01, 0xFF];
    timeout = Quick;
    address = 0x88;
    response = None;
}

crate::visca_const_command! {
    /// Command to clear the interface (broadcast, serial only).
    ///
    /// This resets the command buffer and clears any pending commands.
    /// Note: This is a broadcast command that affects all cameras on the bus.
    pub(crate) struct InterfaceClearCommand;
    bytes = [0x88, 0x01, 0x00, 0x01, 0xFF];
    timeout = Quick;
    address = 0x88;
    response = None;
}

/// Motion sync modes for coordinated camera movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionSyncMode {
    /// Motion sync disabled.
    Off,
    /// Motion sync enabled.
    On,
}

impl TryFrom<u8> for MotionSyncMode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x02 => Ok(MotionSyncMode::Off),
            0x03 => Ok(MotionSyncMode::On),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("0x02 (Off) or 0x03 (On)"),
                actual: vec![value],
            }),
        }
    }
}

/// Motion sync speed settings for camera movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionSyncSpeed {
    /// Slow motion sync speed.
    Slow,
    /// Normal motion sync speed.
    Normal,
    /// Fast motion sync speed.
    Fast,
}

impl TryFrom<u8> for MotionSyncSpeed {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(MotionSyncSpeed::Slow),
            0x01 => Ok(MotionSyncSpeed::Normal),
            0x02 => Ok(MotionSyncSpeed::Fast),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("0x00 (Slow), 0x01 (Normal), or 0x02 (Fast)"),
                actual: vec![value],
            }),
        }
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

impl From<Socket> for u8 {
    fn from(socket: Socket) -> u8 {
        match socket {
            Socket::Socket1 => 0x21,
            Socket::Socket2 => 0x22,
        }
    }
}

visca_param_command! {
    /// Command to cancel pending commands on a specific socket.
    ///
    /// This cancels any in-progress commands on the specified socket.
    pub(crate) struct CommandCancelCommand {
        socket: Socket,
    }
    prefix = [0x81];
    param_byte = u8::from(*socket);
    timeout = Quick;
}

impl CommandCancelCommand {
    /// Create a new command cancel command.
    pub fn new(socket: Socket) -> Self {
        Self { socket }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{command::encode_visca::EncodeVisca, timeout::CommandCategory};

    #[test]
    fn test_address_set_command() {
        let cmd = AddressSetCommand::new();
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            vec![0x88, 0x30, 0x01, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_interface_clear_command() {
        let cmd = InterfaceClearCommand::new();
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            vec![0x88, 0x01, 0x00, 0x01, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_command_cancel_socket1() {
        let cmd = CommandCancelCommand::new(Socket::Socket1);
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            vec![0x81, 0x21, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_command_cancel_socket2() {
        let cmd = CommandCancelCommand::new(Socket::Socket2);
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            vec![0x81, 0x22, 0xFF]
        );
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
