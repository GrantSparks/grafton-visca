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
pub trait MenuControlOpsBlocking {
    /// Show or hide the on-screen menu.
    fn set_menu_display(&self, display: bool) -> Result<(), Error>;

    /// Navigate the menu cursor.
    fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error>;

    /// Perform a menu action (select or cancel).
    fn menu_action(&self, action: MenuAction) -> Result<(), Error>;
}

/// Blocking direct menu control methods for cameras that support advanced menu control.
pub trait DirectMenuControlOpsBlocking: MenuControlOpsBlocking {
    /// Send a direct menu control command.
    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error>;

    /// Toggle menu open/close.
    fn toggle_menu(&self) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> MenuControlOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn set_menu_display(&self, display: bool) -> Result<(), Error> {
        use crate::command::menu::MenuDisplayCommand;
        let cmd = MenuDisplayCommand::new(display);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error> {
        use crate::command::menu::MenuNavigateCommand;
        let cmd = MenuNavigateCommand::new(direction);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn menu_action(&self, action: MenuAction) -> Result<(), Error> {
        use crate::command::menu::MenuActionCommand;
        let cmd = MenuActionCommand::new(action);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Async implementation for DirectMenuControlOps
#[cfg(feature = "async")]
impl<P, T, E> DirectMenuControlOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControlCommand;
        let cmd = DirectMenuControlCommand::new(control1, control2);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn toggle_menu(&self) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControlCommand;
        let cmd = DirectMenuControlCommand::open_close();
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> MenuControlOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn set_menu_display(&self, display: bool) -> Result<(), Error> {
        use crate::command::menu::MenuDisplayCommand;
        let cmd = MenuDisplayCommand::new(display);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error> {
        use crate::command::menu::MenuNavigateCommand;
        let cmd = MenuNavigateCommand::new(direction);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn menu_action(&self, action: MenuAction) -> Result<(), Error> {
        use crate::command::menu::MenuActionCommand;
        let cmd = MenuActionCommand::new(action);
        self.send_command(&cmd)?;
        Ok(())
    }
}

// Blocking implementation for DirectMenuControlOpsBlocking
impl<P, T> DirectMenuControlOpsBlocking
    for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControlCommand;
        let cmd = DirectMenuControlCommand::new(control1, control2);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn toggle_menu(&self) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControlCommand;
        let cmd = DirectMenuControlCommand::open_close();
        self.send_command(&cmd)?;
        Ok(())
    }
}
