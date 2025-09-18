//! Zoom control implementation for PTZ cameras.
//!
//! This module provides comprehensive zoom control functionality including:
//! - Standard speed zoom operations (tele/wide)
//! - Variable speed zoom control with fine-grained speed levels
//! - Absolute zoom positioning with normalized values
//! - Digital zoom enable/disable for extended zoom range
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{camera::ViscaClient, command::zoom::ZoomSpeed, mode::Mode, units::Normalized, Error};

/// Zoom operations for PTZ cameras.
///
/// This trait provides comprehensive zoom control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.zoom_tele_std()?;  // Start zooming in
/// thread::sleep(Duration::from_secs(1));
/// camera.zoom_stop()?;  // Stop zooming
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.zoom_tele_std().await?;  // Start zooming in
/// sleep(Duration::from_secs(1)).await;
/// camera.zoom_stop().await?;  // Stop zooming
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait ZoomControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Stop any zoom operation currently in progress.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_stop(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Start zooming in (telephoto direction) at standard speed.
    ///
    /// The zoom will continue until `zoom_stop()` is called or the maximum zoom is reached.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_tele_std(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Start zooming out (wide angle direction) at standard speed.
    ///
    /// The zoom will continue until `zoom_stop()` is called or the minimum zoom is reached.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_wide_std(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Start zooming in at a specified variable speed.
    ///
    /// # Arguments
    /// * `speed` - Zoom speed (0-7, where 0 is slowest and 7 is fastest)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_tele_variable(
        &self,
        speed: ZoomSpeed,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Start zooming out at a specified variable speed.
    ///
    /// # Arguments
    /// * `speed` - Zoom speed (0-7, where 0 is slowest and 7 is fastest)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_wide_variable(
        &self,
        speed: ZoomSpeed,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set zoom to an absolute normalized position.
    ///
    /// # Arguments
    /// * `position` - Normalized position (0.0 = wide, 1.0 = full telephoto)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_absolute(
        &self,
        position: Normalized,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set zoom to a specific raw position value.
    ///
    /// # Arguments
    /// * `position` - Raw zoom position value (camera-specific range)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_zoom_position(
        &self,
        position: crate::types::ZoomPosition,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set digital zoom on or off.
    ///
    /// When enabled, zoom can continue past the optical zoom limit using digital processing.
    ///
    /// # Arguments
    /// * `enabled` - true to enable digital zoom, false to disable
    fn set_digital_zoom(&self, enabled: bool) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ZoomControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn zoom_stop(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.execute(Zoom::Stop)
    }

    fn zoom_tele_std(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.execute(Zoom::TeleStd)
    }

    fn zoom_wide_std(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.execute(Zoom::WideStd)
    }

    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.execute(Zoom::TeleVariable(speed))
    }

    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.execute(Zoom::WideVariable(speed))
    }

    fn zoom_absolute(&self, position: Normalized) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        match crate::types::ZoomPosition::try_from(*position.value()) {
            Ok(zoom_pos) => self.execute(Zoom::Position(zoom_pos)),
            Err(e) => self.error(e),
        }
    }

    fn set_zoom_position(
        &self,
        position: crate::types::ZoomPosition,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.execute(Zoom::Position(position))
    }

    fn set_digital_zoom(&self, enabled: bool) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::DigitalZoom;
        self.execute(DigitalZoom::new(enabled))
    }
}
