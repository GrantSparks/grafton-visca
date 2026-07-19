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
    Error, UnitInterval, ZoomDomain,
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

    /// Set zoom to an absolute raw VISCA position.
    ///
    /// Values are validated against the selected profile's supported optical
    /// or optical-plus-digital zoom range before any command is encoded.
    fn set_zoom(&self, position: ZoomPosition) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set zoom to a normalized optical position.
    ///
    /// This always maps `0.0..=1.0` across the selected profile's documented
    /// optical zoom range.
    fn set_zoom_normalized(
        &self,
        position: UnitInterval,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
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

    /// Set zoom to a normalized position in a documented zoom domain.
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
    /// camera.set_zoom_normalized_in_domain(
    ///     UnitInterval::new(0.5)?,
    ///     ZoomDomain::Optical
    /// )?;
    ///
    /// // Set to 75% of full zoom range (including digital)
    /// camera.set_zoom_normalized_in_domain(
    ///     UnitInterval::new(0.75)?,
    ///     ZoomDomain::OpticalPlusDigital
    /// )?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support digital zoom and OpticalPlusDigital domain is specified
    /// - The command fails to send or receive a response
    fn set_zoom_normalized_in_domain(
        &self,
        position: UnitInterval,
        domain: ZoomDomain,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

fn validate_zoom_speed<P>(speed: ZoomSpeed) -> Result<(), Error>
where
    P: crate::capabilities::zoom::Zoom,
{
    if P::ZOOM_SPEED_RANGE.contains(speed.value()) {
        Ok(())
    } else {
        Err(crate::capabilities::ValidationError::OutOfRange {
            parameter: "zoom speed",
            value: f64::from(speed.value()),
            min: f64::from(P::ZOOM_SPEED_RANGE.min()),
            max: f64::from(P::ZOOM_SPEED_RANGE.max()),
        }
        .into())
    }
}

pub(crate) fn zoom_tele_command<P>(speed: Option<ZoomSpeed>) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::zoom::Zoom,
{
    match speed {
        None => Ok(ZoomCommand::TeleStd),
        Some(speed) => {
            validate_zoom_speed::<P>(speed)?;
            Ok(ZoomCommand::TeleVariable(speed))
        }
    }
}

pub(crate) fn zoom_wide_command<P>(speed: Option<ZoomSpeed>) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::zoom::Zoom,
{
    match speed {
        None => Ok(ZoomCommand::WideStd),
        Some(speed) => {
            validate_zoom_speed::<P>(speed)?;
            Ok(ZoomCommand::WideVariable(speed))
        }
    }
}

pub(crate) fn zoom_position_command<P>(position: ZoomPosition) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
{
    use crate::capabilities::zoom::ZoomExt;

    P::default().validate_zoom_position(position.value())?;
    Ok(ZoomCommand::Position(position))
}

pub(crate) fn zoom_normalized_command<P>(position: UnitInterval) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
{
    zoom_from_normalized_for_profile::<P>(position, ZoomDomain::Optical)
        .and_then(zoom_position_command::<P>)
}

pub(crate) fn zoom_normalized_in_domain_command<P>(
    position: UnitInterval,
    domain: ZoomDomain,
) -> Result<ZoomCommand, Error>
where
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDigitalZoomRange,
{
    zoom_from_normalized_for_profile::<P>(position, domain).and_then(zoom_position_command::<P>)
}

pub(crate) fn zoom_from_normalized_for_profile<P>(
    position: UnitInterval,
    domain: ZoomDomain,
) -> Result<ZoomPosition, Error>
where
    P: crate::capabilities::Profile + crate::capabilities::zoom::Zoom,
{
    crate::zoom_from_normalized(position, domain, P::OPTICAL_ZOOM_MAX, P::DIGITAL_ZOOM_MAX)
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
        self.execute_operation(ZoomCommand::Stop)
    }

    fn zoom_tele(&self, speed: Option<ZoomSpeed>) -> M::Fut<'_, Result<(), Error>> {
        match zoom_tele_command::<P>(speed) {
            Ok(command) => self.execute_operation(command),
            Err(err) => self.error(err),
        }
    }

    fn zoom_wide(&self, speed: Option<ZoomSpeed>) -> M::Fut<'_, Result<(), Error>> {
        match zoom_wide_command::<P>(speed) {
            Ok(command) => self.execute_operation(command),
            Err(err) => self.error(err),
        }
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

    fn set_zoom(&self, position: ZoomPosition) -> M::Fut<'_, Result<(), Error>> {
        match zoom_position_command::<P>(position) {
            Ok(command) => self.execute_operation(command),
            Err(e) => self.error(e),
        }
    }

    fn set_zoom_normalized(&self, position: UnitInterval) -> M::Fut<'_, Result<(), Error>> {
        match zoom_normalized_command::<P>(position) {
            Ok(command) => self.execute_operation(command),
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

    fn set_zoom_normalized_in_domain(
        &self,
        position: UnitInterval,
        domain: ZoomDomain,
    ) -> M::Fut<'_, Result<(), Error>> {
        match zoom_normalized_in_domain_command::<P>(position, domain) {
            Ok(command) => self.execute_operation(command),
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
        let metadata =
            crate::command::ViscaCommand::operation_metadata(&command).ok_or_else(|| {
                Error::InvalidState("built-in zoom operation is missing metadata".into())
            })?;
        self.submit_op::<crate::camera::ZoomOperation, _>(&command, metadata)
            .await
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
    /// Set zoom to a raw VISCA position and return an operation handle.
    #[deprecated(
        since = "1.1.0",
        note = "use `submit(cmd)` and drive the returned handle with `await_applied` / `await_settled`; the `_op` methods are removed in 2.0"
    )]
    pub async fn set_zoom_op(
        &self,
        position: ZoomPosition,
    ) -> Result<crate::camera::InFlight<'_, crate::camera::ZoomOperation, P, Exec>, Error> {
        self.start_zoom_operation(zoom_position_command::<P>(position)?)
            .await
    }

    /// Set zoom to a normalized optical position and return an operation handle.
    #[deprecated(
        since = "1.1.0",
        note = "use `submit(cmd)` and drive the returned handle with `await_applied` / `await_settled`; the `_op` methods are removed in 2.0"
    )]
    pub async fn set_zoom_normalized_op(
        &self,
        position: UnitInterval,
    ) -> Result<crate::camera::InFlight<'_, crate::camera::ZoomOperation, P, Exec>, Error> {
        self.start_zoom_operation(zoom_normalized_command::<P>(position)?)
            .await
    }
}

