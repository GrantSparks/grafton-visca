//! High-level extension trait for position control with different coordinate systems.

#![allow(deprecated)]

// Crate imports
use crate::{
    command::InquiryCommand,
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
    /// * `speed` - Optional pan and tilt speeds (1-24 for pan, 1-20 for tilt)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPositionExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Move to 45 degrees right, 30 degrees up
    /// transport.move_to_degrees(45.0, 30.0, None)?;
    ///
    /// // Move to left 90 degrees, level, at specific speeds
    /// transport.move_to_degrees(-90.0, 0.0, Some((10, 10)))?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_to_degrees(
        &mut self,
        pan_deg: f32,
        tilt_deg: f32,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        let position = DegreePosition {
            pan: pan_deg,
            tilt: tilt_deg,
        };
        let visca_pos = position.to_visca(CameraModel::PTZOpticsG2);

        if let Some((pan_speed, tilt_speed)) = speed {
            self.move_to_position(visca_pos.pan, visca_pos.tilt, Some((pan_speed, tilt_speed)))
        } else {
            self.move_to_position(visca_pos.pan, visca_pos.tilt, None)
        }
    }

    /// Move to normalized position.
    ///
    /// # Arguments
    /// * `pan` - Normalized pan position (-1.0 = full left, 0.0 = center, 1.0 = full right)
    /// * `tilt` - Normalized tilt position (-1.0 = full down, 0.0 = center, 1.0 = full up)
    /// * `speed` - Optional pan and tilt speeds (1-24 for pan, 1-20 for tilt)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPositionExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Move to center
    /// transport.move_to_normalized(0.0, 0.0, None)?;
    ///
    /// // Move to top-right corner
    /// transport.move_to_normalized(1.0, 1.0, Some((15, 15)))?;
    ///
    /// // Move to 25% left, 50% up
    /// transport.move_to_normalized(-0.25, 0.5, None)?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_to_normalized(
        &mut self,
        pan: f32,
        tilt: f32,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        let position = NormalizedPosition { pan, tilt };
        let visca_pos = position.to_visca(CameraModel::PTZOpticsG2);

        if let Some((pan_speed, tilt_speed)) = speed {
            self.move_to_position(visca_pos.pan, visca_pos.tilt, Some((pan_speed, tilt_speed)))
        } else {
            self.move_to_position(visca_pos.pan, visca_pos.tilt, None)
        }
    }

    /// Get current position in degrees.
    ///
    /// # Returns
    /// Current pan/tilt position in degrees
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPositionExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// let pos = transport.get_position_degrees()?;
    /// println!("Pan: {:.1}°, Tilt: {:.1}°", pos.pan, pos.tilt);
    /// # Ok(())
    /// # }
    /// ```
    fn get_position_degrees(&mut self) -> Result<DegreePosition, ViscaError>
    where
        Self: Sized,
    {
        let response = self.send_and_wait(&InquiryCommand::PanTiltPosition)?;
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
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPositionExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// let pos = transport.get_position_normalized()?;
    /// println!("Pan: {:.0}%, Tilt: {:.0}%", pos.pan * 100.0, pos.tilt * 100.0);
    /// # Ok(())
    /// # }
    /// ```
    fn get_position_normalized(&mut self) -> Result<NormalizedPosition, ViscaError>
    where
        Self: Sized,
    {
        let response = self.send_and_wait(&InquiryCommand::PanTiltPosition)?;
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
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPositionExt};
    /// # use grafton_visca::constants::CameraModel;
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Move using 30X camera model parameters
    /// transport.move_to_degrees_for_model(
    ///     45.0,
    ///     30.0,
    ///     CameraModel::PTZOptics30X,
    ///     Some((12, 12))
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_to_degrees_for_model(
        &mut self,
        pan_deg: f32,
        tilt_deg: f32,
        model: CameraModel,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        let position = DegreePosition {
            pan: pan_deg,
            tilt: tilt_deg,
        };
        let visca_pos = position.to_visca(model);

        if let Some((pan_speed, tilt_speed)) = speed {
            self.move_to_position(visca_pos.pan, visca_pos.tilt, Some((pan_speed, tilt_speed)))
        } else {
            self.move_to_position(visca_pos.pan, visca_pos.tilt, None)
        }
    }

    /// Move by relative degrees.
    ///
    /// # Arguments
    /// * `pan_deg` - Relative pan movement in degrees
    /// * `tilt_deg` - Relative tilt movement in degrees
    /// * `speed` - Optional pan and tilt speeds (1-24 for pan, 1-20 for tilt)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPositionExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Move 10 degrees right and 5 degrees up from current position
    /// transport.move_by_degrees(10.0, 5.0, None)?;
    ///
    /// // Move 45 degrees left at high speed
    /// transport.move_by_degrees(-45.0, 0.0, Some((20, 20)))?;
    /// # Ok(())
    /// # }
    /// ```
    fn move_by_degrees(
        &mut self,
        pan_deg: f32,
        tilt_deg: f32,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        // Convert degrees to VISCA units using the conversion factor
        let pan_units = crate::constants::pan_degrees_to_visca(pan_deg);
        let tilt_units = crate::constants::tilt_degrees_to_visca(tilt_deg);

        if let Some((pan_speed, tilt_speed)) = speed {
            self.move_relative(pan_units, tilt_units, Some((pan_speed, tilt_speed)))
        } else {
            self.move_relative(pan_units, tilt_units, None)
        }
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
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPositionExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set to 50% speed
    /// let (pan, tilt) = transport.set_normalized_speeds(0.5, 0.5)?;
    /// println!("Set speeds: pan={}, tilt={}", pan, tilt);
    ///
    /// // Set to maximum speed
    /// transport.set_normalized_speeds(1.0, 1.0)?;
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
