//! Menu control commands for camera on-screen display (OSD) menu navigation.
//!
//! This module provides VISCA commands for controlling the camera's built-in menu system,
//! allowing remote navigation and configuration. These commands are particularly useful
//! for Sony FR7 and other cameras with comprehensive on-screen menus.

use crate::{command::bytes::constants, macros::internal::*};

visca_bool_command! {
    /// Menu display control command.
    ///
    /// Toggles the camera's on-screen menu display on or off.
    ///
    /// VISCA format: `81 01 06 06 0p FF` where p = 2 (On) or 3 (Off)
    struct MenuDisplayCommand {
        prefix: constants::menu::TOGGLE_PREFIX,
        on: 0x02,
        off: 0x03,
    }
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

visca_builder! {
    /// Menu navigation command for cursor movement.
    ///
    /// Moves the menu cursor in the specified direction.
    ///
    /// VISCA format: `81 01 06 01 VV WW XX YY FF` where:
    /// - VV = Pan speed (0x0E for menu)
    /// - WW = Tilt speed (0x0E for menu)
    /// - XX YY = Direction codes
    pub struct MenuNavigate {
        direction: MenuDirection,
    }
    builder<9> => |builder, direction| {
        let builder = builder.append(constants::menu::NAVIGATE_PREFIX);
        match *direction {
            MenuDirection::Up => builder.push(0x03).push(0x01),
            MenuDirection::Down => builder.push(0x03).push(0x02),
            MenuDirection::Left => builder.push(0x01).push(0x03),
            MenuDirection::Right => builder.push(0x02).push(0x03),
        }
    }
    timeout = Quick;
}

impl MenuNavigate {
    /// Create a new menu navigation command.
    pub fn new(direction: MenuDirection) -> Self {
        Self { direction }
    }
}

/// Menu action type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Select/Enter the current menu item
    Select,
    /// Cancel/Back to previous menu
    Cancel,
}

impl From<MenuAction> for u8 {
    fn from(action: MenuAction) -> u8 {
        match action {
            MenuAction::Select => 0x05,
            MenuAction::Cancel => 0x04,
        }
    }
}

visca_param_command! {
    /// Menu action command for select/cancel operations.
    ///
    /// Performs menu selection (Enter) or cancellation (Back) actions.
    ///
    /// VISCA format: `81 01 06 06 0p FF` where p = 5 (Select) or 4 (Cancel)
    pub struct MenuActionCommand {
        action: MenuAction,
    }
    prefix = constants::menu::TOGGLE_PREFIX;
    param_byte = u8::from(*action);
    timeout = Quick;
}

impl MenuActionCommand {
    /// Create a new menu action command.
    pub fn new(action: MenuAction) -> Self {
        Self { action }
    }
}

visca_builder! {
    /// Direct menu control command for Sony FR7.
    ///
    /// Provides direct control over the FR7's advanced menu system using
    /// manufacturer-specific codes for button presses and dial turns.
    ///
    /// VISCA format: `81 01 7E 04 72 pp qq FF`
    pub struct DirectMenuControl {
        /// First control byte (pp)
        control1: u8,
        /// Second control byte (qq)
        control2: u8,
    }
    builder<8> => |builder, control1, control2| {
        builder
            .append(constants::menu::SETTINGS_PREFIX)
            .push(*control1)
            .push(*control2)
    }
    timeout = Quick;
}

impl DirectMenuControl {
    /// Create a new direct menu control command.
    pub fn new(control1: u8, control2: u8) -> Self {
        Self { control1, control2 }
    }

    /// Menu open/close toggle (FR7).
    pub fn open_close() -> Self {
        Self::new(0x00, 0x01)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        MenuDisplayCommand,
        test_menu_display_on,
        MenuDisplayCommand::new(true),
        &[0x81, 0x01, 0x06, 0x06, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        MenuDisplayCommand,
        test_menu_display_off,
        MenuDisplayCommand::new(false),
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
        MenuActionCommand,
        test_menu_select,
        MenuActionCommand::new(MenuAction::Select),
        &[0x81, 0x01, 0x06, 0x06, 0x05, VISCA_TERMINATOR]
    );

    visca_test!(
        MenuActionCommand,
        test_menu_cancel,
        MenuActionCommand::new(MenuAction::Cancel),
        &[0x81, 0x01, 0x06, 0x06, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        DirectMenuControl,
        test_direct_menu_control,
        DirectMenuControl::new(0x00, 0x01),
        &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        DirectMenuControl,
        test_direct_menu_open_close,
        DirectMenuControl::open_close(),
        &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, VISCA_TERMINATOR]
    );
}
