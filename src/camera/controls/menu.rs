//! Menu control methods for cameras.

use crate::{
    command::{MenuAction, MenuDirection},
    Error,
};

/// Menu control methods for cameras that support menu navigation.
pub trait MenuControl {
    /// Show or hide the on-screen menu.
    ///
    /// # Arguments
    /// * `display` - Whether to show (true) or hide (false) the menu
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    #[cfg(feature = "async")]
    fn set_menu_display(
        &self,
        display: bool,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Show or hide the on-screen menu.
    ///
    /// # Arguments
    /// * `display` - Whether to show (true) or hide (false) the menu
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    #[cfg(not(feature = "async"))]
    fn set_menu_display(&mut self, display: bool) -> Result<(), Error>;

    /// Navigate the menu cursor.
    ///
    /// # Arguments
    /// * `direction` - The direction to move the cursor
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    #[cfg(feature = "async")]
    fn menu_navigate(
        &self,
        direction: MenuDirection,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Navigate the menu cursor.
    ///
    /// # Arguments
    /// * `direction` - The direction to move the cursor
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    #[cfg(not(feature = "async"))]
    fn menu_navigate(&mut self, direction: MenuDirection) -> Result<(), Error>;

    /// Perform a menu action (select or cancel).
    ///
    /// # Arguments
    /// * `action` - The action to perform
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    #[cfg(feature = "async")]
    fn menu_action(
        &self,
        action: MenuAction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Perform a menu action (select or cancel).
    ///
    /// # Arguments
    /// * `action` - The action to perform
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support menu control.
    #[cfg(not(feature = "async"))]
    fn menu_action(&mut self, action: MenuAction) -> Result<(), Error>;
}

/// Direct menu control methods for cameras that support advanced menu control.
pub trait DirectMenuControl: MenuControl {
    /// Send a direct menu control command.
    ///
    /// # Arguments
    /// * `control1` - First control byte
    /// * `control2` - Second control byte
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    #[cfg(feature = "async")]
    fn direct_menu_control(
        &self,
        control1: u8,
        control2: u8,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Send a direct menu control command.
    ///
    /// # Arguments
    /// * `control1` - First control byte
    /// * `control2` - Second control byte
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    #[cfg(not(feature = "async"))]
    fn direct_menu_control(&mut self, control1: u8, control2: u8) -> Result<(), Error>;

    /// Toggle menu open/close.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    #[cfg(feature = "async")]
    fn toggle_menu(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Toggle menu open/close.
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support direct menu control.
    #[cfg(not(feature = "async"))]
    fn toggle_menu(&mut self) -> Result<(), Error>;
}

// Unified implementation of MenuControl for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> MenuControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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
        use crate::command::menu::MenuActionCommand;

        let cmd = MenuActionCommand::new(action);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Unified implementation of DirectMenuControl for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> DirectMenuControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> Result<(), Error> {
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

// Unified implementation of MenuControl for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> MenuControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn set_menu_display(&mut self, display: bool) -> Result<(), Error> {
        use crate::command::menu::MenuDisplayCommand;
        let cmd = MenuDisplayCommand::new(display);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn menu_navigate(&mut self, direction: MenuDirection) -> Result<(), Error> {
        use crate::command::menu::MenuNavigate;

        let cmd = MenuNavigate::new(direction);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn menu_action(&mut self, action: MenuAction) -> Result<(), Error> {
        use crate::command::menu::MenuActionCommand;

        let cmd = MenuActionCommand::new(action);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }
}

// Unified implementation of DirectMenuControl for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> DirectMenuControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::MenuControl + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn direct_menu_control(&mut self, control1: u8, control2: u8) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControl;
        let cmd = DirectMenuControl::new(control1, control2);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn toggle_menu(&mut self) -> Result<(), Error> {
        use crate::command::menu::DirectMenuControl;

        let cmd = DirectMenuControl::open_close();
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }
}
