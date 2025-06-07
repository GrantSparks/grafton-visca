//! High-level extension trait for position control with different coordinate systems.

// Crate imports
use crate::{
    command::{
        pan_tilt::{PanSpeed, TiltSpeed},
        InquiryCommand,
    },
    constants::{
        CameraModel, DegreePosition, NormalizedPosition, PositionConversion, ViscaPosition,
    },
    error::ViscaError,
    pan_tilt_ext::ViscaPanTiltExt,
    ViscaResponse,
};

/// Extension trait providing position control with different coordinate systems.
pub trait ViscaPositionExt: ViscaPanTiltExt {
    /// Move to position specified in degrees.
    ///
    /// # Arguments
    /// * `pan_deg` - Pan position in degrees (-170° to +170° for most cameras)
    /// * `tilt_deg` - Tilt position in degrees (varies by camera model)
    /// * `speed` - Optional pan and tilt speeds
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send, the camera returns an error,
    /// or if the degrees are out of range for the camera model.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaPositionExt, PanSpeed, TiltSpeed};
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example() -> Result<(), grafton_visca::ViscaError> {
    /// # let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    /// // Move to 45 degrees right, 30 degrees up
    /// client.move_to_degrees(45.0, 30.0, None)?;
    ///
    /// // Move to left 90 degrees, level, at specific speeds
    /// let pan_speed = PanSpeed::new(10)?;
    /// let tilt_speed = TiltSpeed::new(10)?;
    /// client.move_to_degrees(-90.0, 0.0, Some((pan_speed, tilt_speed)))?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_to_degrees(
        &mut self,
        pan_deg: f32,
        tilt_deg: f32,
        speed: Option<(PanSpeed, TiltSpeed)>,
    ) -> Result<(), ViscaError> {
        let position = DegreePosition {
            pan: pan_deg,
            tilt: tilt_deg,
        };
        let visca_pos = position.to_visca(CameraModel::PTZOpticsG2);
        self.move_to_position(visca_pos.pan, visca_pos.tilt, speed)
    }

