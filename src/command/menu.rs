//! Menu control commands for camera on-screen display (OSD) menu navigation.
//!
//! This module provides VISCA commands for controlling the camera's built-in menu system,
//! allowing remote navigation and configuration. These commands are particularly useful
//! for Sony FR7 and other cameras with comprehensive on-screen menus.

use crate::{command::bytes::ConstCommandBuilder, visca_command, Error};

visca_command! {
    /// Menu display control command.
    ///
    /// Toggles the camera's on-screen menu display on or off.
    ///
    /// VISCA format: `81 01 06 06 0p FF` where p = 2 (On) or 3 (Off)
    pub struct SetMenuDisplay {
        on: bool,
    };
    prefix = [0x01, 0x06, 0x06];
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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

// Manual implementation for MenuNavigate due to complex direction mapping
impl crate::command::encode::WireEncode for MenuNavigate {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let mut builder = ConstCommandBuilder::<9>::new();
        builder.push_mut(camera_id.to_address_byte());
        builder.append_mut(&[0x01, 0x06, 0x01, 0x0E, 0x0E]);
        match self.direction {
            MenuDirection::Up => builder.append_mut(&[0x03, 0x01]),
            MenuDirection::Down => builder.append_mut(&[0x03, 0x02]),
            MenuDirection::Left => builder.append_mut(&[0x01, 0x03]),
            MenuDirection::Right => builder.append_mut(&[0x02, 0x03]),
        };
        builder.terminate().build_into(buffer)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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

visca_command! {
    /// Menu action command for select/cancel operations.
    ///
    /// Performs menu selection (Enter) or cancellation (Back) actions.
    ///
    /// VISCA format: `81 01 06 06 0p FF` where p = 5 (Select) or 4 (Cancel)
    pub struct PerformMenuAction {
        action: MenuAction,
    };
    prefix = [0x01, 0x06, 0x06];
    param = u8::from(*action);
    max_param_size = 1;
}

impl PerformMenuAction {
    /// Create a new menu action command.
    pub fn new(action: MenuAction) -> Self {
        Self { action }
    }
}

/// Direct menu control command for Sony FR7.
///
/// Provides direct control over the FR7's advanced menu system using
/// manufacturer-specific codes for button presses and dial turns.
///
/// VISCA format: `81 01 7E 04 72 pp qq FF`
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DirectMenuControl {
    /// First control byte (pp).
    control1: u8,
    /// Second control byte (qq).
    control2: u8,
}

impl crate::command::encode::WireEncode for DirectMenuControl {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let mut builder = ConstCommandBuilder::<8>::new();
        builder.push_mut(camera_id.to_address_byte());
        builder.append_mut(&[0x01, 0x7E, 0x04, 0x72]);
        builder.push_mut(self.control1);
        builder.push_mut(self.control2);
        builder.terminate().build_into(buffer)
    }
}

impl DirectMenuControl {
    /// Create a new direct menu control command.
    ///
    /// `0xFF` immediately followed by a VISCA address (`0x81..=0x88`) would
    /// encode as an apparent second VISCA frame. Such a pair is rejected here
    /// to match raw-frame admission before a command can be submitted.
    pub fn new(control1: u8, control2: u8) -> Result<Self, Error> {
        if control1 == crate::command::bytes::VISCA_TERMINATOR && (0x81..=0x88).contains(&control2)
        {
            return Err(Error::InvalidRequest(
                "direct menu control cannot contain 0xFF followed by a VISCA address; it would encode as a second frame".into(),
            ));
        }

        Ok(Self { control1, control2 })
    }

    /// Return the first direct-menu control byte.
    #[must_use]
    pub const fn control1(self) -> u8 {
        self.control1
    }

    /// Return the second direct-menu control byte.
    #[must_use]
    pub const fn control2(self) -> u8 {
        self.control2
    }

    /// Menu open/close toggle (FR7).
    pub fn open_close() -> Self {
        Self {
            control1: 0x00,
            control2: 0x01,
        }
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
        DirectMenuControl::new(0x00, 0x01).expect("valid direct menu control"),
        &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        DirectMenuControl,
        test_direct_menu_open_close,
        DirectMenuControl::open_close(),
        &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0x01, VISCA_TERMINATOR]
    );

    // Regression for #683: a 0xFF direct-menu control parameter is a data byte.
    // The frame must still be terminated: `... 72 00 FF FF`, not `... 72 00 FF`.
    visca_test!(
        DirectMenuControl,
        test_direct_menu_control_0xff_param_keeps_terminator,
        DirectMenuControl::new(0x00, 0xFF).expect("0xFF data byte is valid"),
        &[0x81, 0x01, 0x7E, 0x04, 0x72, 0x00, 0xFF, VISCA_TERMINATOR]
    );

    #[test]
    fn direct_menu_rejects_an_apparent_second_visca_frame() {
        for control2 in 0x81..=0x88 {
            assert!(matches!(
                DirectMenuControl::new(0xFF, control2),
                Err(Error::InvalidRequest(_))
            ));
        }
    }

    /// Every accepted control pair, including a trailing `0xFF` data byte,
    /// encodes to one terminated frame. The rejected `0xFF 0x81..=0x88`
    /// sequences are the raw admission guard's apparent second frame.
    #[test]
    fn every_accepted_direct_menu_param_terminates() {
        use crate::command::encode::WireEncode;

        for control1 in [0x00u8, 0x7F, 0x80, 0xFF] {
            for control2 in 0..=u8::MAX {
                let command = DirectMenuControl::new(control1, control2);
                if control1 == 0xFF && (0x81..=0x88).contains(&control2) {
                    assert!(matches!(command, Err(Error::InvalidRequest(_))));
                    continue;
                }
                let command = command.expect("all remaining control pairs are valid");
                let mut buffer = [0u8; 32];
                let len = command
                    .write_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
                    .expect("direct menu control encodes into a 32-byte buffer");
                assert_eq!(
                    buffer[..len],
                    [
                        0x81,
                        0x01,
                        0x7E,
                        0x04,
                        0x72,
                        control1,
                        control2,
                        VISCA_TERMINATOR
                    ],
                    "direct menu control1={control1:#x} control2={control2:#x} wire bytes"
                );
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
