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

use crate::{
    command::{bytes::VISCA_TERMINATOR, encode::WireEncode},
    error::Error,
    ViscaSocket,
};

/// Command to set camera address (broadcast, serial only).
///
/// This is used during initial setup of VISCA cameras on a serial bus.
/// Note: This is a broadcast command that affects all cameras on the bus.
#[derive(Debug, Copy, Clone)]
pub(crate) struct AddressSetCommand;

impl AddressSetCommand {
    /// Create a new address set command.
    pub fn new() -> Self {
        AddressSetCommand
    }
}

impl WireEncode for AddressSetCommand {
    const MAX_SIZE: usize = 4;

    fn write_into(
        &self,
        _camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 4 {
            return Err(Error::BufferTooSmall {
                required: 4,
                actual: buffer.len(),
            });
        }
        buffer[0] = 0x88;
        buffer[1] = 0x30;
        buffer[2] = 0x01;
        buffer[3] = VISCA_TERMINATOR;
        Ok(4)
    }
}

/// Command to clear the interface (broadcast, serial only).
///
/// This resets the command buffer and clears any pending commands.
/// Note: This is a broadcast command that affects all cameras on the bus.
#[derive(Debug, Copy, Clone)]
pub(crate) struct InterfaceClearCommand;

impl InterfaceClearCommand {
    /// Create a new interface clear command.
    pub fn new() -> Self {
        InterfaceClearCommand
    }
}

impl WireEncode for InterfaceClearCommand {
    const MAX_SIZE: usize = 5;

    fn write_into(
        &self,
        _camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 5 {
            return Err(Error::BufferTooSmall {
                required: 5,
                actual: buffer.len(),
            });
        }
        buffer[0] = 0x88;
        buffer[1] = 0x01;
        buffer[2] = 0x00;
        buffer[3] = 0x01;
        buffer[4] = VISCA_TERMINATOR;
        Ok(5)
    }
}

/// Motion sync modes for coordinated camera movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum MotionSyncMode {
    /// Motion sync enabled.
    On = 0x02,
    /// Motion sync disabled.
    Off = 0x03,
}

/// Motion sync preset speed settings for camera movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum MotionSyncPreset {
    /// Slow motion sync speed.
    Slow = 0x00,
    /// Normal motion sync speed.
    Normal = 0x01,
    /// Fast motion sync speed.
    Fast = 0x02,
}

/// Command to cancel pending commands on a specific socket.
///
/// This cancels any in-progress commands on the specified socket.
#[derive(Debug, Copy, Clone)]
pub(crate) struct CommandCancelCommand {
    socket: ViscaSocket,
}

impl WireEncode for CommandCancelCommand {
    const MAX_SIZE: usize = 3;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 3 {
            return Err(Error::BufferTooSmall {
                required: 3,
                actual: buffer.len(),
            });
        }
        buffer[0] = camera_id.to_address_byte();
        buffer[1] = self.socket.as_cancel_byte();
        buffer[2] = VISCA_TERMINATOR;
        Ok(3)
    }
}

impl CommandCancelCommand {
    /// Create a new command cancel command.
    pub fn new(socket: ViscaSocket) -> Self {
        Self { socket }
    }
}

/// Command to save current camera settings to non-volatile memory.
///
/// This command persists the current camera configuration. PTZOptics cameras
/// may require this after certain setting changes (like flip mode) to ensure
/// the changes persist across power cycles.
///
/// # VISCA Command
/// `81 01 04 A5 10 FF`
///
/// # Vendor Support
/// - PTZOptics G2/G3/30X: Supported
/// - Sony: Not applicable
#[derive(Debug, Copy, Clone)]
pub struct SettingsSaveCommand;

impl SettingsSaveCommand {
    /// Create a new settings save command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SettingsSaveCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl WireEncode for SettingsSaveCommand {
    const MAX_SIZE: usize = 6;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 6 {
            return Err(Error::BufferTooSmall {
                required: 6,
                actual: buffer.len(),
            });
        }
        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0xA5;
        buffer[4] = 0x10;
        buffer[5] = VISCA_TERMINATOR;
        Ok(6)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::macros::test_utils::visca_test;

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

    visca_test!(
        SettingsSaveCommand,
        test_settings_save_command,
        SettingsSaveCommand::new(),
        &[0x81, 0x01, 0x04, 0xA5, 0x10, VISCA_TERMINATOR]
    );

    #[test]
    fn test_settings_save_default() {
        let cmd1 = SettingsSaveCommand::new();
        let cmd2 = SettingsSaveCommand;
        // Both should produce the same bytes
        use crate::camera_id::CameraId;
        assert_eq!(
            crate::command::test_wire_bytes(&cmd1, CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap()
        );
    }

    #[test]
    fn test_socket_enum() {
        assert_eq!(ViscaSocket::S1, ViscaSocket::S1);
        assert_eq!(ViscaSocket::S2, ViscaSocket::S2);
        assert_ne!(ViscaSocket::S1, ViscaSocket::S2);
    }

    /// Test that cancel commands are correctly addressed for different camera IDs.
    ///
    /// This verifies the fix for issue #481: cancel operations must use the
    /// explicitly provided camera ID rather than falling back to CAMERA_1.
    #[test]
    fn test_cancel_command_camera_addressing() {
        use crate::camera_id::CameraId;

        // Test CAMERA_1 addressing (0x81 = 0x80 + 1)
        let cmd = CommandCancelCommand::new(ViscaSocket::S1);
        let bytes = crate::command::test_wire_bytes(&cmd, CameraId::CAMERA_1).unwrap();
        assert_eq!(bytes[0], 0x81, "First byte should be 0x81 for CAMERA_1");

        // Test CAMERA_2 addressing (0x82 = 0x80 + 2)
        let cmd = CommandCancelCommand::new(ViscaSocket::S1);
        let bytes = crate::command::test_wire_bytes(&cmd, CameraId::CAMERA_2).unwrap();
        assert_eq!(bytes[0], 0x82, "First byte should be 0x82 for CAMERA_2");

        // Test CAMERA_3 addressing (0x83 = 0x80 + 3)
        let cmd = CommandCancelCommand::new(ViscaSocket::S2);
        let bytes = crate::command::test_wire_bytes(&cmd, CameraId::CAMERA_3).unwrap();
        assert_eq!(bytes[0], 0x83, "First byte should be 0x83 for CAMERA_3");

        // Test BROADCAST addressing (0x88 = 0x80 + 8)
        let cmd = CommandCancelCommand::new(ViscaSocket::S1);
        let bytes = crate::command::test_wire_bytes(&cmd, CameraId::BROADCAST).unwrap();
        assert_eq!(bytes[0], 0x88, "First byte should be 0x88 for BROADCAST");

        // Verify full command structure for CAMERA_2 with Socket S2
        let cmd = CommandCancelCommand::new(ViscaSocket::S2);
        let bytes = crate::command::test_wire_bytes(&cmd, CameraId::CAMERA_2).unwrap();
        assert_eq!(
            bytes.as_slice(),
            &[0x82, 0x22, VISCA_TERMINATOR],
            "Full cancel command for CAMERA_2/S2 should be [0x82, 0x22, VISCA_TERMINATOR]"
        );
    }
}
