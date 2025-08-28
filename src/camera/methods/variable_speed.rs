//! Variable speed mode control methods for Sony FR7.
//!
//! This module provides methods for controlling the variable speed mode
//! on cameras that support it (currently only Sony FR7).

use crate::{
    command::{VariableSpeedMode, VariableSpeedModeCmd, ViscaResponse},
    error::Error,
};

/// Async methods for variable speed mode control.
#[cfg(feature = "async")]
pub trait VariableSpeedControl: Send + Sync + 'static {
    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    ///
    /// # Arguments
    /// * `mode` - The speed mode to set
    ///
    /// # Returns
    /// Result indicating success or error
    fn set_variable_speed_mode(
        &self,
        mode: VariableSpeedMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, Tr, Exec> VariableSpeedControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::VariableSpeed
        + crate::capabilities::HasVariableSpeed
        + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error> {
        // No runtime check needed - compile-time guarantee via HasVariableSpeed marker trait
        let cmd = VariableSpeedModeCmd::new(mode);
        match self.send_command(&cmd).await? {
            ViscaResponse::CmdAck | ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

/// Blocking methods for variable speed mode control.
pub trait VariableSpeedControlBlocking {
    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    ///
    /// # Arguments
    /// * `mode` - The speed mode to set
    ///
    /// # Returns
    /// Result indicating success or error
    fn set_variable_speed_mode(&mut self, mode: VariableSpeedMode) -> Result<(), Error>;
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> VariableSpeedControlBlocking for crate::camera::BlockingCamera<P, T>
where
    P: crate::capabilities::Profile
        + crate::capabilities::VariableSpeed
        + crate::capabilities::HasVariableSpeed
        + Default,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn set_variable_speed_mode(&mut self, mode: VariableSpeedMode) -> Result<(), Error> {
        // No runtime check needed - compile-time guarantee via HasVariableSpeed marker trait
        let cmd = VariableSpeedModeCmd::new(mode);
        match self.send_command(&cmd)? {
            ViscaResponse::CmdAck | ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
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
        let cmd = VariableSpeedModeCmd::new(VariableSpeedMode::Standard24);
        assert!(matches!(cmd.mode, VariableSpeedMode::Standard24));

        let cmd = VariableSpeedModeCmd::new(VariableSpeedMode::Fine50);
        assert!(matches!(cmd.mode, VariableSpeedMode::Fine50));
    }
}
