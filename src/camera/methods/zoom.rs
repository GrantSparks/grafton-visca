//! Zoom methods for cameras that support zoom control.

use crate::camera::Camera;
use crate::capabilities::{ProfileMetadata, SupportsZoom};
// Removed unused imports
use crate::Error;

/// Extension trait that adds zoom methods to cameras.
#[allow(async_fn_in_trait)]
pub trait ZoomMethods {
    /// Stop zooming.
    #[cfg(not(feature = "async"))]
    fn zoom_stop(&mut self) -> Result<(), Error>;

    /// Stop zooming.
    #[cfg(feature = "async")]
    async fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    #[cfg(not(feature = "async"))]
    fn zoom_in(&mut self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    #[cfg(feature = "async")]
    async fn zoom_in(&self) -> Result<(), Error>;

    /// Start zooming out (wide).
    #[cfg(not(feature = "async"))]
    fn zoom_out(&mut self) -> Result<(), Error>;

    /// Start zooming out (wide).
    #[cfg(feature = "async")]
    async fn zoom_out(&self) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    #[cfg(not(feature = "async"))]
    fn zoom_absolute(&mut self, position: f32) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    #[cfg(feature = "async")]
    async fn zoom_absolute(&self, position: f32) -> Result<(), Error>;
}

// Blanket implementation for cameras with zoom support
#[cfg(not(feature = "async"))]
impl<P, T> ZoomMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsZoom,
    T: crate::transport::blocking::BlockingTransport,
{
    fn zoom_stop(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::ZOOM_STOP);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    fn zoom_in(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        // Use medium speed by default
        let speed = P::ZOOM_SPEED_RANGE.end / 2;

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::ZOOM_TELE_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    fn zoom_out(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        // Use medium speed by default
        let speed = P::ZOOM_SPEED_RANGE.end / 2;

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::ZOOM_WIDE_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    fn zoom_absolute(&mut self, position: f32) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_u16_visca, CommandBuilder};

        // Validate position
        if !(0.0..=1.0).contains(&position) {
            return Err(Error::ValidationError(
                crate::capabilities::ValidationError::OutOfRange {
                    parameter: "zoom position",
                    value: position as f64,
                    min: 0.0,
                    max: 1.0,
                },
            ));
        }

        // Convert normalized position to camera units
        let units = (position * P::OPTICAL_ZOOM_MAX as f32) as u16;
        let units = units.min(P::OPTICAL_ZOOM_MAX);

        let mut cmd = CommandBuilder::<10>::new();
        cmd.append(&commands::ZOOM_ABSOLUTE_PREFIX);
        encode_u16_visca(units, &mut cmd);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> ZoomMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsZoom,
    T: crate::transport::AsyncTransport,
{
    async fn zoom_stop(&self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::ZOOM_STOP);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }

    async fn zoom_in(&self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        // Use medium speed by default
        let speed = P::ZOOM_SPEED_RANGE.end / 2;

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::ZOOM_TELE_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }

    async fn zoom_out(&self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        // Use medium speed by default
        let speed = P::ZOOM_SPEED_RANGE.end / 2;

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::ZOOM_WIDE_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }

    async fn zoom_absolute(&self, position: f32) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_u16_visca, CommandBuilder};

        // Validate position
        if !(0.0..=1.0).contains(&position) {
            return Err(Error::ValidationError(
                crate::capabilities::ValidationError::OutOfRange {
                    parameter: "zoom position",
                    value: position as f64,
                    min: 0.0,
                    max: 1.0,
                },
            ));
        }

        // Convert normalized position to camera units
        let units = (position * P::OPTICAL_ZOOM_MAX as f32) as u16;
        let units = units.min(P::OPTICAL_ZOOM_MAX);

        let mut cmd = CommandBuilder::<10>::new();
        cmd.append(&commands::ZOOM_ABSOLUTE_PREFIX);
        encode_u16_visca(units, &mut cmd);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }
}
