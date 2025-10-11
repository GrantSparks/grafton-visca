//! Menu control implementation for PTZ cameras.
//!
//! This module provides on-screen menu control functionality including:
//! - Menu display control (show/hide)
//! - Menu navigation (up, down, left, right)
//! - Menu actions (select, cancel)
//! - Direct menu control for advanced operations
//! - Menu toggle functionality
//!
//! Menu controls allow remote operation of the camera's on-screen display (OSD)
//! menu system. This enables configuration of camera settings that may not
//! have dedicated VISCA commands, providing access to manufacturer-specific
//! features and advanced configuration options.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::{MenuAction, MenuDirection},
    mode::Mode,
    Error,
};

/// Menu control methods for cameras that support menu navigation.
///
/// This trait provides basic menu control functionality including display control,
/// navigation, and action commands. It works seamlessly for both blocking and
/// async cameras through the Mode trait system.
///
/// # Menu Operations
///
/// - **Display Control**: Show or hide the on-screen menu
/// - **Navigation**: Move cursor up, down, left, right through menu items
/// - **Actions**: Select current item or cancel/go back
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.set_menu_display(true)?;  // Show menu
/// camera.menu_navigate(MenuDirection::Down)?;  // Navigate down
/// camera.menu_action(MenuAction::Select)?;  // Select item
/// camera.set_menu_display(false)?;  // Hide menu
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.set_menu_display(true).await?;  // Show menu
/// camera.menu_navigate(MenuDirection::Down).await?;  // Navigate down
/// camera.menu_action(MenuAction::Select).await?;  // Select item
/// camera.set_menu_display(false).await?;  // Hide menu
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait MenuControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Show or hide the on-screen menu.
    ///
    /// Controls the visibility of the camera's on-screen display (OSD) menu.
    /// When shown, the menu typically appears as an overlay on the video output.
    ///
    /// # Parameters
    /// - `display`: True to show the menu, false to hide it
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    fn set_menu_display(
        &self,
        display: bool,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Navigate the menu cursor.
    ///
    /// Moves the menu cursor in the specified direction through the menu items.
    /// The menu must be visible for navigation to have any effect.
    ///
    /// # Parameters
    /// - `direction`: The direction to move the cursor (Up, Down, Left, Right)
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    fn menu_navigate(
        &self,
        direction: MenuDirection,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Perform a menu action (select or cancel).
    ///
    /// Executes an action on the currently highlighted menu item.
    /// Select typically enters a submenu or changes a setting,
    /// while Cancel typically goes back or exits.
    ///
    /// # Parameters
    /// - `action`: The action to perform (Select or Cancel)
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    fn menu_action(
        &self,
        action: MenuAction,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Direct menu control methods for cameras that support advanced menu control.
///
/// This trait extends basic menu control with advanced functionality including
/// direct control commands and menu toggle operations. It provides lower-level
/// access to menu functions that may not be available through standard navigation.
///
/// # Advanced Operations
///
/// - **Direct Control**: Send raw control bytes for manufacturer-specific functions
/// - **Menu Toggle**: Quick open/close functionality
///
/// # Examples
///
/// ```ignore
/// camera.toggle_menu()?;  // Quick toggle menu
/// camera.direct_menu_control(0x01, 0x02)?;  // Send raw control
/// ```
pub trait DirectMenuControl: MenuControl {
    /// Send a direct menu control command.
    ///
    /// Sends raw control bytes directly to the menu system. This provides
    /// access to manufacturer-specific menu functions that may not be
    /// available through standard navigation commands.
    ///
    /// # Parameters
    /// - `control1`: First control byte
    /// - `control2`: Second control byte
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    fn direct_menu_control(
        &self,
        control1: u8,
        control2: u8,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Toggle menu open/close.
    ///
    /// Toggles the menu visibility state - opens the menu if closed,
    /// closes the menu if open. This is a convenience function for
    /// quick menu access.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    fn toggle_menu(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for MenuControl
impl<M, P, Tr, Exec> MenuControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + crate::capabilities::MenuCapability + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_menu_display(
        &self,
        display: bool,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::menu::SetMenuDisplay;
        self.execute_with_opts(SetMenuDisplay::new(display), opts)
    }

    fn menu_navigate(
        &self,
        direction: MenuDirection,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::menu::MenuNavigate;
        self.execute_with_opts(MenuNavigate::new(direction), opts)
    }

    fn menu_action(
        &self,
        action: MenuAction,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::menu::PerformMenuAction;
        self.execute_with_opts(PerformMenuAction::new(action), opts)
    }
}

// Single unified implementation for DirectMenuControl
impl<M, P, Tr, Exec> DirectMenuControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + crate::capabilities::MenuCapability + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    fn direct_menu_control(
        &self,
        control1: u8,
        control2: u8,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::menu::DirectMenuControl;
        self.execute_with_opts(DirectMenuControl::new(control1, control2), opts)
    }

    fn toggle_menu(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::menu::DirectMenuControl;
        self.execute_with_opts(DirectMenuControl::open_close(), opts)
    }
}
