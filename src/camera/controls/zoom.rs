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

use crate::{
    camera::ViscaClient,
    command::zoom::Zoom as ZoomCommand,
    mode::Mode,
    types::{ZoomPosition, ZoomSpeed},
    Error,
};

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

    /// Set zoom to an absolute position.
    ///
    /// This method accepts any type that can be converted to `ZoomPosition`, providing
    /// a flexible API for setting zoom using different units:
    ///
    /// - `Percentage(50.0)` - Set zoom to 50% of range
    /// - `Normalized(0.5)` - Set zoom to 0.5 (equivalent to 50%)
    /// - `Magnification(10.0)` - Set zoom to 10x magnification
    /// - `Raw(0x4000)` - Set zoom to raw VISCA value
    /// - `ZoomPosition` - Set zoom to specific position directly
    ///
    /// # Arguments
    /// * `position` - Target zoom position (accepts multiple types via `TryInto<ZoomPosition>`)
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Magnification, Normalized, Raw};
    ///
    /// // Using percentage
    /// camera.set_zoom(Percentage(50.0))?;
    ///
    /// // Using magnification
    /// camera.set_zoom(Magnification(10.0))?;
    ///
    /// // Using normalized value
    /// camera.set_zoom(Normalized(0.5))?;
    ///
    /// // Using raw value
    /// camera.set_zoom(Raw(0x4000_u16))?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The conversion to `ZoomPosition` fails (e.g., value out of range)
    /// - The command fails to send or receive a response
    fn set_zoom<T>(&self, position: T) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>
    where
        T: TryInto<ZoomPosition>,
        T::Error: Into<Error>;

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

fn zoom_tele_command(speed: Option<ZoomSpeed>) -> ZoomCommand {
    match speed {
        None => ZoomCommand::TeleStd,
        Some(speed) => ZoomCommand::TeleVariable(speed),
    }
}

fn zoom_wide_command(speed: Option<ZoomSpeed>) -> ZoomCommand {
    match speed {
        None => ZoomCommand::WideStd,
        Some(speed) => ZoomCommand::WideVariable(speed),
    }
}

fn zoom_position_command<T>(position: T) -> Result<ZoomCommand, Error>
where
    T: TryInto<ZoomPosition>,
    T::Error: Into<Error>,
{
    position
        .try_into()
        .map(ZoomCommand::Position)
        .map_err(Into::into)
}

fn zoom_absolute_normalized_command<P>(
    position: crate::Normalized,
    domain: crate::ZoomDomain,
) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
{
    use crate::ZoomPositionExt;

    if domain == crate::ZoomDomain::OpticalPlusDigital && P::DIGITAL_ZOOM_MAX.is_none() {
        return Err(Error::FeatureNotSupported {
            feature: "Digital zoom",
        });
    }

    ZoomPosition::from_normalized(position, domain, P::OPTICAL_ZOOM_MAX, P::DIGITAL_ZOOM_MAX)
        .map(ZoomCommand::Position)
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
        self.execute(ZoomCommand::Stop)
    }

    fn zoom_tele(&self, speed: Option<ZoomSpeed>) -> M::Fut<'_, Result<(), Error>> {
        self.execute(zoom_tele_command(speed))
    }

    fn zoom_wide(&self, speed: Option<ZoomSpeed>) -> M::Fut<'_, Result<(), Error>> {
        self.execute(zoom_wide_command(speed))
    }

    fn set_zoom<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        T: TryInto<ZoomPosition>,
        T::Error: Into<Error>,
    {
        match zoom_position_command(position) {
            Ok(command) => self.execute(command),
            Err(e) => self.error(e),
        }
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
        match zoom_absolute_normalized_command::<P>(position, domain) {
            Ok(command) => self.execute(command),
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
    Exec: crate::executor::Executor + Send + Sync + Clone + 'static,
{
    async fn start_zoom_operation(
        &self,
        command: ZoomCommand,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::Zoom, P, Exec>, Error>
    {
        let (id, response_future) = self.start_command_with_id(&command).await?;
        Ok(crate::camera::inflight::InFlight::new(
            id,
            self.camera_id(),
            self.runtime(),
            response_future,
        ))
    }

    /// Start zooming in and return an operation handle.
    #[cfg(feature = "dyn-api")]
    pub(crate) async fn zoom_tele_op(
        &self,
        speed: Option<ZoomSpeed>,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::Zoom, P, Exec>, Error>
    {
        self.start_zoom_operation(zoom_tele_command(speed)).await
    }

    /// Start zooming out and return an operation handle.
    #[cfg(feature = "dyn-api")]
    pub(crate) async fn zoom_wide_op(
        &self,
        speed: Option<ZoomSpeed>,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::Zoom, P, Exec>, Error>
    {
        self.start_zoom_operation(zoom_wide_command(speed)).await
    }

    /// Set zoom to a position and return an operation handle.
    ///
    /// This method accepts any type that can be converted to `ZoomPosition`, providing
    /// a flexible API for setting zoom using different units. Returns an `InFlight`
    /// handle for fine-grained control over timeouts and cancellation.
    ///
    /// # Arguments
    /// * `position` - Target zoom position (accepts multiple types via `TryInto<ZoomPosition>`)
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Magnification};
    /// use std::time::Duration;
    ///
    /// // Using percentage
    /// let handle = camera.set_zoom_op(Percentage(50.0)).await?;
    /// handle.await_completion(Duration::from_secs(5)).await?;
    ///
    /// // Using magnification
    /// let handle = camera.set_zoom_op(Magnification(10.0)).await?;
    /// handle.await_completion(Duration::from_secs(5)).await?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The conversion to `ZoomPosition` fails (e.g., value out of range)
    /// - The command fails to send
    pub async fn set_zoom_op<T>(
        &self,
        position: T,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::Zoom, P, Exec>, Error>
    where
        T: TryInto<ZoomPosition>,
        T::Error: Into<Error>,
    {
        self.start_zoom_operation(zoom_position_command(position)?)
            .await
    }

    /// Set zoom to an absolute normalized position and return an operation handle.
    #[cfg(feature = "dyn-api")]
    pub(crate) async fn zoom_absolute_normalized_op(
        &self,
        position: crate::Normalized,
        domain: crate::ZoomDomain,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::Zoom, P, Exec>, Error>
    {
        self.start_zoom_operation(zoom_absolute_normalized_command::<P>(position, domain)?)
            .await
    }
}
