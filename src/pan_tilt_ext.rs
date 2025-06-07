//! High-level extension trait for pan/tilt control operations.

// Crate imports
use crate::{
    command::pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    error::ViscaError,
    ViscaDevice, ViscaResponse,
};

/// Extension trait providing high-level pan/tilt control methods.
pub trait ViscaPanTiltExt: ViscaDevice {
    /// Move camera to an absolute pan/tilt position.
    ///
    /// # Arguments
    /// * `pan` - Target pan position (-2448 to 2448)
    /// * `tilt` - Target tilt position (-1296 to 1296)
    /// * `speed` - Optional pan and tilt speeds (1-24). If None, uses default speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Move to center position at default speed
    /// client.move_to_position(0, 0, None)?;
    ///
    /// // Move to specific position with custom speeds
    /// client.move_to_position(1000, -500, Some((10, 15)))?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ViscaError::InvalidParameter` if pan_speed > 24 or tilt_speed > 20.
    /// Returns `ViscaError::UnexpectedResponseType` if camera returns unexpected response.
    /// Returns camera-specific errors if the command is rejected.
    /// Returns transport errors if communication fails.
    fn move_to_position(
        &mut self,
        pan: i16,
        tilt: i16,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        let (pan_speed, tilt_speed) = speed.unwrap_or((18, 14));
        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
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
    /// * `speed` - Optional pan and tilt speeds (1-24). If None, uses default speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Move 100 units right and 50 units up
    /// client.move_relative(100, 50, None)?;
    ///
    /// // Move left and down with custom speeds
    /// client.move_relative(-200, -100, Some((20, 20)))?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ViscaError::InvalidParameter` if pan_speed > 24 or tilt_speed > 20.
    /// Returns `ViscaError::UnexpectedResponseType` if camera returns unexpected response.
    /// Returns camera-specific errors if the command is rejected.
    /// Returns transport errors if communication fails.
    fn move_relative(
        &mut self,
        pan_delta: i16,
        tilt_delta: i16,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        let (pan_speed, tilt_speed) = speed.unwrap_or((18, 14));
        let command = PanTiltCommand::RelativePosition {
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
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
    /// * `pan_speed` - Pan speed (1-24)
    /// * `tilt_speed` - Tilt speed (1-18)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPanTiltExt, PanTiltDirection};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Start moving up-right
    /// client.start_moving(PanTiltDirection::UpRight, 10, 10)?;
    ///
    /// // Start moving left at maximum speed
    /// client.start_moving(PanTiltDirection::Left, 24, 0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn start_moving(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), ViscaError> {
        let pan_speed = PanSpeed::new(pan_speed)
            .map_err(|_| ViscaError::InvalidParameter("Pan speed must be 0-24".into()))?;
        let tilt_speed = TiltSpeed::new(tilt_speed)
            .map_err(|_| ViscaError::InvalidParameter("Tilt speed must be 0-20".into()))?;
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
