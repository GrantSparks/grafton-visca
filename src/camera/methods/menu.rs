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
pub trait MenuControlOps: Send + Sync {
    /// Show or hide the on-screen menu.
    async fn set_menu_display(&self, display: bool) -> Result<Response, Error>;

    /// Navigate the menu cursor.
    async fn menu_navigate(&self, direction: MenuDirection) -> Result<Response, Error>;

    /// Perform a menu action (select or cancel).
    async fn menu_action(&self, action: MenuAction) -> Result<Response, Error>;
}

/// Async direct menu control methods for cameras that support advanced menu control.
#[cfg(feature = "async")]
pub trait DirectMenuControlOps: MenuControlOps {
    /// Send a direct menu control command.
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error>;

    /// Toggle menu open/close.
    async fn toggle_menu(&self) -> Result<Response, Error>;
}

/// Blocking menu control methods for cameras that support menu navigation.
#[cfg(not(feature = "async"))]
pub trait MenuControlOpsBlocking {
    /// Show or hide the on-screen menu.
    fn set_menu_display(&self, display: bool) -> Result<Response, Error>;

    /// Navigate the menu cursor.
    fn menu_navigate(&self, direction: MenuDirection) -> Result<Response, Error>;

    /// Perform a menu action (select or cancel).
    fn menu_action(&self, action: MenuAction) -> Result<Response, Error>;
}

/// Blocking direct menu control methods for cameras that support advanced menu control.
#[cfg(not(feature = "async"))]
pub trait DirectMenuControlOpsBlocking: MenuControlOpsBlocking {
    /// Send a direct menu control command.
    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error>;

    /// Toggle menu open/close.
    fn toggle_menu(&self) -> Result<Response, Error>;
}

/// Implementation for async cameras with basic menu control.
#[cfg(feature = "async")]
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
}

/// Implementation for async cameras with direct menu control.
#[cfg(feature = "async")]
impl<
        P: crate::capabilities::Profile + crate::capabilities::HasDirectMenuControl,
        T: crate::transport::Transport + Send + Sync + 'static,
    > DirectMenuControlOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error> {
        let cmd = DirectMenuControlCommand::new(control1, control2);
        self.send_command(&cmd).await
    }

    async fn toggle_menu(&self) -> Result<Response, Error> {
        let cmd = DirectMenuControlCommand::open_close();
        self.send_command(&cmd).await
    }
}

/// Implementation for blocking cameras with basic menu control.
#[cfg(not(feature = "async"))]
impl<
        P: crate::capabilities::Profile,
        T: crate::transport::Transport
            + Send
            + Sync
            + 'static
            + crate::transport::core::BlockingTransport,
    > MenuControlOpsBlocking for crate::camera::generic::Camera<P, T>
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
}

/// Implementation for blocking cameras with direct menu control.
#[cfg(not(feature = "async"))]
impl<
        P: crate::capabilities::Profile + crate::capabilities::HasDirectMenuControl,
        T: crate::transport::Transport
            + Send
            + Sync
            + 'static
            + crate::transport::core::BlockingTransport,
    > DirectMenuControlOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<Response, Error> {
        let cmd = DirectMenuControlCommand::new(control1, control2);
        self.send_command_blocking(&cmd)
    }

    fn toggle_menu(&self) -> Result<Response, Error> {
        let cmd = DirectMenuControlCommand::open_close();
        self.send_command_blocking(&cmd)
    }
}
