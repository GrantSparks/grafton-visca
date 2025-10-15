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

use crate::{camera::ViscaClient, mode::Mode, types::ZoomSpeed, units::Normalized, Error};

/// Zoom operations for PTZ cameras.
///
/// This trait provides comprehensive zoom control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// // Zoom in with standard speed
/// camera.zoom_tele(None)?;
/// thread::sleep(Duration::from_secs(1));
/// camera.zoom_stop()?;
///
/// // Zoom out with specific speed
/// camera.zoom_wide(Some(ZoomSpeed::new(5)?))?;
/// ```
///
/// ## Async mode
/// ```ignore
/// // Zoom in with standard speed
/// camera.zoom_tele(None).await?;
/// sleep(Duration::from_secs(1)).await;
/// camera.zoom_stop().await?;
///
/// // Zoom out with specific speed
/// camera.zoom_wide(Some(ZoomSpeed::new(5)?)).await?;
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

    /// Start zooming in (telephoto direction) with optional speed control.
    ///
    /// When `speed` is `None`, uses standard zoom speed. When `Some(speed)` is provided,
    /// uses variable speed zoom with the specified speed (0-7).
    ///
    /// The zoom will continue until `zoom_stop()` is called or the maximum zoom is reached.
    ///
    /// # Arguments
    /// * `speed` - Optional zoom speed (0-7). If None, uses standard speed.
    ///
    /// # Examples
    /// ```ignore
    /// // Standard speed
    /// camera.zoom_tele(None)?;
    ///
    /// // Variable speed
    /// camera.zoom_tele(Some(ZoomSpeed::new(5)?))?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_tele(
        &self,
        speed: Option<ZoomSpeed>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Start zooming out (wide angle direction) with optional speed control.
    ///
    /// When `speed` is `None`, uses standard zoom speed. When `Some(speed)` is provided,
    /// uses variable speed zoom with the specified speed (0-7).
    ///
    /// The zoom will continue until `zoom_stop()` is called or the minimum zoom is reached.
    ///
    /// # Arguments
    /// * `speed` - Optional zoom speed (0-7). If None, uses standard speed.
    ///
    /// # Examples
    /// ```ignore
    /// // Standard speed
    /// camera.zoom_wide(None)?;
    ///
    /// // Variable speed
    /// camera.zoom_wide(Some(ZoomSpeed::new(3)?))?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn zoom_wide(
        &self,
        speed: Option<ZoomSpeed>,
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

    /// Set zoom to an absolute normalized position with domain awareness.
    ///
    /// This method provides domain-aware zoom control, allowing you to specify
    /// whether the normalized position should map to the optical zoom range only
    /// or include digital zoom as well.
    ///
    /// # Arguments
    /// * `position` - Normalized position (0.0 = wide, 1.0 = full telephoto for the domain)
    /// * `domain` - The zoom domain to use (Optical or OpticalPlusDigital)
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::{ZoomDomain, Normalized};
    ///
    /// // Set to 50% of optical zoom range
    /// camera.zoom_absolute_normalized(
    ///     Normalized::new(0.5)?,
    ///     ZoomDomain::Optical
    /// )?;
    ///
    /// // Set to 75% of full zoom range (including digital)
    /// camera.zoom_absolute_normalized(
    ///     Normalized::new(0.75)?,
    ///     ZoomDomain::OpticalPlusDigital
    /// )?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support digital zoom and OpticalPlusDigital domain is specified
    /// - The command fails to send or receive a response
    fn zoom_absolute_normalized(
        &self,
        position: crate::Normalized,
        domain: crate::ZoomDomain,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
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

    fn zoom_tele(&self, speed: Option<ZoomSpeed>) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        match speed {
            None => self.execute(Zoom::TeleStd),
            Some(s) => self.execute(Zoom::TeleVariable(s)),
        }
    }

    fn zoom_wide(&self, speed: Option<ZoomSpeed>) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        match speed {
            None => self.execute(Zoom::WideStd),
            Some(s) => self.execute(Zoom::WideVariable(s)),
        }
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

    fn zoom_absolute_normalized(
        &self,
        position: crate::Normalized,
        domain: crate::ZoomDomain,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::{command::zoom::Zoom, ZoomPositionExt};

        // Check if digital zoom is supported when OpticalPlusDigital is requested
        if domain == crate::ZoomDomain::OpticalPlusDigital && P::DIGITAL_ZOOM_MAX.is_none() {
            return self.error(Error::FeatureNotSupported {
                feature: "Digital zoom",
            });
        }

        // Convert normalized position to zoom position based on domain
        match crate::types::ZoomPosition::from_normalized(position, domain) {
            Ok(zoom_pos) => self.execute(Zoom::Position(zoom_pos)),
            Err(e) => self.error(e),
        }
    }
}

// Separate implementation for async-mode _op methods on async Camera
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    /// Set zoom to an absolute normalized position and return an operation handle.
    pub async fn zoom_absolute_op(
        &self,
        position: Normalized,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::Zoom, Self>, Error>
    {
        use crate::command::zoom::Zoom;

        let zoom_pos = crate::types::ZoomPosition::try_from(*position.value())?;
        let cmd = Zoom::Position(zoom_pos);

        let (id, _response_fut) = self.send_command_with_id(&cmd).await?;
        Ok(crate::camera::inflight::InFlight::new(id, self))
    }
}
