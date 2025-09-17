//! Menu control commands for camera on-screen display (OSD) menu navigation.
//!
//! This module provides VISCA commands for controlling the camera's built-in menu system,
//! allowing remote navigation and configuration. These commands are particularly useful
//! for Sony FR7 and other cameras with comprehensive on-screen menus.

use crate::{timeout::CommandCategory, visca_command};

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
    category = CommandCategory::Quick;
}

impl SetMenuDisplay {
    /// Create a new menu display command.
    pub fn new(on: bool) -> Self {
        Self { on }
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

// Manual implementation for MenuNavigate due to complex direction mapping
impl crate::command::encode::ViscaCommand for MenuNavigate {
    type Response = ();
    const MAX_SIZE: usize = 9;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        use crate::command::bytes::ConstCommandBuilder;

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

    fn response_kind(&self) -> Option<crate::command::InquiryKind> {
        None
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
    category = CommandCategory::Quick;
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
#[derive(Debug, Copy, Clone)]
pub struct DirectMenuControl {
    /// The control1 parameter.
    /// First control byte (pp)
    pub control1: u8,
    /// The control2 parameter.
    /// Second control byte (qq)
    pub control2: u8,
}

impl crate::command::encode::ViscaCommand for DirectMenuControl {
    type Response = ();
    const MAX_SIZE: usize = 8;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let mut builder = ConstCommandBuilder::<8>::new();
        builder.push_mut(camera_id.to_address_byte());
        builder.append_mut(&[0x01, 0x7E, 0x04, 0x72]);
        builder.push_mut(self.control1);
        builder.push_mut(self.control2);
        builder.terminate().build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::InquiryKind> {
        None
    }

    fn validate_for_model(
        &self,
        model: crate::constants::CameraVariant,
    ) -> Result<(), crate::Error> {
        use crate::constants::CameraVariant;
        use std::borrow::Cow;

        match model {
            CameraVariant::SonyFR7 => Ok(()),
            _ => Err(crate::Error::ModelValidation {
                model,
                command: Cow::Borrowed("DirectMenuControl"),
                reason: Cow::Borrowed("Direct menu control is only supported on Sony FR7 cameras"),
            }),
        }
    }
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
