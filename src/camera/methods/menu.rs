//! Menu control methods for cameras.

use crate::{
    command::{MenuAction, MenuDirection},
    Error,
};

/// Async menu control methods for cameras that support menu navigation.
#[cfg(feature = "async")]
pub trait MenuControlOps: Send + Sync {
    /// Show or hide the on-screen menu.
    async fn set_menu_display(&self, display: bool) -> Result<(), Error>;

    /// Navigate the menu cursor.
    async fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error>;

    /// Perform a menu action (select or cancel).
    async fn menu_action(&self, action: MenuAction) -> Result<(), Error>;
}

/// Async direct menu control methods for cameras that support advanced menu control.
#[cfg(feature = "async")]
pub trait DirectMenuControlOps: MenuControlOps {
    /// Send a direct menu control command.
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error>;

    /// Toggle menu open/close.
    async fn toggle_menu(&self) -> Result<(), Error>;
}

/// Blocking menu control methods for cameras that support menu navigation.
#[cfg(not(feature = "async"))]
pub trait MenuControlOpsBlocking {
    /// Show or hide the on-screen menu.
    fn set_menu_display(&self, display: bool) -> Result<(), Error>;

    /// Navigate the menu cursor.
    fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error>;

    /// Perform a menu action (select or cancel).
    fn menu_action(&self, action: MenuAction) -> Result<(), Error>;
}

/// Blocking direct menu control methods for cameras that support advanced menu control.
#[cfg(not(feature = "async"))]
pub trait DirectMenuControlOpsBlocking: MenuControlOpsBlocking {
    /// Send a direct menu control command.
    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error>;

    /// Toggle menu open/close.
    fn toggle_menu(&self) -> Result<(), Error>;
}
