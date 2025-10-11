//! ND filter control implementation for PTZ cameras.
//!
//! This module provides neutral density (ND) filter control functionality including:
//! - ND filter mode selection (preset or variable)
//! - Direct value setting for precise control
//! - Stop-based setting for photographic convenience
//! - Step-based adjustment for incremental changes
//! - Automatic ND filter operation
//! - Current filter status inquiry
//!
//! ND filters reduce the amount of light entering the camera without affecting
//! color rendition. They are essential for maintaining proper exposure in bright
//! conditions while using desired aperture and shutter speed settings.
//! This feature is typically found on professional cameras like the Sony FR7.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::{
        inquiry_structs::NdFilterInquiry,
        nd_filter::{AutoNdCommand, NdFilterModeCommand, NdFilterStepCommand, NdFilterValue},
        NdFilterMode as CommandNdFilterMode, NdFilterStep,
    },
    mode::Mode,
    Error,
};

/// ND filter operations for PTZ cameras.
///
/// This trait provides comprehensive ND filter control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # ND Filter Concepts
///
/// - **Neutral Density**: Reduces light without affecting color
/// - **Stops**: Measurement of light reduction (1 stop = 50% light reduction)
/// - **Variable ND**: Continuously adjustable density
/// - **Preset ND**: Fixed density positions (e.g., 1/4, 1/8, 1/16)
/// - **Auto ND**: Automatic adjustment based on lighting conditions
///
/// # Common Use Cases
///
/// - Outdoor shooting in bright sunlight
/// - Maintaining shallow depth of field in bright light
/// - Achieving slower shutter speeds for motion blur effects
/// - Preventing overexposure when zoom or lighting changes
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.set_nd_filter_stops(3.0)?;  // 3-stop ND filter
/// camera.step_nd_filter(NdFilterStep::Up)?;  // Increase density
/// camera.set_auto_nd(true)?;  // Enable auto ND
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.set_nd_filter_stops(3.0).await?;  // 3-stop ND filter
/// camera.step_nd_filter(NdFilterStep::Up).await?;  // Increase density
/// camera.set_auto_nd(true).await?;  // Enable auto ND
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait NdFilterControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set ND filter mode (preset or variable).
    ///
    /// Configures whether the ND filter operates in preset mode
    /// (fixed positions) or variable mode (continuous adjustment).
    ///
    /// # Parameters
    /// - `mode`: The ND filter mode to set
    ///
    /// # Errors
    /// Returns an error if the command fails or the camera doesn't support ND filters.
    fn set_nd_filter_mode(
        &self,
        mode: CommandNdFilterMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set ND filter value directly (for variable mode).
    ///
    /// Sets the ND filter to a specific numeric value. This provides
    /// precise control when in variable mode.
    ///
    /// # Parameters
    /// - `value`: The ND filter value to set (range depends on camera model)
    ///
    /// # Errors
    /// Returns an error if the value is out of range or the command fails.
    fn set_nd_filter_value(&self, value: u16) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    ///
    /// Sets the ND filter using photographic stop values, which is more
    /// intuitive for exposure calculations. Each stop halves the light.
    ///
    /// # Parameters
    /// - `stops`: The number of stops of light reduction (2.0 to 7.0)
    ///
    /// # Examples
    /// - 2.0 stops = 1/4 light (75% reduction)
    /// - 3.0 stops = 1/8 light (87.5% reduction)
    /// - 6.0 stops = 1/64 light (98.4% reduction)
    ///
    /// # Errors
    /// Returns an error if the stops value is out of range or the command fails.
    fn set_nd_filter_stops(&self, stops: f32) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Step ND filter up or down.
    ///
    /// Adjusts the ND filter in incremental steps. Up increases density
    /// (more light reduction), down decreases density (less light reduction).
    ///
    /// # Parameters
    /// - `direction`: The direction to step (Up or Down)
    ///
    /// # Errors
    /// Returns an error if the command fails or the filter is at its limit.
    fn step_nd_filter(
        &self,
        direction: NdFilterStep,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable or disable auto ND.
    ///
    /// Controls automatic ND filter adjustment based on lighting conditions.
    /// When enabled, the camera automatically adjusts the ND filter to
    /// maintain proper exposure.
    ///
    /// # Parameters
    /// - `enabled`: True to enable auto ND, false to disable
    ///
    /// # Errors
    /// Returns an error if the command fails or auto ND is not supported.
    fn set_auto_nd(&self, enabled: bool) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Get current ND filter setting.
    ///
    /// Returns the current ND filter position or value.
    /// The interpretation depends on the current mode (preset vs variable).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or ND filters are not supported.
    fn nd_filter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> NdFilterControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::nd_filter::NdFilter,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_nd_filter_mode(&self, mode: CommandNdFilterMode) -> M::Fut<'_, Result<(), Error>> {
        let cmd = NdFilterModeCommand::new(mode);
        self.execute(cmd)
    }

    fn set_nd_filter_value(&self, value: u16) -> M::Fut<'_, Result<(), Error>> {
        match NdFilterValue::new(value) {
            Ok(cmd) => self.execute(cmd),
            Err(_) => self.error(Error::InvalidParameter {
                parameter: "value",
                value: value.to_string().into(),
                reason: "ND filter value out of range".into(),
            }),
        }
    }

    fn set_nd_filter_stops(&self, stops: f32) -> M::Fut<'_, Result<(), Error>> {
        match NdFilterValue::from_stops(stops) {
            Ok(cmd) => self.execute(cmd),
            Err(_) => self.error(Error::InvalidParameter {
                parameter: "stops",
                value: stops.to_string().into(),
                reason: "ND filter stops must be between 2.0 and 7.0".into(),
            }),
        }
    }

    fn step_nd_filter(&self, direction: NdFilterStep) -> M::Fut<'_, Result<(), Error>> {
        let cmd = NdFilterStepCommand::new(direction);
        self.execute(cmd)
    }

    fn set_auto_nd(&self, enabled: bool) -> M::Fut<'_, Result<(), Error>> {
        let cmd = AutoNdCommand::new(enabled);
        self.execute(cmd)
    }

    fn nd_filter(&self) -> M::Fut<'_, Result<u8, Error>> {
        self.query(NdFilterInquiry)
    }
}
