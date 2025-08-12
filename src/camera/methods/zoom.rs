//! Zoom methods for cameras using the new GAT architecture.

use crate::{
    capabilities::ValidationError,
    command::zoom::{DigitalZoomCommand, Zoom as ZoomCommand, ZoomSpeed},
    types::ZoomPosition,
    units::Normalized,
    Error,
};

/// Zoom operations (async).
#[cfg(feature = "async")]
pub trait ZoomOps: Sized {
    /// Stop zooming.
    async fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    async fn zoom_in(&self) -> Result<(), Error>;

    /// Start zooming out (wide).
    async fn zoom_out(&self) -> Result<(), Error>;

    /// Start zooming in at standard speed.
    async fn zoom_in_standard(&self) -> Result<(), Error>;

    /// Start zooming out at standard speed.
    async fn zoom_out_standard(&self) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;

    /// Enable digital zoom.
    async fn enable_digital_zoom(&self) -> Result<(), Error>;

    /// Disable digital zoom.
    async fn disable_digital_zoom(&self) -> Result<(), Error>;
}

/// Zoom operations (blocking).
#[cfg(not(feature = "async"))]
pub trait ZoomOpsBlocking: Sized {
    /// Stop zooming.
    fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    fn zoom_in(&self) -> Result<(), Error>;

    /// Start zooming out (wide).
    fn zoom_out(&self) -> Result<(), Error>;

    /// Start zooming in at standard speed.
    fn zoom_in_standard(&self) -> Result<(), Error>;

    /// Start zooming out at standard speed.
    fn zoom_out_standard(&self) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;

    /// Enable digital zoom.
    fn enable_digital_zoom(&self) -> Result<(), Error>;

    /// Disable digital zoom.
    fn disable_digital_zoom(&self) -> Result<(), Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    ZoomOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn zoom_stop(&self) -> Result<(), Error> {
        let command = ZoomCommand::Stop;
        self.send_action_command(&command).await
    }

    async fn zoom_in(&self) -> Result<(), Error> {
        // Use medium speed by default
        let speed = self.zoom_speed_range().end / 2;
        let zoom_speed = ZoomSpeed::new(speed)?;
        let command = ZoomCommand::TeleVariable(zoom_speed);
        self.send_action_command(&command).await
    }

    async fn zoom_out(&self) -> Result<(), Error> {
        // Use medium speed by default
        let speed = self.zoom_speed_range().end / 2;
        let zoom_speed = ZoomSpeed::new(speed)?;
        let command = ZoomCommand::WideVariable(zoom_speed);
        self.send_action_command(&command).await
    }

    async fn zoom_in_standard(&self) -> Result<(), Error> {
        let command = ZoomCommand::TeleStd;
        self.send_action_command(&command).await
    }

    async fn zoom_out_standard(&self) -> Result<(), Error> {
        let command = ZoomCommand::WideStd;
        self.send_action_command(&command).await
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
        self.send_action_command(&command).await
    }

    async fn enable_digital_zoom(&self) -> Result<(), Error> {
        let command = DigitalZoomCommand::new(true);
        self.send_action_command(&command).await
    }

    async fn disable_digital_zoom(&self) -> Result<(), Error> {
        let command = DigitalZoomCommand::new(false);
        self.send_action_command(&command).await
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<
        P: crate::capabilities::Profile,
        T: crate::transport::Transport
            + Send
            + Sync
            + 'static
            + crate::transport::core::BlockingTransport,
    > ZoomOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn zoom_stop(&self) -> Result<(), Error> {
        let command = ZoomCommand::Stop;
        self.send_action_command_blocking(&command)
    }

    fn zoom_in(&self) -> Result<(), Error> {
        // Use medium speed by default
        let speed = self.zoom_speed_range().end / 2;
        let zoom_speed = ZoomSpeed::new(speed)?;
        let command = ZoomCommand::TeleVariable(zoom_speed);
        self.send_action_command_blocking(&command)
    }

    fn zoom_out(&self) -> Result<(), Error> {
        // Use medium speed by default
        let speed = self.zoom_speed_range().end / 2;
        let zoom_speed = ZoomSpeed::new(speed)?;
        let command = ZoomCommand::WideVariable(zoom_speed);
        self.send_action_command_blocking(&command)
    }

    fn zoom_in_standard(&self) -> Result<(), Error> {
        let command = ZoomCommand::TeleStd;
        self.send_action_command_blocking(&command)
    }

    fn zoom_out_standard(&self) -> Result<(), Error> {
        let command = ZoomCommand::WideStd;
        self.send_action_command_blocking(&command)
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
        self.send_action_command_blocking(&command)
    }

    fn enable_digital_zoom(&self) -> Result<(), Error> {
        let command = DigitalZoomCommand::new(true);
        self.send_action_command_blocking(&command)
    }

    fn disable_digital_zoom(&self) -> Result<(), Error> {
        let command = DigitalZoomCommand::new(false);
        self.send_action_command_blocking(&command)
    }
}
