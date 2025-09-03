//! Variable speed mode control methods for Sony FR7.
//!
//! This module provides methods for controlling the variable speed mode
//! on cameras that support it (currently only Sony FR7).

use crate::{
    command::{VariableSpeedMode, VariableSpeedModeCommand, ViscaResponse},
    error::Error,
};

/// Variable speed mode control for cameras.
///
/// This trait provides variable speed control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait VariableSpeedControl {
    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    ///
    /// # Arguments
    /// * `mode` - The speed mode to set
    ///
    /// # Returns
    /// Result indicating success or error
    #[cfg(feature = "async")]
    fn set_variable_speed_mode(
        &self,
        mode: VariableSpeedMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    ///
    /// # Arguments
    /// * `mode` - The speed mode to set
    ///
    /// # Returns
    /// Result indicating success or error
    #[cfg(not(feature = "async"))]
    fn set_variable_speed_mode(&mut self, mode: VariableSpeedMode) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, Tr, Exec> VariableSpeedControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + crate::capabilities::VariableSpeed + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error> {
        // No runtime check needed - compile-time guarantee via VariableSpeed capability trait
        let cmd = VariableSpeedModeCommand::new(mode);
        match self.send_command(&cmd).await? {
            ViscaResponse::CmdAck { .. } | ViscaResponse::Completion { .. } => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
// Keep the old trait names for backward compatibility during transition
/// Async variable speed control trait (deprecated, use VariableSpeedControl instead).
/// Blocking variable speed control trait (deprecated, use VariableSpeedControl instead).
// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> VariableSpeedControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::VariableSpeed + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn set_variable_speed_mode(&mut self, mode: VariableSpeedMode) -> Result<(), Error> {
        // No runtime check needed - compile-time guarantee via VariableSpeed capability trait
        let cmd = VariableSpeedModeCommand::new(mode);
        match pollster::block_on(self.send_command(&cmd))? {
            ViscaResponse::CmdAck { .. } | ViscaResponse::Completion { .. } => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::command::{VariableSpeedMode, VariableSpeedModeCommand};

    #[test]
    fn test_variable_speed_mode_command_creation() {
        // Test that commands can be created correctly
        let cmd = VariableSpeedModeCommand::new(VariableSpeedMode::Standard24);
        assert!(matches!(cmd.mode, VariableSpeedMode::Standard24));

        let cmd = VariableSpeedModeCommand::new(VariableSpeedMode::Fine50);
        assert!(matches!(cmd.mode, VariableSpeedMode::Fine50));
    }
}
