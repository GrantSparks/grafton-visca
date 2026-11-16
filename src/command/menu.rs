//! Menu control commands for camera on-screen display (OSD) menu navigation.
//!
//! This module provides VISCA commands for controlling the camera's built-in menu system,
//! allowing remote navigation and configuration. These commands are particularly useful
//! for Sony FR7 and other cameras with comprehensive on-screen menus.

use crate::wire::WireValue;

use crate::{
    command::bytes::{constants::menu, FrameWriter},
    macros::consolidated::visca_command,
    Error,
};

visca_command! {
    /// Menu display control command.
    ///
    /// Toggles the camera's on-screen menu display on or off.
    ///
    /// VISCA format: `81 01 06 06 0p FF` where p = 2 (On) or 3 (Off)
    pub struct SetMenuDisplay {
        on: bool,
    };
    prefix = menu::MENU;
    param = if *on { 0x02 } else { 0x03 };
    max_param_size = 1;
}

impl SetMenuDisplay {
    /// Create a new menu display command.
    pub fn new(on: bool) -> Self {
        Self { on }
    }
}

/// Menu navigation direction.
#[crate::value_enum(wire=(u8,u8))]
pub enum MenuDirection {
    /// Move cursor up
    #[wire((3, 1))]
    Up,
    /// Move cursor down
    #[wire((3, 2))]
    Down,
    /// Move cursor left
    #[wire((1, 3))]
    Left,
    /// Move cursor right
    #[wire((2, 3))]
    Right,
}

/// OSD navigation is the pan/tilt drive at the fixed `0E 0E` speed pair, so the
/// direction pair is the pan/tilt direction's.
impl crate::command::encode::WireEncode for MenuNavigate {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let (pan, tilt) = self.direction.to_wire();
        FrameWriter::new(camera_id, buffer)
            .bytes(&menu::NAVIGATE)
            .byte(pan)
            .byte(tilt)
            .finish()
    }
}

/// Menu navigation command for cursor movement.
///
/// Moves the menu cursor in the specified direction.
///
/// # Safety
///
/// With the menu closed, this is a continuous physical pan/tilt drive.
/// Send an explicit STOP or call `stop_all_motion()` to end the movement.
///
/// VISCA format: `81 01 06 01 VV WW XX YY FF` where:
/// - VV = Pan speed (0x0E for menu)
/// - WW = Tilt speed (0x0E for menu)
/// - XX YY = Direction codes
#[derive(Debug, Clone, Copy)]
pub struct MenuNavigate {
    direction: MenuDirection,
}

impl MenuNavigate {
    /// Create a new menu navigation command.
    pub fn new(direction: MenuDirection) -> Self {
        Self { direction }
    }
}

/// Menu action type.
#[crate::value_enum]
pub enum MenuAction {
    /// Select/Enter the current menu item
    Select = 5,
    /// Cancel/Back to previous menu
    Cancel = 4,
}

visca_command! {
    /// Menu action command for select/cancel operations.
    ///
    /// Performs menu selection (Enter) or cancellation (Back) actions.
    ///
    /// VISCA format: `81 01 06 06 0p FF` where p = 5 (Select) or 4 (Cancel)
    pub struct PerformMenuAction {
        action: MenuAction,
    };
    prefix = menu::MENU;
    param = (*action).to_wire();
    max_param_size = 1;
}

impl PerformMenuAction {
    /// Create a new menu action command.
    pub fn new(action: MenuAction) -> Self {
        Self { action }
    }
}

/// Sony FR7 direct-menu target (R7 v4.00, page 20).
#[crate::value_enum]
pub enum DirectMenuTarget {
    /// Neutral-density filter control.
    NdFilter = 0x00,
    /// Iris control.
    Iris = 0x01,
    /// ISO or gain control.
    IsoGain = 0x02,
    /// Shutter control.
    Shutter = 0x03,
    /// Auto-exposure level or mode control.
    AeLevelMode = 0x04,
    /// Exit the direct menu.
    Exit = 0x7F,
}

/// Sony FR7 direct-menu button state (R7 v4.00, page 20).
#[crate::value_enum]
pub enum MenuButton {
    /// Release the selected control.
    Release = 0x00,
    /// Press the selected control.
    Press = 0x01,
}

/// Direct menu control command for Sony FR7.
///
/// R7 v4.00 page 20 closes both domains in `8x 01 7E 04 72 pp 0q FF`:
/// one of six targets and a release or press. Construction cannot send an
/// undocumented control byte.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DirectMenuControl {
    target: DirectMenuTarget,
    button: MenuButton,
}

impl crate::command::encode::WireEncode for DirectMenuControl {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&menu::DIRECT)
            .byte(self.target.to_wire())
            .byte(self.button.to_wire())
            .finish()
    }
}

