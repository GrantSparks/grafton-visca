//! Zoom control implementation for PTZ cameras.
//!
//! This module provides comprehensive zoom control functionality including:
//! - Standard speed zoom operations (tele/wide)
//! - Variable speed zoom control with fine-grained speed levels
//! - Marker-gated absolute zoom positioning
//! - Marker-gated digital zoom enable/disable for supported profiles
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
}

/// Direct absolute zoom positioning for profiles with source-backed support.
#[grafton_visca_macros::delegate_to_session]
pub trait DirectZoomControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set zoom to an absolute position.
    ///
    /// Values are validated against the selected profile's supported optical
    /// or optical-plus-digital zoom range before any command is encoded.
    fn set_zoom<T>(&self, position: T) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>
    where
        T: TryInto<ZoomPosition>,
        T::Error: Into<Error>;
}

/// VISCA digital zoom toggle for profiles that document the enable/disable opcode.
#[grafton_visca_macros::delegate_to_session]
pub trait DigitalZoomControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set digital zoom on or off.
    fn set_digital_zoom(&self, enabled: bool) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Absolute zoom positioning across a documented optical-plus-digital range.
#[grafton_visca_macros::delegate_to_session]
pub trait DigitalZoomRangeControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set zoom to an absolute normalized position with domain awareness.
    ///
    /// This method provides domain-aware zoom control, allowing you to specify
    /// whether the normalized position should map to the optical zoom range only
    /// or include digital zoom as well.
    ///
    /// # Arguments
    /// * `position` - UnitInterval position (0.0 = wide, 1.0 = full telephoto for the domain)
    /// * `domain` - The zoom domain to use (Optical or OpticalPlusDigital)
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::{ZoomDomain, UnitInterval};
    ///
    /// // Set to 50% of optical zoom range
    /// camera.zoom_absolute_normalized(
    ///     UnitInterval::new(0.5)?,
    ///     ZoomDomain::Optical
    /// )?;
    ///
    /// // Set to 75% of full zoom range (including digital)
    /// camera.zoom_absolute_normalized(
    ///     UnitInterval::new(0.75)?,
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
        position: crate::UnitInterval,
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

pub(crate) fn zoom_position_command<P, T>(position: T) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
    T: TryInto<ZoomPosition>,
    T::Error: Into<Error>,
{
    use crate::capabilities::zoom::ZoomExt;

    let position = position.try_into().map_err(Into::into)?;
    P::default().validate_zoom_position(position.value())?;
    Ok(ZoomCommand::Position(position))
}

pub(crate) fn zoom_absolute_normalized_command<P>(
    position: crate::UnitInterval,
    domain: crate::ZoomDomain,
) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDigitalZoomRange,
{
    use crate::ZoomPositionExt;

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
}

// Direct absolute zoom positioning for profiles with source-backed support.
impl<M, P, Tr, Exec> DirectZoomControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDirectZoom,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_zoom<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        T: TryInto<ZoomPosition>,
        T::Error: Into<Error>,
    {
        match zoom_position_command::<P, T>(position) {
            Ok(command) => self.execute(command),
            Err(e) => self.error(e),
        }
    }
}

impl<M, P, Tr, Exec> DigitalZoomControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDigitalZoomToggle,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_digital_zoom(&self, enabled: bool) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::zoom::DigitalZoom;
        self.execute(DigitalZoom::new(enabled))
    }
}

impl<M, P, Tr, Exec> DigitalZoomRangeControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDigitalZoomRange,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn zoom_absolute_normalized(
        &self,
        position: crate::UnitInterval,
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
    pub(crate) async fn start_zoom_operation(
        &self,
        command: ZoomCommand,
    ) -> Result<crate::camera::InFlight<'_, crate::camera::ZoomOperation, P, Exec>, Error> {
        let (id, response_future) = self.start_command_with_id(&command).await?;
        Ok(crate::camera::InFlight::new(
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
    ) -> Result<crate::camera::InFlight<'_, crate::camera::ZoomOperation, P, Exec>, Error> {
        self.start_zoom_operation(zoom_tele_command(speed)).await
    }

    /// Start zooming out and return an operation handle.
    #[cfg(feature = "dyn-api")]
    pub(crate) async fn zoom_wide_op(
        &self,
        speed: Option<ZoomSpeed>,
    ) -> Result<crate::camera::InFlight<'_, crate::camera::ZoomOperation, P, Exec>, Error> {
        self.start_zoom_operation(zoom_wide_command(speed)).await
    }
}

#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDirectZoom,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor + Send + Sync + Clone + 'static,
{
    /// Set zoom to a position and return an operation handle.
    pub async fn set_zoom_op<T>(
        &self,
        position: T,
    ) -> Result<crate::camera::InFlight<'_, crate::camera::ZoomOperation, P, Exec>, Error>
    where
        T: TryInto<ZoomPosition>,
        T::Error: Into<Error>,
    {
        self.start_zoom_operation(zoom_position_command::<P, T>(position)?)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::zoom_position_command;
    use crate::{
        camera::profiles::PtzOpticsG2, capabilities::ValidationError, types::ZoomPosition, Error,
    };

    #[test]
    fn profile_zoom_position_command_rejects_out_of_profile_digital_range() {
        let result = zoom_position_command::<PtzOpticsG2, _>(ZoomPosition::MAX);

        assert!(matches!(
            result,
            Err(Error::ValidationError(ValidationError::OutOfRange {
                parameter: "zoom position",
                ..
            }))
        ));
    }
}
