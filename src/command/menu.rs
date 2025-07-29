//! Menu control commands for camera on-screen display (OSD) menu navigation.
//!
//! This module provides VISCA commands for controlling the camera's built-in menu system,
//! allowing remote navigation and configuration. These commands are particularly useful
//! for Sony FR7 and other cameras with comprehensive on-screen menus.

use crate::{
    capabilities::{CameraFeature, CommandFeatures},
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Menu display control command.
///
/// Toggles the camera's on-screen menu display on or off.
///
/// VISCA format: `81 01 06 06 0p FF` where p = 2 (On) or 3 (Off)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuDisplayCommand {
    /// Whether to show the menu
    pub display: bool,
}

impl MenuDisplayCommand {
    /// Create a new menu display command.
    pub fn new(display: bool) -> Self {
        Self { display }
    }
}

impl EncodeVisca for MenuDisplayCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(
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
        buffer[2] = 0x06;
        buffer[3] = 0x06;
        buffer[4] = if self.display { 0x02 } else { 0x03 };
        buffer[5] = 0xFF;

        Ok(6)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl CommandFeatures for MenuDisplayCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::MenuControl]
    }
}

/// Menu navigation command for cursor movement.
///
/// Moves the menu cursor in the specified direction.
///
/// VISCA format: `81 01 06 01 VV WW XX YY FF` where:
/// - VV = Pan speed (0x0E for menu)
/// - WW = Tilt speed (0x0E for menu)
/// - XX YY = Direction codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuNavigateCommand {
    /// Navigation direction
    pub direction: MenuDirection,
}

/// Menu navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuDirection {
    /// Move cursor up
    Up,
    /// Move cursor down
    Down,
    /// Move cursor left
    Left,
    /// Move cursor right
    Right,
}

impl MenuNavigateCommand {
    /// Create a new menu navigation command.
    pub fn new(direction: MenuDirection) -> Self {
        Self { direction }
    }
}

impl EncodeVisca for MenuNavigateCommand {
    type Response = ();
    const MAX_SIZE: usize = 10;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 10 {
            return Err(Error::BufferTooSmall {
                required: 10,
                actual: buffer.len(),
            });
        }

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x06;
        buffer[3] = 0x01;
        buffer[4] = 0x0E; // Pan speed
        buffer[5] = 0x0E; // Tilt speed

        // Direction codes
        match self.direction {
            MenuDirection::Up => {
                buffer[6] = 0x03;
                buffer[7] = 0x01;
            }
            MenuDirection::Down => {
                buffer[6] = 0x03;
                buffer[7] = 0x02;
            }
            MenuDirection::Left => {
                buffer[6] = 0x01;
                buffer[7] = 0x03;
            }
            MenuDirection::Right => {
                buffer[6] = 0x02;
                buffer[7] = 0x03;
            }
        }

        buffer[8] = 0xFF;

        Ok(9)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl CommandFeatures for MenuNavigateCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::MenuControl]
    }
}

/// Menu action command for select/cancel operations.
///
/// Performs menu selection (Enter) or cancellation (Back) actions.
///
/// VISCA format: `81 01 06 06 0p FF` where p = 5 (Select) or 4 (Cancel)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuActionCommand {
    /// The action to perform
    pub action: MenuAction,
}

/// Menu action type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Select/Enter the current menu item
    Select,
    /// Cancel/Back to previous menu
    Cancel,
}

impl MenuActionCommand {
    /// Create a new menu action command.
    pub fn new(action: MenuAction) -> Self {
        Self { action }
    }
}

impl EncodeVisca for MenuActionCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(
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
        buffer[2] = 0x06;
        buffer[3] = 0x06;
        buffer[4] = match self.action {
            MenuAction::Select => 0x05,
            MenuAction::Cancel => 0x04,
        };
        buffer[5] = 0xFF;

        Ok(6)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl CommandFeatures for MenuActionCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::MenuControl]
    }
}

/// Direct menu control command for Sony FR7.
///
/// Provides direct control over the FR7's advanced menu system using
/// manufacturer-specific codes for button presses and dial turns.
///
/// VISCA format: `81 01 7E 04 72 pp qq FF`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectMenuControlCommand {
    /// First control byte (pp)
    pub control1: u8,
    /// Second control byte (qq)
    pub control2: u8,
}

impl DirectMenuControlCommand {
    /// Create a new direct menu control command.
    pub fn new(control1: u8, control2: u8) -> Self {
        Self { control1, control2 }
    }

    /// Menu open/close toggle (FR7).
    pub fn open_close() -> Self {
        Self::new(0x00, 0x01)
    }
}

impl EncodeVisca for DirectMenuControlCommand {
    type Response = ();
    const MAX_SIZE: usize = 8;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 8 {
            return Err(Error::BufferTooSmall {
                required: 8,
                actual: buffer.len(),
            });
        }

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x7E;
        buffer[3] = 0x04;
        buffer[4] = 0x72;
        buffer[5] = self.control1;
        buffer[6] = self.control2;
        buffer[7] = 0xFF;

        Ok(8)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl CommandFeatures for DirectMenuControlCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::MenuControl]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_display_on() {
        let cmd = MenuDisplayCommand::new(true);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..size], &[0x81, 0x01, 0x06, 0x06, 0x02, 0xFF]);
    }

    #[test]
    fn test_menu_display_off() {
        let cmd = MenuDisplayCommand::new(false);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..size], &[0x81, 0x01, 0x06, 0x06, 0x03, 0xFF]);
    }

    #[test]
    fn test_menu_navigate_up() {
        let cmd = MenuNavigateCommand::new(MenuDirection::Up);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(
            &buffer[..size],
            &[0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x03, 0x01, 0xFF]
        );
    }

    #[test]
    fn test_menu_navigate_down() {
        let cmd = MenuNavigateCommand::new(MenuDirection::Down);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(
            &buffer[..size],
            &[0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x03, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_menu_navigate_left() {
        let cmd = MenuNavigateCommand::new(MenuDirection::Left);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(
            &buffer[..size],
            &[0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x01, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_menu_navigate_right() {
        let cmd = MenuNavigateCommand::new(MenuDirection::Right);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(
            &buffer[..size],
            &[0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x02, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_menu_select() {
        let cmd = MenuActionCommand::new(MenuAction::Select);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..size], &[0x81, 0x01, 0x06, 0x06, 0x05, 0xFF]);
    }

    #[test]
    fn test_menu_cancel() {
        let cmd = MenuActionCommand::new(MenuAction::Cancel);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..size], &[0x81, 0x01, 0x06, 0x06, 0x04, 0xFF]);
    }

    #[test]
    fn test_direct_menu_control() {
        let cmd = DirectMenuControlCommand::new(0x00, 0x01);
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(
            &buffer[..size],
            &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, 0xFF]
        );
    }

    #[test]
    fn test_direct_menu_open_close() {
        let cmd = DirectMenuControlCommand::open_close();
        let mut buffer = [0u8; 10];
        let size = cmd
            .encode_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(
            &buffer[..size],
            &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, 0xFF]
        );
    }
}
