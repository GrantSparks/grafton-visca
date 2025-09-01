//! Zoom methods for unified camera API.

use crate::{command::zoom::ZoomSpeed, units::Normalized, Error};

/// Zoom operations for cameras.
///
/// This trait provides zoom control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait ZoomControl {
    /// Stop zooming.
    #[cfg(feature = "async")]
    fn zoom_stop(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Stop zooming.
    #[cfg(not(feature = "async"))]
    fn zoom_stop(&mut self) -> Result<(), Error>;

    /// Start zooming in at standard speed.
    #[cfg(feature = "async")]
    fn zoom_tele_std(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Start zooming in at standard speed.
    #[cfg(not(feature = "async"))]
    fn zoom_tele_std(&mut self) -> Result<(), Error>;

    /// Start zooming out at standard speed.
    #[cfg(feature = "async")]
    fn zoom_wide_std(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Start zooming out at standard speed.
    #[cfg(not(feature = "async"))]
    fn zoom_wide_std(&mut self) -> Result<(), Error>;

    /// Start zooming in at variable speed.
    #[cfg(feature = "async")]
    fn zoom_tele_variable(
        &self,
        speed: ZoomSpeed,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Start zooming in at variable speed.
    #[cfg(not(feature = "async"))]
    fn zoom_tele_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Start zooming out at variable speed.
    #[cfg(feature = "async")]
    fn zoom_wide_variable(
        &self,
        speed: ZoomSpeed,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Start zooming out at variable speed.
    #[cfg(not(feature = "async"))]
    fn zoom_wide_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    #[cfg(feature = "async")]
    fn zoom_absolute(
        &self,
        position: Normalized,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    #[cfg(not(feature = "async"))]
    fn zoom_absolute(&mut self, position: Normalized) -> Result<(), Error>;

    /// Set zoom to a specific position value.
    #[cfg(feature = "async")]
    fn zoom_position(
        &self,
        position: crate::types::ZoomPosition,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set zoom to a specific position value.
    #[cfg(not(feature = "async"))]
    fn zoom_position(&mut self, position: crate::types::ZoomPosition) -> Result<(), Error>;

    /// Query the current zoom position.
    #[cfg(feature = "async")]
    fn zoom_position_inquiry(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::types::ZoomPosition, Error>> + Send + '_;

    /// Query the current zoom position.
    #[cfg(not(feature = "async"))]
    fn zoom_position_inquiry(&mut self) -> Result<crate::types::ZoomPosition, Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async zoom control trait (deprecated, use ZoomControl instead).
/// Blocking zoom control trait (deprecated, use ZoomControl instead).
// Async implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> ZoomControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> ZoomControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn zoom_stop(&mut self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let cmd = Zoom::Stop;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn zoom_tele_std(&mut self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::TeleStd;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn zoom_wide_std(&mut self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::WideStd;
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn zoom_tele_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::TeleVariable(speed);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn zoom_wide_variable(&mut self, speed: ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::WideVariable(speed);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn zoom_absolute(&mut self, position: Normalized) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        // Convert normalized position to zoom position value
        let zoom_pos = crate::types::ZoomPosition::try_from(*position.value())?;
        let cmd = Zoom::Position(zoom_pos);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn zoom_position(&mut self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        use crate::command::zoom::Zoom;

        let cmd = Zoom::Position(position);
        pollster::block_on(self.send_command(&cmd))?;
        Ok(())
    }

    fn zoom_position_inquiry(&mut self) -> Result<crate::types::ZoomPosition, Error> {
        use crate::command::inquiry_structs::ZoomPositionInquiry;

        pollster::block_on(self.send_command_typed(&ZoomPositionInquiry))
    }
}
