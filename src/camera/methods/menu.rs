//! Menu control methods for cameras.

use crate::{
    command::{MenuAction, MenuDirection},
    Error,
};

/// Async menu control methods for cameras that support menu navigation.
#[cfg(feature = "async")]
pub trait MenuControl: Send + Sync {
    /// Show or hide the on-screen menu.
    async fn set_menu_display(&self, display: bool) -> Result<(), Error>;

    /// Navigate the menu cursor.
    async fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error>;

    /// Perform a menu action (select or cancel).
    async fn menu_action(&self, action: MenuAction) -> Result<(), Error>;
}

/// Async direct menu control methods for cameras that support advanced menu control.
#[cfg(feature = "async")]
pub trait DirectMenuControl: MenuControl {
    /// Send a direct menu control command.
    async fn direct_menu_control(&mut self, control1: u8, control2: u8) -> Result<(), Error>;

    /// Toggle menu open/close.
    async fn toggle_menu(&self) -> Result<(), Error>;
}

/// Blocking menu control methods for cameras that support menu navigation.
pub trait MenuControlBlocking {
    /// Show or hide the on-screen menu.
    fn set_menu_display(&mut self, display: bool) -> Result<(), Error>;

    /// Navigate the menu cursor.
    fn menu_navigate(&mut self, direction: MenuDirection) -> Result<(), Error>;

    /// Perform a menu action (select or cancel).
    fn menu_action(&mut self, action: MenuAction) -> Result<(), Error>;
}

/// Blocking direct menu control methods for cameras that support advanced menu control.
pub trait DirectMenuControlBlocking: MenuControlBlocking {
    /// Send a direct menu control command.
    fn direct_menu_control(&mut self, control1: u8, control2: u8) -> Result<(), Error>;

    /// Toggle menu open/close.
    fn toggle_menu(&mut self) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> MenuControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn set_menu_display(&self, display: bool) -> Result<(), Error> {
        use crate::command::menu::MenuDisplayCommand;

        let cmd = MenuDisplayCommand::new(display);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn menu_navigate(&self, direction: MenuDirection) -> Result<(), Error> {
        use crate::command::menu::MenuNavigate;

        let cmd = MenuNavigate::new(direction);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn menu_action(&self, action: MenuAction) -> Result<(), Error> {
        use crate::command::menu::MenuActionCmd;

        let cmd = MenuActionCmd::new(action);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Async implementation for DirectMenuControlOps
#[cfg(feature = "async")]
impl<P, T, E> DirectMenuControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn direct_menu_control(&mut self, control1: u8, control2: u8) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControl;

        let cmd = DirectMenuControl::new(control1, control2);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn toggle_menu(&self) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControl;

        let cmd = DirectMenuControl::open_close();
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> MenuControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn set_menu_display(&mut self, display: bool) -> Result<(), Error> {
        use crate::command::menu::MenuDisplayCommand;

        let cmd = MenuDisplayCommand::new(display);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn menu_navigate(&mut self, direction: MenuDirection) -> Result<(), Error> {
        use crate::command::menu::MenuNavigate;

        let cmd = MenuNavigate::new(direction);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn menu_action(&mut self, action: MenuAction) -> Result<(), Error> {
        use crate::command::menu::MenuActionCmd;

        let cmd = MenuActionCmd::new(action);
        self.send_command(&cmd)?;
        Ok(())
    }
}

// Blocking implementation for DirectMenuControlOpsBlocking
#[cfg(not(feature = "async"))]
impl<P, T> DirectMenuControlBlocking
    for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn direct_menu_control(&mut self, control1: u8, control2: u8) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControl;

        let cmd = DirectMenuControl::new(control1, control2);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn toggle_menu(&mut self) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControl;

        let cmd = DirectMenuControl::open_close();
        self.send_command(&cmd)?;
        Ok(())
    }
}
