//! Variable speed mode control methods for Sony FR7.
//!
//! This module provides methods for controlling the variable speed mode
//! on cameras that support it (currently only Sony FR7).

use crate::{
    capabilities::FeatureDetection,
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
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> VariableSpeedOps
    for crate::camera::generic::Camera<P, T>
{
    async fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error> {
        // Check if camera supports variable speed mode
        if !self.supports_feature(crate::CameraFeature::VariableSpeedMode) {
            return Err(Error::FeatureNotSupported {
                feature: "Variable speed mode",
            });
        }

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

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport>
    VariableSpeedOpsBlocking for crate::camera::generic::Camera<P, T>
{
    fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<(), Error> {
        // Check if camera supports variable speed mode
        if !self.supports_feature(crate::CameraFeature::VariableSpeedMode) {
            return Err(Error::FeatureNotSupported {
                feature: "Variable speed mode",
            });
        }

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
