//! Zoom methods for cameras using mode markers.

use crate::{command::zoom::ZoomSpeed, units::Normalized, Error};

/// Zoom operations (async).
#[cfg(feature = "async")]
pub trait ZoomControl: Sized {
    /// Stop zooming.
    async fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in at standard speed.
    async fn zoom_tele_std(&self) -> Result<(), Error>;

    /// Start zooming out at standard speed.
    async fn zoom_wide_std(&self) -> Result<(), Error>;

    /// Start zooming in at variable speed.
    async fn zoom_tele_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Start zooming out at variable speed.
    async fn zoom_wide_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;

    /// Set zoom to a specific position value.
    async fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error>;

    /// Query the current zoom position.
    async fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error>;
}

/// Zoom operations (blocking).
pub trait ZoomControlBlocking: Sized {
    /// Stop zooming.
    fn zoom_stop(&mut self) -> Result<(), Error>;

    /// Start zooming in at standard speed.
    fn zoom_tele_std(&mut self) -> Result<(), Error>;

    /// Start zooming out at standard speed.
    fn zoom_wide_std(&mut self) -> Result<(), Error>;

    /// Start zooming in at variable speed.
    fn zoom_tele_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Start zooming out at variable speed.
    fn zoom_wide_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(&mut self, position: Normalized) -> Result<(), Error>;

    /// Set zoom to a specific position value.
    fn zoom_position(&mut self, position: crate::types::ZoomPosition) -> Result<(), Error>;

    /// Query the current zoom position.
    fn zoom_position_inquiry(&mut self) -> Result<crate::types::ZoomPosition, Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> ZoomControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn zoom_stop(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::Stop;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn zoom_tele_std(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::TeleStd;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn zoom_wide_std(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::WideStd;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn zoom_tele_variable(&self, speed: ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::TeleVariable(speed);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn zoom_wide_variable(&self, speed: ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::WideVariable(speed);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        // Convert normalized position to zoom position value
        let zoom_pos = crate::types::ZoomPosition::try_from(*position.value())?;
        let cmd = Zoom::Position(zoom_pos);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::Position(position);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error> {
        use crate::command::inquiry_structs::ZoomPositionInquiry;

        self.send_command_typed(&ZoomPositionInquiry).await
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> ZoomControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + Default,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn zoom_stop(&mut self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::Stop;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn zoom_tele_std(&mut self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::TeleStd;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn zoom_wide_std(&mut self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::WideStd;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn zoom_tele_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::TeleVariable(speed);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn zoom_wide_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::WideVariable(speed);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn zoom_absolute(&mut self, position: Normalized) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        // Convert normalized position to zoom position value
        let zoom_pos = crate::types::ZoomPosition::try_from(*position.value())?;
        let cmd = Zoom::Position(zoom_pos);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn zoom_position(&mut self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::Position(position);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn zoom_position_inquiry(&mut self) -> Result<crate::types::ZoomPosition, Error> {
        use crate::command::inquiry_structs::ZoomPositionInquiry;

        self.send_command_typed(&ZoomPositionInquiry)
    }
}
