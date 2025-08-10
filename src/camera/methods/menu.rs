//! Menu control methods for cameras.

use crate::{
    command::{
        DirectMenuControlCommand, MenuAction, MenuActionCommand, MenuDirection, MenuDisplayCommand,
        MenuNavigateCommand, Response,
    },
    Error,
};

/// Async menu control methods for cameras that support menu navigation.
#[cfg(feature = "async")]
#[async_trait::async_trait]
pub trait MenuControlOps: Send + Sync {
    /// Show or hide the on-screen menu.
    async fn set_menu_display(&self, display: bool) -> Result<Response, Error>;

    /// Navigate the menu cursor.
    async fn menu_navigate(&self, direction: MenuDirection) -> Result<Response, Error>;

    /// Perform a menu action (select or cancel).
    async fn menu_action(&self, action: MenuAction) -> Result<Response, Error>;

    /// Send a direct menu control command (FR7 only).
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error>;

    /// Toggle menu open/close (FR7 only).
    async fn toggle_menu(&self) -> Result<Response, Error>;
}

/// Blocking menu control methods for cameras that support menu navigation.
pub trait MenuControlOpsBlocking {
    /// Show or hide the on-screen menu.
    fn set_menu_display(&self, display: bool) -> Result<Response, Error>;

    /// Navigate the menu cursor.
    fn menu_navigate(&self, direction: MenuDirection) -> Result<Response, Error>;

    /// Perform a menu action (select or cancel).
    fn menu_action(&self, action: MenuAction) -> Result<Response, Error>;

    /// Send a direct menu control command (FR7 only).
    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error>;

    /// Toggle menu open/close (FR7 only).
    fn toggle_menu(&self) -> Result<Response, Error>;
}

/// Implementation for async cameras with menu control.
#[cfg(feature = "async")]
#[async_trait::async_trait]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    MenuControlOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn set_menu_display(&self, display: bool) -> Result<Response, Error> {
        let cmd = MenuDisplayCommand::new(display);
        self.send_command(&cmd).await
    }

    async fn menu_navigate(&self, direction: MenuDirection) -> Result<Response, Error> {
        let cmd = MenuNavigateCommand::new(direction);
        self.send_command(&cmd).await
    }

    async fn menu_action(&self, action: MenuAction) -> Result<Response, Error> {
        let cmd = MenuActionCommand::new(action);
        self.send_command(&cmd).await
    }

    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error> {
        // Check if camera supports direct menu control (FR7 only)
        if self.model_name() != "Sony FR7" {
            return Err(Error::FeatureNotSupported {
                feature: "Direct menu control",
            });
        }
        let cmd = DirectMenuControlCommand::new(control1, control2);
        self.send_command(&cmd).await
    }

    async fn toggle_menu(&self) -> Result<Response, Error> {
        // Check if camera supports direct menu control (FR7 only)
        if self.model_name() != "Sony FR7" {
            return Err(Error::FeatureNotSupported {
                feature: "Direct menu control",
            });
        }
        let cmd = DirectMenuControlCommand::open_close();
        self.send_command(&cmd).await
    }
}

/// Implementation for blocking cameras with menu control.
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    MenuControlOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn set_menu_display(&self, display: bool) -> Result<Response, Error> {
        let cmd = MenuDisplayCommand::new(display);
        self.send_command_blocking(&cmd)
    }

    fn menu_navigate(&self, direction: MenuDirection) -> Result<Response, Error> {
        let cmd = MenuNavigateCommand::new(direction);
        self.send_command_blocking(&cmd)
    }

    fn menu_action(&self, action: MenuAction) -> Result<Response, Error> {
        let cmd = MenuActionCommand::new(action);
        self.send_command_blocking(&cmd)
    }

    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error> {
        // Check if camera supports direct menu control (FR7 only)
        if self.model_name() != "Sony FR7" {
            return Err(Error::FeatureNotSupported {
                feature: "Direct menu control",
            });
        }
        let cmd = DirectMenuControlCommand::new(control1, control2);
        self.send_command_blocking(&cmd)
    }

    fn toggle_menu(&self) -> Result<Response, Error> {
        // Check if camera supports direct menu control (FR7 only)
        if self.model_name() != "Sony FR7" {
            return Err(Error::FeatureNotSupported {
                feature: "Direct menu control",
            });
        }
        let cmd = DirectMenuControlCommand::open_close();
        self.send_command_blocking(&cmd)
    }
}
