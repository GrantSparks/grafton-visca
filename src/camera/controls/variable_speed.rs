//! Variable speed mode control implementation for PTZ cameras.
//!
//! This module provides variable speed mode control functionality including:
//! - Speed mode selection (24-step or 50-step resolution)
//! - Fine-grained control over pan/tilt movement speeds
//! - Enhanced precision for smooth camera movements
//!
//! Variable speed modes allow for more precise control over camera movement
//! speeds, particularly useful for smooth camera operations in professional
//! video production. The 50-step mode provides finer granularity than the
//! standard 24-step mode, enabling more precise speed control.
//!
//! This feature is primarily available on professional cameras like the Sony FR7
//! that support advanced movement control capabilities.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::{SetVariableSpeedMode, VariableSpeedMode},
    mode::Mode,
    Error,
};

/// Variable speed mode control for cameras.
///
/// This trait provides variable speed control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Speed Modes
///
/// - **Standard 24-step**: Traditional VISCA speed control with 24 discrete levels
/// - **Fine 50-step**: Enhanced precision with 50 discrete speed levels
///
/// # Benefits of Fine Speed Control
///
/// - Smoother camera movements for professional video production
/// - Better control over acceleration and deceleration
/// - More precise speed matching for complex camera moves
/// - Enhanced capability for programmatic camera control
///
/// # Compatibility
///
/// Variable speed modes are primarily supported on professional cameras
/// like the Sony FR7. Check camera documentation for availability.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.set_variable_speed_mode(VariableSpeedMode::Fine50)?;  // Enable fine control
/// camera.set_variable_speed_mode(VariableSpeedMode::Standard24)?;  // Standard mode
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.set_variable_speed_mode(VariableSpeedMode::Fine50).await?;  // Enable fine control
/// camera.set_variable_speed_mode(VariableSpeedMode::Standard24).await?;  // Standard mode
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait VariableSpeedControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Configures the camera's speed control resolution for pan/tilt movements.
    /// The 50-step mode provides finer granularity for more precise control.
    ///
    /// # Parameters
    /// - `mode`: The variable speed mode to set (Standard24 or Fine50)
    ///
    /// # Compatibility
    /// This feature is primarily available on Sony FR7 and other professional
    /// cameras that support enhanced speed control.
    ///
    /// # Errors
    /// Returns an error if the command fails or variable speed modes are not supported.
    fn set_variable_speed_mode(
        &self,
        mode: VariableSpeedMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> VariableSpeedControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::VariableSpeed,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> M::Fut<'_, Result<(), Error>> {
        let cmd = SetVariableSpeedMode::new(mode);
        self.execute(cmd)
    }
}

#[cfg(test)]
mod tests {
    use crate::command::{SetVariableSpeedMode, VariableSpeedMode};

    #[test]
    fn test_variable_speed_mode_command_creation() {
        // Test that commands can be created correctly
        let cmd = SetVariableSpeedMode::new(VariableSpeedMode::Standard24);
        assert!(matches!(cmd.mode, VariableSpeedMode::Standard24));

        let cmd = SetVariableSpeedMode::new(VariableSpeedMode::Fine50);
        assert!(matches!(cmd.mode, VariableSpeedMode::Fine50));
    }
}
