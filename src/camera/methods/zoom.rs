//! Zoom methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    capabilities::ValidationError,
    command::{zoom::{Zoom as ZoomCommand, ZoomSpeed}, Response},
    types::ZoomPosition,
    units::Normalized,
    Error,
};


/// Zoom operations.
pub trait ZoomOps: Sized {

    /// Stop zooming.
    #[cfg(feature = "tokio")]
    async fn zoom_stop(&self) -> Result<(), Error>;

    /// Stop zooming. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn zoom_stop_blocking(&mut self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    #[cfg(feature = "tokio")]
    async fn zoom_in(&self) -> Result<(), Error>;

    /// Start zooming in (telephoto). (blocking).
    #[cfg(not(feature = "tokio"))]
    fn zoom_in_blocking(&mut self) -> Result<(), Error>;

    /// Start zooming out (wide).
    #[cfg(feature = "tokio")]
    async fn zoom_out(&self) -> Result<(), Error>;

    /// Start zooming out (wide). (blocking).
    #[cfg(not(feature = "tokio"))]
    fn zoom_out_blocking(&mut self) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    #[cfg(feature = "tokio")]
    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele). (blocking).
    #[cfg(not(feature = "tokio"))]
    fn zoom_absolute_blocking(&mut self, position: Normalized) -> Result<(), Error>;
}

impl ZoomOps for Camera {
    #[cfg(feature = "tokio")]
    async fn zoom_stop(&self) -> Result<(), Error> {
            let command = ZoomCommand::Stop;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }

    #[cfg(not(feature = "tokio"))]
    fn zoom_stop_blocking(&mut self) -> Result<(), Error> {
        
            let command = ZoomCommand::Stop;
            let response = self.send_command_blocking(&command)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    async fn zoom_in(&self) -> Result<(), Error> {
            // Use medium speed by default
            let speed = self.zoom_speed_range().end / 2;
            let zoom_speed = ZoomSpeed::new(speed)?;
            let command = ZoomCommand::TeleVariable(zoom_speed);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }

    #[cfg(not(feature = "tokio"))]
    fn zoom_in_blocking(&mut self) -> Result<(), Error> {
        
            // Use medium speed by default
            let speed = self.zoom_speed_range().end / 2;
            let zoom_speed = ZoomSpeed::new(speed)?;
            let command = ZoomCommand::TeleVariable(zoom_speed);
            let response = self.send_command_blocking(&command)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    async fn zoom_out(&self) -> Result<(), Error> {
            // Use medium speed by default
            let speed = self.zoom_speed_range().end / 2;
            let zoom_speed = ZoomSpeed::new(speed)?;
            let command = ZoomCommand::WideVariable(zoom_speed);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }

    #[cfg(not(feature = "tokio"))]
    fn zoom_out_blocking(&mut self) -> Result<(), Error> {
        
            // Use medium speed by default
            let speed = self.zoom_speed_range().end / 2;
            let zoom_speed = ZoomSpeed::new(speed)?;
            let command = ZoomCommand::WideVariable(zoom_speed);
            let response = self.send_command_blocking(&command)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error> {
            let normalized = position;
            let position_value = normalized.0;

            // Validate position
            if !(0.0..=1.0).contains(&position_value) {
                return Err(Error::ValidationError(ValidationError::OutOfRange {
                    parameter: "zoom position",
                    value: position_value as f64,
                    min: 0.0,
                    max: 1.0,
                }));
            }

            // Convert normalized position to VISCA units
            let max_zoom = self.digital_zoom_max().unwrap_or(self.optical_zoom_max());
            let zoom_pos = (position_value * max_zoom as f32) as u16;
            let zoom_position = ZoomPosition::new(zoom_pos)?;
            let command = ZoomCommand::Position(zoom_position);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }

    #[cfg(not(feature = "tokio"))]
    fn zoom_absolute_blocking(&mut self, position: Normalized) -> Result<(), Error> {
        
            let normalized = position;
            let position_value = normalized.0;

            // Validate position
            if !(0.0..=1.0).contains(&position_value) {
                return Err(Error::ValidationError(ValidationError::OutOfRange {
                    parameter: "zoom position",
                    value: position_value as f64,
                    min: 0.0,
                    max: 1.0,
                }));
            }

            // Convert normalized position to VISCA units
            let max_zoom = self.digital_zoom_max().unwrap_or(self.optical_zoom_max());
            let zoom_pos = (position_value * max_zoom as f32) as u16;
            let zoom_position = ZoomPosition::new(zoom_pos)?;
            let command = ZoomCommand::Position(zoom_position);
            let response = self.send_command_blocking(&command)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
}

