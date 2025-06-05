//! High-level extension trait for zoom control operations.

// Crate imports
use crate::{
    command::zoom::{ZoomCommand, ZoomSpeed},
    error::ViscaError,
    transport_ext::ViscaTransportExt,
};

/// Extension trait providing high-level zoom control methods.
pub trait ViscaZoomExt: ViscaTransportExt {
    /// Move zoom to an absolute position.
    ///
    /// # Arguments
    /// * `position` - Target zoom position (0x0000 to 0x4000)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaZoomExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Move to minimum zoom (wide)
    /// transport.zoom_to(0x0000)?;
    ///
    /// // Move to maximum zoom (telephoto)
    /// transport.zoom_to(0x4000)?;
    ///
    /// // Move to mid-range zoom
    /// transport.zoom_to(0x2000)?;
    /// # Ok(())
    /// # }
    /// ```
    fn zoom_to(&mut self, position: u16) -> Result<(), ViscaError> {
        let command = ZoomCommand::Direct(position);
        self.send_command(&command)?;
        Ok(())
    }

    /// Start zooming in (telephoto direction).
    ///
    /// # Arguments
    /// * `speed` - Optional zoom speed (0-7). If None, uses standard speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaZoomExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Zoom in at standard speed
    /// transport.zoom_in(None)?;
    ///
    /// // Zoom in at maximum speed
    /// transport.zoom_in(Some(7))?;
    /// # Ok(())
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
        self.send_command(&command)?;
        Ok(())
    }

    /// Start zooming out (wide direction).
    ///
    /// # Arguments
    /// * `speed` - Optional zoom speed (0-7). If None, uses standard speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaZoomExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Zoom out at standard speed
    /// transport.zoom_out(None)?;
    ///
    /// // Zoom out at slow speed
    /// transport.zoom_out(Some(2))?;
    /// # Ok(())
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
        self.send_command(&command)?;
        Ok(())
    }

    /// Stop zoom movement.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaZoomExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Start zooming in
    /// transport.zoom_in(None)?;
    ///
    /// // ... wait some time ...
    ///
    /// // Stop zooming
    /// transport.stop_zoom()?;
    /// # Ok(())
    /// # }
    /// ```
    fn stop_zoom(&mut self) -> Result<(), ViscaError> {
        let command = ZoomCommand::Stop;
        self.send_command(&command)?;
        Ok(())
    }
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: ViscaTransportExt> ViscaZoomExt for T {}
