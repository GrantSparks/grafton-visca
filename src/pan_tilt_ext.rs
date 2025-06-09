//! High-level extension trait for pan/tilt control operations.

// Crate imports
use crate::{
    command::pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    error::Error as ViscaError,
    ViscaDevice, ViscaResponse,
};

/// Extension trait providing high-level pan/tilt control methods.
pub trait ViscaPanTiltExt: ViscaDevice {
    /// Move camera to an absolute pan/tilt position.
    ///
    /// # Arguments
    /// * `pan` - Target pan position (-2448 to 2448)
    /// * `tilt` - Target tilt position (-1296 to 1296)
    /// * `speed` - Optional pan and tilt speeds. If None, uses default speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt, PanSpeed, TiltSpeed};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Move to center position at default speed
    /// client.move_to_position(0, 0, None)?;
    ///
    /// // Move to specific position with custom speeds
    /// let pan_speed = PanSpeed::new(10)?;
    /// let tilt_speed = TiltSpeed::new(15)?;
    /// client.move_to_position(1000, -500, Some((pan_speed, tilt_speed)))?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ViscaError::UnexpectedResponseType` if camera returns unexpected response.
    /// Returns camera-specific errors if the command is rejected.
    /// Returns transport errors if communication fails.
    fn move_to_position(
        &mut self,
        pan: i16,
        tilt: i16,
        speed: Option<(PanSpeed, TiltSpeed)>,
    ) -> Result<(), ViscaError> {
        let (pan_speed, tilt_speed) = speed.unwrap_or_else(|| {
            // Use safe default values that won't fail
            // 18 for pan and 14 for tilt are within valid ranges (0-24 and 0-20)
            (
                PanSpeed::new(18).unwrap_or(PanSpeed::ZERO),
                TiltSpeed::new(14).unwrap_or(TiltSpeed::ZERO),
            )
        });
        let command = PanTiltCommand::AbsolutePosition {
            pan_speed,
            tilt_speed,
            pan,
            tilt,
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Move camera relative to its current position.
    ///
    /// # Arguments
    /// * `pan_delta` - Relative pan movement (-2448 to 2448)
    /// * `tilt_delta` - Relative tilt movement (-1296 to 1296)
    /// * `speed` - Optional pan and tilt speeds. If None, uses default speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt, PanSpeed, TiltSpeed};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Move 100 units right and 50 units up
    /// client.move_relative(100, 50, None)?;
    ///
    /// // Move left and down with custom speeds
    /// let pan_speed = PanSpeed::new(20)?;
    /// let tilt_speed = TiltSpeed::new(20)?;
    /// client.move_relative(-200, -100, Some((pan_speed, tilt_speed)))?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ViscaError::UnexpectedResponseType` if camera returns unexpected response.
    /// Returns camera-specific errors if the command is rejected.
    /// Returns transport errors if communication fails.
    fn move_relative(
        &mut self,
        pan_delta: i16,
        tilt_delta: i16,
        speed: Option<(PanSpeed, TiltSpeed)>,
    ) -> Result<(), ViscaError> {
        let (pan_speed, tilt_speed) = speed.unwrap_or_else(|| {
            // Use safe default values that won't fail
            // 18 for pan and 14 for tilt are within valid ranges (0-24 and 0-20)
            (
                PanSpeed::new(18).unwrap_or(PanSpeed::ZERO),
                TiltSpeed::new(14).unwrap_or(TiltSpeed::ZERO),
            )
        });
        let command = PanTiltCommand::RelativePosition {
            pan_speed,
            tilt_speed,
            pan: pan_delta,
            tilt: tilt_delta,
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Start continuous pan/tilt movement in the specified direction.
    ///
    /// # Arguments
    /// * `direction` - Direction of movement
    /// * `pan_speed` - Pan speed
    /// * `tilt_speed` - Tilt speed
    ///
    /// # Errors
    /// Returns `ViscaError::UnexpectedResponseType` if camera returns unexpected response.
    /// Returns camera-specific errors if the command is rejected.
    /// Returns transport errors if communication fails.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt, PanTiltDirection, PanSpeed, TiltSpeed};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Start moving up-right
    /// let pan_speed = PanSpeed::new(10)?;
    /// let tilt_speed = TiltSpeed::new(10)?;
    /// client.start_moving(PanTiltDirection::UpRight, pan_speed, tilt_speed)?;
    ///
    /// // Start moving left at maximum speed
    /// let pan_speed = PanSpeed::new(24)?;
    /// let tilt_speed = TiltSpeed::new(0)?;
    /// client.start_moving(PanTiltDirection::Left, pan_speed, tilt_speed)?;
    /// # Ok(())
    /// # }
    /// ```
    fn start_moving(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), ViscaError> {
        let command = PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Stop all pan/tilt movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.stop_movement()?;
    /// # Ok(())
    /// # }
    /// ```
    fn stop_movement(&mut self) -> Result<(), ViscaError> {
        let pan_speed = PanSpeed::new(0)?;
        let tilt_speed = TiltSpeed::new(0)?;
        let command = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed,
            tilt_speed,
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Return camera to home position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.go_home()?;
    /// # Ok(())
    /// # }
    /// ```
    fn go_home(&mut self) -> Result<(), ViscaError> {
        let command = PanTiltCommand::Home;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: ViscaDevice> ViscaPanTiltExt for T {}
