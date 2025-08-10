//! Variable speed mode control methods for Sony FR7.
//!
//! This module provides methods for controlling the variable speed mode
//! on cameras that support it (currently only Sony FR7).

use crate::{
    command::{Response, VariableSpeedMode, VariableSpeedModeCommand},
    error::Error,
};

/// Async methods for variable speed mode control.
#[cfg(feature = "async")]
#[allow(async_fn_in_trait)]
pub trait VariableSpeedOps {
    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    ///
    /// # Arguments
    /// * `mode` - The speed mode to set
    ///
    /// # Returns
    /// Result indicating success or error
    async fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error>;
}

#[cfg(feature = "async")]
impl<P, T> VariableSpeedOps for crate::camera::generic::Camera<P, T>
where
    P: crate::capabilities::Profile
        + crate::capabilities::VariableSpeed
        + crate::capabilities::HasVariableSpeed,
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error> {
        // No runtime check needed - compile-time guarantee via HasVariableSpeed marker trait
        let cmd = VariableSpeedModeCommand::new(mode);
        match self.send_command(&cmd).await? {
            Response::CmdAck | Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

/// Blocking methods for variable speed mode control.
pub trait VariableSpeedOpsBlocking {
    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    ///
    /// # Arguments
    /// * `mode` - The speed mode to set
    ///
    /// # Returns
    /// Result indicating success or error
    fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error>;
}

impl<P, T> VariableSpeedOpsBlocking for crate::camera::generic::Camera<P, T>
where
    P: crate::capabilities::Profile
        + crate::capabilities::VariableSpeed
        + crate::capabilities::HasVariableSpeed,
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error> {
        // No runtime check needed - compile-time guarantee via HasVariableSpeed marker trait
        let cmd = VariableSpeedModeCommand::new(mode);
        match self.send_command_blocking(&cmd)? {
            Response::CmdAck | Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_speed_mode_command_creation() {
        // Test that commands can be created correctly
        let cmd = VariableSpeedModeCommand::new(VariableSpeedMode::Standard24);
        assert!(matches!(cmd.mode, VariableSpeedMode::Standard24));

        let cmd = VariableSpeedModeCommand::new(VariableSpeedMode::Fine50);
        assert!(matches!(cmd.mode, VariableSpeedMode::Fine50));
    }
}