#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + Default
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::HasDirectZoom
        + crate::capabilities::HasDigitalZoomRange,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor + Send + Sync + Clone + 'static,
{
    /// Set zoom to a normalized position in a documented domain and return an operation handle.
    #[deprecated(
        since = "1.1.0",
        note = "use `submit(cmd)` and drive the returned handle with `await_applied` / `await_settled`; the `_op` methods are removed in 2.0"
    )]
    pub async fn set_zoom_normalized_in_domain_op(
        &self,
        position: UnitInterval,
        domain: ZoomDomain,
    ) -> Result<crate::camera::InFlight<'_, crate::camera::ZoomOperation, P, Exec>, Error> {
        self.start_zoom_operation(zoom_normalized_in_domain_command::<P>(position, domain)?)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        zoom_normalized_command, zoom_normalized_in_domain_command, zoom_position_command,
    };
    use crate::{
        camera::profiles::{PtzOpticsG2, SonyFR7},
        capabilities::ValidationError,
        command::{ViscaCommand, VISCA_TERMINATOR},
        types::ZoomPosition,
        CameraId, Error, UnitInterval, ZoomDomain,
    };

    #[test]
    fn profile_zoom_position_command_rejects_out_of_profile_digital_range() {
        let result = zoom_position_command::<PtzOpticsG2>(ZoomPosition::MAX);

        assert!(matches!(
            result,
            Err(Error::ValidationError(ValidationError::OutOfRange {
                parameter: "zoom position",
                ..
            }))
        ));
    }

    #[test]
    fn ptzoptics_g2_optical_normalized_half_encodes_profile_max_half() -> Result<(), Error> {
        let command = zoom_normalized_command::<PtzOpticsG2>(UnitInterval::new(0.5)?)?;
        let mut buffer = [0; 16];
        let len = command.write_into(CameraId::CAMERA_1, &mut buffer)?;

        assert_eq!(
            &buffer[..len],
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x02,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
        Ok(())
    }

    #[test]
    fn ptzoptics_g2_optical_normalized_one_encodes_optical_max() -> Result<(), Error> {
        let command = zoom_normalized_command::<PtzOpticsG2>(UnitInterval::ONE)?;
        let mut buffer = [0; 16];
        let len = command.write_into(CameraId::CAMERA_1, &mut buffer)?;

        assert_eq!(
            &buffer[..len],
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x04,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
        Ok(())
    }

    #[test]
    fn sony_fr7_digital_domain_normalized_one_encodes_digital_max() -> Result<(), Error> {
        let command = zoom_normalized_in_domain_command::<SonyFR7>(
            UnitInterval::ONE,
            ZoomDomain::OpticalPlusDigital,
        )?;
        let mut buffer = [0; 16];
        let len = command.write_into(CameraId::CAMERA_1, &mut buffer)?;

        assert_eq!(
            &buffer[..len],
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x07,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
        Ok(())
    }
}
