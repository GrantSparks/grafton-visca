//! Zoom methods for cameras using mode markers.

use crate::{command::zoom::ZoomSpeed, units::Normalized, Error};

/// Zoom operations (async).
#[cfg(feature = "async")]
pub trait ZoomOps: Sized {
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
#[cfg(not(feature = "async"))]
pub trait ZoomOpsBlocking: Sized {
    /// Stop zooming.
    fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in at standard speed.
    fn zoom_tele_std(&self) -> Result<(), Error>;

    /// Start zooming out at standard speed.
    fn zoom_wide_std(&self) -> Result<(), Error>;

    /// Start zooming in at variable speed.
    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Start zooming out at variable speed.
    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;

    /// Set zoom to a specific position value.
    fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error>;

    /// Query the current zoom position.
    fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T> ZoomOps for crate::camera::Camera<crate::camera::AsyncMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
{
    async fn zoom_stop(&self) -> Result<(), Error> {
        self.zoom_stop().await
    }

    async fn zoom_tele_std(&self) -> Result<(), Error> {
        self.zoom_tele_std().await
    }

    async fn zoom_wide_std(&self) -> Result<(), Error> {
        self.zoom_wide_std().await
    }

    async fn zoom_tele_variable(&self, speed: ZoomSpeed) -> Result<(), Error> {
        self.zoom_tele_variable(speed).await
    }

    async fn zoom_wide_variable(&self, speed: ZoomSpeed) -> Result<(), Error> {
        self.zoom_wide_variable(speed).await
    }

    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error> {
        self.zoom_absolute(position).await
    }

    async fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        self.zoom_position(position).await
    }

    async fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error> {
        self.zoom_position_inquiry().await
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> ZoomOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn zoom_stop(&self) -> Result<(), Error> {
        self.zoom_stop()
    }

    fn zoom_tele_std(&self) -> Result<(), Error> {
        self.zoom_tele_std()
    }

    fn zoom_wide_std(&self) -> Result<(), Error> {
        self.zoom_wide_std()
    }

    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> Result<(), Error> {
        self.zoom_tele_variable(speed)
    }

    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> Result<(), Error> {
        self.zoom_wide_variable(speed)
    }

    fn zoom_absolute(&self, position: Normalized) -> Result<(), Error> {
        self.zoom_absolute(position)
    }

    fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        self.zoom_position(position)
    }

    fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error> {
        self.zoom_position_inquiry()
    }
}