    /// Move to normalized position.
    ///
    /// # Arguments
    /// * `pan` - Normalized pan position (-1.0 = full left, 0.0 = center, 1.0 = full right)
    /// * `tilt` - Normalized tilt position (-1.0 = full down, 0.0 = center, 1.0 = full up)
    /// * `speed` - Optional pan and tilt speeds
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send, the camera returns an error,
    /// or if the normalized values are out of range (-1.0 to 1.0).
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaPositionExt, PanSpeed, TiltSpeed};
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example() -> Result<(), grafton_visca::ViscaError> {
    /// # let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    /// // Move to center
    /// client.move_to_normalized(0.0, 0.0, None)?;
    ///
    /// // Move to top-right corner
    /// let pan_speed = PanSpeed::new(15)?;
    /// let tilt_speed = TiltSpeed::new(15)?;
    /// client.move_to_normalized(1.0, 1.0, Some((pan_speed, tilt_speed)))?;
    ///
    /// // Move to 25% left, 50% up
    /// client.move_to_normalized(-0.25, 0.5, None)?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_to_normalized(
        &mut self,
        pan: f32,
        tilt: f32,
        speed: Option<(PanSpeed, TiltSpeed)>,
    ) -> Result<(), ViscaError> {
        let position = NormalizedPosition { pan, tilt };
        let visca_pos = position.to_visca(CameraModel::PTZOpticsG2);
        self.move_to_position(visca_pos.pan, visca_pos.tilt, speed)
    }

    /// Get current position in degrees.
    ///
    /// # Returns
    /// Current pan/tilt position in degrees
    ///
    /// # Errors
    /// Returns `ViscaError::UnexpectedResponseType` if the camera returns an unexpected response.
    /// Returns transport errors if communication fails.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaPositionExt};
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example() -> Result<(), grafton_visca::ViscaError> {
    /// # let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    /// let pos = client.get_position_degrees()?;
    /// println!("Pan: {:.1}°, Tilt: {:.1}°", pos.pan, pos.tilt);
    /// # Ok(())
    /// # }
    /// ```
    fn get_position_degrees(&mut self) -> Result<DegreePosition, ViscaError>
    where
        Self: Sized,
    {
        let response = self.execute_command(&InquiryCommand::PanTiltPosition)?;
        match response {
            ViscaResponse::InquiryResponse(crate::ViscaInquiryResponse::PanTiltPosition {
                pan,
                tilt,
            }) => {
                let visca_pos = ViscaPosition { pan, tilt };
                Ok(visca_pos.to_degrees(CameraModel::PTZOpticsG2))
            }
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current position normalized.
    ///
    /// # Returns
    /// Current pan/tilt position as normalized values (-1.0 to 1.0)
    ///
    /// # Errors
    /// Returns `ViscaError::UnexpectedResponseType` if the camera returns an unexpected response.
    /// Returns transport errors if communication fails.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaPositionExt};
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example() -> Result<(), grafton_visca::ViscaError> {
    /// # let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    /// let pos = client.get_position_normalized()?;
    /// println!("Pan: {:.0}%, Tilt: {:.0}%", pos.pan * 100.0, pos.tilt * 100.0);
    /// # Ok(())
    /// # }
    /// ```
    fn get_position_normalized(&mut self) -> Result<NormalizedPosition, ViscaError>
    where
        Self: Sized,
    {
        let response = self.execute_command(&InquiryCommand::PanTiltPosition)?;
        match response {
            ViscaResponse::InquiryResponse(crate::ViscaInquiryResponse::PanTiltPosition {
                pan,
                tilt,
            }) => {
                let visca_pos = ViscaPosition { pan, tilt };
                Ok(visca_pos.to_normalized(CameraModel::PTZOpticsG2))
            }
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Move to position specified in degrees with a specific camera model.
    ///
    /// # Arguments
    /// * `pan_deg` - Pan position in degrees
    /// * `tilt_deg` - Tilt position in degrees
    /// * `model` - Camera model for proper conversion
    /// * `speed` - Optional pan and tilt speeds
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send, the camera returns an error,
    /// or if the degrees are out of range for the specified camera model.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaPositionExt, PanSpeed, TiltSpeed};
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::constants::CameraModel;
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example() -> Result<(), grafton_visca::ViscaError> {
    /// # let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    /// // Move using 30X camera model parameters
    /// let pan_speed = PanSpeed::new(12)?;
    /// let tilt_speed = TiltSpeed::new(12)?;
    /// client.move_to_degrees_for_model(
    ///     45.0,
    ///     30.0,
    ///     CameraModel::PTZOptics30X,
    ///     Some((pan_speed, tilt_speed))
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_to_degrees_for_model(
        &mut self,
        pan_deg: f32,
        tilt_deg: f32,
        model: CameraModel,
        speed: Option<(PanSpeed, TiltSpeed)>,
    ) -> Result<(), ViscaError> {
        let position = DegreePosition {
            pan: pan_deg,
            tilt: tilt_deg,
        };
        let visca_pos = position.to_visca(model);
        self.move_to_position(visca_pos.pan, visca_pos.tilt, speed)
    }

    /// Move by relative degrees.
    ///
    /// # Arguments
    /// * `pan_deg` - Relative pan movement in degrees
    /// * `tilt_deg` - Relative tilt movement in degrees
    /// * `speed` - Optional pan and tilt speeds (1-24 for pan, 1-20 for tilt)
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send, the camera returns an error,
    /// or if the relative movement would exceed camera limits.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaPositionExt};
    /// # use grafton_visca::command::pan_tilt::{PanSpeed, TiltSpeed};
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example() -> Result<(), grafton_visca::ViscaError> {
    /// # let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    /// // Move 10 degrees right and 5 degrees up from current position
    /// client.move_by_degrees(10.0, 5.0, None)?;
    ///
    /// // Move 45 degrees left at high speed
    /// let pan_speed = PanSpeed::new(20)?;
    /// let tilt_speed = TiltSpeed::new(20)?;
    /// client.move_by_degrees(-45.0, 0.0, Some((pan_speed, tilt_speed)))?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_by_degrees(
        &mut self,
        pan_deg: f32,
        tilt_deg: f32,
        speed: Option<(PanSpeed, TiltSpeed)>,
    ) -> Result<(), ViscaError> {
        // Convert degrees to VISCA units using the conversion factor
        let pan_units = crate::constants::pan_degrees_to_visca(pan_deg);
        let tilt_units = crate::constants::tilt_degrees_to_visca(tilt_deg);
        self.move_relative(pan_units, tilt_units, speed)
    }

    /// Set normalized pan/tilt speeds.
    ///
    /// # Arguments
    /// * `pan_speed` - Normalized pan speed (0.0 to 1.0)
    /// * `tilt_speed` - Normalized tilt speed (0.0 to 1.0)
    ///
    /// # Returns
    /// Actual pan and tilt speed values that were set
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speed values are out of range (0.0 to 1.0).
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaPositionExt};
    /// # #[cfg(feature = "blocking-client")]
    /// # fn example() -> Result<(), grafton_visca::ViscaError> {
    /// # let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    /// // Set to 50% speed
    /// let (pan, tilt) = client.set_normalized_speeds(0.5, 0.5)?;
    /// println!("Set speeds: pan={}, tilt={}", pan, tilt);
    ///
    /// // Set to maximum speed
    /// client.set_normalized_speeds(1.0, 1.0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_normalized_speeds(
        &mut self,
        pan_speed: f32,
        tilt_speed: f32,
    ) -> Result<(u8, u8), ViscaError> {
        let pan = crate::constants::pan_speed_normalized_to_visca(pan_speed);
        let tilt = crate::constants::tilt_speed_normalized_to_visca(tilt_speed);
        Ok((pan, tilt))
    }
}

/// Implement the trait for all types that implement `ViscaPanTiltExt`
impl<T: ViscaPanTiltExt> ViscaPositionExt for T {}
