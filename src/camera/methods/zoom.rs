//! Zoom methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    capabilities::ValidationError,
    command::{
        zoom::{Zoom as ZoomCommand, ZoomSpeed},
        Response,
    },
    types::ZoomPosition,
    units::Normalized,
    Error,
};

/// Zoom operations (async).
pub trait ZoomOps: Sized {
    /// Stop zooming.
    async fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    async fn zoom_in(&self) -> Result<(), Error>;

    /// Start zooming out (wide).
    async fn zoom_out(&self) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;
}

/// Zoom operations (blocking).
pub trait ZoomOpsBlocking: Sized {
    /// Stop zooming.
    fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    fn zoom_in(&self) -> Result<(), Error>;

    /// Start zooming out (wide).
    fn zoom_out(&self) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;
}

// Async implementation
impl ZoomOps for Camera {
    async fn zoom_stop(&self) -> Result<(), Error> {
        let command = ZoomCommand::Stop;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

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
}

// Blocking implementation
impl ZoomOpsBlocking for Camera {
    fn zoom_stop(&self) -> Result<(), Error> {
        let command = ZoomCommand::Stop;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn zoom_in(&self) -> Result<(), Error> {
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

    fn zoom_out(&self) -> Result<(), Error> {
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

    fn zoom_absolute(&self, position: Normalized) -> Result<(), Error> {
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
