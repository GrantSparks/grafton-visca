//! Menu control methods for cameras.

use crate::{
    command::{
        util::map_ack_to_unit, DirectMenuControlCommand, MenuAction, MenuActionCommand,
        MenuDirection, MenuDisplayCommand, MenuNavigateCommand,
    },
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

/// Implementation for async cameras with basic menu control.
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    MenuControlOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn set_menu_display(&self, display: bool) -> Result<(), Error> {
        let cmd = MenuDisplayCommand::new(display);
        let resp = self.send_command(&cmd).await?;
        map_ack_to_unit(resp)
    }

    async fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error> {
        let cmd = MenuNavigateCommand::new(direction);
        let resp = self.send_command(&cmd).await?;
        map_ack_to_unit(resp)
    }

    async fn menu_action(&self, action: MenuAction) -> Result<(), Error> {
        let cmd = MenuActionCommand::new(action);
        let resp = self.send_command(&cmd).await?;
        map_ack_to_unit(resp)
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
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error> {
        let cmd = DirectMenuControlCommand::new(control1, control2);
        let resp = self.send_command(&cmd).await?;
        map_ack_to_unit(resp)
    }

    async fn toggle_menu(&self) -> Result<(), Error> {
        let cmd = DirectMenuControlCommand::open_close();
        let resp = self.send_command(&cmd).await?;
        map_ack_to_unit(resp)
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
    fn set_menu_display(&self, display: bool) -> Result<(), Error> {
        let cmd = MenuDisplayCommand::new(display);
        let resp = self.send_command_blocking(&cmd)?;
        map_ack_to_unit(resp)
    }

    fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error> {
        let cmd = MenuNavigateCommand::new(direction);
        let resp = self.send_command_blocking(&cmd)?;
        map_ack_to_unit(resp)
    }

    fn menu_action(&self, action: MenuAction) -> Result<(), Error> {
        let cmd = MenuActionCommand::new(action);
        let resp = self.send_command_blocking(&cmd)?;
        map_ack_to_unit(resp)
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
    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error> {
        let cmd = DirectMenuControlCommand::new(control1, control2);
        let resp = self.send_command_blocking(&cmd)?;
        map_ack_to_unit(resp)
    }

    fn toggle_menu(&self) -> Result<(), Error> {
        let cmd = DirectMenuControlCommand::open_close();
        let resp = self.send_command_blocking(&cmd)?;
        map_ack_to_unit(resp)
    }
}
