//! High-level extension trait for focus control operations.

// Crate imports
use crate::{
    command::focus::{FocusCommand, FocusSpeed},
    error::Error as Error,
    Response, Transport,
};

/// Extension trait providing high-level focus control methods.
pub trait ViscaFocusExt: Transport {
    /// Enable or disable auto-focus mode.
    ///
    /// # Arguments
    /// * `enabled` - true to enable auto-focus, false for manual focus
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaFocusExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Enable auto-focus
    /// client.set_auto_focus(true)?;
    ///
    /// // Switch to manual focus
    /// client.set_auto_focus(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_auto_focus(&mut self, enabled: bool) -> Result<(), Error> {
        let command = if enabled {
            FocusCommand::Auto
        } else {
            FocusCommand::Manual
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Move focus to an absolute position.
    ///
    /// # Arguments
    /// * `position` - Target focus position (0x1000 to 0xF000)
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaFocusExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Focus to near position
    /// client.focus_to(0x1000)?;
    ///
    /// // Focus to far position
    /// client.focus_to(0xF000)?;
    ///
    /// // Focus to mid-range
    /// client.focus_to(0x8000)?;
    /// # Ok(())
    /// # }
    /// ```
    fn focus_to(&mut self, position: u16) -> Result<(), Error> {
        let command = FocusCommand::Direct(position);
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Start focusing near (closer to camera).
    ///
    /// # Arguments
    /// * `speed` - Optional focus speed. If None, uses standard speed.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaFocusExt, FocusSpeed};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Focus near at standard speed
    /// client.focus_near(None)?;
    ///
    /// // Focus near at maximum speed
    /// client.focus_near(Some(FocusSpeed::new(7)?))?;
    /// # Ok(())
    /// # }
    /// ```
    fn focus_near(&mut self, speed: Option<FocusSpeed>) -> Result<(), Error> {
        let command = speed.map_or(FocusCommand::NearStandard, FocusCommand::NearVariable);
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Start focusing far (farther from camera).
    ///
    /// # Arguments
    /// * `speed` - Optional focus speed. If None, uses standard speed.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaFocusExt, FocusSpeed};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Focus far at standard speed
    /// client.focus_far(None)?;
    ///
    /// // Focus far at slow speed
    /// client.focus_far(Some(FocusSpeed::new(2)?))?;
    /// # Ok(())
    /// # }
    /// ```
    fn focus_far(&mut self, speed: Option<FocusSpeed>) -> Result<(), Error> {
        let command = speed.map_or(FocusCommand::FarStandard, FocusCommand::FarVariable);
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Stop focus movement.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaFocusExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Start focusing near
    /// client.focus_near(None)?;
    ///
    /// // ... wait some time ...
    ///
    /// // Stop focusing
    /// client.stop_focus()?;
    /// # Ok(())
    /// # }
    /// ```
    fn stop_focus(&mut self) -> Result<(), Error> {
        let command = FocusCommand::Stop;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Trigger one-push auto-focus.
    ///
    /// This performs a single auto-focus operation, even when in manual focus mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaFocusExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Set manual focus mode
    /// client.set_auto_focus(false)?;
    ///
    /// // Trigger one-time auto-focus
    /// client.trigger_one_push_focus()?;
    /// # Ok(())
    /// # }
    /// ```
    fn trigger_one_push_focus(&mut self) -> Result<(), Error> {
        let command = FocusCommand::OnePushTrigger;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: Transport> ViscaFocusExt for T {}
