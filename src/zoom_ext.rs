//! High-level extension trait for zoom control operations.

// Crate imports
use crate::{
    command::{
        zoom::{ZoomCommand, ZoomSpeed},
        InquiryCommand,
    },
    error::ViscaError,
    ViscaDevice, ViscaResponse,
};

/// Extension trait providing high-level zoom control methods.
pub trait ViscaZoomExt: ViscaDevice {
    /// Move zoom to an absolute position.
    ///
    /// # Arguments
    /// * `position` - Target zoom position (0x0000 to 0x4000)
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// // Move to minimum zoom (wide)
    /// client.zoom_to(0x0000)?;
    ///
    /// // Move to maximum zoom (telephoto)
    /// client.zoom_to(0x4000)?;
    ///
    /// // Move to mid-range zoom
    /// client.zoom_to(0x2000)?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn zoom_to(&mut self, position: u16) -> Result<(), ViscaError> {
        let command = ZoomCommand::Direct(position);
        match self.execute_command(&command)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Start zooming in (telephoto direction).
    ///
    /// # Arguments
    /// * `speed` - Optional zoom speed (0-7). If None, uses standard speed.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// // Zoom in at standard speed
    /// client.zoom_in(None)?;
    ///
    /// // Zoom in at maximum speed
    /// client.zoom_in(Some(7))?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn zoom_in(&mut self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Zoom speed must be 0-7".into(),
                ));
            }
            ZoomCommand::TeleVariable(ZoomSpeed::new(s)?)
        } else {
            ZoomCommand::TeleStandard
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Start zooming out (wide direction).
    ///
    /// # Arguments
    /// * `speed` - Optional zoom speed (0-7). If None, uses standard speed.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// // Zoom out at standard speed
    /// client.zoom_out(None)?;
    ///
    /// // Zoom out at slow speed
    /// client.zoom_out(Some(2))?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn zoom_out(&mut self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Zoom speed must be 0-7".into(),
                ));
            }
            ZoomCommand::WideVariable(ZoomSpeed::new(s)?)
        } else {
            ZoomCommand::WideStandard
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Stop zoom movement.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// // Start zooming in
    /// client.zoom_in(None)?;
    ///
    /// // ... wait some time ...
    ///
    /// // Stop zooming
    /// client.stop_zoom()?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn stop_zoom(&mut self) -> Result<(), ViscaError> {
        let command = ZoomCommand::Stop;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Set zoom by magnification factor.
    ///
    /// # Arguments
    /// * `magnification` - Zoom magnification (1.0 to 20.0 for 20x cameras)
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// // Set to 1x (wide)
    /// client.zoom_to_magnification(1.0)?;
    ///
    /// // Set to 10x zoom
    /// client.zoom_to_magnification(10.0)?;
    ///
    /// // Set to maximum 20x zoom
    /// client.zoom_to_magnification(20.0)?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn zoom_to_magnification(&mut self, magnification: f32) -> Result<(), ViscaError> {
        let position = crate::constants::zoom_magnification_to_visca(magnification);
        self.zoom_to(position)
    }

    /// Get current zoom magnification.
    ///
    /// # Returns
    /// Current zoom magnification (1.0 to 20.0 for 20x cameras)
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// let magnification = client.get_zoom_magnification()?;
    /// println!("Current zoom: {:.1}x", magnification);
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn get_zoom_magnification(&mut self) -> Result<f32, ViscaError>
    where
        Self: Sized,
    {
        let response = self.execute_command(&InquiryCommand::ZoomPosition)?;
        match response {
            ViscaResponse::InquiryResponse(crate::ViscaInquiryResponse::ZoomPosition {
                position,
            }) => Ok(crate::constants::zoom_visca_to_magnification(position)),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Set zoom by normalized value.
    ///
    /// # Arguments
    /// * `normalized` - Normalized zoom value (0.0 = wide, 1.0 = telephoto)
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// // Set to wide (0%)
    /// client.zoom_to_normalized(0.0)?;
    ///
    /// // Set to mid-range (50%)
    /// client.zoom_to_normalized(0.5)?;
    ///
    /// // Set to telephoto (100%)
    /// client.zoom_to_normalized(1.0)?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn zoom_to_normalized(&mut self, normalized: f32) -> Result<(), ViscaError> {
        let position = crate::constants::zoom_normalized_to_visca(normalized);
        self.zoom_to(position)
    }

    /// Get current zoom as normalized value.
    ///
    /// # Returns
    /// Normalized zoom value (0.0 = wide, 1.0 = telephoto)
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// # use grafton_visca::{ViscaError, ViscaClient, ViscaZoomExt};
    /// # fn example(client: &mut ViscaClient) -> Result<(), ViscaError> {
    /// let normalized = client.get_zoom_normalized()?;
    /// println!("Current zoom: {:.0}%", normalized * 100.0);
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    fn get_zoom_normalized(&mut self) -> Result<f32, ViscaError>
    where
        Self: Sized,
    {
        let response = self.execute_command(&InquiryCommand::ZoomPosition)?;
        match response {
            ViscaResponse::InquiryResponse(crate::ViscaInquiryResponse::ZoomPosition {
                position,
            }) => Ok(crate::constants::zoom_visca_to_normalized(position)),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
}

// Blanket implementation for all types that implement ViscaDevice
impl<T: ViscaDevice> ViscaZoomExt for T {}
