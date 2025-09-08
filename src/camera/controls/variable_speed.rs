//! Unified variable speed mode control implementation using Mode trait.

use crate::{
    camera::CameraSend,
    command::{VariableSpeedMode, VariableSpeedModeCommand},
    mode::Mode,
    Error,
};

/// Unified variable speed mode control for cameras.
///
/// This trait provides variable speed control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
pub trait VariableSpeedControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

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
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> VariableSpeedControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::VariableSpeed,
    Self: CameraSend<M>,
{
    type Mode = M;

    fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> M::Ret<'_, Result<(), Error>> {
        let cmd = VariableSpeedModeCommand::new(mode);
        self.send_and_complete(cmd)
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
