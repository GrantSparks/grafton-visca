//! Unified menu control implementation using Mode trait.

use crate::{
    camera::CameraSend,
    command::{MenuAction, MenuDirection},
    mode::Mode,
    Error,
};

/// Unified menu control methods for cameras that support menu navigation.
pub trait MenuControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Show or hide the on-screen menu.
    ///
    /// # Arguments
    /// * `display` - Whether to show (true) or hide (false) the menu
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    fn set_menu_display(&self, display: bool) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Navigate the menu cursor.
    ///
    /// # Arguments
    /// * `direction` - The direction to move the cursor
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    fn menu_navigate(
        &self,
        direction: MenuDirection,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Perform a menu action (select or cancel).
    ///
    /// # Arguments
    /// * `action` - The action to perform
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    fn menu_action(&self, action: MenuAction) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

/// Unified direct menu control methods for cameras that support advanced menu control.
pub trait DirectMenuControl: MenuControl {
    /// Send a direct menu control command.
    ///
    /// # Arguments
    /// * `control1` - First control byte
    /// * `control2` - Second control byte
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    fn direct_menu_control(
        &self,
        control1: u8,
        control2: u8,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Toggle menu open/close.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    fn toggle_menu(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for MenuControl
impl<M, P, Tr, Exec> MenuControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    Self: CameraSend<M>,
{
    type Mode = M;

    fn set_menu_display(&self, display: bool) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::menu::MenuDisplayCommand;
        self.send_and_complete(MenuDisplayCommand::new(display))
    }

    fn menu_navigate(&self, direction: MenuDirection) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::menu::MenuNavigate;
        self.send_and_complete(MenuNavigate::new(direction))
    }

    fn menu_action(&self, action: MenuAction) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::menu::MenuActionCommand;
        self.send_and_complete(MenuActionCommand::new(action))
    }
}

// Single unified implementation for DirectMenuControl
impl<M, P, Tr, Exec> DirectMenuControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    Self: CameraSend<M>,
{
    fn direct_menu_control(&self, control1: u8, control2: u8) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::menu::DirectMenuControl;
        self.send_and_complete(DirectMenuControl::new(control1, control2))
    }

    fn toggle_menu(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::menu::DirectMenuControl;
        self.send_and_complete(DirectMenuControl::open_close())
    }
}
