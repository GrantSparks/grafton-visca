//! Zoom methods for cameras using mode markers.

use crate::{command::zoom::ZoomSpeed, impl_camera_ops, units::Normalized, Error};

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

// Use macro to generate implementations
impl_camera_ops!(
    async,
    ZoomOps,
    async fn zoom_stop(&self) -> Result<(), Error>;,
    async fn zoom_tele_std(&self) -> Result<(), Error>;,
    async fn zoom_wide_std(&self) -> Result<(), Error>;,
    async fn zoom_tele_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;,
    async fn zoom_wide_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;,
    async fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;,
    async fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error>;,
    async fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error>;
);

impl_camera_ops!(
    blocking,
    ZoomOpsBlocking,
    fn zoom_stop(&self) -> Result<(), Error>;,
    fn zoom_tele_std(&self) -> Result<(), Error>;,
    fn zoom_wide_std(&self) -> Result<(), Error>;,
    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;,
    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> Result<(), Error>;,
    fn zoom_absolute(&self, position: Normalized) -> Result<(), Error>;,
    fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error>;,
    fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error>;
);