impl DirectMenuControl {
    /// Creates a direct-menu button command.
    #[must_use]
    pub const fn new(target: DirectMenuTarget, button: MenuButton) -> Self {
        Self { target, button }
    }

    /// Returns the selected direct-menu target.
    #[must_use]
    pub const fn target(self) -> DirectMenuTarget {
        self.target
    }

    /// Returns the selected button state.
    #[must_use]
    pub const fn button(self) -> MenuButton {
        self.button
    }

    /// Opens or closes the FR7 direct menu by pressing its ND control.
    #[must_use]
    pub const fn open_close() -> Self {
        Self::new(DirectMenuTarget::NdFilter, MenuButton::Press)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    use crate::{command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test};

    visca_test!(
        SetMenuDisplay,
        test_menu_display_on,
        SetMenuDisplay::new(true),
        &[0x81, 0x01, 0x06, 0x06, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        SetMenuDisplay,
        test_menu_display_off,
        SetMenuDisplay::new(false),
        &[0x81, 0x01, 0x06, 0x06, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        MenuNavigate,
        test_menu_navigate_up,
        MenuNavigate::new(MenuDirection::Up),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x0E,
            0x0E,
            0x03,
            0x01,
            VISCA_TERMINATOR
        ]
    );

    visca_test!(
        MenuNavigate,
        test_menu_navigate_down,
        MenuNavigate::new(MenuDirection::Down),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x0E,
            0x0E,
            0x03,
            0x02,
            VISCA_TERMINATOR
        ]
    );

    visca_test!(
        MenuNavigate,
        test_menu_navigate_left,
        MenuNavigate::new(MenuDirection::Left),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x0E,
            0x0E,
            0x01,
            0x03,
            VISCA_TERMINATOR
        ]
    );

    visca_test!(
        MenuNavigate,
        test_menu_navigate_right,
        MenuNavigate::new(MenuDirection::Right),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x0E,
            0x0E,
            0x02,
            0x03,
            VISCA_TERMINATOR
        ]
    );

    visca_test!(
        PerformMenuAction,
        test_menu_select,
        PerformMenuAction::new(MenuAction::Select),
        &[0x81, 0x01, 0x06, 0x06, 0x05, VISCA_TERMINATOR]
    );

    visca_test!(
        PerformMenuAction,
        test_menu_cancel,
        PerformMenuAction::new(MenuAction::Cancel),
        &[0x81, 0x01, 0x06, 0x06, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        DirectMenuControl,
        test_direct_menu_control,
        DirectMenuControl::new(DirectMenuTarget::NdFilter, MenuButton::Press),
        &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        DirectMenuControl,
        test_direct_menu_open_close,
        DirectMenuControl::open_close(),
        &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, VISCA_TERMINATOR]
    );

    #[test]
    fn direct_menu_domains_round_trip_and_encode_exactly() {
        use crate::command::encode::WireEncode;
        for byte in 0..=u8::MAX {
            let target = DirectMenuTarget::from_wire(byte);
            assert_eq!(target.is_ok(), matches!(byte, 0..=4 | 0x7F));
            let button = MenuButton::from_wire(byte);
            assert_eq!(button.is_ok(), byte <= 1);
            if let Ok(button) = button {
                assert_eq!(button.to_wire(), byte);
            }
            if let Ok(target) = target {
                assert_eq!(target.to_wire(), byte);
                for button in [MenuButton::Release, MenuButton::Press] {
                    let command = DirectMenuControl::new(target, button);
                    let mut buffer = [0; 8];
                    let len = command
                        .write_into(crate::CameraId::CAMERA_1, &mut buffer)
                        .unwrap();
                    assert_eq!(len, 8);
                    assert_eq!(
                        buffer,
                        [
                            0x81,
                            1,
                            0x7E,
                            4,
                            0x72,
                            byte,
                            button.to_wire(),
                            VISCA_TERMINATOR
                        ]
                    );
                }
            }
        }
    }

    /// Every menu navigation direction encodes to a terminated 9-byte frame.
    #[test]
    fn every_menu_direction_terminates() {
        use crate::command::encode::WireEncode;

        for direction in [
            MenuDirection::Up,
            MenuDirection::Down,
            MenuDirection::Left,
            MenuDirection::Right,
        ] {
            let command = MenuNavigate::new(direction);
            let mut buffer = [0u8; 32];
            let len = command
                .write_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
                .expect("menu navigation encodes into a 32-byte buffer");
            assert_eq!(len, 9, "menu {direction:?} must be a 9-byte frame");
            assert_eq!(buffer[0], 0x81);
            assert_eq!(
                buffer[len - 1],
                VISCA_TERMINATOR,
                "menu {direction:?} terminator"
            );
        }
    }
}
